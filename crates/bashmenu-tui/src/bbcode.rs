//! BBCode formatting parser, tag detector, and bracket splitter for terminal UI.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Parses standard and extended color names or hex codes into `ratatui::style::Color`
pub fn parse_color(name: &str) -> Option<Color> {
    let clean = name.trim().to_lowercase();
    match clean.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "purple" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "lightred" | "brightred" | "bright_red" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "brightgreen" | "bright_green" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "brightyellow" | "bright_yellow" | "light_yellow" => {
            Some(Color::LightYellow)
        }
        "lightblue" | "brightblue" | "bright_blue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "brightmagenta" | "bright_magenta" | "light_magenta" => {
            Some(Color::LightMagenta)
        }
        "lightcyan" | "brightcyan" | "bright_cyan" | "light_cyan" => Some(Color::LightCyan),
        "reset" | "default" => Some(Color::Reset),
        _ => {
            if let Some(hex) = clean.strip_prefix('#') {
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    return Some(Color::Rgb(r, g, b));
                } else if hex.len() == 3 {
                    let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                    let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                    let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                    return Some(Color::Rgb(r, g, b));
                }
            }
            None
        }
    }
}

/// Checks if a bracketed token is a known console formatting tag
pub fn is_bbcode_tag(content: &str) -> bool {
    let clean = content.trim().to_lowercase();
    if matches!(
        clean.as_str(),
        "b" | "/b"
            | "bold"
            | "/bold"
            | "u"
            | "/u"
            | "underline"
            | "/underline"
            | "dim"
            | "/dim"
            | "i"
            | "/i"
            | "italic"
            | "/italic"
            | "reverse"
            | "/reverse"
            | "reversed"
            | "/reversed"
            | "invert"
            | "/invert"
            | "strike"
            | "/strike"
            | "s"
            | "/s"
            | "/"
            | "/color"
            | "/fg"
            | "/bg"
            | "/on"
    ) {
        return true;
    }

    if clean.starts_with("color=")
        || clean.starts_with("fg=")
        || clean.starts_with("bg=")
        || clean.starts_with("on ")
    {
        return true;
    }

    if let Some(color_name) = clean.strip_prefix('/') {
        return parse_color(color_name).is_some();
    }

    parse_color(&clean).is_some()
}

/// Identifies and splits standard `[brackets]` used for right-aligned text.
///
/// Returns `(left_label, right_bracket)`. If the trailing bracket is a BBCode
/// formatting tag (e.g. `[/b]`), it is NOT treated as a right-aligned bracket.
pub fn split_label_brackets(label: &str) -> (&str, &str) {
    let text = label.trim();
    if !text.ends_with(']') {
        return (label, "");
    }

    let mut brace_count = 0;
    let mut match_idx = None;

    for (idx, ch) in text.char_indices().rev() {
        if ch == ']' {
            brace_count += 1;
        } else if ch == '[' {
            brace_count -= 1;
            if brace_count == 0 {
                match_idx = Some(idx);
                break;
            }
        }
    }

    if let Some(idx) = match_idx {
        let left = text[..idx].trim_end();
        let right = &text[idx..];
        let content = right[1..right.len() - 1].trim();
        if is_bbcode_tag(content) {
            return (label, "");
        }
        return (left, right);
    }

    (label, "")
}

/// Parses a string with BBCode formatting tags into styled Ratatui `Span`s.
pub fn parse_bbcode_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut style_stack = vec![base_style];
    let mut current_style = base_style;
    let mut pending = String::new();

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Escaped bracket: \[
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '[' {
            pending.push('[');
            i += 2;
            continue;
        }

        if chars[i] == '[' {
            // Find closing ']'
            if let Some(rel_close) = chars[i + 1..].iter().position(|&c| c == ']') {
                let close_idx = i + 1 + rel_close;
                let tag_str: String = chars[i + 1..close_idx].iter().collect();

                if is_bbcode_tag(&tag_str) {
                    if !pending.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut pending), current_style));
                    }

                    let clean = tag_str.trim().to_lowercase();
                    if clean == "/"
                        || clean == "/color"
                        || clean == "/fg"
                        || clean == "/bg"
                        || clean == "/on"
                        || clean.starts_with('/')
                    {
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                    } else {
                        let mut new_style = current_style;
                        if clean == "b" || clean == "bold" {
                            new_style = new_style.add_modifier(Modifier::BOLD);
                        } else if clean == "dim" {
                            new_style = new_style.add_modifier(Modifier::DIM);
                        } else if clean == "u" || clean == "underline" {
                            new_style = new_style.add_modifier(Modifier::UNDERLINED);
                        } else if clean == "i" || clean == "italic" {
                            new_style = new_style.add_modifier(Modifier::ITALIC);
                        } else if clean == "reverse" || clean == "reversed" || clean == "invert" {
                            new_style = new_style.add_modifier(Modifier::REVERSED);
                        } else if clean == "strike" || clean == "s" {
                            new_style = new_style.add_modifier(Modifier::CROSSED_OUT);
                        } else if let Some(col_str) = clean.strip_prefix("color=") {
                            if let Some(c) = parse_color(col_str) {
                                new_style = new_style.fg(c);
                            }
                        } else if let Some(col_str) = clean.strip_prefix("fg=") {
                            if let Some(c) = parse_color(col_str) {
                                new_style = new_style.fg(c);
                            }
                        } else if let Some(col_str) = clean.strip_prefix("bg=") {
                            if let Some(c) = parse_color(col_str) {
                                new_style = new_style.bg(c);
                            }
                        } else if let Some(col_str) = clean.strip_prefix("on ") {
                            if let Some(c) = parse_color(col_str) {
                                new_style = new_style.bg(c);
                            }
                        } else if let Some(c) = parse_color(&clean) {
                            new_style = new_style.fg(c);
                        }

                        style_stack.push(new_style);
                        current_style = new_style;
                    }

                    i = close_idx + 1;
                    continue;
                }
            }
        }

        pending.push(chars[i]);
        i += 1;
    }

    if !pending.is_empty() {
        spans.push(Span::styled(pending, current_style));
    }

    spans
}

