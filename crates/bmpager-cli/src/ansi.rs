//! ANSI escape sequence utilities for terminal text clipping and formatting.

use regex::Regex;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthChar;

/// Regular expression matching ANSI CSI control sequences (`\x1b[...]`).
static ANSI_CSI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[a-zA-Z]").unwrap());

/// Regular expression matching ANSI OSC operating system command sequences (`\x1b]...`).
static ANSI_OSC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\][^\x07\x1b]*(\x07|\x1b\\)").unwrap());

/// Strips all ANSI escape codes (CSI and OSC) from a string.
pub fn strip_ansi(text: &str) -> String {
    let no_csi = ANSI_CSI_RE.replace_all(text, "");
    ANSI_OSC_RE.replace_all(&no_csi, "").to_string()
}

/// Computes the visual display width of a string in terminal cells, ignoring ANSI codes.
pub fn visual_width(text: &str) -> usize {
    let clean = strip_ansi(text);
    clean
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

/// Clips an ANSI string to fit within `max_cols` terminal columns.
///
/// If `strip_colors` is true, all ANSI codes are stripped before clipping.
/// Otherwise, ANSI codes are passed through without counting towards width,
/// and `\x1b[0m` is appended if truncated or formatted.
pub fn clip_ansi_line(line: &str, max_cols: usize, strip_colors: bool) -> String {
    if strip_colors {
        let clean = strip_ansi(line);
        let mut out = String::new();
        let mut used = 0;
        for c in clean.chars() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + w > max_cols {
                break;
            }
            out.push(c);
            used += w;
        }
        return out;
    }

    if !line.contains('\x1b') {
        let mut out = String::new();
        let mut used = 0;
        for c in line.chars() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + w > max_cols {
                break;
            }
            out.push(c);
            used += w;
        }
        return out;
    }

    let mut out = String::new();
    let mut used = 0;
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut had_ansi = false;

    while i < bytes.len() && used < max_cols {
        if bytes[i] == 0x1b {
            had_ansi = true;
            // Check for CSI \x1b[ ...
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                let start = i;
                i += 2;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    if b.is_ascii_alphabetic() {
                        break;
                    }
                }
                out.push_str(&line[start..i]);
                continue;
            }
            // Check for OSC \x1b] ... (\x07 or \x1b\)
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                let start = i;
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                out.push_str(&line[start..i]);
                continue;
            }

            // Unrecognized escape: consume bare byte
            out.push('\x1b');
            i += 1;
            continue;
        }

        // Regular UTF-8 char
        let rem = &line[i..];
        if let Some(c) = rem.chars().next() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + w > max_cols {
                break;
            }
            out.push(c);
            used += w;
            i += c.len_utf8();
        } else {
            break;
        }
    }

    if had_ansi || used >= max_cols {
        out.push_str("\x1b[0m");
    }
    out
}

/// Splits an ANSI string at visual column `max_cols` into `(head, tail)`.
/// `head` has at most `max_cols` visual width and ends with `\x1b[0m` if formatted.
pub fn split_ansi_line(line: &str, max_cols: usize) -> (String, String) {
    if max_cols == 0 {
        return (String::new(), line.to_string());
    }
    let mut out = String::new();
    let mut used = 0;
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut had_ansi = false;

    while i < bytes.len() && used < max_cols {
        if bytes[i] == 0x1b {
            had_ansi = true;
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                let start = i;
                i += 2;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    if b.is_ascii_alphabetic() {
                        break;
                    }
                }
                out.push_str(&line[start..i]);
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                let start = i;
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                out.push_str(&line[start..i]);
                continue;
            }
            out.push('\x1b');
            i += 1;
            continue;
        }

        let rem = &line[i..];
        if let Some(c) = rem.chars().next() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + w > max_cols {
                break;
            }
            out.push(c);
            used += w;
            i += c.len_utf8();
        } else {
            break;
        }
    }

    let remainder = line[i..].to_string();
    if had_ansi {
        out.push_str("\x1b[0m");
    }
    (out, remainder)
}

