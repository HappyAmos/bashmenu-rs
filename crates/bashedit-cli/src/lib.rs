//! Core text buffer management, swap file persistence, and keyboard commands for BashEdit.

pub mod swap;

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
pub use swap::*;

/// Snapshot of editor state for undo/redo history.
#[derive(Clone, Debug)]
pub struct UndoSnapshot {
    /// Document text lines at snapshot time
    pub lines: Vec<String>,
    /// Cursor row index at snapshot time
    pub cursor_row: usize,
    /// Cursor column index at snapshot time
    pub cursor_col: usize,
    /// Whether visual selection mark was active
    pub mark_active: bool,
    /// Selection anchor row index
    pub mark_row: usize,
    /// Selection anchor column index
    pub mark_col: usize,
}

/// Text editing document buffer with cursor tracking, dirty state, and swap persistence.
pub struct EditorBuffer {
    /// Destination file system path if file has been saved or opened
    pub file_path: Option<PathBuf>,
    /// Colocated swap file path (`.<filename>.swp`)
    pub swap_path: PathBuf,
    /// Logical text lines of document
    pub lines: Vec<String>,
    /// Active cursor row index (0..lines.len())
    pub cursor_row: usize,
    /// Active cursor Unicode column position
    pub cursor_col: usize,
    /// Vertical viewport scroll offset in lines
    pub scroll_row: usize,
    /// Horizontal viewport scroll offset in columns
    pub scroll_col: usize,
    /// Flag indicating whether buffer has unsaved edits
    pub is_modified: bool,
    /// Read-only status (e.g. when concurrent swap lock collision occurs)
    pub is_read_only: bool,
    /// File modification time when loaded
    pub base_mtime: f64,
    /// SHA256 checksum of file when loaded
    pub base_sha256: String,
    /// Count of keystrokes since last swap file flush
    pub keystroke_counter: usize,
    /// Status bar notification string
    pub status_message: String,
    /// Stack of prior states for undo operations
    pub undo_stack: Vec<UndoSnapshot>,
    /// Stack of undone states for redo operations
    pub redo_stack: Vec<UndoSnapshot>,
    /// Whether visual mark / text selection mode is active
    pub mark_active: bool,
    /// Selection mark anchor row index
    pub mark_row: usize,
    /// Selection mark anchor column index
    pub mark_col: usize,
}

impl EditorBuffer {
    /// Creates a new empty buffer (untitled, unsaved).
    pub fn new_empty() -> Self {
        Self {
            file_path: None,
            swap_path: PathBuf::new(),
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            is_modified: false,
            is_read_only: false,
            base_mtime: 0.0,
            base_sha256: String::new(),
            keystroke_counter: 0,
            status_message: "New Buffer".to_string(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mark_active: false,
            mark_row: 0,
            mark_col: 0,
        }
    }

    /// Associates the buffer with a destination file path.
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.swap_path = get_swap_path(&path);
        self.file_path = Some(path);
        self.flush_swap();
    }

    /// Opens an existing document from disk or creates a new file at path.
    pub fn open(file_path: impl AsRef<Path>) -> Result<Self> {
        let raw = file_path.as_ref();
        let path = if let Some(stripped) = raw.to_str().and_then(|s| s.strip_prefix("~/")) {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(stripped)
            } else {
                raw.to_path_buf()
            }
        } else {
            raw.to_path_buf()
        };
        let swap_path = get_swap_path(&path);

