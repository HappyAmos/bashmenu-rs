//! Confirmation modal dialog (Yes / No / Cancel).

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Tri-state confirmation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Yes,
    No,
    Cancel,
}

impl ConfirmChoice {
    pub fn is_yes(&self) -> bool {
        matches!(self, ConfirmChoice::Yes)
    }

    pub fn is_no(&self) -> bool {
        matches!(self, ConfirmChoice::No)
    }

    pub fn is_cancel(&self) -> bool {
        matches!(self, ConfirmChoice::Cancel)
    }
}

/// Interactive Confirm modal dialog with Yes, No, and Cancel buttons.
#[derive(Debug, Clone)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub selected_index: usize, // 0: Yes, 1: No, 2: Cancel
    pub selected_yes: bool,
}

impl ConfirmModal {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            selected_index: 0,
            selected_yes: true,
        }
    }

    fn sync_yes(&mut self) {
        self.selected_yes = self.selected_index == 0;
    }

    /// Handles keyboard input.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<ConfirmChoice> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_index == 0 {
                    self.selected_index = 2;
                } else {
                    self.selected_index -= 1;
                }
                self.sync_yes();
                None
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.selected_index = (self.selected_index + 1) % 3;
                self.sync_yes();
                None
            }
            KeyCode::BackTab => {
                if self.selected_index == 0 {
                    self.selected_index = 2;
                } else {
                    self.selected_index -= 1;
                }
                self.sync_yes();
                None
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => Some(ConfirmChoice::Yes),
            KeyCode::Char('n') | KeyCode::Char('N') => Some(ConfirmChoice::No),
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => Some(ConfirmChoice::Cancel),
            KeyCode::Enter => match self.selected_index {
                0 => Some(ConfirmChoice::Yes),
                1 => Some(ConfirmChoice::No),
                _ => Some(ConfirmChoice::Cancel),
            },
            _ => None,
        }
    }

    /// Handles immediate mouse click.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<ConfirmChoice> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            let dialog_w = (self.message.len() as u16 + 8).clamp(48, 70);
            let inner_area = if area.width <= dialog_w {
                area
            } else {
                super::frame::compute_modal_inner(area, dialog_w, 8)
            };

            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return Some(ConfirmChoice::Cancel);
            }

            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);

            if row == btn_y {
                let total_btns_w: u16 = 34;
                let start_x = inner_area.x + inner_area.width.saturating_sub(total_btns_w) / 2;

                if col >= start_x && col < start_x + 10 {
                    return Some(ConfirmChoice::Yes);
                } else if col >= start_x + 10 && col < start_x + 21 {
                    return Some(ConfirmChoice::No);
                } else if col >= start_x + 21 && col <= start_x + total_btns_w + 2 {
                    return Some(ConfirmChoice::Cancel);
                }
            }
        }
        None
    }

    /// Renders the confirm modal with Yes, No, Cancel buttons.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (self.message.len() as u16 + 8).clamp(48, 70);
        let dialog_h = 8;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Message Text
        let msg_y = inner.y + 1;
        let msg_x = inner.x + 2;
        buf.set_string(msg_x, msg_y, &self.message, styles.text);

        // 2. Render Action Buttons [ Yes ] [  No ] [ Cancel ]
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let total_btns_w: u16 = 34;
        let start_x = inner.x + inner.width.saturating_sub(total_btns_w) / 2;

        let yes_str = if self.selected_index == 0 {
            " [ Yes ] "
        } else {
            "   Yes   "
        };
        let no_str = if self.selected_index == 1 {
            " [  No ] "
        } else {
            "    No   "
        };
        let cancel_str = if self.selected_index == 2 {
            " [ Cancel ] "
        } else {
            "   Cancel   "
        };

        let yes_style = if self.selected_index == 0 {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            styles.accent.add_modifier(Modifier::BOLD)
        };

        let no_style = if self.selected_index == 1 {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightRed)
        };

        let cancel_style = if self.selected_index == 2 {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let yes_x = start_x;
        let no_x = start_x + 11;
        let cancel_x = start_x + 22;

        buf.set_string(yes_x, btn_y, yes_str, yes_style);
        buf.set_string(no_x, btn_y, no_str, no_style);
        buf.set_string(cancel_x, btn_y, cancel_str, cancel_style);

        inner
    }
}
