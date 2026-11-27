//! Interactive Markdown form modal rendering static Markdown alongside embedded
//! `{input:...}` single-line and multi-line form controls.

use super::frame::{is_close_button_clicked, render_modal_frame, render_modal_scrollbar};
use crate::style::ResolvedThemeStyles;
use bashmenu_core::{t, InputFieldSpec, MarkdownFormBlock};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::cell::RefCell;
use std::collections::HashMap;

/// Action emitted by [`InteractiveMarkdownModal`] on user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveFormAction {
    /// User submitted the form (`[ OK ]` / Enter); maps `id` -> field value.
    Submit(HashMap<String, String>),
    /// User cancelled (`Esc` or `[X]` close button).
    Cancel,
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

/// Interactive Markdown form modal with embedded `{input:...}` widgets.
#[derive(Debug, Clone)]
pub struct InteractiveMarkdownModal {
    /// Dialog title header (empty string omits title badge).
    pub title: String,
    /// Optional window width as percentage of terminal width (`1..=100`).
    pub width_percent: Option<u16>,
    /// Optional window height as percentage of terminal height (`1..=100`).
    pub height_percent: Option<u16>,
    /// Ordered Markdown text and input field blocks.
    pub blocks: Vec<MarkdownFormBlock>,
    /// Runtime state for each [`MarkdownFormBlock::Input`] in order.
    pub fields: Vec<InputFieldState>,
    /// Index of currently focused input field, or `None` when `[ OK ]` button is focused.
    pub focused_field: Option<usize>,
    /// Outer modal vertical scroll offset in rows.
    pub scroll_offset: usize,
    /// Cached screen rectangles for each rendered input field (for mouse hit-testing).
    last_field_rects: RefCell<Vec<Rect>>,
    /// Cached `[ OK ]` button screen rectangle.
    last_ok_rect: RefCell<Option<Rect>>,
}

impl InteractiveMarkdownModal {
    /// Constructs a new interactive Markdown form modal from parsed blocks.
    pub fn new(title: impl Into<String>, blocks: Vec<MarkdownFormBlock>) -> Self {
        let fields: Vec<InputFieldState> = blocks
            .iter()
            .filter_map(|b| match b {
                MarkdownFormBlock::Input(spec) => Some(InputFieldState::from_spec(spec.clone())),
                MarkdownFormBlock::Text(_) => None,
            })
            .collect();
        let focused_field = if fields.is_empty() { None } else { Some(0) };
        Self {
            title: title.into(),
            width_percent: None,
            height_percent: None,
            blocks,
            fields,
            focused_field,
            scroll_offset: 0,
            last_field_rects: RefCell::new(Vec::new()),
            last_ok_rect: RefCell::new(None),
        }
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

    /// Collects all named (`!id.is_empty()`) input field values into a map.
    pub fn collect_values(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for f in &self.fields {
            if !f.spec.id.is_empty() {
                map.insert(f.spec.id.clone(), f.value());
            }
        }
        map
    }

    fn cycle_focus(&mut self, forward: bool) {
        if self.fields.is_empty() {
            self.focused_field = None;
            return;
        }
        let total = self.fields.len() + 1; // fields + [ OK ] button
        let cur = self.focused_field.unwrap_or(self.fields.len());
        let next = if forward {
            (cur + 1) % total
        } else if cur == 0 {
            total - 1
        } else {
            cur - 1
        };
        self.focused_field = if next < self.fields.len() {
            Some(next)
        } else {
            None
        };
    }

    /// Handles keyboard input with modifiers.
    pub fn handle_key_with_modifiers(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<InteractiveFormAction> {
        match code {
            KeyCode::Esc => return Some(InteractiveFormAction::Cancel),
            KeyCode::Tab => {
                self.cycle_focus(true);
                return None;
            }
            KeyCode::BackTab => {
                self.cycle_focus(false);
                return None;
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InteractiveFormAction::Submit(self.collect_values()));
            }
            _ => {}
        }

        let Some(idx) = self.focused_field else {
            // Focus is on the [ OK ] button
            return match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    Some(InteractiveFormAction::Submit(self.collect_values()))
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    None
                }
                _ => None,
            };
        };

