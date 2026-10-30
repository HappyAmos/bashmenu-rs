//! Single-line text input modal prompt dialog.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Interactive text input prompt modal
#[derive(Debug, Clone)]
pub struct InputModal {
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub cursor_pos: usize,
}

impl InputModal {
    pub fn new(title: impl Into<String>, prompt: impl Into<String>, initial_val: &str) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            value: initial_val.to_string(),
            cursor_pos: initial_val.len(),
        }
    }

    /// Handles keyboard input.
    /// Returns:
    /// - `None` while pending
    /// - `Some(Some(value))` when user submits with Enter
    /// - `Some(None)` when user cancels with Esc
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<String>> {
        match code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                None
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.value.remove(self.cursor_pos);
                }
                None
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.value.len() {
                    self.value.remove(self.cursor_pos);
                }
                None
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            KeyCode::Right => {
                if self.cursor_pos < self.value.len() {
                    self.cursor_pos += 1;
                }
                None
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                None
            }
            KeyCode::End => {
                self.cursor_pos = self.value.len();
                None
            }
            KeyCode::Enter => Some(Some(self.value.clone())),
            KeyCode::Esc => Some(None),
            _ => None,
        }
    }

    /// Handles mouse clicks on OK or Cancel buttons
    pub fn handle_mouse(&mut self, mouse: MouseEvent, inner_area: Rect) -> Option<Option<String>> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;
            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);

            if row == btn_y {
                let center_x = inner_area.x + inner_area.width / 2;
                if col < center_x {
                    return Some(Some(self.value.clone()));
                } else {
                    return Some(None);
                }
            }
        }
        None
    }

    /// Renders the input modal dialog
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 60.min(area.width.saturating_sub(4));
        let dialog_h = 9;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Prompt label
        buf.set_string(inner.x + 2, inner.y + 1, &self.prompt, styles.text);

        // 2. Input Box Field
        let input_y = inner.y + 3;
        let input_x = inner.x + 2;
        let input_w = inner.width.saturating_sub(4) as usize;

        // Render input field background bar
        let field_str = format!("{:<width$}", self.value, width = input_w);
        buf.set_string(input_x, input_y, &field_str, styles.highlight);

        // Render cursor indicator
        if self.cursor_pos < input_w {
            let cx = input_x + self.cursor_pos as u16;
            let ch = self.value.chars().nth(self.cursor_pos).unwrap_or(' ');
            buf[(cx, input_y)]
                .set_char(ch)
                .set_style(styles.highlight.add_modifier(Modifier::REVERSED));
        }

        // 3. Action Buttons [ OK ] and [ Cancel ]
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;
        let ok_x = center_x.saturating_sub(14);
        let cancel_x = center_x + 2;

        buf.set_string(
            ok_x,
            btn_y,
            " [ OK ] ",
            styles.highlight.add_modifier(Modifier::BOLD),
        );
        buf.set_string(cancel_x, btn_y, " [ Cancel ] ", styles.text);

        inner
    }
}
