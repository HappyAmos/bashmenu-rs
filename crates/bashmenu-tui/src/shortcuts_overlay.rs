//! Leader-key chord state machine, Crossterm key-event normalizer, and compact *which-key* popup overlay.

use crate::modals::{compute_modal_inner, is_close_button_clicked, render_modal_frame};
use crate::style::ResolvedThemeStyles;
use bashmenu_core::{normalize_key_spec, ShortcutEntry, ShortcutsConfig};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// Outcome of processing a keyboard event through [`ShortcutDispatcher`].
#[derive(Debug, Clone, PartialEq)]
pub enum ShortcutDispatchResult {
    /// Event did not match any shortcut or leader key; pass through to normal menu handling.
    PassThrough,
    /// User pressed the configured `leader_key`; dispatcher is now in Leader Pending mode.
    LeaderActivated,
    /// User cancelled Leader Pending mode via `Esc`.
    LeaderCancelled,
    /// User pressed an unbound key while in Leader Pending mode (returns the unbound key string).
    LeaderUnbound(String),
    /// A direct shortcut or `<leader> <key>` chord matched an enabled [`ShortcutEntry`].
    Matched(Box<ShortcutEntry>),
}

/// Converts a Crossterm [`KeyEvent`] into a canonical normalized key token string.
///
/// Handles cross-terminal `<Ctrl+Space>` variations (`KeyCode::Char('\0')`, `KeyCode::Null`,
/// and `KeyCode::Char(' ')` with `KeyModifiers::CONTROL`).
pub fn key_event_to_token(key: KeyEvent) -> Option<String> {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let has_alt = key.modifiers.contains(KeyModifiers::ALT);
    let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // Cross-terminal Ctrl+Space detection (NUL byte or KeyCode::Null)
    if key.code == KeyCode::Null || key.code == KeyCode::Char('\0') {
        if has_alt {
            return Some("ctrl+alt+space".to_string());
        }
        return Some("ctrl+space".to_string());
    }

    let base = match key.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) => {
            if has_ctrl || has_alt {
                c.to_ascii_lowercase().to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::F(n) => format!("F{}", n),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "shift+tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        _ => return None,
    };

    if base == "shift+tab" {
        return Some(base);
    }

    let mut parts = Vec::new();
    if has_ctrl {
        parts.push("ctrl");
    }
    if has_alt {
        parts.push("alt");
    }
    // Only attach explicit shift+ for non-Char keys (since Char already carries uppercase/shifted symbol)
    if has_shift && !matches!(key.code, KeyCode::Char(_)) {
        parts.push("shift");
    }
    parts.push(&base);
    Some(parts.join("+"))
}

/// Stateful dispatcher managing direct shortcuts, leader-key chord timeouts,
/// and the compact *which-key* popup overlay.
#[derive(Debug, Clone)]
pub struct ShortcutDispatcher {
    config: ShortcutsConfig,
    leader_pending_since: Option<Instant>,
}

impl ShortcutDispatcher {
    /// Creates a new shortcut dispatcher from a [`ShortcutsConfig`].
    pub fn new(config: ShortcutsConfig) -> Self {
        Self {
            config,
            leader_pending_since: None,
        }
    }

    /// Replaces the active [`ShortcutsConfig`] and resets any pending leader state.
    pub fn set_config(&mut self, config: ShortcutsConfig) {
        self.config = config;
        self.leader_pending_since = None;
    }

    /// Returns a reference to the current [`ShortcutsConfig`].
    pub fn config(&self) -> &ShortcutsConfig {
        &self.config
    }

    /// Returns `true` if the dispatcher is currently waiting for a follow-up key after `leader_key`.
    pub fn is_leader_pending(&self) -> bool {
        self.leader_pending_since.is_some()
    }

    /// Explicitly cancels Leader Pending mode.
    pub fn cancel_leader(&mut self) {
        self.leader_pending_since = None;
    }

    /// Checks whether the leader key timeout has elapsed.
    /// Returns `true` if Leader Pending mode expired and the UI should redraw.
    pub fn poll_timeout(&mut self) -> bool {
        if let Some(since) = self.leader_pending_since {
            let timeout_ms = self.config.leader_timeout_ms;
            if timeout_ms > 0 && since.elapsed() >= Duration::from_millis(timeout_ms) {
                self.leader_pending_since = None;
                return true;
            }
        }
        false
    }

    /// Returns all enabled `<leader> <key>` shortcuts as `(chord_key, &ShortcutEntry)`.
    pub fn active_leader_entries(&self) -> Vec<(String, &ShortcutEntry)> {
        self.config
            .shortcuts
            .iter()
            .filter(|s| s.enabled)
            .filter_map(|s| s.leader_chord_key().map(|k| (k, s)))
            .collect()
    }

