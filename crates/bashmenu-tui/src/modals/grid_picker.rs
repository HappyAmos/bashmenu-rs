//! 16x16 2D grid pickers for 256 colors and CP437 ASCII characters.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use bashmenu_core::cp437_to_char;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Interactive 16x16 2D color grid picker (0..=255)
#[derive(Debug, Clone)]
pub struct ColorGridModal {
    pub title: String,
    pub selected_code: u8,
}

impl ColorGridModal {
    pub fn new(title: impl Into<String>, initial_code: u8) -> Self {
        Self {
            title: title.into(),
            selected_code: initial_code,
        }
    }

    /// Handles keyboard navigation across the 16x16 grid (±1, ±16, ±64)
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<u8>> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_code = self.selected_code.saturating_sub(1);
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected_code = self.selected_code.saturating_add(1);
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_code = self.selected_code.saturating_sub(16);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_code <= 239 {
                    self.selected_code += 16;
                }
                None
            }
            KeyCode::PageUp => {
                self.selected_code = self.selected_code.saturating_sub(64);
                None
            }
            KeyCode::PageDown => {
                if self.selected_code <= 191 {
                    self.selected_code += 64;
                }
                None
            }
            KeyCode::Home => {
                self.selected_code = 0;
                None
            }
            KeyCode::End => {
                self.selected_code = 255;
                None
            }
            KeyCode::Enter => Some(Some(self.selected_code)),
            KeyCode::Esc => Some(None),
            _ => None,
        }
    }

    /// Handles immediate mouse click on the 16x16 grid
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        grid_start: (u16, u16),
    ) -> Option<Option<u8>> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let (gx, gy) = grid_start;
            let col = mouse.column;
            let row = mouse.row;

            if col >= gx && col < gx + (16 * 3) && row >= gy && row < gy + 16 {
                let cell_x = ((col - gx) / 3) as u8;
                let cell_y = (row - gy) as u8;
                let code = cell_y * 16 + cell_x;
                self.selected_code = code;
                return Some(Some(code));
            }
        }
        None
    }

    /// Renders the 16x16 color grid modal
    pub fn render(
        &self,
        area: Rect,
        styles: &ResolvedThemeStyles,
        buf: &mut Buffer,
    ) -> (Rect, (u16, u16)) {
        let dialog_w = 56;
        let dialog_h = 22;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let grid_x = inner.x + 2;
        let grid_y = inner.y + 1;

        for row in 0..16u8 {
            for col in 0..16u8 {
                let code = row * 16 + col;
                let px = grid_x + (col as u16 * 3);
                let py = grid_y + row as u16;

                let is_selected = code == self.selected_code;
                let cell_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Indexed(code))
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().bg(Color::Indexed(code)).fg(Color::Reset)
                };

                let cell_str = if is_selected { "► " } else { "  " };
                buf.set_string(px, py, cell_str, cell_style);
            }
        }

        // Status badge: Selected code
        let badge_y = inner.y + inner.height.saturating_sub(1);
        let badge = format!(" Color ID: {} (Enter to confirm) ", self.selected_code);
        buf.set_string(inner.x + 2, badge_y, &badge, styles.gutter);

        (inner, (grid_x, grid_y))
    }
}

/// Interactive 16x16 2D CP437 ASCII character picker (0..=255)
#[derive(Debug, Clone)]
pub struct AsciiGridModal {
    pub title: String,
    pub selected_code: u8,
}

impl AsciiGridModal {
    pub fn new(title: impl Into<String>, initial_code: u8) -> Self {
        Self {
            title: title.into(),
            selected_code: initial_code,
        }
    }

    /// Handles keyboard navigation across CP437 ASCII grid
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<char>> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_code = self.selected_code.saturating_sub(1);
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected_code = self.selected_code.saturating_add(1);
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_code = self.selected_code.saturating_sub(16);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_code <= 239 {
                    self.selected_code += 16;
                }
                None
            }
            KeyCode::PageUp => {
                self.selected_code = self.selected_code.saturating_sub(64);
                None
            }
            KeyCode::PageDown => {
                if self.selected_code <= 191 {
                    self.selected_code += 64;
                }
                None
            }
            KeyCode::Home => {
                self.selected_code = 0;
                None
            }
            KeyCode::End => {
                self.selected_code = 255;
                None
            }
            KeyCode::Enter => Some(Some(cp437_to_char(self.selected_code))),
            KeyCode::Esc => Some(None),
            _ => None,
        }
    }

    /// Handles immediate mouse click on the 16x16 CP437 grid
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        grid_start: (u16, u16),
    ) -> Option<Option<char>> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let (gx, gy) = grid_start;
            let col = mouse.column;
            let row = mouse.row;

            if col >= gx && col < gx + (16 * 3) && row >= gy && row < gy + 16 {
                let cell_x = ((col - gx) / 3) as u8;
                let cell_y = (row - gy) as u8;
                let code = cell_y * 16 + cell_x;
                self.selected_code = code;
                return Some(Some(cp437_to_char(code)));
            }
        }
        None
    }

    /// Renders CP437 ASCII grid modal
    pub fn render(
        &self,
        area: Rect,
        styles: &ResolvedThemeStyles,
        buf: &mut Buffer,
    ) -> (Rect, (u16, u16)) {
        let dialog_w = 56;
        let dialog_h = 22;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let grid_x = inner.x + 2;
        let grid_y = inner.y + 1;

        for row in 0..16u8 {
            for col in 0..16u8 {
                let code = row * 16 + col;
                let px = grid_x + (col as u16 * 3);
                let py = grid_y + row as u16;

                let is_selected = code == self.selected_code;
                let ch = cp437_to_char(code);

                let cell_style = if is_selected {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                };

                let cell_str = format!(" {} ", ch);
                buf.set_string(px, py, &cell_str, cell_style);
            }
        }

        // Status badge: Selected ASCII code & character
        let badge_y = inner.y + inner.height.saturating_sub(1);
        let badge = format!(
            " Code: {} ('{}') - Enter to select ",
            self.selected_code,
            cp437_to_char(self.selected_code)
        );
        buf.set_string(inner.x + 2, badge_y, &badge, styles.gutter);

        (inner, (grid_x, grid_y))
    }
}
