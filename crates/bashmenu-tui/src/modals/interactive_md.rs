//! Interactive Markdown form modal rendering multi-page (`{clear}`) static
//! Markdown alongside embedded `{input:...}` and `{button:...}` controls.

use super::frame::{is_close_button_clicked, render_modal_frame, render_modal_scrollbar};
use crate::style::ResolvedThemeStyles;
use bashmenu_core::{
    substitute_id_placeholders, t, ButtonSpec, ButtonType, CheckboxSpec, InputFieldSpec,
    MarkdownFormBlock, RadioLayout, RadioSpec,
};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Action emitted by [`InteractiveMarkdownModal`] on user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveFormAction {
    /// A `submit` / `ok` button (or fallback `[ OK ]`) was activated.
    Continue {
        /// Optional reserved `bashmenu-bash:id="..."` block to run.
        action: Option<String>,
        /// Committed `id -> value` map across all visited pages.
        vars: HashMap<String, String>,
    },
    /// A `cancel` button (or `Esc` / `[X]`) was activated.
    Cancel {
        /// Optional reserved `bashmenu-bash:id="..."` cleanup block to run.
        action: Option<String>,
        /// Committed `id -> value` map across all visited pages.
        vars: HashMap<String, String>,
    },
}

/// Active focus target on the current form page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFocus {
    /// Focused on `fields[idx]`.
    Input(usize),
    /// Focused on `radios[idx]`.
    Radio(usize),
    /// Focused on `checkboxes[idx]`.
    Checkbox(usize),
    /// Focused on `buttons[idx]` (an inline `{button:...}` control).
    Button(usize),
    /// Focused on the fallback footer `[ OK ]` button (only when page has no `{button:...}`).
    FallbackOk,
}

/// Runtime state for an embedded `{radio:...}` option group.
#[derive(Debug, Clone)]
pub struct RadioFieldState {
    /// Original parsed radio specification.
    pub spec: RadioSpec,
    /// Currently selected option index (`0..spec.options.len()`).
    pub selected: usize,
}

impl RadioFieldState {
    /// Creates a radio state initialized to `spec.default_index`.
    pub fn from_spec(spec: RadioSpec) -> Self {
        let selected = if spec.options.is_empty() {
            0
        } else {
            spec.default_index.min(spec.options.len() - 1)
        };
        Self { spec, selected }
    }

    /// Returns the currently selected option text.
    pub fn value(&self) -> String {
        self.spec.value_at(self.selected).to_string()
    }
}

/// Runtime state for an embedded `{checkbox:...}` boolean toggle.
#[derive(Debug, Clone)]
pub struct CheckboxFieldState {
    /// Original parsed checkbox specification.
    pub spec: CheckboxSpec,
    /// Current checked state.
    pub checked: bool,
}

impl CheckboxFieldState {
    /// Creates a checkbox state initialized to `spec.default_checked`.
    pub fn from_spec(spec: CheckboxSpec) -> Self {
        let checked = spec.default_checked;
        Self { spec, checked }
    }

    /// Returns `"true"` or `"false"`.
    pub fn value(&self) -> String {
        if self.checked {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }
}

/// Editable runtime state for a single embedded `{input:...}` widget.
#[derive(Debug, Clone)]
pub struct InputFieldState {
    /// Original parsed field specification (`id`, `width`, `multiline`, `scrollbar`, `height`).
    pub spec: InputFieldSpec,
    /// Logical text lines (always length >= 1; single element when `!spec.multiline`).
    pub lines: Vec<String>,
    /// Active cursor row (`0..lines.len()`).
    pub cursor_row: usize,
    /// Active cursor column in Unicode scalar values.
    pub cursor_col: usize,
    /// Vertical scroll offset for multi-line inputs.
    pub scroll_y: usize,
    /// Horizontal scroll offset for single-line inputs.
    pub scroll_x: usize,
}

impl InputFieldState {
    /// Creates an editable input state pre-populated with `spec.default_value`.
    pub fn from_spec(spec: InputFieldSpec) -> Self {
        let lines: Vec<String> = if spec.multiline {
            if spec.default_value.is_empty() {
                vec![String::new()]
            } else {
                spec.default_value.lines().map(String::from).collect()
            }
        } else {
            vec![spec.default_value.replace(['\r', '\n'], " ")]
        };
        let cursor_row = lines.len().saturating_sub(1);
        let cursor_col = lines[cursor_row].chars().count();
        Self {
            spec,
            lines,
            cursor_row,
            cursor_col,
            scroll_y: 0,
            scroll_x: 0,
        }
    }

    /// Returns the combined string value of this field.
    pub fn value(&self) -> String {
        self.lines.join("\n")
    }

    /// Visible row height of this input box (`1` for single-line, `spec.height` for multi-line).
    pub fn visible_rows(&self) -> usize {
        if self.spec.multiline {
            (self.spec.height as usize).max(1)
        } else {
            1
        }
    }

    fn byte_index_at(line: &str, char_col: usize) -> usize {
        line.char_indices()
            .nth(char_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    /// Inserts a character at the active cursor position.
    pub fn insert_char(&mut self, ch: char) {
        if !self.spec.multiline && (ch == '\n' || ch == '\r') {
            return;
        }
        if ch == '\n' {
            let line = &mut self.lines[self.cursor_row];
            let byte_idx = Self::byte_index_at(line, self.cursor_col);
            let tail = line.split_off(byte_idx);
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.lines.insert(self.cursor_row, tail);
            self.ensure_cursor_visible();
            return;
        }
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = Self::byte_index_at(line, self.cursor_col);
        line.insert(byte_idx, ch);
        self.cursor_col += 1;
    }

    /// Pastes text at the cursor, normalizing `\r\n` / `\r`.
    pub fn paste(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for ch in normalized.chars() {
            if !self.spec.multiline && ch == '\n' {
                self.insert_char(' ');
            } else {
                self.insert_char(ch);
            }
        }
    }

    /// Deletes the character immediately before the cursor (joining lines if multiline).
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let start = Self::byte_index_at(line, self.cursor_col - 1);
            let end = Self::byte_index_at(line, self.cursor_col);
            line.replace_range(start..end, "");
            self.cursor_col -= 1;
        } else if self.spec.multiline && self.cursor_row > 0 {
            let removed = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&removed);
            self.cursor_col = prev_len;
            self.ensure_cursor_visible();
        }
    }

