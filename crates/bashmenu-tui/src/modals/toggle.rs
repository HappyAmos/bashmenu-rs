//! Toggle modal dialog for boolean configuration options (True / False / Cancel).

use super::frame::{compute_modal_inner, is_close_button_clicked, render_modal_frame};
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// User choice from a toggle dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleChoice {
    True,
    False,
    Cancel,
}

/// Interactive Toggle modal dialog matching Python Textual `ToggleModalScreen`
#[derive(Debug, Clone)]
pub struct ToggleModal {
    pub title: String,
    pub message: String,
    pub focused_idx: usize, // 0 = True, 1 = False, 2 = Cancel
}

impl ToggleModal {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            focused_idx: 0,
        }
    }

    /// Handles keyboard input. Returns `Some(choice)` when an action is selected, `None` while pending.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<ToggleChoice> {
        match code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                self.focused_idx = if self.focused_idx == 0 {
                    2
                } else {
                    self.focused_idx - 1
                };
                None
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.focused_idx = (self.focused_idx + 1) % 3;
                None
            }
            KeyCode::Char('t') | KeyCode::Char('T') => Some(ToggleChoice::True),
            KeyCode::Char('f') | KeyCode::Char('F') => Some(ToggleChoice::False),
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => Some(ToggleChoice::Cancel),
            KeyCode::Enter | KeyCode::Char(' ') => match self.focused_idx {
                0 => Some(ToggleChoice::True),
                1 => Some(ToggleChoice::False),
                _ => Some(ToggleChoice::Cancel),
            },
            _ => None,
        }
    }

    /// Handles immediate mouse click
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<ToggleChoice> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            let dialog_w = (self.message.len() as u16 + 12).clamp(42, 70);
            let inner_area = if area.width <= dialog_w {
                area
            } else {
                compute_modal_inner(area, dialog_w, 8)
            };

            if is_close_button_clicked(inner_area, row, col) {
                return Some(ToggleChoice::Cancel);
            }

            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);
            if row == btn_y {
                let third = inner_area.width / 3;
                let rel_x = col.saturating_sub(inner_area.x);
                if rel_x < third {
                    return Some(ToggleChoice::True);
                } else if rel_x < third * 2 {
                    return Some(ToggleChoice::False);
                } else {
                    return Some(ToggleChoice::Cancel);
                }
            }
        }
        None
    }

    /// Renders the toggle modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (self.message.len() as u16 + 12).clamp(42, 70);
        let dialog_h = 8;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Message Text
        let msg_y = inner.y + 1;
        let msg_x = inner.x + 2;
        let avail_w = inner.width.saturating_sub(4) as usize;
        let truncated: String = self.message.chars().take(avail_w).collect();
        buf.set_string(msg_x, msg_y, &truncated, styles.text);

        // 2. Render Action Buttons: [ True ]  [ False ]  [ Cancel ]
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let third = inner.width / 3;

        let true_str = if self.focused_idx == 0 {
            " [  True  ] "
        } else {
            "    True    "
        };
        let false_str = if self.focused_idx == 1 {
            " [ False  ] "
        } else {
            "   False    "
        };
        let cancel_str = if self.focused_idx == 2 {
            " [ Cancel ] "
        } else {
            "   Cancel   "
        };

        let true_style = if self.focused_idx == 0 {
            styles.accent.add_modifier(Modifier::REVERSED)
        } else {
            styles.text
        };
        let false_style = if self.focused_idx == 1 {
            styles.accent.add_modifier(Modifier::REVERSED)
        } else {
            styles.text
        };
        let cancel_style = if self.focused_idx == 2 {
            styles.accent.add_modifier(Modifier::REVERSED)
        } else {
            styles.text
        };

        let t_x = inner.x + 2;
        let f_x = inner.x + third + 2;
        let c_x = inner.x + third * 2 + 2;

        buf.set_string(t_x, btn_y, true_str, true_style);
        buf.set_string(f_x, btn_y, false_str, false_style);
        buf.set_string(c_x, btn_y, cancel_str, cancel_style);

        inner
    }
}
