//! Formatted note viewer modal with interactive markdown checkboxes and scrolling.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Action triggered by user within the note viewer
#[derive(Debug, Clone, PartialEq)]
pub enum NoteViewerAction {
    /// No state transition
    None,
    /// Close the viewer modal
    Close,
    /// Request opening full text editor for note content
    Edit,
    /// Request renaming the current note
    Rename,
    /// Request deleting the current note
    Delete,
    /// Content was modified (e.g. checkbox toggled)
    ContentModified(String),
}

/// Formatted note viewer modal
#[derive(Debug, Clone)]
pub struct NoteViewerModal {
    pub title: String,
    pub filepath: String,
    pub content: String,
    pub show_completed: bool,
    pub scroll_offset: usize,
    pub selected_line: usize,
    pub follow_selection: bool,
    pub last_visible_raw_indices: Vec<usize>,
}

impl NoteViewerModal {
    pub fn new(title: impl Into<String>, filepath: impl Into<String>, content: &str) -> Self {
        Self {
            title: title.into(),
            filepath: filepath.into(),
            content: content.to_string(),
            show_completed: false,
            scroll_offset: 0,
            selected_line: 0,
            follow_selection: true,
            last_visible_raw_indices: Vec::new(),
        }
    }

    fn get_selectable_lines(&self) -> Vec<usize> {
        let raw_lines: Vec<&str> = self.content.lines().collect();
        let mut selectable = Vec::new();
        for (idx, line) in raw_lines.iter().enumerate() {
            if !self.show_completed && is_completed_checkbox(line) {
                continue;
            }
            selectable.push(idx);
        }
        selectable
    }

