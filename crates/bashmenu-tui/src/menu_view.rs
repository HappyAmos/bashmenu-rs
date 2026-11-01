//! Main menu view rendering, viewport management, and keyboard/mouse interaction.

use crate::bbcode::{
    parse_bbcode_spans, split_label_brackets, truncate_to_width, visible_text_len,
};
use crate::gutter::wrap_spread_help_gutter;
use crate::style::ResolvedThemeStyles;
use crate::wallpaper::{composite_buffer, WallpaperCompositor};
use bashmenu_core::{
    format_icon_slot, interpolate_placeholders, BashMenuConfig, MacroContext, MenuItem,
    MenuItemType, MenuSchema, PluginDisplayRow, WallpaperConfig,
};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Outcome of handling a user input event
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// No action needed (selection shifted, scroll, etc.)
    None,
    /// Executes a menu command or script
    Execute(Box<MenuItem>),
    /// Return to parent menu or exit application
    Back,
    /// Open help manual (F1)
    Help,
    /// Open theme selector modal (F2)
    ThemeSelector,
    /// Launch text editor (F3)
    LaunchEditor,
    /// Launch menu editor (F4)
    EditMenu,
    /// Request menu and config reload (F5)
    Reload,
    /// Toggle menu shortcuts (F6)
    ToggleShortcuts,
    /// Configure status gutter format (F7)
    ConfigureGutter,
    /// Launch theme editor (F8)
    LaunchThemeEditor,
    /// Open a notebook menu
    OpenNotebook(Box<MenuItem>),
    /// View/read a notebook note
    ViewNote {
        filepath: String,
        note_title: String,
    },
    /// Create a new note inside current notebook
    NewNote(String),
    /// Rename selected note inside current notebook
    RenameNote(String),
    /// Delete selected note inside current notebook
    DeleteNote(String),
    /// Exit the menu application
    Exit,
}

#[derive(Debug, Clone)]
struct MenuLevel {
    title: String,
    items: Vec<MenuItem>,
    selected_idx: usize,
    scroll_offset: usize,
    is_notebook: bool,
    notebook_file: Option<String>,
}

/// The interactive menu widget and state machine
pub struct MenuView {
    stack: Vec<MenuLevel>,
    styles: ResolvedThemeStyles,
    use_nerd_fonts: bool,
    status_gutter: String,
    show_shortcuts: bool,
    plugin_rows: Vec<PluginDisplayRow>,
    pub config: BashMenuConfig,
    wallpaper: Option<WallpaperCompositor>,
}

impl MenuView {
    /// Creates a new MenuView initialized with a root MenuSchema
    pub fn new(
        menu: MenuSchema,
        styles: ResolvedThemeStyles,
        use_nerd_fonts: bool,
        status_gutter: String,
    ) -> Self {
        let initial_level = MenuLevel {
            title: menu.title,
            items: menu.items,
            selected_idx: 0,
            scroll_offset: 0,
            is_notebook: false,
            notebook_file: None,
        };

        let mut config = BashMenuConfig::default();
        config.settings.use_nerd_fonts = Some(use_nerd_fonts);

        let mut view = Self {
            stack: vec![initial_level],
            styles,
            use_nerd_fonts,
            status_gutter,
            show_shortcuts: true,
            plugin_rows: Vec::new(),
            config,
            wallpaper: None,
        };
        view.ensure_selectable();
        view
    }