    /// Processes a [`KeyEvent`] when no blocking modal is active.
    pub fn handle_key(&mut self, key: KeyEvent) -> ShortcutDispatchResult {
        let Some(token) = key_event_to_token(key) else {
            return ShortcutDispatchResult::PassThrough;
        };

        // 1. If we are in Leader Pending mode, evaluate the follow-up key
        if self.leader_pending_since.is_some() {
            self.leader_pending_since = None;
            if token.eq_ignore_ascii_case("esc") {
                return ShortcutDispatchResult::LeaderCancelled;
            }

            for entry in &self.config.shortcuts {
                if !entry.enabled {
                    continue;
                }
                if let Some(chord_key) = entry.leader_chord_key() {
                    if chord_keys_match(&chord_key, &token) {
                        return ShortcutDispatchResult::Matched(Box::new(entry.clone()));
                    }
                }
            }
            return ShortcutDispatchResult::LeaderUnbound(token);
        }

        // 2. Check if the key pressed is the configured leader_key
        let leader_norm = normalize_key_spec(&self.config.leader_key);
        if !leader_norm.is_empty() && leader_norm.eq_ignore_ascii_case(&token) {
            self.leader_pending_since = Some(Instant::now());
            return ShortcutDispatchResult::LeaderActivated;
        }

        // 3. Check direct single-stroke shortcuts (non-leader)
        for entry in &self.config.shortcuts {
            if !entry.enabled || entry.leader_chord_key().is_some() {
                continue;
            }
            let norm = entry.normalized_key();
            if norm.eq_ignore_ascii_case(&token) {
                return ShortcutDispatchResult::Matched(Box::new(entry.clone()));
            }
        }

        ShortcutDispatchResult::PassThrough
    }

    /// Computes the bounding box `(box_w, box_h)` for the compact *which-key* leader popup overlay.
    fn popup_dimensions(&self, area: Rect) -> (u16, u16) {
        let entries = self.active_leader_entries();
        let row_count = entries.len().max(1);
        let max_w = area.width.saturating_sub(6).clamp(36, 68);
        // Header + separator + entries + separator + footer + 2 border lines
        let desired_h = (row_count as u16).saturating_add(5);
        let max_h = area.height.saturating_sub(4).clamp(6, 20);
        (max_w, desired_h.min(max_h))
    }

    /// Handles mouse clicks while the compact *which-key* leader popup overlay is visible.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> ShortcutDispatchResult {
        if !self.is_leader_pending() {
            return ShortcutDispatchResult::PassThrough;
        }
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return ShortcutDispatchResult::PassThrough;
        }

        let (box_w, box_h) = self.popup_dimensions(area);
        let inner = compute_modal_inner(area, box_w, box_h);
        if is_close_button_clicked(inner, mouse.row, mouse.column) {
            self.leader_pending_since = None;
            return ShortcutDispatchResult::LeaderCancelled;
        }
        if mouse.column >= inner.x
            && mouse.column < inner.x + inner.width
            && mouse.row >= inner.y
            && mouse.row < inner.y + inner.height
        {
            let entries = self.active_leader_entries();
            let list_start_y = inner.y;
            let list_max_rows = inner.height.saturating_sub(2) as usize;
            let rel_row = (mouse.row - list_start_y) as usize;
            if rel_row < list_max_rows && rel_row < entries.len() {
                let chosen = entries[rel_row].1.clone();
                self.leader_pending_since = None;
                return ShortcutDispatchResult::Matched(Box::new(chosen));
            }
            return ShortcutDispatchResult::PassThrough;
        }

        // Clicking outside dismisses the leader popup
        self.leader_pending_since = None;
        ShortcutDispatchResult::LeaderCancelled
    }

    /// Renders the compact *which-key* popup overlay when in Leader Pending mode.
    pub fn render_leader_popup(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        if !self.is_leader_pending() || !self.config.show_leader_hint {
            return;
        }

        let (box_w, box_h) = self.popup_dimensions(area);
        let title = format!("Leader ({}) — Press Key", self.config.leader_key);
        let inner = render_modal_frame(area, box_w, box_h, &title, styles, buf);
        if inner.width < 10 || inner.height < 2 {
            return;
        }

        let base_bg = styles.background;
        let text_style = styles.text.bg(base_bg);
        let key_style = styles.shortcut_key.bg(base_bg).add_modifier(Modifier::BOLD);
        let badge_style = styles.help_text.bg(base_bg);

        let entries = self.active_leader_entries();
        let list_rows = inner.height.saturating_sub(2) as usize;

        if entries.is_empty() {
            let msg = "No <leader> shortcuts configured. Press 'F9' to add.";
            buf.set_string(inner.x + 1, inner.y, msg, badge_style);
        } else {
            for (idx, (chord_key, entry)) in entries.iter().take(list_rows).enumerate() {
                let y = inner.y + idx as u16;
                let key_badge = format!("[{}]", chord_key);
                buf.set_string(inner.x + 1, y, &key_badge, key_style);

                let key_col_w = 8u16.max(UnicodeWidthStr::width(key_badge.as_str()) as u16 + 2);
                let mode_str = entry.mode_badge();
                let mode_w = UnicodeWidthStr::width(mode_str) as u16;

                let desc_x = inner.x + 1 + key_col_w;
                let desc_max_w = inner.width.saturating_sub(key_col_w + mode_w + 4) as usize;
                let mut desc = entry.description.clone();
                if UnicodeWidthStr::width(desc.as_str()) > desc_max_w && desc_max_w > 1 {
                    let mut clipped = String::new();
                    let mut w = 0;
                    for ch in desc.chars() {
                        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                        if w + cw + 1 > desc_max_w {
                            break;
                        }
                        clipped.push(ch);
                        w += cw;
                    }
                    clipped.push('…');
                    desc = clipped;
                }
                buf.set_string(desc_x, y, &desc, text_style);

                let badge_x = (inner.x + inner.width).saturating_sub(mode_w + 1);
                if badge_x > desc_x {
                    buf.set_string(badge_x, y, mode_str, badge_style);
                }
            }
        }

        // Bottom separator and footer hint
        let sep_y = inner.y + inner.height.saturating_sub(2);
        let footer_y = inner.y + inner.height.saturating_sub(1);
        if sep_y > inner.y {
            let rule = "─".repeat(inner.width as usize);
            buf.set_string(
                inner.x,
                sep_y,
                &rule,
                Style::default().fg(styles.border).bg(base_bg),
            );
        }
        let footer = "Press key to launch  |  Click item  |  [ESC] Cancel";
        let fw = UnicodeWidthStr::width(footer) as u16;
        let fx = inner.x + inner.width.saturating_sub(fw) / 2;
        buf.set_string(fx, footer_y, footer, badge_style);
    }
}

