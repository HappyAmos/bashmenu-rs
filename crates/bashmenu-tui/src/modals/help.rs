//! Scrollable markdown help modal.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Scrollable markdown / text help modal
#[derive(Debug, Clone)]
pub struct HelpModal {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
}

impl HelpModal {
    pub fn new(title: impl Into<String>, content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        Self {
            title: title.into(),
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
                if self.scroll_offset + 1 < self.lines.len() {
                    self.scroll_offset += 1;
                }
                false
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(15);
                false
            }
            KeyCode::PageDown => {
                self.scroll_offset =
                    (self.scroll_offset + 15).min(self.lines.len().saturating_sub(1));
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
    pub fn handle_mouse(&mut self, mouse: MouseEvent, _inner_area: Rect) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.scroll_offset + 2 < self.lines.len() {
                    self.scroll_offset += 2;
                }
                false
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                false
            }
            _ => false,
        }
    }

    /// Renders help modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (area.width.saturating_sub(4)).min(78);
        let dialog_h = (area.height.saturating_sub(4)).clamp(12, 26);

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);
        let content_h = inner.height.saturating_sub(2) as usize;

        for (i, line) in self
            .lines
            .iter()
            .skip(self.scroll_offset)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let max_w = inner.width.saturating_sub(2) as usize;
            let display_text: String = line.chars().take(max_w).collect();
            buf.set_string(inner.x + 1, y, &display_text, styles.text);
        }

        // Render scroll footer
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let footer_text = format!(
            " [↑/↓/PgUp/PgDn] Line {}/{}  [Esc/q] Close ",
            self.scroll_offset + 1,
            self.lines.len().max(1)
        );
        let footer_x = (inner.x + inner.width).saturating_sub(footer_text.len() as u16 + 1);
        buf.set_string(
            footer_x.max(inner.x + 1),
            footer_y,
            &footer_text,
            styles.gutter.add_modifier(Modifier::DIM),
        );

        inner
    }
}
