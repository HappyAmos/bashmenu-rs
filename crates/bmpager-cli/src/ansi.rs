//! ANSI escape sequence utilities for terminal text clipping and formatting.

use regex::Regex;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthChar;

static ANSI_CSI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[a-zA-Z]").unwrap());
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
    if had_ansi || used >= max_cols {
        out.push_str("\x1b[0m");
    }
    (out, remainder)
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
}
