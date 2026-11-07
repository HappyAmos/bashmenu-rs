//! Note conflict resolution modal dialog.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Conflict resolution choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteConflictChoice {
    /// 3-way line merge
    Merge,
    /// Overwrite disk version with local edits
    Overwrite,
    /// Discard local edits and reload from disk
    Reload,
    /// Save local edits under a new copy
    SaveAsCopy,
    /// Cancel and return to editing
    Cancel,
}

const BUTTON_LABELS: [&str; 5] = [
    "[ Merge ]",
    "[ Overwrite Disk ]",
    "[ Reload Disk ]",
    "[ Save As Copy ]",
    "[ Cancel ]",
];

/// Interactive modal presented when concurrent external edits conflict with
/// local changes.
#[derive(Debug, Clone)]
pub struct NoteConflictModal {
    /// Modal header title
    pub title: String,
    /// Explanatory message
    pub message: String,
    /// Note title being edited
    pub note_title: String,
    /// Local edited content
    pub local_content: String,
    /// Current disk content
    pub disk_content: String,
    /// Original base content
    pub base_content: String,
    /// Filepath of the notebook
    pub filepath: String,
    /// Currently focused button index (0..5)
    pub selected_index: usize,
}

impl NoteConflictModal {
    /// Creates a new note conflict dialog.
    pub fn new(
        note_title: &str,
        filepath: &str,
        local_content: &str,
        disk_content: &str,
        base_content: &str,
    ) -> Self {
        Self {
            title: format!("Conflict: {}", note_title),
            message: "This note was modified externally while you were \
editing it.\nChoose how to resolve this concurrent change:"
                .to_string(),
            note_title: note_title.to_string(),
            local_content: local_content.to_string(),
            disk_content: disk_content.to_string(),
            base_content: base_content.to_string(),
            filepath: filepath.to_string(),
            selected_index: 0,
        }
    }

