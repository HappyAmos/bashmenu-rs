//! Comprehensive modal editor for menu item properties and flags.
//!
//! Provides 1:1 parity with the Python Textual `ItemEditModal`, featuring
//! dynamic divider / standard field layouts, 3-way execution mode selection,
//! live item and divider preview boxes, file browse triggers, and full footer shortcut keys.

use bashmenu_core::{
    interpolate_placeholders, BashMenuConfig, MacroContext, MenuItem, MenuItemType,
};
use bashmenu_tui::{
    compute_modal_inner, is_close_button_clicked, render_modal_frame, split_label_brackets,
    strip_bbcode, visible_text_len, ResolvedThemeStyles,
};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

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
                    self.selection_anchor = Some(0);
                    self.cursor = self.chars.len();
                    return true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String = self.chars[start..end].iter().collect();
                        bashmenu_tui::clipboard::set_clipboard_text(&snippet);
                    }
                    return true;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some((start, end)) = self.selection_range() {
                        let snippet: String = self.chars[start..end].iter().collect();
                        bashmenu_tui::clipboard::set_clipboard_text(&snippet);
                        self.delete_selection();
                        return true;
                    }
                    return false;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = bashmenu_tui::clipboard::get_clipboard_text() {
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

/// Interactive Unicode multi-line text input state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiLineInput {
    pub lines: Vec<Vec<char>>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub selection_anchor: Option<(usize, usize)>,
}

impl Default for MultiLineInput {
    fn default() -> Self {
        Self {
            lines: vec![Vec::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            selection_anchor: None,
        }
    }
}

impl MultiLineInput {
    pub fn new(s: &str) -> Self {
        let lines: Vec<Vec<char>> = if s.is_empty() {
            vec![Vec::new()]
        } else {
            let l: Vec<Vec<char>> = s.split('\n').map(|line| line.chars().collect()).collect();
            if l.is_empty() {
                vec![Vec::new()]
            } else {
                l
            }
        };
        Self {
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            selection_anchor: None,
        }
    }

    pub fn as_string(&self) -> String {
        self.lines
            .iter()
            .map(|chars| chars.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn set_text(&mut self, s: &str) {
        *self = Self::new(s);
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn ensure_cursor_visible(&mut self, v_rows: usize) {
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + v_rows && v_rows > 0 {
            self.scroll_row = self.cursor_row.saturating_sub(v_rows - 1);
        }
    }

    pub fn move_up(&mut self) -> bool {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.lines[self.cursor_row].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
            true
        } else {
            false
        }
    }

    pub fn move_down(&mut self) -> bool {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let len = self.lines[self.cursor_row].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
            true
        } else {
            false
        }
    }

    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let (ar, ac) = self.selection_anchor?;
        let (cr, cc) = (self.cursor_row, self.cursor_col);
        if (ar, ac) == (cr, cc) {
            return None;
        }
        if (ar, ac) < (cr, cc) {
            Some(((ar, ac), (cr, cc)))
        } else {
            Some(((cr, cc), (ar, ac)))
        }
    }

    pub fn is_pos_selected(&self, row: usize, col: usize) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let pos = (row, col);
            pos >= start && pos < end
        } else {
            false
        }
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let (start_r, start_c) = start;
        let (end_r, end_c) = end;

        if start_r == end_r {
            if start_r < self.lines.len() {
                let line = &self.lines[start_r];
                let s = start_c.min(line.len());
                let e = end_c.min(line.len());
                return Some(line[s..e].iter().collect());
            }
            return None;
        }

        let mut result = Vec::new();
        for r in start_r..=end_r {
            if r >= self.lines.len() {
                break;
            }
            let line = &self.lines[r];
            if r == start_r {
                let s = start_c.min(line.len());
                result.push(line[s..].iter().collect::<String>());
            } else if r == end_r {
                let e = end_c.min(line.len());
                result.push(line[..e].iter().collect::<String>());
            } else {
                result.push(line.iter().collect::<String>());
            }
        }
        Some(result.join("\n"))
    }

    pub fn delete_selection(&mut self) -> bool {
        let (start, end) = match self.selection_range() {
            Some(range) => range,
            None => {
                self.selection_anchor = None;
                return false;
            }
        };
        let (start_r, start_c) = start;
        let (end_r, end_c) = end;

        if start_r == end_r {
            if start_r < self.lines.len() {
                let s = start_c.min(self.lines[start_r].len());
                let e = end_c.min(self.lines[start_r].len());
                self.lines[start_r].drain(s..e);
            }
        } else {
            let mut prefix: Vec<char> = if start_r < self.lines.len() {
                let s = start_c.min(self.lines[start_r].len());
                self.lines[start_r][..s].to_vec()
            } else {
                Vec::new()
            };
            let suffix: Vec<char> = if end_r < self.lines.len() {
                let e = end_c.min(self.lines[end_r].len());
                self.lines[end_r][e..].to_vec()
            } else {
                Vec::new()
            };
            prefix.extend(suffix);

            let drain_end = (end_r + 1).min(self.lines.len());
            if start_r < drain_end {
                self.lines.drain(start_r..drain_end);
            }
            self.lines.insert(start_r, prefix);
        }

        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }

        self.cursor_row = start_r.min(self.lines.len().saturating_sub(1));
        self.cursor_col = start_c.min(self.lines[self.cursor_row].len());
        self.selection_anchor = None;
        true
    }

    pub fn paste_text(&mut self, text: &str) {
        self.delete_selection();
        let paste_lines: Vec<&str> = text.split('\n').collect();
        if paste_lines.is_empty() {
            return;
        }

        if paste_lines.len() == 1 {
            let clean: Vec<char> = paste_lines[0].chars().filter(|c| *c != '\r').collect();
            if self.cursor_row >= self.lines.len() {
                self.lines.push(Vec::new());
                self.cursor_row = self.lines.len() - 1;
            }
            let len = self.lines[self.cursor_row].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
            for (idx, ch) in clean.into_iter().enumerate() {
                self.lines[self.cursor_row].insert(self.cursor_col + idx, ch);
            }
            self.cursor_col += text.chars().filter(|c| *c != '\r' && *c != '\n').count();
        } else {
            if self.cursor_row >= self.lines.len() {
                self.lines.push(Vec::new());
                self.cursor_row = self.lines.len() - 1;
            }
            let line_len = self.lines[self.cursor_row].len();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
            let remainder: Vec<char> = self.lines[self.cursor_row]
                .drain(self.cursor_col..)
                .collect();

            let first_clean: Vec<char> = paste_lines[0].chars().filter(|c| *c != '\r').collect();
            self.lines[self.cursor_row].extend(first_clean);

            for (i, p_line) in paste_lines[1..paste_lines.len() - 1].iter().enumerate() {
                let clean: Vec<char> = p_line.chars().filter(|c| *c != '\r').collect();
                self.lines.insert(self.cursor_row + 1 + i, clean);
            }

            let last_clean: Vec<char> = paste_lines
                .last()
                .unwrap()
                .chars()
                .filter(|c| *c != '\r')
                .collect();
            let new_col = last_clean.len();
            let mut final_line = last_clean;
            final_line.extend(remainder);
            self.lines
                .insert(self.cursor_row + paste_lines.len() - 1, final_line);

            self.cursor_row += paste_lines.len() - 1;
            self.cursor_col = new_col;
        }
        self.selection_anchor = None;
    }

    pub fn move_word_left(&mut self) {
        if self.cursor_row >= self.lines.len() {
            return;
        }
        let line = &self.lines[self.cursor_row];
        let mut col = self.cursor_col.min(line.len());
        while col > 0 && line[col - 1].is_whitespace() {
            col -= 1;
        }
        while col > 0 && !line[col - 1].is_whitespace() {
            col -= 1;
        }
        self.cursor_col = col;
    }

    pub fn move_word_right(&mut self) {
        if self.cursor_row >= self.lines.len() {
            return;
        }
        let line = &self.lines[self.cursor_row];
        let mut col = self.cursor_col;
        while col < line.len() && !line[col].is_whitespace() {
            col += 1;
        }
        while col < line.len() && line[col].is_whitespace() {
            col += 1;
        }
        self.cursor_col = col;
    }

    pub fn handle_key_with_modifiers(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let is_ctrl = modifiers.contains(KeyModifiers::CONTROL);
        let is_shift = modifiers.contains(KeyModifiers::SHIFT);

        if is_ctrl {
            match code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.selection_anchor = Some((0, 0));
                    self.cursor_row = self.lines.len().saturating_sub(1);
                    self.cursor_col = self.lines.get(self.cursor_row).map_or(0, |l| l.len());
                    return true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some(snippet) = self.get_selected_text() {
                        bashmenu_tui::clipboard::set_clipboard_text(&snippet);
                    }
                    return true;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some(snippet) = self.get_selected_text() {
                        bashmenu_tui::clipboard::set_clipboard_text(&snippet);
                        self.delete_selection();
                    }
                    return true;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    if let Some(text) = bashmenu_tui::clipboard::get_clipboard_text() {
                        self.paste_text(&text);
                    }
                    return true;
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
                    return true;
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
                    return true;
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
                    return true;
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
                    self.cursor_col = self.lines.get(self.cursor_row).map_or(0, |l| l.len());
                    return true;
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
            KeyCode::Char(c) => {
                self.delete_selection();
                if self.cursor_row >= self.lines.len() {
                    self.lines.push(Vec::new());
                    self.cursor_row = self.lines.len() - 1;
                }
                let line_len = self.lines[self.cursor_row].len();
                if self.cursor_col > line_len {
                    self.cursor_col = line_len;
                }
                self.lines[self.cursor_row].insert(self.cursor_col, c);
                self.cursor_col += 1;
                true
            }
            KeyCode::Enter => {
                self.delete_selection();
                if self.cursor_row >= self.lines.len() {
                    self.lines.push(Vec::new());
                    self.cursor_row = self.lines.len() - 1;
                }
                let line_len = self.lines[self.cursor_row].len();
                if self.cursor_col > line_len {
                    self.cursor_col = line_len;
                }
                let remainder: Vec<char> = self.lines[self.cursor_row]
                    .drain(self.cursor_col..)
                    .collect();
                self.lines.insert(self.cursor_row + 1, remainder);
                self.cursor_row += 1;
                self.cursor_col = 0;
                true
            }
            KeyCode::Backspace => {
                if self.delete_selection() {
                    true
                } else if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                    if self.cursor_row < self.lines.len()
                        && self.cursor_col < self.lines[self.cursor_row].len()
                    {
                        self.lines[self.cursor_row].remove(self.cursor_col);
                    }
                    true
                } else if self.cursor_row > 0 {
                    let current = self.lines.remove(self.cursor_row);
                    self.cursor_row -= 1;
                    self.cursor_col = self.lines[self.cursor_row].len();
                    self.lines[self.cursor_row].extend(current);
                    true
                } else {
                    false
                }
            }
            KeyCode::Delete => {
                if self.delete_selection() {
                    true
                } else if self.cursor_row < self.lines.len()
                    && self.cursor_col < self.lines[self.cursor_row].len()
                {
                    self.lines[self.cursor_row].remove(self.cursor_col);
                    true
                } else if self.cursor_row + 1 < self.lines.len() {
                    let next = self.lines.remove(self.cursor_row + 1);
                    self.lines[self.cursor_row].extend(next);
                    true
                } else {
                    false
                }
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                    true
                } else if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.cursor_col = self.lines[self.cursor_row].len();
                    true
                } else {
                    false
                }
            }
            KeyCode::Right => {
                if self.cursor_row < self.lines.len()
                    && self.cursor_col < self.lines[self.cursor_row].len()
                {
                    self.cursor_col += 1;
                    true
                } else if self.cursor_row + 1 < self.lines.len() {
                    self.cursor_row += 1;
                    self.cursor_col = 0;
                    true
                } else {
                    false
                }
            }
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => {
                self.cursor_col = 0;
                true
            }
            KeyCode::End => {
                if self.cursor_row < self.lines.len() {
                    self.cursor_col = self.lines[self.cursor_row].len();
                }
                true
            }
            KeyCode::PageUp => {
                self.cursor_row = self.cursor_row.saturating_sub(5);
                let len = self.lines.get(self.cursor_row).map_or(0, |l| l.len());
                if self.cursor_col > len {
                    self.cursor_col = len;
                }
                true
            }
            KeyCode::PageDown => {
                self.cursor_row = (self.cursor_row + 5).min(self.lines.len().saturating_sub(1));
                let len = self.lines.get(self.cursor_row).map_or(0, |l| l.len());
                if self.cursor_col > len {
                    self.cursor_col = len;
                }
                true
            }
            _ => false,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        self.handle_key_with_modifiers(code, KeyModifiers::NONE)
    }
}

