//! Modal frame layout with classic drop-shadow overlays.

use crate::style::ResolvedThemeStyles;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};

/// Calculates the outer `Rect` for a modal dialog box within the viewport.
pub fn compute_modal_rect(area: Rect, dialog_w: u16, dialog_h: u16) -> Rect {
    let w = dialog_w.min(area.width.saturating_sub(4));
    let h = dialog_h.min(area.height.saturating_sub(2));

    let start_x = area.x + (area.width.saturating_sub(w + 2)) / 2;
    let start_y = area.y + (area.height.saturating_sub(h + 1)) / 2;

    Rect::new(start_x, start_y, w, h)
}

/// Calculates the inner `Rect` for a modal dialog given viewport and dimensions.
pub fn compute_modal_inner(area: Rect, dialog_w: u16, dialog_h: u16) -> Rect {
    let rect = compute_modal_rect(area, dialog_w, dialog_h);
    Rect::new(
        rect.x + 1,
        rect.y + 1,
        rect.width.saturating_sub(2),
        rect.height.saturating_sub(2),
    )
}

/// Renders classic 2-column right and 1-row bottom drop shadow around `box_rect`.
pub fn draw_box_drop_shadow(
    area: Rect,
    box_rect: Rect,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) {
    let shadow_char = styles.borders.shadow_char.chars().next().unwrap_or('░');
    let shadow_style = styles.shadow;
    let start_x = box_rect.x;
    let start_y = box_rect.y;
    let w = box_rect.width;
    let h = box_rect.height;

    // 1. Right Shadow (2 columns wide, starts at start_y + 1)
    let right_shadow_x = start_x + w;
    for y in (start_y + 1)..(start_y + h + 1) {
        for dx in 0..2 {
            let px = right_shadow_x + dx;
            if px < area.x + area.width && y < area.y + area.height {
                buf[(px, y)].set_char(shadow_char).set_style(shadow_style);
            }
        }
    }

    // 2. Bottom Shadow (1 row high, starts at start_x + 2)
    let bottom_shadow_y = start_y + h;
    for px in (start_x + 2)..(start_x + w + 2) {
        if px < area.x + area.width && bottom_shadow_y < area.y + area.height {
            buf[(px, bottom_shadow_y)]
                .set_char(shadow_char)
                .set_style(shadow_style);
        }
    }
}

/// Renders an intermediate animated modal frame with drop shadow into `buf`.
pub fn render_animated_modal_frame(
    area: Rect,
    box_rect: Rect,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) {
    if box_rect.width < 3 || box_rect.height < 2 {
        return;
    }

    draw_box_drop_shadow(area, box_rect, styles, buf);

    // 3. Clear Dialog Region & Render Border Block
    Clear.render(box_rect, buf);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(styles.border))
        .style(Style::default().bg(styles.background));

    block.render(box_rect, buf);
}

/// Renders a centered modal dialog box with classic drop-shadow frame into `buf`.
/// Returns the inner `Rect` available for modal content.
///
/// Invariant:
/// - 2-cell right shadow (`start_y + 1` to `start_y + box_h`)
/// - 1-row bottom shadow (`start_x + 2` to `start_x + box_w + 2`)
/// - Shadow character: `░`
pub fn render_modal_frame(
    area: Rect,
    dialog_w: u16,
    dialog_h: u16,
    title: &str,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) -> Rect {
    // Clamp to viewport size within shadow margins
    let w = dialog_w.min(area.width.saturating_sub(4));
    let h = dialog_h.min(area.height.saturating_sub(2));

    let start_x = area.x + (area.width.saturating_sub(w + 2)) / 2;
    let start_y = area.y + (area.height.saturating_sub(h + 1)) / 2;
    let dialog_rect = Rect::new(start_x, start_y, w, h);

    draw_box_drop_shadow(area, dialog_rect, styles, buf);

    // 3. Clear Dialog Region & Render Border Block
    Clear.render(dialog_rect, buf);

    let clean_title = title.trim();
    let clean_title = clean_title
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    // Guard title length against window width to prevent collision with [X]
    let max_title_w = (w as usize).saturating_sub(10);
    let mut title_str = String::new();
    if max_title_w > 0 && !clean_title.is_empty() {
        let cur_w = unicode_width::UnicodeWidthStr::width(clean_title);
        if cur_w <= max_title_w {
            title_str = format!("[ {} ]", clean_title);
        } else if max_title_w > 1 {
            let mut clipped = String::new();
            let mut w_count = 0;
            for ch in clean_title.chars() {
                let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                if w_count + cw + 1 > max_title_w {
                    break;
                }
                clipped.push(ch);
                w_count += cw;
            }
            clipped.push('…');
            title_str = format!("[ {} ]", clipped);
        }
    }

    let border_style = Style::default().fg(styles.border);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(styles.background));

    if !title_str.is_empty() {
        block = block.title(Line::from(vec![
            Span::styled("─", border_style),
            Span::styled(title_str, styles.title.add_modifier(Modifier::BOLD)),
        ]));
    }

    let inner = block.inner(dialog_rect);
    block.render(dialog_rect, buf);

    // 4. Render Close Button [X] just to the left of top-right border corner
    if w >= 8 {
        let close_x = start_x + w - 4;
        let border_st = Style::default().fg(styles.border).bg(styles.background);
        let mut close_st = styles.window_close_button;
        if close_st.bg.is_none() || close_st.bg == Some(Color::Reset) {
            close_st = close_st.bg(styles.background);
        }
        buf[(close_x, start_y)].set_char('[').set_style(border_st);
        buf[(close_x + 1, start_y)]
            .set_char('X')
            .set_style(close_st);
        buf[(close_x + 2, start_y)]
            .set_char(']')
            .set_style(border_st);
    }

    inner
}

