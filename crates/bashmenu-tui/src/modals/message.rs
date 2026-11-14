//! Information / alert message modal dialog with markdown and BBCode support.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use std::cell::Cell;

/// Simple message / alert modal dialog
#[derive(Debug, Clone)]
pub struct MessageModal {
    /// Dialog title header
    pub title: String,
    /// Alert or information message body
    pub message: String,
    /// Optional window width as percentage of terminal width (1..=100)
    pub width_percent: Option<u16>,
    /// Optional window height as percentage of terminal height (1..=100)
    pub height_percent: Option<u16>,
    /// Vertical scroll offset in lines
    pub scroll_offset: usize,
    /// Maximum scroll offset recorded during the last layout/render pass
    last_max_scroll: Cell<Option<usize>>,
    /// Content viewport height recorded during the last layout/render pass
    last_content_h: Cell<Option<usize>>,
}

impl MessageModal {
    /// Constructs a new message modal with the given title and message.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            width_percent: None,
            height_percent: None,
            scroll_offset: 0,
            last_max_scroll: Cell::new(None),
            last_content_h: Cell::new(None),
        }
    }

    /// Sets optional percentage-based width and height overrides (1..=100).
    pub fn with_dimensions(
        mut self,
        width_percent: Option<u16>,
        height_percent: Option<u16>,
    ) -> Self {
        self.width_percent = width_percent.map(|w| w.clamp(1, 100));
        self.height_percent = height_percent.map(|h| h.clamp(1, 100));
        self
    }

    /// Handles keyboard input. Returns `true` when dismissed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => true,
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(max_s) = self.last_max_scroll.get() {
                    self.scroll_offset = self.scroll_offset.min(max_s);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(max_s) = self.last_max_scroll.get() {
                    self.scroll_offset = (self.scroll_offset.min(max_s) + 1).min(max_s);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                }
                false
            }
            KeyCode::PageUp => {
                let step = self.last_content_h.get().unwrap_or(10).max(1);
                if let Some(max_s) = self.last_max_scroll.get() {
                    self.scroll_offset = self.scroll_offset.min(max_s);
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(step);
                false
            }
            KeyCode::PageDown => {
                let step = self.last_content_h.get().unwrap_or(10).max(1);
                if let Some(max_s) = self.last_max_scroll.get() {
                    self.scroll_offset = self
                        .scroll_offset
                        .min(max_s)
                        .saturating_add(step)
                        .min(max_s);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_add(step);
                }
                false
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                false
            }
            KeyCode::End => {
                self.scroll_offset = self.last_max_scroll.get().unwrap_or(usize::MAX);
                false
            }
            _ => false,
        }
    }

    /// Handles mouse scroll and scrollbar clicks. Returns `true` when dismissed.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);
        let content_h = inner.height.saturating_sub(3) as usize;
        let avail_inner_w = inner.width.saturating_sub(4) as usize;
        let total_lines = parse_markdown_message_lines(&self.message, avail_inner_w, None).len();
        let max_scroll = total_lines.saturating_sub(content_h);
        self.last_max_scroll.set(Some(max_scroll));
        self.last_content_h.set(Some(content_h));

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.min(max_scroll).saturating_sub(2);
                false
            }
            MouseEventKind::ScrollDown => {
                self.scroll_offset = (self.scroll_offset.min(max_scroll) + 2).min(max_scroll);
                false
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if total_lines > content_h && content_h > 0 && inner.width >= 4 {
                    let sb_x = inner.x + inner.width.saturating_sub(1);
                    let content_start_y = inner.y + 1;
                    let content_end_y = content_start_y + content_h as u16;
                    if mouse.column == sb_x
                        && mouse.row >= content_start_y
                        && mouse.row < content_end_y
                    {
                        if content_h >= 3 {
                            if mouse.row == content_start_y {
                                self.scroll_offset =
                                    self.scroll_offset.min(max_scroll).saturating_sub(1);
                            } else if mouse.row == content_end_y - 1 {
                                self.scroll_offset =
                                    (self.scroll_offset.min(max_scroll) + 1).min(max_scroll);
                            } else if max_scroll > 0 {
                                let track_len = content_h - 2;
                                let rel_y = (mouse.row - (content_start_y + 1)) as usize;
                                self.scroll_offset = if track_len <= 1 {
                                    max_scroll
                                } else {
                                    (rel_y * max_scroll) / (track_len - 1)
                                }
                                .min(max_scroll);
                            }
                        } else if content_h == 2 {
                            if mouse.row == content_start_y {
                                self.scroll_offset =
                                    self.scroll_offset.min(max_scroll).saturating_sub(1);
                            } else {
                                self.scroll_offset =
                                    (self.scroll_offset.min(max_scroll) + 1).min(max_scroll);
                            }
                        }
                        return false;
                    }
                }
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if total_lines > content_h && content_h >= 3 && inner.width >= 4 && max_scroll > 0 {
                    let sb_x = inner.x + inner.width.saturating_sub(1);
                    let content_start_y = inner.y + 1;
                    let content_end_y = content_start_y + content_h as u16;
                    if mouse.column == sb_x
                        && mouse.row > content_start_y
                        && mouse.row < content_end_y - 1
                    {
                        let track_len = content_h - 2;
                        let rel_y = (mouse.row - (content_start_y + 1)) as usize;
                        self.scroll_offset = if track_len <= 1 {
                            max_scroll
                        } else {
                            (rel_y * max_scroll) / (track_len - 1)
                        }
                        .min(max_scroll);
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let max_w = if self.width_percent.is_some() {
            area.width.saturating_sub(4).max(20.min(area.width))
        } else {
            (area.width.saturating_sub(4)).clamp(20, 96)
        };
        let dialog_w = if let Some(wp) = self.width_percent {
            let target_w = ((area.width as u32 * wp.clamp(1, 100) as u32) / 100) as u16;
            target_w.clamp(20.min(max_w), max_w)
        } else {
            let avail_inner_w = max_w.saturating_sub(4) as usize;
            let lines = parse_markdown_message_lines(&self.message, avail_inner_w, None);
            let max_line_w = lines
                .iter()
                .map(|l| {
                    l.spans
                        .iter()
                        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                        .sum::<usize>()
                })
                .max()
                .unwrap_or(20);
            (max_line_w as u16 + 6).min(max_w).max(20.min(max_w))
        };

        let max_h = area.height.saturating_sub(2);
        let dialog_h = if let Some(hp) = self.height_percent {
            let target_h = ((area.height as u32 * hp.clamp(1, 100) as u32) / 100) as u16;
            target_h.clamp(8.min(max_h), max_h)
        } else {
            let avail_inner_w = dialog_w.saturating_sub(6) as usize;
            let lines = parse_markdown_message_lines(&self.message, avail_inner_w, None);
            (lines.len() as u16 + 5).min(max_h).max(8.min(max_h))
        };
        (dialog_w, dialog_h)
    }

    /// Renders message modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let (dialog_w, dialog_h) = self.dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let avail_inner_w = inner.width.saturating_sub(4) as usize;
        let lines = parse_markdown_message_lines(&self.message, avail_inner_w, Some(styles));
        let content_h = inner.height.saturating_sub(3) as usize;
        let total_lines = lines.len();
        let max_scroll = total_lines.saturating_sub(content_h);
        let eff_scroll = self.scroll_offset.min(max_scroll);

        self.last_max_scroll.set(Some(max_scroll));
        self.last_content_h.set(Some(content_h));

        // 1. Render message (wrapped, scrolled, and clamped to visible inner area)
        let avail_line_w = inner.width.saturating_sub(4);
        for (i, line) in lines.iter().skip(eff_scroll).take(content_h).enumerate() {
            let y = inner.y + 1 + i as u16;
            buf.set_line(inner.x + 2, y, line, avail_line_w);
        }

        // 2. Render scrollbar if message exceeds visible window height
        if total_lines > content_h && content_h > 0 && inner.width >= 4 {
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
                buf.set_string(sb_x, content_start_y, "▲", arrow_style);
                buf.set_string(
                    sb_x,
                    content_start_y + content_h as u16 - 1,
                    "▼",
                    arrow_style,
                );

                let track_len = content_h - 2;
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

        // 3. Render [ OK ] button
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;
        let ok_x = center_x.saturating_sub(4).max(inner.x + 1);
        let right_bound = inner.x + inner.width;

        if ok_x < right_bound {
            let ok_str = " [ OK ] ";
            let max_ok = (right_bound - ok_x) as usize;
            let clipped_ok: String = ok_str.chars().take(max_ok).collect();
            buf.set_string(
                ok_x,
                btn_y,
                &clipped_ok,
                styles.highlight.add_modifier(Modifier::BOLD),
            );
        }

        inner
    }
}

/// Checks whether a text line starts and ends with pipes (table row).
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

/// Checks whether a markdown table row is a delimiter line (e.g. `|---|`).
fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return false;
    }
    let parts: Vec<&str> = trimmed.split('|').collect();
    if parts.len() < 3 {
        return false;
    }
    parts[1..parts.len() - 1].iter().all(|c| {
        let s = c.trim();
        !s.is_empty() && s.chars().all(|ch| ch == '-' || ch == ':') && s.contains('-')
    })
}