    /// Handles keyboard events.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<NoteConflictChoice> {
        match code {
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
                if self.selected_index == 0 {
                    self.selected_index = BUTTON_LABELS.len() - 1;
                } else {
                    self.selected_index -= 1;
                }
                None
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                self.selected_index = (self.selected_index + 1) % BUTTON_LABELS.len();
                None
            }
            KeyCode::Char('m') | KeyCode::Char('M') => Some(NoteConflictChoice::Merge),
            KeyCode::Char('o') | KeyCode::Char('O') => Some(NoteConflictChoice::Overwrite),
            KeyCode::Char('r') | KeyCode::Char('R') => Some(NoteConflictChoice::Reload),
            KeyCode::Char('s') | KeyCode::Char('S') => Some(NoteConflictChoice::SaveAsCopy),
            KeyCode::Enter => match self.selected_index {
                0 => Some(NoteConflictChoice::Merge),
                1 => Some(NoteConflictChoice::Overwrite),
                2 => Some(NoteConflictChoice::Reload),
                3 => Some(NoteConflictChoice::SaveAsCopy),
                _ => Some(NoteConflictChoice::Cancel),
            },
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                Some(NoteConflictChoice::Cancel)
            }
            _ => None,
        }
    }

    /// Selects appropriately sized button labels for the available width.
    fn button_labels(&self, inner_width: u16) -> [&'static str; 5] {
        if inner_width >= 68 {
            BUTTON_LABELS
        } else if inner_width >= 50 {
            [
                "[ Merge ]",
                "[ Overwrite ]",
                "[ Reload ]",
                "[ Save Copy ]",
                "[ Cancel ]",
            ]
        } else {
            ["[Merge]", "[Over]", "[Reload]", "[Copy]", "[Cancel]"]
        }
    }

    /// Computes inner bounds and layout for modal frame and button hits.
    fn compute_layout(&self, area: Rect) -> (Rect, u16, u16) {
        let max_w = area.width.saturating_sub(4);
        let dialog_w = 74.min(max_w).max(20.min(max_w));
        let avail_w = dialog_w.saturating_sub(6) as usize;
        let wrapped = if avail_w > 0 {
            super::input::wrap_prompt_text(&self.message, avail_w)
        } else {
            Vec::new()
        };
        let msg_lines = wrapped.len().max(1);
        let max_h = area.height.saturating_sub(2);
        let dialog_h = ((msg_lines + 5) as u16).min(max_h).max(8.min(max_h));
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);
        (inner, dialog_w, dialog_h)
    }

    /// Handles mouse interaction.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<NoteConflictChoice> {
        let (inner, _, _) = self.compute_layout(area);

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            if super::frame::is_close_button_clicked(inner, row, col) {
                return Some(NoteConflictChoice::Cancel);
            }

            let btn_y = inner.y + inner.height.saturating_sub(2);
            if row == btn_y {
                let btn_positions = self.calculate_button_positions(inner);
                for (idx, (start_x, end_x)) in btn_positions.into_iter().enumerate() {
                    if col >= start_x && col < end_x {
                        self.selected_index = idx;
                        return match idx {
                            0 => Some(NoteConflictChoice::Merge),
                            1 => Some(NoteConflictChoice::Overwrite),
                            2 => Some(NoteConflictChoice::Reload),
                            3 => Some(NoteConflictChoice::SaveAsCopy),
                            _ => Some(NoteConflictChoice::Cancel),
                        };
                    }
                }
            }
        }
        None
    }

    /// Calculates horizontal spans `(start_x, end_x)` for each button.
    fn calculate_button_positions(&self, inner: Rect) -> Vec<(u16, u16)> {
        let mut positions = Vec::new();
        let labels = self.button_labels(inner.width);
        let total_btn_len: usize = labels.iter().map(|l| l.len()).sum();
        let total_gaps = labels.len().saturating_sub(1);
        let avail = inner.width.saturating_sub(4) as usize;
        let spacing = if avail > total_btn_len && total_gaps > 0 {
            ((avail - total_btn_len) / total_gaps).max(1)
        } else {
            1
        };

        let mut curr_x = inner.x + 2;
        let right_bound = inner.x + inner.width;
        for label in labels {
            let start = curr_x;
            let end = (curr_x + label.len() as u16).min(right_bound);
            positions.push((start, end));
            curr_x = end + spacing as u16;
        }
        positions
    }

    /// Renders the note conflict resolution dialog.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let (inner, dialog_w, dialog_h) = self.compute_layout(area);
        render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Message Lines (wrapped and clipped within inner width)
        let avail_w = inner.width.saturating_sub(4) as usize;
        if avail_w > 0 {
            let text_lines = super::input::wrap_prompt_text(&self.message, avail_w);
            for (i, line) in text_lines.iter().enumerate() {
                let py = inner.y + 1 + i as u16;
                if py < inner.y + inner.height.saturating_sub(2) {
                    let clipped: String = line.chars().take(avail_w).collect();
                    buf.set_string(inner.x + 2, py, &clipped, styles.text);
                }
            }
        }

        // 2. Buttons
        let btn_y = inner.y + inner.height.saturating_sub(2);
        let positions = self.calculate_button_positions(inner);
        let labels = self.button_labels(inner.width);
        let right_bound = inner.x + inner.width;

        let normal_style = styles.text;
        let focused_style = styles.highlight.add_modifier(Modifier::BOLD);
        let danger_focused = Style::default()
            .fg(Color::Black)
            .bg(Color::LightRed)
            .add_modifier(Modifier::BOLD);

        for (idx, (start_x, _)) in positions.into_iter().enumerate() {
            if start_x >= right_bound {
                break;
            }
            let is_cur = idx == self.selected_index;
            let label = labels[idx];
            let max_w = (right_bound - start_x) as usize;
            let clipped_label: String = label.chars().take(max_w).collect();
            let st = if is_cur {
                if idx == 1 {
                    danger_focused
                } else {
                    focused_style
                }
            } else {
                normal_style
            };
            buf.set_string(start_x, btn_y, &clipped_label, st);
        }

        inner
    }
}