/// Target destination for file browser dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseTarget {
    Action,
    Template,
    Target,
    StartDir,
    OnYes,
    OnNo,
}

/// Result returned from `ItemEditModal::handle_key` or `handle_mouse`.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemEditResult {
    Continue,
    Save(Box<MenuItem>),
    Cancel,
    ChangeType,
    ShowHelp,
    ShowAscii,
    ShowGlyphs,
    ShowPlaceholders,
    Browse(BrowseTarget),
}

/// Dynamic focusable control types within the editor form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusControl {
    ChangeTypeBtn,
    // Divider fields
    CharInput,
    LengthInput,
    // Standard fields
    TitleInput,
    IconInput,
    ActionInput,
    ActionBrowseBtn,
    OnYesInput,
    OnYesBrowseBtn,
    OnNoInput,
    OnNoBrowseBtn,
    PromptInput,
    RadioOptionsInput,
    TemplateInput,
    TemplateBrowseBtn,
    TargetInput,
    TargetBrowseBtn,
    BlockIdInput,
    StartDirInput,
    StartDirBrowseBtn,
    TabstopInput,
    ExecModeSelector,
    WindowPercentInput,
    AltBufferChk,
    NoFormattingChk,
    MaskedChk,
    RefreshChk,
    ShowWhitespaceChk,
    ExternalChk,
    // Dialog action buttons
    SaveBtn,
    CancelBtn,
}

/// Modal dialog for editing all schema properties of a menu item.
#[derive(Debug, Clone)]
pub struct ItemEditModal {
    pub item: MenuItem,
    pub current_type: String,
    pub original_type: String,

    // Divider fields
    pub char_input: TextInput,
    pub length_input: TextInput,

    // Standard fields
    pub title_input: TextInput,
    pub icon_input: TextInput,
    pub action_input: TextInput,
    pub prompt_input: TextInput,
    pub on_yes_input: TextInput,
    pub on_no_input: TextInput,
    pub radio_options_input: MultiLineInput,
    pub template_input: TextInput,
    pub target_input: TextInput,
    pub block_id_input: TextInput,
    pub start_dir_input: TextInput,
    pub tabstop_input: TextInput,

    // Execution mode: 0 = Streaming, 1 = Interactive, 2 = Console
    pub exec_mode: usize,
    pub window_percent_input: TextInput,

    // Boolean checkboxes
    pub alt_buffer: bool,
    pub no_formatting: bool,
    pub masked: bool,
    pub refresh: bool,
    pub show_whitespace: bool,
    pub external: bool,

    // Focus and scroll state
    pub active_control_idx: usize,
    pub scroll_offset: usize,
}

