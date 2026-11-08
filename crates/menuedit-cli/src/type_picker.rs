//! Modal selector for changing menu item types.

use bashmenu_tui::{
    compute_modal_inner, is_close_button_clicked, render_modal_frame, ResolvedThemeStyles,
};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Available menu item types presented in the type selection modal `(type_name, description)`.
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
    /// Currently highlighted item index in `ITEM_TYPES`
    pub selected_idx: usize,
    /// Vertical scroll offset for long item type lists
    pub scroll_offset: usize,
}

impl ItemTypePickerModal {
    /// Constructs a new type picker modal pre-selecting the current type.
    pub fn new(current_type: &str) -> Self {
        let selected_idx = ITEM_TYPES
            .iter()
            .position(|(t, _)| {
                *t == current_type || (*t == "{divider}" && current_type == "divider")
            })
            .unwrap_or(0);
        let scroll_offset = if selected_idx >= 12 {
            selected_idx.saturating_sub(6)
        } else {
            0
        };
        Self {
            selected_idx,
            scroll_offset,
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
            KeyCode::Home => {
                self.selected_idx = 0;
                self.scroll_offset = 0;
                None
            }
            KeyCode::End => {
                self.selected_idx = ITEM_TYPES.len().saturating_sub(1);
                None
            }
            KeyCode::PageUp => {
                self.selected_idx = self.selected_idx.saturating_sub(5);
                if self.selected_idx < self.scroll_offset {
                    self.scroll_offset = self.selected_idx;
                }
                None
            }
            KeyCode::PageDown => {
                self.selected_idx = (self.selected_idx + 5).min(ITEM_TYPES.len().saturating_sub(1));
                None
            }
            _ => None,
        }
    }

    /// Handles mouse event
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<Option<String>> {
        let dialog_w = 64.min(area.width.saturating_sub(4));
        let dialog_h = (ITEM_TYPES.len() as u16 + 4).min(area.height.saturating_sub(2));
        let inner = compute_modal_inner(area, dialog_w, dialog_h);

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.selected_idx + 1 < ITEM_TYPES.len() {
                    self.selected_idx += 1;
                }
                if self.scroll_offset + 1 < ITEM_TYPES.len() {
                    self.scroll_offset += 1;
                }
                None
            }
            MouseEventKind::ScrollUp => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_close_button_clicked(inner, mouse.row, mouse.column) {
                    return Some(None);
                }
                let content_h = inner.height.saturating_sub(2) as usize;
                if mouse.row > inner.y && mouse.row < inner.y + 1 + content_h as u16 {
                    let clicked_row = (mouse.row - (inner.y + 1)) as usize;
                    let target_idx = self.scroll_offset + clicked_row;
                    if target_idx < ITEM_TYPES.len() {
                        self.selected_idx = target_idx;
                        return Some(Some(ITEM_TYPES[target_idx].0.to_string()));
                    }
                }
                let footer_y = inner.y + inner.height.saturating_sub(1);
                if mouse.row == footer_y {
                    let footer_text = " [Enter] Select  [Esc] Cancel ";
                    let avail_footer_w = inner.width.saturating_sub(2) as usize;
                    let clipped_footer: String = footer_text.chars().take(avail_footer_w).collect();
                    let footer_x = (inner.x + inner.width)
                        .saturating_sub(clipped_footer.len() as u16 + 1)
                        .max(inner.x + 1);
                    if mouse.column >= footer_x
                        && mouse.column < footer_x + clipped_footer.len() as u16
                    {
                        let rel_x = (mouse.column - footer_x) as usize;
                        if let Some(pos) = clipped_footer.find("[Esc]") {
                            if rel_x >= pos {
                                return Some(None);
                            }
                        }
                        if let Some(pos) = clipped_footer.find("[Enter]") {
                            if rel_x >= pos && self.selected_idx < ITEM_TYPES.len() {
                                return Some(Some(ITEM_TYPES[self.selected_idx].0.to_string()));
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Renders the type picker modal
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 64.min(area.width.saturating_sub(4));
        let dialog_h = (ITEM_TYPES.len() as u16 + 4).min(area.height.saturating_sub(2));

        let inner = render_modal_frame(
            area,
            dialog_w,
            dialog_h,
            " Select Menu Item Type ",
            styles,
            buf,
        );

        let content_h = inner.height.saturating_sub(2) as usize;

        // Ensure scroll_offset keeps selected_idx visible
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + content_h && content_h > 0 {
            self.scroll_offset = self.selected_idx.saturating_sub(content_h - 1);
        }
        if self.scroll_offset + content_h > ITEM_TYPES.len() {
            self.scroll_offset = ITEM_TYPES.len().saturating_sub(content_h);
        }

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
                .take(inner.width.saturating_sub(3) as usize)
                .collect();
            buf.set_string(inner.x + 1, y, &truncated, row_style);
        }

        // Scroll indicators when items overflow content area
        if self.scroll_offset > 0 {
            buf.set_string(
                inner.x + inner.width.saturating_sub(3),
                inner.y + 1,
                "▲",
                styles.accent.add_modifier(Modifier::BOLD),
            );
        }
        if self.scroll_offset + content_h < ITEM_TYPES.len() {
            buf.set_string(
                inner.x + inner.width.saturating_sub(3),
                inner.y + content_h as u16,
                "▼",
                styles.accent.add_modifier(Modifier::BOLD),
            );
        }

        // Footer
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let footer_text = " [Enter] Select  [Esc] Cancel ";
        let avail_footer_w = inner.width.saturating_sub(2) as usize;
        let clipped_footer: String = footer_text.chars().take(avail_footer_w).collect();
        let footer_x = (inner.x + inner.width).saturating_sub(clipped_footer.len() as u16 + 1);
        buf.set_string(
            footer_x.max(inner.x + 1),
            footer_y,
            &clipped_footer,
            styles.gutter.add_modifier(Modifier::DIM),
        );

        inner
    }
}
