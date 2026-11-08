//! Standardized Help Gutter and Footer layout engine.
//!
//! Follows the universal BashMenu rules:
//! 1. Help gutters may use up to two lines (or three lines when hovered).
//! 2. Help text treats the pipe character `|` as a word boundary for wrapping.
//! 3. Text is spread evenly across lines with centered pipe delimiters.

use std::time::{Duration, Instant};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Measures visual display column width of gutter text.
#[inline]
pub fn gutter_text_width(s: &str) -> usize {
    s.width()
}

/// Truncates a string to at most `max_w` display columns.
pub fn truncate_gutter_text(text: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut cur_w = 0;
    for c in text.chars() {
        let cw = c.width().unwrap_or(0);
        if cur_w + cw > max_w {
            break;
        }
        out.push(c);
        cur_w += cw;
    }
    out
}

/// Parses help text or tokens separated by '|' into cleaned item strings.
/// Treats '|' as an atomic word / delimiter boundary.
pub fn parse_help_tokens(input: &[&str]) -> Vec<String> {
    let mut items = Vec::new();
    for token in input {
        for part in token.split('|') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                items.push(trimmed.to_string());
            }
        }
    }
    items
}

/// Helper function to format items onto a single line, spreading them apart
/// evenly across `max_w` with centered pipe delimiters.
///
/// Returns a tuple of:
/// - The formatted line string (matching `max_w` visual length if sufficient items/space).
/// - A vector of `(start_col, end_col)` character index ranges for each item within
///   that line (for interactive mouse click hit-testing).
pub fn spread_gutter_line_layout(items: &[&str], max_w: usize) -> (String, Vec<(usize, usize)>) {
    if items.is_empty() || max_w == 0 {
        return (String::new(), Vec::new());
    }
    if items.len() == 1 {
        let text = if gutter_text_width(items[0]) > max_w {
            truncate_gutter_text(items[0], max_w)
        } else {
            items[0].to_string()
        };
        let len = gutter_text_width(&text);
        return (text, vec![(0, len)]);
    }

    let num_gaps = items.len() - 1;
    let item_lens: Vec<usize> = items.iter().map(|s| gutter_text_width(s)).collect();
    let tot_item_len: usize = item_lens.iter().sum();
    let min_line_len = tot_item_len + num_gaps * 3; // 3 chars per gap: " | "

    if min_line_len >= max_w {
        // Not enough space to spread apart; join with minimal " | " and truncate if needed
        let mut line = String::new();
        let mut ranges = Vec::new();
        let mut cur_col = 0;
        for (i, &it) in items.iter().enumerate() {
            let it_len = gutter_text_width(it);
            if cur_col >= max_w {
                break;
            }
            let end_col = (cur_col + it_len).min(max_w);
            ranges.push((cur_col, end_col));
            line.push_str(it);
            cur_col += it_len;
            if i < num_gaps {
                line.push_str(" | ");
                cur_col += 3;
            }
        }
        if gutter_text_width(&line) > max_w {
            line = truncate_gutter_text(&line, max_w);
        }
        return (line, ranges);
    }

    let rem = max_w - min_line_len;
    let base_extra = rem / num_gaps;
    let extra = rem % num_gaps;

    let mut line = String::new();
    let mut ranges = Vec::new();
    let mut cur_col = 0;

    for (i, &it) in items.iter().enumerate() {
        let it_len = gutter_text_width(it);
        ranges.push((cur_col, cur_col + it_len));
        line.push_str(it);
        cur_col += it_len;

        if i < num_gaps {
            let total_extra = base_extra + if i < extra { 1 } else { 0 };
            let left_spaces = 1 + (total_extra / 2);
            let right_spaces = 1 + (total_extra - (total_extra / 2));
            let sep_spaces_left = " ".repeat(left_spaces);
            let sep_spaces_right = " ".repeat(right_spaces);

            line.push_str(&sep_spaces_left);
            line.push('|');
            line.push_str(&sep_spaces_right);
            cur_col += left_spaces + 1 + right_spaces;
        }
    }

    (line, ranges)
}