/// Strips all BBCode formatting tags from a string
pub fn strip_bbcode(text: &str) -> String {
    let spans = parse_bbcode_spans(text, Style::default());
    spans.into_iter().map(|s| s.content.into_owned()).collect()
}

/// Visible character length excluding BBCode markup
pub fn visible_text_len(text: &str) -> usize {
    strip_bbcode(text).chars().count()
}

/// Truncates a string to at most `max_w` visible characters, preserving BBCode formatting tags
pub fn truncate_to_width(text: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }

    let mut vis = 0;
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if vis + 1 > max_w {
                break;
            }
            out.push('[');
            vis += 1;
            i += 2;
            continue;
        }

        if chars[i] == '[' {
            if let Some(rel_close) = chars[i + 1..].iter().position(|&c| c == ']') {
                let close_idx = i + 1 + rel_close;
                let tag_candidate: String = chars[i + 1..close_idx].iter().collect();
                if is_bbcode_tag(&tag_candidate) {
                    out.push_str(&chars[i..=close_idx].iter().collect::<String>());
                    i = close_idx + 1;
                    continue;
                }
            }
        }

        if vis + 1 > max_w {
            break;
        }
        out.push(chars[i]);
        vis += 1;
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_bbcode_tag() {
        assert!(is_bbcode_tag("b"));
        assert!(is_bbcode_tag("/b"));
        assert!(is_bbcode_tag("bold"));
        assert!(is_bbcode_tag("green"));
        assert!(is_bbcode_tag("/green"));
        assert!(is_bbcode_tag("#00ff00"));
        assert!(is_bbcode_tag("/#00ff00"));
        assert!(is_bbcode_tag("color=cyan"));
        assert!(is_bbcode_tag("on red"));

        assert!(!is_bbcode_tag("80x48"));
        assert!(!is_bbcode_tag("themeedit"));
        assert!(!is_bbcode_tag("/path/to/notes.yml"));
        assert!(!is_bbcode_tag("Resume Last Session"));
    }

    #[test]
    fn test_split_label_brackets() {
        let (left, right) = split_label_brackets("Notes [/path/to/notes.yml]");
        assert_eq!(left, "Notes");
        assert_eq!(right, "[/path/to/notes.yml]");

        let (left, right) = split_label_brackets("Theme Editor [themeedit]");
        assert_eq!(left, "Theme Editor");
        assert_eq!(right, "[themeedit]");

        let (left, right) = split_label_brackets("[b]Bashmenu Settings[/b]");
        assert_eq!(left, "[b]Bashmenu Settings[/b]");
        assert_eq!(right, "");

        let (left, right) = split_label_brackets("Plain Item");
        assert_eq!(left, "Plain Item");
        assert_eq!(right, "");
    }

    #[test]
    fn test_parse_bbcode_spans() {
        let base = Style::default();
        let spans = parse_bbcode_spans("Hello [b]World[/b] [green]OK[/green]", base);
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].content, "Hello ");
        assert_eq!(spans[1].content, "World");
        assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[2].content, " ");
        assert_eq!(spans[3].content, "OK");
        assert_eq!(spans[3].style.fg, Some(Color::Green));
    }

    #[test]
    fn test_visible_text_len() {
        assert_eq!(visible_text_len("[b]Alienware[/b]"), 9);
        assert_eq!(visible_text_len("[green]ONLINE[/green]"), 6);
        assert_eq!(visible_text_len("Plain"), 5);
    }

    #[test]
    fn test_truncate_to_width() {
        let text = "[b]Long Label[/b]";
        let trunc = truncate_to_width(text, 4);
        assert_eq!(strip_bbcode(&trunc), "Long");
    }
}
