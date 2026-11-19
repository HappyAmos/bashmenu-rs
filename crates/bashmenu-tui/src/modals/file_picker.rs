//! Interactive file and directory picker dialog

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use bashmenu_core::t;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents an entry (file or sub-directory) displayed in the file picker.
#[derive(Debug, Clone)]
struct DirItem {
    /// Display name of the entry
    name: String,
    /// Whether this entry is a directory
    is_dir: bool,
    /// Absolute or relative filesystem path to the entry
    path: PathBuf,
}

/// Focus state within the Save As file picker dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerFocus {
    /// File and directory list
    List,
    /// File name text input box
    FileNameInput,
    /// Save button
    SaveButton,
    /// Cancel button
    CancelButton,
}

/// Interactive file and directory picker dialog
#[derive(Debug, Clone)]
pub struct FilePickerModal {
    /// Dialog title header
    pub title: String,
    /// Currently navigated directory path
    pub current_dir: PathBuf,
    /// Discovered directory entries (sub-directories and files)
    items: Vec<DirItem>,
    /// Index of currently selected item in directory list
    pub selected_idx: usize,
    /// Vertical scroll offset in lines
    pub scroll_offset: usize,
    /// Whether only directories can be chosen
    pub only_dirs: bool,
    /// Whether user is permitted to create new files or folders
    pub allow_new: bool,
    /// Whether modal is currently in 'create new item' text prompt mode
    pub creating_new: bool,
    /// Text input buffer when creating a new file or folder
    pub new_name_input: String,
    /// Whether hidden files and directories (dotfiles) are displayed
    pub show_hidden: bool,
    /// Whether modal is operating as a 'Save As' destination picker dialog
    pub is_save_as: bool,
    /// Text input buffer for the destination file name in Save As mode
    pub file_name_input: String,
    /// Cursor position in characters within `file_name_input`
    pub input_cursor_pos: usize,
    /// Currently focused control in Save As mode
    pub focused_control: FilePickerFocus,
}