/// Convenience function that returns only the formatted line string spread across `max_w`.
pub fn format_spread_gutter_line(items: &[&str], max_w: usize) -> String {
    spread_gutter_line_layout(items, max_w).0
}

/// Represents an item on a gutter line with its `(token_string, start_col, end_col)` character range.
pub type GutterItemRange = (String, usize, usize);

/// A formatted gutter line along with character ranges for interactive mouse hit-testing.
pub type GutterLineLayout = (String, Vec<GutterItemRange>);

/// Wraps help items into at most `max_lines` (typically 1 or 2),
/// treating the pipe character `|` as a word/delimiter boundary.
///
/// Returns a vector of tuples: `(line_string, items_with_ranges)` where each item range is `(token_str, start_col, end_col)`.
pub fn wrap_spread_help_gutter_layout(
    items: &[&str],
    max_w: usize,
    max_lines: usize,
) -> Vec<GutterLineLayout> {
    if items.is_empty() || max_w == 0 || max_lines == 0 {
        return Vec::new();
    }

    let cleaned = parse_help_tokens(items);
    if cleaned.is_empty() {
        return Vec::new();
    }
    let cleaned_refs: Vec<&str> = cleaned.iter().map(|s| s.as_str()).collect();

    // 1. Check if all items fit on 1 line
    let total_item_len: usize = cleaned_refs.iter().map(|s| gutter_text_width(s)).sum();
    let min_1_line_len = total_item_len + cleaned_refs.len().saturating_sub(1) * 3;

    if min_1_line_len <= max_w || max_lines == 1 {
        let items_to_spread = if min_1_line_len <= max_w {
            cleaned_refs
        } else {
            let mut fit = Vec::new();
            let mut cur_len = 0;
            for &item in &cleaned_refs {
                let needed = if fit.is_empty() {
                    gutter_text_width(item)
                } else {
                    3 + gutter_text_width(item)
                };
                if cur_len + needed <= max_w {
                    fit.push(item);
                    cur_len += needed;
                } else {
                    break;
                }
            }
            if fit.is_empty() && !cleaned_refs.is_empty() {
                fit.push(cleaned_refs[0]);
            }
            fit
        };
        let (line_str, ranges) = spread_gutter_line_layout(&items_to_spread, max_w);
        let items_with_ranges: Vec<(String, usize, usize)> = items_to_spread
            .iter()
            .zip(ranges)
            .map(|(&txt, (s, e))| (txt.to_string(), s, e))
            .collect();
        return vec![(line_str, items_with_ranges)];
    }

    // 2. Try 2 lines (max_lines >= 2)
    // Find the most balanced split point k such that both lines fit in max_w
    let n = cleaned_refs.len();
    let mut best_k: Option<usize> = None;
    let mut best_imbalance = usize::MAX;

    for k in 1..n {
        let l1 = &cleaned_refs[..k];
        let l2 = &cleaned_refs[k..];

        let l1_len: usize = l1.iter().map(|s| gutter_text_width(s)).sum::<usize>()
            + (l1.len().saturating_sub(1) * 3);
        let l2_len: usize = l2.iter().map(|s| gutter_text_width(s)).sum::<usize>()
            + (l2.len().saturating_sub(1) * 3);

        if l1_len <= max_w && l2_len <= max_w {
            let count_diff = k.abs_diff(n - k);
            let len_diff = l1_len.abs_diff(l2_len);
            let imbalance = count_diff * 10 + len_diff;

            if imbalance < best_imbalance {
                best_imbalance = imbalance;
                best_k = Some(k);
            }
        }
    }

    if let Some(k) = best_k {
        let (l1_str, r1) = spread_gutter_line_layout(&cleaned_refs[..k], max_w);
        let (l2_str, r2) = spread_gutter_line_layout(&cleaned_refs[k..], max_w);
        let items1: Vec<(String, usize, usize)> = cleaned_refs[..k]
            .iter()
            .zip(r1)
            .map(|(&txt, (s, e))| (txt.to_string(), s, e))
            .collect();
        let items2: Vec<(String, usize, usize)> = cleaned_refs[k..]
            .iter()
            .zip(r2)
            .map(|(&txt, (s, e))| (txt.to_string(), s, e))
            .collect();
        return vec![(l1_str, items1), (l2_str, items2)];
    }

    // 3. Try 3 lines if max_lines >= 3
    if max_lines >= 3 && n >= 3 {
        let mut best_3way: Option<(usize, usize)> = None;
        let mut best_imbalance_3 = usize::MAX;

        for k1 in 1..n {
            for k2 in (k1 + 1)..n {
                let l1 = &cleaned_refs[..k1];
                let l2 = &cleaned_refs[k1..k2];
                let l3 = &cleaned_refs[k2..];

                let l1_len: usize = l1.iter().map(|s| gutter_text_width(s)).sum::<usize>()
                    + (l1.len().saturating_sub(1) * 3);
                let l2_len: usize = l2.iter().map(|s| gutter_text_width(s)).sum::<usize>()
                    + (l2.len().saturating_sub(1) * 3);
                let l3_len: usize = l3.iter().map(|s| gutter_text_width(s)).sum::<usize>()
                    + (l3.len().saturating_sub(1) * 3);

                if l1_len <= max_w && l2_len <= max_w && l3_len <= max_w {
                    let len1 = k1;
                    let len2 = k2 - k1;
                    let len3 = n - k2;
                    let count_diff = len1.abs_diff(len2) + len2.abs_diff(len3);
                    let len_diff = l1_len.abs_diff(l2_len) + l2_len.abs_diff(l3_len);
                    let imbalance = count_diff * 10 + len_diff;

                    if imbalance < best_imbalance_3 {
                        best_imbalance_3 = imbalance;
                        best_3way = Some((k1, k2));
                    }
                }
            }
        }

        if let Some((k1, k2)) = best_3way {
            let (l1_str, r1) = spread_gutter_line_layout(&cleaned_refs[..k1], max_w);
            let (l2_str, r2) = spread_gutter_line_layout(&cleaned_refs[k1..k2], max_w);
            let (l3_str, r3) = spread_gutter_line_layout(&cleaned_refs[k2..], max_w);
            let items1: Vec<(String, usize, usize)> = cleaned_refs[..k1]
                .iter()
                .zip(r1)
                .map(|(&txt, (s, e))| (txt.to_string(), s, e))
                .collect();
            let items2: Vec<(String, usize, usize)> = cleaned_refs[k1..k2]
                .iter()
                .zip(r2)
                .map(|(&txt, (s, e))| (txt.to_string(), s, e))
                .collect();
            let items3: Vec<(String, usize, usize)> = cleaned_refs[k2..]
                .iter()
                .zip(r3)
                .map(|(&txt, (s, e))| (txt.to_string(), s, e))
                .collect();
            return vec![(l1_str, items1), (l2_str, items2), (l3_str, items3)];
        }
    }

    // 4. Fallback: Greedy packing into at most max_lines
    let mut packed_lines: Vec<Vec<&str>> = Vec::new();
    let mut remaining = &cleaned_refs[..];

    for _ in 0..max_lines {
        if remaining.is_empty() {
            break;
        }
        let mut cur_line_items = Vec::new();
        let mut cur_len = 0;
        while !remaining.is_empty() {
            let it = remaining[0];
            let needed = if cur_line_items.is_empty() {
                gutter_text_width(it)
            } else {
                3 + gutter_text_width(it)
            };
            if cur_len + needed <= max_w {
                cur_line_items.push(it);
                cur_len += needed;
                remaining = &remaining[1..];
            } else {
                break;
            }
        }
        if cur_line_items.is_empty() && !remaining.is_empty() {
            cur_line_items.push(remaining[0]);
            remaining = &remaining[1..];
        }
        packed_lines.push(cur_line_items);
    }

    let mut result = Vec::new();
    for line_items in packed_lines {
        if !line_items.is_empty() {
            let (s, r) = spread_gutter_line_layout(&line_items, max_w);
            let items: Vec<(String, usize, usize)> = line_items
                .iter()
                .zip(r)
                .map(|(&txt, (s_c, e_c))| (txt.to_string(), s_c, e_c))
                .collect();
            result.push((s, items));
        }
    }
    result
}