impl ItemEditModal {
    pub fn new(item: &MenuItem) -> Self {
        let current_type = Self::detect_item_type(item);
        let original_type = current_type.clone();

        let char_val = item
            .extra
            .get("char")
            .and_then(|v| v.as_str())
            .unwrap_or("{ascii:196}");
        let length_val = item
            .extra
            .get("length")
            .and_then(|v| v.as_str())
            .unwrap_or("{window_width}");

        let title_val = item.label.as_str();
        let icon_val = item.icon.as_deref().unwrap_or_default();

        let action_val = item
            .action
            .as_deref()
            .or(item.command.as_deref())
            .or(item.script.as_deref())
            .or(item.file.as_deref())
            .or_else(|| item.extra.get("key").and_then(|v| v.as_str()))
            .or_else(|| item.extra.get("python").and_then(|v| v.as_str()))
            .unwrap_or_default();

        let prompt_val = item
            .prompt
            .as_deref()
            .or_else(|| item.extra.get("message").and_then(|v| v.as_str()))
            .unwrap_or_default();

        let on_yes_val = item.on_yes().unwrap_or_default();
        let on_no_val = item.on_no().unwrap_or_default();

        let radio_lines =
            if let Some(opts) = item.extra.get("options").and_then(|v| v.as_sequence()) {
                opts.iter()
                    .filter_map(|o| {
                        if let Some(map) = o.as_mapping() {
                            let lbl = map
                                .get(serde_yml::Value::String("label".to_string()))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let val = map
                                .get(serde_yml::Value::String("value".to_string()))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if lbl.is_empty() || lbl == val {
                                Some(val.to_string())
                            } else {
                                Some(format!("{} = {}", lbl, val))
                            }
                        } else {
                            o.as_str().map(|s| s.to_string())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };

        let template_val = item
            .extra
            .get("template")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let target_val = item
            .extra
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let block_id_val = item
            .extra
            .get("block_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let start_dir_val = item.start_dir.as_deref().unwrap_or_default();
        let tabstop_val = item
            .tabstop
            .map(|t| t.to_string())
            .unwrap_or_else(|| "4".to_string());

        let is_stream = item
            .extra
            .get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || item.interactive == Some(false);
        let is_interactive = item.extra.get("exec_mode").and_then(|v| v.as_str())
            == Some("interactive")
            || item
                .extra
                .get("floating")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        let exec_mode = if is_stream {
            0
        } else if is_interactive {
            1
        } else {
            2
        };

        let window_percent_val = item
            .extra
            .get("window_percent")
            .and_then(|v| {
                if let Some(n) = v.as_u64() {
                    Some(n.to_string())
                } else {
                    v.as_str().map(|s| s.to_string())
                }
            })
            .unwrap_or_else(|| "65".to_string());

        let alt_buffer = item.alt_buffer.unwrap_or(true);
        let no_formatting = item
            .extra
            .get("no_formatting")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let masked = item
            .extra
            .get("masked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let refresh = item
            .extra
            .get("refresh")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let show_whitespace = item
            .extra
            .get("show_whitespace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let external = item
            .extra
            .get("external")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Self {
            item: item.clone(),
            current_type,
            original_type,
            char_input: TextInput::new(char_val),
            length_input: TextInput::new(length_val),
            title_input: TextInput::new(title_val),
            icon_input: TextInput::new(icon_val),
            action_input: TextInput::new(action_val),
            prompt_input: TextInput::new(prompt_val),
            on_yes_input: TextInput::new(on_yes_val),
            on_no_input: TextInput::new(on_no_val),
            radio_options_input: MultiLineInput::new(&radio_lines),
            template_input: TextInput::new(template_val),
            target_input: TextInput::new(target_val),
            block_id_input: TextInput::new(block_id_val),
            start_dir_input: TextInput::new(start_dir_val),
            tabstop_input: TextInput::new(&tabstop_val),
            exec_mode,
            window_percent_input: TextInput::new(&window_percent_val),
            alt_buffer,
            no_formatting,
            masked,
            refresh,
            show_whitespace,
            external,
            active_control_idx: 1, // Start on the first field below Change Type
            scroll_offset: 0,
        }
    }

    /// Detect item type string from menu item
    pub fn detect_item_type(item: &MenuItem) -> String {
        if item.is_divider() {
            return "{divider}".to_string();
        }
        match &item.r#type {
            MenuItemType::Submenu => "submenu".to_string(),
            MenuItemType::Command => "command".to_string(),
            MenuItemType::Script => "script".to_string(),
            MenuItemType::Config => "config".to_string(),
            MenuItemType::Toggle => "toggle".to_string(),
            MenuItemType::Radio => "radio".to_string(),
            MenuItemType::Editor => "editor".to_string(),
            MenuItemType::Notebook => "notebook".to_string(),
            MenuItemType::Confirm => "confirm".to_string(),
            MenuItemType::Python => "python".to_string(),
            MenuItemType::Exit => "exit".to_string(),
            MenuItemType::Back => "back".to_string(),
            MenuItemType::ThemeSelector => "theme_selector".to_string(),
            MenuItemType::Other(s) => s.clone(),
            _ => {
                if item.submenu.is_some() {
                    "submenu".to_string()
                } else if item.on_yes.is_some() || item.on_no.is_some() {
                    "confirm".to_string()
                } else if item.command.is_some() {
                    "command".to_string()
                } else if item.script.is_some() {
                    "script".to_string()
                } else if item.file.is_some() {
                    "editor".to_string()
                } else if item.extra.contains_key("key") {
                    "config".to_string()
                } else {
                    "command".to_string()
                }
            }
        }
    }

    pub fn is_divider(&self) -> bool {
        self.current_type == "{divider}" || self.current_type == "divider"
    }

    pub fn is_confirm(&self) -> bool {
        self.current_type == "confirm"
    }

    pub fn is_radio(&self) -> bool {
        self.current_type == "radio" || self.current_type == "config_radio"
    }

    pub fn is_nav_type(&self) -> bool {
        matches!(
            self.current_type.as_str(),
            "submenu" | "notebook" | "theme_selector" | "back" | "exit"
        )
    }

    pub fn is_executable(&self) -> bool {
        matches!(
            self.current_type.as_str(),
            "command" | "script" | "confirm" | "python"
        )
    }

    pub fn shows_start_dir(&self) -> bool {
        matches!(self.current_type.as_str(), "command" | "script" | "editor")
    }

    pub fn shows_prompt(&self) -> bool {
        matches!(
            self.current_type.as_str(),
            "command" | "script" | "config" | "toggle" | "radio" | "confirm" | "message" | "python"
        )
    }

    pub fn shows_action(&self) -> bool {
        !self.is_divider()
            && !self.is_confirm()
            && self.current_type != "inject_block"
            && !matches!(
                self.current_type.as_str(),
                "submenu" | "theme_selector" | "back" | "exit"
            )
    }

    pub fn shows_action_browse(&self) -> bool {
        matches!(
            self.current_type.as_str(),
            "command" | "script" | "editor" | "notebook" | "python"
        )
    }

    /// Dynamically lists active focusable controls based on item type and mode
    pub fn focus_controls(&self) -> Vec<FocusControl> {
        let mut controls = vec![FocusControl::ChangeTypeBtn];

        if self.is_divider() {
            controls.push(FocusControl::CharInput);
            controls.push(FocusControl::LengthInput);
        } else {
            controls.push(FocusControl::TitleInput);
            controls.push(FocusControl::IconInput);

            if self.is_confirm() {
                controls.push(FocusControl::PromptInput);
                controls.push(FocusControl::OnYesInput);
                controls.push(FocusControl::OnYesBrowseBtn);
                controls.push(FocusControl::OnNoInput);
                controls.push(FocusControl::OnNoBrowseBtn);
            } else if self.shows_action() {
                controls.push(FocusControl::ActionInput);
                if self.shows_action_browse() {
                    controls.push(FocusControl::ActionBrowseBtn);
                }
                if self.shows_prompt() {
                    controls.push(FocusControl::PromptInput);
                }
            } else if self.shows_prompt() {
                controls.push(FocusControl::PromptInput);
            }

            if self.is_radio() {
                controls.push(FocusControl::RadioOptionsInput);
            }

            if self.current_type == "inject_block" {
                controls.push(FocusControl::TemplateInput);
                controls.push(FocusControl::TemplateBrowseBtn);
                controls.push(FocusControl::TargetInput);
                controls.push(FocusControl::TargetBrowseBtn);
                controls.push(FocusControl::BlockIdInput);
            }

            if self.shows_start_dir() {
                controls.push(FocusControl::StartDirInput);
                controls.push(FocusControl::StartDirBrowseBtn);
            }

            if self.current_type == "editor" {
                controls.push(FocusControl::TabstopInput);
            }

            if self.is_executable() {
                controls.push(FocusControl::ExecModeSelector);
                if self.exec_mode == 0 || self.exec_mode == 1 {
                    controls.push(FocusControl::WindowPercentInput);
                }
                if self.exec_mode == 2 {
                    controls.push(FocusControl::AltBufferChk);
                }
            }

            if self.current_type == "editor" {
                controls.push(FocusControl::NoFormattingChk);
                controls.push(FocusControl::ShowWhitespaceChk);
            }

            if matches!(self.current_type.as_str(), "command" | "script") {
                controls.push(FocusControl::MaskedChk);
            }

            if !self.is_nav_type() {
                controls.push(FocusControl::RefreshChk);
                controls.push(FocusControl::ExternalChk);
            }
        }

        controls.push(FocusControl::SaveBtn);
        controls.push(FocusControl::CancelBtn);
        controls
    }

    pub fn current_control(&self) -> FocusControl {
        let list = self.focus_controls();
        if self.active_control_idx < list.len() {
            list[self.active_control_idx]
        } else {
            FocusControl::SaveBtn
        }
    }

    pub fn apply_type_change(&mut self, new_type: &str) {
        self.current_type = new_type.to_string();
        if self.is_divider() {
            self.item.r#type = MenuItemType::Divider;
            self.item.label = "{divider}".to_string();
        } else {
            self.item.r#type = match new_type {
                "submenu" => MenuItemType::Submenu,
                "command" => MenuItemType::Command,
                "script" => MenuItemType::Script,
                "config" => MenuItemType::Config,
                "toggle" => MenuItemType::Toggle,
                "radio" => MenuItemType::Radio,
                "editor" => MenuItemType::Editor,
                "notebook" => MenuItemType::Notebook,
                "confirm" => MenuItemType::Confirm,
                "python" => MenuItemType::Python,
                "exit" => MenuItemType::Exit,
                "back" => MenuItemType::Back,
                "theme_selector" => MenuItemType::ThemeSelector,
                other => MenuItemType::Other(other.to_string()),
            };
        }
        if new_type == "confirm"
            && self.on_yes_input.as_string().is_empty()
            && !self.action_input.as_string().is_empty()
        {
            self.on_yes_input.set_text(&self.action_input.as_string());
        }
        let list = self.focus_controls();
        if self.active_control_idx >= list.len() {
            self.active_control_idx = 1;
        }
    }

    pub fn set_browse_result(&mut self, target: BrowseTarget, path: &str) {
        match target {
            BrowseTarget::Action => self.action_input.set_text(path),
            BrowseTarget::Template => self.template_input.set_text(path),
            BrowseTarget::Target => self.target_input.set_text(path),
            BrowseTarget::StartDir => self.start_dir_input.set_text(path),
            BrowseTarget::OnYes => self.on_yes_input.set_text(path),
            BrowseTarget::OnNo => self.on_no_input.set_text(path),
        }
    }

    pub fn get_action_label_text(&self) -> &'static str {
        match self.current_type.as_str() {
            "command" => "Shell Command (command):",
            "script" => "Script Path (script):",
            "editor" | "notebook" => "File to Edit (file):",
            "config" | "toggle" | "radio" => "Configuration Key (key):",
            "python" => "Python Expression / Routine (python):",
            "submenu" => "Submenu Action / Submenu Title:",
            _ => "Action / Command / Script / File / Key:",
        }
    }

    pub fn get_type_display_str(&self) -> String {
        let badge = match self.current_type.as_str() {
            "{divider}" | "divider" => "[DIV]",
            "submenu" => "[DIR]",
            "command" => "[CMD]",
            "script" => "[SCR]",
            "config" => "[CFG]",
            "toggle" => "[TGL]",
            "radio" => "[RAD]",
            "editor" => "[EDT]",
            "notebook" => "[NBK]",
            "confirm" => "[CNF]",
            "message" => "[MSG]",
            "python" => "[PY ]",
            "inject_block" => "[INJ]",
            "theme_selector" => "[THM]",
            "back" => "[BCK]",
            "exit" => "[EXT]",
            _ => "[???]",
        };
        let desc = match self.current_type.as_str() {
            "{divider}" | "divider" => "Visual Divider Line",
            "submenu" => "Submenu Folder",
            "command" => "Shell Command",
            "script" => "Script File",
            "config" => "Config Setting",
            "toggle" => "Settings Toggle",
            "radio" => "Radio Setting",
            "editor" => "File Editor",
            "notebook" => "Notebook Menu",
            "confirm" => "Confirm Dialog",
            "message" => "Message Popup",
            "python" => "Python Routine",
            "inject_block" => "Inject Block",
            "theme_selector" => "Theme Selector",
            "back" => "Back Button",
            "exit" => "Exit Button",
            other => other,
        };
        format!("{} {}", badge, desc)
    }

    /// Handles keyboard events
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ItemEditResult {
        let ctrl = self.current_control();

        // Check if focused text input or multiline input intercepts Ctrl+A / Ctrl+C / Ctrl+X / Ctrl+V
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('a')
                | KeyCode::Char('A')
                | KeyCode::Char('c')
                | KeyCode::Char('C')
                | KeyCode::Char('x')
                | KeyCode::Char('X')
                | KeyCode::Char('v')
                | KeyCode::Char('V') => {
                    if ctrl == FocusControl::RadioOptionsInput {
                        if self
                            .radio_options_input
                            .handle_key_with_modifiers(code, modifiers)
                        {
                            self.radio_options_input.ensure_cursor_visible(5);
                            return ItemEditResult::Continue;
                        }
                    } else {
                        let input_ref = match ctrl {
                            FocusControl::CharInput => Some(&mut self.char_input),
                            FocusControl::LengthInput => Some(&mut self.length_input),
                            FocusControl::TitleInput => Some(&mut self.title_input),
                            FocusControl::IconInput => Some(&mut self.icon_input),
                            FocusControl::ActionInput => Some(&mut self.action_input),
                            FocusControl::PromptInput => Some(&mut self.prompt_input),
                            FocusControl::OnYesInput => Some(&mut self.on_yes_input),
                            FocusControl::OnNoInput => Some(&mut self.on_no_input),
                            FocusControl::TemplateInput => Some(&mut self.template_input),
                            FocusControl::TargetInput => Some(&mut self.target_input),
                            FocusControl::BlockIdInput => Some(&mut self.block_id_input),
                            FocusControl::StartDirInput => Some(&mut self.start_dir_input),
                            FocusControl::TabstopInput => Some(&mut self.tabstop_input),
                            FocusControl::WindowPercentInput => {
                                Some(&mut self.window_percent_input)
                            }
                            _ => None,
                        };
                        if let Some(inp) = input_ref {
                            if inp.handle_key_with_modifiers(code, modifiers) {
                                return ItemEditResult::Continue;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Global modal hotkeys
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('s') => {
                    return ItemEditResult::Save(Box::new(self.build_updated_item()))
                }
                KeyCode::Char('t') => return ItemEditResult::ChangeType,
                KeyCode::Char('a') => return ItemEditResult::ShowAscii,
                KeyCode::Char('n') => return ItemEditResult::ShowGlyphs,
                KeyCode::Char('p') => return ItemEditResult::ShowPlaceholders,
                _ => {}
            }
        }

        match code {
            KeyCode::F(1) => return ItemEditResult::ShowHelp,
            KeyCode::F(2) => return ItemEditResult::Save(Box::new(self.build_updated_item())),
            KeyCode::F(3) => return ItemEditResult::ShowAscii,
            KeyCode::F(4) => return ItemEditResult::ShowGlyphs,
            KeyCode::F(5) => return ItemEditResult::ShowPlaceholders,
            KeyCode::Esc => return ItemEditResult::Cancel,
            _ => {}
        }

        // Check if currently focused on a button or selector
        match ctrl {
            FocusControl::ChangeTypeBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::ChangeType;
                }
            }
            FocusControl::SaveBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Save(Box::new(self.build_updated_item()));
                }
            }
            FocusControl::CancelBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Cancel;
                }
            }
            FocusControl::ActionBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::Action);
                }
            }
            FocusControl::OnYesBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::OnYes);
                }
            }
            FocusControl::OnNoBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::OnNo);
                }
            }
            FocusControl::TemplateBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::Template);
                }
            }
            FocusControl::TargetBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::Target);
                }
            }
            FocusControl::StartDirBrowseBtn => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return ItemEditResult::Browse(BrowseTarget::StartDir);
                }
            }
            FocusControl::ExecModeSelector => match code {
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('k') => {
                    self.exec_mode = (self.exec_mode + 2) % 3;
                    return ItemEditResult::Continue;
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('j') | KeyCode::Char(' ') => {
                    self.exec_mode = (self.exec_mode + 1) % 3;
                    return ItemEditResult::Continue;
                }
                KeyCode::Char('1') => {
                    self.exec_mode = 0;
                    return ItemEditResult::Continue;
                }
                KeyCode::Char('2') => {
                    self.exec_mode = 1;
                    return ItemEditResult::Continue;
                }
                KeyCode::Char('3') => {
                    self.exec_mode = 2;
                    return ItemEditResult::Continue;
                }
                _ => {}
            },
            FocusControl::AltBufferChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.alt_buffer = !self.alt_buffer;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::NoFormattingChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.no_formatting = !self.no_formatting;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::MaskedChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.masked = !self.masked;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::RefreshChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.refresh = !self.refresh;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::ShowWhitespaceChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.show_whitespace = !self.show_whitespace;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::ExternalChk => {
                if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.external = !self.external;
                    return ItemEditResult::Continue;
                }
            }
            FocusControl::RadioOptionsInput if code == KeyCode::Enter => {
                self.radio_options_input
                    .handle_key_with_modifiers(KeyCode::Enter, modifiers);
                self.radio_options_input.ensure_cursor_visible(5);
                return ItemEditResult::Continue;
            }
            _ => {}
        }

        // Shift+directional navigation inside RadioOptionsInput
        if ctrl == FocusControl::RadioOptionsInput
            && modifiers.contains(KeyModifiers::SHIFT)
            && matches!(
                code,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            )
            && self
                .radio_options_input
                .handle_key_with_modifiers(code, modifiers)
        {
            self.radio_options_input.ensure_cursor_visible(5);
            return ItemEditResult::Continue;
        }

        // Field navigation
        let controls = self.focus_controls();
        match code {
            KeyCode::Tab => {
                self.active_control_idx = (self.active_control_idx + 1) % controls.len();
                return ItemEditResult::Continue;
            }
            KeyCode::BackTab => {
                if self.active_control_idx == 0 {
                    self.active_control_idx = controls.len() - 1;
                } else {
                    self.active_control_idx -= 1;
                }
                return ItemEditResult::Continue;
            }
            KeyCode::Down if !modifiers.contains(KeyModifiers::SHIFT) => {
                if ctrl == FocusControl::RadioOptionsInput && self.radio_options_input.move_down() {
                    self.radio_options_input.ensure_cursor_visible(5);
                    return ItemEditResult::Continue;
                }
                self.active_control_idx = (self.active_control_idx + 1) % controls.len();
                return ItemEditResult::Continue;
            }
            KeyCode::Up if !modifiers.contains(KeyModifiers::SHIFT) => {
                if ctrl == FocusControl::RadioOptionsInput && self.radio_options_input.move_up() {
                    self.radio_options_input.ensure_cursor_visible(5);
                    return ItemEditResult::Continue;
                }
                if self.active_control_idx == 0 {
                    self.active_control_idx = controls.len() - 1;
                } else {
                    self.active_control_idx -= 1;
                }
                return ItemEditResult::Continue;
            }
            _ => {}
        }

        // Multi-line radio options editing
        if ctrl == FocusControl::RadioOptionsInput
            && self
                .radio_options_input
                .handle_key_with_modifiers(code, modifiers)
        {
            self.radio_options_input.ensure_cursor_visible(5);
            return ItemEditResult::Continue;
        }

        // Text input editing
        let input_ref = match ctrl {
            FocusControl::CharInput => Some(&mut self.char_input),
            FocusControl::LengthInput => Some(&mut self.length_input),
            FocusControl::TitleInput => Some(&mut self.title_input),
            FocusControl::IconInput => Some(&mut self.icon_input),
            FocusControl::ActionInput => Some(&mut self.action_input),
            FocusControl::PromptInput => Some(&mut self.prompt_input),
            FocusControl::OnYesInput => Some(&mut self.on_yes_input),
            FocusControl::OnNoInput => Some(&mut self.on_no_input),
            FocusControl::TemplateInput => Some(&mut self.template_input),
            FocusControl::TargetInput => Some(&mut self.target_input),
            FocusControl::BlockIdInput => Some(&mut self.block_id_input),
            FocusControl::StartDirInput => Some(&mut self.start_dir_input),
            FocusControl::TabstopInput => Some(&mut self.tabstop_input),
            FocusControl::WindowPercentInput => Some(&mut self.window_percent_input),
            _ => None,
        };

        if let Some(inp) = input_ref {
            if inp.handle_key_with_modifiers(code, modifiers) {
                return ItemEditResult::Continue;
            }
        }

        ItemEditResult::Continue
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> ItemEditResult {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = compute_modal_inner(area, 86, 32);

            if is_close_button_clicked(inner, mouse.row, mouse.column) {
                return ItemEditResult::Cancel;
            }

            // Click inside buttons row (near bottom)
            let btn_y = inner.y + inner.height.saturating_sub(4);
            if mouse.row == btn_y {
                let mid_x = inner.x + inner.width / 2;
                if mouse.column >= inner.x + 4 && mouse.column < mid_x {
                    return ItemEditResult::Save(Box::new(self.build_updated_item()));
                } else if mouse.column >= mid_x && mouse.column < inner.x + inner.width - 4 {
                    return ItemEditResult::Cancel;
                }
            }

            // Click footer shortcuts
            let footer_y1 = inner.y + inner.height.saturating_sub(2);
            let footer_y2 = inner.y + inner.height.saturating_sub(1);
            if mouse.row == footer_y1 {
                let w = inner.width / 4;
                if mouse.column < inner.x + w {
                    return ItemEditResult::ShowHelp;
                } else if mouse.column < inner.x + w * 2 {
                    return ItemEditResult::Save(Box::new(self.build_updated_item()));
                } else if mouse.column < inner.x + w * 3 {
                    return ItemEditResult::ChangeType;
                } else {
                    return ItemEditResult::ShowAscii;
                }
            } else if mouse.row == footer_y2 {
                let w = inner.width / 3;
                if mouse.column < inner.x + w {
                    return ItemEditResult::ShowGlyphs;
                } else if mouse.column < inner.x + w * 2 {
                    return ItemEditResult::ShowPlaceholders;
                } else {
                    return ItemEditResult::Cancel;
                }
            }
        } else if mouse.kind == MouseEventKind::ScrollDown {
            self.scroll_offset = self.scroll_offset.saturating_add(2);
        } else if mouse.kind == MouseEventKind::ScrollUp {
            self.scroll_offset = self.scroll_offset.saturating_sub(2);
        }

        ItemEditResult::Continue
    }

    /// Builds the updated MenuItem matching Python `perform_save()`
    pub fn build_updated_item(&self) -> MenuItem {
        if self.is_divider() {
            let mut item = MenuItem::divider();
            let c = self.char_input.as_string();
            let l = self.length_input.as_string();
            if !c.is_empty() && c != "{ascii:196}" {
                item.extra
                    .insert("char".to_string(), serde_yml::Value::String(c));
            }
            if !l.is_empty() && l != "{window_width}" {
                item.extra
                    .insert("length".to_string(), serde_yml::Value::String(l));
            }
            return item;
        }

        let mut item = self.item.clone();
        item.extra.remove("char");
        item.extra.remove("length");

        let title = self.title_input.as_string().trim().to_string();
        let icon = self.icon_input.as_string().trim().to_string();
        let action = self.action_input.as_string().trim().to_string();
        let prompt = self.prompt_input.as_string().trim().to_string();
        let on_yes = self.on_yes_input.as_string().trim().to_string();
        let on_no = self.on_no_input.as_string().trim().to_string();
        let template = self.template_input.as_string().trim().to_string();
        let target = self.target_input.as_string().trim().to_string();
        let block_id = self.block_id_input.as_string().trim().to_string();
        let start_dir = self.start_dir_input.as_string().trim().to_string();
        let tabstop = self.tabstop_input.as_string().trim().to_string();

        item.label = title;
        item.icon = if icon.is_empty() { None } else { Some(icon) };

        if self.is_confirm() {
            item.r#type = MenuItemType::Confirm;
            item.on_yes = if on_yes.is_empty() {
                None
            } else {
                Some(on_yes.as_str().into())
            };
            item.on_no = if on_no.is_empty() {
                None
            } else {
                Some(on_no.as_str().into())
            };
            item.extra.remove("on_yes");
            item.extra.remove("on_no");
            item.action = None;
            item.command = None;
            item.script = None;
            item.file = None;
        } else {
            item.on_yes = None;
            item.on_no = None;
            item.extra.remove("on_yes");
            item.extra.remove("on_no");

            // Normalize payload mapping based on current type
            match self.current_type.as_str() {
                "command" => {
                    item.command = if action.is_empty() {
                        None
                    } else {
                        Some(action.clone())
                    };
                    item.action = item.command.clone();
                    item.script = None;
                    item.file = None;
                }
                "script" => {
                    item.script = if action.is_empty() {
                        None
                    } else {
                        Some(action.clone())
                    };
                    item.action = item.script.clone();
                    item.command = None;
                    item.file = None;
                }
                "editor" | "notebook" => {
                    item.file = if action.is_empty() {
                        None
                    } else {
                        Some(action.clone())
                    };
                    item.action = item.file.clone();
                    item.command = None;
                    item.script = None;
                }
                "config" | "toggle" | "radio" => {
                    if !action.is_empty() {
                        item.extra
                            .insert("key".to_string(), serde_yml::Value::String(action.clone()));
                    } else {
                        item.extra.remove("key");
                    }
                    item.action = if action.is_empty() {
                        None
                    } else {
                        Some(action)
                    };
                }
                "python" => {
                    if !action.is_empty() {
                        item.extra.insert(
                            "python".to_string(),
                            serde_yml::Value::String(action.clone()),
                        );
                    } else {
                        item.extra.remove("python");
                    }
                    item.action = if action.is_empty() {
                        None
                    } else {
                        Some(action)
                    };
                }
                "submenu" => {
                    if item.submenu.is_none() {
                        item.submenu = Some(bashmenu_core::Submenu {
                            title: item.label.clone(),
                            items: Vec::new(),
                        });
                    }
                }
                _ => {
                    item.action = if action.is_empty() {
                        None
                    } else {
                        Some(action)
                    };
                }
            }
        }

        item.prompt = if prompt.is_empty() {
            None
        } else {
            Some(prompt)
        };

        if !template.is_empty() {
            item.extra
                .insert("template".to_string(), serde_yml::Value::String(template));
        } else {
            item.extra.remove("template");
        }

        if !target.is_empty() {
            item.extra
                .insert("target".to_string(), serde_yml::Value::String(target));
        } else {
            item.extra.remove("target");
        }

        if !block_id.is_empty() {
            item.extra
                .insert("block_id".to_string(), serde_yml::Value::String(block_id));
        } else {
            item.extra.remove("block_id");
        }

        item.start_dir = if start_dir.is_empty() {
            None
        } else {
            Some(start_dir)
        };
        item.tabstop = tabstop.parse().ok();

        // Radio options parsing
        if self.is_radio() {
            let raw_text = self.radio_options_input.as_string();
            let mut opts = Vec::new();
            for line in raw_text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let mut map = serde_yml::Mapping::new();
                if let Some((lbl, val)) = line.split_once('=') {
                    map.insert(
                        serde_yml::Value::String("label".to_string()),
                        serde_yml::Value::String(lbl.trim().to_string()),
                    );
                    map.insert(
                        serde_yml::Value::String("value".to_string()),
                        serde_yml::Value::String(val.trim().to_string()),
                    );
                } else if let Some((lbl, val)) = line.split_once(':') {
                    map.insert(
                        serde_yml::Value::String("label".to_string()),
                        serde_yml::Value::String(lbl.trim().to_string()),
                    );
                    map.insert(
                        serde_yml::Value::String("value".to_string()),
                        serde_yml::Value::String(val.trim().to_string()),
                    );
                } else {
                    map.insert(
                        serde_yml::Value::String("label".to_string()),
                        serde_yml::Value::String(line.to_string()),
                    );
                    map.insert(
                        serde_yml::Value::String("value".to_string()),
                        serde_yml::Value::String(line.to_string()),
                    );
                }
                opts.push(serde_yml::Value::Mapping(map));
            }
            if !opts.is_empty() {
                item.extra
                    .insert("options".to_string(), serde_yml::Value::Sequence(opts));
            } else {
                item.extra.remove("options");
            }
        }

        // Execution mode
        match self.exec_mode {
            0 => {
                item.extra
                    .insert("stream".to_string(), serde_yml::Value::Bool(true));
                item.extra.insert(
                    "exec_mode".to_string(),
                    serde_yml::Value::String("streaming".to_string()),
                );
                item.extra.remove("floating");
                item.interactive = Some(false);
                item.quiet = Some(true);
                let pct = self
                    .window_percent_input
                    .as_string()
                    .trim()
                    .trim_end_matches('%')
                    .parse::<u64>()
                    .unwrap_or(65)
                    .clamp(20, 100);
                item.extra.insert(
                    "window_percent".to_string(),
                    serde_yml::Value::Number(pct.into()),
                );
            }
            1 => {
                item.extra
                    .insert("stream".to_string(), serde_yml::Value::Bool(false));
                item.extra.insert(
                    "exec_mode".to_string(),
                    serde_yml::Value::String("interactive".to_string()),
                );
                item.extra
                    .insert("floating".to_string(), serde_yml::Value::Bool(true));
                item.interactive = Some(true);
                item.quiet = Some(true);
                let pct = self
                    .window_percent_input
                    .as_string()
                    .trim()
                    .trim_end_matches('%')
                    .parse::<u64>()
                    .unwrap_or(65)
                    .clamp(20, 100);
                item.extra.insert(
                    "window_percent".to_string(),
                    serde_yml::Value::Number(pct.into()),
                );
            }
            _ => {
                item.extra
                    .insert("stream".to_string(), serde_yml::Value::Bool(false));
                item.extra.insert(
                    "exec_mode".to_string(),
                    serde_yml::Value::String("console".to_string()),
                );
                item.extra.remove("floating");
                item.interactive = Some(true);
                item.quiet = Some(false);
                item.alt_buffer = Some(self.alt_buffer);
            }
        }

        item.extra.insert(
            "no_formatting".to_string(),
            serde_yml::Value::Bool(self.no_formatting),
        );
        item.extra
            .insert("masked".to_string(), serde_yml::Value::Bool(self.masked));
        item.extra
            .insert("refresh".to_string(), serde_yml::Value::Bool(self.refresh));
        item.extra.insert(
            "show_whitespace".to_string(),
            serde_yml::Value::Bool(self.show_whitespace),
        );
        item.extra.insert(
            "external".to_string(),
            serde_yml::Value::Bool(self.external),
        );

        item
    }

    /// Renders the property editor modal with live previews and scrollable form.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 86.min(area.width.saturating_sub(4));
        let dialog_h = 32.min(area.height.saturating_sub(2));

        let title_header = format!(" Edit Properties [{}] ", self.current_type.to_uppercase());
        let inner = render_modal_frame(area, dialog_w, dialog_h, &title_header, styles, buf);

        if inner.width < 30 || inner.height < 10 {
            return inner;
        }

        let cur_ctrl = self.current_control();

        // 1. Top Type Header Row
        let type_row_y = inner.y;
        let is_type_btn_active = cur_ctrl == FocusControl::ChangeTypeBtn;
        let type_str = format!(" Item Type: {:<36}", self.get_type_display_str());
        buf.set_string(
            inner.x + 1,
            type_row_y,
            &type_str,
            styles.text.add_modifier(Modifier::BOLD),
        );

        let type_btn_text = " [Change Type (CTRL+T)] ";
        let type_btn_x = (inner.x + inner.width).saturating_sub(type_btn_text.len() as u16 + 2);
        let type_btn_style = if is_type_btn_active {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            styles.accent.add_modifier(Modifier::BOLD)
        };
        buf.set_string(type_btn_x, type_row_y, type_btn_text, type_btn_style);

        // Divider under type header
        let div_line = "─".repeat(inner.width.saturating_sub(2) as usize);
        buf.set_string(
            inner.x + 1,
            inner.y + 1,
            &div_line,
            Style::default().fg(styles.border),
        );

        // 2. Scrollable Viewport bounds
        let form_start_y = inner.y + 2;
        let footer_rows = 4; // 1 buttons row + 1 blank + 2 footer lines
        let form_h = inner.height.saturating_sub(footer_rows + 2) as usize;

        // Build virtual form lines: (label, input_text_or_display, is_active, is_button, is_checkbox)
        let mut form_lines: Vec<FormRow> = Vec::new();

        if self.is_divider() {
            form_lines.push(FormRow::Label("Repeating Character (char):"));
            form_lines.push(FormRow::Input {
                ctrl: FocusControl::CharInput,
                text: self.char_input.as_string(),
                cursor: self.char_input.cursor,
                selection: self.char_input.selection_range(),
            });
            form_lines.push(FormRow::Label("Length Directive (length):"));
            form_lines.push(FormRow::Input {
                ctrl: FocusControl::LengthInput,
                text: self.length_input.as_string(),
                cursor: self.length_input.cursor,
                selection: self.length_input.selection_range(),
            });
            form_lines.push(FormRow::Label("Preview:"));
            // Live divider preview
            let preview_len = (inner.width.saturating_sub(6) as usize).min(60);
            let mut exp_c = self.char_input.as_string();
            if exp_c.is_empty() {
                exp_c = "─".to_string();
            } else if exp_c.starts_with("{ascii:") {
                let code_str = exp_c.trim_start_matches("{ascii:").trim_end_matches('}');
                if let Ok(code) = code_str.parse::<u8>() {
                    exp_c = match code {
                        196 => "─",
                        205 => "═",
                        179 => "│",
                        186 => "║",
                        _ => "─",
                    }
                    .to_string();
                }
            }
            let repeated = exp_c.repeat(preview_len);
            let div_preview: String = repeated.chars().take(preview_len).collect();
            form_lines.push(FormRow::PreviewBox(div_preview));
        } else {
            form_lines.push(FormRow::Label("Title / Label:"));
            form_lines.push(FormRow::Input {
                ctrl: FocusControl::TitleInput,
                text: self.title_input.as_string(),
                cursor: self.title_input.cursor,
                selection: self.title_input.selection_range(),
            });

            form_lines.push(FormRow::Label("Icon / Glyph ({nf:<char>:<hex>:<emoji>}):"));
            form_lines.push(FormRow::Input {
                ctrl: FocusControl::IconInput,
                text: self.icon_input.as_string(),
                cursor: self.icon_input.cursor,
                selection: self.icon_input.selection_range(),
            });

            if self.is_confirm() {
                form_lines.push(FormRow::Label("Confirmation Message / Prompt Text:"));
                form_lines.push(FormRow::Input {
                    ctrl: FocusControl::PromptInput,
                    text: self.prompt_input.as_string(),
                    cursor: self.prompt_input.cursor,
                    selection: self.prompt_input.selection_range(),
                });

                form_lines.push(FormRow::Label("On Yes Action (on_yes):"));
                form_lines.push(FormRow::InputWithBrowse {
                    input_ctrl: FocusControl::OnYesInput,
                    text: self.on_yes_input.as_string(),
                    cursor: self.on_yes_input.cursor,
                    selection: self.on_yes_input.selection_range(),
                    btn_ctrl: FocusControl::OnYesBrowseBtn,
                });

                form_lines.push(FormRow::Label("On No Action (on_no):"));
                form_lines.push(FormRow::InputWithBrowse {
                    input_ctrl: FocusControl::OnNoInput,
                    text: self.on_no_input.as_string(),
                    cursor: self.on_no_input.cursor,
                    selection: self.on_no_input.selection_range(),
                    btn_ctrl: FocusControl::OnNoBrowseBtn,
                });
            } else if self.shows_action() {
                form_lines.push(FormRow::Label(self.get_action_label_text()));
                if self.shows_action_browse() {
                    form_lines.push(FormRow::InputWithBrowse {
                        input_ctrl: FocusControl::ActionInput,
                        text: self.action_input.as_string(),
                        cursor: self.action_input.cursor,
                        selection: self.action_input.selection_range(),
                        btn_ctrl: FocusControl::ActionBrowseBtn,
                    });
                } else {
                    form_lines.push(FormRow::Input {
                        ctrl: FocusControl::ActionInput,
                        text: self.action_input.as_string(),
                        cursor: self.action_input.cursor,
                        selection: self.action_input.selection_range(),
                    });
                }

                if self.shows_prompt() {
                    let prompt_label = match self.current_type.as_str() {
                        "command" | "script" => "Prompt Text (for {param} or file pickers):",
                        "config" => "Config Prompt Message:",
                        "toggle" => "Toggle Confirmation Prompt (optional):",
                        "radio" => "Radio Dialog Title / Prompt:",
                        _ => "Message / Prompt Text:",
                    };
                    form_lines.push(FormRow::Label(prompt_label));
                    form_lines.push(FormRow::Input {
                        ctrl: FocusControl::PromptInput,
                        text: self.prompt_input.as_string(),
                        cursor: self.prompt_input.cursor,
                        selection: self.prompt_input.selection_range(),
                    });
                }
            } else if self.shows_prompt() {
                form_lines.push(FormRow::Label("Message / Prompt Text:"));
                form_lines.push(FormRow::Input {
                    ctrl: FocusControl::PromptInput,
                    text: self.prompt_input.as_string(),
                    cursor: self.prompt_input.cursor,
                    selection: self.prompt_input.selection_range(),
                });
            }

            if self.is_radio() {
                form_lines.push(FormRow::Label(
                    "Radio Options (one per line, e.g. 'Label = value' or 'value'):",
                ));
                let v_rows = 5;
                let scroll_r = self.radio_options_input.scroll_row;
                let total_lines = self.radio_options_input.lines.len();
                let sel_range = self.radio_options_input.selection_range();
                for i in 0..v_rows {
                    let actual_line_idx = scroll_r + i;
                    let text = if actual_line_idx < total_lines {
                        self.radio_options_input.lines[actual_line_idx]
                            .iter()
                            .collect()
                    } else {
                        String::new()
                    };
                    let is_cursor_row = actual_line_idx == self.radio_options_input.cursor_row;
                    let cursor = if is_cursor_row && cur_ctrl == FocusControl::RadioOptionsInput {
                        Some(self.radio_options_input.cursor_col)
                    } else {
                        None
                    };
                    let line_sel = if let Some(((start_r, start_c), (end_r, end_c))) = sel_range {
                        let line_len = text.chars().count();
                        if actual_line_idx < start_r || actual_line_idx > end_r {
                            None
                        } else if actual_line_idx == start_r && actual_line_idx == end_r {
                            Some((start_c, end_c))
                        } else if actual_line_idx == start_r {
                            Some((start_c, line_len))
                        } else if actual_line_idx == end_r {
                            Some((0, end_c))
                        } else {
                            Some((0, line_len))
                        }
                    } else {
                        None
                    };
                    let has_more_above = i == 0 && scroll_r > 0;
                    let has_more_below = i == v_rows - 1 && scroll_r + v_rows < total_lines;

                    form_lines.push(FormRow::MultiLineInputRow {
                        ctrl: FocusControl::RadioOptionsInput,
                        text,
                        cursor,
                        selection: line_sel,
                        is_focused_line: is_cursor_row,
                        has_more_above,
                        has_more_below,
                    });
                }
            }

            if self.current_type == "inject_block" {
                form_lines.push(FormRow::Label("Template Path (inject_block):"));
                form_lines.push(FormRow::InputWithBrowse {
                    input_ctrl: FocusControl::TemplateInput,
                    text: self.template_input.as_string(),
                    cursor: self.template_input.cursor,
                    selection: self.template_input.selection_range(),
                    btn_ctrl: FocusControl::TemplateBrowseBtn,
                });

                form_lines.push(FormRow::Label("Target Path (inject_block):"));
                form_lines.push(FormRow::InputWithBrowse {
                    input_ctrl: FocusControl::TargetInput,
                    text: self.target_input.as_string(),
                    cursor: self.target_input.cursor,
                    selection: self.target_input.selection_range(),
                    btn_ctrl: FocusControl::TargetBrowseBtn,
                });

                form_lines.push(FormRow::Label("Block ID (inject_block):"));
                form_lines.push(FormRow::Input {
                    ctrl: FocusControl::BlockIdInput,
                    text: self.block_id_input.as_string(),
                    cursor: self.block_id_input.cursor,
                    selection: self.block_id_input.selection_range(),
                });
            }

            if self.shows_start_dir() {
                form_lines.push(FormRow::Label(
                    "Start Directory ({file_picker} / {dir_picker}):",
                ));
                form_lines.push(FormRow::InputWithBrowse {
                    input_ctrl: FocusControl::StartDirInput,
                    text: self.start_dir_input.as_string(),
                    cursor: self.start_dir_input.cursor,
                    selection: self.start_dir_input.selection_range(),
                    btn_ctrl: FocusControl::StartDirBrowseBtn,
                });
            }

            if self.current_type == "editor" {
                form_lines.push(FormRow::Label("Tabstop:"));
                form_lines.push(FormRow::Input {
                    ctrl: FocusControl::TabstopInput,
                    text: self.tabstop_input.as_string(),
                    cursor: self.tabstop_input.cursor,
                    selection: self.tabstop_input.selection_range(),
                });
            }

            if self.is_executable() {
                form_lines.push(FormRow::Label("Execution Mode:"));
                form_lines.push(FormRow::ExecMode {
                    mode: self.exec_mode,
                    is_active: cur_ctrl == FocusControl::ExecModeSelector,
                });

                if self.exec_mode == 0 || self.exec_mode == 1 {
                    form_lines.push(FormRow::Label("Window Size % (20-100):"));
                    form_lines.push(FormRow::Input {
                        ctrl: FocusControl::WindowPercentInput,
                        text: self.window_percent_input.as_string(),
                        cursor: self.window_percent_input.cursor,
                        selection: self.window_percent_input.selection_range(),
                    });
                }

                if self.exec_mode == 2 {
                    form_lines.push(FormRow::Checkbox {
                        ctrl: FocusControl::AltBufferChk,
                        label: "Run in Alternate Screen Buffer (alt_buffer=true)",
                        checked: self.alt_buffer,
                    });
                }
            }

            if self.current_type == "editor" {
                form_lines.push(FormRow::Checkbox {
                    ctrl: FocusControl::NoFormattingChk,
                    label: "Disable Formatting (no_formatting=true)",
                    checked: self.no_formatting,
                });
                form_lines.push(FormRow::Checkbox {
                    ctrl: FocusControl::ShowWhitespaceChk,
                    label: "Show Whitespace in Editor (show_whitespace=true)",
                    checked: self.show_whitespace,
                });
            }

            if matches!(self.current_type.as_str(), "command" | "script") {
                form_lines.push(FormRow::Checkbox {
                    ctrl: FocusControl::MaskedChk,
                    label: "Mask Input (masked=true)",
                    checked: self.masked,
                });
            }

            if !self.is_nav_type() {
                form_lines.push(FormRow::Checkbox {
                    ctrl: FocusControl::RefreshChk,
                    label: "Refresh Environment (refresh=true)",
                    checked: self.refresh,
                });
                form_lines.push(FormRow::Checkbox {
                    ctrl: FocusControl::ExternalChk,
                    label: "External Execution (external=true)",
                    checked: self.external,
                });
            }

            form_lines.push(FormRow::Label("Preview:"));
            // Live item preview
            let title = self.title_input.as_string();
            let icon = self.icon_input.as_string();
            let mut preview_item = self.item.clone();
            preview_item.label = if title.is_empty() {
                "(No Title)".to_string()
            } else {
                title
            };
            preview_item.icon = if icon.is_empty() { None } else { Some(icon) };
            let preview_str = render_item_preview_line(
                &preview_item,
                (inner.width.saturating_sub(6) as usize).min(60),
            );
            form_lines.push(FormRow::PreviewBox(preview_str));
        }

        // Adjust scroll offset to keep active control in view
        let active_row_idx = form_lines.iter().position(|r| r.has_ctrl(cur_ctrl));
        let mut scroll = self.scroll_offset;
        if let Some(pos) = active_row_idx {
            if pos < scroll {
                scroll = pos;
            } else if pos >= scroll + form_h && form_h > 0 {
                scroll = pos.saturating_sub(form_h - 1);
            }
        }
        if scroll >= form_lines.len() {
            scroll = form_lines.len().saturating_sub(form_h);
        }

        // Render visible form rows
        for (i, row) in form_lines.iter().skip(scroll).take(form_h).enumerate() {
            let y = form_start_y + i as u16;
            row.render(inner, y, cur_ctrl, styles, buf);
        }

        // 3. Action Buttons Row (Save / Cancel)
        let btn_y = inner.y + inner.height.saturating_sub(4);
        let is_save_active = cur_ctrl == FocusControl::SaveBtn;
        let is_cancel_active = cur_ctrl == FocusControl::CancelBtn;

        let save_text = " [ Save Changes (CTRL+S) ] ";
        let cancel_text = " [ Cancel (ESC) ] ";
        let total_btn_w = save_text.len() + cancel_text.len() + 4;
        let start_btn_x = inner.x + (inner.width.saturating_sub(total_btn_w as u16)) / 2;

        let save_style = if is_save_active {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            styles.accent.add_modifier(Modifier::BOLD)
        };
        let cancel_style = if is_cancel_active {
            styles.highlight.add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        buf.set_string(start_btn_x, btn_y, save_text, save_style);
        buf.set_string(
            start_btn_x + save_text.len() as u16 + 4,
            btn_y,
            cancel_text,
            cancel_style,
        );

        // 4. Two-Row Footer matching Python Menuedit
        let footer_y1 = inner.y + inner.height.saturating_sub(2);
        let footer_y2 = inner.y + inner.height.saturating_sub(1);

        let items_r1 = [
            "[F1] Help",
            "[CTRL+S / F2] Save",
            "[CTRL+T] Type",
            "[CTRL+A / F3] ASCII",
        ];
        let items_r2 = [
            "[CTRL+N / F4] Glyphs",
            "[CTRL+P / F5] Placeholders",
            "[ESC / C] Cancel",
        ];

        let footer_w = inner.width.saturating_sub(2) as usize;
        let spread_r1 = bashmenu_tui::format_spread_gutter_line(&items_r1, footer_w);
        let spread_r2 = bashmenu_tui::format_spread_gutter_line(&items_r2, footer_w);

        buf.set_string(
            inner.x + 1,
            footer_y1,
            spread_r1,
            Style::default().fg(Color::DarkGray),
        );
        buf.set_string(
            inner.x + 1,
            footer_y2,
            spread_r2,
            Style::default().fg(Color::DarkGray),
        );

        inner
    }
}

