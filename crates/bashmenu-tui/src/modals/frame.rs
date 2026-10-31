//! Modal frame layout with classic drop-shadow overlays.

use crate::style::ResolvedThemeStyles;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

/// Calculates the inner `Rect` for a modal dialog given the viewport and target dimensions
pub fn compute_modal_inner(area: Rect, dialog_w: u16, dialog_h: u16) -> Rect {
    let w = dialog_w.min(area.width.saturating_sub(4));
    let h = dialog_h.min(area.height.saturating_sub(2)).min(28);

    let start_x = area.x + (area.width.saturating_sub(w + 2)) / 2;
    let start_y = area.y + (area.height.saturating_sub(h + 1)) / 2;

    Rect::new(
        start_x + 1,
        start_y + 1,
        w.saturating_sub(2),
        h.saturating_sub(2),
    )
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
    let h = dialog_h.min(area.height.saturating_sub(2)).min(28);

    let start_x = area.x + (area.width.saturating_sub(w + 2)) / 2;
    let start_y = area.y + (area.height.saturating_sub(h + 1)) / 2;

    let shadow_char = '░';
    let shadow_style = Style::default().fg(Color::DarkGray).bg(Color::Black);

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

    // 3. Clear Dialog Region & Render Border Block
    let dialog_rect = Rect::new(start_x, start_y, w, h);
    Clear.render(dialog_rect, buf);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(styles.border))
        .style(Style::default().bg(styles.background))
        .title(format!(" [ {} ] ", title))
        .title_style(styles.title.add_modifier(Modifier::BOLD));

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
