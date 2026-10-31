//! Interactive in-modal note editor with auto-formatting, cursor navigation, and direct save.

use super::frame::render_modal_frame;
use super::note_viewer::wrap_footer_tokens;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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

/// A single visual row produced by soft-wrapping a logical line
#[derive(Debug, Clone)]
pub struct VisualLine {
    pub logical_row: usize,
    pub start_col: usize,
    pub chars: Vec<char>,
    pub cursor_col_in_chunk: Option<usize>,
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
    pub last_inner_w: usize,
    pub last_inner_rect: Rect,
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
            last_inner_w: 76,
            last_inner_rect: Rect::default(),
        }
    }

    /// Computes soft-wrapped visual lines for the current logical lines
    pub fn compute_visual_lines(&self, width: usize) -> (Vec<VisualLine>, usize) {
        let mut visual_lines = Vec::new();
        let mut cursor_vis_idx = 0;
        let w = width.max(10);

        for (l_idx, line) in self.lines.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let is_active_row = l_idx == self.cursor_row;

            if chars.is_empty() {
                if is_active_row {
                    cursor_vis_idx = visual_lines.len();
                }
                visual_lines.push(VisualLine {
                    logical_row: l_idx,
                    start_col: 0,
                    chars: Vec::new(),
                    cursor_col_in_chunk: if is_active_row { Some(0) } else { None },
                });
            } else {
                let chunks = wrap_chars_by_word(&chars, w);
                let num_chunks = chunks.len();

                for (chunk_idx, (start, chunk)) in chunks.into_iter().enumerate() {
                    let end = start + chunk.len();
                    let is_last_chunk = chunk_idx + 1 == num_chunks;

                    let cursor_here = if is_active_row {
                        if self.cursor_col >= start
                            && (self.cursor_col < end || (is_last_chunk && self.cursor_col >= end))
                        {
                            cursor_vis_idx = visual_lines.len();
                            Some(self.cursor_col - start)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    visual_lines.push(VisualLine {
                        logical_row: l_idx,
                        start_col: start,
                        chars: chunk,
                        cursor_col_in_chunk: cursor_here,
                    });
                }
            }
        }

        (visual_lines, cursor_vis_idx)
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
                    self.cursor_col = self.lines[self.cursor_row].chars().count();
                }
                NoteEditorAction::None
            }
            KeyCode::Right => {
                let cur_len = self.current_line().chars().count();
                if self.cursor_col < cur_len {
                    self.cursor_col += 1;
                } else if self.cursor_row + 1 < self.lines.len() {
                    self.cursor_row += 1;
                    self.cursor_col = 0;
                }
                NoteEditorAction::None
            }
            KeyCode::Up => {
                let (vlines, cur_vidx) = self.compute_visual_lines(self.last_inner_w);
                if cur_vidx > 0 {
                    let cur_offset = vlines[cur_vidx].cursor_col_in_chunk.unwrap_or(0);
                    let target = &vlines[cur_vidx - 1];
                    self.cursor_row = target.logical_row;
                    let target_line_len = self.lines[target.logical_row].chars().count();
                    self.cursor_col = (target.start_col + cur_offset)
                        .min(target.start_col + target.chars.len())
                        .min(target_line_len);
                }
                NoteEditorAction::None
            }
            KeyCode::Down => {
                let (vlines, cur_vidx) = self.compute_visual_lines(self.last_inner_w);
                if cur_vidx + 1 < vlines.len() {
                    let cur_offset = vlines[cur_vidx].cursor_col_in_chunk.unwrap_or(0);
                    let target = &vlines[cur_vidx + 1];
                    self.cursor_row = target.logical_row;
                    let target_line_len = self.lines[target.logical_row].chars().count();
                    self.cursor_col = (target.start_col + cur_offset)
                        .min(target.start_col + target.chars.len())
                        .min(target_line_len);
                }
                NoteEditorAction::None
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                NoteEditorAction::None
            }
            KeyCode::End => {
                self.cursor_col = self.lines[self.cursor_row].chars().count();
                NoteEditorAction::None
            }
            KeyCode::PageUp => {
                let (vlines, cur_vidx) = self.compute_visual_lines(self.last_inner_w);
                let target_vidx = cur_vidx.saturating_sub(10);
                let cur_offset = vlines[cur_vidx].cursor_col_in_chunk.unwrap_or(0);
                let target = &vlines[target_vidx];
                self.cursor_row = target.logical_row;
                let target_line_len = self.lines[target.logical_row].chars().count();
                self.cursor_col = (target.start_col + cur_offset)
                    .min(target.start_col + target.chars.len())
                    .min(target_line_len);
                NoteEditorAction::None
            }
            KeyCode::PageDown => {
                let (vlines, cur_vidx) = self.compute_visual_lines(self.last_inner_w);
                let target_vidx = (cur_vidx + 10).min(vlines.len().saturating_sub(1));
                let cur_offset = vlines[cur_vidx].cursor_col_in_chunk.unwrap_or(0);
                let target = &vlines[target_vidx];
                self.cursor_row = target.logical_row;
                let target_line_len = self.lines[target.logical_row].chars().count();
                self.cursor_col = (target.start_col + cur_offset)
                    .min(target.start_col + target.chars.len())
                    .min(target_line_len);
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
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> NoteEditorAction {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                let (vlines, _) = self.compute_visual_lines(self.last_inner_w);
                if self.scroll_offset + 2 < vlines.len() {
                    self.scroll_offset += 2;
                }
                NoteEditorAction::None
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                NoteEditorAction::None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let inner = if self.last_inner_rect.width > 0 {
                    self.last_inner_rect
                } else {
                    let dialog_w = (area.width.saturating_sub(4)).clamp(20, 84);
                    let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);
                    let w = dialog_w.min(area.width.saturating_sub(4));
                    let h = dialog_h.min(area.height.saturating_sub(2)).min(28);
                    let start_x = area.x + (area.width.saturating_sub(w + 2)) / 2;
                    let start_y = area.y + (area.height.saturating_sub(h + 1)) / 2;
                    Rect::new(
                        start_x + 1,
                        start_y + 1,
                        w.saturating_sub(2),
                        h.saturating_sub(2),
                    )
                };
                let x = inner.x.saturating_sub(1);
                let y = inner.y.saturating_sub(1);
                let w = inner.width + 2;

                // Check if close [X] clicked (top right: x + w - 3)
                if mouse.row == y
                    && mouse.column >= x + w.saturating_sub(3)
                    && mouse.column <= x + w.saturating_sub(2)
                {
                    return NoteEditorAction::Cancel;
                }

                let content_start_y = inner.y + 1;
                let (vlines, _) = self.compute_visual_lines(self.last_inner_w);

                if mouse.row >= content_start_y && mouse.column >= inner.x + 2 {
                    let vidx_rel = (mouse.row - content_start_y) as usize;
                    let target_vidx = self.scroll_offset + vidx_rel;
                    if target_vidx < vlines.len() {
                        let target = &vlines[target_vidx];
                        self.cursor_row = target.logical_row;
                        let col_click = (mouse.column.saturating_sub(inner.x + 2)) as usize;
                        let target_line_len = self.lines[target.logical_row].chars().count();
                        self.cursor_col = (target.start_col + col_click.min(target.chars.len()))
                            .min(target_line_len);
                    }
                }
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

    fn insert_char(&mut self, c: char) {
        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
            self.cursor_row = self.lines.len() - 1;
        }
        let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let col = self.cursor_col.min(chars.len());
        chars.insert(col, c);
        self.lines[self.cursor_row] = chars.into_iter().collect();
        self.cursor_col = col + 1;
    }

    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    fn handle_backspace(&mut self) {
        if self.cursor_col > 0 {
            let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
            if self.cursor_col <= chars.len() {
                chars.remove(self.cursor_col - 1);
                self.lines[self.cursor_row] = chars.into_iter().collect();
                self.cursor_col -= 1;
            }
        } else if self.cursor_row > 0 {
            let cur_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&cur_line);
            self.cursor_col = prev_len;
        }
    }

    fn handle_delete(&mut self) {
        if self.cursor_row < self.lines.len() {
            let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
            if self.cursor_col < chars.len() {
                chars.remove(self.cursor_col);
                self.lines[self.cursor_row] = chars.into_iter().collect();
            } else if self.cursor_row + 1 < self.lines.len() {
                let next_line = self.lines.remove(self.cursor_row + 1);
                self.lines[self.cursor_row].push_str(&next_line);
            }
        }
    }

    fn handle_enter(&mut self) {
        let chars: Vec<char> = self.current_line().chars().collect();
        let col = self.cursor_col.min(chars.len());
        let prefix: String = chars[..col].iter().collect();
        let suffix: String = chars[col..].iter().collect();

        // Checkbox continuation / de-escalation
        let mut continuation = String::new();
        let mut de_escalated = false;

        let trimmed_prefix = prefix.trim_start();
        let indent = &prefix[..prefix.len() - trimmed_prefix.len()];

        if trimmed_prefix.starts_with("- [ ] ")
            || trimmed_prefix.starts_with("- [x] ")
            || trimmed_prefix.starts_with("- [X] ")
        {
            let item_text = &trimmed_prefix[6..];
            if item_text.trim().is_empty() && suffix.trim().is_empty() {
                // Empty checkbox item: de-escalate to plain indent
                self.lines[self.cursor_row] = indent.to_string();
                self.cursor_col = indent.chars().count();
                de_escalated = true;
            } else {
                continuation = format!("{}- [ ] ", indent);
            }
        } else if trimmed_prefix.starts_with("- ")
            || trimmed_prefix.starts_with("* ")
            || trimmed_prefix.starts_with("+ ")
        {
            let item_text = &trimmed_prefix[2..];
            if item_text.trim().is_empty() && suffix.trim().is_empty() {
                // Empty bullet: de-escalate
                self.lines[self.cursor_row] = indent.to_string();
                self.cursor_col = indent.chars().count();
                de_escalated = true;
            } else {
                let bullet = &trimmed_prefix[..2];
                continuation = format!("{}{}", indent, bullet);
            }
        } else if let Some(cap) = parse_ordered_list(trimmed_prefix) {
            let (num_str, rest) = cap;
            if rest.trim().is_empty() && suffix.trim().is_empty() {
                // Empty numbered item: de-escalate
                self.lines[self.cursor_row] = indent.to_string();
                self.cursor_col = indent.chars().count();
                de_escalated = true;
            } else if let Ok(num) = num_str.parse::<usize>() {
                continuation = format!("{}{}. ", indent, num + 1);
            }
        } else if let Some(quote_text) = trimmed_prefix.strip_prefix("> ") {
            if quote_text.trim().is_empty() && suffix.trim().is_empty() {
                self.lines[self.cursor_row] = indent.to_string();
                self.cursor_col = indent.chars().count();
                de_escalated = true;
            } else {
                continuation = format!("{}> ", indent);
            }
        }

        if !de_escalated {
            let left_part = prefix;
            let right_part = format!("{}{}", continuation, suffix);
            self.lines[self.cursor_row] = left_part;
            self.cursor_row += 1;
            self.lines.insert(self.cursor_row, right_part);
            self.cursor_col = continuation.chars().count();
        }
    }

    /// Renders note editor modal with soft-wrapping and no line numbers
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (area.width.saturating_sub(4)).clamp(20, 84);
        let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);

        let modal_title = format!(" [ Edit Note: {} ] ", self.title);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &modal_title, styles, buf);
        self.last_inner_rect = inner;

        let footer_tokens = ["[Ctrl+S] Save", "|", "[Ctrl+C] Copy", "|", "[ESC] Cancel"];
        let max_footer_w = inner.width.saturating_sub(4) as usize;
        let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
        let footer_h = footer_lines.len().max(1);

        let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
        let inner_w = inner.width.saturating_sub(4) as usize;
        self.last_inner_w = inner_w;

        let (visual_lines, cursor_vis_idx) = self.compute_visual_lines(inner_w);

        // Ensure cursor is visible in scroll window
        if cursor_vis_idx < self.scroll_offset {
            self.scroll_offset = cursor_vis_idx;
        } else if cursor_vis_idx >= self.scroll_offset + content_h {
            self.scroll_offset = cursor_vis_idx + 1 - content_h;
        }

        // Render editor lines without line numbers
        for (i, vline) in visual_lines
            .iter()
            .skip(self.scroll_offset)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let line_start_x = inner.x + 2;
            let raw_line = &self.lines[vline.logical_row];

            let line_style = if raw_line.starts_with("- [x]") || raw_line.starts_with("- [X]") {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else if raw_line.contains("- [ ]") {
                Style::default().fg(Color::Yellow)
            } else if raw_line.starts_with('#') {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            for (col_idx, &c) in vline.chars.iter().enumerate() {
                let is_cursor = vline.cursor_col_in_chunk == Some(col_idx);
                let cell_style = if is_cursor {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    line_style
                };
                buf.set_string(line_start_x + col_idx as u16, y, c.to_string(), cell_style);
            }

            // If cursor is at or beyond end of line chunk
            if let Some(c_col) = vline.cursor_col_in_chunk {
                if c_col >= vline.chars.len() && c_col < inner_w {
                    buf.set_string(
                        line_start_x + c_col as u16,
                        y,
                        " ",
                        styles.highlight.add_modifier(Modifier::BOLD),
                    );
                }
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

fn parse_ordered_list(text: &str) -> Option<(&str, &str)> {
    let dot_idx = text.find(". ")?;
    let num_part = &text[..dot_idx];
    if !num_part.is_empty() && num_part.chars().all(|c| c.is_ascii_digit()) {
        Some((num_part, &text[dot_idx + 2..]))
    } else {
        None
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(B64_CHARS[(b0 >> 2) as usize] as char);
        out.push(B64_CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn wrap_chars_by_word(chars: &[char], w: usize) -> Vec<(usize, Vec<char>)> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        if chars.len() - start <= w {
            result.push((start, chars[start..].to_vec()));
            break;
        }
        let candidate_end = start + w;
        let mut split_at = None;
        for i in (start + 1..=candidate_end).rev() {
            if chars[i - 1].is_whitespace() {
                split_at = Some(i);
                break;
            }
        }
        let end = match split_at {
            Some(idx) => idx,
            None => candidate_end,
        };
        result.push((start, chars[start..end].to_vec()));
        start = end;
    }
    result
}
