//! Interactive split-screen editor for `shortcuts.yml` (`shortcutedit` / `menuedit --shortcuts`).

use anyhow::Result;
use bashmenu_core::{
    interpolate_placeholders, load_config, load_shortcuts, load_themes, normalize_key_spec,
    resolve_app_path, save_shortcuts, validate_key_spec, BashMenuConfig, ExecutionMode,
    MacroContext, MenuItemType, ShortcutEntry, ShortcutsConfig, ToastNotification,
    PLACEHOLDER_HELP_TEXT,
};
use bashmenu_tui::{
    key_event_to_token, render_modal_frame, visible_text_len, wrap_prompt_text,
    wrap_spread_help_gutter_layout, ConfirmChoice, ConfirmModal, FilePickerModal, HelpModal,
    ResolvedThemeStyles, TerminalGuard, TextInput, ToastRack,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Core state for the `shortcutedit` TUI, testable headlessly.
#[derive(Debug, Clone)]
pub struct ShortcutEditorState {
    pub config: ShortcutsConfig,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub dirty: bool,
}

impl ShortcutEditorState {
    pub fn new(config: ShortcutsConfig) -> Self {
        Self {
            config,
            selected_idx: 0,
            scroll_offset: 0,
            dirty: false,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_idx + 1 < self.config.shortcuts.len() {
            self.selected_idx += 1;
        }
    }

    pub fn move_item_up(&mut self) -> bool {
        if self.selected_idx > 0 && self.selected_idx < self.config.shortcuts.len() {
            self.config
                .shortcuts
                .swap(self.selected_idx, self.selected_idx - 1);
            self.selected_idx -= 1;
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn move_item_down(&mut self) -> bool {
        if self.selected_idx + 1 < self.config.shortcuts.len() {
            self.config
                .shortcuts
                .swap(self.selected_idx, self.selected_idx + 1);
            self.selected_idx += 1;
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn toggle_selected(&mut self) -> Option<bool> {
        if let Some(entry) = self.config.shortcuts.get_mut(self.selected_idx) {
            entry.enabled = !entry.enabled;
            self.dirty = true;
            return Some(entry.enabled);
        }
        None
    }

    pub fn add_shortcut(&mut self, entry: ShortcutEntry) {
        if self.config.shortcuts.is_empty() {
            self.config.shortcuts.push(entry);
            self.selected_idx = 0;
        } else {
            let insert_at = (self.selected_idx + 1).min(self.config.shortcuts.len());
            self.config.shortcuts.insert(insert_at, entry);
            self.selected_idx = insert_at;
        }
        self.dirty = true;
    }

    pub fn delete_selected(&mut self) -> Option<ShortcutEntry> {
        if self.selected_idx < self.config.shortcuts.len() {
            let removed = self.config.shortcuts.remove(self.selected_idx);
            if self.selected_idx >= self.config.shortcuts.len() && !self.config.shortcuts.is_empty()
            {
                self.selected_idx = self.config.shortcuts.len() - 1;
            }
            self.dirty = true;
            return Some(removed);
        }
        None
    }

    pub fn selected_entry(&self) -> Option<&ShortcutEntry> {
        self.config.shortcuts.get(self.selected_idx)
    }

    pub fn selected_entry_mut(&mut self) -> Option<&mut ShortcutEntry> {
        self.config.shortcuts.get_mut(self.selected_idx)
    }
}

/// Formats the right-hand Property Inspector lines for the currently selected shortcut.
pub fn format_shortcut_inspector(
    state: &ShortcutEditorState,
    app_cfg: &BashMenuConfig,
    avail_w: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let rule = "─".repeat(avail_w.clamp(10, 60));

    lines.push("Global Leader Configuration".to_string());
    lines.push(rule.clone());
    lines.push(format!("  Leader Key      : {}", state.config.leader_key));
    lines.push(format!(
        "  Chord Timeout   : {} ms",
        state.config.leader_timeout_ms
    ));
    lines.push(format!(
        "  Popup Hint      : {}",
        if state.config.show_leader_hint {
            "Enabled (which-key overlay)"
        } else {
            "Disabled"
        }
    ));
    lines.push(String::new());

    let Some(entry) = state.selected_entry() else {
        lines.push("No shortcut selected. Press [a] to add a shortcut.".to_string());
        return lines;
    };

    lines.push("Selected Shortcut Properties".to_string());
    lines.push(rule.clone());
    lines.push(format!("  Description     : {}", entry.description));
    lines.push(format!("  Key / Chord     : {}", entry.normalized_key()));
    lines.push(format!(
        "  Trigger Kind    : {}",
        if entry.leader_chord_key().is_some() {
            "Leader Chord (<leader> + key)"
        } else {
            "Direct Single-Stroke Combo"
        }
    ));
    lines.push(format!(
        "  Status          : {}",
        if entry.enabled { "Enabled" } else { "Disabled" }
    ));

    let type_str = match &entry.r#type {
        MenuItemType::Command => "command",
        MenuItemType::Script => "script",
        MenuItemType::Editor => "editor",
        MenuItemType::Confirm => "confirm",
        MenuItemType::Notebook => "notebook",
        MenuItemType::ThemeSelector => "theme_selector",
        MenuItemType::Other(s) => s.as_str(),
        _ => "command",
    };
    lines.push(format!(
        "  Item Type       : {} {}",
        type_str,
        entry.mode_badge()
    ));

    let raw_target = entry.target_summary();
    lines.push(format!("  Target (Raw)    : {}", raw_target));

    let ctx = MacroContext::new(app_cfg);
    let expanded = interpolate_placeholders(raw_target, &ctx);
    if expanded != raw_target {
        lines.push(format!("  Target (Expand) : {}", expanded));
    }

    if matches!(entry.r#type, MenuItemType::Command | MenuItemType::Script) {
        let mode = entry.effective_execution_mode();
        let mode_label = match mode {
            ExecutionMode::Console => "Console (Full Terminal)",
            ExecutionMode::Streaming => "Streaming (Floating Output Modal)",
            ExecutionMode::Interactive => "Interactive (Floating Stdin/Stdout Modal)",
        };
        lines.push(format!("  Execution Mode  : {}", mode_label));
        if matches!(mode, ExecutionMode::Streaming | ExecutionMode::Interactive) {
            lines.push(format!(
                "  Window Size %   : {}%",
                entry.window_percent.unwrap_or(65)
            ));
        } else {
            lines.push(format!(
                "  Quiet Mode      : {}",
                entry.quiet.unwrap_or(false)
            ));
            lines.push(format!(
                "  Alt Buffer      : {}",
                entry.alt_buffer.unwrap_or(true)
            ));
        }
    } else if entry.r#type == MenuItemType::Editor {
        lines.push(format!(
            "  Tabstop         : {}",
            entry.tabstop.unwrap_or(4)
        ));
    } else if matches!(entry.r#type, MenuItemType::Other(ref s) if s == "toast") {
        lines.push(format!(
            "  Toast Title     : {}",
            entry.title.as_deref().unwrap_or("(default)")
        ));
        lines.push(format!(
            "  Toast Timeout   : {:.1}s",
            entry.timeout.unwrap_or(2.0)
        ));
    }

    lines
}

/// Interactive modal for recording a direct key combo or `<leader> <key>` chord.
pub struct KeyCaptureModal {
    pub leader_key: String,
    pub captured_key: String,
    pub waiting_for_chord: bool,
    pub error_msg: Option<String>,
    pub is_leader_def: bool,
}

impl KeyCaptureModal {
    pub fn new(current_key: &str, leader_key: &str, is_leader_def: bool) -> Self {
        Self {
            leader_key: normalize_key_spec(leader_key),
            captured_key: normalize_key_spec(current_key),
            waiting_for_chord: false,
            error_msg: None,
            is_leader_def,
        }
    }

    /// Returns `Some(Some(new_key))` on confirm, `Some(None)` on cancel, or `None` to keep capturing.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Option<String>> {
        if !self.waiting_for_chord {
            if key.code == KeyCode::Esc {
                return Some(None);
            }
            if key.code == KeyCode::Enter {
                match validate_key_spec(&self.captured_key, self.is_leader_def) {
                    Ok(valid) => return Some(Some(valid)),
                    Err(err) => {
                        self.error_msg = Some(err);
                        return None;
                    }
                }
            }
        }

        let token = key_event_to_token(key)?;

        if self.waiting_for_chord {
            self.waiting_for_chord = false;
            if token.eq_ignore_ascii_case("esc") {
                self.error_msg = Some("Leader chord capture cancelled.".to_string());
                return None;
            }
            let candidate = format!("<leader> {}", token);
            match validate_key_spec(&candidate, false) {
                Ok(valid) => {
                    self.captured_key = valid;
                    self.error_msg = None;
                }
                Err(err) => {
                    self.error_msg = Some(err);
                }
            }
            return None;
        }

        // If capturing a shortcut (not the leader key itself) and user presses the leader key,
        // enter chord-waiting state!
        if !self.is_leader_def && token.eq_ignore_ascii_case(&self.leader_key) {
            self.waiting_for_chord = true;
            self.error_msg = None;
            return None;
        }

        match validate_key_spec(&token, self.is_leader_def) {
            Ok(valid) => {
                self.captured_key = valid;
                self.error_msg = None;
            }
            Err(err) => {
                // If user pressed a plain character while capturing a shortcut, offer helpful hint
                self.error_msg = Some(err);
            }
        }
        None
    }

    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let box_w = area.width.saturating_sub(8).clamp(46, 68);
        let box_h = 13;
        let title = if self.is_leader_def {
            "Capture Leader Key"
        } else {
            "Capture Shortcut Key / Chord"
        };
        let inner = render_modal_frame(area, box_w, box_h, title, styles, buf);
        if inner.width < 10 || inner.height < 6 {
            return;
        }

        let base_bg = styles.background;
        let text_style = styles.text.bg(base_bg);
        let bold_style = styles.shortcut_key.bg(base_bg).add_modifier(Modifier::BOLD);
        let hint_style = styles.help_text.bg(base_bg);
        let err_style = Style::default()
            .fg(Color::Red)
            .bg(base_bg)
            .add_modifier(Modifier::BOLD);

        let avail_w = inner.width.saturating_sub(2) as usize;

        let prompt = if self.is_leader_def {
            "Press desired Leader Key combo (e.g. Ctrl+Space, Ctrl+B, Alt+Space):".to_string()
        } else {
            format!(
                "Press a direct key (F9, Alt+K, Ctrl+G) OR press Leader ({}) then a key:",
                self.leader_key
            )
        };
        let wrapped_prompt = wrap_prompt_text(&prompt, avail_w);
        let mut cur_y = inner.y;
        for line in wrapped_prompt.iter().take(2) {
            let clipped: String = line.chars().take(avail_w).collect();
            buf.set_string(inner.x + 1, cur_y, &clipped, text_style);
            cur_y += 1;
        }

        cur_y = cur_y.max(inner.y + 2);

        let status_line = if self.waiting_for_chord {
            format!(
                "Leader ({}) pressed! Waiting for follow-up key...",
                self.leader_key
            )
        } else {
            format!("Captured Binding :  [ {} ]", self.captured_key)
        };
        let clipped_status: String = status_line.chars().take(avail_w).collect();
        buf.set_string(inner.x + 1, cur_y, &clipped_status, bold_style);
        cur_y += 2;

        if let Some(ref err) = self.error_msg {
            let wrapped_err = wrap_prompt_text(err, avail_w);
            for line in wrapped_err.iter().take(2) {
                if cur_y < inner.y + inner.height.saturating_sub(1) {
                    let clipped: String = line.chars().take(avail_w).collect();
                    buf.set_string(inner.x + 1, cur_y, &clipped, err_style);
                    cur_y += 1;
                }
            }
        } else {
            let hint = "Valid binding ready. Press [ENTER] to accept.";
            let clipped_hint: String = hint.chars().take(avail_w).collect();
            buf.set_string(inner.x + 1, cur_y, &clipped_hint, hint_style);
        }

        let footer = "[ENTER] Accept Binding    [ESC] Cancel";
        let fw = footer.len();
        let footer_text: String = if fw <= avail_w {
            footer.to_string()
        } else {
            let short_footer = "[ENTER] Accept  [ESC] Cancel";
            short_footer.chars().take(avail_w).collect()
        };
        let fx = inner.x + inner.width.saturating_sub(footer_text.len() as u16) / 2;
        buf.set_string(
            fx,
            inner.y + inner.height.saturating_sub(1),
            &footer_text,
            hint_style,
        );
    }
}

/// Modal for editing global leader key settings (`leader_key`, `leader_timeout_ms`, `show_leader_hint`).
pub struct LeaderSettingsModal {
    pub key_input: TextInput,
    pub timeout_input: TextInput,
    pub show_hint: bool,
    pub focus_idx: usize, // 0 = key, 1 = capture btn, 2 = timeout, 3 = show_hint, 4 = Save, 5 = Cancel
    pub error_msg: Option<String>,
}

impl LeaderSettingsModal {
    pub fn new(cfg: &ShortcutsConfig) -> Self {
        Self {
            key_input: TextInput::new(&cfg.leader_key),
            timeout_input: TextInput::new(&cfg.leader_timeout_ms.to_string()),
            show_hint: cfg.show_leader_hint,
            focus_idx: 0,
            error_msg: None,
        }
    }

    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let box_w = area.width.saturating_sub(8).clamp(48, 68);
        let box_h = 16;
        let inner = render_modal_frame(area, box_w, box_h, "Leader Key Settings", styles, buf);
        if inner.width < 15 || inner.height < 10 {
            return;
        }

        let base_bg = styles.background;
        let label_style = styles.text.bg(base_bg);
        let active_style = styles.text.bg(base_bg).add_modifier(Modifier::REVERSED);
        let hint_style = styles.help_text.bg(base_bg);
        let err_style = Style::default()
            .fg(Color::Red)
            .bg(base_bg)
            .add_modifier(Modifier::BOLD);

        let avail_w = inner.width.saturating_sub(2) as usize;
        let field_w = inner.width.saturating_sub(22).max(12);

        // 0: Leader Key Input
        buf.set_string(inner.x + 1, inner.y + 1, "Leader Key      :", label_style);
        draw_input(
            buf,
            Rect::new(inner.x + 19, inner.y + 1, field_w, 1),
            &self.key_input,
            styles,
            self.focus_idx == 0,
        );

        // 1: Capture Leader Key Button
        let cap_lbl = "[ Capture Key (Ctrl+K) ]";
        let cap_style = if self.focus_idx == 1 {
            active_style
        } else {
            styles.accent.bg(base_bg)
        };
        buf.set_string(inner.x + 19, inner.y + 3, cap_lbl, cap_style);

        // 2: Timeout ms
        buf.set_string(inner.x + 1, inner.y + 5, "Timeout (ms)    :", label_style);
        draw_input(
            buf,
            Rect::new(inner.x + 19, inner.y + 5, field_w, 1),
            &self.timeout_input,
            styles,
            self.focus_idx == 2,
        );

        // 3: Show Hint Toggle
        let chk = if self.show_hint { "[x]" } else { "[ ]" };
        let hint_lbl = format!("{} Show Compact Popup Overlay", chk);
        let tgl_style = if self.focus_idx == 3 {
            active_style
        } else {
            label_style
        };
        let clipped_hint: String = hint_lbl.chars().take(avail_w).collect();
        buf.set_string(inner.x + 1, inner.y + 7, &clipped_hint, tgl_style);

        if let Some(ref err) = self.error_msg {
            let wrapped_err = wrap_prompt_text(err, avail_w);
            for (idx, line) in wrapped_err.iter().take(2).enumerate() {
                let clipped: String = line.chars().take(avail_w).collect();
                buf.set_string(inner.x + 1, inner.y + 9 + idx as u16, &clipped, err_style);
            }
        }

        // Buttons (4 = Save, 5 = Cancel)
        let btn_y = inner.y + inner.height.saturating_sub(3);
        let save_lbl = "   [ Save ]   ";
        let cancel_lbl = "  [ Cancel ]  ";
        let save_style = if self.focus_idx == 4 {
            active_style
        } else {
            styles.shortcut_key.bg(base_bg)
        };
        let cancel_style = if self.focus_idx == 5 {
            active_style
        } else {
            styles.help_text.bg(base_bg)
        };
        let center_x = inner.x + inner.width / 2;
        buf.set_string(center_x.saturating_sub(16), btn_y, save_lbl, save_style);
        buf.set_string(center_x + 2, btn_y, cancel_lbl, cancel_style);

        // 2-line wrapped footer to ensure text NEVER exceeds inner width
        let f_line1 = "[Tab/Up/Down] Navigate | [Ctrl+K] Capture Key";
        let f_line2 = "[Enter] Select | [Esc] Cancel";
        let f1_clipped: String = f_line1.chars().take(avail_w).collect();
        let f2_clipped: String = f_line2.chars().take(avail_w).collect();
        let fx1 = inner.x + inner.width.saturating_sub(f1_clipped.len() as u16) / 2;
        let fx2 = inner.x + inner.width.saturating_sub(f2_clipped.len() as u16) / 2;
        buf.set_string(
            fx1,
            inner.y + inner.height.saturating_sub(2),
            &f1_clipped,
            hint_style,
        );
        buf.set_string(
            fx2,
            inner.y + inner.height.saturating_sub(1),
            &f2_clipped,
            hint_style,
        );
    }
}

/// Property editor modal for a single [`ShortcutEntry`].
pub struct ShortcutEditModal {
    pub description: TextInput,
    pub key_input: TextInput,
    pub use_leader: bool,
    pub item_type_idx: usize,
    pub target_input: TextInput,
    pub execution_mode: ExecutionMode,
    pub window_percent: TextInput,
    pub quiet: bool,
    pub alt_buffer: bool,
    pub enabled: bool,
    pub focus_idx: usize,
    pub error_msg: Option<String>,
}

const SHORTCUT_TYPES: &[(&str, MenuItemType)] = &[
    ("command", MenuItemType::Command),
    ("script", MenuItemType::Script),
    ("editor", MenuItemType::Editor),
    ("toast", MenuItemType::Other(String::new())), // mapped to Other("toast")
    ("notebook", MenuItemType::Notebook),
    ("theme_selector", MenuItemType::ThemeSelector),
];

impl ShortcutEditModal {
    pub fn from_entry(entry: &ShortcutEntry) -> Self {
        let norm_key = entry.normalized_key();
        let (use_leader, key_body) = if let Some(rest) = norm_key.strip_prefix("<leader> ") {
            (true, rest.to_string())
        } else {
            (false, norm_key)
        };

        let item_type_idx = match &entry.r#type {
            MenuItemType::Command => 0,
            MenuItemType::Script => 1,
            MenuItemType::Editor => 2,
            MenuItemType::Other(s) if s == "toast" => 3,
            MenuItemType::Notebook => 4,
            MenuItemType::ThemeSelector => 5,
            _ => 0,
        };

        let target_str = entry
            .action
            .as_deref()
            .or(entry.command.as_deref())
            .or(entry.script.as_deref())
            .or(entry.file.as_deref())
            .or(entry.message.as_deref())
            .unwrap_or_default();

        Self {
            description: TextInput::new(&entry.description),
            key_input: TextInput::new(&key_body),
            use_leader,
            item_type_idx,
            target_input: TextInput::new(target_str),
            execution_mode: entry.effective_execution_mode(),
            window_percent: TextInput::new(&entry.window_percent.unwrap_or(65).to_string()),
            quiet: entry.quiet.unwrap_or(false),
            alt_buffer: entry.alt_buffer.unwrap_or(true),
            enabled: entry.enabled,
            focus_idx: 0,
            error_msg: None,
        }
    }

    pub fn current_type(&self) -> MenuItemType {
        match self.item_type_idx {
            0 => MenuItemType::Command,
            1 => MenuItemType::Script,
            2 => MenuItemType::Editor,
            3 => MenuItemType::Other("toast".to_string()),
            4 => MenuItemType::Notebook,
            5 => MenuItemType::ThemeSelector,
            _ => MenuItemType::Command,
        }
    }

    pub fn composed_key(&self) -> String {
        let raw_owned = self.key_input.as_string();
        let raw = raw_owned.trim();
        if raw.starts_with("<leader>") {
            normalize_key_spec(raw)
        } else if self.use_leader {
            normalize_key_spec(&format!("<leader> {}", raw))
        } else {
            normalize_key_spec(raw)
        }
    }

    pub fn set_captured_key(&mut self, full_key: &str) {
        let norm = normalize_key_spec(full_key);
        if let Some(rest) = norm.strip_prefix("<leader> ") {
            self.use_leader = true;
            self.key_input.set_text(rest);
        } else {
            self.use_leader = false;
            self.key_input.set_text(&norm);
        }
        self.error_msg = None;
    }

    pub fn build_entry(
        &mut self,
        orig: &ShortcutEntry,
    ) -> std::result::Result<ShortcutEntry, String> {
        let desc = self.description.as_string().trim().to_string();
        if desc.is_empty() {
            return Err("Description cannot be empty.".to_string());
        }
        let full_key = self.composed_key();
        let valid_key = validate_key_spec(&full_key, false)?;

        let r#type = self.current_type();
        let target = self.target_input.as_string().trim().to_string();
        if target.is_empty() && r#type != MenuItemType::ThemeSelector {
            return Err("Target action / file / command cannot be empty.".to_string());
        }

        let pct = self
            .window_percent
            .as_string()
            .trim()
            .parse::<u16>()
            .unwrap_or(65)
            .clamp(20, 100);

        let mut entry = orig.clone();
        entry.key = valid_key;
        entry.description = desc;
        entry.r#type = r#type.clone();
        entry.enabled = self.enabled;

        // Assign canonical target fields
        entry.action = None;
        entry.command = None;
        entry.script = None;
        entry.file = None;
        entry.message = None;

        match r#type {
            MenuItemType::Editor | MenuItemType::Notebook => {
                entry.file = Some(target.clone());
                entry.action = Some(target);
                if r#type == MenuItemType::Editor && entry.tabstop.is_none() {
                    entry.tabstop = Some(4);
                }
            }
            MenuItemType::Script => {
                entry.script = Some(target.clone());
                entry.action = Some(target);
            }
            MenuItemType::Other(ref s) if s == "toast" => {
                entry.action = Some(target.clone());
                entry.message = Some(target);
                if entry.timeout.is_none() {
                    entry.timeout = Some(3.0);
                }
            }
            MenuItemType::ThemeSelector => {
                entry.action = Some("theme_selector".to_string());
            }
            _ => {
                entry.action = Some(target.clone());
                entry.command = Some(target);
            }
        }

        if matches!(entry.r#type, MenuItemType::Command | MenuItemType::Script) {
            entry.execution_mode = Some(self.execution_mode);
            if matches!(
                self.execution_mode,
                ExecutionMode::Streaming | ExecutionMode::Interactive
            ) {
                entry.window_percent = Some(pct);
            } else {
                entry.quiet = Some(self.quiet);
                entry.alt_buffer = Some(self.alt_buffer);
            }
        }

        Ok(entry)
    }

    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let box_w = area.width.saturating_sub(6).clamp(56, 76);
        let box_h = area.height.saturating_sub(2).clamp(22, 26);
        let inner = render_modal_frame(area, box_w, box_h, "Edit Keyboard Shortcut", styles, buf);
        if inner.width < 20 || inner.height < 14 {
            return;
        }

        let base_bg = styles.background;
        let label_style = styles.text.bg(base_bg);
        let active_style = styles.text.bg(base_bg).add_modifier(Modifier::REVERSED);
        let hint_style = styles.help_text.bg(base_bg);
        let err_style = Style::default()
            .fg(Color::Red)
            .bg(base_bg)
            .add_modifier(Modifier::BOLD);

        let avail_w = inner.width.saturating_sub(2) as usize;
        let field_x = inner.x + 19;
        let field_w = inner.width.saturating_sub(21).max(14);
        let mut y = inner.y;

        // 0: Description
        buf.set_string(inner.x + 1, y, "Description     :", label_style);
        draw_input(
            buf,
            Rect::new(field_x, y, field_w, 1),
            &self.description,
            styles,
            self.focus_idx == 0,
        );
        y += 2;

        // 1: Key Input + 2: Capture Key button
        buf.set_string(inner.x + 1, y, "Key / Chord     :", label_style);
        let key_w = field_w.saturating_sub(18).max(8);
        draw_input(
            buf,
            Rect::new(field_x, y, key_w, 1),
            &self.key_input,
            styles,
            self.focus_idx == 1,
        );
        let cap_lbl = "[ Capture (F2) ]";
        let cap_style = if self.focus_idx == 2 {
            active_style
        } else {
            styles.accent.bg(base_bg)
        };
        buf.set_string(field_x + key_w + 1, y, cap_lbl, cap_style);
        y += 2;

        // 3: Use <leader> prefix toggle
        let ldr_chk = if self.use_leader { "[x]" } else { "[ ]" };
        let ldr_str = format!(
            "{} Prefix with <leader>  (Effective: {})",
            ldr_chk,
            self.composed_key()
        );
        let ldr_clipped: String = ldr_str.chars().take(avail_w).collect();
        let ldr_style = if self.focus_idx == 3 {
            active_style
        } else {
            label_style
        };
        buf.set_string(inner.x + 1, y, &ldr_clipped, ldr_style);
        y += 2;

        // 4: Item Type selector
        let type_name = SHORTCUT_TYPES[self.item_type_idx].0;
        let type_str = if field_w >= 36 {
            format!("◄  {}  ►   (Left/Right/Space)", type_name)
        } else if field_w >= 22 {
            format!("◄  {}  ►", type_name)
        } else {
            format!("◄{}►", type_name)
        };
        let type_clipped: String = type_str.chars().take(field_w as usize).collect();
        buf.set_string(inner.x + 1, y, "Shortcut Type   :", label_style);
        let type_style = if self.focus_idx == 4 {
            active_style
        } else {
            styles.shortcut_key.bg(base_bg).add_modifier(Modifier::BOLD)
        };
        buf.set_string(field_x, y, &type_clipped, type_style);
        y += 2;

        // 5: Target Action / File + 6: Browse button
        let target_lbl = match self.current_type() {
            MenuItemType::Editor | MenuItemType::Notebook => "File Path       :",
            MenuItemType::Script => "Script Path     :",
            MenuItemType::Other(ref s) if s == "toast" => "Toast Message   :",
            _ => "Command / Action:",
        };
        buf.set_string(inner.x + 1, y, target_lbl, label_style);
        let tgt_w = field_w.saturating_sub(12).max(10);
        draw_input(
            buf,
            Rect::new(field_x, y, tgt_w, 1),
            &self.target_input,
            styles,
            self.focus_idx == 5,
        );
        let brw_lbl = "[ Browse ]";
        let brw_style = if self.focus_idx == 6 {
            active_style
        } else {
            styles.accent.bg(base_bg)
        };
        buf.set_string(field_x + tgt_w + 1, y, brw_lbl, brw_style);
        y += 2;

        // 7: Execution Mode
        buf.set_string(inner.x + 1, y, "Execution Mode  :", label_style);
        let c_mark = if self.execution_mode == ExecutionMode::Console {
            "(•)"
        } else {
            "( )"
        };
        let s_mark = if self.execution_mode == ExecutionMode::Streaming {
            "(•)"
        } else {
            "( )"
        };
        let i_mark = if self.execution_mode == ExecutionMode::Interactive {
            "(•)"
        } else {
            "( )"
        };
        let mode_line = if field_w >= 44 {
            format!(
                "{} Console   {} Streaming   {} Interactive",
                c_mark, s_mark, i_mark
            )
        } else if field_w >= 34 {
            format!("{} Console  {} Stream  {} Inter", c_mark, s_mark, i_mark)
        } else {
            format!("{} Con  {} Str  {} Int", c_mark, s_mark, i_mark)
        };
        let mode_clipped: String = mode_line.chars().take(field_w as usize).collect();
        let mode_style = if self.focus_idx == 7 {
            active_style
        } else {
            label_style
        };
        buf.set_string(field_x, y, &mode_clipped, mode_style);
        y += 2;

        // 8: Window Size % (or Quiet Mode) & 9: Alt Buffer & 10: Enabled
        buf.set_string(inner.x + 1, y, "Window Size %   :", label_style);
        draw_input(
            buf,
            Rect::new(field_x, y, 6, 1),
            &self.window_percent,
            styles,
            self.focus_idx == 8,
        );
        let pct_hint_avail = field_w.saturating_sub(8) as usize;
        let pct_hint = if pct_hint_avail >= 35 {
            "(20-100%, Streaming/Interactive)"
        } else if pct_hint_avail >= 12 {
            "(20-100%)"
        } else {
            ""
        };
        let pct_clipped: String = pct_hint.chars().take(pct_hint_avail).collect();
        buf.set_string(field_x + 8, y, &pct_clipped, hint_style);
        y += 2;

        // 9: Quiet, 10: Alt Buffer, 11: Enabled checkboxes
        let q_chk = if self.quiet { "[x]" } else { "[ ]" };
        let alt_chk = if self.alt_buffer { "[x]" } else { "[ ]" };
        let en_chk = if self.enabled { "[x]" } else { "[ ]" };

        let q_lbl = if avail_w >= 54 {
            format!("{} Quiet Mode", q_chk)
        } else {
            format!("{} Quiet", q_chk)
        };
        let alt_lbl = format!("{} Alt Buffer", alt_chk);
        let en_lbl = format!("{} Enabled", en_chk);

        let q_x = inner.x + 1;
        let alt_x = q_x + q_lbl.len() as u16 + 2;
        let en_x = alt_x + alt_lbl.len() as u16 + 2;

        buf.set_string(
            q_x,
            y,
            &q_lbl,
            if self.focus_idx == 9 {
                active_style
            } else {
                label_style
            },
        );
        buf.set_string(
            alt_x,
            y,
            &alt_lbl,
            if self.focus_idx == 10 {
                active_style
            } else {
                label_style
            },
        );
        buf.set_string(
            en_x,
            y,
            &en_lbl,
            if self.focus_idx == 11 {
                active_style
            } else {
                label_style
            },
        );
        y += 2;

        if let Some(ref err) = self.error_msg {
            let wrapped_err = wrap_prompt_text(err, avail_w);
            for (idx, line) in wrapped_err.iter().take(2).enumerate() {
                let clipped: String = line.chars().take(avail_w).collect();
                buf.set_string(inner.x + 1, y + idx as u16, &clipped, err_style);
            }
        }

        // 12: Save, 13: Cancel
        let btn_y = inner.y + inner.height.saturating_sub(3);
        let save_lbl = "   [ Save (Ctrl+S) ]   ";
        let cancel_lbl = "   [ Cancel (Esc) ]   ";
        let save_style = if self.focus_idx == 12 {
            active_style
        } else {
            styles.shortcut_key.bg(base_bg)
        };
        let cancel_style = if self.focus_idx == 13 {
            active_style
        } else {
            styles.help_text.bg(base_bg)
        };
        let center_x = inner.x + inner.width / 2;
        buf.set_string(center_x.saturating_sub(24), btn_y, save_lbl, save_style);
        buf.set_string(center_x + 2, btn_y, cancel_lbl, cancel_style);

        // 2-line wrapped footer to ensure text NEVER exceeds inner width
        let f_line1 = if avail_w >= 56 {
            "[Tab/Up/Down] Field | [F2] Capture Key | [Ctrl+P] Macros"
        } else {
            "[Tab/Down] Field | [F2] Capture | [Ctrl+P] Macros"
        };
        let f_line2 = "[Ctrl+S] Save | [Esc] Cancel";
        let f1_clipped: String = f_line1.chars().take(avail_w).collect();
        let f2_clipped: String = f_line2.chars().take(avail_w).collect();
        let fx1 = inner.x + inner.width.saturating_sub(f1_clipped.len() as u16) / 2;
        let fx2 = inner.x + inner.width.saturating_sub(f2_clipped.len() as u16) / 2;
        buf.set_string(
            fx1,
            inner.y + inner.height.saturating_sub(2),
            &f1_clipped,
            hint_style,
        );
        buf.set_string(
            fx2,
            inner.y + inner.height.saturating_sub(1),
            &f2_clipped,
            hint_style,
        );
    }
}