impl FilePickerModal {
    /// Constructs a new file picker starting at the specified path.
    ///
    /// Resolves relative paths (such as `Path::new(".")`) to absolute paths so
    /// that parent traversal (`..`) can ascend out of the project directory.
    pub fn new(title: impl Into<String>, start_dir: &Path, only_dirs: bool) -> Self {
        let abs_start = if start_dir.is_absolute() {
            start_dir.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(start_dir))
                .unwrap_or_else(|_| start_dir.to_path_buf())
        };
        let canonical = abs_start.canonicalize().unwrap_or(abs_start);
        let current_dir = if canonical.is_dir() {
            canonical
        } else if let Some(parent) = canonical.parent() {
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
            show_hidden: false,
            is_save_as: false,
            file_name_input: String::new(),
            input_cursor_pos: 0,
            focused_control: FilePickerFocus::List,
        };
        picker.refresh_entries();
        picker
    }

    /// Builder method to allow creating new files or folders
    pub fn with_allow_new(mut self, allow_new: bool) -> Self {
        self.allow_new = allow_new;
        self
    }

    /// Builder method to configure initial hidden files visibility
    pub fn with_show_hidden(mut self, show_hidden: bool) -> Self {
        self.show_hidden = show_hidden;
        self.refresh_entries();
        self
    }

    /// Configures modal as a 'Save As' dialog with a dedicated file name input
    /// box and Save/Cancel buttons.
    pub fn with_save_as(mut self, save_as: bool) -> Self {
        self.is_save_as = save_as;
        self
    }

    /// Sets the initial file name shown in the Save As text input box.
    pub fn with_file_name(mut self, name: &str) -> Self {
        self.file_name_input = name.to_string();
        self.input_cursor_pos = self.file_name_input.chars().count();
        self
    }

    /// Handles clipboard paste event into active text field
    pub fn handle_paste(&mut self, text: &str) {
        let clean: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        if clean.is_empty() {
            return;
        }
        if self.is_save_as {
            let mut chars: Vec<char> = self.file_name_input.chars().collect();
            let pos = self.input_cursor_pos.min(chars.len());
            for (i, ch) in clean.chars().enumerate() {
                chars.insert(pos + i, ch);
            }
            self.input_cursor_pos += clean.chars().count();
            self.file_name_input = chars.into_iter().collect();
        } else if self.creating_new {
            self.new_name_input.push_str(&clean);
        }
    }

    /// Confirms saving the file in the selected directory using the entered
    /// file name.
    pub fn confirm_save(&mut self) -> Option<Option<PathBuf>> {
        let trimmed = self.file_name_input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let target_dir = if let Some(item) = self.items.get(self.selected_idx) {
            if item.is_dir
                && item.name != t!("common.dialogs.parent_dir")
                && item.name != ".. (Parent Directory)"
            {
                item.path.clone()
            } else {
                self.current_dir.clone()
            }
        } else {
            self.current_dir.clone()
        };
        Some(Some(target_dir.join(trimmed)))
    }

    /// Returns total number of discovered items in currently navigated directory
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns display name of currently selected item, if any
    pub fn selected_item_name(&self) -> Option<&str> {
        self.items.get(self.selected_idx).map(|i| i.name.as_str())
    }

    /// Toggles visibility of hidden dotfile entries and refreshes directory
    /// listing.
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let prev_path = self.items.get(self.selected_idx).map(|i| i.path.clone());
        self.refresh_entries();
        if let Some(p) = prev_path {
            if let Some(idx) = self.items.iter().position(|i| i.path == p) {
                self.selected_idx = idx;
            }
        }
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
                name: format!("[ {} ]", t!("common.dialogs.select_current_dir")),
                is_dir: false,
                path: self.current_dir.clone(),
            });
        }

        // Parent directory item
        if let Some(parent) = self.current_dir.parent() {
            self.items.push(DirItem {
                name: t!("common.dialogs.parent_dir"),
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

                if !self.show_hidden && name.starts_with('.') {
                    continue; // Skip hidden unless show_hidden is enabled
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

    /// Handles keyboard input with modifiers. Returns:
    /// - `Some(Some(path))` when file/dir selected
    /// - `Some(None)` when cancelled
    /// - `None` while open
    pub fn handle_key_with_modifiers(
        &mut self,
        code: KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) -> Option<Option<PathBuf>> {
        if self.is_save_as {
            if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if let Some(text) = crate::clipboard::get_clipboard_text() {
                            self.handle_paste(&text);
                        }
                        return None;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if !self.file_name_input.is_empty() {
                            crate::clipboard::set_clipboard_text(&self.file_name_input);
                        }
                        return None;
                    }
                    KeyCode::Char('u') | KeyCode::Char('U') => {
                        self.file_name_input.clear();
                        self.input_cursor_pos = 0;
                        return None;
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        self.input_cursor_pos = 0;
                        return None;
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') => {
                        self.input_cursor_pos = self.file_name_input.chars().count();
                        return None;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        self.toggle_hidden();
                        return None;
                    }
                    _ => {}
                }
            }

            match code {
                KeyCode::Esc => return Some(None),
                KeyCode::Tab => {
                    self.focused_control = match self.focused_control {
                        FilePickerFocus::FileNameInput => FilePickerFocus::SaveButton,
                        FilePickerFocus::SaveButton => FilePickerFocus::CancelButton,
                        FilePickerFocus::CancelButton => FilePickerFocus::List,
                        FilePickerFocus::List => FilePickerFocus::FileNameInput,
                    };
                    return None;
                }
                KeyCode::BackTab => {
                    self.focused_control = match self.focused_control {
                        FilePickerFocus::FileNameInput => FilePickerFocus::List,
                        FilePickerFocus::List => FilePickerFocus::CancelButton,
                        FilePickerFocus::CancelButton => FilePickerFocus::SaveButton,
                        FilePickerFocus::SaveButton => FilePickerFocus::FileNameInput,
                    };
                    return None;
                }
                _ => {}
            }

            match self.focused_control {
                FilePickerFocus::FileNameInput => {
                    let char_count = self.file_name_input.chars().count();
                    if self.input_cursor_pos > char_count {
                        self.input_cursor_pos = char_count;
                    }
                    match code {
                        KeyCode::Enter => self.confirm_save(),
                        KeyCode::Up => {
                            self.focused_control = FilePickerFocus::List;
                            None
                        }
                        KeyCode::Down => {
                            self.focused_control = FilePickerFocus::SaveButton;
                            None
                        }
                        KeyCode::Left => {
                            if self.input_cursor_pos > 0 {
                                self.input_cursor_pos -= 1;
                            }
                            None
                        }
                        KeyCode::Right => {
                            if self.input_cursor_pos < char_count {
                                self.input_cursor_pos += 1;
                            }
                            None
                        }
                        KeyCode::Home => {
                            self.input_cursor_pos = 0;
                            None
                        }
                        KeyCode::End => {
                            self.input_cursor_pos = char_count;
                            None
                        }
                        KeyCode::Backspace => {
                            if self.input_cursor_pos > 0 {
                                let mut chars: Vec<char> = self.file_name_input.chars().collect();
                                chars.remove(self.input_cursor_pos - 1);
                                self.file_name_input = chars.into_iter().collect();
                                self.input_cursor_pos -= 1;
                            }
                            None
                        }
                        KeyCode::Delete => {
                            if self.input_cursor_pos < char_count {
                                let mut chars: Vec<char> = self.file_name_input.chars().collect();
                                chars.remove(self.input_cursor_pos);
                                self.file_name_input = chars.into_iter().collect();
                            }
                            None
                        }
                        KeyCode::Char(c) => {
                            let mut chars: Vec<char> = self.file_name_input.chars().collect();
                            let pos = self.input_cursor_pos.min(chars.len());
                            chars.insert(pos, c);
                            self.file_name_input = chars.into_iter().collect();
                            self.input_cursor_pos += 1;
                            None
                        }
                        _ => None,
                    }
                }
                FilePickerFocus::SaveButton => match code {
                    KeyCode::Enter | KeyCode::Char(' ') => self.confirm_save(),
                    KeyCode::Right => {
                        self.focused_control = FilePickerFocus::CancelButton;
                        None
                    }
                    KeyCode::Left | KeyCode::Up => {
                        self.focused_control = FilePickerFocus::FileNameInput;
                        None
                    }
                    _ => None,
                },
                FilePickerFocus::CancelButton => match code {
                    KeyCode::Enter | KeyCode::Char(' ') => Some(None),
                    KeyCode::Left => {
                        self.focused_control = FilePickerFocus::SaveButton;
                        None
                    }
                    KeyCode::Up => {
                        self.focused_control = FilePickerFocus::FileNameInput;
                        None
                    }
                    _ => None,
                },
                FilePickerFocus::List => match code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.selected_idx > 0 {
                            self.selected_idx -= 1;
                            if let Some(item) = self.items.get(self.selected_idx) {
                                if !item.is_dir {
                                    self.file_name_input = item.name.clone();
                                    self.input_cursor_pos = self.file_name_input.chars().count();
                                }
                            }
                        }
                        None
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !self.items.is_empty() && self.selected_idx + 1 < self.items.len() {
                            self.selected_idx += 1;
                            if let Some(item) = self.items.get(self.selected_idx) {
                                if !item.is_dir {
                                    self.file_name_input = item.name.clone();
                                    self.input_cursor_pos = self.file_name_input.chars().count();
                                }
                            }
                        } else {
                            self.focused_control = FilePickerFocus::FileNameInput;
                        }
                        None
                    }
                    KeyCode::Enter => {
                        if let Some(item) = self.items.get(self.selected_idx) {
                            if item.is_dir {
                                self.activate_selected()
                            } else {
                                self.file_name_input = item.name.clone();
                                self.input_cursor_pos = self.file_name_input.chars().count();
                                self.confirm_save()
                            }
                        } else {
                            None
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('.') => {
                        self.toggle_hidden();
                        None
                    }
                    _ => None,
                },
            }
        } else if self.creating_new {
            if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        if let Some(text) = crate::clipboard::get_clipboard_text() {
                            let clean: String =
                                text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                            self.new_name_input.push_str(&clean);
                        }
                        return None;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if !self.new_name_input.is_empty() {
                            crate::clipboard::set_clipboard_text(&self.new_name_input);
                        }
                        return None;
                    }
                    KeyCode::Char('u') | KeyCode::Char('U') => {
                        self.new_name_input.clear();
                        return None;
                    }
                    _ => {}
                }
            }
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
            if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        self.toggle_hidden();
                        return None;
                    }
                    _ => {}
                }
            }
            match code {
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('.') => {
                    self.toggle_hidden();
                    None
                }
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

    /// Handles keyboard input (without explicit modifiers).
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<PathBuf>> {
        self.handle_key_with_modifiers(code, crossterm::event::KeyModifiers::NONE)
    }

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let dialog_w = 70.min(area.width.saturating_sub(4));
        let dialog_h = if self.is_save_as {
            24.min(area.height.saturating_sub(2))
        } else {
            22.min(area.height.saturating_sub(2))
        };
        (dialog_w, dialog_h)
    }

    /// Handles immediate mouse interaction
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<PathBuf>> {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner_area = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;
            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return Some(None);
            }

            if self.is_save_as {
                let list_start_y = inner_area.y + 2;
                let list_h = inner_area.height.saturating_sub(6);
                let input_y = inner_area.y + inner_area.height.saturating_sub(3);
                let btn_y = inner_area.y + inner_area.height.saturating_sub(1);

                if row >= list_start_y && row < list_start_y + list_h {
                    let clicked_idx = self.scroll_offset + (row - list_start_y) as usize;
                    if clicked_idx < self.items.len() {
                        let is_already_selected = self.selected_idx == clicked_idx;
                        self.selected_idx = clicked_idx;
                        self.focused_control = FilePickerFocus::List;
                        let item = &self.items[clicked_idx];
                        if item.is_dir {
                            if is_already_selected {
                                return self.activate_selected();
                            }
                        } else {
                            self.file_name_input = item.name.clone();
                            self.input_cursor_pos = self.file_name_input.chars().count();
                            if is_already_selected {
                                return self.confirm_save();
                            }
                        }
                    }
                } else if row == input_y {
                    self.focused_control = FilePickerFocus::FileNameInput;
                    let input_label = format!(" {} [ ", t!("common.dialogs.filename_label"));
                    let label_len = input_label.len() as u16;
                    let input_start_x = inner_area.x + 2 + label_len;
                    if col >= input_start_x {
                        let click_offset = (col - input_start_x) as usize;
                        let char_count = self.file_name_input.chars().count();
                        self.input_cursor_pos = click_offset.min(char_count);
                    }
                } else if row == btn_y {
                    let rel_x = col.saturating_sub(inner_area.x + 2) as usize;
                    let save_btn_str = format!("[ {} ]", t!("common.buttons.save"));
                    let cancel_btn_str = format!("[ {} ]", t!("common.buttons.cancel"));
                    let hidden_tag = if self.show_hidden {
                        format!("[h] {}", t!("common.dialogs.hide_hidden"))
                    } else {
                        format!("[h] {}", t!("common.dialogs.show_hidden"))
                    };
                    let save_w = save_btn_str.len();
                    let cancel_start = save_w + 2;
                    let cancel_w = cancel_btn_str.len();
                    let hidden_start =
                        (inner_area.width as usize).saturating_sub(hidden_tag.len() + 2);

                    if rel_x < save_w {
                        self.focused_control = FilePickerFocus::SaveButton;
                        return self.confirm_save();
                    } else if rel_x >= cancel_start && rel_x < cancel_start + cancel_w {
                        self.focused_control = FilePickerFocus::CancelButton;
                        return Some(None);
                    } else if col >= inner_area.x + hidden_start as u16 {
                        self.toggle_hidden();
                        return None;
                    }
                }
            } else {
                let list_start_y = inner_area.y + 2;
                let list_h = inner_area.height.saturating_sub(4);

                if row >= list_start_y && row < list_start_y + list_h {
                    let clicked_idx = self.scroll_offset + (row - list_start_y) as usize;
                    if clicked_idx < self.items.len() {
                        self.selected_idx = clicked_idx;
                        return self.activate_selected();
                    }
                } else if !self.creating_new {
                    let btn_y = inner_area.y + inner_area.height.saturating_sub(1);
                    if row == btn_y {
                        let rel_x = col.saturating_sub(inner_area.x + 2) as usize;
                        let hidden_tag = if self.show_hidden {
                            format!("[h] {}", t!("common.dialogs.hide_hidden"))
                        } else {
                            format!("[h] {}", t!("common.dialogs.show_hidden"))
                        };
                        let select_hint = format!("[Enter] {}", t!("common.gutters.select"));
                        let cancel_hint = format!("[Esc] {}", t!("common.gutters.back"));
                        let new_hint = format!("[n] {}", t!("common.dialogs.new_file"));
                        let bottom_str = if self.allow_new {
                            format!(
                                "{}   {}   {}   {}",
                                select_hint, hidden_tag, new_hint, cancel_hint
                            )
                        } else {
                            format!("{}   {}   {}", select_hint, hidden_tag, cancel_hint)
                        };
                        if let Some(pos) = bottom_str.find(&cancel_hint) {
                            if rel_x >= pos {
                                return Some(None);
                            }
                        }
                        if let Some(pos) = bottom_str.find(&new_hint) {
                            if rel_x >= pos && rel_x < pos + new_hint.len() && self.allow_new {
                                self.creating_new = true;
                                self.new_name_input.clear();
                                return None;
                            }
                        }
                        if let Some(pos) = bottom_str.find(&hidden_tag) {
                            if rel_x >= pos && rel_x < pos + hidden_tag.len() {
                                self.toggle_hidden();
                                return None;
                            }
                        }
                        if let Some(pos) = bottom_str.find(&select_hint) {
                            if rel_x >= pos && rel_x < pos + select_hint.len() {
                                return self.activate_selected();
                            }
                        }
                    }
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

    /// Activates selected directory item (entering subfolder) or selected file
    /// (returning chosen path).
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
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Current Directory Path Banner
        let avail_banner_w = inner.width.saturating_sub(2) as usize;
        let path_str = format!(
            " {} {} ",
            t!("common.dialogs.dir_label"),
            self.current_dir.display()
        );
        let clipped_path: String = path_str.chars().take(avail_banner_w).collect();
        buf.set_string(
            inner.x + 1,
            inner.y,
            &clipped_path,
            styles.accent.add_modifier(Modifier::BOLD),
        );

        // 2. Entries Viewport List
        let list_y = inner.y + 2;
        let list_h = if self.is_save_as {
            inner.height.saturating_sub(6) as usize
        } else {
            inner.height.saturating_sub(4) as usize
        };
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

        // 3. Bottom controls / Save As inputs
        if self.is_save_as {
            let sep_y = inner.y + inner.height.saturating_sub(4);
            let sep_w = inner.width.saturating_sub(2) as usize;
            buf.set_string(inner.x + 1, sep_y, "─".repeat(sep_w), styles.footer);

            let input_y = inner.y + inner.height.saturating_sub(3);
            let is_input_focused = self.focused_control == FilePickerFocus::FileNameInput;
            let input_label = format!(" {} [ ", t!("common.dialogs.filename_label"));
            buf.set_string(
                inner.x + 2,
                input_y,
                &input_label,
                if is_input_focused {
                    styles.accent.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                },
            );

            let input_content_x = inner.x + 2 + input_label.len() as u16;
            let max_input_w = inner.width.saturating_sub(4 + input_label.len() as u16 + 2) as usize;

            let chars: Vec<char> = self.file_name_input.chars().collect();
            let cursor_pos = self.input_cursor_pos.min(chars.len());

            let mut input_spans = Vec::new();
            if is_input_focused {
                let pre: String = chars[..cursor_pos].iter().collect();
                let cursor_ch = chars.get(cursor_pos).copied().unwrap_or(' ');
                let post: String = if cursor_pos < chars.len() {
                    chars[cursor_pos + 1..].iter().collect()
                } else {
                    String::new()
                };

                input_spans.push(Span::styled(pre, styles.highlight));
                input_spans.push(Span::styled(
                    cursor_ch.to_string(),
                    styles.highlight.add_modifier(Modifier::REVERSED),
                ));
                input_spans.push(Span::styled(post, styles.highlight));
            } else {
                input_spans.push(Span::styled(&self.file_name_input, styles.text));
            }
            input_spans.push(Span::styled(
                " ]",
                if is_input_focused {
                    styles.accent.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                },
            ));

            let input_line = Line::from(input_spans);
            buf.set_line(
                input_content_x,
                input_y,
                &input_line,
                max_input_w as u16 + 2,
            );

            let btn_y = inner.y + inner.height.saturating_sub(1);
            let save_focused = self.focused_control == FilePickerFocus::SaveButton;
            let cancel_focused = self.focused_control == FilePickerFocus::CancelButton;

            let save_style = if save_focused {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.accent.add_modifier(Modifier::BOLD)
            };
            let cancel_style = if cancel_focused {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            let save_btn_str = format!("[ {} ]", t!("common.buttons.save"));
            let cancel_btn_str = format!("[ {} ]", t!("common.buttons.cancel"));
            buf.set_string(inner.x + 2, btn_y, &save_btn_str, save_style);
            let cancel_x = inner.x + 2 + save_btn_str.len() as u16 + 2;
            buf.set_string(cancel_x, btn_y, &cancel_btn_str, cancel_style);

            let hidden_tag = if self.show_hidden {
                format!("[h] {}", t!("common.dialogs.hide_hidden"))
            } else {
                format!("[h] {}", t!("common.dialogs.show_hidden"))
            };
            let hidden_x = (inner.x + inner.width).saturating_sub(hidden_tag.len() as u16 + 2);
            if hidden_x > inner.x + 25 {
                buf.set_string(hidden_x, btn_y, &hidden_tag, styles.gutter);
            }
        } else {
            let btn_y = inner.y + inner.height.saturating_sub(1);
            let ctrl_w = inner.width.saturating_sub(4) as usize;
            if self.creating_new {
                let prompt = format!(
                    " {} ",
                    t!(
                        "common.dialogs.new_filename_prompt",
                        name => &self.new_name_input
                    )
                );
                let clipped_prompt: String = prompt.chars().take(ctrl_w).collect();
                buf.set_string(inner.x + 2, btn_y, &clipped_prompt, styles.highlight);
            } else {
                let hidden_tag = if self.show_hidden {
                    format!("[h] {}", t!("common.dialogs.hide_hidden"))
                } else {
                    format!("[h] {}", t!("common.dialogs.show_hidden"))
                };
                let select_hint = format!("[Enter] {}", t!("common.gutters.select"));
                let cancel_hint = format!("[Esc] {}", t!("common.gutters.back"));
                let new_hint = format!("[n] {}", t!("common.dialogs.new_file"));
                let bottom_str = if self.allow_new {
                    format!(
                        "{}   {}   {}   {}",
                        select_hint, hidden_tag, new_hint, cancel_hint
                    )
                } else {
                    format!("{}   {}   {}", select_hint, hidden_tag, cancel_hint)
                };
                let clipped_bottom: String = bottom_str.chars().take(ctrl_w).collect();
                buf.set_string(inner.x + 2, btn_y, &clipped_bottom, styles.gutter);
            }
        }

        inner
    }
}
