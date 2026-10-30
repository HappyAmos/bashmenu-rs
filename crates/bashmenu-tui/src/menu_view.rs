//! Main menu view rendering, viewport management, and keyboard/mouse interaction.

use crate::style::ResolvedThemeStyles;
use bashmenu_core::{format_icon_slot, MenuItem, MenuItemType, MenuSchema};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

/// Outcome of handling a user input event
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// No action needed (selection shifted, scroll, etc.)
    None,
    /// Executes a menu command or script
    Execute(Box<MenuItem>),
    /// Return to parent menu or exit application
    Back,
    /// Open help manual
    Help,
    /// Request menu and config reload
    Reload,
    /// Exit the menu application
    Exit,
}

#[derive(Debug, Clone)]
struct MenuLevel {
    title: String,
    items: Vec<MenuItem>,
    selected_idx: usize,
    scroll_offset: usize,
}

/// The interactive menu widget and state machine
pub struct MenuView {
    stack: Vec<MenuLevel>,
    styles: ResolvedThemeStyles,
    use_nerd_fonts: bool,
    status_gutter: String,
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
        };

        let mut view = Self {
            stack: vec![initial_level],
            styles,
            use_nerd_fonts,
            status_gutter,
        };
        view.ensure_selectable();
        view
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
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                // Number shortcuts 1..9
                let target_num = (c as u8 - b'0') as usize;
                self.activate_shortcut_index(target_num)
            }
            KeyCode::F(1) => MenuAction::Help,
            KeyCode::F(5) => MenuAction::Reload,
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
                    let item_start_y = area.y + 2; // Below border and title
                    if row >= item_start_y {
                        let item_idx = (row - item_start_y) as usize;
                        let level = self.current_level();
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
        match item.r#type {
            MenuItemType::Submenu => {
                if let Some(sub) = item.submenu {
                    self.stack.push(MenuLevel {
                        title: sub.title,
                        items: sub.items,
                        selected_idx: 0,
                        scroll_offset: 0,
                    });
                    self.ensure_selectable();
                }
                MenuAction::None
            }
            MenuItemType::Exit => MenuAction::Exit,
            _ => MenuAction::Execute(Box::new(item)),
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

        let level = self.current_level();
        let title_text = format!(" [ {} ] ", level.title);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(ratatui::style::Style::default().fg(self.styles.border))
            .style(ratatui::style::Style::default().bg(self.styles.background))
            .title(title_text);

        block.render(area, buf);

        let inner_x = area.x + 2;
        let inner_y = area.y + 2;
        let avail_w = area.width.saturating_sub(4) as usize;
        let avail_h = area.height.saturating_sub(4) as usize;

        // Calculate scroll window
        let selected = level.selected_idx;
        let mut offset = level.scroll_offset;
        if selected < offset {
            offset = selected;
        } else if selected >= offset + avail_h {
            offset = selected.saturating_sub(avail_h - 1);
        }
        self.current_level_mut().scroll_offset = offset;

        let level = self.current_level();
        let mut selectable_num = 0;

        for (idx, item) in level.items.iter().enumerate().skip(offset).take(avail_h) {
            let row_y = inner_y + (idx - offset) as u16;
            if row_y >= area.y + area.height - 2 {
                break;
            }

            if item.is_divider() {
                // Render divider rule
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

            // 1. Cursor Indicator
            let indicator = if is_active {
                &self.styles.indicator_symbol
            } else {
                "  "
            };

            // 2. Icon slot (4 columns invariant)
            let icon_slot = format_icon_slot(item.icon.as_deref(), self.use_nerd_fonts);

            // 3. Shortcut badge (e.g. "1. ")
            let shortcut = if selectable_num <= 9 {
                format!("{}. ", selectable_num)
            } else {
                "   ".to_string()
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
                Span::styled(icon_slot, item_style),
                Span::styled(shortcut, self.styles.shortcut_key),
                Span::styled(&item.label, item_style),
            ];

            // If submenu, add indicator badge at the end
            if item.r#type == MenuItemType::Submenu {
                spans.push(Span::styled("  »", self.styles.accent));
            }

            buf.set_line(inner_x, row_y, &Line::from(spans), avail_w as u16);
        }

        // Render Status Gutter at bottom border row
        let gutter_y = area.y + area.height - 1;
        let nav_help = " [↑/↓] Navigate  [Enter] Select  [Esc] Back ";
        buf.set_string(
            area.x + 2,
            gutter_y,
            nav_help,
            self.styles.gutter.add_modifier(Modifier::DIM),
        );

        if !self.status_gutter.is_empty() {
            let max_stat_w = (area.width / 2) as usize;
            let stat_str: String = self.status_gutter.chars().take(max_stat_w).collect();
            let stat_x = (area.x + area.width).saturating_sub(stat_str.len() as u16 + 2);
            buf.set_string(stat_x, gutter_y, &stat_str, self.styles.gutter);
        }
    }
}