/// Soft word-wraps a single ANSI or plain text line to fit within `max_width` visual columns.
///
/// Leading whitespace / indentation is preserved on the first line. Continuation lines
/// inherit any active ANSI formatting state if formatted, or simply wrap cleanly.
/// If a single word exceeds `max_width`, it is split cleanly into chunks using `split_ansi_line`.
pub fn wrap_text_line(line: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }

    if visual_width(line) <= max_width {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut current_vis = 0;

    // Preserve leading whitespace if any
    let num_leading_spaces = line
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
        .count();
    let (indent, content) = if num_leading_spaces > 0 && num_leading_spaces < max_width {
        let prefix: String = line.chars().take(num_leading_spaces).collect();
        let rest = &line[prefix.len()..];
        (prefix, rest)
    } else {
        (String::new(), line)
    };

    if !indent.is_empty() {
        current_vis = visual_width(&indent);
        current_line.push_str(&indent);
    }

    // Split words on whitespace while keeping token boundaries intact
    for word in content.split_whitespace() {
        let word_vis = visual_width(word);

        if current_line.is_empty() || current_line == indent {
            if current_vis + word_vis <= max_width {
                current_line.push_str(word);
                current_vis += word_vis;
            } else {
                // Word alone exceeds available line width; split into hard-wrapped chunks
                let mut rem = word.to_string();
                while !rem.is_empty() {
                    let avail = max_width.saturating_sub(current_vis).max(1);
                    let (chunk, tail) = split_ansi_line(&rem, avail);
                    if chunk.is_empty() {
                        let mut chars = rem.chars();
                        if let Some(ch) = chars.next() {
                            current_line.push(ch);
                            rem = chars.as_str().to_string();
                        } else {
                            break;
                        }
                    } else {
                        current_line.push_str(&chunk);
                        rem = tail;
                    }
                    if !rem.is_empty() {
                        result.push(current_line);
                        current_line = String::new();
                        current_vis = 0;
                    }
                }
                current_vis = visual_width(&current_line);
            }
        } else if current_vis + 1 + word_vis <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
            current_vis += 1 + word_vis;
        } else {
            result.push(current_line);
            current_line = String::new();

            if word_vis <= max_width {
                current_line.push_str(word);
                current_vis = word_vis;
            } else {
                let mut rem = word.to_string();
                while !rem.is_empty() {
                    let (chunk, tail) = split_ansi_line(&rem, max_width);
                    if chunk.is_empty() {
                        let mut chars = rem.chars();
                        if let Some(ch) = chars.next() {
                            current_line.push(ch);
                            rem = chars.as_str().to_string();
                        } else {
                            break;
                        }
                    } else {
                        current_line.push_str(&chunk);
                        rem = tail;
                    }
                    if !rem.is_empty() {
                        result.push(current_line);
                        current_line = String::new();
                    }
                }
                current_vis = visual_width(&current_line);
            }
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

/// Wraps all text lines in `raw_content` to `max_width` columns if wrap is enabled.
pub fn wrap_text_content(raw_content: &str, max_width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw_content.lines() {
        let wrapped = wrap_text_line(line, max_width);
        out.extend(wrapped);
    }
    // If original content ends with a newline and raw_content is not empty, lines() handles all lines
    if raw_content.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[1;32mHello\x1b[0m \x1b]8;;http://example.com\x1b\\link\x1b]8;;\x1b\\";
        assert_eq!(strip_ansi(input), "Hello link");
    }

    #[test]
    fn test_visual_width() {
        let input = "\x1b[1;31mHello World!\x1b[0m";
        assert_eq!(visual_width(input), 12);
    }

    #[test]
    fn test_clip_ansi_line() {
        let input = "\x1b[1;34m1234567890\x1b[0m";
        let clipped = clip_ansi_line(input, 5, false);
        assert_eq!(strip_ansi(&clipped), "12345");
        assert!(clipped.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_split_ansi_line() {
        let input = "\x1b[1;34m1234567890\x1b[0m";
        let (head, tail) = split_ansi_line(input, 5);
        assert_eq!(strip_ansi(&head), "12345");
        assert_eq!(strip_ansi(&tail), "67890");
    }

    #[test]
    fn test_wrap_text_line_plain() {
        let line = "The quick brown fox jumps over the lazy dog";
        let wrapped = wrap_text_line(line, 15);
        assert_eq!(
            wrapped,
            vec![
                "The quick brown".to_string(),
                "fox jumps over".to_string(),
                "the lazy dog".to_string(),
            ]
        );
    }

    #[test]
    fn test_wrap_text_line_with_indent() {
        let line = "    indented line that needs wrapping";
        let wrapped = wrap_text_line(line, 20);
        assert_eq!(wrapped[0], "    indented line");
        assert_eq!(wrapped[1], "that needs wrapping");
    }

    #[test]
    fn test_wrap_text_line_long_word() {
        let line = "abcdefghijklmnopqrstuvwxyz";
        let wrapped = wrap_text_line(line, 10);
        assert_eq!(
            wrapped,
            vec![
                "abcdefghij".to_string(),
                "klmnopqrst".to_string(),
                "uvwxyz".to_string(),
            ]
        );
    }
}