/// Helper enum for form rows within the modal viewport
enum FormRow {
    Label(&'static str),
    Input {
        ctrl: FocusControl,
        text: String,
        cursor: usize,
        selection: Option<(usize, usize)>,
    },
    MultiLineInputRow {
        ctrl: FocusControl,
        text: String,
        cursor: Option<usize>,
        selection: Option<(usize, usize)>,
        is_focused_line: bool,
        has_more_above: bool,
        has_more_below: bool,
    },
    InputWithBrowse {
        input_ctrl: FocusControl,
        text: String,
        cursor: usize,
        selection: Option<(usize, usize)>,
        btn_ctrl: FocusControl,
    },
    ExecMode {
        mode: usize,
        is_active: bool,
    },
    Checkbox {
        ctrl: FocusControl,
        label: &'static str,
        checked: bool,
    },
    PreviewBox(String),
}

impl FormRow {
    fn has_ctrl(&self, ctrl: FocusControl) -> bool {
        match self {
            FormRow::Input { ctrl: c, .. } => *c == ctrl,
            FormRow::MultiLineInputRow {
                ctrl: c,
                is_focused_line,
                ..
            } => *c == ctrl && *is_focused_line,
            FormRow::InputWithBrowse {
                input_ctrl,
                btn_ctrl,
                ..
            } => *input_ctrl == ctrl || *btn_ctrl == ctrl,
            FormRow::ExecMode { is_active, .. } => *is_active,
            FormRow::Checkbox { ctrl: c, .. } => *c == ctrl,
            _ => false,
        }
    }

