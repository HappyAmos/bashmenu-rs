//! Modal selector for changing menu item types.

use bashmenu_tui::{render_modal_frame, ResolvedThemeStyles};
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

pub const ITEM_TYPES: &[(&str, &str)] = &[
    ("submenu", "Submenu Folder (nested options list)"),
    ("command", "Shell Command (executes shell command)"),
    ("script", "Script File (runs script from app folder)"),
    ("config", "Config Setting (edits bashmenu.yml value)"),
    ("toggle", "Settings Toggle (True/False/Cancel toggle modal)"),
    ("radio", "Radio Setting (select option from radio modal)"),
    ("editor", "File Editor (opens file in built-in editor)"),
    ("notebook", "Notebook Menu (notes collection file)"),
    ("confirm", "Confirm Dialog (Yes/No/Cancel dialog)"),
    ("message", "Message Popup (alert / info text dialog)"),
    ("python", "Python Routine (built-in python function)"),
    ("inject_block", "Inject Block (dynamic code block template)"),
    ("theme_selector", "Theme Selector (dynamic theme menu)"),
    ("back", "Back Button (returns to parent menu)"),
    ("exit", "Exit Button (terminates application)"),
    ("{divider}", "Visual Divider Line (aesthetic separator)"),
];

/// Modal dialog for picking an item type
#[derive(Debug, Clone)]
pub struct ItemTypePickerModal {
    pub selected_idx: usize,
    pub scroll_offset: usize,
}

impl ItemTypePickerModal {
    pub fn new(current_type: &str) -> Self {
        let selected_idx = ITEM_TYPES
            .iter()
            .position(|(t, _)| *t == current_type)
            .unwrap_or(0);
        Self {
            selected_idx,
            scroll_offset: 0,
        }
    }

    /// Handles keyboard event. Returns `Some(Some(type))` on choice, `Some(None)` on cancel, `None` to continue.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<String>> {
        match code {
            KeyCode::Enter => {
                let chosen = ITEM_TYPES[self.selected_idx].0.to_string();
                Some(Some(chosen))
            }
            KeyCode::Esc | KeyCode::Char('q') => Some(None),
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
                if self.selected_idx < self.scroll_offset {
                    self.scroll_offset = self.selected_idx;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_idx + 1 < ITEM_TYPES.len() {
                    self.selected_idx += 1;
                }
                None
            }
            _ => None,
        }
    }

    /// Handles mouse event
    pub fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> Option<Option<String>> {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.selected_idx + 1 < ITEM_TYPES.len() {
                    self.selected_idx += 1;
                }
                None
            }
            MouseEventKind::ScrollUp => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
                None
            }
            _ => None,
        }
    }

    /// Renders the type picker modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 64.min(area.width.saturating_sub(4));
        let dialog_h = 18.min(area.height.saturating_sub(4));

        let inner = render_modal_frame(
            area,
            dialog_w,
            dialog_h,
            " Select Menu Item Type ",
            styles,
            buf,
        );

        let content_h = inner.height.saturating_sub(2) as usize;

        for (i, (type_id, desc)) in ITEM_TYPES
            .iter()
            .skip(self.scroll_offset)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let is_sel = (self.scroll_offset + i) == self.selected_idx;

            let row_style = if is_sel {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            let line_str = format!(" {:<14} - {} ", type_id, desc);
            let truncated: String = line_str
                .chars()
                .take(inner.width.saturating_sub(2) as usize)
                .collect();
            buf.set_string(inner.x + 1, y, &truncated, row_style);
        }

        inner
    }
}
