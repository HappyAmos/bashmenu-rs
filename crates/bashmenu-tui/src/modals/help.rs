//! Scrollable markdown help modal.

use super::frame::render_modal_frame;
use super::note_viewer::wrap_footer_tokens;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Scrollable markdown / text help modal
#[derive(Debug, Clone)]
pub struct HelpModal {
    pub title: String,
    pub raw_content: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
}

impl HelpModal {
    pub fn new(title: impl Into<String>, content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        Self {
            title: title.into(),
            raw_content: content.to_string(),
            lines,
            scroll_offset: 0,
        }
    }

    /// Handles keyboard input. Returns `true` when dismissed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset += 1;
                false
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(15);
                false
            }
            KeyCode::PageDown => {
                self.scroll_offset += 15;
                false
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                false
            }
            KeyCode::End => {
                self.scroll_offset = self.lines.len().saturating_sub(10);
                false
            }
            _ => false,
        }
    }

    /// Handles mouse event
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.scroll_offset += 2;
                false
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                false
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let dialog_w = ((area.width as f32 * 0.90) as u16)
                    .min(area.width.saturating_sub(4))
                    .max(20);
                let dialog_h = (area.height.saturating_sub(2)).max(10);
                let inner = if area.width <= dialog_w {
                    area
                } else {
                    super::frame::compute_modal_inner(area, dialog_w, dialog_h)
                };
                super::frame::is_close_button_clicked(inner, mouse.row, mouse.column)
            }
            _ => false,
        }
    }

    /// Renders help modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        // Expand to 90% of screen width within margins
        let dialog_w = ((area.width as f32 * 0.90) as u16)
            .min(area.width.saturating_sub(4))
            .max(20);
        let dialog_h = (area.height.saturating_sub(2)).max(10);

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let footer_tokens = [
            "[UP/DN/PGUP/PGDN]: Scroll",
            "|",
            "[HOME/END]: Top/Bottom",
            "|",
            "[ESC/ENTER]: Close",
        ];
        let max_footer_w = inner.width.saturating_sub(4) as usize;
        let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
        let footer_h = footer_lines.len().max(1);

        let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
        let avail_w = inner.width.saturating_sub(4) as usize;

        // Collect wrapped lines with formatting tags
        let mut wrapped_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        let mut in_code_block = false;

        let raw_lines: Vec<&str> = self.raw_content.lines().collect();
        let mut line_idx = 0;

        while line_idx < raw_lines.len() {
            let line = raw_lines[line_idx];
            let trimmed = line.trim();

            // 1. Fenced code block delimiters
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code_block = !in_code_block;
                wrapped_lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(
                        line.to_string(),
                        Style::default()
                            .fg(Color::DarkGray)
                            .bg(styles.background)
                            .add_modifier(Modifier::DIM),
                    ),
                ]));
                line_idx += 1;
                continue;
            }

            // 2. Lines inside code block
            if in_code_block {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            wl,
                            Style::default().fg(Color::Yellow).bg(styles.background),
                        ),
                    ]));
                }
                line_idx += 1;
                continue;
            }

            // 3. Markdown Tables
            if is_table_row(trimmed)
                && line_idx + 1 < raw_lines.len()
                && is_table_delimiter(raw_lines[line_idx + 1].trim())
            {
                let mut table_rows: Vec<Vec<String>> = Vec::new();
                table_rows.push(parse_cells(trimmed));
                line_idx += 2; // skip header and delimiter

                while line_idx < raw_lines.len() && is_table_row(raw_lines[line_idx].trim()) {
                    table_rows.push(parse_cells(raw_lines[line_idx].trim()));
                    line_idx += 1;
                }

                render_table(&table_rows, avail_w, styles, &mut wrapped_lines);
                continue;
            }

            // 4. Section Headings & List Items
            if trimmed.starts_with("# ") {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            wl,
                            Style::default()
                                .fg(Color::Cyan)
                                .bg(styles.background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            wl,
                            styles
                                .accent
                                .bg(styles.background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            } else {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(wl, styles.text.bg(styles.background)),
                    ]));
                }
            }

            line_idx += 1;
        }

        let max_scroll = wrapped_lines.len().saturating_sub(content_h);
        let eff_scroll = self.scroll_offset.min(max_scroll);

        for (i, line) in wrapped_lines
            .iter()
            .skip(eff_scroll)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            buf.set_line(inner.x + 2, y, line, avail_w as u16);
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

