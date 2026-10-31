//! Core text buffer management, swap file persistence, and keyboard commands for BashEdit.

pub mod swap;

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
pub use swap::*;

/// Text editing document buffer with cursor tracking, dirty state, and swap persistence.
pub struct EditorBuffer {
    pub file_path: Option<PathBuf>,
    pub swap_path: PathBuf,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub is_modified: bool,
    pub is_read_only: bool,
    pub base_mtime: f64,
    pub base_sha256: String,
    pub keystroke_counter: usize,
    pub status_message: String,
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
        }
    }

    /// Associates the buffer with a destination file path.
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.swap_path = get_swap_path(&path);
        self.file_path = Some(path);
        self.flush_swap();
    }

    pub fn open(file_path: impl AsRef<Path>) -> Result<Self> {
        let path = file_path.as_ref().to_path_buf();
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

    pub fn insert_char(&mut self, ch: char) {
        if self.is_read_only {
            return;
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

    pub fn insert_newline(&mut self) {
        if self.is_read_only {
            return;
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

    pub fn delete_backspace(&mut self) {
        if self.is_read_only {
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
            // Merge with previous line
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current);
            self.cursor_col = prev_len;
            self.record_edit();
        }
    }

    pub fn delete_char(&mut self) {
        if self.is_read_only {
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
                // Merge next line into current
                let next = self.lines.remove(self.cursor_row + 1);
                self.lines[self.cursor_row].push_str(&next);
                self.record_edit();
            }
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

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

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        if self.cursor_row < self.lines.len() {
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(page_size);
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

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