/// Extracts trimmed text cells from between pipe delimiters of a row.
fn parse_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split('|').collect();
    if parts.len() >= 2 {
        parts[1..parts.len() - 1]
            .iter()
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    }
}

/// Wraps cell text chunking long strings without spaces when needed.
fn wrap_cell_text(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return Vec::new();
    }
    let stripped = crate::bbcode::strip_bbcode(text);
    let vis_w = unicode_width::UnicodeWidthStr::width(stripped.as_str());
    if vis_w <= max_w {
        return vec![text.to_string()];
    }
    if text.contains(' ') {
        super::input::wrap_prompt_text(text, max_w)
    } else {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let end = (i + max_w).min(chars.len());
            chunks.push(chars[i..end].iter().collect());
            i = end;
        }
        chunks
    }
}

/// Renders parsed markdown table rows into formatted box-drawing Lines.
fn render_table(
    table_rows: &[Vec<String>],
    avail_w: usize,
    styles: Option<&ResolvedThemeStyles>,
    out_lines: &mut Vec<ratatui::text::Line<'static>>,
) {
    if table_rows.is_empty() {
        return;
    }
    let cols = table_rows[0].len();
    if cols == 0 {
        return;
    }

    let mut col_widths = vec![0usize; cols];
    for row in table_rows {
        for (c, cw) in col_widths.iter_mut().enumerate() {
            let cell = row.get(c).map(|s| s.as_str()).unwrap_or("");
            let vis_len =
                unicode_width::UnicodeWidthStr::width(crate::bbcode::strip_bbcode(cell).as_str());
            *cw = (*cw).max(vis_len);
        }
    }

    // Shrink columns if table exceeds available width
    let overhead = cols * 3 + 1;
    let max_text_w = avail_w.saturating_sub(overhead);
    if cols > 0 && max_text_w >= cols * 4 {
        while col_widths.iter().sum::<usize>() > max_text_w {
            let (max_c, &max_val) = col_widths
                .iter()
                .enumerate()
                .max_by_key(|(_, &w)| w)
                .unwrap();
            if max_val <= 4 {
                break;
            }
            col_widths[max_c] -= 1;
        }
    }

    let border_style = styles
        .map(|s| Style::default().fg(s.border).bg(s.background))
        .unwrap_or_else(|| Style::default().fg(Color::DarkGray));
    let header_style = styles
        .map(|s| s.accent.bg(s.background).add_modifier(Modifier::BOLD))
        .unwrap_or_else(|| {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        });
    let col0_style = styles
        .map(|s| s.accent.bg(s.background).add_modifier(Modifier::BOLD))
        .unwrap_or_else(|| {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        });
    let col_text_style = styles.map(|s| s.text.bg(s.background)).unwrap_or_default();

    // 1. Top border: ┌──────┬──────┐
    let mut top_spans = Vec::new();
    top_spans.push(ratatui::text::Span::styled("┌", border_style));
    for (c, &cw) in col_widths.iter().enumerate() {
        top_spans.push(ratatui::text::Span::styled(
            "─".repeat(cw + 2),
            border_style,
        ));
        if c + 1 < cols {
            top_spans.push(ratatui::text::Span::styled("┬", border_style));
        }
    }
    top_spans.push(ratatui::text::Span::styled("┐", border_style));
    out_lines.push(ratatui::text::Line::from(top_spans));

    // 2. Header row: │ Header 1 │ Header 2 │
    let mut hdr_spans = Vec::new();
    hdr_spans.push(ratatui::text::Span::styled("│", border_style));
    for (c, &cw) in col_widths.iter().enumerate() {
        let cell_text = table_rows[0].get(c).map(|s| s.as_str()).unwrap_or("");
        hdr_spans.push(ratatui::text::Span::styled(" ", header_style));
        let mut spans = crate::bbcode::parse_bbcode_spans(cell_text, header_style);
        let cell_vis =
            unicode_width::UnicodeWidthStr::width(crate::bbcode::strip_bbcode(cell_text).as_str());
        let pad = cw.saturating_sub(cell_vis);
        spans.push(ratatui::text::Span::styled(
            " ".repeat(pad + 1),
            header_style,
        ));
        hdr_spans.extend(spans);
        hdr_spans.push(ratatui::text::Span::styled("│", border_style));
    }
    out_lines.push(ratatui::text::Line::from(hdr_spans));

    // 3. Header separator: ├──────┼──────┤
    let mut mid_spans = Vec::new();
    mid_spans.push(ratatui::text::Span::styled("├", border_style));
    for (c, &cw) in col_widths.iter().enumerate() {
        mid_spans.push(ratatui::text::Span::styled(
            "─".repeat(cw + 2),
            border_style,
        ));
        if c + 1 < cols {
            mid_spans.push(ratatui::text::Span::styled("┼", border_style));
        }
    }
    mid_spans.push(ratatui::text::Span::styled("┤", border_style));
    out_lines.push(ratatui::text::Line::from(mid_spans));

    // 4. Data rows
    for r in table_rows.iter().skip(1) {
        let cell_lines: Vec<Vec<String>> = (0..cols)
            .map(|c| {
                let cell_text = r.get(c).map(|s| s.as_str()).unwrap_or("");
                wrap_cell_text(cell_text, col_widths[c])
            })
            .collect();
        let row_height = cell_lines.iter().map(|l| l.len()).max().unwrap_or(1);
        for h in 0..row_height {
            let mut row_spans = Vec::new();
            row_spans.push(ratatui::text::Span::styled("│", border_style));
            for c in 0..cols {
                let line_text = cell_lines[c].get(h).map(|s| s.as_str()).unwrap_or("");
                let cell_style = if c == 0 { col0_style } else { col_text_style };
                row_spans.push(ratatui::text::Span::styled(" ", cell_style));
                let mut spans = crate::bbcode::parse_bbcode_spans(line_text, cell_style);
                let cell_vis = unicode_width::UnicodeWidthStr::width(
                    crate::bbcode::strip_bbcode(line_text).as_str(),
                );
                let pad = col_widths[c].saturating_sub(cell_vis);
                spans.push(ratatui::text::Span::styled(" ".repeat(pad + 1), cell_style));
                row_spans.extend(spans);
                row_spans.push(ratatui::text::Span::styled("│", border_style));
            }
            out_lines.push(ratatui::text::Line::from(row_spans));
        }
    }

    // 5. Bottom border: └──────┴──────┘
    let mut bot_spans = Vec::new();
    bot_spans.push(ratatui::text::Span::styled("└", border_style));
    for (c, &cw) in col_widths.iter().enumerate() {
        bot_spans.push(ratatui::text::Span::styled(
            "─".repeat(cw + 2),
            border_style,
        ));
        if c + 1 < cols {
            bot_spans.push(ratatui::text::Span::styled("┴", border_style));
        }
    }
    bot_spans.push(ratatui::text::Span::styled("┘", border_style));
    out_lines.push(ratatui::text::Line::from(bot_spans));
}