        let (lines, base_mtime, base_sha256) = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let parsed_lines: Vec<String> = if content.is_empty() {
                vec![String::new()]
            } else {
                content.lines().map(|s| s.to_string()).collect()
            };
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let sha = compute_file_sha256(&path);
            (parsed_lines, mtime, sha)
        } else {
            (vec![String::new()], 0.0, String::new())
        };

        let mut buf = Self {
            file_path: Some(path),
            swap_path,
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            is_modified: false,
            is_read_only: false,
            base_mtime,
            base_sha256,
            keystroke_counter: 0,
            status_message: "Ready".to_string(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mark_active: false,
            mark_row: 0,
            mark_col: 0,
        };

        // Write initial clean swap file if not read-only
        buf.flush_swap();
        Ok(buf)
    }

    /// Flushes current buffer state to the colocated `.swp` file
    pub fn flush_swap(&mut self) -> bool {
        if self.is_read_only {
            return false;
        }
        if let Some(fp) = &self.file_path {
            if !self.swap_path.as_os_str().is_empty() {
                return write_swap_file(
                    &self.swap_path,
                    fp,
                    &self.lines,
                    self.base_mtime,
                    &self.base_sha256,
                );
            }
        }
        true
    }

    /// Removes the swap file upon clean buffer exit or discard
    pub fn cleanup_swap(&self) {
        if !self.is_read_only && !self.swap_path.as_os_str().is_empty() {
            remove_swap_file(&self.swap_path);
        }
    }

    /// Persists buffer contents to the target file on disk
    pub fn save_to_disk(&mut self) -> Result<()> {
        if self.is_read_only {
            anyhow::bail!("Cannot save: File is open in Read-Only mode");
        }
        let Some(path) = &self.file_path else {
            anyhow::bail!("Cannot save: No file path set");
        };
        let payload = self.lines.join("\n");
        fs::write(path, payload)?;

        self.base_mtime = fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.base_sha256 = compute_file_sha256(path);
        self.is_modified = false;
        self.status_message = format!("Saved {} lines to disk", self.lines.len());
        self.cleanup_swap();
        Ok(())
    }

    /// Notifies keystroke and periodically auto-flushes swap file (every 20 keystrokes)
    fn record_edit(&mut self) {
        self.is_modified = true;
        self.keystroke_counter += 1;
        if self.keystroke_counter >= 20 {
            self.flush_swap();
            self.keystroke_counter = 0;
        }
    }

    /// Records an undo snapshot of lines, cursor, and mark state
    pub fn push_undo(&mut self) {
        self.undo_stack.push(UndoSnapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
            mark_active: self.mark_active,
            mark_row: self.mark_row,
            mark_col: self.mark_col,
        });
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Reverts buffer state to preceding undo snapshot
    pub fn undo(&mut self) -> bool {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(UndoSnapshot {
                lines: self.lines.clone(),
                cursor_row: self.cursor_row,
                cursor_col: self.cursor_col,
                mark_active: self.mark_active,
                mark_row: self.mark_row,
                mark_col: self.mark_col,
            });
            self.lines = snap.lines;
            self.cursor_row = snap.cursor_row;
            self.cursor_col = snap.cursor_col;
            self.mark_active = snap.mark_active;
            self.mark_row = snap.mark_row;
            self.mark_col = snap.mark_col;
            self.clamp_cursor();
            self.record_edit();
            self.status_message = "Undo".to_string();
            true
        } else {
            self.status_message = "Nothing to undo".to_string();
            false
        }
    }

    /// Reapplies a previously undone state
    pub fn redo(&mut self) -> bool {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(UndoSnapshot {
                lines: self.lines.clone(),
                cursor_row: self.cursor_row,
                cursor_col: self.cursor_col,
                mark_active: self.mark_active,
                mark_row: self.mark_row,
                mark_col: self.mark_col,
            });
            self.lines = snap.lines;
            self.cursor_row = snap.cursor_row;
            self.cursor_col = snap.cursor_col;
            self.mark_active = snap.mark_active;
            self.mark_row = snap.mark_row;
            self.mark_col = snap.mark_col;
            self.clamp_cursor();
            self.record_edit();
            self.status_message = "Redo".to_string();
            true
        } else {
            self.status_message = "Nothing to redo".to_string();
            false
        }
    }

    /// Toggles text range selection mark
    pub fn toggle_mark(&mut self) -> bool {
        self.mark_active = !self.mark_active;
        if self.mark_active {
            self.mark_row = self.cursor_row;
            self.mark_col = self.cursor_col;
            self.status_message = "Mark set".to_string();
        } else {
            self.status_message = "Mark cleared".to_string();
        }
        self.mark_active
    }

    /// Computes ordered `((start_y, start_x), (end_y, end_x))` bounds of active selection
    pub fn get_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        if !self.mark_active
            || (self.mark_row == self.cursor_row && self.mark_col == self.cursor_col)
        {
            return None;
        }
        let p1 = (self.mark_row, self.mark_col);
        let p2 = (self.cursor_row, self.cursor_col);
        if p1 <= p2 {
            Some((p1, p2))
        } else {
            Some((p2, p1))
        }
    }

    /// Deletes text within active selection bounds
    pub fn delete_selection(&mut self) -> bool {
        let Some(((sy, sx), (ey, ex))) = self.get_selection_range() else {
            return false;
        };
        if sy == ey {
            let line = &mut self.lines[sy];
            let chars: Vec<char> = line.chars().collect();
            let s_idx = sx.min(chars.len());
            let e_idx = ex.min(chars.len());
            let prefix: String = chars[..s_idx].iter().collect();
            let suffix: String = chars[e_idx..].iter().collect();
            *line = format!("{}{}", prefix, suffix);
        } else {
            let start_prefix: String = self.lines[sy].chars().take(sx).collect();
            let end_suffix: String = self.lines[ey].chars().skip(ex).collect();
            self.lines[sy] = format!("{}{}", start_prefix, end_suffix);
            self.lines.drain((sy + 1)..=ey);
        }
        self.cursor_row = sy;
        self.cursor_col = sx;
        self.mark_active = false;
        self.clamp_cursor();
        self.record_edit();
        true
    }

    /// Copies selected range or active line into cutbuffer
    pub fn copy_selection(&self, cutbuffer: &mut Vec<String>) -> bool {
        if let Some(((sy, sx), (ey, ex))) = self.get_selection_range() {
            if sy == ey {
                let line = &self.lines[sy];
                let chars: Vec<char> = line.chars().collect();
                let s_idx = sx.min(chars.len());
                let e_idx = ex.min(chars.len());
                let snippet: String = chars[s_idx..e_idx].iter().collect();
                *cutbuffer = vec![snippet];
            } else {
                let mut cut = Vec::new();
                let start_part: String = self.lines[sy].chars().skip(sx).collect();
                cut.push(start_part);
                cut.extend(self.lines[(sy + 1)..ey].iter().cloned());
                let end_part: String = self.lines[ey].chars().take(ex).collect();
                cut.push(end_part);
                *cutbuffer = cut;
            }
        } else if self.cursor_row < self.lines.len() {
            *cutbuffer = vec![self.lines[self.cursor_row].clone()];
        }
        set_clipboard_text(&cutbuffer.join("\n"));
        true
    }

    /// Cuts selected range or active line into cutbuffer
    pub fn cut(&mut self, cutbuffer: &mut Vec<String>) -> bool {
        if self.is_read_only {
            return false;
        }
        self.push_undo();
        if self.get_selection_range().is_some() {
            self.copy_selection(cutbuffer);
            self.delete_selection();
        } else if self.cursor_row < self.lines.len() {
            if self.lines.len() > 1 {
                let removed = self.lines.remove(self.cursor_row);
                *cutbuffer = vec![removed];
                if self.cursor_row >= self.lines.len() {
                    self.cursor_row = self.lines.len() - 1;
                }
            } else {
                *cutbuffer = vec![self.lines[0].clone()];
                self.lines[0].clear();
                self.cursor_col = 0;
            }
            self.clamp_cursor();
            self.record_edit();
        }
        set_clipboard_text(&cutbuffer.join("\n"));
        true
    }

    /// Pastes lines from cutbuffer or fallback clipboard at cursor
    pub fn paste(&mut self, cutbuffer: &[String]) -> bool {
        if self.is_read_only {
            return false;
        }
        let fallback = if cutbuffer.is_empty() {
            get_clipboard_text()
                .map(|s| s.lines().map(|l| l.to_string()).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            cutbuffer.to_vec()
        };
        if fallback.is_empty() {
            self.status_message = "Buffer empty".to_string();
            return false;
        }

        self.push_undo();
        if self.mark_active && self.get_selection_range().is_some() {
            self.delete_selection();
        }

        if fallback.len() == 1 {
            let snippet = &fallback[0];
            let line = &mut self.lines[self.cursor_row];
            let chars: Vec<char> = line.chars().collect();
            let col = self.cursor_col.min(chars.len());
            let prefix: String = chars[..col].iter().collect();
            let suffix: String = chars[col..].iter().collect();
            *line = format!("{}{}{}", prefix, snippet, suffix);
            self.cursor_col = col + snippet.chars().count();
        } else {
            let line = &self.lines[self.cursor_row];
            let chars: Vec<char> = line.chars().collect();
            let col = self.cursor_col.min(chars.len());
            let prefix: String = chars[..col].iter().collect();
            let suffix: String = chars[col..].iter().collect();

            self.lines[self.cursor_row] = format!("{}{}", prefix, fallback[0]);
            for (idx, c_line) in fallback.iter().enumerate().skip(1) {
                self.lines.insert(self.cursor_row + idx, c_line.clone());
            }
            self.cursor_row += fallback.len() - 1;
            let end_col = fallback.last().map(|s| s.chars().count()).unwrap_or(0);
            self.lines[self.cursor_row].push_str(&suffix);
            self.cursor_col = end_col;
        }
        self.clamp_cursor();
        self.record_edit();
        self.status_message = "Pasted".to_string();
        true
    }

    /// Searches for `query` starting from current cursor position
    pub fn search(&mut self, query: &str) -> bool {
        if query.is_empty() || self.lines.is_empty() {
            return false;
        }
        let total_lines = self.lines.len();
        let start_row = self.cursor_row;
        let start_col = self.cursor_col + 1;

        // Search remainder of current line
        if start_row < total_lines {
            let line = &self.lines[start_row];
            let chars: Vec<char> = line.chars().collect();
            if start_col < chars.len() {
                let remainder: String = chars[start_col..].iter().collect();
                if let Some(pos) = remainder.find(query) {
                    let match_char_offset = remainder[..pos].chars().count();
                    self.cursor_row = start_row;
                    self.cursor_col = start_col + match_char_offset;
                    self.status_message = format!(
                        "Found '{}' at line {}, col {}",
                        query,
                        self.cursor_row + 1,
                        self.cursor_col + 1
                    );
                    return true;
                }
            }
        }

        // Search following lines
        for r in (start_row + 1)..total_lines {
            if let Some(pos) = self.lines[r].find(query) {
                let match_char_offset = self.lines[r][..pos].chars().count();
                self.cursor_row = r;
                self.cursor_col = match_char_offset;
                self.status_message = format!(
                    "Found '{}' at line {}, col {}",
                    query,
                    self.cursor_row + 1,
                    self.cursor_col + 1
                );
                return true;
            }
        }

        // Wrap around from beginning of document
        for r in 0..=start_row {
            let line = &self.lines[r];
            if r == start_row {
                let chars: Vec<char> = line.chars().collect();
                let limit = start_col.min(chars.len());
                let prefix: String = chars[..limit].iter().collect();
                if let Some(pos) = prefix.find(query) {
                    let match_char_offset = prefix[..pos].chars().count();
                    self.cursor_row = r;
                    self.cursor_col = match_char_offset;
                    self.status_message = format!(
                        "Found '{}' at line {}, col {}",
                        query,
                        self.cursor_row + 1,
                        self.cursor_col + 1
                    );
                    return true;
                }
            } else if let Some(pos) = line.find(query) {
                let match_char_offset = line[..pos].chars().count();
                self.cursor_row = r;
                self.cursor_col = match_char_offset;
                self.status_message = format!(
                    "Found '{}' at line {}, col {}",
                    query,
                    self.cursor_row + 1,
                    self.cursor_col + 1
                );
                return true;
            }
        }

        self.status_message = format!("'{}' not found", query);
        false
    }

    /// Inserts a single Unicode character at current cursor position, replacing selection if active.
    pub fn insert_char(&mut self, ch: char) {
        if self.is_read_only {
            return;
        }
        self.push_undo();
        if self.mark_active && self.get_selection_range().is_some() {
            self.delete_selection();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let line = &mut self.lines[self.cursor_row];
        let byte_pos = line
            .char_indices()
            .nth(self.cursor_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        line.insert(byte_pos, ch);
        self.cursor_col += 1;
        self.record_edit();
    }

    /// Splits current line at cursor position, creating a new line below.
    pub fn insert_newline(&mut self) {
        if self.is_read_only {
            return;
        }
        self.push_undo();
        if self.mark_active && self.get_selection_range().is_some() {
            self.delete_selection();
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
            self.lines.push(String::new());
            self.cursor_row = 1;
            self.cursor_col = 0;
            self.record_edit();
            return;
        }

        let line = &self.lines[self.cursor_row];
        let byte_pos = line
            .char_indices()
            .nth(self.cursor_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());

        let remainder = line[byte_pos..].to_string();
        self.lines[self.cursor_row].truncate(byte_pos);
        self.lines.insert(self.cursor_row + 1, remainder);

        self.cursor_row += 1;
        self.cursor_col = 0;
        self.record_edit();
    }

    /// Deletes the character before the cursor (or active selection).
    pub fn delete_backspace(&mut self) {
        if self.is_read_only {
            return;
        }
        self.push_undo();
        if self.mark_active && self.get_selection_range().is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let prev_col = self.cursor_col - 1;
            let byte_pos = line
                .char_indices()
                .nth(prev_col)
                .map(|(i, _)| i)
                .unwrap_or(0);
            line.remove(byte_pos);
            self.cursor_col -= 1;
            self.record_edit();
        } else if self.cursor_row > 0 {
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current);
            self.cursor_col = prev_len;
            self.record_edit();
        }
    }

    /// Deletes the character under the cursor (or merges following line).
    pub fn delete_char(&mut self) {
        if self.is_read_only {
            return;
        }
        self.push_undo();
        if self.mark_active && self.get_selection_range().is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor_row < self.lines.len() {
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col < line_len {
                let line = &mut self.lines[self.cursor_row];
                let byte_pos = line
                    .char_indices()
                    .nth(self.cursor_col)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                line.remove(byte_pos);
                self.record_edit();
            } else if self.cursor_row + 1 < self.lines.len() {
                let next = self.lines.remove(self.cursor_row + 1);
                self.lines[self.cursor_row].push_str(&next);
                self.record_edit();
            }
        }
    }

    /// Clamps cursor row and column within document bounds.
    pub fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_row >= self.lines.len() {
            self.cursor_row = self.lines.len() - 1;
        }
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Moves cursor left by one character, wrapping to previous line end if at column 0.
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    /// Moves cursor left to previous word boundary.
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

    /// Moves cursor right to next word boundary.
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

    /// Moves cursor upward to start of previous paragraph block.
    pub fn move_paragraph_up(&mut self) {
        if self.cursor_row == 0 {
            self.cursor_col = 0;
            return;
        }
        let mut r = self.cursor_row - 1;
        while r > 0 && self.lines[r].trim().is_empty() {
            r -= 1;
        }
        while r > 0 && !self.lines[r - 1].trim().is_empty() {
            r -= 1;
        }
        self.cursor_row = r;
        self.cursor_col = 0;
    }

    /// Moves cursor downward to start of next paragraph block.
    pub fn move_paragraph_down(&mut self) {
        if self.lines.is_empty() || self.cursor_row >= self.lines.len() - 1 {
            return;
        }
        let mut r = self.cursor_row + 1;
        while r < self.lines.len() && !self.lines[r].trim().is_empty() {
            r += 1;
        }
        while r < self.lines.len() && self.lines[r].trim().is_empty() {
            r += 1;
        }
        self.cursor_row = r.min(self.lines.len() - 1);
        self.cursor_col = 0;
    }

    /// Moves cursor right by one character, wrapping to next line start if at end.
    pub fn move_right(&mut self) {
        if self.cursor_row < self.lines.len() {
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col < line_len {
                self.cursor_col += 1;
            } else if self.cursor_row + 1 < self.lines.len() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
        }
    }

    /// Moves cursor up by one line, clamping column to target line length.
    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    /// Moves cursor down by one line, clamping column to target line length.
    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    /// Moves cursor to column 0 of current line.
    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    /// Moves cursor to end of current line.
    pub fn move_end(&mut self) {
        if self.cursor_row < self.lines.len() {
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    /// Scrolls cursor up by page size in lines.
    pub fn page_up(&mut self, page_size: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(page_size);
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Scrolls cursor down by page size in lines.
    pub fn page_down(&mut self, page_size: usize) {
        if !self.lines.is_empty() {
            self.cursor_row = (self.cursor_row + page_size).min(self.lines.len() - 1);
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }
}

/// Returns the filesystem path to the shared cross-tool cutbuffer file.
pub fn get_shared_cutbuffer_path() -> PathBuf {
    bashmenu_tui::clipboard::get_shared_cutbuffer_path()
}

/// Reads current text content from the shared system or local clipboard.
pub fn get_clipboard_text() -> Option<String> {
    bashmenu_tui::get_clipboard_text()
}

/// Writes text to the shared system or local clipboard.
pub fn set_clipboard_text(text: &str) {
    bashmenu_tui::set_clipboard_text(text);
}

/// Represents a single visual screen row for a soft-wrapped document line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualLine {
    /// Zero-based document line index
    pub doc_row: usize,
    /// Start character column offset (inclusive) in document line
    pub start_col: usize,
    /// End character column offset (exclusive) in document line
    pub end_col: usize,
    /// True if this is the first visual row of the logical line
    pub is_first_chunk: bool,
    /// True if this is the last visual row of the logical line
    pub is_last_chunk: bool,
}

/// Computes word-wrapped slices `(start_col, end_col)` for a character line.
///
/// Wraps at whitespace if possible, breaking long words only if they exceed
/// `width`.
pub fn wrap_line_ranges(chars: &[char], width: usize) -> Vec<(usize, usize)> {
    if chars.is_empty() || width == 0 {
        return vec![(0, 0)];
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let remaining = chars.len() - start;
        if remaining <= width {
            ranges.push((start, chars.len()));
            break;
        }

        let max_end = start + width;
        let mut break_point = None;
        for i in (start + 1..=max_end).rev() {
            if chars[i - 1].is_whitespace() {
                break_point = Some(i);
                break;
            }
        }

        let end = break_point.unwrap_or(max_end);
        ranges.push((start, end));
        start = end;
    }
    ranges
}

/// Computes the complete visual line layout for all lines in the buffer.
pub fn compute_visual_lines(lines: &[String], width: usize) -> Vec<VisualLine> {
    if lines.is_empty() || width == 0 {
        return vec![VisualLine {
            doc_row: 0,
            start_col: 0,
            end_col: 0,
            is_first_chunk: true,
            is_last_chunk: true,
        }];
    }
    let mut result = Vec::new();
    for (doc_row, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let ranges = wrap_line_ranges(&chars, width);
        let n_ranges = ranges.len();
        for (chunk_idx, (start_col, end_col)) in ranges.into_iter().enumerate() {
            result.push(VisualLine {
                doc_row,
                start_col,
                end_col,
                is_first_chunk: chunk_idx == 0,
                is_last_chunk: chunk_idx + 1 == n_ranges,
            });
        }
    }
    result
}

/// Finds the visual row index and column offset for given logical coordinates.
pub fn cursor_to_visual(
    v_lines: &[VisualLine],
    cursor_row: usize,
    cursor_col: usize,
) -> (usize, usize) {
    if v_lines.is_empty() {
        return (0, 0);
    }
    let mut last_match = 0;
    for (v_idx, vl) in v_lines.iter().enumerate() {
        if vl.doc_row == cursor_row {
            last_match = v_idx;
            if vl.is_last_chunk {
                if cursor_col >= vl.start_col && cursor_col <= vl.end_col {
                    return (v_idx, cursor_col.saturating_sub(vl.start_col));
                }
            } else if cursor_col >= vl.start_col && cursor_col < vl.end_col {
                return (v_idx, cursor_col - vl.start_col);
            }
        } else if vl.doc_row > cursor_row {
            break;
        }
    }
    let vl = &v_lines[last_match];
    (last_match, cursor_col.saturating_sub(vl.start_col))
}

/// Moves cursor up by one visual row in soft-wrapped layout.
pub fn move_visual_up(v_lines: &[VisualLine], cursor_row: &mut usize, cursor_col: &mut usize) {
    if v_lines.is_empty() {
        return;
    }
    let (v_idx, col_offset) = cursor_to_visual(v_lines, *cursor_row, *cursor_col);
    if v_idx > 0 {
        let target_vl = &v_lines[v_idx - 1];
        let slice_len = target_vl.end_col - target_vl.start_col;
        let max_col = if target_vl.is_last_chunk {
            slice_len
        } else {
            slice_len.saturating_sub(1)
        };
        *cursor_row = target_vl.doc_row;
        *cursor_col = target_vl.start_col + col_offset.min(max_col);
    }
}

/// Moves cursor down by one visual row in soft-wrapped layout.
pub fn move_visual_down(v_lines: &[VisualLine], cursor_row: &mut usize, cursor_col: &mut usize) {
    if v_lines.is_empty() {
        return;
    }
    let (v_idx, col_offset) = cursor_to_visual(v_lines, *cursor_row, *cursor_col);
    if v_idx + 1 < v_lines.len() {
        let target_vl = &v_lines[v_idx + 1];
        let slice_len = target_vl.end_col - target_vl.start_col;
        let max_col = if target_vl.is_last_chunk {
            slice_len
        } else {
            slice_len.saturating_sub(1)
        };
        *cursor_row = target_vl.doc_row;
        *cursor_col = target_vl.start_col + col_offset.min(max_col);
    }
}

/// Moves cursor to start of visual row (or document line start if already there).
pub fn move_visual_home(v_lines: &[VisualLine], cursor_row: usize, cursor_col: &mut usize) {
    if v_lines.is_empty() {
        *cursor_col = 0;
        return;
    }
    let (v_idx, _) = cursor_to_visual(v_lines, cursor_row, *cursor_col);
    let target_vl = &v_lines[v_idx];
    if *cursor_col != target_vl.start_col {
        *cursor_col = target_vl.start_col;
    } else {
        *cursor_col = 0;
    }
}

/// Moves cursor to end of visual row (or document line end if already there).
pub fn move_visual_end(
    v_lines: &[VisualLine],
    cursor_row: usize,
    cursor_col: &mut usize,
    line_len: usize,
) {
    if v_lines.is_empty() {
        *cursor_col = line_len;
        return;
    }
    let (v_idx, _) = cursor_to_visual(v_lines, cursor_row, *cursor_col);
    let target_vl = &v_lines[v_idx];
    let end_limit = if target_vl.is_last_chunk {
        line_len
    } else {
        target_vl.end_col.saturating_sub(1)
    };
    if *cursor_col < end_limit {
        *cursor_col = end_limit;
    } else {
        *cursor_col = line_len;
    }
}

/// Scrolls cursor up by `page_size` visual rows.
pub fn page_visual_up(
    v_lines: &[VisualLine],
    cursor_row: &mut usize,
    cursor_col: &mut usize,
    page_size: usize,
) {
    if v_lines.is_empty() {
        return;
    }
    let (v_idx, col_offset) = cursor_to_visual(v_lines, *cursor_row, *cursor_col);
    let target_idx = v_idx.saturating_sub(page_size);
    let target_vl = &v_lines[target_idx];
    let slice_len = target_vl.end_col - target_vl.start_col;
    let max_col = if target_vl.is_last_chunk {
        slice_len
    } else {
        slice_len.saturating_sub(1)
    };
    *cursor_row = target_vl.doc_row;
    *cursor_col = target_vl.start_col + col_offset.min(max_col);
}

/// Scrolls cursor down by `page_size` visual rows.
pub fn page_visual_down(
    v_lines: &[VisualLine],
    cursor_row: &mut usize,
    cursor_col: &mut usize,
    page_size: usize,
) {
    if v_lines.is_empty() {
        return;
    }
    let (v_idx, col_offset) = cursor_to_visual(v_lines, *cursor_row, *cursor_col);
    let target_idx = (v_idx + page_size).min(v_lines.len().saturating_sub(1));
    let target_vl = &v_lines[target_idx];
    let slice_len = target_vl.end_col - target_vl.start_col;
    let max_col = if target_vl.is_last_chunk {
        slice_len
    } else {
        slice_len.saturating_sub(1)
    };
    *cursor_row = target_vl.doc_row;
    *cursor_col = target_vl.start_col + col_offset.min(max_col);
}
