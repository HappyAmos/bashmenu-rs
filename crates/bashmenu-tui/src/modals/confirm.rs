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
    /// User confirmed the affirmative action
    Yes,
    /// User confirmed the negative action
    No,
    /// User cancelled or dismissed the modal
    Cancel,
}

impl ConfirmChoice {
    /// Returns `true` if affirmative option was selected.
    pub fn is_yes(&self) -> bool {
        matches!(self, ConfirmChoice::Yes)
    }

    /// Returns `true` if negative option was selected.
    pub fn is_no(&self) -> bool {
        matches!(self, ConfirmChoice::No)
    }

    /// Returns `true` if cancel/abort option was selected.
    pub fn is_cancel(&self) -> bool {
        matches!(self, ConfirmChoice::Cancel)
    }
}

/// Interactive Confirm modal dialog with Yes, No, and Cancel buttons.
#[derive(Debug, Clone)]
pub struct ConfirmModal {
    /// Modal header title
    pub title: String,
    /// Modal prompt or confirmation query text
    pub message: String,
    /// Currently focused button index (0: Yes, 1: No, 2: Cancel)
    pub selected_index: usize,
    /// Convenience boolean reflecting if Yes button is selected
    pub selected_yes: bool,
}

impl ConfirmModal {
    /// Constructs a new confirmation dialog with the given title and prompt text.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            selected_index: 0,
            selected_yes: true,
        }
    }

    /// Synchronizes the `selected_yes` helper boolean with `selected_index == 0`.
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
        let max_w = 70.min(area.width.saturating_sub(4));
        let wrapped =
            super::input::wrap_prompt_text(&self.message, max_w.saturating_sub(4) as usize);
        let max_line_w = wrapped
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(20);
        let dialog_w = (max_line_w as u16 + 8).clamp(48, max_w);
        let dialog_h = (wrapped.len() as u16 + 5).clamp(8, area.height.saturating_sub(2));

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Message Text (wrapped and clamped to visible inner area)
        for (i, line) in wrapped.iter().enumerate() {
            let y = inner.y + 1 + i as u16;
            if y < inner.y + inner.height.saturating_sub(2) {
                let clipped: String = line
                    .chars()
                    .take(inner.width.saturating_sub(4) as usize)
                    .collect();
                buf.set_string(inner.x + 2, y, &clipped, styles.text);
            }
        }

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