/// Returns whether any items from `items` cannot fully fit in `max_lines` at `max_w`.
pub fn has_hidden_gutter_text(items: &[&str], max_w: usize, max_lines: usize) -> bool {
    if items.is_empty() || max_w == 0 || max_lines == 0 {
        return false;
    }
    let cleaned = parse_help_tokens(items);
    if cleaned.is_empty() {
        return false;
    }
    let layouts = wrap_spread_help_gutter_layout(items, max_w, max_lines);
    let displayed_count: usize = layouts.iter().map(|(_, l_items)| l_items.len()).sum();
    if displayed_count < cleaned.len() {
        return true;
    }
    // Check if any displayed item was truncated compared to original
    let mut idx = 0;
    for (_, l_items) in &layouts {
        for (tok, _, _) in l_items {
            if idx < cleaned.len() && tok != &cleaned[idx] {
                return true;
            }
            idx += 1;
        }
    }
    false
}

/// Tracks stationary mouse hover over a help gutter for tooltip expansion.
#[derive(Debug, Clone)]
pub struct HelpGutterHover {
    /// The last recorded mouse cell (column, row).
    hover_pos: Option<(u16, u16)>,
    /// Timestamp when mouse became stationary at `hover_pos`.
    hover_start: Option<Instant>,
    /// Whether the help gutter tooltip is currently expanded by an extra row.
    is_expanded: bool,
    /// Duration of stationary hover required to trigger expansion (default 4s).
    hover_duration: Duration,
}

