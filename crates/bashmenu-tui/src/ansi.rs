//! ANSI escape code parser and conversion to Ratatui Spans.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Converts an ANSI-escaped text line into a Ratatui `Line` of styled `Span`s.
pub fn ansi_to_line(text: &str, default_style: Style) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current_style = default_style;
    let mut buf = String::new();

    let bytes = text.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == 0x1b && idx + 1 < bytes.len() && bytes[idx + 1] == b'[' {
            // Flush text before ANSI code
            if !buf.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buf), current_style));
            }

            idx += 2; // skip \x1b[
            let start_param = idx;
            while idx < bytes.len() && !bytes[idx].is_ascii_alphabetic() {
                idx += 1;
            }

            if idx < bytes.len() {
                let cmd = bytes[idx];
                let param_str = std::str::from_utf8(&bytes[start_param..idx]).unwrap_or("");
                idx += 1; // skip final letter

                if cmd == b'm' {
                    // SGR sequence
                    current_style = apply_sgr(param_str, current_style, default_style);
                }
            }
        } else {
            buf.push(bytes[idx] as char);
            idx += 1;
        }
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, current_style));
    }

    if spans.is_empty() {
        Line::from(vec![Span::styled("", default_style)])
    } else {
        Line::from(spans)
    }
}

/// Applies SGR parameters (semicolon-delimited) to an existing style
fn apply_sgr(param_str: &str, mut style: Style, default_style: Style) -> Style {
    if param_str.is_empty() || param_str == "0" {
        return default_style;
    }

    let parts: Vec<u32> = param_str
        .split(';')
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    let mut i = 0;
    while i < parts.len() {
        match parts[i] {
            0 => style = default_style,
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            22 => {
                style = style
                    .remove_modifier(Modifier::BOLD)
                    .remove_modifier(Modifier::DIM)
            }
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            27 => style = style.remove_modifier(Modifier::REVERSED),
            // Foreground standard (30..=37)
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::Gray),
            39 => style.fg = default_style.fg,
            // Background standard (40..=47)
            40 => style = style.bg(Color::Black),
            41 => style = style.bg(Color::Red),
            42 => style = style.bg(Color::Green),
            43 => style = style.bg(Color::Yellow),
            44 => style = style.bg(Color::Blue),
            45 => style = style.bg(Color::Magenta),
            46 => style = style.bg(Color::Cyan),
            47 => style = style.bg(Color::Gray),
            49 => style.bg = default_style.bg,
            // Bright foreground (90..=97)
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::White),
            // Bright background (100..=107)
            100 => style = style.bg(Color::DarkGray),
            101 => style = style.bg(Color::LightRed),
            102 => style = style.bg(Color::LightGreen),
            103 => style = style.bg(Color::LightYellow),
            104 => style = style.bg(Color::LightBlue),
            105 => style = style.bg(Color::LightMagenta),
            106 => style = style.bg(Color::LightCyan),
            107 => style = style.bg(Color::White),
            // Extended 256 or RGB color
            38 => {
                if i + 2 < parts.len() && parts[i + 1] == 5 {
                    style = style.fg(Color::Indexed(parts[i + 2] as u8));
                    i += 2;
                } else if i + 4 < parts.len() && parts[i + 1] == 2 {
                    style = style.fg(Color::Rgb(
                        parts[i + 2] as u8,
                        parts[i + 3] as u8,
                        parts[i + 4] as u8,
                    ));
                    i += 4;
                }
            }
            48 => {
                if i + 2 < parts.len() && parts[i + 1] == 5 {
                    style = style.bg(Color::Indexed(parts[i + 2] as u8));
                    i += 2;
                } else if i + 4 < parts.len() && parts[i + 1] == 2 {
                    style = style.bg(Color::Rgb(
                        parts[i + 2] as u8,
                        parts[i + 3] as u8,
                        parts[i + 4] as u8,
                    ));
                    i += 4;
                }
            }
            _ => {}
        }
        i += 1;
    }

    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_to_line_parsing() {
        let text = "\x1b[31mRed text\x1b[0m and \x1b[1;32mBold Green\x1b[0m";
        let line = ansi_to_line(text, Style::default());
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "Red text");
        assert_eq!(line.spans[0].style.fg, Some(Color::Red));
        assert_eq!(line.spans[1].content, " and ");
        assert_eq!(line.spans[2].content, "Bold Green");
        assert_eq!(line.spans[2].style.fg, Some(Color::Green));
        assert!(line.spans[2].style.add_modifier.contains(Modifier::BOLD));
    }
}