    /// Sets the application configuration
    pub fn with_config(mut self, config: BashMenuConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets background wallpaper compositor configuration
    pub fn with_wallpaper(mut self, config: Option<WallpaperConfig>) -> Self {
        self.set_wallpaper(config);
        self
    }

    /// Updates background wallpaper compositor configuration
    pub fn set_wallpaper(&mut self, config: Option<WallpaperConfig>) {
        self.wallpaper = config.map(WallpaperCompositor::new);
    }

    /// Updates configuration
    pub fn set_config(&mut self, config: BashMenuConfig) {
        self.config = config;
    }

    /// Update theme styles
    pub fn set_styles(&mut self, styles: ResolvedThemeStyles) {
        self.styles = styles;
    }

    /// Update nerd fonts rendering mode
    pub fn set_use_nerd_fonts(&mut self, use_nerd: bool) {
        self.use_nerd_fonts = use_nerd;
    }

    /// Update status gutter template or value
    pub fn set_status_gutter(&mut self, gutter: String) {
        self.status_gutter = gutter;
    }

    /// Sets whether shortcut numbers/letters are displayed
    pub fn set_show_shortcuts(&mut self, show: bool) {
        self.show_shortcuts = show;
    }

    /// Toggles shortcut display on/off
    pub fn toggle_shortcuts(&mut self) -> bool {
        self.show_shortcuts = !self.show_shortcuts;
        self.show_shortcuts
    }

    /// Whether shortcuts are currently visible
    pub fn show_shortcuts(&self) -> bool {
        self.show_shortcuts
    }

    /// Reload the menu with a new schema
    pub fn reload_menu(&mut self, menu: MenuSchema) {
        self.stack = vec![MenuLevel {
            title: menu.title,
            items: menu.items,
            selected_idx: 0,
            scroll_offset: 0,
            is_notebook: false,
            notebook_file: None,
        }];
        self.ensure_selectable();
    }

    /// Whether the currently displayed menu level is a notebook notes collection
    pub fn is_current_notebook(&self) -> bool {
        self.current_level().is_notebook
    }

    /// Notebook file path if currently inside a notebook
    pub fn current_notebook_file(&self) -> Option<&str> {
        self.current_level().notebook_file.as_deref()
    }

    /// Pushes a dynamic notebook notes submenu onto the stack
    pub fn push_notebook(&mut self, title: String, items: Vec<MenuItem>, filepath: String) {
        self.stack.push(MenuLevel {
            title,
            items,
            selected_idx: 0,
            scroll_offset: 0,
            is_notebook: true,
            notebook_file: Some(filepath),
        });
        self.ensure_selectable();
    }

    /// Reloads current notebook menu items and keeps or updates target selection
    pub fn reload_current_notebook(&mut self, items: Vec<MenuItem>, select_title: Option<&str>) {
        let level = self.current_level_mut();
        level.items = items;
        if let Some(target) = select_title {
            for (idx, it) in level.items.iter().enumerate() {
                if it.label == target {
                    level.selected_idx = idx;
                    break;
                }
            }
        }
        self.ensure_selectable();
    }

    /// Update plugin rows rendered below menu items
    pub fn set_plugin_rows(&mut self, rows: Vec<PluginDisplayRow>) {
        self.plugin_rows = rows;
    }

    /// Current active menu items
    pub fn current_items(&self) -> &[MenuItem] {
        &self.current_level().items
    }

    /// Current selected index
    pub fn selected_index(&self) -> usize {
        self.current_level().selected_idx
    }

    fn current_level(&self) -> &MenuLevel {
        self.stack.last().expect("Menu stack must not be empty")
    }

    fn current_level_mut(&mut self) -> &mut MenuLevel {
        self.stack.last_mut().expect("Menu stack must not be empty")
    }

    /// Ensures selected index does not land on a divider or non-selectable item
    pub fn ensure_selectable(&mut self) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return;
        }

