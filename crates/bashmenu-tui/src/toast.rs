//! Non-blocking toast notification rack and popup rendering.
//!
//! Provides 1:1 visual and functional parity with Python Textual's Toast notifications.
//! Features:
//! - Docked in the bottom-right corner above the status gutter / bottom border.
//! - Styled with an accent left indicator block `▌`, bold title header, and padded body.
//! - Non-blocking timer auto-dismissal.
//! - Instant dismissal upon mouse click.
//! - Stacking rack for multiple active notifications.

use crate::style::ResolvedThemeStyles;
use bashmenu_core::{ToastNotification, ToastSeverity};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

/// Represents an active toast notification currently held in the rack.
#[derive(Debug, Clone)]
pub struct ActiveToast {
    pub id: usize,
    pub title: Option<String>,
    pub message: String,
    pub severity: ToastSeverity,
    pub timeout_secs: f64,
    pub created_at: Instant,
    /// Computed screen bounding box for mouse click hit testing
    pub bounds: Option<Rect>,
}

/// Container that manages the lifecycle, stacking, and rendering of toasts.
#[derive(Debug, Default)]
pub struct ToastRack {
    toasts: Vec<ActiveToast>,
    next_id: usize,
}

impl ToastRack {
    /// Creates a new empty ToastRack.
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_id: 1,
        }
    }

    /// Adds a notification to the rack.
    pub fn add(&mut self, notification: ToastNotification) {
        let active = ActiveToast {
            id: self.next_id,
            title: notification.title,
            message: notification.message,
            severity: notification.severity,
            timeout_secs: notification.timeout_secs,
            created_at: Instant::now(),
            bounds: None,
        };
        self.next_id += 1;
        self.toasts.push(active);
    }

    /// Whether there are any active toasts in the rack.
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Returns the number of active toasts.
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// Advances the timer, removing expired toasts.
    /// Returns `true` if any toast expired and a redraw is needed.
    pub fn tick(&mut self) -> bool {
        let prev_len = self.toasts.len();
        self.toasts
            .retain(|t| t.created_at.elapsed().as_secs_f64() < t.timeout_secs);
        self.toasts.len() != prev_len
    }

    /// Handles a mouse click event. If the coordinates fall inside an active toast,
    /// dismisses that toast and returns `true`.
    pub fn handle_mouse(&mut self, col: u16, row: u16) -> bool {
        let mut clicked_idx = None;
        for (idx, toast) in self.toasts.iter().enumerate() {
            if let Some(rect) = toast.bounds {
                if col >= rect.x
                    && col < rect.x + rect.width
                    && row >= rect.y
                    && row < rect.y + rect.height
                {
                    clicked_idx = Some(idx);
                    break;
                }
            }
        }

        if let Some(idx) = clicked_idx {
            self.toasts.remove(idx);
            true
        } else {
            false
        }
    }

    /// Clears all active toasts immediately.
    pub fn clear(&mut self) {
        self.toasts.clear();
    }

    /// Renders active toasts stacked at the bottom-right corner of `area`.
    pub fn render(&mut self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        if self.toasts.is_empty() || area.width < 10 || area.height < 5 {
            return;
        }

        let max_w = ((area.width as usize * 50) / 100).clamp(28, 60);
        let mut curr_bottom_y = area.y + area.height.saturating_sub(3);

        // Toast background color: inherit from theme background if non-reset, or dark slate panel
        let bg_color = match styles.background {
            Color::Reset => Color::Rgb(52, 63, 73),
            c => c,
        };

        for toast in self.toasts.iter_mut().rev() {
            if curr_bottom_y < area.y + 2 {
                toast.bounds = None;
                continue;
            }

            let indicator_color = match toast.severity {
                ToastSeverity::Information => styles.toast_info,
                ToastSeverity::Warning => styles.toast_warning,
                ToastSeverity::Error => styles.toast_error,
            };

            let title_text = toast.title.as_deref().unwrap_or(match toast.severity {
                ToastSeverity::Information => "Information",
                ToastSeverity::Warning => "Warning",
                ToastSeverity::Error => "Error",
            });

            // Wrap message text
            let content_avail_w = max_w.saturating_sub(4).max(10);
            let message_lines = wrap_text_to_width(&toast.message, content_avail_w);

            let has_title = !title_text.is_empty();
            let total_h = 2 + (if has_title { 1 } else { 0 }) + message_lines.len().max(1);

            let longest_line_w = message_lines
                .iter()
                .map(|l| UnicodeWidthStr::width(l.as_str()))
                .chain(if has_title {
                    Some(UnicodeWidthStr::width(title_text))
                } else {
                    None
                })
                .max()
                .unwrap_or(10);

            let toast_w = (longest_line_w + 4).clamp(28, max_w);
            let toast_h = total_h as u16;

            let toast_x = area.x + area.width.saturating_sub(toast_w as u16 + 2);
            let toast_y = curr_bottom_y.saturating_sub(toast_h.saturating_sub(1));

            let toast_rect = Rect::new(toast_x, toast_y, toast_w as u16, toast_h);
            toast.bounds = Some(toast_rect);

            // Left indicator character from active theme (default: '▌' / U+258C)
            let left_bar = &styles.toast_bar_char;
            let bar_style = Style::default().fg(indicator_color).bg(bg_color);
            let content_bg_style = Style::default().bg(bg_color);
            let title_style = Style::default()
                .fg(indicator_color)
                .bg(bg_color)
                .add_modifier(Modifier::BOLD);
            let text_style = Style::default()
                .fg(styles.text.fg.unwrap_or(Color::White))
                .bg(bg_color);

            let inner_w = toast_w.saturating_sub(1);

            // Row 0: Top padding row
            let r0_y = toast_y;
            if r0_y < area.y + area.height {
                buf.set_string(toast_x, r0_y, left_bar, bar_style);
                buf.set_string(toast_x + 1, r0_y, " ".repeat(inner_w), content_bg_style);
            }

            let mut row_offset = 1;

            // Row 1: Title (if present)
            if has_title {
                let r_y = toast_y + row_offset;
                if r_y < area.y + area.height {
                    buf.set_string(toast_x, r_y, left_bar, bar_style);
                    buf.set_string(toast_x + 1, r_y, " ", content_bg_style);
                    buf.set_string(toast_x + 2, r_y, title_text, title_style);
                    let used = 1 + UnicodeWidthStr::width(title_text);
                    let pad = inner_w.saturating_sub(used);
                    if pad > 0 {
                        buf.set_string(
                            toast_x + 1 + used as u16,
                            r_y,
                            " ".repeat(pad),
                            content_bg_style,
                        );
                    }
                }
                row_offset += 1;
            }

            // Message rows
            for line in &message_lines {
                let r_y = toast_y + row_offset;
                if r_y < area.y + area.height {
                    buf.set_string(toast_x, r_y, left_bar, bar_style);
                    buf.set_string(toast_x + 1, r_y, " ", content_bg_style);
                    buf.set_string(toast_x + 2, r_y, line, text_style);
                    let used = 1 + UnicodeWidthStr::width(line.as_str());
                    let pad = inner_w.saturating_sub(used);
                    if pad > 0 {
                        buf.set_string(
                            toast_x + 1 + used as u16,
                            r_y,
                            " ".repeat(pad),
                            content_bg_style,
                        );
                    }
                }
                row_offset += 1;
            }

            // Bottom padding row
            let r_bot = toast_y + row_offset;
            if r_bot < area.y + area.height {
                buf.set_string(toast_x, r_bot, left_bar, bar_style);
                buf.set_string(toast_x + 1, r_bot, " ".repeat(inner_w), content_bg_style);
            }

            if toast_y < 2 {
                break;
            }
            curr_bottom_y = toast_y.saturating_sub(2);
        }
    }
}