        let field = &mut self.fields[idx];
        match code {
            KeyCode::Enter => {
                if field.spec.multiline {
                    field.insert_char('\n');
                } else {
                    self.cycle_focus(true);
                }
            }
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
            KeyCode::Up => field.move_vertical(-1),
            KeyCode::Down => field.move_vertical(1),
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                field.insert_char(c);
            }
            _ => {}
        }
        None
    }

    /// Pastes text into the currently focused input field (if any).
    pub fn paste(&mut self, text: &str) {
        if let Some(idx) = self.focused_field {
            self.fields[idx].paste(text);
        }
    }

    /// Handles mouse clicks (field focus, `[ OK ]`, `[X]`) and wheel scrolling.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<InteractiveFormAction> {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if is_close_button_clicked(inner, mouse.row, mouse.column) {
                    return Some(InteractiveFormAction::Cancel);
                }
                if let Some(ok_rect) = *self.last_ok_rect.borrow() {
                    if mouse.row == ok_rect.y
                        && mouse.column >= ok_rect.x
                        && mouse.column < ok_rect.x + ok_rect.width
                    {
                        return Some(InteractiveFormAction::Submit(self.collect_values()));
                    }
                }
                let rects = self.last_field_rects.borrow();
                for (i, r) in rects.iter().enumerate() {
                    if mouse.column >= r.x
                        && mouse.column < r.x + r.width
                        && mouse.row >= r.y
                        && mouse.row < r.y + r.height
                    {
                        self.focused_field = Some(i);
                        return None;
                    }
                }
                None
            }
            MouseEventKind::ScrollUp => {
                if let Some(idx) = self.focused_field {
                    if self.fields[idx].spec.multiline && self.fields[idx].scroll_y > 0 {
                        self.fields[idx].scroll_y = self.fields[idx].scroll_y.saturating_sub(1);
                        return None;
                    }
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                None
            }
            MouseEventKind::ScrollDown => {
                if let Some(idx) = self.focused_field {
                    let f = &mut self.fields[idx];
                    let max_y = f.lines.len().saturating_sub(f.visible_rows());
                    if f.spec.multiline && f.scroll_y < max_y {
                        f.scroll_y = (f.scroll_y + 1).min(max_y);
                        return None;
                    }
                }
                self.scroll_offset = self.scroll_offset.saturating_add(2);
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
            for b in &self.blocks {
                match b {
                    MarkdownFormBlock::Text(t) => {
                        for line in t.lines() {
                            est_rows += super::input::wrap_prompt_text(line, avail_w.max(1)).len();
                        }
                        est_rows += 1;
                    }
                    MarkdownFormBlock::Input(_) => {
                        if let Some(f) = self.fields.get(field_idx) {
                            est_rows += f.visible_rows() + 1;
                        }
                        field_idx += 1;
                    }
                }
            }
            (est_rows as u16 + 6).min(max_h).max(10.min(max_h))
        };
        (dialog_w, dialog_h)
    }

    /// Renders the interactive Markdown form modal with full theme adherence.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let left_x = inner.x + 2;
        let avail_w = inner.width.saturating_sub(4) as usize;
        let content_top = inner.y + 1;
        let content_h = inner.height.saturating_sub(3) as usize;

        // Build virtual rows: either a styled text line or a reference to an input field row
        enum VirtualRow {
            Text(String, Style),
            InputRow {
                field_idx: usize,
                row_in_field: usize,
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
        for block in &self.blocks {
            match block {
                MarkdownFormBlock::Text(t) => {
                    for raw_line in t.lines() {
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
                }
                MarkdownFormBlock::Input(_) => {
                    let f_idx = field_counter;
                    field_counter += 1;
                    if let Some(f) = self.fields.get(f_idx) {
                        for r in 0..f.visible_rows() {
                            vrows.push(VirtualRow::InputRow {
                                field_idx: f_idx,
                                row_in_field: r,
                            });
                        }
                        vrows.push(VirtualRow::Blank);
                    }
                }
            }
        }

        let max_scroll = vrows.len().saturating_sub(content_h);
        let eff_scroll = self.scroll_offset.min(max_scroll);

        let mut field_rects = vec![Rect::default(); self.fields.len()];

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
                } => {
                    let f = &self.fields[*field_idx];
                    let is_focused = self.focused_field == Some(*field_idx);
                    let box_w = f
                        .spec
                        .width
                        .map(|w| (w as usize).min(avail_w).max(4))
                        .unwrap_or(avail_w) as u16;

                    if field_rects[*field_idx].width == 0 {
                        field_rects[*field_idx] =
                            Rect::new(left_x, y, box_w, f.visible_rows() as u16);
                    }

                    let box_style = if is_focused {
                        styles.input_box
                    } else {
                        styles.input_box_unfocused
                    };

                    // Fill background of this input row
                    for dx in 0..box_w {
                        buf[(left_x + dx, y)].set_char(' ').set_style(box_style);
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
                        buf.set_string(left_x, y, &visible, box_style);

                        if is_focused && line_idx == f.cursor_row {
                            let cur_vis_x = f.cursor_col.saturating_sub(h_scroll);
                            if cur_vis_x < text_w {
                                let cx = left_x + cur_vis_x as u16;
                                let cur_ch = chars.get(f.cursor_col).copied().unwrap_or(' ');
                                buf[(cx, y)]
                                    .set_char(cur_ch)
                                    .set_style(box_style.add_modifier(Modifier::REVERSED));
                            }
                        }
                    }

                    // Render per-field scrollbar on top row of multiline box
                    if show_sb && *row_in_field == 0 {
                        render_modal_scrollbar(
                            left_x + box_w.saturating_sub(1),
                            y,
                            f.visible_rows(),
                            f.lines.len(),
                            f.scroll_y,
                            styles,
                            buf,
                        );
                    }
                }
            }
        }

        *self.last_field_rects.borrow_mut() = field_rects;

        // Outer modal scrollbar when total virtual rows overflow
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

        // Render [ OK ] button
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let ok_str = format!(" [ {} ] ", t!("common.buttons.ok"));
        let ok_w = ok_str.chars().count() as u16;
        let center_x = inner.x + inner.width / 2;
        let ok_x = center_x.saturating_sub(ok_w / 2).max(inner.x + 1);
        let right_bound = inner.x + inner.width;
        if ok_x < right_bound {
            let max_ok = (right_bound - ok_x) as usize;
            let clipped_ok: String = ok_str.chars().take(max_ok).collect();
            let ok_style = if self.focused_field.is_none() {
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

        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bashmenu_core::parse_markdown_form_blocks;

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
        assert_eq!(modal.focused_field, Some(0));

        // Append to single-line field 0
        modal.handle_key_with_modifiers(KeyCode::Char('1'), KeyModifiers::NONE);
        // Tab to multiline field 1 and add a second line
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focused_field, Some(1));
        modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        modal.paste("Line 2");

        // Tab to [ OK ] and press Enter to submit
        modal.handle_key_with_modifiers(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(modal.focused_field, None);
        let action = modal.handle_key_with_modifiers(KeyCode::Enter, KeyModifiers::NONE);
        match action {
            Some(InteractiveFormAction::Submit(vals)) => {
                assert_eq!(vals.get("userName").map(String::as_str), Some("andrew1"));
                assert_eq!(
                    vals.get("userNotes").map(String::as_str),
                    Some("Line 1\nLine 2")
                );
            }
            other => panic!("expected Submit, got {other:?}"),
        }
    }
}