enum ActiveShortcutModal {
    None,
    Edit {
        modal: Box<ShortcutEditModal>,
        is_new: bool,
        child_capture: Option<KeyCaptureModal>,
        child_picker: Option<FilePickerModal>,
        child_help: Option<HelpModal>,
    },
    CaptureStandalone(KeyCaptureModal),
    LeaderSettings {
        modal: LeaderSettingsModal,
        child_capture: Option<KeyCaptureModal>,
    },
    ConfirmDelete(ConfirmModal),
    ConfirmExit(ConfirmModal),
    Help(HelpModal),
}

/// Launches the interactive split-screen `shortcuts.yml` editor.
pub fn run_shortcut_editor(shortcuts_path: PathBuf) -> Result<()> {
    let config_path = resolve_app_path(Path::new("bashmenu.yml"));
    let themes_path = resolve_app_path(Path::new("bashmenu.themes"));

    let app_config = load_config(&config_path).unwrap_or_default();
    let use_nerd = app_config.settings.use_nerd_fonts.unwrap_or(false);
    let themes = load_themes(&themes_path).unwrap_or_default();
    let styles = themes
        .get(&app_config.theme)
        .map(|t| ResolvedThemeStyles::from_theme(t, use_nerd))
        .unwrap_or_default();

    let shortcuts_cfg = if shortcuts_path.exists() {
        load_shortcuts(&shortcuts_path).unwrap_or_default()
    } else {
        ShortcutsConfig::default()
    };

    let mut state = ShortcutEditorState::new(shortcuts_cfg);
    let mut active_modal = ActiveShortcutModal::None;
    let mut toast_rack = ToastRack::new();
    let mut guard = TerminalGuard::init()?;
    let mut need_redraw = true;

    loop {
        if toast_rack.tick() {
            need_redraw = true;
        }

        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        if need_redraw {
            guard.draw(|f| {
                render_shortcut_editor_screen(
                    f.area(),
                    &mut state,
                    &app_config,
                    &styles,
                    f.buffer_mut(),
                );
                match &mut active_modal {
                    ActiveShortcutModal::None => {}
                    ActiveShortcutModal::Edit {
                        modal,
                        child_capture,
                        child_picker,
                        child_help,
                        ..
                    } => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                        if let Some(cap) = child_capture {
                            cap.render(f.area(), &styles, f.buffer_mut());
                        } else if let Some(picker) = child_picker {
                            picker.render(f.area(), &styles, f.buffer_mut());
                        } else if let Some(help) = child_help {
                            help.render(f.area(), &styles, f.buffer_mut());
                        }
                    }
                    ActiveShortcutModal::CaptureStandalone(cap) => {
                        cap.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveShortcutModal::LeaderSettings {
                        modal,
                        child_capture,
                    } => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                        if let Some(cap) = child_capture {
                            cap.render(f.area(), &styles, f.buffer_mut());
                        }
                    }
                    ActiveShortcutModal::ConfirmDelete(modal)
                    | ActiveShortcutModal::ConfirmExit(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveShortcutModal::Help(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                }
                toast_rack.render(f.area(), &styles, f.buffer_mut());
            })?;
            need_redraw = false;
        }

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let ev = event::read()?;
        if let Event::Key(k) = &ev {
            if k.kind == crossterm::event::KeyEventKind::Release {
                continue;
            }
        }
        if let Event::Mouse(m) = &ev {
            if m.kind == MouseEventKind::Moved {
                continue;
            }
            if m.kind == MouseEventKind::Down(MouseButton::Left)
                && toast_rack.handle_mouse(m.column, m.row)
            {
                need_redraw = true;
                continue;
            }
        }
        need_redraw = true;

        match &mut active_modal {
            ActiveShortcutModal::Help(modal) => {
                let dismiss = match ev {
                    Event::Key(k) => modal.handle_key(k.code),
                    Event::Mouse(m) => modal.handle_mouse(m, area),
                    _ => false,
                };
                if dismiss {
                    active_modal = ActiveShortcutModal::None;
                }
            }
            ActiveShortcutModal::ConfirmDelete(modal) => {
                let choice = match ev {
                    Event::Key(k) => modal.handle_key(k.code),
                    Event::Mouse(m) => modal.handle_mouse(m, area),
                    _ => None,
                };
                if let Some(c) = choice {
                    if c == ConfirmChoice::Yes {
                        if let Some(removed) = state.delete_selected() {
                            toast_rack.add(ToastNotification::new(
                                format!("Deleted shortcut '{}'", removed.description),
                                Some("Shortcut Deleted".to_string()),
                                2.5,
                            ));
                        }
                    }
                    active_modal = ActiveShortcutModal::None;
                }
            }
            ActiveShortcutModal::ConfirmExit(modal) => {
                let choice = match ev {
                    Event::Key(k) => modal.handle_key(k.code),
                    Event::Mouse(m) => modal.handle_mouse(m, area),
                    _ => None,
                };
                if let Some(c) = choice {
                    match c {
                        ConfirmChoice::Yes => break,
                        ConfirmChoice::No | ConfirmChoice::Cancel => {
                            active_modal = ActiveShortcutModal::None;
                        }
                    }
                }
            }
            ActiveShortcutModal::CaptureStandalone(cap) => {
                if let Event::Key(k) = ev {
                    if let Some(res) = cap.handle_key(k) {
                        if let Some(new_key) = res {
                            if let Some(entry) = state.selected_entry_mut() {
                                entry.key = new_key.clone();
                                state.dirty = true;
                                toast_rack.add(ToastNotification::new(
                                    format!("Updated key binding to '{}'", new_key),
                                    Some("Key Captured".to_string()),
                                    2.5,
                                ));
                            }
                        }
                        active_modal = ActiveShortcutModal::None;
                    }
                }
            }
            ActiveShortcutModal::LeaderSettings {
                modal,
                child_capture,
            } => {
                if let Some(cap) = child_capture {
                    if let Event::Key(k) = ev {
                        if let Some(res) = cap.handle_key(k) {
                            if let Some(new_ldr) = res {
                                modal.key_input.set_text(&new_ldr);
                                modal.error_msg = None;
                            }
                            *child_capture = None;
                        }
                    }
                    continue;
                }

                if let Event::Key(k) = ev {
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('k') | KeyCode::Char('K'))
                    {
                        *child_capture = Some(KeyCaptureModal::new(
                            &modal.key_input.as_string(),
                            &modal.key_input.as_string(),
                            true,
                        ));
                        continue;
                    }
                    match k.code {
                        KeyCode::Esc => {
                            active_modal = ActiveShortcutModal::None;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            modal.focus_idx = (modal.focus_idx + 1) % 6;
                        }
                        KeyCode::BackTab | KeyCode::Up => {
                            modal.focus_idx = (modal.focus_idx + 5) % 6;
                        }
                        KeyCode::Char(' ') if modal.focus_idx == 3 => {
                            modal.show_hint = !modal.show_hint;
                        }
                        KeyCode::Enter => match modal.focus_idx {
                            1 => {
                                *child_capture = Some(KeyCaptureModal::new(
                                    &modal.key_input.as_string(),
                                    &modal.key_input.as_string(),
                                    true,
                                ));
                            }
                            3 => {
                                modal.show_hint = !modal.show_hint;
                            }
                            5 => {
                                active_modal = ActiveShortcutModal::None;
                            }
                            0 | 2 | 4 => {
                                match validate_key_spec(&modal.key_input.as_string(), true) {
                                    Ok(valid_ldr) => {
                                        let timeout = modal
                                            .timeout_input
                                            .as_string()
                                            .trim()
                                            .parse::<u64>()
                                            .unwrap_or(1500);
                                        state.config.leader_key = valid_ldr.clone();
                                        state.config.leader_timeout_ms = timeout;
                                        state.config.show_leader_hint = modal.show_hint;
                                        state.dirty = true;
                                        toast_rack.add(ToastNotification::new(
                                            format!(
                                                "Leader key set to '{}' ({}ms)",
                                                valid_ldr, timeout
                                            ),
                                            Some("Leader Settings Updated".to_string()),
                                            2.5,
                                        ));
                                        active_modal = ActiveShortcutModal::None;
                                    }
                                    Err(err) => {
                                        modal.error_msg = Some(err);
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => {
                            if modal.focus_idx == 0 {
                                let _ = modal
                                    .key_input
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            } else if modal.focus_idx == 2 {
                                let _ = modal
                                    .timeout_input
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            }
                        }
                    }
                }
            }
            ActiveShortcutModal::Edit {
                modal,
                is_new,
                child_capture,
                child_picker,
                child_help,
            } => {
                if let Some(help) = child_help {
                    let dismiss = match ev {
                        Event::Key(k) => help.handle_key(k.code),
                        Event::Mouse(m) => help.handle_mouse(m, area),
                        _ => false,
                    };
                    if dismiss {
                        *child_help = None;
                    }
                    continue;
                }
                if let Some(picker) = child_picker {
                    let chosen = match ev {
                        Event::Key(k) => picker.handle_key(k.code),
                        Event::Mouse(m) => picker.handle_mouse(m, area),
                        _ => None,
                    };
                    if let Some(opt_path) = chosen {
                        if let Some(p) = opt_path {
                            modal.target_input.set_text(&p.to_string_lossy());
                        }
                        *child_picker = None;
                    }
                    continue;
                }
                if let Some(cap) = child_capture {
                    if let Event::Key(k) = ev {
                        if let Some(res) = cap.handle_key(k) {
                            if let Some(new_key) = res {
                                modal.set_captured_key(&new_key);
                            }
                            *child_capture = None;
                        }
                    }
                    continue;
                }

                if let Event::Key(k) = ev {
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('p') | KeyCode::Char('P'))
                    {
                        *child_help = Some(HelpModal::new(
                            "BashMenu Placeholders & Macros",
                            PLACEHOLDER_HELP_TEXT,
                        ));
                        continue;
                    }
                    if k.code == KeyCode::F(2)
                        || (k.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(k.code, KeyCode::Char('k') | KeyCode::Char('K')))
                    {
                        *child_capture = Some(KeyCaptureModal::new(
                            &modal.composed_key(),
                            &state.config.leader_key,
                            false,
                        ));
                        continue;
                    }
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('s') | KeyCode::Char('S'))
                    {
                        let orig = state.selected_entry().cloned().unwrap_or_default();
                        match modal.build_entry(&orig) {
                            Ok(built) => {
                                if let Some(slot) = state.selected_entry_mut() {
                                    *slot = built;
                                    state.dirty = true;
                                }
                                active_modal = ActiveShortcutModal::None;
                            }
                            Err(err) => {
                                modal.error_msg = Some(err);
                            }
                        }
                        continue;
                    }

                    // Field focus index map:
                    // 0 = description, 1 = key_input, 2 = capture_btn, 3 = use_leader,
                    // 4 = item_type, 5 = target_input, 6 = browse_btn, 7 = execution_mode,
                    // 8 = window_percent, 9 = quiet, 10 = alt_buffer, 11 = enabled,
                    // 12 = Save, 13 = Cancel
                    match k.code {
                        KeyCode::Esc => {
                            // Cancel edits: if this was a freshly added item, discard it from state
                            if *is_new {
                                state.delete_selected();
                            }
                            active_modal = ActiveShortcutModal::None;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            // Forward field navigation (cycles 0..13)
                            modal.focus_idx = (modal.focus_idx + 1) % 14;
                        }
                        KeyCode::BackTab | KeyCode::Up => {
                            // Backward field navigation
                            modal.focus_idx = (modal.focus_idx + 13) % 14;
                        }
                        KeyCode::Left if modal.focus_idx == 4 => {
                            // Cycle item type backwards (command, script, editor, toast, notebook, theme_selector)
                            modal.item_type_idx = (modal.item_type_idx + SHORTCUT_TYPES.len() - 1)
                                % SHORTCUT_TYPES.len();
                        }
                        KeyCode::Right | KeyCode::Char(' ') if modal.focus_idx == 4 => {
                            // Cycle item type forwards
                            modal.item_type_idx = (modal.item_type_idx + 1) % SHORTCUT_TYPES.len();
                        }
                        KeyCode::Left if modal.focus_idx == 7 => {
                            // Cycle execution mode backwards (Console, Streaming, Interactive)
                            modal.execution_mode = match modal.execution_mode {
                                ExecutionMode::Console => ExecutionMode::Interactive,
                                ExecutionMode::Streaming => ExecutionMode::Console,
                                ExecutionMode::Interactive => ExecutionMode::Streaming,
                            };
                        }
                        KeyCode::Right | KeyCode::Char(' ') if modal.focus_idx == 7 => {
                            // Cycle execution mode forwards
                            modal.execution_mode = match modal.execution_mode {
                                ExecutionMode::Console => ExecutionMode::Streaming,
                                ExecutionMode::Streaming => ExecutionMode::Interactive,
                                ExecutionMode::Interactive => ExecutionMode::Console,
                            };
                        }
                        KeyCode::Char(' ') if modal.focus_idx == 3 => {
                            // Toggle `<leader>` prefix on/off
                            modal.use_leader = !modal.use_leader;
                        }
                        KeyCode::Char(' ') if modal.focus_idx == 9 => {
                            // Toggle Quiet mode (suppresses "Press [ENTER] to return...")
                            modal.quiet = !modal.quiet;
                        }
                        KeyCode::Char(' ') if modal.focus_idx == 10 => {
                            // Toggle Alt Buffer (preserves or clears terminal alternate screen)
                            modal.alt_buffer = !modal.alt_buffer;
                        }
                        KeyCode::Char(' ') if modal.focus_idx == 11 => {
                            // Toggle Enabled status
                            modal.enabled = !modal.enabled;
                        }
                        KeyCode::Enter => match modal.focus_idx {
                            2 => {
                                // Activate interactive key capture modal
                                *child_capture = Some(KeyCaptureModal::new(
                                    &modal.composed_key(),
                                    &state.config.leader_key,
                                    false,
                                ));
                            }
                            3 => {
                                modal.use_leader = !modal.use_leader;
                            }
                            4 => {
                                modal.item_type_idx =
                                    (modal.item_type_idx + 1) % SHORTCUT_TYPES.len();
                            }
                            6 => {
                                let start =
                                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                *child_picker = Some(FilePickerModal::new(
                                    "Select Target File or Script",
                                    &start,
                                    false,
                                ));
                            }
                            7 => {
                                modal.execution_mode = match modal.execution_mode {
                                    ExecutionMode::Console => ExecutionMode::Streaming,
                                    ExecutionMode::Streaming => ExecutionMode::Interactive,
                                    ExecutionMode::Interactive => ExecutionMode::Console,
                                };
                            }
                            9 => modal.quiet = !modal.quiet,
                            10 => modal.alt_buffer = !modal.alt_buffer,
                            11 => modal.enabled = !modal.enabled,
                            13 => {
                                if *is_new {
                                    state.delete_selected();
                                }
                                active_modal = ActiveShortcutModal::None;
                            }
                            0 | 1 | 5 | 8 | 12 => {
                                let orig = state.selected_entry().cloned().unwrap_or_default();
                                match modal.build_entry(&orig) {
                                    Ok(built) => {
                                        if let Some(slot) = state.selected_entry_mut() {
                                            *slot = built;
                                            state.dirty = true;
                                        }
                                        active_modal = ActiveShortcutModal::None;
                                    }
                                    Err(err) => {
                                        modal.error_msg = Some(err);
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => match modal.focus_idx {
                            0 => {
                                let _ = modal
                                    .description
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            }
                            1 => {
                                let _ = modal
                                    .key_input
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            }
                            5 => {
                                let _ = modal
                                    .target_input
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            }
                            8 => {
                                let _ = modal
                                    .window_percent
                                    .handle_key_with_modifiers(k.code, k.modifiers);
                            }
                            _ => {}
                        },
                    }
                }
            }
            ActiveShortcutModal::None => match ev {
                Event::Key(k) => {
                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                        match k.code {
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                match save_shortcuts(&shortcuts_path, &state.config) {
                                    Ok(()) => {
                                        state.dirty = false;
                                        toast_rack.add(ToastNotification::new(
                                            format!("Saved {}", shortcuts_path.display()),
                                            Some("Shortcuts Saved".to_string()),
                                            2.5,
                                        ));
                                    }
                                    Err(e) => {
                                        toast_rack.add(ToastNotification::new(
                                            format!("Save failed: {}", e),
                                            Some("Error".to_string()),
                                            4.0,
                                        ));
                                    }
                                }
                                continue;
                            }
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                active_modal = ActiveShortcutModal::Help(HelpModal::new(
                                    "BashMenu Placeholders & Macros",
                                    PLACEHOLDER_HELP_TEXT,
                                ));
                                continue;
                            }
                            _ => {}
                        }
                    }

                    match k.code {
                        KeyCode::Up | KeyCode::Char('k')
                            if !k.modifiers.contains(KeyModifiers::SHIFT) =>
                        {
                            state.move_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.move_down();
                        }
                        // [Home] / [End] Jump to first / last item
                        KeyCode::Home => {
                            state.selected_idx = 0;
                        }
                        KeyCode::End => {
                            if !state.config.shortcuts.is_empty() {
                                state.selected_idx = state.config.shortcuts.len() - 1;
                            }
                        }
                        // [a] Add new shortcut entry
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            let new_entry = ShortcutEntry::default();
                            state.add_shortcut(new_entry.clone());
                            active_modal = ActiveShortcutModal::Edit {
                                modal: Box::new(ShortcutEditModal::from_entry(&new_entry)),
                                is_new: true,
                                child_capture: None,
                                child_picker: None,
                                child_help: None,
                            };
                        }
                        // [e] / [Enter] Edit currently selected shortcut
                        KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') => {
                            if let Some(entry) = state.selected_entry() {
                                active_modal = ActiveShortcutModal::Edit {
                                    modal: Box::new(ShortcutEditModal::from_entry(entry)),
                                    is_new: false,
                                    child_capture: None,
                                    child_picker: None,
                                    child_help: None,
                                };
                            }
                        }
                        // [c] / [F2] Quick-capture shortcut key chord for current item
                        KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::F(2) => {
                            if let Some(entry) = state.selected_entry() {
                                active_modal =
                                    ActiveShortcutModal::CaptureStandalone(KeyCaptureModal::new(
                                        &entry.key,
                                        &state.config.leader_key,
                                        false,
                                    ));
                            }
                        }
                        // [l] Global leader key settings modal
                        KeyCode::Char('l') | KeyCode::Char('L') => {
                            active_modal = ActiveShortcutModal::LeaderSettings {
                                modal: LeaderSettingsModal::new(&state.config),
                                child_capture: None,
                            };
                        }
                        // [Space] Toggle enable / disable on selected shortcut
                        KeyCode::Char(' ') => {
                            state.toggle_selected();
                        }
                        // [d] / [Delete] Delete selected shortcut (prompts confirmation)
                        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                            if let Some(entry) = state.selected_entry() {
                                active_modal =
                                    ActiveShortcutModal::ConfirmDelete(ConfirmModal::new(
                                        "Delete Shortcut",
                                        format!(
                                            "Delete shortcut '{}' ({})?",
                                            entry.description, entry.key
                                        ),
                                    ));
                            }
                        }
                        // [m] Move shortcut item down
                        KeyCode::Char('m') => {
                            state.move_item_down();
                        }
                        // [M] Move shortcut item up
                        KeyCode::Char('M') => {
                            state.move_item_up();
                        }
                        // [s] Save shortcuts to disk
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            match save_shortcuts(&shortcuts_path, &state.config) {
                                Ok(()) => {
                                    state.dirty = false;
                                    toast_rack.add(ToastNotification::new(
                                        format!("Saved {}", shortcuts_path.display()),
                                        Some("Shortcuts Saved".to_string()),
                                        2.5,
                                    ));
                                }
                                Err(e) => {
                                    toast_rack.add(ToastNotification::new(
                                        format!("Save failed: {}", e),
                                        Some("Error".to_string()),
                                        4.0,
                                    ));
                                }
                            }
                        }
                        // [F1] / [?] Placeholders & Macros reference modal
                        KeyCode::F(1) | KeyCode::Char('?') => {
                            active_modal = ActiveShortcutModal::Help(HelpModal::new(
                                "BashMenu Placeholders & Macros",
                                PLACEHOLDER_HELP_TEXT,
                            ));
                        }
                        // [Esc] / [q] Quit application (prompts confirmation if unsaved changes exist)
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                            if state.dirty {
                                active_modal = ActiveShortcutModal::ConfirmExit(ConfirmModal::new(
                                    "Unsaved Changes",
                                    "Discard unsaved shortcut changes and exit?",
                                ));
                            } else {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                    let list_start_y = 4u16;
                    let left_w = area.width / 2;
                    let list_end_y = area.height.saturating_sub(4);
                    if m.column >= 2
                        && m.column < left_w.saturating_sub(2)
                        && m.row >= list_start_y
                        && m.row < list_end_y
                    {
                        let clicked_idx = state.scroll_offset + (m.row - list_start_y) as usize;
                        if clicked_idx < state.config.shortcuts.len() {
                            if state.selected_idx == clicked_idx {
                                if let Some(entry) = state.selected_entry() {
                                    active_modal = ActiveShortcutModal::Edit {
                                        modal: Box::new(ShortcutEditModal::from_entry(entry)),
                                        is_new: false,
                                        child_capture: None,
                                        child_picker: None,
                                        child_help: None,
                                    };
                                }
                            } else {
                                state.selected_idx = clicked_idx;
                            }
                        }
                    }
                }
                _ => {}
            },
        }
    }

    Ok(())
}

pub fn render_shortcut_editor_screen(
    area: Rect,
    state: &mut ShortcutEditorState,
    app_config: &BashMenuConfig,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) {
    if area.width < 20 || area.height < 8 {
        return;
    }

    let base_bg = styles.background;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)]
                .set_symbol(" ")
                .set_style(Style::default().bg(styles.background));
        }
    }

    let help_raw = "[a] Add | [e/ENTER] Edit | [c/F2] Capture Key | [l] Leader Key | [SPACE] On/Off | [d] Delete | [m/M] Move | [Ctrl+P] Macros | [s/Ctrl+S] Save | [ESC/q] Exit";
    let inner_w = area.width.saturating_sub(4) as usize;
    let help_items: Vec<&str> = help_raw.split(" | ").collect();
    let gutter_lines = wrap_spread_help_gutter_layout(&help_items, inner_w, 2);
    let gutter_h = gutter_lines.len().max(1) as u16;

    let panel_h = area.height.saturating_sub(gutter_h + 1);
    let left_w = area.width / 2;
    let right_w = area.width.saturating_sub(left_w);

    let dirty_mark = if state.dirty { " *" } else { "" };
    let left_title = format!("Keyboard Shortcuts{}", dirty_mark);
    draw_connected_panel(
        Rect::new(0, 0, left_w, panel_h),
        &left_title,
        "├",
        "┘",
        styles,
        buf,
    );
    draw_connected_panel(
        Rect::new(left_w, 0, right_w, panel_h),
        "Shortcut Inspector",
        "└",
        "┤",
        styles,
        buf,
    );

    // Left pane header (Leader summary)
    let hdr_style = styles.shortcut_key.bg(base_bg).add_modifier(Modifier::BOLD);
    let hdr_text = format!(
        "Leader: [{}]  Timeout: {}ms  Hint: {}",
        state.config.leader_key,
        state.config.leader_timeout_ms,
        if state.config.show_leader_hint {
            "ON"
        } else {
            "OFF"
        }
    );
    let clipped_hdr: String = hdr_text
        .chars()
        .take(left_w.saturating_sub(4) as usize)
        .collect();
    buf.set_string(2, 2, &clipped_hdr, hdr_style);
    if left_w > 4 {
        let sep = "─".repeat(left_w.saturating_sub(4) as usize);
        buf.set_string(2, 3, &sep, Style::default().fg(styles.border).bg(base_bg));
    }

    // Left pane list (reserving 1 row above bottom border per invariant)
    let list_start_y = 4u16;
    let list_avail_rows = panel_h.saturating_sub(6) as usize;
    if list_avail_rows > 0 {
        if state.selected_idx < state.scroll_offset {
            state.scroll_offset = state.selected_idx;
        } else if state.selected_idx >= state.scroll_offset + list_avail_rows {
            state.scroll_offset = state.selected_idx + 1 - list_avail_rows;
        }

        let item_max_w = left_w.saturating_sub(5) as usize;
        for (vis_i, idx) in (state.scroll_offset..state.config.shortcuts.len())
            .take(list_avail_rows)
            .enumerate()
        {
            let entry = &state.config.shortcuts[idx];
            let y = list_start_y + vis_i as u16;
            let is_sel = idx == state.selected_idx;

            let chk = if entry.enabled { "[x]" } else { "[ ]" };
            let key_disp = format!("[{}]", entry.normalized_key());
            let badge = entry.mode_badge();
            let row_text = format!("{} {:<13} {} {}", chk, key_disp, badge, entry.description);
            let mut clipped: String = row_text.chars().take(item_max_w).collect();
            let cur_w = visible_text_len(&clipped);
            if cur_w < item_max_w {
                clipped.push_str(&" ".repeat(item_max_w - cur_w));
            }

            let row_style = if is_sel {
                styles.highlight
            } else if !entry.enabled {
                styles.help_text.bg(base_bg)
            } else {
                styles.text.bg(base_bg)
            };
            buf.set_string(2, y, &clipped, row_style);
        }

        // Scrollbar 1 column inside right border when overflowing
        let total = state.config.shortcuts.len();
        if total > list_avail_rows && left_w >= 4 {
            let sb_x = left_w - 2;
            for r in 0..list_avail_rows {
                buf[(sb_x, list_start_y + r as u16)]
                    .set_symbol("│")
                    .set_style(Style::default().fg(styles.border).bg(base_bg));
            }
            let thumb_pos = (state.selected_idx * list_avail_rows.saturating_sub(1))
                / total.saturating_sub(1).max(1);
            buf[(sb_x, list_start_y + thumb_pos as u16)]
                .set_symbol("█")
                .set_style(styles.shortcut_key.bg(base_bg));
        }
    }

    // Right pane inspector
    let right_avail_w = right_w.saturating_sub(4) as usize;
    let insp_lines = format_shortcut_inspector(state, app_config, right_avail_w);
    let insp_max_rows = panel_h.saturating_sub(3) as usize;
    for (i, line) in insp_lines.iter().take(insp_max_rows).enumerate() {
        let y = 2 + i as u16;
        let clipped: String = line.chars().take(right_avail_w).collect();
        let st = if line.ends_with("Configuration") || line.ends_with("Properties") {
            styles.shortcut_key.bg(base_bg).add_modifier(Modifier::BOLD)
        } else if line.starts_with('─') {
            Style::default().fg(styles.border).bg(base_bg)
        } else {
            styles.text.bg(base_bg)
        };
        buf.set_string(left_w + 2, y, &clipped, st);
    }

    // Bottom spread help gutter
    let gutter_start_y = panel_h;
    let bot_y = area.height.saturating_sub(1);
    let b_style = Style::default().fg(styles.border).bg(base_bg);

    for y in gutter_start_y..bot_y {
        buf.set_string(0, y, "│", b_style);
        buf.set_string(area.width.saturating_sub(1), y, "│", b_style);
    }

    for (i, gline) in gutter_lines.iter().enumerate() {
        let y = gutter_start_y + i as u16;
        if y < bot_y {
            buf.set_string(2, y, &gline.0, styles.help_text.bg(base_bg));
        }
    }

    // Bottom horizontal border line
    let bot_rule = format!("└{}┘", "─".repeat(area.width.saturating_sub(2) as usize));
    buf.set_string(0, bot_y, &bot_rule, b_style);
}