    /// Deletes the character at the cursor (joining next line if multiline).
    pub fn delete(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len {
            let line = &mut self.lines[self.cursor_row];
            let start = Self::byte_index_at(line, self.cursor_col);
            let end = Self::byte_index_at(line, self.cursor_col + 1);
            line.replace_range(start..end, "");
        } else if self.spec.multiline && self.cursor_row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    /// Moves cursor vertically within a multi-line input.
    pub fn move_vertical(&mut self, delta: isize) {
        if !self.spec.multiline {
            return;
        }
        let target = (self.cursor_row as isize + delta)
            .clamp(0, self.lines.len().saturating_sub(1) as isize) as usize;
        self.cursor_row = target;
        let line_len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.cursor_col.min(line_len);
        self.ensure_cursor_visible();
    }

    /// Ensures `scroll_y` keeps `cursor_row` inside the visible multi-line viewport.
    pub fn ensure_cursor_visible(&mut self) {
        let vis = self.visible_rows();
        if self.cursor_row < self.scroll_y {
            self.scroll_y = self.cursor_row;
        } else if self.cursor_row >= self.scroll_y + vis {
            self.scroll_y = self.cursor_row + 1 - vis;
        }
    }
}

/// Interactive multi-page Markdown form modal with `{input:...}` and `{button:...}`.
#[derive(Debug, Clone)]
pub struct InteractiveMarkdownModal {
    /// Dialog title header (empty string omits title badge).
    pub title: String,
    /// Optional window width as percentage of terminal width (`1..=100`).
    pub width_percent: Option<u16>,
    /// Optional window height as percentage of terminal height (`1..=100`).
    pub height_percent: Option<u16>,
    /// Ordered pages separated by `{clear}`.
    pub pages: Vec<Vec<MarkdownFormBlock>>,
    /// Index of the currently displayed page (`0..pages.len()`).
    pub current_page: usize,
    /// Runtime state for each [`MarkdownFormBlock::Input`] on the current page.
    pub fields: Vec<InputFieldState>,
    /// Runtime state for each [`MarkdownFormBlock::Radio`] on the current page.
    pub radios: Vec<RadioFieldState>,
    /// Runtime state for each [`MarkdownFormBlock::Checkbox`] on the current page.
    pub checkboxes: Vec<CheckboxFieldState>,
    /// Flattened list of [`ButtonSpec`] controls on the current page.
    pub buttons: Vec<ButtonSpec>,
    /// Committed `id -> value` map persisted across pages for `{id:<name>}` interpolation.
    pub committed_vars: HashMap<String, String>,
    /// Active focus target on the current page.
    pub focus: FormFocus,
    /// Outer modal vertical scroll offset in rows.
    pub scroll_offset: usize,
    /// Maximum outer modal scroll offset recorded during the last render pass.
    last_max_scroll: Cell<Option<usize>>,
    /// Outer modal content viewport height recorded during the last render pass.
    last_content_h: Cell<Option<usize>>,
    /// Cached screen rectangles for each rendered input field (for mouse hit-testing).
    last_field_rects: RefCell<Vec<Rect>>,
    /// Cached screen rectangles for each radio option `[radio_idx][opt_idx]`.
    last_radio_rects: RefCell<Vec<Vec<Rect>>>,
    /// Cached screen rectangles for each checkbox row.
    last_checkbox_rects: RefCell<Vec<Rect>>,
    /// Cached screen rectangles for each rendered inline button.
    last_button_rects: RefCell<Vec<Rect>>,
    /// Cached fallback `[ OK ]` button screen rectangle.
    last_ok_rect: RefCell<Option<Rect>>,
}

impl InteractiveMarkdownModal {
    /// Constructs a single-page interactive Markdown form modal from parsed blocks.
    pub fn new(title: impl Into<String>, blocks: Vec<MarkdownFormBlock>) -> Self {
        Self::from_pages(title, vec![blocks])
    }

    /// Constructs a multi-page interactive Markdown form modal from `{clear}`-delimited pages.
    pub fn from_pages(title: impl Into<String>, mut pages: Vec<Vec<MarkdownFormBlock>>) -> Self {
        if pages.is_empty() {
            pages.push(Vec::new());
        }
        let mut modal = Self {
            title: title.into(),
            width_percent: None,
            height_percent: None,
            pages,
            current_page: 0,
            fields: Vec::new(),
            radios: Vec::new(),
            checkboxes: Vec::new(),
            buttons: Vec::new(),
            committed_vars: HashMap::new(),
            focus: FormFocus::FallbackOk,
            scroll_offset: 0,
            last_max_scroll: Cell::new(None),
            last_content_h: Cell::new(None),
            last_field_rects: RefCell::new(Vec::new()),
            last_radio_rects: RefCell::new(Vec::new()),
            last_checkbox_rects: RefCell::new(Vec::new()),
            last_button_rects: RefCell::new(Vec::new()),
            last_ok_rect: RefCell::new(None),
        };
        modal.load_current_page();
        modal
    }

    /// Sets optional percentage-based width and height overrides (`1..=100`).
    pub fn with_dimensions(
        mut self,
        width_percent: Option<u16>,
        height_percent: Option<u16>,
    ) -> Self {
        self.width_percent = width_percent.map(|w| w.clamp(1, 100));
        self.height_percent = height_percent.map(|h| h.clamp(1, 100));
        self
    }

    /// Populates `fields`, `radios`, `checkboxes`, `buttons`, `committed_vars` defaults,
    /// and initial focus for `self.current_page`.
    fn load_current_page(&mut self) {
        self.fields.clear();
        self.radios.clear();
        self.checkboxes.clear();
        self.buttons.clear();
        if let Some(blocks) = self.pages.get(self.current_page) {
            for b in blocks {
                match b {
                    MarkdownFormBlock::Input { spec, .. } => {
                        let st = InputFieldState::from_spec(spec.clone());
                        if !st.spec.id.is_empty() {
                            self.committed_vars
                                .entry(st.spec.id.clone())
                                .or_insert_with(|| st.value());
                        }
                        self.fields.push(st);
                    }
                    MarkdownFormBlock::Radio(spec) => {
                        let st = RadioFieldState::from_spec(spec.clone());
                        if !st.spec.id.is_empty() {
                            self.committed_vars
                                .entry(st.spec.id.clone())
                                .or_insert_with(|| st.value());
                        }
                        self.radios.push(st);
                    }
                    MarkdownFormBlock::Checkbox(spec) => {
                        let st = CheckboxFieldState::from_spec(spec.clone());
                        if !st.spec.id.is_empty() {
                            self.committed_vars
                                .entry(st.spec.id.clone())
                                .or_insert_with(|| st.value());
                        }
                        self.checkboxes.push(st);
                    }
                    MarkdownFormBlock::Buttons(btns) => {
                        self.buttons.extend(btns.iter().cloned());
                    }
                    MarkdownFormBlock::Text(_) => {}
                }
            }
        }
        self.scroll_offset = 0;
        self.last_max_scroll.set(None);
        self.focus = self
            .focus_ring()
            .first()
            .copied()
            .unwrap_or(FormFocus::FallbackOk);
    }

