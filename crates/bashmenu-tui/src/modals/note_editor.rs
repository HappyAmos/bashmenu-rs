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
    pub indent_margin: usize,
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
    pub selection_anchor: Option<(usize, usize)>,
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
            selection_anchor: None,
        }
    }

    /// Computes ordered `((start_y, start_x), (end_y, end_x))` bounds of active selection
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.selection_anchor.and_then(|anchor| {
            let cur = (self.cursor_row, self.cursor_col);
            if anchor != cur {
                if anchor < cur {
                    Some((anchor, cur))
                } else {
                    Some((cur, anchor))
                }
            } else {
                None
            }
        })
    }

    /// Checks if a logical row and column is within active selection
    pub fn is_pos_selected(&self, row: usize, col: usize) -> bool {
        if let Some(((sy, sx), (ey, ex))) = self.selection_range() {
            if row < sy || row > ey {
                false
            } else if sy == ey {
                row == sy && col >= sx && col < ex
            } else if row == sy {
                col >= sx
            } else if row == ey {
                col < ex
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Extracts the text currently selected
    pub fn get_selected_text(&self) -> Option<String> {
        let ((sy, sx), (ey, ex)) = self.selection_range()?;
        if sy == ey {
            let line = self.lines.get(sy)?;
            let chars: Vec<char> = line.chars().collect();
            let s_idx = sx.min(chars.len());
            let e_idx = ex.min(chars.len());
            Some(chars[s_idx..e_idx].iter().collect())
        } else {
            let mut cut = Vec::new();
            if let Some(first) = self.lines.get(sy) {
                cut.push(first.chars().skip(sx).collect());
            }
            for r in (sy + 1)..ey {
                if let Some(mid) = self.lines.get(r) {
                    cut.push(mid.clone());
                }
            }
            if let Some(last) = self.lines.get(ey) {
                cut.push(last.chars().take(ex).collect());
            }
            Some(cut.join("\n"))
        }
    }

    /// Deletes the currently selected text
    pub fn delete_selection(&mut self) -> bool {
        let Some(((sy, sx), (ey, ex))) = self.selection_range() else {
            return false;
        };
        if sy == ey {
            if sy < self.lines.len() {
                let mut chars: Vec<char> = self.lines[sy].chars().collect();
                let s_idx = sx.min(chars.len());
                let e_idx = ex.min(chars.len());
                chars.drain(s_idx..e_idx);
                self.lines[sy] = chars.into_iter().collect();
            }
        } else if sy < self.lines.len() && ey < self.lines.len() {
            let start_prefix: String = self.lines[sy].chars().take(sx).collect();
            let end_suffix: String = self.lines[ey].chars().skip(ex).collect();
            self.lines[sy] = format!("{}{}", start_prefix, end_suffix);
            self.lines.drain((sy + 1)..=ey);
        }
        self.cursor_row = sy;
        self.cursor_col = sx;
        self.selection_anchor = None;
        self.is_modified = true;
        true
    }

    /// Pastes text at cursor, replacing selection if active
    pub fn paste_text(&mut self, text: &str) {
        self.delete_selection();
        let paste_lines: Vec<&str> = text.split('\n').collect();
        if paste_lines.is_empty() {
            return;
        }
        if paste_lines.len() == 1 {
            let s = paste_lines[0].trim_end_matches('\r');
            for c in s.chars() {
                self.insert_char(c);
            }
        } else {
            if self.cursor_row >= self.lines.len() {
                self.lines.push(String::new());
            }
            let current = self.current_line().to_string();
            let chars: Vec<char> = current.chars().collect();
            let col = self.cursor_col.min(chars.len());
            let prefix: String = chars[..col].iter().collect();
            let suffix: String = chars[col..].iter().collect();

            let first_line = paste_lines[0].trim_end_matches('\r');
            self.lines[self.cursor_row] = format!("{}{}", prefix, first_line);

            let mut insert_row = self.cursor_row + 1;
            for line in &paste_lines[1..paste_lines.len() - 1] {
                self.lines
                    .insert(insert_row, line.trim_end_matches('\r').to_string());
                insert_row += 1;
            }

            let last_line = paste_lines.last().unwrap().trim_end_matches('\r');
            self.lines
                .insert(insert_row, format!("{}{}", last_line, suffix));
            self.cursor_row = insert_row;
            self.cursor_col = last_line.chars().count();
        }
        self.selection_anchor = None;
        self.is_modified = true;
    }

    /// Sets initial cursor position (row, col)
    pub fn with_cursor(mut self, row: usize, col: usize) -> Self {
        self.cursor_row = row.min(self.lines.len().saturating_sub(1));
        let line_len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = col.min(line_len);
        self
    }

    /// Move cursor left by one word
    pub fn move_word_left(&mut self) {
        if self.cursor_col == 0 {
            if self.cursor_row > 0 {
                self.cursor_row -= 1;
                self.cursor_col = self.lines[self.cursor_row].chars().count();
            }
            return;
        }

        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let mut i = self.cursor_col.min(chars.len());

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

        self.cursor_col = i;
    }

    /// Move cursor right by one word
    pub fn move_word_right(&mut self) {
        if self.cursor_row >= self.lines.len() {
            return;
        }

        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let len = chars.len();

        if self.cursor_col >= len {
            if self.cursor_row + 1 < self.lines.len() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
            return;
        }

        let mut i = self.cursor_col;
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

        self.cursor_col = i;
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
                    indent_margin: 0,
                });
            } else {
                let margin = detect_list_margin(line);
                let chunks = if let Some(m) = margin {
                    wrap_chars_by_word_with_margin(&chars, w, w.saturating_sub(m).max(10))
                } else {
                    wrap_chars_by_word(&chars, w)
                };
                let num_chunks = chunks.len();

                for (chunk_idx, (start, chunk)) in chunks.into_iter().enumerate() {
                    let end = start + chunk.len();
                    let is_last_chunk = chunk_idx + 1 == num_chunks;
                    let chunk_margin = if chunk_idx > 0 {
                        margin.unwrap_or(0)
                    } else {
                        0
                    };

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
                        indent_margin: chunk_margin,
                    });
                }
            }
        }

        (visual_lines, cursor_vis_idx)
    }

    /// Handles keyboard input
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> NoteEditorAction {
        let is_shift = modifiers.contains(KeyModifiers::SHIFT);
        let is_ctrl = modifiers.contains(KeyModifiers::CONTROL);

        // Ctrl shortcuts
        if is_ctrl {
            match code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    return NoteEditorAction::Save(self.lines.join("\n"));
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.selection_anchor = Some((0, 0));
                    self.cursor_row = self.lines.len().saturating_sub(1);
                    self.cursor_col = self.lines[self.cursor_row].chars().count();
                    return NoteEditorAction::None;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some(text) = self.get_selected_text() {
                        crate::clipboard::set_clipboard_text(&text);
                    } else if self.cursor_row < self.lines.len() {
                        crate::clipboard::set_clipboard_text(&self.lines[self.cursor_row]);
                    }
                    return NoteEditorAction::None;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some(text) = self.get_selected_text() {
                        crate::clipboard::set_clipboard_text(&text);
                        self.delete_selection();
                    } else if self.cursor_row < self.lines.len() {
                        crate::clipboard::set_clipboard_text(&self.lines[self.cursor_row]);
                        if self.lines.len() > 1 {
                            self.lines.remove(self.cursor_row);
                            if self.cursor_row >= self.lines.len() {
                                self.cursor_row = self.lines.len() - 1;
                            }
                            self.cursor_col = 0;
                        } else {
                            self.lines[0].clear();
                            self.cursor_col = 0;
                        }
                        self.is_modified = true;
                    }
                    return NoteEditorAction::None;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = crate::clipboard::get_clipboard_text() {
                        self.paste_text(&text);
                    }
                    return NoteEditorAction::None;
                }
                KeyCode::Left => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.move_word_left();
                    return NoteEditorAction::None;
                }
                KeyCode::Right => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.move_word_right();
                    return NoteEditorAction::None;
                }
                KeyCode::Home => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    return NoteEditorAction::None;
                }
                KeyCode::End => {
                    if is_shift {
                        if self.selection_anchor.is_none() {
                            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
                        }
                    } else {
                        self.selection_anchor = None;
                    }
                    self.cursor_row = self.lines.len().saturating_sub(1);
                    self.cursor_col = self.lines[self.cursor_row].chars().count();
                    return NoteEditorAction::None;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some((self.cursor_row, self.cursor_col));
                    }
                } else {
                    self.selection_anchor = None;
                }
            }
            _ => {}
        }

        match code {
            KeyCode::Esc => NoteEditorAction::Cancel,
            KeyCode::Enter => {
                self.delete_selection();
                self.handle_enter();
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Backspace => {
                if !self.delete_selection() {
                    self.handle_backspace();
                }
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Delete => {
                if !self.delete_selection() {
                    self.handle_delete();
                }
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Left if modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_word_left();
                NoteEditorAction::None
            }
            KeyCode::Right if modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_word_right();
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
                self.delete_selection();
                self.insert_str("    ");
                self.is_modified = true;
                NoteEditorAction::None
            }
            KeyCode::Char(c)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.delete_selection();
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
                // Check if close [X] clicked
                if super::frame::is_close_button_clicked(inner, mouse.row, mouse.column) {
                    return NoteEditorAction::Cancel;
                }

                let sb_x = inner.x + inner.width.saturating_sub(1);
                if mouse.column == sb_x {
                    let (vlines, _) = self.compute_visual_lines(self.last_inner_w);
                    let footer_tokens = [
                        "[Ctrl+S] Save",
                        "|",
                        "[Ctrl+C] Copy",
                        "|",
                        "[Ctrl+V] Paste",
                        "|",
                        "[ESC] Cancel",
                    ];
                    let max_footer_w = inner.width.saturating_sub(4) as usize;
                    let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
                    let footer_h = footer_lines.len().max(1);
                    let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
                    let content_start_y = inner.y + 1;
                    let content_end_y = content_start_y + content_h as u16;

                    if mouse.row >= content_start_y && mouse.row < content_end_y {
                        let max_scroll = vlines.len().saturating_sub(content_h);
                        if mouse.row == content_start_y {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        } else if mouse.row == content_end_y - 1 {
                            self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
                        } else if content_h >= 3 && max_scroll > 0 {
                            let track_len = content_h - 2;
                            let rel_y = (mouse.row - (content_start_y + 1)) as usize;
                            self.scroll_offset = (rel_y * max_scroll) / track_len;
                        }
                        return NoteEditorAction::None;
                    }
                }

                let content_start_y = inner.y + 1;
                let (vlines, _) = self.compute_visual_lines(self.last_inner_w);

                if mouse.row >= content_start_y
                    && mouse.column >= inner.x + 2
                    && mouse.column < sb_x
                {
                    let vidx_rel = (mouse.row - content_start_y) as usize;
                    let target_vidx = self.scroll_offset + vidx_rel;
                    if target_vidx < vlines.len() {
                        let target = &vlines[target_vidx];
                        self.cursor_row = target.logical_row;
                        let line_x = inner.x + 2 + target.indent_margin as u16;
                        let col_click = (mouse.column.saturating_sub(line_x)) as usize;
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

        let modal_title = format!("Edit Note: {}", self.title);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &modal_title, styles, buf);
        self.last_inner_rect = inner;

        let footer_tokens = [
            "[Ctrl+S] Save",
            "|",
            "[Ctrl+C] Copy",
            "|",
            "[Ctrl+V] Paste",
            "|",
            "[ESC] Cancel",
        ];
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
            let line_start_x = inner.x + 2 + vline.indent_margin as u16;
            let raw_line = &self.lines[vline.logical_row];

            let mut line_style = if raw_line.starts_with("- [x]") || raw_line.starts_with("- [X]") {
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
            if line_style.bg.is_none() || line_style.bg == Some(Color::Reset) {
                line_style = line_style.bg(styles.background);
            }

            if vline.indent_margin > 0 {
                buf.set_string(inner.x + 2, y, " ".repeat(vline.indent_margin), line_style);
            }

            for (col_idx, &c) in vline.chars.iter().enumerate() {
                let logical_col = vline.start_col + col_idx;
                let is_selected = self.is_pos_selected(vline.logical_row, logical_col);
                let is_cursor = vline.cursor_col_in_chunk == Some(col_idx);
                let cell_style = if is_selected {
                    line_style.add_modifier(Modifier::REVERSED)
                } else if is_cursor {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    line_style
                };
                buf.set_string(line_start_x + col_idx as u16, y, c.to_string(), cell_style);
            }

            // If cursor is at or beyond end of line chunk
            if let Some(c_col) = vline.cursor_col_in_chunk {
                if c_col >= vline.chars.len() && vline.indent_margin + c_col < inner_w {
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

        // Render scrollbar if note has more lines than window height (content_h)
        if visual_lines.len() > content_h && content_h > 0 {
            let sb_x = inner.x + inner.width.saturating_sub(1);
            let content_start_y = inner.y + 1;
            let sb_fg = styles
                .scrollbar
                .fg
                .unwrap_or(styles.accent.fg.unwrap_or(Color::Cyan));
            let sb_bg = styles.background;
            let arrow_style = Style::default()
                .fg(sb_fg)
                .bg(sb_bg)
                .add_modifier(Modifier::BOLD);
            let thumb_style = Style::default().fg(sb_fg).bg(sb_bg);
            let track_style = Style::default().fg(Color::DarkGray).bg(sb_bg);

            let max_scroll = visual_lines.len().saturating_sub(content_h);
            let eff_scroll = self.scroll_offset.min(max_scroll);

            if content_h >= 3 {
                // Top arrow
                buf.set_string(sb_x, content_start_y, "▲", arrow_style);
                // Bottom arrow
                buf.set_string(
                    sb_x,
                    content_start_y + content_h as u16 - 1,
                    "▼",
                    arrow_style,
                );

                let track_len = content_h - 2;
                let total_lines = visual_lines.len();
                let thumb_size = ((track_len as f64 * content_h as f64) / total_lines as f64)
                    .round()
                    .clamp(1.0, track_len as f64) as usize;
                let available_track = track_len.saturating_sub(thumb_size);
                let scroll_ratio = if max_scroll > 0 {
                    eff_scroll as f64 / max_scroll as f64
                } else {
                    0.0
                };
                let thumb_start = (scroll_ratio * available_track as f64).round() as usize;

                for t_idx in 0..track_len {
                    let y = content_start_y + 1 + t_idx as u16;
                    if t_idx >= thumb_start && t_idx < thumb_start + thumb_size {
                        buf.set_string(sb_x, y, "█", thumb_style);
                    } else {
                        buf.set_string(sb_x, y, "░", track_style);
                    }
                }
            } else if content_h == 2 {
                buf.set_string(sb_x, content_start_y, "▲", arrow_style);
                buf.set_string(sb_x, content_start_y + 1, "▼", arrow_style);
            } else if content_h == 1 {
                buf.set_string(sb_x, content_start_y, "█", thumb_style);
            }
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

fn detect_list_margin(line: &str) -> Option<usize> {
    let indent_len = line.chars().take_while(|c| c.is_whitespace()).count();
    let trimmed = line.trim_start();
    if trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
        || trimmed.starts_with("- [X] ")
        || trimmed.starts_with("* [ ] ")
        || trimmed.starts_with("* [x] ")
        || trimmed.starts_with("* [X] ")
    {
        Some(indent_len + 6)
    } else if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ "))
        && !trimmed.starts_with("- [")
        && !trimmed.starts_with("* [")
    {
        Some(indent_len + 2)
    } else {
        None
    }
}

fn wrap_chars_by_word_with_margin(
    chars: &[char],
    first_w: usize,
    cont_w: usize,
) -> Vec<(usize, Vec<char>)> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut is_first = true;
    while start < chars.len() {
        let w = if is_first { first_w } else { cont_w };
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
        is_first = false;
    }
    result
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
