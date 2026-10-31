//! Interactive in-modal note editor with auto-formatting, cursor navigation, and direct save.

use super::frame::render_modal_frame;
use super::note_viewer::wrap_footer_tokens;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Action returned by the in-modal note editor
#[derive(Debug, Clone, PartialEq)]
pub enum NoteEditorAction {
    /// No change
    None,
    /// Note content was saved
    Save(String),
    /// Editing was cancelled without saving
    Cancel,
}

/// In-modal note editor
#[derive(Debug, Clone)]
pub struct NoteEditorModal {
    pub title: String,
    pub filepath: String,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub is_modified: bool,
}

impl NoteEditorModal {
    pub fn new(title: impl Into<String>, filepath: impl Into<String>, content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|s| s.to_string()).collect()
        };
        let initial_lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            title: title.into(),
            filepath: filepath.into(),
            lines: initial_lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            is_modified: false,
        }
    }

    /// Handles keyboard input
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> NoteEditorAction {
        // Ctrl+S: Save note
        if (code == KeyCode::Char('s') || code == KeyCode::Char('S'))
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            return NoteEditorAction::Save(self.lines.join("\n"));
        }

        // Ctrl+C: Copy to clipboard via OSC 52
        if (code == KeyCode::Char('c') || code == KeyCode::Char('C'))
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            let text = self.lines.join("\n");
            let b64 = base64_encode(text.as_bytes());
            print!("\x1b]52;c;{}\x07", b64);
            return NoteEditorAction::None;
        }

        match code {
            KeyCode::Esc => NoteEditorAction::Cancel,
            KeyCode::Enter => {
                self.handle_enter();
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Backspace => {
                self.handle_backspace();
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Delete => {
                self.handle_delete();
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                } else if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.cursor_col = self.lines[self.cursor_row].len();
                }
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::Right => {
                let cur_len = self.current_line().len();
                if self.cursor_col < cur_len {
                    self.cursor_col += 1;
                } else if self.cursor_row + 1 < self.lines.len() {
                    self.cursor_row += 1;
                    self.cursor_col = 0;
                }
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::Up => {
                if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.clamp_cursor_col();
                }
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::Down => {
                if self.cursor_row + 1 < self.lines.len() {
                    self.cursor_row += 1;
                    self.clamp_cursor_col();
                }
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                NoteEditorAction::None
            }
            KeyCode::End => {
                self.cursor_col = self.current_line().len();
                NoteEditorAction::None
            }
            KeyCode::PageUp => {
                self.cursor_row = self.cursor_row.saturating_sub(15);
                self.clamp_cursor_col();
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::PageDown => {
                self.cursor_row = (self.cursor_row + 15).min(self.lines.len().saturating_sub(1));
                self.clamp_cursor_col();
                self.ensure_cursor_visible(20);
                NoteEditorAction::None
            }
            KeyCode::Tab => {
                self.insert_str("    ");
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Char(c)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_char(c);
                self.is_modified = true;
                NoteEditorAction::None
            }
            _ => NoteEditorAction::None,
        }
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> NoteEditorAction {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.scroll_offset + 2 < self.lines.len() {
                    self.scroll_offset += 2;
                }
                NoteEditorAction::None
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                NoteEditorAction::None
            }
            _ => NoteEditorAction::None,
        }
    }

    fn current_line(&self) -> &str {
        if self.cursor_row < self.lines.len() {
            &self.lines[self.cursor_row]
        } else {
            ""
        }
    }

    fn clamp_cursor_col(&mut self) {
        let max_len = self.current_line().len();
        if self.cursor_col > max_len {
            self.cursor_col = max_len;
        }
    }

    fn ensure_cursor_visible(&mut self, view_h: usize) {
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + view_h {
            self.scroll_offset = self.cursor_row.saturating_sub(view_h.saturating_sub(1));
        }
    }

    fn insert_char(&mut self, c: char) {
        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
            self.cursor_row = self.lines.len() - 1;
        }
        let line = &mut self.lines[self.cursor_row];
        if self.cursor_col > line.len() {
            self.cursor_col = line.len();
        }
        line.insert(self.cursor_col, c);
        self.cursor_col += 1;
    }

    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    fn handle_backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            line.remove(self.cursor_col - 1);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let cur_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&cur_line);
            self.cursor_col = prev_len;
        }
    }

    fn handle_delete(&mut self) {
        if self.cursor_row < self.lines.len() {
            let cur_len = self.lines[self.cursor_row].len();
            if self.cursor_col < cur_len {
                self.lines[self.cursor_row].remove(self.cursor_col);
            } else if self.cursor_row + 1 < self.lines.len() {
                let next_line = self.lines.remove(self.cursor_row + 1);
                self.lines[self.cursor_row].push_str(&next_line);
            }
        }
    }

    fn handle_enter(&mut self) {
        let cur_line = self.current_line().to_string();
        let prefix = &cur_line[..self.cursor_col.min(cur_line.len())];
        let suffix = &cur_line[self.cursor_col.min(cur_line.len())..];

        // Checkbox continuation / de-escalation
        let mut continuation = String::new();
        let mut de_escalated = false;

        let trimmed_prefix = prefix.trim_start();
        let indent = &prefix[..prefix.len() - trimmed_prefix.len()];

        if trimmed_prefix == "- [ ]" || trimmed_prefix == "- [x]" || trimmed_prefix == "- [X]" {
            // De-escalate empty checkbox
            self.lines[self.cursor_row] = indent.to_string();
            self.cursor_col = indent.len();
            de_escalated = true;
        } else if trimmed_prefix.starts_with("- [ ] ")
            || trimmed_prefix.starts_with("- [x] ")
            || trimmed_prefix.starts_with("- [X] ")
        {
            continuation = format!("{}- [ ] ", indent);
        } else if trimmed_prefix == "- " || trimmed_prefix == "* " || trimmed_prefix == "+ " {
            // De-escalate empty bullet
            self.lines[self.cursor_row] = indent.to_string();
            self.cursor_col = indent.len();
            de_escalated = true;
        } else if trimmed_prefix.starts_with("- ")
            || trimmed_prefix.starts_with("* ")
            || trimmed_prefix.starts_with("+ ")
        {
            let bullet = &trimmed_prefix[..2];
            continuation = format!("{}{}", indent, bullet);
        }

        if !de_escalated {
            let left_part = prefix.to_string();
            let right_part = format!("{}{}", continuation, suffix);
            self.lines[self.cursor_row] = left_part;
            self.cursor_row += 1;
            self.lines.insert(self.cursor_row, right_part);
            self.cursor_col = continuation.len();
        }
    }

    /// Renders note editor modal
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (area.width.saturating_sub(4)).clamp(20, 84);
        let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);

        let modal_title = format!(" [ Edit Note: {} ] ", self.title);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &modal_title, styles, buf);

        let footer_tokens = ["[Ctrl+S] Save", "|", "[Ctrl+C] Copy", "|", "[ESC] Cancel"];
        let max_footer_w = inner.width.saturating_sub(4) as usize;
        let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
        let footer_h = footer_lines.len().max(1);

        let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
        self.ensure_cursor_visible(content_h);

        let inner_w = inner.width.saturating_sub(6) as usize;

        // Render editor lines
        for (i, line_idx) in
            (self.scroll_offset..self.lines.len().min(self.scroll_offset + content_h)).enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let line_num_str = format!("{:2} ", line_idx + 1);
            buf.set_string(
                inner.x + 2,
                y,
                &line_num_str,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            );

            let line = &self.lines[line_idx];
            let line_start_x = inner.x + 5;
            let display_chars: Vec<char> = line.chars().take(inner_w).collect();

            // Print line text
            for (col_idx, &c) in display_chars.iter().enumerate() {
                let is_cursor = line_idx == self.cursor_row && col_idx == self.cursor_col;
                let cell_style = if is_cursor {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else if line.starts_with("- [x]") || line.starts_with("- [X]") {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM)
                } else if line.contains("- [ ]") {
                    Style::default().fg(Color::Yellow)
                } else if line.starts_with('#') {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                };
                buf.set_string(line_start_x + col_idx as u16, y, c.to_string(), cell_style);
            }

            // If cursor is at or beyond end of line
            if line_idx == self.cursor_row
                && self.cursor_col >= display_chars.len()
                && self.cursor_col < inner_w
            {
                buf.set_string(
                    line_start_x + self.cursor_col as u16,
                    y,
                    " ",
                    styles.highlight.add_modifier(Modifier::BOLD),
                );
            }
        }

        // Render wrapped footer help lines
        let footer_start_y = inner.y + inner.height.saturating_sub(1 + footer_h as u16);
        for (idx, line) in footer_lines.iter().enumerate() {
            let footer_x = inner.x + (inner.width.saturating_sub(line.len() as u16) / 2);
            buf.set_string(
                footer_x,
                footer_start_y + idx as u16,
                line,
                Style::default().fg(Color::DarkGray),
            );
        }

        inner
    }
}

/// Simple Base64 encoder for OSC 52 clipboard strings
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