    fn render(
        &self,
        inner: Rect,
        y: u16,
        active_ctrl: FocusControl,
        styles: &ResolvedThemeStyles,
        buf: &mut Buffer,
    ) {
        let avail_w = inner.width.saturating_sub(4) as usize;
        match self {
            FormRow::Label(lbl) => {
                buf.set_string(
                    inner.x + 2,
                    y,
                    lbl,
                    styles.text.add_modifier(Modifier::BOLD),
                );
            }
            FormRow::Input {
                ctrl,
                text,
                cursor,
                selection,
            } => {
                let is_active = *ctrl == active_ctrl;
                let box_style = if is_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                };
                let display = format!(" {:<width$} ", text, width = avail_w.saturating_sub(4));
                buf.set_string(inner.x + 2, y, &display, box_style);

                if is_active {
                    if let Some((start, end)) = selection {
                        let text_len = text.chars().count();
                        let s = (*start).min(text_len);
                        let e = (*end).min(text_len);
                        for idx in s..e {
                            let cur_x = inner.x + 3 + idx as u16;
                            if cur_x < inner.x + inner.width - 2 {
                                buf[(cur_x, y)].set_style(
                                    Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::White)
                                        .add_modifier(Modifier::REVERSED),
                                );
                            }
                        }
                    }

                    let cur_x = inner.x + 3 + *cursor as u16;
                    if cur_x < inner.x + inner.width - 2 {
                        buf[(cur_x, y)].set_style(
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::REVERSED),
                        );
                    }
                }
            }
            FormRow::MultiLineInputRow {
                ctrl,
                text,
                cursor,
                selection,
                is_focused_line,
                has_more_above,
                has_more_below,
                ..
            } => {
                let is_ctrl_active = *ctrl == active_ctrl;
                let line_active = is_ctrl_active && *is_focused_line;
                let box_style = if line_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                };