impl Default for HelpGutterHover {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpGutterHover {
    /// Creates a new hover tracker with the standard 4-second delay.
    pub fn new() -> Self {
        Self {
            hover_pos: None,
            hover_start: None,
            is_expanded: false,
            hover_duration: Duration::from_secs(4),
        }
    }

    /// Creates a hover tracker with a custom hover duration.
    pub fn with_duration(hover_duration: Duration) -> Self {
        Self {
            hover_pos: None,
            hover_start: None,
            is_expanded: false,
            hover_duration,
        }
    }

    /// Sets the hover duration required before expanding the help gutter.
    pub fn set_duration(&mut self, hover_duration: Duration) {
        self.hover_duration = hover_duration;
    }

    /// Returns the current hover duration.
    pub fn duration(&self) -> Duration {
        self.hover_duration
    }

    /// Handles a mouse movement event.
    ///
    /// - If the mouse moves inside `gutter_bounds`, updates hover position.
    /// - If the mouse was stationary and moved to a new cell, any expansion is collapsed (unless duration is 0).
    /// - If the mouse moves outside the gutter, resets hover tracking.
    ///
    /// Returns `true` if the visual state changed (e.g. collapsed or expanded) and needs redrawing.
    pub fn handle_mouse_move(&mut self, col: u16, row: u16, in_gutter: bool) -> bool {
        if in_gutter {
            if self.hover_pos == Some((col, row)) {
                // Mouse stayed in the exact same cell; keep timer running
                false
            } else {
                let was_expanded = self.is_expanded;
                self.hover_pos = Some((col, row));
                self.hover_start = Some(Instant::now());
                if self.hover_duration.is_zero() {
                    self.is_expanded = true;
                    !was_expanded
                } else {
                    self.is_expanded = false;
                    was_expanded
                }
            }
        } else if self.hover_pos.is_some() || self.is_expanded {
            let changed = self.is_expanded;
            self.reset();
            changed
        } else {
            false
        }
    }

