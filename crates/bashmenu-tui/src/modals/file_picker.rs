//! Interactive file and directory picker dialog

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct DirItem {
    name: String,
    is_dir: bool,
    path: PathBuf,
}

/// Interactive file and directory picker dialog
#[derive(Debug, Clone)]
pub struct FilePickerModal {
    pub title: String,
    pub current_dir: PathBuf,
    items: Vec<DirItem>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub only_dirs: bool,
    pub allow_new: bool,
    pub creating_new: bool,
    pub new_name_input: String,
}

impl FilePickerModal {
    pub fn new(title: impl Into<String>, start_dir: &Path, only_dirs: bool) -> Self {
        let current_dir = if start_dir.is_dir() {
            start_dir.to_path_buf()
        } else if let Some(parent) = start_dir.parent() {
            if parent.is_dir() {
                parent.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
            }
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        };

        let mut picker = Self {
            title: title.into(),
            current_dir,
            items: Vec::new(),
            selected_idx: 0,
            scroll_offset: 0,
            only_dirs,
            allow_new: false,
            creating_new: false,
            new_name_input: String::new(),
        };
        picker.refresh_entries();
        picker
    }

    /// Builder method to allow creating new files or folders
    pub fn with_allow_new(mut self, allow_new: bool) -> Self {
        self.allow_new = allow_new;
        self
    }