                let content_w = avail_w.saturating_sub(4);
                let mut chars: Vec<char> = text.chars().collect();
                if chars.len() > content_w {
                    chars.truncate(content_w);
                }
                while chars.len() < content_w {
                    chars.push(' ');
                }

                if *has_more_above && chars.len() >= 3 {
                    let len = chars.len();
                    chars[len - 3] = ' ';
                    chars[len - 2] = '▲';
                    chars[len - 1] = ' ';
                } else if *has_more_below && chars.len() >= 3 {
                    let len = chars.len();
                    chars[len - 3] = ' ';
                    chars[len - 2] = '▼';
                    chars[len - 1] = ' ';
                }

                let line_content: String = chars.into_iter().collect();
                let display_str = format!(" {} ", line_content);
                buf.set_string(inner.x + 2, y, &display_str, box_style);

                if is_ctrl_active {
                    if let Some((start, end)) = selection {
                        let s = (*start).min(content_w);
                        let e = (*end).min(content_w);
                        for idx in s..e {
                            let cur_x = inner.x + 3 + idx as u16;
                            if cur_x < inner.x + 2 + content_w as u16 + 1 {
                                buf[(cur_x, y)].set_style(
                                    Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::White)
                                        .add_modifier(Modifier::REVERSED),
                                );
                            }
                        }
                    }
                }

                if line_active {
                    if let Some(col) = cursor {
                        let cur_x = inner.x + 3 + *col as u16;
                        if cur_x < inner.x + 2 + content_w as u16 + 1 {
                            buf[(cur_x, y)].set_style(
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::REVERSED),
                            );
                        }
                    }
                }
            }
            FormRow::InputWithBrowse {
                input_ctrl,
                text,
                cursor,
                selection,
                btn_ctrl,
            } => {
                let is_inp_active = *input_ctrl == active_ctrl;
                let is_btn_active = *btn_ctrl == active_ctrl;

                let browse_btn = " [Browse] ";
                let inp_w = avail_w.saturating_sub(browse_btn.len() + 4);

                let inp_style = if is_inp_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                };
                let display = format!(" {:<width$} ", text, width = inp_w);
                buf.set_string(inner.x + 2, y, &display, inp_style);

                if is_inp_active {
                    if let Some((start, end)) = selection {
                        let text_len = text.chars().count();
                        let s = (*start).min(text_len);
                        let e = (*end).min(text_len);
                        for idx in s..e {
                            let cur_x = inner.x + 3 + idx as u16;
                            if cur_x < inner.x + 2 + inp_w as u16 + 2 {
                                buf[(cur_x, y)].set_style(
                                    Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::White)
                                        .add_modifier(Modifier::REVERSED),
                                );
                            }
                        }
                    }

                    let cur_x = inner.x + 3 + *cursor as u16;
                    if cur_x < inner.x + 2 + inp_w as u16 + 2 {
                        buf[(cur_x, y)].set_style(
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::REVERSED),
                        );
                    }
                }

                let btn_x = inner.x + 2 + inp_w as u16 + 3;
                let btn_style = if is_btn_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    styles.accent.add_modifier(Modifier::BOLD)
                };
                buf.set_string(btn_x, y, browse_btn, btn_style);
            }
            FormRow::ExecMode { mode, is_active } => {
                let mode_str = match mode {
                    0 => " ● 📡 Streaming Mode (interactive=false, live popup output) ",
                    1 => " ● 💬 Interactive Mode (interactive=true, floating window) ",
                    _ => " ● 🖥️ Console Mode (interactive=true, full-screen terminal) ",
                };
                let style = if *is_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(styles.border)
                };
                buf.set_string(inner.x + 2, y, mode_str, style);
            }
            FormRow::Checkbox {
                ctrl,
                label,
                checked,
            } => {
                let is_active = *ctrl == active_ctrl;
                let mark = if *checked { "[X]" } else { "[ ]" };
                let chk_str = format!(" {} {}", mark, label);
                let style = if is_active {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                };
                buf.set_string(inner.x + 2, y, &chk_str, style);
            }
            FormRow::PreviewBox(preview) => {
                let border_style = Style::default().fg(Color::Cyan);
                let text_style = Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
                let box_str = format!("┌─{}─┐", "─".repeat(avail_w.saturating_sub(4)));
                let content_str =
                    format!("│ {:<width$} │", preview, width = avail_w.saturating_sub(4));
                buf.set_string(inner.x + 2, y, &content_str, text_style);
                buf.set_string(inner.x + 2, y.saturating_sub(1), &box_str, border_style);
            }
        }
    }
}