/// Simple word wrapper for help modal text
fn wrap_help_text(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 || text.len() <= max_w {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_w {
            current.push(' ');
            current.push_str(word);
        } else {
            result.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

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

fn render_table(
    table_rows: &[Vec<String>],
    avail_w: usize,
    styles: &ResolvedThemeStyles,
    wrapped_lines: &mut Vec<ratatui::text::Line<'static>>,
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
            let len = row.get(c).map(|s| s.chars().count()).unwrap_or(0);
            *cw = (*cw).max(len);
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

    let border_style = Style::default().fg(Color::DarkGray).bg(styles.background);
    let header_style = Style::default()
        .fg(Color::Cyan)
        .bg(styles.background)
        .add_modifier(Modifier::BOLD);
    let code_col_style = styles
        .accent
        .bg(styles.background)
        .add_modifier(Modifier::BOLD);
    let text_col_style = styles.text.bg(styles.background);

    // 1. Top Border: ┌─────┬─────┐
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
    wrapped_lines.push(ratatui::text::Line::from(top_spans));

    // 2. Header Row: │ Header 1 │ Header 2 │
    let mut hdr_spans = Vec::new();
    hdr_spans.push(ratatui::text::Span::styled("│", border_style));
    for (c, &cw) in col_widths.iter().enumerate() {
        let cell_text = table_rows[0].get(c).map(|s| s.as_str()).unwrap_or("");
        let cell_vis = cell_text.chars().count();
        let pad = cw.saturating_sub(cell_vis);
        hdr_spans.push(ratatui::text::Span::styled(
            format!(" {}{} ", cell_text, " ".repeat(pad)),
            header_style,
        ));
        hdr_spans.push(ratatui::text::Span::styled("│", border_style));
    }
    wrapped_lines.push(ratatui::text::Line::from(hdr_spans));

    // 3. Header Separator: ├─────┼─────┤
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
    wrapped_lines.push(ratatui::text::Line::from(mid_spans));

    // 4. Data Rows
    for r in table_rows.iter().skip(1) {
        let cell_lines: Vec<Vec<String>> = (0..cols)
            .map(|c| {
                let cell_text = r.get(c).map(|s| s.as_str()).unwrap_or("");
                wrap_help_text(cell_text, col_widths[c])
            })
            .collect();
        let row_height = cell_lines.iter().map(|l| l.len()).max().unwrap_or(1);
        for h in 0..row_height {
            let mut row_spans = Vec::new();
            row_spans.push(ratatui::text::Span::styled("│", border_style));
            for c in 0..cols {
                let line_text = cell_lines[c].get(h).map(|s| s.as_str()).unwrap_or("");
                let cell_vis = line_text.chars().count();
                let pad = col_widths[c].saturating_sub(cell_vis);
                let cell_style = if c == 0 {
                    code_col_style
                } else {
                    text_col_style
                };
                row_spans.push(ratatui::text::Span::styled(
                    format!(" {}{} ", line_text, " ".repeat(pad)),
                    cell_style,
                ));
                row_spans.push(ratatui::text::Span::styled("│", border_style));
            }
            wrapped_lines.push(ratatui::text::Line::from(row_spans));
        }
    }

    // 5. Bottom Border: └─────┴─────┘
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
    wrapped_lines.push(ratatui::text::Line::from(bot_spans));
}
