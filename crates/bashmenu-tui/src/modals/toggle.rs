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
    /// True/Enabled option selected
    True,
    /// False/Disabled option selected
    False,
    /// Cancelled without change
    Cancel,
}

/// Interactive Toggle modal dialog matching Python Textual `ToggleModalScreen`
#[derive(Debug, Clone)]
pub struct ToggleModal {
    /// Dialog title header
    pub title: String,
    /// Description or prompt message
    pub message: String,
    /// Currently focused button index (0 = True, 1 = False, 2 = Cancel)
    pub focused_idx: usize,
}

impl ToggleModal {
    /// Constructs a toggle modal with the given title and message.
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

    /// Computes dialog dimensions and inner rect
    fn compute_layout(&self, area: Rect) -> (Rect, Vec<String>) {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = compute_modal_inner(area, dialog_w, dialog_h);
        let max_w = 70_u16.min(area.width.saturating_sub(4));
        let avail_w = max_w.saturating_sub(6) as usize;
        let msg_lines = if avail_w > 0 && !self.message.trim().is_empty() {
            super::input::wrap_prompt_text(&self.message, avail_w)
        } else {
            Vec::new()
        };
        (inner, msg_lines)
    }

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let max_w = 70_u16.min(area.width.saturating_sub(4));
        let avail_w = max_w.saturating_sub(6) as usize;
        let msg_lines = if avail_w > 0 && !self.message.trim().is_empty() {
            super::input::wrap_prompt_text(&self.message, avail_w)
        } else {
            Vec::new()
        };
        let msg_count = msg_lines.len().max(1);
        let max_h = area.height.saturating_sub(2);
        let dialog_w = (self.message.len() as u16 + 12)
            .min(max_w)
            .max(20.min(max_w));
        let dialog_h = ((msg_count + 5) as u16).min(max_h).max(7.min(max_h));
        (dialog_w, dialog_h)
    }

    /// Handles immediate mouse click
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<ToggleChoice> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;
            let (inner_area, _) = self.compute_layout(area);

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
        let max_w = 70_u16.min(area.width.saturating_sub(4));
        let avail_w = max_w.saturating_sub(6) as usize;
        let msg_lines = if avail_w > 0 && !self.message.trim().is_empty() {
            super::input::wrap_prompt_text(&self.message, avail_w)
        } else {
            Vec::new()
        };
        let msg_count = msg_lines.len().max(1);
        let max_h = area.height.saturating_sub(2);
        let dialog_w = (self.message.len() as u16 + 12)
            .min(max_w)
            .max(20.min(max_w));
        let dialog_h = ((msg_count + 5) as u16).min(max_h).max(7.min(max_h));

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);
        let inner_avail_w = inner.width.saturating_sub(4) as usize;

        // 1. Render Message Text (wrapped and clipped within inner width)
        for (i, line) in msg_lines.iter().enumerate() {
            let py = inner.y + 1 + i as u16;
            if py < inner.y + inner.height.saturating_sub(2) {
                let clipped: String = line.chars().take(inner_avail_w).collect();
                buf.set_string(inner.x + 2, py, &clipped, styles.text);
            }
        }

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
        let right_bound = inner.x + inner.width;

        if t_x < right_bound {
            let max_w = (right_bound - t_x) as usize;
            let clipped: String = true_str.chars().take(max_w).collect();
            buf.set_string(t_x, btn_y, &clipped, true_style);
        }
        if f_x < right_bound {
            let max_w = (right_bound - f_x) as usize;
            let clipped: String = false_str.chars().take(max_w).collect();
            buf.set_string(f_x, btn_y, &clipped, false_style);
        }
        if c_x < right_bound {
            let max_w = (right_bound - c_x) as usize;
            let clipped: String = cancel_str.chars().take(max_w).collect();
            buf.set_string(c_x, btn_y, &clipped, cancel_style);
        }

        inner
    }
}
