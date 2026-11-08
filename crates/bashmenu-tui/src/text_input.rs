//! Single-line interactive text input with Unicode cursor navigation, selection, and clipboard support.

use crossterm::event::{KeyCode, KeyModifiers};

/// Interactive Unicode text input state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    /// Underlying vector of Unicode code points
    pub chars: Vec<char>,
    /// Insertion cursor position measured in Unicode character indices (0..=chars.len())
    pub cursor: usize,
    /// Selection anchor point when selecting text via Shift+Arrows or Ctrl+A
    pub selection_anchor: Option<usize>,
    /// Horizontal scroll offset for rendering within constrained viewport
    pub scroll_offset: usize,
}

impl TextInput {
    /// Initializes a text input pre-filled with the provided string, positioning cursor at the end.
    pub fn new(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let cursor = chars.len();
        Self {
            chars,
            cursor,
            selection_anchor: None,
            scroll_offset: 0,
        }
    }

    /// Converts the current Unicode characters into a standard `String`.
    pub fn as_string(&self) -> String {
        self.chars.iter().collect()
    }

    /// Overwrites the current text content, resetting cursor to the end and clearing selection.
    pub fn set_text(&mut self, s: &str) {
        self.chars = s.chars().collect();
        self.cursor = self.chars.len();
        self.selection_anchor = None;
        self.scroll_offset = 0;
    }

    /// Returns `true` if the input contains zero characters.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Returns the number of Unicode characters in the buffer.
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// Selects the entire text buffer from index 0 to `len()`.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.chars.len();
    }

    /// Clears any active selection range without modifying the text content.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Returns the active selection character range `(start, end)` if an anchor exists and is non-empty.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.and_then(|anchor| {
            let c = self.cursor.min(self.chars.len());
            let a = anchor.min(self.chars.len());
            if a != c {
                Some((a.min(c), a.max(c)))
            } else {
                None
            }
        })
    }

    /// Deletes the currently selected character slice, returning `true` if characters were removed.
    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            self.chars.drain(start..end);
            self.cursor = start;
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    /// Ensures that the cursor is visible within a visible viewport width of `w` characters.
    /// Updates and returns the horizontal scroll offset.
    pub fn ensure_cursor_visible(&mut self, w: usize) -> usize {
        if w == 0 {
            self.scroll_offset = 0;
            return 0;
        }
        let safe_cursor = self.cursor.min(self.chars.len());
        if safe_cursor < self.scroll_offset {
            self.scroll_offset = safe_cursor;
        } else if safe_cursor >= self.scroll_offset + w {
            self.scroll_offset = safe_cursor.saturating_sub(w) + 1;
        }
        self.scroll_offset
    }

    /// Computes the scroll offset needed to display the cursor within width `w` without mutating state.
    pub fn scroll_for_width(&self, w: usize) -> usize {
        if w == 0 {
            return 0;
        }
        let safe_cursor = self.cursor.min(self.chars.len());
        if safe_cursor < self.scroll_offset {
            safe_cursor
        } else if safe_cursor >= self.scroll_offset + w {
            safe_cursor.saturating_sub(w) + 1
        } else {
            self.scroll_offset
        }
    }

    /// Handles positioning the cursor from a mouse click at `clicked_col` character index.
    /// If `is_shift` is true, anchors the selection at the previous cursor position.
    pub fn handle_mouse_click(&mut self, clicked_col: usize, is_shift: bool) {
        let target = clicked_col.min(self.chars.len());
        if is_shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        self.cursor = target;
    }

    /// Processes keyboard events with active modifier flags (Shift, Ctrl, Alt).
    pub fn handle_key_with_modifiers(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let is_shift = modifiers.contains(KeyModifiers::SHIFT);
        let is_ctrl = modifiers.contains(KeyModifiers::CONTROL);

        if is_ctrl {
            match code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.select_all();
                    return true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String = self.chars[start..end].iter().collect();
                        crate::clipboard::set_clipboard_text(&snippet);
                    }
                    return true;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String = self.chars[start..end].iter().collect();
                        crate::clipboard::set_clipboard_text(&snippet);
                        self.delete_selection();
                        return true;
                    }
                    return false;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = crate::clipboard::get_clipboard_text() {
                        self.delete_selection();
                        let clean: Vec<char> =
                            text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                        for (idx, ch) in clean.into_iter().enumerate() {
                            self.chars.insert(self.cursor + idx, ch);
                        }
                        self.cursor += text.chars().filter(|c| *c != '\n' && *c != '\r').count();
                        self.selection_anchor = None;
                        return true;
                    }
                    return false;
                }
                KeyCode::Left => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some(self.cursor);
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
                        self.cursor -= 1;
                    }
                    while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
                        self.cursor -= 1;
                    }
                    return true;
                }
                KeyCode::Right => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some(self.cursor);
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    while self.cursor < self.chars.len() && !self.chars[self.cursor].is_whitespace()
                    {
                        self.cursor += 1;
                    }
                    while self.cursor < self.chars.len() && self.chars[self.cursor].is_whitespace()
                    {
                        self.cursor += 1;
                    }
                    return true;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor);
                    }
                } else {
                    self.selection_anchor = None;
                }
            }
            _ => {}
        }

        match code {
            KeyCode::Char(c) => {
                self.delete_selection();
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.delete_selection() {
                    true
                } else if self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Delete => {
                if self.delete_selection() {
                    true
                } else if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Right => {
                if self.cursor < self.chars.len() {
                    self.cursor += 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.scroll_offset = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.chars.len();
                true
            }
            _ => false,
        }
    }

    /// Processes a single key code with no modifier keys pressed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        self.handle_key_with_modifiers(code, KeyModifiers::NONE)
    }
}
