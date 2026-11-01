//! Single-line interactive text input with Unicode cursor navigation, selection, and clipboard support.

use crossterm::event::{KeyCode, KeyModifiers};

/// Interactive Unicode text input state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    pub chars: Vec<char>,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
}

impl TextInput {
    pub fn new(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let cursor = chars.len();
        Self {
            chars,
            cursor,
            selection_anchor: None,
        }
    }

    pub fn as_string(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn set_text(&mut self, s: &str) {
        self.chars = s.chars().collect();
        self.cursor = self.chars.len();
        self.selection_anchor = None;
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.chars.len();
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

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
                true
            }
            KeyCode::End => {
                self.cursor = self.chars.len();
                true
            }
            _ => false,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        self.handle_key_with_modifiers(code, KeyModifiers::NONE)
    }
}
