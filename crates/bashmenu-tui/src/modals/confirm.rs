//! Confirmation modal dialog (Yes / No).

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Interactive Confirm modal dialog
#[derive(Debug, Clone)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub selected_yes: bool,
}

impl ConfirmModal {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            selected_yes: true,
        }
    }

    /// Handles keyboard input. Returns `Some(true)` for Yes, `Some(false)` for No, `None` while pending.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<bool> {
        match code {
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Tab
            | KeyCode::Char('h')
            | KeyCode::Char('l') => {
                self.selected_yes = !self.selected_yes;
                None
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => Some(true),
            KeyCode::Char('n') | KeyCode::Char('N') => Some(false),
            KeyCode::Enter => Some(self.selected_yes),
            KeyCode::Esc => Some(false),
            _ => None,
        }
    }

    /// Handles immediate mouse click
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<bool> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            let dialog_w = (self.message.len() as u16 + 8).clamp(32, 70);
            let inner_area = if area.width <= dialog_w {
                area
            } else {
                super::frame::compute_modal_inner(area, dialog_w, 8)
            };

            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return Some(false);
            }

            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);

            if row == btn_y {
                // Check if clicked in left half (Yes) or right half (No)
                let center_x = inner_area.x + inner_area.width / 2;
                if col < center_x {
                    return Some(true);
                } else {
                    return Some(false);
                }
            }
        }
        None
    }

    /// Renders the confirm modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (self.message.len() as u16 + 8).max(42);
        let dialog_h = 8;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Message Text
        let msg_y = inner.y + 1;
        let msg_x = inner.x + 2;
        buf.set_string(msg_x, msg_y, &self.message, styles.text);

        // 2. Render Action Buttons
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;

        let yes_str = if self.selected_yes {
            " [ Yes ] "
        } else {
            "   Yes   "
        };
        let no_str = if !self.selected_yes {
            " [  No ] "
        } else {
            "    No   "
        };

        let yes_style = if self.selected_yes {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            styles.text
        };

        let no_style = if !self.selected_yes {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            styles.text
        };

        let yes_x = center_x.saturating_sub(12);
        let no_x = center_x + 3;

        buf.set_string(yes_x, btn_y, yes_str, yes_style);
        buf.set_string(no_x, btn_y, no_str, no_style);

        inner
    }
}
