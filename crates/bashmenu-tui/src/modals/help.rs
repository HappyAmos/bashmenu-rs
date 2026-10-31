//! Scrollable markdown help modal.

use super::frame::render_modal_frame;
use super::note_viewer::wrap_footer_tokens;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Scrollable markdown / text help modal
#[derive(Debug, Clone)]
pub struct HelpModal {
    pub title: String,
    pub raw_content: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
}

impl HelpModal {
    pub fn new(title: impl Into<String>, content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        Self {
            title: title.into(),
            raw_content: content.to_string(),
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
                self.scroll_offset += 1;
                false
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(15);
                false
            }
            KeyCode::PageDown => {
                self.scroll_offset += 15;
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
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.scroll_offset += 2;
                false
            }
            MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(2);
                false
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let dialog_w = ((area.width as f32 * 0.90) as u16)
                    .min(area.width.saturating_sub(4))
                    .max(20);
                let dialog_h = (area.height.saturating_sub(2)).max(10);
                let inner = if area.width <= dialog_w {
                    area
                } else {
                    super::frame::compute_modal_inner(area, dialog_w, dialog_h)
                };
                super::frame::is_close_button_clicked(inner, mouse.row, mouse.column)
            }
            _ => false,
        }
    }

    /// Renders help modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        // Expand to 90% of screen width within margins
        let dialog_w = ((area.width as f32 * 0.90) as u16)
            .min(area.width.saturating_sub(4))
            .max(20);
        let dialog_h = (area.height.saturating_sub(2)).max(10);

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let footer_tokens = [
            "[UP/DN/PGUP/PGDN]: Scroll",
            "|",
            "[HOME/END]: Top/Bottom",
            "|",
            "[ESC/ENTER]: Close",
        ];
        let max_footer_w = inner.width.saturating_sub(4) as usize;
        let footer_lines = wrap_footer_tokens(&footer_tokens, max_footer_w);
        let footer_h = footer_lines.len().max(1);

        let content_h = inner.height.saturating_sub(1 + footer_h as u16 + 1) as usize;
        let avail_w = inner.width.saturating_sub(4) as usize;

        // Collect wrapped lines with formatting tags
        let mut wrapped_lines: Vec<(String, Option<Style>)> = Vec::new();
        let mut in_code_block = false;

        for line in self.raw_content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code_block = !in_code_block;
                wrapped_lines.push((
                    line.to_string(),
                    Some(
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    ),
                ));
                continue;
            }

            if in_code_block {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push((wl, Some(Style::default().fg(Color::Yellow))));
                }
                continue;
            }

            if trimmed.starts_with("# ") {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push((
                        wl,
                        Some(
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ));
                }
            } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push((wl, Some(styles.accent.add_modifier(Modifier::BOLD))));
                }
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push((wl, Some(styles.text)));
                }
            } else {
                let wrapped = wrap_help_text(line, avail_w);
                for wl in wrapped {
                    wrapped_lines.push((wl, None));
                }
            }
        }

        let max_scroll = wrapped_lines.len().saturating_sub(content_h);
        let eff_scroll = self.scroll_offset.min(max_scroll);

        for (i, (line, opt_style)) in wrapped_lines
            .iter()
            .skip(eff_scroll)
            .take(content_h)
            .enumerate()
        {
            let y = inner.y + 1 + i as u16;
            let mut st = opt_style.unwrap_or(styles.text);
            if st.bg.is_none() || st.bg == Some(Color::Reset) {
                st = st.bg(styles.background);
            }
            buf.set_string(inner.x + 2, y, line, st);
        }

        // Render wrapped footer help lines with equal spacing
        let footer_start_y = inner.y + inner.height.saturating_sub(1 + footer_h as u16);
        for (idx, line) in footer_lines.iter().enumerate() {
            let footer_x = inner.x + (inner.width.saturating_sub(line.len() as u16) / 2);
            buf.set_string(
                footer_x,
                footer_start_y + idx as u16,
                line,
                Style::default().fg(Color::DarkGray),
            );
        }

        inner
    }
}

/// Simple word wrapper for help modal text
fn wrap_help_text(text: &str, max_w: usize) -> Vec<String> {
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

    if result.is_empty() {
        result.push(String::new());
    }

    result
}