    /// Returns true if another `{clear}` page follows the current page.
    pub fn has_next_page(&self) -> bool {
        self.current_page + 1 < self.pages.len()
    }

    /// Advances to the next `{clear}` page (clearing the visible document while
    /// preserving `committed_vars`). Returns `false` if already on the last page.
    pub fn advance_page(&mut self) -> bool {
        if self.current_page + 1 < self.pages.len() {
            self.soft_refresh();
            self.current_page += 1;
            self.load_current_page();
            true
        } else {
            false
        }
    }

    /// Commits all current page input, radio, and checkbox values into [`Self::committed_vars`].
    pub fn soft_refresh(&mut self) {
        for f in &self.fields {
            if !f.spec.id.is_empty() {
                self.committed_vars.insert(f.spec.id.clone(), f.value());
            }
        }
        for r in &self.radios {
            if !r.spec.id.is_empty() {
                self.committed_vars.insert(r.spec.id.clone(), r.value());
            }
        }
        for c in &self.checkboxes {
            if !c.spec.id.is_empty() {
                self.committed_vars.insert(c.spec.id.clone(), c.value());
            }
        }
    }

    /// Returns the display label for a [`ButtonSpec`].
    fn button_label(spec: &ButtonSpec) -> String {
        if let Some(ref lbl) = spec.label {
            return lbl.clone();
        }
        match spec.button_type {
            ButtonType::Submit => "Submit".to_string(),
            ButtonType::Ok => t!("common.buttons.ok"),
            ButtonType::Cancel => t!("common.buttons.cancel"),
        }
    }

    /// Emits the [`InteractiveFormAction`] for activating `spec`.
    fn activate_button(&mut self, spec: &ButtonSpec) -> InteractiveFormAction {
        self.soft_refresh();
        match spec.button_type {
            ButtonType::Cancel => InteractiveFormAction::Cancel {
                action: spec.action.clone(),
                vars: self.committed_vars.clone(),
            },
            ButtonType::Submit | ButtonType::Ok => InteractiveFormAction::Continue {
                action: spec.action.clone(),
                vars: self.committed_vars.clone(),
            },
        }
    }

