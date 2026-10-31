//! Formatted note viewer modal with interactive markdown checkboxes and scrolling.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Action triggered by user within the note viewer
#[derive(Debug, Clone, PartialEq)]
pub enum NoteViewerAction {
    /// No state transition
    None,
    /// Close the viewer modal
    Close,
    /// Request opening full text editor for note content
    Edit,
    /// Request renaming the current note
    Rename,
    /// Request deleting the current note
    Delete,
    /// Content was modified (e.g. checkbox toggled)
    ContentModified(String),
}

/// Formatted note viewer modal
#[derive(Debug, Clone)]
pub struct NoteViewerModal {
    pub title: String,
    pub filepath: String,
    pub content: String,
    pub show_completed: bool,
    pub scroll_offset: usize,
    pub selected_line: usize,
}

impl NoteViewerModal {
    pub fn new(title: impl Into<String>, filepath: impl Into<String>, content: &str) -> Self {
        Self {
            title: title.into(),
            filepath: filepath.into(),
            content: content.to_string(),
            show_completed: false,
            scroll_offset: 0,
            selected_line: 0,
        }
    }

    /// Handles keyboard input
    pub fn handle_key(&mut self, code: KeyCode) -> NoteViewerAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => NoteViewerAction::Close,
            KeyCode::Char('e') | KeyCode::Char('E') => NoteViewerAction::Edit,
            KeyCode::Char('m') | KeyCode::Char('M') => NoteViewerAction::Rename,
            KeyCode::Char('d') | KeyCode::Char('D') => NoteViewerAction::Delete,
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.show_completed = !self.show_completed;
                NoteViewerAction::None
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                // Copy to terminal clipboard via OSC 52
                let b64 = base64_encode(self.content.as_bytes());
                print!("\x1b]52;c;{}\x07", b64);
                NoteViewerAction::None
            }
            KeyCode::Char(' ') => {
                // Toggle checkbox on the active line
                let lines: Vec<&str> = self.content.lines().collect();
                if self.selected_line < lines.len() {
                    let line = lines[self.selected_line];
                    if let Some(toggled) = toggle_checkbox_line(line) {
                        let mut new_lines: Vec<String> =
                            lines.iter().map(|s| s.to_string()).collect();
                        new_lines[self.selected_line] = toggled;
                        self.content = new_lines.join("\n");
                        return NoteViewerAction::ContentModified(self.content.clone());
                    }
                }
                NoteViewerAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_line > 0 {
                    self.selected_line -= 1;
                }
                if self.selected_line < self.scroll_offset {
                    self.scroll_offset = self.selected_line;
                }
                NoteViewerAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let total_lines = self.content.lines().count();
                if self.selected_line + 1 < total_lines {
                    self.selected_line += 1;
                }
                NoteViewerAction::None
            }
            KeyCode::PageUp => {
                self.selected_line = self.selected_line.saturating_sub(15);
                self.scroll_offset = self.scroll_offset.saturating_sub(15);
                NoteViewerAction::None
            }
            KeyCode::PageDown => {
                let total_lines = self.content.lines().count();
                self.selected_line = (self.selected_line + 15).min(total_lines.saturating_sub(1));
                NoteViewerAction::None
            }
            KeyCode::Home => {
                self.selected_line = 0;
                self.scroll_offset = 0;
                NoteViewerAction::None
            }
            KeyCode::End => {
                let total_lines = self.content.lines().count();
                self.selected_line = total_lines.saturating_sub(1);
                NoteViewerAction::None
            }
            _ => NoteViewerAction::None,
        }
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent, _inner_area: Rect) -> NoteViewerAction {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                let total_lines = self.content.lines().count();
                if self.scroll_offset + 2 < total_lines {
                    self.scroll_offset += 2;
                }
                NoteViewerAction::None
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                NoteViewerAction::None
            }
            _ => NoteViewerAction::None,
        }
    }

    /// Renders note viewer modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = (area.width.saturating_sub(4)).min(78);
        let dialog_h = (area.height.saturating_sub(4)).clamp(14, 28);

        let modal_title = format!(" [ Note: {} ] ", self.title);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &modal_title, styles, buf);
        let content_h = inner.height.saturating_sub(3) as usize;

        let raw_lines: Vec<&str> = self.content.lines().collect();
        let avail_w = inner.width.saturating_sub(4) as usize;

        // Collect visible lines (filtering completed checkboxes if requested)
        let mut visible_entries: Vec<(usize, String)> = Vec::new();
        for (idx, line) in raw_lines.iter().enumerate() {
            if !self.show_completed && is_completed_checkbox(line) {
                continue;
            }
            // Word wrap line to avail_w
            let wrapped = textwrap_simple(line, avail_w);
            for w_line in wrapped {
                visible_entries.push((idx, w_line));
            }
        }

        for (i, (raw_idx, line_text)) in visible_entries
            .iter()
            .skip(self.scroll_offset)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let is_sel = *raw_idx == self.selected_line;

            let line_style = if is_sel {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else if is_completed_checkbox(line_text) {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else if line_text.contains("- [ ]") {
                Style::default().fg(Color::Yellow)
            } else if line_text.starts_with('#') {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            buf.set_string(inner.x + 2, y, line_text, line_style);
        }

        // Footer help bar
        let footer_y = inner.y + inner.height - 2;
        let toggle_action = if self.show_completed {
            "Hide Done"
        } else {
            "Show Done"
        };
        let footer_text = format!(
            " [SPACE] Toggle | [H] {} | [E] Edit | [M] Rename | [D] Delete | [ESC] Close ",
            toggle_action
        );
        let footer_x = inner.x + (inner.width.saturating_sub(footer_text.len() as u16) / 2);
        buf.set_string(
            footer_x,
            footer_y,
            &footer_text,
            Style::default().fg(Color::DarkGray),
        );

        inner
    }
}

/// Checks whether a line contains a completed task list item `- [x]` or `- [X]`
fn is_completed_checkbox(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- [x]")
        || trimmed.starts_with("- [X]")
        || trimmed.starts_with("* [x]")
        || trimmed.starts_with("* [X]")
}

/// Toggles task list checkbox on a single line
fn toggle_checkbox_line(line: &str) -> Option<String> {
    if line.contains("- [ ]") {
        Some(line.replacen("- [ ]", "- [x]", 1))
    } else if line.contains("- [x]") {
        Some(line.replacen("- [x]", "- [ ]", 1))
    } else if line.contains("- [X]") {
        Some(line.replacen("- [X]", "- [ ]", 1))
    } else if line.contains("* [ ]") {
        Some(line.replacen("* [ ]", "* [x]", 1))
    } else if line.contains("* [x]") {
        Some(line.replacen("* [x]", "* [ ]", 1))
    } else if line.contains("* [X]") {
        Some(line.replacen("* [X]", "* [ ]", 1))
    } else {
        None
    }
}

/// Simple word wrapper for note view lines
fn textwrap_simple(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 || text.len() <= max_w {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_w {
            current.push(' ');
            current.push_str(word);
        } else {
            result.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Minimal base64 encoder for OSC 52 clipboard sequences
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
