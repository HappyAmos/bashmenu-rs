//! Standardized Help Gutter and Footer layout engine.
//!
//! Follows the universal BashMenu rules:
//! 1. Help gutters may use up to two lines.
//! 2. Help text treats the pipe character `|` as a word boundary for wrapping.
//! 3. Text is spread evenly across one (if help is only one line) or two lines,
//!    spreading items apart across the full available width with centered pipe delimiters.

use crate::bbcode::{truncate_to_width, visible_text_len};

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
        let text = if visible_text_len(items[0]) > max_w {
            truncate_to_width(items[0], max_w)
        } else {
            items[0].to_string()
        };
        let len = visible_text_len(&text);
        return (text, vec![(0, len)]);
    }

    let num_gaps = items.len() - 1;
    let item_lens: Vec<usize> = items.iter().map(|s| visible_text_len(s)).collect();
    let tot_item_len: usize = item_lens.iter().sum();
    let min_line_len = tot_item_len + num_gaps * 3; // 3 chars per gap: " | "

    if min_line_len >= max_w {
        // Not enough space to spread apart; join with minimal " | " and truncate if needed
        let mut line = String::new();
        let mut ranges = Vec::new();
        let mut cur_col = 0;
        for (i, &it) in items.iter().enumerate() {
            let it_len = visible_text_len(it);
            ranges.push((cur_col, cur_col + it_len));
            line.push_str(it);
            cur_col += it_len;
            if i < num_gaps {
                line.push_str(" | ");
                cur_col += 3;
            }
        }
        if visible_text_len(&line) > max_w {
            line = truncate_to_width(&line, max_w);
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
        let it_len = visible_text_len(it);
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

/// Wraps help items into at most `max_lines` (typically 1 or 2),
/// treating the pipe character `|` as a word/delimiter boundary.
///
/// - If all items fit on 1 line: returns 1 line spread evenly across `max_w`.
/// - If items exceed 1 line and `max_lines >= 2`: distributes items as evenly as
///   possible across 2 lines and spreads each line across `max_w`.
pub fn wrap_spread_help_gutter(items: &[&str], max_w: usize, max_lines: usize) -> Vec<String> {
    if items.is_empty() || max_w == 0 || max_lines == 0 {
        return Vec::new();
    }

    let cleaned = parse_help_tokens(items);
    if cleaned.is_empty() {
        return Vec::new();
    }
    let cleaned_refs: Vec<&str> = cleaned.iter().map(|s| s.as_str()).collect();

    // 1. Check if all items fit on 1 line
    let total_item_len: usize = cleaned_refs.iter().map(|s| visible_text_len(s)).sum();
    let min_1_line_len = total_item_len + cleaned_refs.len().saturating_sub(1) * 3;

    if min_1_line_len <= max_w || max_lines == 1 {
        if min_1_line_len <= max_w {
            return vec![format_spread_gutter_line(&cleaned_refs, max_w)];
        }
        // If max_lines == 1 and doesn't fit, pack what fits
        let mut fit_items = Vec::new();
        let mut cur_len = 0;
        for &item in &cleaned_refs {
            let needed = if fit_items.is_empty() {
                visible_text_len(item)
            } else {
                3 + visible_text_len(item)
            };
            if cur_len + needed <= max_w {
                fit_items.push(item);
                cur_len += needed;
            } else {
                break;
            }
        }
        if fit_items.is_empty() && !cleaned_refs.is_empty() {
            fit_items.push(cleaned_refs[0]);
        }
        return vec![format_spread_gutter_line(&fit_items, max_w)];
    }

    // 2. Needs 2 lines (max_lines >= 2)
    // Find the most balanced split point k such that both lines fit in max_w
    let n = cleaned_refs.len();
    let mut best_k: Option<usize> = None;
    let mut best_imbalance = usize::MAX;

    for k in 1..n {
        let l1 = &cleaned_refs[..k];
        let l2 = &cleaned_refs[k..];

        let l1_len: usize = l1.iter().map(|s| visible_text_len(s)).sum::<usize>()
            + (l1.len().saturating_sub(1) * 3);
        let l2_len: usize = l2.iter().map(|s| visible_text_len(s)).sum::<usize>()
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
        return vec![
            format_spread_gutter_line(&cleaned_refs[..k], max_w),
            format_spread_gutter_line(&cleaned_refs[k..], max_w),
        ];
    }

    // Fallback: Greedy 2-line packing if items cannot cleanly split without overflow
    let mut l1_items = Vec::new();
    let mut cur_len1 = 0;
    let mut remaining = &cleaned_refs[..];

    while !remaining.is_empty() {
        let it = remaining[0];
        let needed = if l1_items.is_empty() {
            visible_text_len(it)
        } else {
            3 + visible_text_len(it)
        };
        if cur_len1 + needed <= max_w {
            l1_items.push(it);
            cur_len1 += needed;
            remaining = &remaining[1..];
        } else {
            break;
        }
    }

    let mut l2_items = Vec::new();
    let mut cur_len2 = 0;
    while !remaining.is_empty() {
        let it = remaining[0];
        let needed = if l2_items.is_empty() {
            visible_text_len(it)
        } else {
            3 + visible_text_len(it)
        };
        if cur_len2 + needed <= max_w {
            l2_items.push(it);
            cur_len2 += needed;
            remaining = &remaining[1..];
        } else {
            break;
        }
    }

    let mut result = Vec::new();
    if !l1_items.is_empty() {
        result.push(format_spread_gutter_line(&l1_items, max_w));
    }
    if !l2_items.is_empty() {
        result.push(format_spread_gutter_line(&l2_items, max_w));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_gutter_line_exact_width() {
        let items = ["F1 Help", "F2 Themes", "ESC Exit"];
        let max_w = 40;
        let line = format_spread_gutter_line(&items, max_w);
        assert_eq!(visible_text_len(&line), max_w);
        assert!(line.starts_with("F1 Help"));
        assert!(line.ends_with("ESC Exit"));
        assert!(line.contains(" | "));
    }

    #[test]
    fn test_spread_gutter_layout_hit_testing() {
        let items = ["A", "B", "C"];
        let max_w = 15;
        let (line, ranges) = spread_gutter_line_layout(&items, max_w);
        assert_eq!(visible_text_len(&line), max_w);
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
        assert_eq!(visible_text_len(&lines[0]), max_w);
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
        assert_eq!(visible_text_len(&lines[0]), max_w);
        assert_eq!(visible_text_len(&lines[1]), max_w);
    }

    #[test]
    fn test_parse_help_tokens_treats_pipe_as_word_delimiter() {
        let input = ["[F1] Help | [F2] Toggle All | [F3] ASCII"];
        let tokens = parse_help_tokens(&input);
        assert_eq!(tokens, vec!["[F1] Help", "[F2] Toggle All", "[F3] ASCII"]);
    }
}