    /// Handles keyboard input
    pub fn handle_key(&mut self, code: KeyCode) -> NoteViewerAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => NoteViewerAction::Close,
            KeyCode::Char('e') | KeyCode::Char('E') => NoteViewerAction::Edit,
            KeyCode::Char('m') | KeyCode::Char('M') => NoteViewerAction::Rename,
            KeyCode::Char('d') | KeyCode::Char('D') => NoteViewerAction::Delete,
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.show_completed = !self.show_completed;
                let selectable = self.get_selectable_lines();
                if !selectable.contains(&self.selected_line) {
                    if let Some(&first) = selectable.first() {
                        self.selected_line = first;
                    }
                }
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                // Copy to terminal clipboard via OSC 52
                let b64 = base64_encode(self.content.as_bytes());
                print!("\x1b]52;c;{}\x07", b64);
                NoteViewerAction::None
            }
            KeyCode::Char(' ') => {
                // Toggle checkbox on the active line if selectable
                let selectable = self.get_selectable_lines();
                if selectable.contains(&self.selected_line) {
                    let lines: Vec<&str> = self.content.lines().collect();
                    if self.selected_line < lines.len() {
                        let line = lines[self.selected_line];
                        if let Some(toggled) = toggle_checkbox_line(line) {
                            let mut new_lines: Vec<String> =
                                lines.iter().map(|s| s.to_string()).collect();
                            new_lines[self.selected_line] = toggled;
                            self.content = new_lines.join("\n");
                            self.follow_selection = true;
                            return NoteViewerAction::ContentModified(self.content.clone());
                        }
                    }
                }
                NoteViewerAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let selectable = self.get_selectable_lines();
                if !selectable.is_empty() {
                    let cur_pos = selectable
                        .iter()
                        .position(|&x| x == self.selected_line)
                        .unwrap_or(0);
                    if cur_pos > 0 {
                        self.selected_line = selectable[cur_pos - 1];
                    }
                }
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let selectable = self.get_selectable_lines();
                if !selectable.is_empty() {
                    let cur_pos = selectable
                        .iter()
                        .position(|&x| x == self.selected_line)
                        .unwrap_or(0);
                    if cur_pos + 1 < selectable.len() {
                        self.selected_line = selectable[cur_pos + 1];
                    }
                }
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::PageUp => {
                let selectable = self.get_selectable_lines();
                if !selectable.is_empty() {
                    let cur_pos = selectable
                        .iter()
                        .position(|&x| x == self.selected_line)
                        .unwrap_or(0);
                    let target_pos = cur_pos.saturating_sub(15);
                    self.selected_line = selectable[target_pos];
                }
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::PageDown => {
                let selectable = self.get_selectable_lines();
                if !selectable.is_empty() {
                    let cur_pos = selectable
                        .iter()
                        .position(|&x| x == self.selected_line)
                        .unwrap_or(0);
                    let target_pos = (cur_pos + 15).min(selectable.len().saturating_sub(1));
                    self.selected_line = selectable[target_pos];
                }
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::Home => {
                let selectable = self.get_selectable_lines();
                self.selected_line = selectable.first().copied().unwrap_or(0);
                self.scroll_offset = 0;
                self.follow_selection = true;
                NoteViewerAction::None
            }
            KeyCode::End => {
                let selectable = self.get_selectable_lines();
                self.selected_line = selectable.last().copied().unwrap_or(0);
                self.follow_selection = true;
                NoteViewerAction::None
            }
            _ => NoteViewerAction::None,
        }
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> NoteViewerAction {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                let total_lines = if !self.last_visible_raw_indices.is_empty() {
                    self.last_visible_raw_indices.len()
                } else {
                    self.content.lines().count()
                };
                if self.scroll_offset + 2 < total_lines {
                    self.scroll_offset += 2;
                }
                self.follow_selection = false;
                NoteViewerAction::None
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                self.follow_selection = false;
                NoteViewerAction::None
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let dialog_w = (area.width.saturating_sub(4)).min(78);
                let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);
                let inner = if area.width <= dialog_w {
                    area
                } else {
                    super::frame::compute_modal_inner(area, dialog_w, dialog_h)
                };
                if super::frame::is_close_button_clicked(inner, mouse.row, mouse.column) {
                    return NoteViewerAction::Close;
                }

                let toggle_action = if self.show_completed {
                    "[H] Hide Done"
                } else {
                    "[H] Show Done"
                };
                let footer_tokens = [
                    "[SPACE] Toggle",
                    "|",
                    toggle_action,
                    "|",
                    "[E] Edit",
                    "|",
                    "[M] Rename",
                    "|",
                    "[D] Delete",
                    "|",
                    "[C] Copy",
                    "|",
                    "[ESC] Close",
                ];
                let max_footer_w = inner.width.saturating_sub(4) as usize;
                let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
                let footer_h = footer_lines.len().max(1);
                let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
                let content_start_y = inner.y + 1;
                let content_end_y = content_start_y + content_h as u16;

                let sb_x = inner.x + inner.width.saturating_sub(1);
                if mouse.column == sb_x {
                    if mouse.row >= content_start_y && mouse.row < content_end_y {
                        let total_lines = if !self.last_visible_raw_indices.is_empty() {
                            self.last_visible_raw_indices.len()
                        } else {
                            self.content.lines().count()
                        };
                        let max_scroll = total_lines.saturating_sub(content_h);
                        if mouse.row == content_start_y {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        } else if mouse.row == content_end_y - 1 {
                            self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
                        } else if content_h >= 3 && max_scroll > 0 {
                            let track_len = content_h - 2;
                            let rel_y = (mouse.row - (content_start_y + 1)) as usize;
                            self.scroll_offset = (rel_y * max_scroll) / track_len;
                        }
                        self.follow_selection = false;
                    }
                    return NoteViewerAction::None;
                }

                // Check footer button clicks
                let footer_start_y = inner.y + inner.height.saturating_sub(1 + footer_h as u16);
                if mouse.row >= footer_start_y && mouse.row < footer_start_y + footer_h as u16 {
                    let f_idx = (mouse.row - footer_start_y) as usize;
                    if f_idx < footer_lines.len() {
                        let f_line = &footer_lines[f_idx];
                        let footer_x =
                            inner.x + (inner.width.saturating_sub(f_line.len() as u16) / 2);
                        if mouse.column >= footer_x && mouse.column < footer_x + f_line.len() as u16
                        {
                            let rel_col = (mouse.column - footer_x) as usize;
                            let line_str = f_line.as_str();

                            if let Some(pos) = line_str.find("[SPACE]") {
                                if rel_col >= pos && rel_col < pos + 14 {
                                    return self.handle_key(KeyCode::Char(' '));
                                }
                            }
                            if let Some(pos) = line_str.find(toggle_action) {
                                if rel_col >= pos && rel_col < pos + toggle_action.len() {
                                    return self.handle_key(KeyCode::Char('h'));
                                }
                            }
                            if let Some(pos) = line_str.find("[E] Edit") {
                                if rel_col >= pos && rel_col < pos + 8 {
                                    return NoteViewerAction::Edit;
                                }
                            }
                            if let Some(pos) = line_str.find("[M] Rename") {
                                if rel_col >= pos && rel_col < pos + 10 {
                                    return NoteViewerAction::Rename;
                                }
                            }
                            if let Some(pos) = line_str.find("[D] Delete") {
                                if rel_col >= pos && rel_col < pos + 10 {
                                    return NoteViewerAction::Delete;
                                }
                            }
                            if let Some(pos) = line_str.find("[C] Copy") {
                                if rel_col >= pos && rel_col < pos + 8 {
                                    let b64 = base64_encode(self.content.as_bytes());
                                    print!("\x1b]52;c;{}\x07", b64);
                                    return NoteViewerAction::None;
                                }
                            }
                            if let Some(pos) = line_str.find("[ESC] Close") {
                                if rel_col >= pos && rel_col < pos + 11 {
                                    return NoteViewerAction::Close;
                                }
                            }
                        }
                    }
                    return NoteViewerAction::None;
                }

                // Check content area row clicks to select active line
                if mouse.row >= content_start_y
                    && mouse.row < content_end_y
                    && mouse.column >= inner.x + 2
                    && mouse.column < sb_x
                {
                    let vis_row = self.scroll_offset + (mouse.row - content_start_y) as usize;
                    if vis_row < self.last_visible_raw_indices.len() {
                        let clicked_raw = self.last_visible_raw_indices[vis_row];
                        self.selected_line = clicked_raw;
                        self.follow_selection = true;
                    }
                }

                NoteViewerAction::None
            }
            _ => NoteViewerAction::None,
        }
    }

    /// Renders note viewer modal with rendered markdown and code blocks
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (area.width.saturating_sub(4)).min(78);
        let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);

        let modal_title = format!("Note: {}", self.title);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &modal_title, styles, buf);

        let toggle_action = if self.show_completed {
            "[H] Hide Done"
        } else {
            "[H] Show Done"
        };
        let footer_tokens = [
            "[SPACE] Toggle",
            "|",
            toggle_action,
            "|",
            "[E] Edit",
            "|",
            "[M] Rename",
            "|",
            "[D] Delete",
            "|",
            "[C] Copy",
            "|",
            "[ESC] Close",
        ];
        let max_footer_w = inner.width.saturating_sub(4) as usize;
        let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
        let footer_h = footer_lines.len().max(1);

        let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
        let avail_w = inner.width.saturating_sub(4) as usize;

        // Build rendered visual lines
        let mut visible_entries: Vec<(usize, Line<'static>)> = Vec::new();

        if self.content.trim().is_empty() {
            let empty_span = Span::styled(
                "*(Empty note — press 'e' to edit content)*",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            );
            visible_entries.push((0, Line::from(vec![empty_span])));
        } else {
            let raw_lines: Vec<&str> = self.content.lines().collect();
            let mut in_code_block = false;

            for (idx, line) in raw_lines.iter().enumerate() {
                let trimmed = line.trim();

                // 1. Fenced Code Block Markers
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    if !in_code_block {
                        in_code_block = true;
                        let lang = trimmed.trim_start_matches(['`', '~']).trim();
                        let label = if lang.is_empty() { "Code" } else { lang };
                        let bar_len = avail_w.saturating_sub(label.len() + 7);
                        let top_str = format!("┌─ [{}] {}", label, "─".repeat(bar_len));
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled(
                                top_str,
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                            )]),
                        ));
                    } else {
                        in_code_block = false;
                        let bot_str = format!("└{}", "─".repeat(avail_w.saturating_sub(1)));
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled(
                                bot_str,
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                            )]),
                        ));
                    }
                    continue;
                }

                // 2. Lines Inside Fenced Code Block
                if in_code_block {
                    let code_w = avail_w.saturating_sub(3).max(10);
                    let chars: Vec<char> = line.chars().collect();
                    if chars.is_empty() {
                        let border_span = Span::styled(
                            "│",
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                        );
                        visible_entries.push((idx, Line::from(vec![border_span])));
                    } else {
                        let mut start = 0;
                        while start < chars.len() {
                            let end = (start + code_w).min(chars.len());
                            let chunk: String = chars[start..end].iter().collect();
                            let border_span = Span::styled(
                                "│ ",
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                            );
                            let code_span = Span::styled(chunk, Style::default().fg(Color::Yellow));
                            visible_entries.push((idx, Line::from(vec![border_span, code_span])));
                            start = end;
                        }
                    }
                    continue;
                }

                // 3. Skip completed checkboxes if hide-completed is active
                if !self.show_completed && is_completed_checkbox(line) {
                    continue;
                }

                // 4. Horizontal Rules
                if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                    visible_entries.push((
                        idx,
                        Line::from(vec![Span::styled("─".repeat(avail_w), styles.divider)]),
                    ));
                    continue;
                }

                // 5. Headings
                if let Some(h1) = trimmed.strip_prefix("# ") {
                    for wl in textwrap_simple(h1, avail_w.saturating_sub(2)) {
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled(
                                format!("# {}", wl),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            )]),
                        ));
                    }
                    continue;
                }
                if let Some(h2) = trimmed.strip_prefix("## ") {
                    for wl in textwrap_simple(h2, avail_w.saturating_sub(3)) {
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled(
                                format!("## {}", wl),
                                styles.accent.add_modifier(Modifier::BOLD),
                            )]),
                        ));
                    }
                    continue;
                }
                if let Some(h3) = trimmed.strip_prefix("### ") {
                    for wl in textwrap_simple(h3, avail_w.saturating_sub(4)) {
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled(
                                format!("### {}", wl),
                                styles.text.add_modifier(Modifier::BOLD),
                            )]),
                        ));
                    }
                    continue;
                }

                // 6. Blockquotes
                if let Some(bq) = trimmed.strip_prefix("> ") {
                    for wl in textwrap_simple(bq, avail_w.saturating_sub(3)) {
                        let bar = Span::styled(
                            "│ ",
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                        );
                        let mut spans = vec![bar];
                        spans.extend(parse_markdown_spans(
                            &wl,
                            styles.text.add_modifier(Modifier::ITALIC),
                        ));
                        visible_entries.push((idx, Line::from(spans)));
                    }
                    continue;
                }

                // 7. Checkboxes
                if let Some((indent_len, is_checked, rest)) = parse_checkbox_line(line) {
                    let margin = indent_len + 6;
                    let is_sel = idx == self.selected_line;
                    let (check_box_span, base_st) = if is_checked {
                        let cb_style = if is_sel {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        } else {
                            Style::default().fg(Color::Green)
                        };
                        (
                            Span::styled("- [x] ", cb_style),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        )
                    } else {
                        let cb_style = if is_sel {
                            styles.text
                        } else {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        };
                        (Span::styled("- [ ] ", cb_style), styles.text)
                    };

                    let wrapped = textwrap_simple(rest, avail_w.saturating_sub(margin));
                    if wrapped.is_empty() {
                        let mut spans = Vec::new();
                        if indent_len > 0 {
                            spans.push(Span::styled(" ".repeat(indent_len), base_st));
                        }
                        spans.push(check_box_span);
                        visible_entries.push((idx, Line::from(spans)));
                    } else {
                        for (w_idx, wl) in wrapped.iter().enumerate() {
                            let mut spans = Vec::new();
                            if w_idx == 0 {
                                if indent_len > 0 {
                                    spans.push(Span::styled(" ".repeat(indent_len), base_st));
                                }
                                spans.push(check_box_span.clone());
                            } else {
                                spans.push(Span::styled(" ".repeat(margin), base_st));
                            }
                            spans.extend(parse_markdown_spans(wl, base_st));
                            visible_entries.push((idx, Line::from(spans)));
                        }
                    }
                    continue;
                }

                // 8. Bullet Lists (- , * , + )
                if let Some(bullet_rest) = parse_bullet_line(line) {
                    let wrapped = textwrap_simple(bullet_rest, avail_w.saturating_sub(3));
                    if wrapped.is_empty() {
                        visible_entries.push((
                            idx,
                            Line::from(vec![Span::styled("• ", Style::default().fg(Color::Cyan))]),
                        ));
                    } else {
                        for (w_idx, wl) in wrapped.iter().enumerate() {
                            let mut spans = Vec::new();
                            if w_idx == 0 {
                                spans.push(Span::styled("• ", Style::default().fg(Color::Cyan)));
                            } else {
                                spans.push(Span::styled("  ", styles.text));
                            }
                            spans.extend(parse_markdown_spans(wl, styles.text));
                            visible_entries.push((idx, Line::from(spans)));
                        }
                    }
                    continue;
                }

                // 9. Standard Text Lines with Inline Markdown
                let wrapped = textwrap_simple(line, avail_w);
                if wrapped.is_empty() {
                    visible_entries.push((idx, Line::from(vec![Span::styled("", styles.text)])));
                } else {
                    for wl in wrapped {
                        let spans = parse_markdown_spans(&wl, styles.text);
                        visible_entries.push((idx, Line::from(spans)));
                    }
                }
            }
        }

        self.last_visible_raw_indices = visible_entries
            .iter()
            .map(|(raw_idx, _)| *raw_idx)
            .collect();

        // Follow selection when navigation or toggling occurred
        if self.follow_selection {
            let first_vis = visible_entries
                .iter()
                .position(|(raw_idx, _)| *raw_idx == self.selected_line);
            let last_vis = visible_entries
                .iter()
                .rposition(|(raw_idx, _)| *raw_idx == self.selected_line);

            if let (Some(first_idx), Some(last_idx)) = (first_vis, last_vis) {
                if content_h > 0 {
                    if last_idx >= self.scroll_offset + content_h {
                        self.scroll_offset = last_idx.saturating_sub(content_h.saturating_sub(1));
                    }
                    if first_idx < self.scroll_offset {
                        self.scroll_offset = first_idx;
                    }
                }
            }
            self.follow_selection = false;
        }

        let max_scroll = visible_entries.len().saturating_sub(content_h);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
        let eff_scroll = self.scroll_offset;

        // Render visible entries
        for (i, (raw_idx, line_obj)) in visible_entries
            .iter()
            .skip(eff_scroll)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let is_sel = *raw_idx == self.selected_line;

            let styled_line = Line::from(
                line_obj
                    .spans
                    .iter()
                    .map(|s| {
                        let mut st = s.style;
                        if st.bg.is_none() || st.bg == Some(Color::Reset) {
                            st = st.bg(styles.background);
                        }
                        if is_sel {
                            st = st.add_modifier(Modifier::REVERSED);
                        }
                        Span::styled(s.content.clone(), st)
                    })
                    .collect::<Vec<_>>(),
            );

            buf.set_line(inner.x + 2, y, &styled_line, avail_w as u16);
        }

        // Render scrollbar if note has more lines than window height (content_h)
        if visible_entries.len() > content_h && content_h > 0 {
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
                let total_lines = visible_entries.len();
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

        // Render wrapped footer help lines with equal spacing
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

/// Parses inline code backticks (`` `code` ``) and bold markers (`**bold**`) into styled spans
fn parse_markdown_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let parts: Vec<&str> = text.split('`').collect();

    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if idx % 2 == 1 {
            // Inline code inside backticks
            spans.push(Span::styled(
                part.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if part.contains("**") {
            let bold_parts: Vec<&str> = part.split("**").collect();
            for (b_idx, b_part) in bold_parts.iter().enumerate() {
                if b_part.is_empty() {
                    continue;
                }
                if b_idx % 2 == 1 {
                    spans.push(Span::styled(
                        b_part.to_string(),
                        base_style.add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(b_part.to_string(), base_style));
                }
            }
        } else {
            spans.push(Span::styled(part.to_string(), base_style));
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base_style));
    }
    spans
}

fn parse_checkbox_line(line: &str) -> Option<(usize, bool, &str)> {
    let indent_len = line.chars().take_while(|c| c.is_whitespace()).count();
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- [ ] ")
        .or_else(|| trimmed.strip_prefix("* [ ] "))
    {
        Some((indent_len, false, rest))
    } else if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
        .or_else(|| trimmed.strip_prefix("* [x] "))
        .or_else(|| trimmed.strip_prefix("* [X] "))
    {
        Some((indent_len, true, rest))
    } else {
        None
    }
}

fn parse_bullet_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        if !rest.starts_with("[ ]") && !rest.starts_with("[x]") && !rest.starts_with("[X]") {
            return Some(rest);
        }
    }
    None
}

