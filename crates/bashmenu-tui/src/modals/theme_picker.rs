//! Interactive theme selector modal.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Interactive theme selector modal dialog
#[derive(Debug, Clone)]
pub struct ThemePickerModal {
    pub themes: Vec<String>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
}

impl ThemePickerModal {
    pub fn new(mut themes: Vec<String>, current_theme: &str) -> Self {
        themes.sort();
        let selected_idx = themes.iter().position(|t| t == current_theme).unwrap_or(0);
        Self {
            themes,
            selected_idx,
            scroll_offset: selected_idx.saturating_sub(5),
        }
    }

    /// Handles keyboard input.
    /// Returns:
    /// - `None` while pending
    /// - `Some(Some(theme_name))` when chosen with Enter
    /// - `Some(None)` when cancelled with Esc
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<String>> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => Some(None),
            KeyCode::Enter => {
                if !self.themes.is_empty() {
                    Some(Some(self.themes[self.selected_idx].clone()))
                } else {
                    Some(None)
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_idx = self.selected_idx.saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_idx + 1 < self.themes.len() {
                    self.selected_idx += 1;
                }
                None
            }
            KeyCode::Home => {
                self.selected_idx = 0;
                None
            }
            KeyCode::End => {
                if !self.themes.is_empty() {
                    self.selected_idx = self.themes.len() - 1;
                }
                None
            }
            KeyCode::PageUp => {
                self.selected_idx = self.selected_idx.saturating_sub(8);
                None
            }
            KeyCode::PageDown => {
                if !self.themes.is_empty() {
                    self.selected_idx = (self.selected_idx + 8).min(self.themes.len() - 1);
                }
                None
            }
            _ => None,
        }
    }

    /// Handles mouse event
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<String>> {
        let dialog_w = 40.min(area.width.saturating_sub(4));
        let dialog_h = 16.min(area.height.saturating_sub(2));
        let inner_area = if area.width <= dialog_w {
            area
        } else {
            super::frame::compute_modal_inner(area, dialog_w, dialog_h)
        };

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.selected_idx + 1 < self.themes.len() {
                    self.selected_idx += 1;
                }
                None
            }
            MouseEventKind::ScrollUp => {
                self.selected_idx = self.selected_idx.saturating_sub(1);
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let row = mouse.row;
                let col = mouse.column;
                if super::frame::is_close_button_clicked(inner_area, row, col) {
                    return Some(None);
                }
                if row > inner_area.y && row < inner_area.y + inner_area.height.saturating_sub(1) {
                    let clicked_row = (row - (inner_area.y + 1)) as usize;
                    let target_idx = self.scroll_offset + clicked_row;
                    if target_idx < self.themes.len() {
                        self.selected_idx = target_idx;
                        return Some(Some(self.themes[target_idx].clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Renders theme picker modal
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 46.min(area.width.saturating_sub(4));
        let dialog_h = 16.min(area.height.saturating_sub(4)).max(8);

        let inner = render_modal_frame(area, dialog_w, dialog_h, "Theme Selector", styles, buf);
        let list_h = inner.height.saturating_sub(2) as usize;

        // Keep scroll offset tracking selected index
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + list_h {
            self.scroll_offset = self.selected_idx.saturating_sub(list_h - 1);
        }

        for (i, t_name) in self
            .themes
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(list_h)
        {
            let y = inner.y + 1 + (i - self.scroll_offset) as u16;
            let is_sel = i == self.selected_idx;
            let marker = "   ";
            let row_style = if is_sel {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };
            let avail = inner.width.saturating_sub(6) as usize;
            let padded_name = format!("{marker}{:<width$}", t_name, width = avail);
            buf.set_string(inner.x + 1, y, &padded_name, row_style);
        }

        // Footer
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let footer_text = " [Enter] Select  [Esc] Cancel ";
        let footer_x = (inner.x + inner.width).saturating_sub(footer_text.len() as u16 + 1);
        buf.set_string(
            footer_x.max(inner.x + 1),
            footer_y,
            footer_text,
            styles.gutter.add_modifier(Modifier::DIM),
        );

        inner
    }
}
