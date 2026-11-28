//! Interactive theme selector modal.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use bashmenu_core::t;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// Interactive theme selector modal dialog
#[derive(Debug, Clone)]
pub struct ThemePickerModal {
    /// Alphabetically sorted list of available theme names
    pub themes: Vec<String>,
    /// Index of currently highlighted theme
    pub selected_idx: usize,
    /// Vertical scroll offset in lines
    pub scroll_offset: usize,
    /// Currently configured theme name for selection restoration
    pub current_theme: String,
}

impl ThemePickerModal {
    /// Constructs a theme picker modal with alphabetically sorted theme names.
    pub fn new(mut themes: Vec<String>, current_theme: &str) -> Self {
        Self::sort_themes(&mut themes);
        let selected_idx = themes
            .iter()
            .position(|t| t.eq_ignore_ascii_case(current_theme))
            .unwrap_or(0);
        Self {
            themes,
            selected_idx,
            scroll_offset: selected_idx.saturating_sub(5),
            current_theme: current_theme.to_string(),
        }
    }

    /// Builder method to ensure the `"[ Random ]"` option is present at the top
    /// of the theme list.
    pub fn with_random_option(mut self) -> Self {
        self.themes
            .retain(|t| !bashmenu_core::is_random_theme_spec(t));
        Self::sort_themes(&mut self.themes);
        self.themes
            .insert(0, bashmenu_core::RANDOM_THEME_LABEL.to_string());
        if bashmenu_core::is_random_theme_spec(&self.current_theme) {
            self.selected_idx = 0;
            self.scroll_offset = 0;
        } else if let Some(pos) = self
            .themes
            .iter()
            .position(|t| t.eq_ignore_ascii_case(&self.current_theme))
        {
            self.selected_idx = pos;
            self.scroll_offset = pos.saturating_sub(5);
        }
        self
    }

    /// Sorts theme names alphabetically (case-insensitively, with deterministic tie-breaking).
    pub fn sort_themes(themes: &mut [String]) {
        themes.sort_by(|a, b| {
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
                .then_with(|| a.cmp(b))
        });
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

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let dialog_w = 46.min(area.width.saturating_sub(4));
        let dialog_h = 16.min(area.height.saturating_sub(4)).max(8);
        (dialog_w, dialog_h)
    }

    /// Handles mouse event
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<String>> {
        let dialog_w = 46.min(area.width.saturating_sub(4));
        let dialog_h = 16.min(area.height.saturating_sub(4)).max(8);
        let inner_area = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

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
                let footer_y = inner_area.y + inner_area.height.saturating_sub(1);
                if row > inner_area.y && row < footer_y {
                    let clicked_row = (row - (inner_area.y + 1)) as usize;
                    let target_idx = self.scroll_offset + clicked_row;
                    if target_idx < self.themes.len() {
                        self.selected_idx = target_idx;
                        return Some(Some(self.themes[target_idx].clone()));
                    }
                } else if row == footer_y {
                    let select_lbl = t!("common.gutters.select");
                    let cancel_lbl = t!("common.buttons.cancel");
                    let footer_text = format!(" [Enter] {select_lbl}  [Esc] {cancel_lbl} ");
                    let avail_footer_w = inner_area.width.saturating_sub(2) as usize;
                    let clipped_footer: String = footer_text.chars().take(avail_footer_w).collect();
                    let footer_x = (inner_area.x + inner_area.width)
                        .saturating_sub(clipped_footer.len() as u16 + 1)
                        .max(inner_area.x + 1);
                    if col >= footer_x && col < footer_x + clipped_footer.len() as u16 {
                        let rel_x = (col - footer_x) as usize;
                        if let Some(pos) = clipped_footer.find("[Esc]") {
                            if rel_x >= pos {
                                return Some(None);
                            }
                        }
                        if let Some(pos) = clipped_footer.find("[Enter]") {
                            if rel_x >= pos && self.selected_idx < self.themes.len() {
                                return Some(Some(self.themes[self.selected_idx].clone()));
                            }
                        }
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

        let title = t!("bashmenu.theme_picker_title");
        let inner = render_modal_frame(area, dialog_w, dialog_h, &title, styles, buf);
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
            let clipped_name: String = t_name.chars().take(avail).collect();
            let padded_name = format!("{marker}{:<width$}", clipped_name, width = avail);
            buf.set_string(inner.x + 1, y, &padded_name, row_style);
        }

        // Footer
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let select_lbl = t!("common.gutters.select");
        let cancel_lbl = t!("common.buttons.cancel");
        let footer_text = format!(" [Enter] {select_lbl}  [Esc] {cancel_lbl} ");
        let avail_footer_w = inner.width.saturating_sub(2) as usize;
        let clipped_footer: String = footer_text.chars().take(avail_footer_w).collect();
        let footer_x = (inner.x + inner.width).saturating_sub(clipped_footer.len() as u16 + 1);
        let mut footer_style = styles.help_text;
        if footer_style.bg.is_none() || footer_style.bg == Some(Color::Reset) {
            footer_style = footer_style.bg(styles.background);
        }
        buf.set_string(
            footer_x.max(inner.x + 1),
            footer_y,
            &clipped_footer,
            footer_style,
        );

        inner
    }
}
