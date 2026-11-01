//! Horizontal scrolling marquee ticker and text slice calculator.
//!
//! Provides 1:1 parity with the Python reference implementation for
//! horizontal text marquees when label text overflows available width.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Formats the spacer between repeated cycles of marquee scrolling text.
pub fn format_marquee_spacer(char: &str, gap: usize) -> String {
    let fill = if char.is_empty() { "•" } else { char };
    fill.repeat(gap)
}

/// Calculate and return a scrolling slice of text for width `avail_w` at `offset`.
pub fn get_marquee_slice(
    text: &str,
    avail_w: usize,
    offset: usize,
    char: &str,
    gap: usize,
) -> String {
    if text.is_empty() || avail_w == 0 {
        return String::new();
    }
    let text_w = UnicodeWidthStr::width(text);
    if text_w <= avail_w {
        return text.to_string();
    }

    let spacer = format_marquee_spacer(char, gap);
    let loop_unit = format!("{}{}", text, spacer);
    let unit_cells = UnicodeWidthStr::width(loop_unit.as_str());
    if unit_cells == 0 {
        return String::new();
    }

    let eff_offset = offset % unit_cells;
    let times = ((eff_offset + avail_w) / unit_cells.max(1)) + 3;
    let stream = loop_unit.repeat(times);

    let mut curr_cells = 0;
    let mut start_char_idx = 0;
    for (i, c) in stream.chars().enumerate() {
        if curr_cells >= eff_offset {
            start_char_idx = i;
            break;
        }
        curr_cells += UnicodeWidthChar::width(c).unwrap_or(0);
    }

    let mut collected = String::new();
    let mut collected_cells = 0;
    for c in stream.chars().skip(start_char_idx) {
        let c_w = UnicodeWidthChar::width(c).unwrap_or(0);
        if collected_cells + c_w > avail_w {
            break;
        }
        collected.push(c);
        collected_cells += c_w;
    }

    if collected_cells < avail_w {
        collected.push_str(&" ".repeat(avail_w - collected_cells));
    }

    collected
}

/// State tracker for pause delays and ticking offsets of marquee text elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarqueeTracker {
    /// Initial pause tick count before scrolling starts
    pub pause_ticks: usize,
    /// Number of ticks remaining in the pause state
    pub ticks_remaining: usize,
    /// Current column scroll offset
    pub offset: usize,
}

impl Default for MarqueeTracker {
    fn default() -> Self {
        Self::new(4)
    }
}

impl MarqueeTracker {
    /// Creates a new MarqueeTracker with the given pause tick count.
    pub fn new(pause_ticks: usize) -> Self {
        Self {
            pause_ticks,
            ticks_remaining: pause_ticks,
            offset: 0,
        }
    }

    /// Resets the tracker back to the start offset and initial pause state.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.ticks_remaining = self.pause_ticks;
    }

    /// Advances the marquee ticker. Returns `true` if the offset advanced and redraw is needed.
    pub fn tick(&mut self) -> bool {
        if self.ticks_remaining > 0 {
            self.ticks_remaining -= 1;
            false
        } else {
            self.offset += 1;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marquee_slice_short_text() {
        assert_eq!(get_marquee_slice("Short", 10, 0, "•", 3), "Short");
        assert_eq!(get_marquee_slice("Exact", 5, 0, "•", 3), "Exact");
    }

    #[test]
    fn test_marquee_slice_scrolling() {
        let text = "Long label text to marquee";
        let avail_w = 10;
        let s0 = get_marquee_slice(text, avail_w, 0, " ", 3);
        assert_eq!(s0, "Long label");

        let s1 = get_marquee_slice(text, avail_w, 1, " ", 3);
        assert_eq!(s1, "ong label ");

        let s5 = get_marquee_slice(text, avail_w, 5, " ", 3);
        assert_eq!(s5, "label text");
    }

    #[test]
    fn test_marquee_tracker_pause_and_advance() {
        let mut tracker = MarqueeTracker::new(3);
        assert_eq!(tracker.offset, 0);
        // Ticks 1..3 should return false (paused)
        assert!(!tracker.tick());
        assert!(!tracker.tick());
        assert!(!tracker.tick());
        assert_eq!(tracker.offset, 0);

        // Next ticks advance offset
        assert!(tracker.tick());
        assert_eq!(tracker.offset, 1);
        assert!(tracker.tick());
        assert_eq!(tracker.offset, 2);

        // Reset
        tracker.reset();
        assert_eq!(tracker.offset, 0);
        assert!(!tracker.tick());
    }
}