    /// Builder method to pre-select an initial path/file if present in directory
    pub fn with_initial_selection(mut self, path: &Path) -> Self {
        if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
            if let Some(idx) = self.items.iter().position(|item| item.name == fname) {
                self.selected_idx = idx;
            }
        }
        self
    }

    /// Reload directory entries
    pub fn refresh_entries(&mut self) {
        self.items.clear();
        self.selected_idx = 0;
        self.scroll_offset = 0;

        // Current directory selection for directory pickers
        if self.only_dirs {
            self.items.push(DirItem {
                name: "[ Select Current Directory ]".to_string(),
                is_dir: false,
                path: self.current_dir.clone(),
            });
        }

        // Parent directory item
        if let Some(parent) = self.current_dir.parent() {
            self.items.push(DirItem {
                name: ".. (Parent Directory)".to_string(),
                is_dir: true,
                path: parent.to_path_buf(),
            });
        }

        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = path.is_dir();
                let name = entry.file_name().to_string_lossy().to_string();

                if name.starts_with('.') {
                    continue; // Skip hidden by default
                }

                if is_dir {
                    dirs.push(DirItem { name, is_dir, path });
                } else if !self.only_dirs {
                    files.push(DirItem { name, is_dir, path });
                }
            }

            dirs.sort_by_key(|a| a.name.to_lowercase());
            files.sort_by_key(|a| a.name.to_lowercase());

            self.items.extend(dirs);
            self.items.extend(files);
        }
    }

    /// Handles keyboard input. Returns:
    /// - `Some(Some(path))` when file/dir selected
    /// - `Some(None)` when cancelled
    /// - `None` while open
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<PathBuf>> {
        if self.creating_new {
            match code {
                KeyCode::Esc => {
                    self.creating_new = false;
                    self.new_name_input.clear();
                    None
                }
                KeyCode::Enter => {
                    let trimmed = self.new_name_input.trim();
                    if !trimmed.is_empty() {
                        let path = self.current_dir.join(trimmed);
                        self.creating_new = false;
                        self.new_name_input.clear();
                        Some(Some(path))
                    } else {
                        None
                    }
                }
                KeyCode::Backspace => {
                    self.new_name_input.pop();
                    None
                }
                KeyCode::Char(c) => {
                    self.new_name_input.push(c);
                    None
                }
                _ => None,
            }
        } else {
            match code {
                KeyCode::Char('n') | KeyCode::Char('N') if self.allow_new => {
                    self.creating_new = true;
                    self.new_name_input.clear();
                    None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_idx > 0 {
                        self.selected_idx -= 1;
                    }
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.items.is_empty() && self.selected_idx + 1 < self.items.len() {
                        self.selected_idx += 1;
                    }
                    None
                }
                KeyCode::Enter => self.activate_selected(),
                KeyCode::Esc => Some(None),
                _ => None,
            }
        }
    }

    /// Handles immediate mouse interaction
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<PathBuf>> {
        let dialog_w = 60.min(area.width.saturating_sub(4));
        let dialog_h = 18.min(area.height.saturating_sub(2));
        let inner_area = if area.width <= dialog_w {
            area
        } else {
            super::frame::compute_modal_inner(area, dialog_w, dialog_h)
        };

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;
            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return Some(None);
            }
            let list_start_y = inner_area.y + 2;
            let list_h = inner_area.height.saturating_sub(4);

            if row >= list_start_y && row < list_start_y + list_h {
                let clicked_idx = self.scroll_offset + (row - list_start_y) as usize;
                if clicked_idx < self.items.len() {
                    self.selected_idx = clicked_idx;
                    return self.activate_selected();
                }
            } else if self.allow_new && !self.creating_new {
                let btn_y = inner_area.y + inner_area.height.saturating_sub(1);
                if row == btn_y {
                    self.creating_new = true;
                    self.new_name_input.clear();
                }
            }
        } else if mouse.kind == MouseEventKind::ScrollDown {
            if self.selected_idx + 1 < self.items.len() {
                self.selected_idx += 1;
            }
        } else if mouse.kind == MouseEventKind::ScrollUp && self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
        None
    }

    fn activate_selected(&mut self) -> Option<Option<PathBuf>> {
        if self.items.is_empty() {
            return None;
        }

        let item = &self.items[self.selected_idx];
        if item.is_dir {
            self.current_dir = item.path.clone();
            self.refresh_entries();
            None
        } else {
            Some(Some(item.path.clone()))
        }
    }

    /// Renders file picker dialog
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 70.min(area.width.saturating_sub(4));
        let dialog_h = 22.min(area.height.saturating_sub(2));

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Current Directory Path Banner
        let path_str = format!(" Dir: {} ", self.current_dir.display());
        buf.set_string(
            inner.x + 1,
            inner.y,
            &path_str,
            styles.accent.add_modifier(Modifier::BOLD),
        );

        // 2. Entries Viewport List
        let list_y = inner.y + 2;
        let list_h = inner.height.saturating_sub(4) as usize;
        let list_w = inner.width.saturating_sub(2) as usize;

        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + list_h {
            self.scroll_offset = self.selected_idx.saturating_sub(list_h - 1);
        }

        for (idx, item) in self
            .items
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(list_h)
        {
            let row_y = list_y + (idx - self.scroll_offset) as u16;
            let is_selected = idx == self.selected_idx;

            let icon = if item.is_dir { "📁 " } else { "📄 " };
            let style = if is_selected {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            let line = Line::from(vec![
                Span::styled(
                    "  ",
                    if is_selected {
                        styles.highlight
                    } else {
                        styles.text
                    },
                ),
                Span::styled(icon, style),
                Span::styled(&item.name, style),
            ]);

            buf.set_line(inner.x + 1, row_y, &line, list_w as u16);
        }

        // 3. Bottom controls / New File input
        let btn_y = inner.y + inner.height.saturating_sub(1);
        if self.creating_new {
            let prompt = format!(" New file name: [{}]_ ", self.new_name_input);
            buf.set_string(inner.x + 2, btn_y, &prompt, styles.highlight);
        } else if self.allow_new {
            buf.set_string(
                inner.x + 2,
                btn_y,
                "[Enter] Select / Enter   [n] New File   [Esc] Cancel",
                styles.gutter,
            );
        } else {
            buf.set_string(
                inner.x + 2,
                btn_y,
                "[Enter] Select / Enter   [Esc] Cancel",
                styles.gutter,
            );
        }

        inner
    }
}