/// Parses message string into styled Lines, rendering headings and tables.
fn parse_markdown_message_lines(
    message: &str,
    avail_w: usize,
    styles: Option<&ResolvedThemeStyles>,
) -> Vec<ratatui::text::Line<'static>> {
    let mut out_lines = Vec::new();
    let raw_lines: Vec<&str> = message.lines().collect();
    let mut idx = 0;

    let h1_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let h2_style = styles
        .map(|s| s.accent.bg(s.background).add_modifier(Modifier::BOLD))
        .unwrap_or_else(|| {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        });
    let h3_style = styles
        .map(|s| s.text.bg(s.background).add_modifier(Modifier::BOLD))
        .unwrap_or_else(|| Style::default().add_modifier(Modifier::BOLD));
    let base_text_style = styles.map(|s| s.text.bg(s.background)).unwrap_or_default();

    while idx < raw_lines.len() {
        let line = raw_lines[idx];
        let trimmed = line.trim();

        // 1. Table Detection
        if is_table_row(trimmed) {
            let mut table_rows: Vec<Vec<String>> = Vec::new();
            while idx < raw_lines.len() && is_table_row(raw_lines[idx]) {
                let row_trimmed = raw_lines[idx].trim();
                if !is_table_delimiter(row_trimmed) {
                    table_rows.push(parse_cells(row_trimmed));
                }
                idx += 1;
            }
            render_table(&table_rows, avail_w, styles, &mut out_lines);
            continue;
        }

        // 2. Headings
        if trimmed.starts_with("# ") {
            let spans = crate::bbcode::parse_bbcode_spans(trimmed, h1_style);
            out_lines.push(ratatui::text::Line::from(spans));
        } else if trimmed.starts_with("## ") {
            let spans = crate::bbcode::parse_bbcode_spans(trimmed, h2_style);
            out_lines.push(ratatui::text::Line::from(spans));
        } else if trimmed.starts_with("### ") {
            let spans = crate::bbcode::parse_bbcode_spans(trimmed, h3_style);
            out_lines.push(ratatui::text::Line::from(spans));
        } else if trimmed.is_empty() {
            out_lines.push(ratatui::text::Line::default());
        } else {
            let wrapped = super::input::wrap_prompt_text(line, avail_w);
            for wl in wrapped {
                let spans = crate::bbcode::parse_bbcode_spans(&wl, base_text_style);
                out_lines.push(ratatui::text::Line::from(spans));
            }
        }

        idx += 1;
    }

    out_lines
}