fn chord_keys_match(configured: &str, pressed: &str) -> bool {
    if configured.chars().count() == 1 && pressed.chars().count() == 1 {
        // Case-sensitive match for single letters so `<leader> t` and `<leader> T` are distinct
        configured == pressed
    } else {
        configured.eq_ignore_ascii_case(pressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bashmenu_core::MenuItemType;

    #[test]
    fn test_ctrl_space_normalization_across_terminals() {
        let ev1 = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_token(ev1).as_deref(), Some("ctrl+space"));

        let ev2 = KeyEvent::new(KeyCode::Char('\0'), KeyModifiers::NONE);
        assert_eq!(key_event_to_token(ev2).as_deref(), Some("ctrl+space"));

        let ev3 = KeyEvent::new(KeyCode::Null, KeyModifiers::CONTROL);
        assert_eq!(key_event_to_token(ev3).as_deref(), Some("ctrl+space"));
    }

    #[test]
    fn test_leader_chord_and_direct_dispatch() {
        let cfg = ShortcutsConfig {
            leader_key: "ctrl+space".to_string(),
            leader_timeout_ms: 1500,
            show_leader_hint: true,
            shortcuts: vec![
                ShortcutEntry {
                    key: "F9".to_string(),
                    description: "Open Shortcuts".to_string(),
                    r#type: MenuItemType::Command,
                    action: Some("shortcutedit".to_string()),
                    ..Default::default()
                },
                ShortcutEntry {
                    key: "<leader> t".to_string(),
                    description: "Terminal".to_string(),
                    r#type: MenuItemType::Command,
                    action: Some("bash".to_string()),
                    ..Default::default()
                },
            ],
        };

        let mut disp = ShortcutDispatcher::new(cfg);
        assert!(!disp.is_leader_pending());

        // 1. Direct F9 match
        let res_f9 = disp.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
        assert!(matches!(res_f9, ShortcutDispatchResult::Matched(ref e) if e.key == "F9"));

        // 2. Leader key activates Leader Pending mode
        let res_leader = disp.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL));
        assert_eq!(res_leader, ShortcutDispatchResult::LeaderActivated);
        assert!(disp.is_leader_pending());

        // 3. Follow-up 't' matches `<leader> t` and exits Leader Pending mode
        let res_t = disp.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(
            matches!(res_t, ShortcutDispatchResult::Matched(ref e) if e.description == "Terminal")
        );
        assert!(!disp.is_leader_pending());
    }
}