/// Wraps footer tokens across lines with equal spacing, treating "|" as a word.
pub fn wrap_footer_tokens(tokens: &[&str], max_w: usize) -> Vec<String> {
    crate::gutter::wrap_spread_help_gutter(tokens, max_w, 2)
}

/// Checks whether a line contains a completed task list item `- [x]` or `- [X]`
fn is_completed_checkbox(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- [x]")
        || trimmed.starts_with("- [X]")
        || trimmed.starts_with("* [x]")
        || trimmed.starts_with("* [X]")
}

/// Toggles task list checkbox on a single line
fn toggle_checkbox_line(line: &str) -> Option<String> {
    if line.contains("- [ ]") {
        Some(line.replacen("- [ ]", "- [x]", 1))
    } else if line.contains("- [x]") {
        Some(line.replacen("- [x]", "- [ ]", 1))
    } else if line.contains("- [X]") {
        Some(line.replacen("- [X]", "- [ ]", 1))
    } else if line.contains("* [ ]") {
        Some(line.replacen("* [ ]", "* [x]", 1))
    } else if line.contains("* [x]") {
        Some(line.replacen("* [x]", "* [ ]", 1))
    } else if line.contains("* [X]") {
        Some(line.replacen("* [X]", "* [ ]", 1))
    } else {
        None
    }
}

/// Simple word wrapper for note view lines
fn textwrap_simple(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 || text.len() <= max_w {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();

    let push_word = |result: &mut Vec<String>, current: &mut String, word: &str| {
        if word.len() <= max_w {
            current.push_str(word);
        } else {
            let chars: Vec<char> = word.chars().collect();
            let mut start = 0;
            while start < chars.len() {
                let end = (start + max_w).min(chars.len());
                let chunk: String = chars[start..end].iter().collect();
                if end < chars.len() {
                    result.push(chunk);
                } else {
                    current.push_str(&chunk);
                }
                start = end;
            }
        }
    };

    for word in text.split_whitespace() {
        if current.is_empty() {
            push_word(&mut result, &mut current, word);
        } else if current.len() + 1 + word.len() <= max_w {
            current.push(' ');
            current.push_str(word);
        } else {
            result.push(current);
            current = String::new();
            push_word(&mut result, &mut current, word);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Minimal base64 encoder for OSC 52 clipboard sequences
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
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
