//! Single-line text input modal prompt dialog.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
    pub scroll_offset: usize,
    pub selection_anchor: Option<usize>,
    pub masked: bool,
}

impl InputModal {
    pub fn new(title: impl Into<String>, prompt: impl Into<String>, initial_val: &str) -> Self {
        let char_count = initial_val.chars().count();
        Self {
            title: title.into(),
            prompt: prompt.into(),
            value: initial_val.to_string(),
            cursor_pos: char_count,
            scroll_offset: 0,
            selection_anchor: None,
            masked: false,
        }
    }

    /// Configures whether input characters are visually masked with asterisks (`*`).
    pub fn with_masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Returns the active selection character index range `(start, end)` if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.and_then(|anchor| {
            let count = self.value.chars().count();
            let c = self.cursor_pos.min(count);
            let a = anchor.min(count);
            if a != c {
                Some((a.min(c), a.max(c)))
            } else {
                None
            }
        })
    }

    /// Deletes currently selected text range, returning true if a selection was removed.
    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let mut chars: Vec<char> = self.value.chars().collect();
            chars.drain(start..end);
            self.value = chars.into_iter().collect();
            self.cursor_pos = start;
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    /// Handles keyboard input with modifiers.
    pub fn handle_key_with_modifiers(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<Option<String>> {
        let char_count = self.value.chars().count();
        if self.cursor_pos > char_count {
            self.cursor_pos = char_count;
        }

        let is_shift = modifiers.contains(KeyModifiers::SHIFT);
        let is_ctrl = modifiers.contains(KeyModifiers::CONTROL);

        // Ctrl shortcuts
        if is_ctrl {
            match code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.selection_anchor = Some(0);
                    self.cursor_pos = self.value.chars().count();
                    return None;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String =
                            self.value.chars().skip(start).take(end - start).collect();
                        crate::clipboard::set_clipboard_text(&snippet);
                    }
                    return None;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String =
                            self.value.chars().skip(start).take(end - start).collect();
                        crate::clipboard::set_clipboard_text(&snippet);
                        self.delete_selection();
                    }
                    return None;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = crate::clipboard::get_clipboard_text() {
                        self.delete_selection();
                        let clean: String =
                            text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                        if !clean.is_empty() {
                            let mut chars: Vec<char> = self.value.chars().collect();
                            for (idx, ch) in clean.chars().enumerate() {
                                chars.insert(self.cursor_pos + idx, ch);
                            }
                            self.cursor_pos += clean.chars().count();
                            self.value = chars.into_iter().collect();
                        }
                    }
                    return None;
                }
                KeyCode::Left => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some(self.cursor_pos);
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.move_word_left();
                    return None;
                }
                KeyCode::Right => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some(self.cursor_pos);
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.move_word_right();
                    return None;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Char(c) => {
                self.delete_selection();
                let mut chars: Vec<char> = self.value.chars().collect();
                chars.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                self.value = chars.into_iter().collect();
                None
            }
            KeyCode::Backspace => {
                if !self.delete_selection() && self.cursor_pos > 0 {
                    let mut chars: Vec<char> = self.value.chars().collect();
                    if self.cursor_pos <= chars.len() {
                        chars.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                        self.value = chars.into_iter().collect();
                    }
                }
                None
            }
            KeyCode::Delete => {
                if !self.delete_selection() {
                    let mut chars: Vec<char> = self.value.chars().collect();
                    if self.cursor_pos < chars.len() {
                        chars.remove(self.cursor_pos);
                        self.value = chars.into_iter().collect();
                    }
                }
                None
            }
            KeyCode::Left => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            KeyCode::Right => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                if self.cursor_pos < self.value.chars().count() {
                    self.cursor_pos += 1;
                }
                None
            }
            KeyCode::Home => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                self.cursor_pos = 0;
                None
            }
            KeyCode::End => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                self.cursor_pos = self.value.chars().count();
                None
            }
            KeyCode::Enter => Some(Some(self.value.clone())),
            KeyCode::Esc => Some(None),
            _ => None,
        }
    }

    /// Handles keyboard input (without explicit modifiers).
    /// Returns:
    /// - `None` while pending
    /// - `Some(Some(value))` when user submits with Enter
    /// - `Some(None)` when user cancels with Esc
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<String>> {
        self.handle_key_with_modifiers(code, KeyModifiers::NONE)
    }

    pub fn move_word_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = self.cursor_pos.min(chars.len());

        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }

        if i > 0 {
            let is_alnum = chars[i - 1].is_alphanumeric() || chars[i - 1] == '_';
            while i > 0 && !chars[i - 1].is_whitespace() {
                if (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') == is_alnum {
                    i -= 1;
                } else {
                    break;
                }
            }
        }

        self.cursor_pos = i;
    }

    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.value.chars().collect();
        let len = chars.len();

        if self.cursor_pos >= len {
            return;
        }

        let mut i = self.cursor_pos;
        let is_alnum = chars[i].is_alphanumeric() || chars[i] == '_';

        while i < len && !chars[i].is_whitespace() {
            if (chars[i].is_alphanumeric() || chars[i] == '_') == is_alnum {
                i += 1;
            } else {
                break;
            }
        }

        while i < len && chars[i].is_whitespace() {
            i += 1;
        }

        self.cursor_pos = i;
    }

    /// Computes dialog dimensions and wrapped prompt lines for the given screen area
    pub fn compute_layout(&self, area: Rect) -> (u16, u16, Vec<String>) {
        let max_w = area.width.saturating_sub(4);
        let preferred_w = 68.max(area.width * 75 / 100);
        let dialog_w = preferred_w.min(max_w).max(20);

        let inner_w = dialog_w.saturating_sub(2);
        let content_w = inner_w.saturating_sub(4) as usize;

        let prompt_lines = if self.prompt.trim().is_empty() {
            Vec::new()
        } else {
            wrap_prompt_text(&self.prompt, content_w.max(10))
        };

        let prompt_count = prompt_lines.len().max(1) as u16;
        let dialog_h = (prompt_count + 7).min(area.height.saturating_sub(2)).max(7);

        (dialog_w, dialog_h, prompt_lines)
    }

    /// Handles mouse clicks on OK or Cancel buttons, or clicking into the input field
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<String>> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            let (dialog_w, dialog_h, prompt_lines) = self.compute_layout(area);
            let inner_area = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return Some(None);
            }

            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);
            let center_x = inner_area.x + inner_area.width / 2;
            let ok_x = center_x.saturating_sub(12);
            let cancel_x = center_x + 2;

            if row == btn_y {
                if col >= ok_x && col < ok_x + 8 {
                    return Some(Some(self.value.clone()));
                } else if col >= cancel_x && col < cancel_x + 12 {
                    return Some(None);
                } else if col < center_x {
                    return Some(Some(self.value.clone()));
                } else {
                    return Some(None);
                }
            }

            // Click inside input box
            let input_y = inner_area.y + prompt_lines.len() as u16 + 1;
            let input_x = inner_area.x + 2;
            let input_w = inner_area.width.saturating_sub(4) as usize;
            if row == input_y && col >= input_x && col < input_x + input_w as u16 {
                let rel_col = (col - input_x) as usize;
                let char_count = self.value.chars().count();
                self.cursor_pos = (self.scroll_offset + rel_col).min(char_count);
            }
        }
        None
    }

    /// Renders the input modal dialog.
    /// Invariant: All text is strictly bounded within the modal frame. Text never exceeds the window width.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let (dialog_w, dialog_h, prompt_lines) = self.compute_layout(area);

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Prompt lines (wrapped at word boundaries)
        for (idx, line) in prompt_lines.iter().enumerate() {
            let py = inner.y + idx as u16;
            if py < inner.y + inner.height.saturating_sub(4) {
                buf.set_string(inner.x + 2, py, line, styles.text);
            }
        }

        // 2. Input Box Field
        let input_y = inner.y + prompt_lines.len() as u16 + 1;
        let input_x = inner.x + 2;
        let input_w = inner.width.saturating_sub(4) as usize;

        if input_w > 0 && input_y < inner.y + inner.height.saturating_sub(1) {
            let char_count = self.value.chars().count();
            let safe_cursor = self.cursor_pos.min(char_count);

            // Compute scroll offset to ensure cursor is visible within input_w
            let scroll = if safe_cursor < self.scroll_offset {
                safe_cursor
            } else if safe_cursor >= self.scroll_offset + input_w {
                safe_cursor.saturating_sub(input_w) + 1
            } else {
                self.scroll_offset
            };

            let vis_chars: Vec<char> = self.value.chars().skip(scroll).take(input_w).collect();
            let sel_range = self.selection_range();

            for i in 0..input_w {
                let char_idx = scroll + i;
                let ch = if i < vis_chars.len() {
                    if self.masked {
                        '*'
                    } else {
                        vis_chars[i]
                    }
                } else {
                    ' '
                };
                let is_selected = sel_range
                    .map(|(s, e)| char_idx >= s && char_idx < e)
                    .unwrap_or(false);
                let is_cur = char_idx == safe_cursor;

                let cell_style = if is_selected || is_cur {
                    styles.highlight.add_modifier(Modifier::REVERSED)
                } else {
                    styles.highlight
                };

                let cell = &mut buf[(input_x + i as u16, input_y)];
                cell.set_char(ch);
                cell.set_style(cell_style);
            }
        }

        // 3. Action Buttons [ OK ] and [ Cancel ]
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;
        let ok_x = center_x.saturating_sub(12);
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

/// Word wrapper for modal prompt strings. Respects explicit newlines and breaks long words.
fn wrap_prompt_text(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                if word.chars().count() <= max_w {
                    current.push_str(word);
                } else {
                    let chars: Vec<char> = word.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        let end = (i + max_w).min(chars.len());
                        let chunk: String = chars[i..end].iter().collect();
                        if end < chars.len() {
                            lines.push(chunk);
                        } else {
                            current.push_str(&chunk);
                        }
                        i = end;
                    }
                }
            } else if current.chars().count() + 1 + word.chars().count() <= max_w {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = String::new();
                if word.chars().count() <= max_w {
                    current.push_str(word);
                } else {
                    let chars: Vec<char> = word.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        let end = (i + max_w).min(chars.len());
                        let chunk: String = chars[i..end].iter().collect();
                        if end < chars.len() {
                            lines.push(chunk);
                        } else {
                            current.push_str(&chunk);
                        }
                        i = end;
                    }
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