/// Renders a single menu item preview line matching Python `render_menu_item_preview()`
pub fn render_item_preview_line(item: &MenuItem, avail_w: usize) -> String {
    let cfg = BashMenuConfig::default();
    let ctx = MacroContext::new(&cfg);
    let expanded_title = interpolate_placeholders(&item.label, &ctx);
    let (left_raw, right_raw) = split_label_brackets(&expanded_title);
    let left_clean = strip_bbcode(left_raw);
    let right_clean = strip_bbcode(right_raw);

    let exp_icon = item
        .icon
        .as_deref()
        .map(|ic| interpolate_placeholders(ic, &ctx))
        .unwrap_or_default();
    let icon_str = if !exp_icon.is_empty() {
        let vis_w = visible_text_len(&exp_icon);
        let pad_w = 4_usize.saturating_sub(vis_w).max(1);
        format!("{}{}", exp_icon, " ".repeat(pad_w))
    } else {
        "    ".to_string()
    };

    let prefix = format!("  1.  {}", icon_str);
    let prefix_len = visible_text_len(&prefix);
    let left_len = visible_text_len(&left_clean);
    let right_len = visible_text_len(&right_clean);

    if !right_clean.is_empty() {
        let total = prefix_len + left_len + right_len;
        if total < avail_w {
            let spacer_len = avail_w.saturating_sub(total);
            format!(
                "{}{}{}{}",
                prefix,
                left_clean,
                " ".repeat(spacer_len),
                right_clean
            )
        } else {
            format!("{} {} {}", prefix, left_clean, right_clean)
        }
    } else {
        format!("{}{}", prefix, left_clean)
    }
}
