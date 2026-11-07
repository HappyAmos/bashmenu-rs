//! Information / alert message modal dialog.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Simple message / alert modal dialog
#[derive(Debug, Clone)]
pub struct MessageModal {
    /// Dialog title header
    pub title: String,
    /// Alert or information message body
    pub message: String,
}

impl MessageModal {
    /// Constructs a new message modal with the given title and message.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    /// Handles keyboard input. Returns `true` when dismissed.
    pub fn handle_key(&self, code: KeyCode) -> bool {
        matches!(
            code,
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q')
        )
    }

    /// Handles immediate mouse click. Returns `true` when dismissed.
    pub fn handle_mouse(&self, mouse: MouseEvent, _inner_area: Rect) -> bool {
        mouse.kind == MouseEventKind::Down(MouseButton::Left)
    }

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let max_w = 64.min(area.width.saturating_sub(4));
        let wrapped =
            super::input::wrap_prompt_text(&self.message, max_w.saturating_sub(4) as usize);
        let max_line_w = wrapped
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(20);
        let dialog_w = (max_line_w as u16 + 8).min(max_w).max(20.min(max_w));
        let max_h = area.height.saturating_sub(2);
        let dialog_h = (wrapped.len() as u16 + 5).min(max_h).max(8.min(max_h));
        (dialog_w, dialog_h)
    }

    /// Renders message modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let max_w = 64.min(area.width.saturating_sub(4));
        let wrapped =
            super::input::wrap_prompt_text(&self.message, max_w.saturating_sub(4) as usize);
        let max_line_w = wrapped
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(20);
        let dialog_w = (max_line_w as u16 + 8).min(max_w).max(20.min(max_w));
        let max_h = area.height.saturating_sub(2);
        let dialog_h = (wrapped.len() as u16 + 5).min(max_h).max(8.min(max_h));

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render message (wrapped and clamped to visible inner area)
        for (i, line) in wrapped.iter().enumerate() {
            let y = inner.y + 1 + i as u16;
            if y < inner.y + inner.height.saturating_sub(2) {
                let clipped: String = line
                    .chars()
                    .take(inner.width.saturating_sub(4) as usize)
                    .collect();
                buf.set_string(inner.x + 2, y, &clipped, styles.text);
            }
        }

        // 2. Render [ OK ] button
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