fn draw_connected_panel(
    rect: Rect,
    title: &str,
    bot_left: &str,
    bot_right: &str,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) {
    if rect.width < 4 || rect.height < 3 {
        return;
    }
    let base_bg = styles.background;
    let b_style = Style::default().fg(styles.border).bg(base_bg);
    let t_style = styles.title.bg(base_bg).add_modifier(Modifier::BOLD);

    let title_block = format!("[ {} ]", title);
    let title_w = visible_text_len(&title_block);
    let inner_w = rect.width.saturating_sub(2) as usize;

    buf.set_string(rect.x, rect.y, "┌─", b_style);
    if inner_w > 2 {
        let clipped_title: String = title_block
            .chars()
            .take(inner_w.saturating_sub(1))
            .collect();
        let cw = visible_text_len(&clipped_title);
        buf.set_string(rect.x + 2, rect.y, &clipped_title, t_style);
        let rem = inner_w.saturating_sub(1 + cw);
        if rem > 0 {
            buf.set_string(rect.x + 2 + cw as u16, rect.y, "─".repeat(rem), b_style);
        }
        let _ = title_w;
    }
    buf.set_string(rect.x + rect.width - 1, rect.y, "┐", b_style);

    for y in (rect.y + 1)..(rect.y + rect.height - 1) {
        buf.set_string(rect.x, y, "│", b_style);
        buf.set_string(rect.x + rect.width - 1, y, "│", b_style);
    }

    let bot = format!("{}{}{}", bot_left, "─".repeat(inner_w), bot_right);
    buf.set_string(rect.x, rect.y + rect.height - 1, &bot, b_style);
}

/// Renders a single-line [`TextInput`] field with a visible cursor and selection highlight.
fn draw_input(
    buf: &mut Buffer,
    rect: Rect,
    input: &TextInput,
    styles: &ResolvedThemeStyles,
    focused: bool,
) {
    if rect.width == 0 {
        return;
    }
    let base = styles
        .text
        .bg(styles.background)
        .add_modifier(Modifier::UNDERLINED);
    let w = rect.width as usize;
    let start = input.cursor.saturating_sub(w.saturating_sub(1));
    let sel = input.selection_range();
    for col in 0..w {
        let idx = start + col;
        let ch = input.chars.get(idx).copied().unwrap_or(' ');
        let mut st = base;
        if sel.is_some_and(|(a, b)| idx >= a && idx < b) {
            st = st.add_modifier(Modifier::REVERSED);
        }
        if focused && idx == input.cursor {
            st = st.add_modifier(Modifier::REVERSED);
        }
        buf[(rect.x + col as u16, rect.y)]
            .set_symbol(&ch.to_string())
            .set_style(st);
    }
}
