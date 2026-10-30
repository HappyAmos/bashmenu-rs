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
    pub title: String,
    pub message: String,
}

impl MessageModal {
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

    /// Renders message modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (self.message.len() as u16 + 8).max(40);
        let dialog_h = 8;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render message
        let msg_y = inner.y + 1;
        let msg_x = inner.x + 2;
        buf.set_string(msg_x, msg_y, &self.message, styles.text);

        // 2. Render [ OK ] button
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let center_x = inner.x + inner.width / 2;
        let ok_x = center_x.saturating_sub(4);

        buf.set_string(
            ok_x,
            btn_y,
            " [ OK ] ",
            styles.highlight.add_modifier(Modifier::BOLD),
        );

        inner
    }
}