        if level.items[level.selected_idx].is_divider() {
            // Find next non-divider
            for i in level.selected_idx..level.items.len() {
                if !level.items[i].is_divider() {
                    level.selected_idx = i;
                    return;
                }
            }
            // Or previous non-divider
            for i in (0..level.selected_idx).rev() {
                if !level.items[i].is_divider() {
                    level.selected_idx = i;
                    return;
                }
            }
        }
    }

    /// Move selection cursor down
    pub fn move_down(&mut self) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return;
        }

        let mut idx = level.selected_idx;
        while idx + 1 < level.items.len() {
            idx += 1;
            if !level.items[idx].is_divider() {
                level.selected_idx = idx;
                break;
            }
        }
    }

    /// Move selection cursor up
    pub fn move_up(&mut self) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return;
        }

        let mut idx = level.selected_idx;
        while idx > 0 {
            idx -= 1;
            if !level.items[idx].is_divider() {
                level.selected_idx = idx;
                break;
            }
        }
    }

    /// Handles keyboard event
    pub fn handle_key(&mut self, code: KeyCode) -> MenuAction {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                MenuAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                MenuAction::None
            }
            KeyCode::Home => {
                let level = self.current_level_mut();
                level.selected_idx = 0;
                self.ensure_selectable();
                MenuAction::None
            }
            KeyCode::End => {
                let level = self.current_level_mut();
                if !level.items.is_empty() {
                    level.selected_idx = level.items.len() - 1;
                    self.ensure_selectable();
                }
                MenuAction::None
            }
            KeyCode::Enter => self.activate_selected(),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => self.go_back(),
            KeyCode::Char('n') | KeyCode::Char('N') if self.is_current_notebook() => {
                if let Some(fp) = self.current_notebook_file() {
                    MenuAction::NewNote(fp.to_string())
                } else {
                    MenuAction::None
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') if self.is_current_notebook() => {
                let level = self.current_level();
                if level.selected_idx < level.items.len() {
                    let sel = &level.items[level.selected_idx];
                    if sel.r#type == MenuItemType::NotebookNote {
                        MenuAction::RenameNote(sel.label.clone())
                    } else {
                        MenuAction::None
                    }
                } else {
                    MenuAction::None
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') if self.is_current_notebook() => {
                let level = self.current_level();
                if level.selected_idx < level.items.len() {
                    let sel = &level.items[level.selected_idx];
                    if sel.r#type == MenuItemType::NotebookNote {
                        MenuAction::DeleteNote(sel.label.clone())
                    } else {
                        MenuAction::None
                    }
                } else {
                    MenuAction::None
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                // Number shortcuts 1..9
                let target_num = (c as u8 - b'0') as usize;
                self.activate_shortcut_index(target_num)
            }
            KeyCode::Char(c) if self.show_shortcuts && c.is_ascii_lowercase() => {
                // Letter shortcuts a..z
                let target_num = 10 + (c as u8 - b'a') as usize;
                self.activate_shortcut_index(target_num)
            }
            KeyCode::Char(c) if self.show_shortcuts && c.is_ascii_uppercase() => {
                // Letter shortcuts A..Z
                let target_num = 36 + (c as u8 - b'A') as usize;
                self.activate_shortcut_index(target_num)
            }
            KeyCode::F(1) => MenuAction::Help,
            KeyCode::F(2) => MenuAction::ThemeSelector,
            KeyCode::F(3) => MenuAction::LaunchEditor,
            KeyCode::F(4) => MenuAction::EditMenu,
            KeyCode::F(5) => MenuAction::Reload,
            KeyCode::F(6) => MenuAction::ToggleShortcuts,
            KeyCode::F(7) => MenuAction::ConfigureGutter,
            KeyCode::F(8) => MenuAction::LaunchThemeEditor,
            _ => MenuAction::None,
        }
    }

    /// Handles immediate mouse event (invoking MouseDown immediately)
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> MenuAction {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Check if inside menu area
                if col >= area.x
                    && col < area.x + area.width
                    && row > area.y
                    && row < area.y + area.height - 1
                {
                    let item_start_y = area.y + 2; // Below border and margin
                    if row >= item_start_y {
                        let row_idx = (row - item_start_y) as usize;
                        let level = self.current_level();
                        let item_idx = level.scroll_offset + row_idx;
                        if item_idx < level.items.len() && !level.items[item_idx].is_divider() {
                            self.current_level_mut().selected_idx = item_idx;
                            return self.activate_selected();
                        }
                    }
                }
                MenuAction::None
            }
            MouseEventKind::ScrollDown => {
                self.move_down();
                MenuAction::None
            }
            MouseEventKind::ScrollUp => {
                self.move_up();
                MenuAction::None
            }
            _ => MenuAction::None,
        }
    }

    fn activate_shortcut_index(&mut self, target_num: usize) -> MenuAction {
        let level = self.current_level();
        let mut selectable_count = 0;
        for (idx, item) in level.items.iter().enumerate() {
            if !item.is_divider() {
                selectable_count += 1;
                if selectable_count == target_num {
                    self.current_level_mut().selected_idx = idx;
                    return self.activate_selected();
                }
            }
        }
        MenuAction::None
    }

    fn activate_selected(&mut self) -> MenuAction {
        let level = self.current_level();
        if level.items.is_empty() {
            return MenuAction::None;
        }

        let item = level.items[level.selected_idx].clone();
        if item.r#type == MenuItemType::Submenu || item.submenu.is_some() {
            if let Some(sub) = item.submenu {
                self.stack.push(MenuLevel {
                    title: sub.title,
                    items: sub.items,
                    selected_idx: 0,
                    scroll_offset: 0,
                    is_notebook: false,
                    notebook_file: None,
                });
                self.ensure_selectable();
            }
            return MenuAction::None;
        }

        match item.r#type {
            MenuItemType::Submenu => MenuAction::None,
            MenuItemType::Notebook => MenuAction::OpenNotebook(Box::new(item)),
            MenuItemType::NotebookNote => {
                let filepath = item.file.unwrap_or_default();
                let note_title = item.action.unwrap_or(item.label);
                MenuAction::ViewNote {
                    filepath,
                    note_title,
                }
            }
            MenuItemType::NotebookEmpty => {
                let filepath = item.file.unwrap_or_default();
                MenuAction::NewNote(filepath)
            }
            MenuItemType::Back => self.go_back(),
            MenuItemType::Exit => MenuAction::Exit,
            MenuItemType::ThemeSelector => MenuAction::ThemeSelector,
            MenuItemType::Other(ref s) if s == "notebook" => {
                MenuAction::OpenNotebook(Box::new(item))
            }
            MenuItemType::Other(ref s) if s == "notebook_note" => {
                let filepath = item.file.unwrap_or_default();
                let note_title = item.action.unwrap_or(item.label);
                MenuAction::ViewNote {
                    filepath,
                    note_title,
                }
            }
            MenuItemType::Other(ref s) if s == "notebook_empty" => {
                let filepath = item.file.unwrap_or_default();
                MenuAction::NewNote(filepath)
            }
            MenuItemType::Other(ref s) if s == "theme_selector" => MenuAction::ThemeSelector,
            MenuItemType::Other(ref s) if s == "back" => self.go_back(),
            _ => {
                if let Some(ref act) = item.action {
                    if act == "themeedit" || act == "action_theme_editor" {
                        return MenuAction::LaunchThemeEditor;
                    }
                    if act == "menuedit" || act == "menuedit.py" {
                        return MenuAction::EditMenu;
                    }
                }
                MenuAction::Execute(Box::new(item))
            }
        }
    }

    fn go_back(&mut self) -> MenuAction {
        if self.stack.len() > 1 {
            self.stack.pop();
            MenuAction::None
        } else {
            MenuAction::Back
        }
    }

    /// Renders the menu component into the terminal buffer
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 5 {
            return;
        }

        let w = area.width as usize;
        let b = self.styles.borders.clone();
        let border_style = ratatui::style::Style::default().fg(self.styles.border);

        // Clear background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_style(ratatui::style::Style::default().bg(self.styles.background));
            }
        }

        // 1. Calculate gutter dimensions and layout
        let avail_both = (w.saturating_sub(11)).max(4);
        let left_gutter_w = avail_both / 2;
        let right_gutter_w = avail_both - left_gutter_w;
        let sep_x = (left_gutter_w + 5).min(w.saturating_sub(2));
        let left_seg_len = sep_x.saturating_sub(1);
        let right_seg_len = w.saturating_sub(2 + sep_x);

        let help_raw = if self.is_current_notebook() {
            "[N]: New Note | [ENTER]: Read | [M]: Rename | [D]: Delete | [UP/DN]: Nav | [F1]: Help | [F2]: Themes | [F7]: Gutter | [F8]: ThemeEdit | [ESC]: Back"
        } else if self.show_shortcuts {
            "[UP/DN]: Nav | [0-9/a-z]: Direct | [F1]: Help | [F2]: Themes | [F3]: Editor | [F4]: Edit | [F5]: Refresh | [F6]: Keys | [F7]: Gutter | [F8]: ThemeEdit | [ESC]: Back"
        } else {
            "[UP/DN]: Nav | [ENTER]: Select | [F1]: Help | [F2]: Themes | [F3]: Editor | [F4]: Edit | [F5]: Refresh | [F6]: Keys | [F7]: Gutter | [F8]: ThemeEdit | [ESC]: Back"
        };
        let help_tokens: Vec<&str> = help_raw
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let help_lines = wrap_spread_help_gutter(&help_tokens, left_gutter_w, 2);

        let status_tokens: Vec<&str> = if self.status_gutter.is_empty() {
            Vec::new()
        } else {
            self.status_gutter
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect()
        };
        let status_lines = wrap_gutter_tokens(&status_tokens, right_gutter_w, 2);

        let gutter_rows = (help_lines.len().max(status_lines.len())).clamp(1, 2);
        let gutter_border_y = area.y + area.height.saturating_sub(2 + gutter_rows as u16);

        // 2. Render Header (Top Border) at area.y
        let cfg = self.config.clone();
        let mut macro_ctx = MacroContext::new(&cfg);
        macro_ctx.window_width = w;
        macro_ctx.screen_width = w;
        macro_ctx.window_height = area.height as usize;

        let level = self.current_level();
        let title_raw = interpolate_placeholders(&level.title, &macro_ctx);
        let title_body = format!(" {} ", title_raw);
        let title_cap_len = b.title_left_cap.chars().count() + b.title_right_cap.chars().count();
        let max_body_w = w.saturating_sub(6 + title_cap_len);
        let display_title_body: String = title_body.chars().take(max_body_w).collect();
        let title_len = title_cap_len + display_title_body.chars().count();

        let left_b = 2.max((w.saturating_sub(title_len)) / 2);
        let right_b = w.saturating_sub(left_b + title_len);

        let left_fill = left_b.saturating_sub(1);
        let right_fill = right_b.saturating_sub(1);
        let top_line = format!(
            "{}{}{}{}{}{}{}",
            b.border_top_left,
            b.border_horizontal_top.repeat(left_fill),
            b.title_left_cap,
            display_title_body,
            b.title_right_cap,
            b.border_horizontal_top.repeat(right_fill),
            b.border_top_right
        );
        buf.set_string(area.x, area.y, &top_line, border_style);
        let title_start_x = area.x + 1 + left_fill as u16 + b.title_left_cap.chars().count() as u16;
        buf.set_string(
            title_start_x,
            area.y,
            &display_title_body,
            self.styles.text.add_modifier(Modifier::BOLD),
        );

        // 3. Render Blank Margin row (area.y + 1)
        let margin_y = area.y + 1;
        if margin_y < gutter_border_y {
            buf.set_string(area.x, margin_y, &b.border_vertical_left, border_style);
            buf.set_string(
                area.x + area.width - 1,
                margin_y,
                &b.border_vertical_right,
                border_style,
            );
        }

        // 4. Content Area: menu items and plugins
        let content_start_y = area.y + 2;
        let content_avail_h = gutter_border_y.saturating_sub(content_start_y) as usize;

        // Render side borders for all rows in content area
        for cy in 0..content_avail_h {
            let row_y = content_start_y + cy as u16;
            buf.set_string(area.x, row_y, &b.border_vertical_left, border_style);
            buf.set_string(
                area.x + area.width - 1,
                row_y,
                &b.border_vertical_right,
                border_style,
            );
        }

        let inner_x = area.x + 3;
        let avail_w = w.saturating_sub(6);

        // Calculate scroll offset
        let selected = level.selected_idx;
        let mut offset = level.scroll_offset;
        if selected < offset {
            offset = selected;
        } else if selected >= offset + content_avail_h {
            offset = selected.saturating_sub(content_avail_h.saturating_sub(1));
        }
        self.current_level_mut().scroll_offset = offset;

        let level = self.current_level();
        let mut selectable_num = 0;

        // Reserve space for plugins if height permits (menu priority over plugins invariant)
        let menu_needed_h = level
            .items
            .len()
            .saturating_sub(offset)
            .min(content_avail_h);
        let remaining_h = content_avail_h.saturating_sub(menu_needed_h);
        let active_plugin_rows = if remaining_h > 0 && !self.plugin_rows.is_empty() {
            remaining_h.min(self.plugin_rows.len())
        } else {
            0
        };
        let menu_display_h = content_avail_h.saturating_sub(active_plugin_rows);

        for (idx, item) in level
            .items
            .iter()
            .enumerate()
            .skip(offset)
            .take(menu_display_h)
        {
            let row_y = content_start_y + (idx - offset) as u16;
            if row_y >= gutter_border_y {
                break;
            }

            if item.is_divider() {
                buf.set_string(area.x, row_y, &b.left_tee, border_style);
                buf.set_string(area.x + area.width - 1, row_y, &b.right_tee, border_style);
                let div_char = "─";
                let div_line = div_char.repeat(avail_w);
                buf.set_line(
                    inner_x,
                    row_y,
                    &Line::from(vec![Span::styled(div_line, self.styles.divider)]),
                    avail_w as u16,
                );
                continue;
            }

            selectable_num += 1;
            let is_active = idx == level.selected_idx;

            let indicator = if is_active {
                &self.styles.indicator_symbol
            } else {
                "  "
            };

            let resolved_icon = item
                .icon
                .as_deref()
                .map(|ic| interpolate_placeholders(ic, &macro_ctx));
            let icon_slot = format_icon_slot(resolved_icon.as_deref(), self.use_nerd_fonts);

            let shortcut = if self.show_shortcuts {
                if selectable_num <= 9 {
                    format!("{}. ", selectable_num)
                } else if selectable_num <= 35 {
                    let ch = (b'a' + (selectable_num - 10) as u8) as char;
                    format!("{}. ", ch)
                } else if selectable_num <= 61 {
                    let ch = (b'A' + (selectable_num - 36) as u8) as char;
                    format!("{}. ", ch)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let item_style = if is_active {
                self.styles.highlight
            } else {
                self.styles.text
            };

            let mut spans = vec![
                Span::styled(
                    indicator,
                    if is_active {
                        self.styles.highlight
                    } else {
                        self.styles.text
                    },
                ),
                Span::styled(icon_slot.clone(), item_style),
            ];
            if !shortcut.is_empty() {
                let sc_style = if is_active {
                    Style::default()
                        .fg(self.styles.shortcut_key.fg.unwrap_or(Color::Magenta))
                        .bg(self.styles.highlight.bg.unwrap_or(Color::Cyan))
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.styles.shortcut_key
                };
                spans.push(Span::styled(shortcut.clone(), sc_style));
            }

            // Expand placeholders in item label
            let raw_label = interpolate_placeholders(&item.label, &macro_ctx);
            let (left_label, right_bracket) = split_label_brackets(&raw_label);
            let prefix_w = visible_text_len(indicator)
                + visible_text_len(&icon_slot)
                + visible_text_len(&shortcut);
            let is_submenu = item.r#type == MenuItemType::Submenu || item.submenu.is_some();
            let submenu_w = if is_submenu { 3 } else { 0 };
            let avail_for_content = avail_w.saturating_sub(prefix_w + submenu_w);

            if right_bracket.is_empty() {
                let left_vis = visible_text_len(left_label);
                let text_to_parse = if left_vis > avail_for_content {
                    truncate_to_width(left_label, avail_for_content)
                } else {
                    left_label.to_string()
                };
                let mut parsed_spans = parse_bbcode_spans(&text_to_parse, item_style);
                spans.append(&mut parsed_spans);
            } else {
                let left_vis = visible_text_len(left_label);
                let right_vis = visible_text_len(right_bracket);

                if left_vis + right_vis < avail_for_content {
                    let spacer_len = avail_for_content.saturating_sub(left_vis + right_vis);
                    let mut parsed_left = parse_bbcode_spans(left_label, item_style);
                    spans.append(&mut parsed_left);
                    spans.push(Span::styled(" ".repeat(spacer_len), item_style));
                    let mut parsed_right = parse_bbcode_spans(right_bracket, item_style);
                    spans.append(&mut parsed_right);
                } else {
                    let half = avail_for_content / 2;
                    let max_left = left_vis.min(half).max(1);
                    let max_right = avail_for_content.saturating_sub(max_left + 1);

                    let trunc_left = truncate_to_width(left_label, max_left);
                    let trunc_right = truncate_to_width(right_bracket, max_right);
                    let trunc_left_vis = visible_text_len(&trunc_left);
                    let trunc_right_vis = visible_text_len(&trunc_right);
                    let spacer_len = avail_for_content
                        .saturating_sub(trunc_left_vis + trunc_right_vis)
                        .max(1);

                    let mut parsed_left = parse_bbcode_spans(&trunc_left, item_style);
                    spans.append(&mut parsed_left);
                    spans.push(Span::styled(" ".repeat(spacer_len), item_style));
                    let mut parsed_right = parse_bbcode_spans(&trunc_right, item_style);
                    spans.append(&mut parsed_right);
                }
            }

            if is_submenu {
                let submenu_style = if is_active {
                    item_style
                } else {
                    self.styles.accent
                };
                spans.push(Span::styled("  »", submenu_style));
            }

            // Pad remaining row width with item_style spaces so highlight extends entirely across avail_w
            let used_w: usize = spans.iter().map(|s| visible_text_len(&s.content)).sum();
            let pad_end = avail_w.saturating_sub(used_w);
            if pad_end > 0 {
                spans.push(Span::styled(" ".repeat(pad_end), item_style));
            }

            buf.set_line(inner_x, row_y, &Line::from(spans), avail_w as u16);

            // Ensure unbroken highlight bar across all cells (including wide glyph continuation cells)
            if is_active {
                if let Some(bg) = item_style.bg {
                    for x in inner_x..(inner_x + avail_w as u16) {
                        buf[(x, row_y)].bg = bg;
                    }
                }
            }
        }

        // Render plugins pinned at the bottom of the content area
        if active_plugin_rows > 0 {
            let plugin_start_y =
                (content_start_y + content_avail_h as u16) - active_plugin_rows as u16;
            for (p_idx, prow) in self.plugin_rows.iter().take(active_plugin_rows).enumerate() {
                let py = plugin_start_y + p_idx as u16;
                if prow.is_divider {
                    buf.set_string(area.x, py, &b.left_tee, border_style);
                    buf.set_string(area.x + area.width - 1, py, &b.right_tee, border_style);
                    buf.set_line(
                        inner_x,
                        py,
                        &Line::from(vec![Span::styled(&prow.text, self.styles.divider)]),
                        avail_w as u16,
                    );
                } else {
                    let spans = parse_bbcode_spans(&prow.text, self.styles.text);
                    buf.set_line(inner_x, py, &Line::from(spans), avail_w as u16);
                }
            }
        }

        // 5. Render Gutter Top Border at gutter_border_y
        let gt_line = format!(
            "{}{}{}{}{}",
            b.gutter_left_tee,
            b.gutter_border.repeat(left_seg_len),
            b.gutter_top_tee,
            b.gutter_border.repeat(right_seg_len),
            b.gutter_right_tee
        );
        buf.set_string(area.x, gutter_border_y, &gt_line, border_style);

        // 6. Render Gutter Content Rows
        let status_offset = gutter_rows - status_lines.len();
        for r in 0..gutter_rows {
            let gy = gutter_border_y + 1 + r as u16;
            buf.set_string(area.x, gy, &b.border_vertical_left, border_style);

            let h_text = help_lines.get(r).map(|s| s.as_str()).unwrap_or("");
            let h_spans = parse_bbcode_spans(h_text, self.styles.help_text);
            buf.set_line(area.x + 3, gy, &Line::from(h_spans), left_gutter_w as u16);

            buf.set_string(area.x + sep_x as u16, gy, &b.gutter_separator, border_style);

            let s_idx = r as isize - status_offset as isize;
            let s_text = if s_idx >= 0 {
                status_lines
                    .get(s_idx as usize)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            let s_vis = visible_text_len(s_text);
            let s_pad = right_gutter_w.saturating_sub(s_vis);
            let mut s_spans = vec![Span::styled(" ".repeat(s_pad), self.styles.gutter)];
            s_spans.append(&mut parse_bbcode_spans(s_text, self.styles.gutter));
            buf.set_line(
                area.x + sep_x as u16 + 3,
                gy,
                &Line::from(s_spans),
                right_gutter_w as u16,
            );

            buf.set_string(
                area.x + area.width - 1,
                gy,
                &b.border_vertical_right,
                border_style,
            );
        }

        // 7. Render Window Bottom Border at area.y + area.height - 1
        let bot_y = area.y + area.height - 1;
        let bot_line = format!(
            "{}{}{}{}{}",
            b.border_bottom_left,
            b.border_horizontal_bottom.repeat(left_seg_len),
            b.gutter_bottom_tee,
            b.border_horizontal_bottom.repeat(right_seg_len),
            b.border_bottom_right
        );
        buf.set_string(area.x, bot_y, &bot_line, border_style);

        // 8. Composite background wallpaper if active
        if let Some(compositor) = &mut self.wallpaper {
            if let Some(grid) = compositor.get_grid(
                area.width as usize,
                area.height as usize,
                self.styles.background,
            ) {
                composite_buffer(area, buf, grid, self.styles.background);
            }
        }
    }
}

/// Helper to wrap gutter tokens into at most `max_lines` within `max_w`
fn wrap_gutter_tokens(tokens: &[&str], max_w: usize, max_lines: usize) -> Vec<String> {
    if tokens.is_empty() || max_w == 0 {
        return Vec::new();
    }
    let cleaned: Vec<&str> = tokens
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Vec::new();
    }

    if max_lines == 2 && cleaned.len() > 1 {
        let mid = cleaned.len().div_ceil(2);
        let l1 = cleaned[..mid].join(" | ");
        let l2 = cleaned[mid..].join(" | ");
        if l1.chars().count() <= max_w && l2.chars().count() <= max_w {
            return vec![l1, l2];
        }
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for token in cleaned {
        if current_line.is_empty() {
            current_line.push_str(token);
        } else {
            let candidate_len = current_line.chars().count() + 3 + token.chars().count();
            if candidate_len <= max_w {
                current_line.push_str(" | ");
                current_line.push_str(token);
            } else {
                lines.push(std::mem::take(&mut current_line));
                if lines.len() == max_lines {
                    break;
                }
                current_line.push_str(token);
            }
        }
    }
    if !current_line.is_empty() && lines.len() < max_lines {
        lines.push(current_line);
    }
    for line in &mut lines {
        if line.chars().count() > max_w {
            *line = line.chars().take(max_w).collect();
        }
    }
    lines
}