/// Helper to wrap text into lines fitting within max_w
fn wrap_text_to_width(text: &str, max_w: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_w = 0;

        for word in words {
            let word_w = UnicodeWidthStr::width(word);
            if current_w == 0 {
                current_line.push_str(word);
                current_w = word_w;
            } else if current_w + 1 + word_w <= max_w {
                current_line.push(' ');
                current_line.push_str(word);
                current_w += 1 + word_w;
            } else {
                lines.push(current_line);
                current_line = word.to_string();
                current_w = word_w;
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_rack_lifecycle() {
        let mut rack = ToastRack::new();
        assert!(rack.is_empty());

        rack.add(ToastNotification::new("Hello, world!", None, 0.05));
        assert_eq!(rack.len(), 1);
        assert!(!rack.is_empty());

        // Not expired yet immediately
        assert!(!rack.tick());
        assert_eq!(rack.len(), 1);

        // Sleep to exceed timeout
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(rack.tick());
        assert_eq!(rack.len(), 0);
        assert!(rack.is_empty());
    }

    #[test]
    fn test_toast_mouse_dismissal() {
        let mut rack = ToastRack::new();
        rack.add(ToastNotification::new("Click me", None, 5.0));

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let styles = ResolvedThemeStyles::default();
        rack.render(area, &styles, &mut buf);

        let bounds = rack.toasts[0].bounds.expect("bounds must be set");
        // Click outside bounds
        assert!(!rack.handle_mouse(0, 0));
        assert_eq!(rack.len(), 1);

        // Click inside bounds
        assert!(rack.handle_mouse(bounds.x + 2, bounds.y + 1));
        assert_eq!(rack.len(), 0);
    }
}
