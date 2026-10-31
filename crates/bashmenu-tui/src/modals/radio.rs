//! Radio selection modal dialog presenting options with radio buttons.

use super::frame::{compute_modal_inner, is_close_button_clicked, render_modal_frame};
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Interactive Radio modal dialog matching Python Textual `RadioModalScreen`
#[derive(Debug, Clone)]
pub struct RadioModal {
    pub title: String,
    pub message: String,
    pub options: Vec<(String, String)>, // (value, label)
    pub selected_val: String,
    pub cursor_idx: usize,
    pub focused_btn: Option<usize>, // None = list, Some(0) = Ok, Some(1) = Cancel
}

impl RadioModal {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        options: Vec<(String, String)>,
        current_val: impl Into<String>,
    ) -> Self {
        let selected_val = current_val.into();
        let mut cursor_idx = 0;
        for (idx, (val, _)) in options.iter().enumerate() {
            if val == &selected_val {
                cursor_idx = idx;
                break;
            }
        }
        Self {
            title: title.into(),
            message: message.into(),
            options,
            selected_val,
            cursor_idx,
            focused_btn: None,
        }
    }

    /// Handles keyboard input. Returns `Some(Some(val))` on submit, `Some(None)` on cancel, `None` while pending.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<String>> {
        match code {
            KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => Some(None),
            KeyCode::Char(ch) if ch.is_ascii_digit() && ch != '0' => {
                let idx = (ch as u8 - b'1') as usize;
                if idx < self.options.len() {
                    self.cursor_idx = idx;
                    self.selected_val = self.options[idx].0.clone();
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.focused_btn.is_some() {
                    self.focused_btn = None;
                } else if self.cursor_idx > 0 {
                    self.cursor_idx -= 1;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.focused_btn.is_none() {
                    if self.cursor_idx + 1 < self.options.len() {
                        self.cursor_idx += 1;
                    } else {
                        self.focused_btn = Some(0);
                    }
                }
                None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(btn) = self.focused_btn {
                    self.focused_btn = Some(if btn == 0 { 1 } else { 0 });
                }
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(btn) = self.focused_btn {
                    self.focused_btn = Some(if btn == 0 { 1 } else { 0 });
                }
                None
            }
            KeyCode::Tab => {
                self.focused_btn = match self.focused_btn {
                    None => Some(0),
                    Some(0) => Some(1),
                    Some(_) => None,
                };
                None
            }
            KeyCode::BackTab => {
                self.focused_btn = match self.focused_btn {
                    None => Some(1),
                    Some(1) => Some(0),
                    Some(_) => None,
                };
                None
            }
            KeyCode::Char(' ') => {
                if self.focused_btn.is_none() && self.cursor_idx < self.options.len() {
                    self.selected_val = self.options[self.cursor_idx].0.clone();
                }
                None
            }
            KeyCode::Enter => {
                match self.focused_btn {
                    Some(1) => Some(None), // Cancel
                    _ => {
                        // If on list, select cursor item and submit
                        if self.cursor_idx < self.options.len() {
                            self.selected_val = self.options[self.cursor_idx].0.clone();
                        }
                        Some(Some(self.selected_val.clone()))
                    }
                }
            }
            _ => None,
        }
    }

    /// Handles immediate mouse click
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<String>> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            let dialog_w = 64_u16;
            let dialog_h = (self.options.len() as u16 + 7).clamp(8, 20);
            let inner_area = if area.width <= dialog_w {
                area
            } else {
                compute_modal_inner(area, dialog_w, dialog_h)
            };

            if is_close_button_clicked(inner_area, row, col) {
                return Some(None);
            }

            // Check option clicks
            let opt_start_y = inner_area.y + 3;
            for (idx, (val, _)) in self.options.iter().enumerate() {
                let opt_y = opt_start_y + idx as u16;
                if row == opt_y && col >= inner_area.x && col < inner_area.x + inner_area.width {
                    self.cursor_idx = idx;
                    self.selected_val = val.clone();
                    self.focused_btn = None;
                    return None;
                }
            }

            // Check button clicks
            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);
            if row == btn_y {
                let center_x = inner_area.x + inner_area.width / 2;
                if col < center_x {
                    return Some(Some(self.selected_val.clone()));
                } else {
                    return Some(None);
                }
            }
        }
        None
    }

    /// Renders the radio modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 64_u16;
        let dialog_h = (self.options.len() as u16 + 7).clamp(8, 20);

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Message Text
        let msg_y = inner.y + 1;
        let msg_x = inner.x + 2;
        let avail_w = inner.width.saturating_sub(4) as usize;
        let truncated: String = self.message.chars().take(avail_w).collect();
        buf.set_string(msg_x, msg_y, &truncated, styles.text);

        // 2. Render Options with Radio Buttons
        let opt_start_y = inner.y + 3;
        for (idx, (val, label)) in self.options.iter().enumerate() {
            let opt_y = opt_start_y + idx as u16;
            if opt_y >= inner.y + inner.height.saturating_sub(2) {
                break;
            }

            let is_checked = val == &self.selected_val;
            let is_cursor = idx == self.cursor_idx && self.focused_btn.is_none();

            let radio_mark = if is_checked { "(●)" } else { "(○)" };
            let opt_text = format!(" {} {}. {}", radio_mark, idx + 1, label);
            let display_text: String = opt_text.chars().take(avail_w).collect();

            let row_style = if is_cursor {
                styles.accent.add_modifier(Modifier::REVERSED)
            } else if is_checked {
                styles.accent
            } else {
                styles.text
            };

            buf.set_string(inner.x + 2, opt_y, &display_text, row_style);
        }

        // 3. Render Buttons: [ Ok ]  [ Cancel ]
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;

        let ok_focused = self.focused_btn == Some(0);
        let cancel_focused = self.focused_btn == Some(1);

        let ok_str = if ok_focused {
            " [  Ok  ] "
        } else {
            "    Ok    "
        };
        let cancel_str = if cancel_focused {
            " [ Cancel ] "
        } else {
            "   Cancel   "
        };

        let ok_style = if ok_focused {
            styles.accent.add_modifier(Modifier::REVERSED)
        } else {
            styles.text
        };
        let cancel_style = if cancel_focused {
            styles.accent.add_modifier(Modifier::REVERSED)
        } else {
            styles.text
        };

        buf.set_string(center_x.saturating_sub(14), btn_y, ok_str, ok_style);
        buf.set_string(center_x + 2, btn_y, cancel_str, cancel_style);

        inner
    }
}