    /// Handles any user interaction (key press, mouse click, scroll).
    ///
    /// Collapses the expanded row and resets hover tracking.
    /// Returns `true` if previously expanded and needs redrawing.
    pub fn handle_interaction(&mut self) -> bool {
        let changed = self.is_expanded;
        self.reset();
        changed
    }

    /// Checks whether the hover threshold has been met.
    ///
    /// If stationary for >= `hover_duration` and `has_hidden_text` is true,
    /// expands the gutter.
    /// Returns `true` if newly expanded and needs redrawing.
    pub fn check_hover_timeout(&mut self, has_hidden_text: bool) -> bool {
        if !self.is_expanded && has_hidden_text {
            if self.hover_duration.is_zero() {
                if self.hover_pos.is_some() {
                    self.is_expanded = true;
                    return true;
                }
            } else if let Some(start) = self.hover_start {
                if start.elapsed() >= self.hover_duration {
                    self.is_expanded = true;
                    return true;
                }
            }
        }
        false
    }

    /// Returns whether the gutter is currently expanded by an extra row.
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }

    /// Resets hover coordinates, timer, and expansion state.
    pub fn reset(&mut self) {
        self.hover_pos = None;
        self.hover_start = None;
        self.is_expanded = false;
    }
}

/// Wraps help items into at most `max_lines` (typically 1 or 2),
/// treating the pipe character `|` as a word/delimiter boundary.
///
/// - If all items fit on 1 line: returns 1 line spread evenly across `max_w`.
/// - If items exceed 1 line and `max_lines >= 2`: distributes items as evenly as
///   possible across 2 lines and spreads each line across `max_w`.
pub fn wrap_spread_help_gutter(items: &[&str], max_w: usize, max_lines: usize) -> Vec<String> {
    wrap_spread_help_gutter_layout(items, max_w, max_lines)
        .into_iter()
        .map(|(line, _)| line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_gutter_line_exact_width() {
        let items = ["F1 Help", "F2 Themes", "ESC Exit"];
        let max_w = 40;
        let line = format_spread_gutter_line(&items, max_w);
        assert_eq!(gutter_text_width(&line), max_w);
        assert!(line.starts_with("F1 Help"));
        assert!(line.ends_with("ESC Exit"));
        assert!(line.contains(" | "));
    }

    #[test]
    fn test_spread_gutter_layout_hit_testing() {
        let items = ["A", "B", "C"];
        let max_w = 15;
        let (line, ranges) = spread_gutter_line_layout(&items, max_w);
        assert_eq!(gutter_text_width(&line), max_w);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], (0, 1)); // "A"
        assert_eq!(ranges[2].1, max_w); // "C" ends at max_w
    }

    #[test]
    fn test_wrap_spread_help_gutter_single_line_when_fits() {
        let items = ["[ESC]: Back", "[ENTER]: Select", "[N]: New Note"];
        let max_w = 80;
        let lines = wrap_spread_help_gutter(&items, max_w, 2);
        assert_eq!(lines.len(), 1, "Should fit on 1 line when max_w is ample");
        assert_eq!(gutter_text_width(&lines[0]), max_w);
    }

    #[test]
    fn test_wrap_spread_help_gutter_two_lines_when_needed() {
        let items = [
            "[UP/DN]: Nav",
            "[ENTER]: Run",
            "[F1]: Help",
            "[F2]: Themes",
            "[F3]: Edit",
            "[F4]: Menu",
            "[F5]: Reload",
            "[F6]: Toggle #s",
            "[F7]: Gutter",
            "[F8]: Theme Editor",
            "[ESC/q]: Exit",
        ];
        let max_w = 70;
        let lines = wrap_spread_help_gutter(&items, max_w, 2);
        assert_eq!(lines.len(), 2, "Should wrap across 2 lines");
        assert_eq!(gutter_text_width(&lines[0]), max_w);
        assert_eq!(gutter_text_width(&lines[1]), max_w);
    }

    #[test]
    fn test_wrap_spread_help_gutter_bbcode_conflict_brackets() {
        // [s] Save and [b] Back must NOT be stripped as BBCode tags strike or bold
        let items = [
            "[s] Save",
            "[b] Back",
            "[i] Info",
            "[u] Undo",
            "[ESC/q] Exit",
        ];
        let max_w = 60;
        let (line, ranges) = spread_gutter_line_layout(&items, max_w);
        assert_eq!(gutter_text_width(&line), max_w);
        assert_eq!(ranges.len(), 5);
        assert!(line.contains("[s] Save"));
        assert!(line.contains("[b] Back"));
        assert!(line.contains("[i] Info"));
        assert!(line.contains("[u] Undo"));
        assert!(line.ends_with("[ESC/q] Exit"));
        assert_eq!(ranges[4].1, max_w);
    }

    #[test]
    fn test_parse_help_tokens_treats_pipe_as_word_delimiter() {
        let input = ["[F1] Help | [F2] Toggle All | [F3] ASCII"];
        let tokens = parse_help_tokens(&input);
        assert_eq!(tokens, vec!["[F1] Help", "[F2] Toggle All", "[F3] ASCII"]);
    }

    #[test]
    fn test_wrap_spread_help_gutter_three_lines() {
        let items = [
            "[ENTER]: Select",
            "[F1]: Help",
            "[F2]: Themes",
            "[F3]: Editor",
            "[F4]: Edit",
            "[F5]: Refresh",
            "[F6]: Keys",
            "[F7]: Gutter",
            "[F8]: ThemeEdit",
            "[F9]: ShortcutEdit",
            "[F10]: Backup",
            "[F12]: Notes",
            "[ESC]: Back",
        ];
        let max_w = 45;
        // On width 45, 13 items cannot fit on 2 lines
        let lines2 = wrap_spread_help_gutter(&items, max_w, 2);
        assert_eq!(lines2.len(), 2);
        assert!(has_hidden_gutter_text(&items, max_w, 2));

        // When max_lines is 3, it should expand to 3 lines
        let lines3 = wrap_spread_help_gutter(&items, max_w, 3);
        assert_eq!(lines3.len(), 3);
    }

    #[test]
    fn test_has_hidden_gutter_text_detection() {
        let items = ["[F1]: Help", "[F2]: Themes", "[ESC]: Back"];
        assert!(!has_hidden_gutter_text(&items, 60, 2));

        let many_items = [
            "[ENTER]: Select",
            "[F1]: Help",
            "[F2]: Themes",
            "[F3]: Editor",
            "[F4]: Edit",
            "[F5]: Refresh",
            "[F6]: Keys",
            "[F7]: Gutter",
            "[F8]: ThemeEdit",
            "[F9]: ShortcutEdit",
            "[ESC]: Back",
        ];
        // In 40 cols, 11 items cannot fit in 2 lines
        assert!(has_hidden_gutter_text(&many_items, 40, 2));
    }

    #[test]
    fn test_help_gutter_hover_lifecycle() {
        let mut hover = HelpGutterHover::with_duration(Duration::from_millis(50));
        assert!(!hover.is_expanded());

        // Mouse enters gutter
        let changed = hover.handle_mouse_move(10, 20, true);
        assert!(!changed);
        assert!(!hover.is_expanded());

        // Same position does not reset timer
        assert!(!hover.handle_mouse_move(10, 20, true));

        // Check before duration expires
        assert!(!hover.check_hover_timeout(true));
        assert!(!hover.is_expanded());

        // Wait for duration to expire
        std::thread::sleep(Duration::from_millis(60));
        let expanded = hover.check_hover_timeout(true);
        assert!(expanded);
        assert!(hover.is_expanded());

        // Mouse moves to a new cell: should collapse
        let collapsed = hover.handle_mouse_move(12, 20, true);
        assert!(collapsed);
        assert!(!hover.is_expanded());

        // Interaction resets state
        hover.handle_interaction();
        assert!(!hover.is_expanded());
    }
}