    /// Scrolls the outer modal viewport by `delta` rows, clamped to `last_max_scroll`.
    fn scroll_modal_by(&mut self, delta: isize) {
        if let Some(max_s) = self.last_max_scroll.get() {
            self.scroll_offset = self.scroll_offset.min(max_s);
            if delta < 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(delta.unsigned_abs());
            } else {
                self.scroll_offset = self.scroll_offset.saturating_add(delta as usize).min(max_s);
            }
        } else if delta < 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(delta.unsigned_abs());
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
        }
    }

    fn focus_ring(&self) -> Vec<FormFocus> {
        let mut ring = Vec::new();
        let mut input_i = 0usize;
        let mut radio_i = 0usize;
        let mut cb_i = 0usize;
        let mut btn_i = 0usize;
        if let Some(blocks) = self.pages.get(self.current_page) {
            for b in blocks {
                match b {
                    MarkdownFormBlock::Input { .. } => {
                        ring.push(FormFocus::Input(input_i));
                        input_i += 1;
                    }
                    MarkdownFormBlock::Radio(_) => {
                        ring.push(FormFocus::Radio(radio_i));
                        radio_i += 1;
                    }
                    MarkdownFormBlock::Checkbox(_) => {
                        ring.push(FormFocus::Checkbox(cb_i));
                        cb_i += 1;
                    }
                    MarkdownFormBlock::Buttons(btns) => {
                        for _ in 0..btns.len() {
                            ring.push(FormFocus::Button(btn_i));
                            btn_i += 1;
                        }
                    }
                    MarkdownFormBlock::Text(_) => {}
                }
            }
        }
        if self.buttons.is_empty() {
            ring.push(FormFocus::FallbackOk);
        }
        ring
    }

    fn cycle_focus(&mut self, forward: bool) {
        let ring = self.focus_ring();
        if ring.is_empty() {
            return;
        }
        let pos = ring.iter().position(|&f| f == self.focus).unwrap_or(0);
        let next = if forward {
            (pos + 1) % ring.len()
        } else if pos == 0 {
            ring.len() - 1
        } else {
            pos - 1
        };
        self.focus = ring[next];
    }

    /// Handles keyboard input with modifiers.
    pub fn handle_key_with_modifiers(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<InteractiveFormAction> {
        match code {
            KeyCode::Esc => {
                self.soft_refresh();
                return Some(InteractiveFormAction::Cancel {
                    action: None,
                    vars: self.committed_vars.clone(),
                });
            }
            KeyCode::Tab => {
                self.cycle_focus(true);
                return None;
            }
            KeyCode::BackTab => {
                self.cycle_focus(false);
                return None;
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => {
                self.soft_refresh();
                return None;
            }
            KeyCode::PageUp => {
                let step = self.last_content_h.get().unwrap_or(10).max(1) as isize;
                self.scroll_modal_by(-step);
                return None;
            }
            KeyCode::PageDown => {
                let step = self.last_content_h.get().unwrap_or(10).max(1) as isize;
                self.scroll_modal_by(step);
                return None;
            }
            _ => {}
        }

        match self.focus {
            FormFocus::FallbackOk => match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.soft_refresh();
                    Some(InteractiveFormAction::Continue {
                        action: None,
                        vars: self.committed_vars.clone(),
                    })
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_modal_by(-1);
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_modal_by(1);
                    None
                }
                _ => None,
            },
            FormFocus::Radio(r_idx) => {
                let len = self
                    .radios
                    .get(r_idx)
                    .map(|r| r.spec.options.len())
                    .unwrap_or(0);
                if len == 0 {
                    return None;
                }
                match code {
                    KeyCode::Up | KeyCode::Left | KeyCode::Char('k') | KeyCode::Char('h') => {
                        let r = &mut self.radios[r_idx];
                        r.selected = if r.selected == 0 {
                            len - 1
                        } else {
                            r.selected - 1
                        };
                        self.soft_refresh();
                        None
                    }
                    KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Char('l') => {
                        let r = &mut self.radios[r_idx];
                        r.selected = (r.selected + 1) % len;
                        self.soft_refresh();
                        None
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        self.soft_refresh();
                        None
                    }
                    _ => None,
                }
            }
            FormFocus::Checkbox(c_idx) => match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(cb) = self.checkboxes.get_mut(c_idx) {
                        cb.checked = !cb.checked;
                    }
                    self.soft_refresh();
                    None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_modal_by(-1);
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_modal_by(1);
                    None
                }
                _ => None,
            },
            FormFocus::Button(btn_idx) => match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let spec = self.buttons.get(btn_idx)?.clone();
                    Some(self.activate_button(&spec))
                }
                KeyCode::Left => {
                    self.cycle_focus(false);
                    None
                }
                KeyCode::Right => {
                    self.cycle_focus(true);
                    None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_modal_by(-1);
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_modal_by(1);
                    None
                }
                _ => None,
            },
            FormFocus::Input(idx) => {
                let is_multiline = self.fields[idx].spec.multiline;
                if code == KeyCode::Enter {
                    if is_multiline {
                        self.fields[idx].insert_char('\n');
                    } else {
                        self.soft_refresh();
                    }
                    return None;
                }
                if code == KeyCode::Up {
                    if is_multiline && self.fields[idx].cursor_row > 0 {
                        self.fields[idx].move_vertical(-1);
                    } else {
                        self.scroll_modal_by(-1);
                    }
                    return None;
                }
                if code == KeyCode::Down {
                    if is_multiline
                        && self.fields[idx].cursor_row + 1 < self.fields[idx].lines.len()
                    {
                        self.fields[idx].move_vertical(1);
                    } else {
                        self.scroll_modal_by(1);
                    }
                    return None;
                }

                let field = &mut self.fields[idx];
                match code {
                    KeyCode::Backspace => field.backspace(),
                    KeyCode::Delete => field.delete(),
                    KeyCode::Left => {
                        field.cursor_col = field.cursor_col.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        let len = field.lines[field.cursor_row].chars().count();
                        field.cursor_col = (field.cursor_col + 1).min(len);
                    }
                    KeyCode::Home => field.cursor_col = 0,
                    KeyCode::End => {
                        field.cursor_col = field.lines[field.cursor_row].chars().count();
                    }
                    KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                        field.insert_char(c);
                    }
                    _ => {}
                }
                None
            }
        }
    }

    /// Pastes text into the currently focused input field (if any).
    pub fn paste(&mut self, text: &str) {
        if let FormFocus::Input(idx) = self.focus {
            self.fields[idx].paste(text);
        }
    }

    /// Handles mouse clicks (field focus, buttons, `[ OK ]`, `[X]`) and wheel scrolling.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<InteractiveFormAction> {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if is_close_button_clicked(inner, mouse.row, mouse.column) {
                    self.soft_refresh();
                    return Some(InteractiveFormAction::Cancel {
                        action: None,
                        vars: self.committed_vars.clone(),
                    });
                }

                // Check inline `{button:...}` clicks
                let clicked_btn: Option<usize> = {
                    let rects = self.last_button_rects.borrow();
                    rects.iter().position(|r| {
                        r.width > 0
                            && mouse.row == r.y
                            && mouse.column >= r.x
                            && mouse.column < r.x + r.width
                    })
                };
                if let Some(btn_idx) = clicked_btn {
                    self.focus = FormFocus::Button(btn_idx);
                    if let Some(spec) = self.buttons.get(btn_idx).cloned() {
                        return Some(self.activate_button(&spec));
                    }
                }

                // Check fallback `[ OK ]` button click
                let ok_rect_opt = *self.last_ok_rect.borrow();
                if let Some(ok_rect) = ok_rect_opt {
                    if mouse.row == ok_rect.y
                        && mouse.column >= ok_rect.x
                        && mouse.column < ok_rect.x + ok_rect.width
                    {
                        self.soft_refresh();
                        return Some(InteractiveFormAction::Continue {
                            action: None,
                            vars: self.committed_vars.clone(),
                        });
                    }
                }

                // Check `{radio:...}` option click
                let clicked_radio: Option<(usize, usize)> = {
                    let rects = self.last_radio_rects.borrow();
                    rects.iter().enumerate().find_map(|(r_idx, opts)| {
                        opts.iter().enumerate().find_map(|(o_idx, r)| {
                            if r.width > 0
                                && mouse.row == r.y
                                && mouse.column >= r.x
                                && mouse.column < r.x + r.width
                            {
                                Some((r_idx, o_idx))
                            } else {
                                None
                            }
                        })
                    })
                };
                if let Some((r_idx, o_idx)) = clicked_radio {
                    self.focus = FormFocus::Radio(r_idx);
                    if let Some(r) = self.radios.get_mut(r_idx) {
                        if o_idx < r.spec.options.len() {
                            r.selected = o_idx;
                        }
                    }
                    self.soft_refresh();
                    return None;
                }

                // Check `{checkbox:...}` click
                let clicked_cb: Option<usize> = {
                    let rects = self.last_checkbox_rects.borrow();
                    rects.iter().position(|r| {
                        r.width > 0
                            && mouse.row == r.y
                            && mouse.column >= r.x
                            && mouse.column < r.x + r.width
                    })
                };
                if let Some(c_idx) = clicked_cb {
                    self.focus = FormFocus::Checkbox(c_idx);
                    if let Some(cb) = self.checkboxes.get_mut(c_idx) {
                        cb.checked = !cb.checked;
                    }
                    self.soft_refresh();
                    return None;
                }

                // Check `{input:...}` field click
                let clicked_field: Option<usize> = {
                    let rects = self.last_field_rects.borrow();
                    rects.iter().position(|r| {
                        r.width > 0
                            && mouse.column >= r.x
                            && mouse.column < r.x + r.width
                            && mouse.row >= r.y
                            && mouse.row < r.y + r.height
                    })
                };
                if let Some(i) = clicked_field {
                    self.focus = FormFocus::Input(i);
                }
                None
            }
            MouseEventKind::ScrollUp => {
                if let FormFocus::Input(idx) = self.focus {
                    if self.fields[idx].spec.multiline && self.fields[idx].scroll_y > 0 {
                        self.fields[idx].scroll_y = self.fields[idx].scroll_y.saturating_sub(1);
                        return None;
                    }
                }
                self.scroll_modal_by(-2);
                None
            }
            MouseEventKind::ScrollDown => {
                if let FormFocus::Input(idx) = self.focus {
                    let f = &mut self.fields[idx];
                    let max_y = f.lines.len().saturating_sub(f.visible_rows());
                    if f.spec.multiline && f.scroll_y < max_y {
                        f.scroll_y = (f.scroll_y + 1).min(max_y);
                        return None;
                    }
                }
                self.scroll_modal_by(2);
                None
            }
            _ => None,
        }
    }

    /// Computes target dialog dimensions for this form modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let max_w = if self.width_percent.is_some() {
            area.width.saturating_sub(4).max(24.min(area.width))
        } else {
            area.width.saturating_sub(4).clamp(24, 96)
        };
        let dialog_w = if let Some(wp) = self.width_percent {
            let target = ((area.width as u32 * wp.clamp(1, 100) as u32) / 100) as u16;
            target.clamp(24.min(max_w), max_w)
        } else {
            max_w.min(76)
        };

        let max_h = area.height.saturating_sub(2);
        let dialog_h = if let Some(hp) = self.height_percent {
            let target = ((area.height as u32 * hp.clamp(1, 100) as u32) / 100) as u16;
            target.clamp(10.min(max_h), max_h)
        } else {
            let avail_w = dialog_w.saturating_sub(6) as usize;
            let mut est_rows = 0usize;
            let mut field_idx = 0usize;
            if let Some(blocks) = self.pages.get(self.current_page) {
                for b in blocks {
                    match b {
                        MarkdownFormBlock::Text(t) => {
                            for line in t.lines() {
                                est_rows +=
                                    super::input::wrap_prompt_text(line, avail_w.max(1)).len();
                            }
                            est_rows += 1;
                        }
                        MarkdownFormBlock::Input { .. } => {
                            if let Some(f) = self.fields.get(field_idx) {
                                est_rows += f.visible_rows() + 1;
                            }
                            field_idx += 1;
                        }
                        MarkdownFormBlock::Radio(spec) => {
                            est_rows += match spec.layout {
                                RadioLayout::Vertical => spec.options.len() + 1,
                                RadioLayout::Horizontal => 2,
                            };
                        }
                        MarkdownFormBlock::Checkbox(_) => {
                            est_rows += 2;
                        }
                        MarkdownFormBlock::Buttons(_) => {
                            est_rows += 2;
                        }
                    }
                }
            }
            (est_rows as u16 + 6).min(max_h).max(10.min(max_h))
        };
        (dialog_w, dialog_h)
    }

    /// Renders the current page of the interactive Markdown form modal.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let has_inline_buttons = !self.buttons.is_empty();
        let left_x = inner.x + 2;
        let avail_w = inner.width.saturating_sub(4) as usize;
        let content_top = inner.y + 1;
        let content_h = if has_inline_buttons {
            inner.height.saturating_sub(2) as usize
        } else {
            inner.height.saturating_sub(3) as usize
        };

        enum VirtualRow {
            Text(String, Style),
            InputRow {
                field_idx: usize,
                row_in_field: usize,
                prefix: Option<String>,
            },
            RadioOptionRow {
                radio_idx: usize,
                opt_idx: usize,
            },
            RadioHorizontalRow {
                radio_idx: usize,
            },
            CheckboxRow {
                checkbox_idx: usize,
            },
            ButtonRow {
                start_btn_idx: usize,
                count: usize,
            },
            Blank,
        }

        let h1_style = styles
            .title
            .bg(styles.background)
            .add_modifier(Modifier::BOLD);
        let h2_style = styles
            .accent
            .bg(styles.background)
            .add_modifier(Modifier::BOLD);
        let base_style = styles.text.bg(styles.background);

        let mut vrows: Vec<VirtualRow> = Vec::new();
        let mut field_counter = 0usize;
        let mut radio_counter = 0usize;
        let mut checkbox_counter = 0usize;
        let mut button_counter = 0usize;

        if let Some(blocks) = self.pages.get(self.current_page) {
            for block in blocks {
                match block {
                    MarkdownFormBlock::Text(t) => {
                        let expanded = substitute_id_placeholders(t, &self.committed_vars);
                        for raw_line in expanded.lines() {
                            let trimmed = raw_line.trim();
                            if trimmed.is_empty() {
                                vrows.push(VirtualRow::Blank);
                            } else if trimmed.starts_with("# ") {
                                vrows.push(VirtualRow::Text(trimmed.to_string(), h1_style));
                            } else if trimmed.starts_with("## ") {
                                vrows.push(VirtualRow::Text(trimmed.to_string(), h2_style));
                            } else {
                                for wl in super::input::wrap_prompt_text(raw_line, avail_w.max(1)) {
                                    vrows.push(VirtualRow::Text(wl, base_style));
                                }
                            }
                        }
                        vrows.push(VirtualRow::Blank);
                    }
                    MarkdownFormBlock::Input { prefix, .. } => {
                        let f_idx = field_counter;
                        field_counter += 1;
                        if let Some(f) = self.fields.get(f_idx) {
                            for r in 0..f.visible_rows() {
                                vrows.push(VirtualRow::InputRow {
                                    field_idx: f_idx,
                                    row_in_field: r,
                                    prefix: if r == 0 { prefix.clone() } else { None },
                                });
                            }
                            vrows.push(VirtualRow::Blank);
                        }
                    }
                    MarkdownFormBlock::Radio(spec) => {
                        let r_idx = radio_counter;
                        radio_counter += 1;
                        match spec.layout {
                            RadioLayout::Vertical => {
                                for opt_idx in 0..spec.options.len() {
                                    vrows.push(VirtualRow::RadioOptionRow {
                                        radio_idx: r_idx,
                                        opt_idx,
                                    });
                                }
                            }
                            RadioLayout::Horizontal => {
                                vrows.push(VirtualRow::RadioHorizontalRow { radio_idx: r_idx });
                            }
                        }
                        vrows.push(VirtualRow::Blank);
                    }
                    MarkdownFormBlock::Checkbox(_) => {
                        let c_idx = checkbox_counter;
                        checkbox_counter += 1;
                        vrows.push(VirtualRow::CheckboxRow {
                            checkbox_idx: c_idx,
                        });
                        vrows.push(VirtualRow::Blank);
                    }
                    MarkdownFormBlock::Buttons(btns) => {
                        let start_idx = button_counter;
                        let count = btns.len();
                        button_counter += count;
                        vrows.push(VirtualRow::ButtonRow {
                            start_btn_idx: start_idx,
                            count,
                        });
                        vrows.push(VirtualRow::Blank);
                    }
                }
            }
        }

        let max_scroll = vrows.len().saturating_sub(content_h);
        let eff_scroll = self.scroll_offset.min(max_scroll);
        self.last_max_scroll.set(Some(max_scroll));
        self.last_content_h.set(Some(content_h));

        let mut field_rects = vec![Rect::default(); self.fields.len()];
        let mut radio_rects: Vec<Vec<Rect>> = self
            .radios
            .iter()
            .map(|r| vec![Rect::default(); r.spec.options.len()])
            .collect();
        let mut checkbox_rects = vec![Rect::default(); self.checkboxes.len()];
        let mut button_rects = vec![Rect::default(); self.buttons.len()];

        for (vis_i, vrow) in vrows.iter().skip(eff_scroll).take(content_h).enumerate() {
            let y = content_top + vis_i as u16;
            match vrow {
                VirtualRow::Blank => {}
                VirtualRow::Text(text, style) => {
                    let spans = crate::bbcode::parse_bbcode_spans(text, *style);
                    buf.set_line(left_x, y, &ratatui::text::Line::from(spans), avail_w as u16);
                }
                VirtualRow::InputRow {
                    field_idx,
                    row_in_field,
                    prefix,
                } => {
                    let f = &self.fields[*field_idx];
                    let is_focused = self.focus == FormFocus::Input(*field_idx);

                    let (input_x, rem_w) = if let Some(ref p) = prefix {
                        let p_expanded = substitute_id_placeholders(p, &self.committed_vars);
                        let spans = crate::bbcode::parse_bbcode_spans(&p_expanded, base_style);
                        let p_vis = unicode_width::UnicodeWidthStr::width(
                            crate::bbcode::strip_bbcode(&p_expanded).as_str(),
                        )
                        .min(avail_w.saturating_sub(4));
                        buf.set_line(left_x, y, &ratatui::text::Line::from(spans), p_vis as u16);
                        (left_x + p_vis as u16, avail_w.saturating_sub(p_vis))
                    } else {
                        (left_x, avail_w)
                    };

                    let box_w = f
                        .spec
                        .width
                        .map(|w| (w as usize).min(rem_w).max(4.min(rem_w)))
                        .unwrap_or(rem_w) as u16;

                    if field_rects[*field_idx].width == 0 {
                        field_rects[*field_idx] =
                            Rect::new(input_x, y, box_w, f.visible_rows() as u16);
                    }

                    let box_style = if is_focused {
                        styles.input_box
                    } else {
                        styles.input_box_unfocused
                    };

                    for dx in 0..box_w {
                        buf[(input_x + dx, y)].set_char(' ').set_style(box_style);
                    }

                    let show_sb = f.spec.multiline
                        && f.spec.scrollbar
                        && f.lines.len() > f.visible_rows()
                        && box_w >= 4;
                    let text_w = if show_sb {
                        box_w.saturating_sub(1) as usize
                    } else {
                        box_w as usize
                    };

                    let line_idx = f.scroll_y + *row_in_field;
                    if let Some(line_str) = f.lines.get(line_idx) {
                        let chars: Vec<char> = line_str.chars().collect();
                        let h_scroll = if !f.spec.multiline && is_focused && f.cursor_col >= text_w
                        {
                            f.cursor_col + 1 - text_w
                        } else {
                            0
                        };
                        let visible: String = chars.iter().skip(h_scroll).take(text_w).collect();
                        buf.set_string(input_x, y, &visible, box_style);

                        if is_focused && line_idx == f.cursor_row {
                            let cur_vis_x = f.cursor_col.saturating_sub(h_scroll);
                            if cur_vis_x < text_w {
                                let cx = input_x + cur_vis_x as u16;
                                let cur_ch = chars.get(f.cursor_col).copied().unwrap_or(' ');
                                buf[(cx, y)]
                                    .set_char(cur_ch)
                                    .set_style(box_style.add_modifier(Modifier::REVERSED));
                            }
                        }
                    }

                    if show_sb && *row_in_field == 0 {
                        render_modal_scrollbar(
                            input_x + box_w.saturating_sub(1),
                            y,
                            f.visible_rows(),
                            f.lines.len(),
                            f.scroll_y,
                            styles,
                            buf,
                        );
                    }
                }
                VirtualRow::RadioOptionRow { radio_idx, opt_idx } => {
                    if let Some(r) = self.radios.get(*radio_idx) {
                        if let Some(opt) = r.spec.options.get(*opt_idx) {
                            let is_focused = self.focus == FormFocus::Radio(*radio_idx);
                            let is_sel = r.selected == *opt_idx;
                            let glyph = if is_sel { "(●)" } else { "( )" };
                            let opt_text = substitute_id_placeholders(opt, &self.committed_vars);
                            let line_str = format!(" {glyph} {opt_text} ");
                            let clipped: String = line_str.chars().take(avail_w).collect();
                            let style = if is_focused && is_sel {
                                styles.highlight.add_modifier(Modifier::BOLD)
                            } else if is_sel {
                                styles
                                    .accent
                                    .bg(styles.background)
                                    .add_modifier(Modifier::BOLD)
                            } else if is_focused {
                                styles.text.bg(styles.background)
                            } else {
                                base_style
                            };
                            buf.set_string(left_x, y, &clipped, style);
                            radio_rects[*radio_idx][*opt_idx] =
                                Rect::new(left_x, y, clipped.chars().count() as u16, 1);
                        }
                    }
                }
                VirtualRow::RadioHorizontalRow { radio_idx } => {
                    if let Some(r) = self.radios.get(*radio_idx) {
                        let is_focused = self.focus == FormFocus::Radio(*radio_idx);
                        let mut cur_x = left_x;
                        let right_bound = inner.x + inner.width.saturating_sub(1);
                        for (opt_idx, opt) in r.spec.options.iter().enumerate() {
                            if cur_x >= right_bound {
                                break;
                            }
                            let is_sel = r.selected == opt_idx;
                            let glyph = if is_sel { "(●)" } else { "( )" };
                            let opt_text = substitute_id_placeholders(opt, &self.committed_vars);
                            let item_str = format!(" {glyph} {opt_text} ");
                            let max_chars = (right_bound - cur_x) as usize;
                            let clipped: String = item_str.chars().take(max_chars).collect();
                            let style = if is_focused && is_sel {
                                styles.highlight.add_modifier(Modifier::BOLD)
                            } else if is_sel {
                                styles
                                    .accent
                                    .bg(styles.background)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                base_style
                            };
                            let w = clipped.chars().count() as u16;
                            buf.set_string(cur_x, y, &clipped, style);
                            radio_rects[*radio_idx][opt_idx] = Rect::new(cur_x, y, w, 1);
                            cur_x = cur_x.saturating_add(w + 1);
                        }
                    }
                }
                VirtualRow::CheckboxRow { checkbox_idx } => {
                    if let Some(cb) = self.checkboxes.get(*checkbox_idx) {
                        let is_focused = self.focus == FormFocus::Checkbox(*checkbox_idx);
                        let glyph = if cb.checked { "[x]" } else { "[ ]" };
                        let lbl = substitute_id_placeholders(&cb.spec.label, &self.committed_vars);
                        let line_str = format!(" {glyph} {lbl} ");
                        let clipped: String = line_str.chars().take(avail_w).collect();
                        let style = if is_focused {
                            styles.highlight.add_modifier(Modifier::BOLD)
                        } else if cb.checked {
                            styles
                                .accent
                                .bg(styles.background)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            base_style
                        };
                        buf.set_string(left_x, y, &clipped, style);
                        checkbox_rects[*checkbox_idx] =
                            Rect::new(left_x, y, clipped.chars().count() as u16, 1);
                    }
                }
                VirtualRow::ButtonRow {
                    start_btn_idx,
                    count,
                } => {
                    let mut cur_x = left_x;
                    let right_bound = inner.x + inner.width.saturating_sub(1);
                    for offset in 0..*count {
                        let idx = start_btn_idx + offset;
                        let Some(spec) = self.buttons.get(idx) else {
                            continue;
                        };
                        let label = Self::button_label(spec);
                        let rendered = format!(" [ {label} ] ");
                        let btn_w = rendered.chars().count() as u16;
                        if cur_x >= right_bound {
                            break;
                        }
                        let max_chars = (right_bound - cur_x) as usize;
                        let clipped: String = rendered.chars().take(max_chars).collect();
                        let base_btn = match spec.button_type {
                            ButtonType::Cancel => styles.modal_cancel_button,
                            ButtonType::Submit | ButtonType::Ok => {
                                styles.modal_yes_button.add_modifier(Modifier::BOLD)
                            }
                        };
                        let btn_style = if self.focus == FormFocus::Button(idx) {
                            base_btn.add_modifier(Modifier::BOLD | Modifier::REVERSED)
                        } else {
                            base_btn
                        };
                        buf.set_string(cur_x, y, &clipped, btn_style);
                        button_rects[idx] = Rect::new(cur_x, y, clipped.chars().count() as u16, 1);
                        cur_x = cur_x.saturating_add(btn_w + 1);
                    }
                }
            }
        }

        *self.last_field_rects.borrow_mut() = field_rects;
        *self.last_radio_rects.borrow_mut() = radio_rects;
        *self.last_checkbox_rects.borrow_mut() = checkbox_rects;
        *self.last_button_rects.borrow_mut() = button_rects;

        // Outer modal scrollbar
        if inner.width >= 4 {
            render_modal_scrollbar(
                inner.x + inner.width.saturating_sub(1),
                content_top,
                content_h,
                vrows.len(),
                eff_scroll,
                styles,
                buf,
            );
        }

        // Render fallback [ OK ] footer button only when the page defines no {button:...} controls
        if has_inline_buttons {
            *self.last_ok_rect.borrow_mut() = None;
        } else {
            let btn_y = inner.y + inner.height.saturating_sub(2);
            let ok_str = format!(" [ {} ] ", t!("common.buttons.ok"));
            let ok_w = ok_str.chars().count() as u16;
            let center_x = inner.x + inner.width / 2;
            let ok_x = center_x.saturating_sub(ok_w / 2).max(inner.x + 1);
            let right_bound = inner.x + inner.width;
            if ok_x < right_bound {
                let max_ok = (right_bound - ok_x) as usize;
                let clipped_ok: String = ok_str.chars().take(max_ok).collect();
                let ok_style = if self.focus == FormFocus::FallbackOk {
                    styles
                        .modal_yes_button
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    styles.modal_yes_button.add_modifier(Modifier::BOLD)
                };
                buf.set_string(ok_x, btn_y, &clipped_ok, ok_style);
                *self.last_ok_rect.borrow_mut() =
                    Some(Rect::new(ok_x, btn_y, clipped_ok.chars().count() as u16, 1));
            }
        }

        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bashmenu_core::{parse_interactive_markdown_doc, parse_markdown_form_blocks};

    #[test]
    fn test_interactive_markdown_modal_input_editing_and_submit() {
        let md = "\
# Profile Setup
Username:
{input: default=\"andrew\" : id = userName : width = 30 : multiline = false : scrollbar = true}
Notes:
{input: default=\"Line 1\" : id = userNotes : multiline = true : scrollbar = true : height = 4}
";
        let blocks = parse_markdown_form_blocks(md);
        let mut modal = InteractiveMarkdownModal::new("Form", blocks);
        assert_eq!(modal.fields.len(), 2);
        assert_eq!(modal.focus, FormFocus::Input(0));

        // Append to single-line field 0 (typing alone does NOT update committed_vars yet)
        modal.handle_key_with_modifiers(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(
            modal.committed_vars.get("userName").map(String::as_str),
            Some("andrew")
        );

        // Pressing Enter soft-refreshes committed_vars and stays on field 0
        assert!(modal
            .handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE)
            .is_none());
        assert_eq!(modal.focus, FormFocus::Input(0));
        assert_eq!(
            modal.committed_vars.get("userName").map(String::as_str),
            Some("andrew1")
        );

        // Tab to multiline field 1 and add a second line
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focus, FormFocus::Input(1));
        modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        modal.paste("Line 2");

        // Tab to fallback [ OK ] and press Enter to continue/submit
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focus, FormFocus::FallbackOk);
        let action = modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        match action {
            Some(InteractiveFormAction::Continue { action: None, vars }) => {
                assert_eq!(vars.get("userName").map(String::as_str), Some("andrew1"));
                assert_eq!(
                    vars.get("userNotes").map(String::as_str),
                    Some("Line 1\nLine 2")
                );
            }
            other => panic!("expected Continue, got {other:?}"),
        }
    }

    #[test]
    fn test_live_id_interpolation_soft_refresh_render() {
        let md = "\
What is your name: {input: default=\"andrew\": id=\"guestName\"}

According to my records, your name is \"{id:guestName}\".
";
        let blocks = parse_markdown_form_blocks(md);
        let mut modal = InteractiveMarkdownModal::new("Live", blocks);
        let styles = ResolvedThemeStyles::default();
        let area = Rect::new(0, 0, 80, 20);

        let buffer_text = |m: &InteractiveMarkdownModal| -> String {
            let mut buf = Buffer::empty(area);
            m.render(area, &styles, &mut buf);
            let mut s = String::new();
            for y in 0..area.height {
                for x in 0..area.width {
                    s.push_str(buf[(x, y)].symbol());
                }
                s.push('\n');
            }
            s
        };

        assert!(buffer_text(&modal).contains("your name is \"andrew\"."));
        modal.handle_key_with_modifiers(KeyCode::Char('!'), KeyModifiers::NONE);
        assert!(buffer_text(&modal).contains("your name is \"andrew\"."));
        modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(modal.focus, FormFocus::Input(0));
        assert!(buffer_text(&modal).contains("your name is \"andrew!\"."));
    }

    #[test]
    fn test_interactive_markdown_modal_arrow_and_page_scrolling() {
        let mut md = String::new();
        for i in 1..=30 {
            md.push_str(&format!("Line {i}\n"));
        }
        md.push_str("Name: {input: default=\"test\": id=\"name\"}\n");
        let blocks = parse_markdown_form_blocks(&md);
        let mut modal = InteractiveMarkdownModal::new("Scrollable", blocks);
        let styles = ResolvedThemeStyles::default();
        let area = Rect::new(0, 0, 60, 14);
        let mut buf = Buffer::empty(area);
        modal.render(area, &styles, &mut buf);

        assert_eq!(modal.focus, FormFocus::Input(0));
        assert_eq!(modal.scroll_offset, 0);
        modal.handle_key_with_modifiers(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(modal.scroll_offset, 1);
        modal.handle_key_with_modifiers(KeyCode::PageDown, KeyModifiers::NONE);
        assert!(modal.scroll_offset > 1);
        let after_pgdn = modal.scroll_offset;
        modal.handle_key_with_modifiers(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(modal.scroll_offset, after_pgdn - 1);
        modal.handle_key_with_modifiers(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(modal.scroll_offset, 0);
    }

    #[test]
    fn test_multipage_clear_buttons_and_reserved_scripts() {
        let md = "\
```bashmenu-bash:id=\"greetUser\"
{message:\"Hello $guestName\"}
```

# Page 1
Name: {input: default=\"Alice\": id=\"guestName\"}

{button: type=\"submit\": action=\"greetUser\"} {button: type=\"cancel\"}

{clear}

# Page 2
Welcome back, {id:guestName}!

{button: type=\"ok\"}
";
        let doc = parse_interactive_markdown_doc(md);
        assert!(doc.reserved_scripts.contains_key("greetUser"));
        assert_eq!(doc.pages.len(), 2);

        let mut modal = InteractiveMarkdownModal::from_pages("Wizard", doc.pages);
        assert!(modal.has_next_page());
        assert_eq!(modal.buttons.len(), 2);
        assert_eq!(modal.focus, FormFocus::Input(0));

        // Tab to Submit button and press Enter
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focus, FormFocus::Button(0));
        let act = modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            act,
            Some(InteractiveFormAction::Continue {
                action: Some("greetUser".to_string()),
                vars: HashMap::from([("guestName".to_string(), "Alice".to_string())]),
            })
        );

        // Advance past {clear} to Page 2; committed_vars persists!
        assert!(modal.advance_page());
        assert!(!modal.has_next_page());
        assert_eq!(modal.current_page, 1);
        assert_eq!(
            modal.committed_vars.get("guestName").map(String::as_str),
            Some("Alice")
        );
        assert_eq!(modal.focus, FormFocus::Button(0));
    }

    #[test]
    fn test_radio_and_checkbox_interaction_and_live_refresh() {
        let md = "\
Choose an option:
{radio: rdoValue: \"Example 1\": \"Example 2\": \"Example 3\"}
{checkbox: enableLogs: \"Enable verbose logging\": false}
Selected: {id:rdoValue} | Logs: {id:enableLogs}
{button: type=\"submit\"}
";
        let doc = parse_interactive_markdown_doc(md);
        let mut modal = InteractiveMarkdownModal::from_pages("Choices", doc.pages);
        assert_eq!(modal.focus, FormFocus::Radio(0));
        assert_eq!(
            modal.committed_vars.get("rdoValue").map(String::as_str),
            Some("Example 1")
        );
        assert_eq!(
            modal.committed_vars.get("enableLogs").map(String::as_str),
            Some("false")
        );

        // Move radio selection down -> live refresh to Example 2
        modal.handle_key_with_modifiers(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(
            modal.committed_vars.get("rdoValue").map(String::as_str),
            Some("Example 2")
        );

        // Tab to checkbox and toggle with Space -> live refresh to true
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focus, FormFocus::Checkbox(0));
        modal.handle_key_with_modifiers(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(
            modal.committed_vars.get("enableLogs").map(String::as_str),
            Some("true")
        );

        // Render and verify selected glyphs and live interpolated text appear
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let styles = ResolvedThemeStyles::default();
        modal.render(area, &styles, &mut buf);
        let mut rendered = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("(●) Example 2"));
        assert!(rendered.contains("[x] Enable verbose logging"));
        assert!(rendered.contains("Selected: Example 2 | Logs: true"));
    }
}