/// Returns true if the mouse click was on the [X] close button for a modal dialog with inner rect `inner`
pub fn is_close_button_clicked(inner: Rect, mouse_row: u16, mouse_col: u16) -> bool {
    let top_y = inner.y.saturating_sub(1);
    let right_x = inner.x + inner.width;
    mouse_row == top_y
        && mouse_col >= right_x.saturating_sub(3)
        && mouse_col <= right_x.saturating_sub(1)
}

/// Renders a vertical modal scrollbar at column `sb_x` spanning `start_y..start_y+content_h`.
pub(crate) fn render_modal_scrollbar(
    sb_x: u16,
    start_y: u16,
    content_h: usize,
    total_lines: usize,
    scroll_offset: usize,
    styles: &ResolvedThemeStyles,
    buf: &mut Buffer,
) {
    if total_lines <= content_h || content_h == 0 {
        return;
    }

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

    let max_scroll = total_lines.saturating_sub(content_h);
    let eff_scroll = scroll_offset.min(max_scroll);

    if content_h >= 3 {
        buf.set_string(sb_x, start_y, "▲", arrow_style);
        buf.set_string(sb_x, start_y + content_h as u16 - 1, "▼", arrow_style);

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
            let y = start_y + 1 + t_idx as u16;
            if t_idx >= thumb_start && t_idx < thumb_start + thumb_size {
                buf.set_string(sb_x, y, "█", thumb_style);
            } else {
                buf.set_string(sb_x, y, "░", track_style);
            }
        }
    } else if content_h == 2 {
        buf.set_string(sb_x, start_y, "▲", arrow_style);
        buf.set_string(sb_x, start_y + 1, "▼", arrow_style);
    } else {
        buf.set_string(sb_x, start_y, "█", thumb_style);
    }
}

/// Returns `true` if `line` is a pipe-delimited Markdown table row.
pub(crate) fn is_markdown_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

/// Returns `true` if `line` is a Markdown table delimiter row (e.g. `|---|---|`).
pub(crate) fn is_markdown_table_delimiter(line: &str) -> bool {
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

/// Extracts trimmed cell strings from a pipe-delimited Markdown table row,
/// treating escaped `\|` as a literal `|` inside a cell rather than a column separator.
pub(crate) fn parse_markdown_table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner
        .strip_suffix('|')
        .filter(|s| !s.ends_with('\\'))
        .unwrap_or(inner);

    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            cur.push('|');
        } else if ch == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

/// Iteratively shrinks the widest columns until total width fits in `avail_w`.
pub(crate) fn shrink_table_columns(col_widths: &mut [usize], avail_w: usize) {
    let cols = col_widths.len();
    let overhead = cols * 3 + 1;
    let max_text_w = avail_w.saturating_sub(overhead);
    if cols > 0 && max_text_w >= cols * 4 {
        while col_widths.iter().sum::<usize>() > max_text_w {
            let Some((max_c, &max_val)) = col_widths.iter().enumerate().max_by_key(|(_, &w)| w)
            else {
                break;
            };
            if max_val <= 4 {
                break;
            }
            col_widths[max_c] -= 1;
        }
    }
}
