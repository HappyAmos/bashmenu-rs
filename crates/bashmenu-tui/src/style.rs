//! Theme color mapping and Ratatui style conversion.

use crate::borders::ResolvedWindowBorders;
use bashmenu_core::Theme;
use ratatui::style::{Color, Modifier, Style};

/// Resolved active theme styles for TUI rendering
#[derive(Debug, Clone)]
pub struct ResolvedThemeStyles {
    pub background: Color,
    pub border: Color,
    pub title: Style,
    pub text: Style,
    pub highlight: Style,
    pub accent: Style,
    pub footer: Style,
    pub gutter: Style,
    pub help_text: Style,
    pub selection: Style,
    pub shortcut_key: Style,
    pub divider: Style,
    pub window_close_button: Style,
    pub indicator_symbol: String,
    pub borders: ResolvedWindowBorders,
}

impl Default for ResolvedThemeStyles {
    fn default() -> Self {
        Self {
            background: Color::Reset,
            border: Color::Cyan,
            title: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::White),
            highlight: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            accent: Style::default().fg(Color::LightCyan),
            footer: Style::default().fg(Color::DarkGray),
            gutter: Style::default().fg(Color::DarkGray),
            help_text: Style::default().fg(Color::Cyan),
            selection: Style::default().fg(Color::Black).bg(Color::Cyan),
            shortcut_key: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            divider: Style::default().fg(Color::Cyan),
            window_close_button: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            indicator_symbol: "► ".to_string(),
            borders: ResolvedWindowBorders::default(),
        }
    }
}

/// Parses a color identifier (number, hex string, or named constant) into a Ratatui Color.
pub fn parse_color_value(val: &serde_yml::Value) -> Option<Color> {
    match val {
        serde_yml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i < 0 {
                    Some(Color::Reset)
                } else if i <= 255 {
                    Some(Color::Indexed(i as u8))
                } else {
                    None
                }
            } else {
                None
            }
        }
        serde_yml::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim == "-1" {
                return Some(Color::Reset);
            }
            if let Ok(i) = s_trim.parse::<i64>() {
                if i < 0 {
                    return Some(Color::Reset);
                } else if i <= 255 {
                    return Some(Color::Indexed(i as u8));
                }
            }
            if s_trim.starts_with('#') {
                let hex = s_trim.trim_start_matches('#');
                if hex.len() == 6 {
                    if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;
                        return Some(Color::Rgb(r, g, b));
                    }
                }
            }
            match s_trim.to_uppercase().as_str() {
                "COLOR_BLACK" | "BLACK" => Some(Color::Black),
                "COLOR_RED" | "RED" => Some(Color::Red),
                "COLOR_GREEN" | "GREEN" => Some(Color::Green),
                "COLOR_YELLOW" | "YELLOW" => Some(Color::Yellow),
                "COLOR_BLUE" | "BLUE" => Some(Color::Blue),
                "COLOR_MAGENTA" | "MAGENTA" => Some(Color::Magenta),
                "COLOR_CYAN" | "CYAN" => Some(Color::Cyan),
                "COLOR_WHITE" | "WHITE" => Some(Color::White),
                "COLOR_GREY" | "COLOR_GRAY" | "GRAY" | "GREY" => Some(Color::DarkGray),
                "COLOR_BRIGHT_BLACK" => Some(Color::DarkGray),
                "COLOR_BRIGHT_RED" => Some(Color::LightRed),
                "COLOR_BRIGHT_GREEN" => Some(Color::LightGreen),
                "COLOR_BRIGHT_YELLOW" => Some(Color::LightYellow),
                "COLOR_BRIGHT_BLUE" => Some(Color::LightBlue),
                "COLOR_BRIGHT_MAGENTA" => Some(Color::LightMagenta),
                "COLOR_BRIGHT_CYAN" => Some(Color::LightCyan),
                "COLOR_BRIGHT_WHITE" => Some(Color::White),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Resolves a pair [fg, bg] into a Ratatui Style
pub fn parse_style_pair(val: Option<&serde_yml::Value>) -> Option<Style> {
    let v = val?;
    match v {
        serde_yml::Value::Sequence(seq) => {
            let mut style = Style::default();
            if let Some(fg_val) = seq.first() {
                if let Some(fg) = parse_color_value(fg_val) {
                    if fg != Color::Reset {
                        style = style.fg(fg);
                    }
                }
            }
            if let Some(bg_val) = seq.get(1) {
                if let Some(bg) = parse_color_value(bg_val) {
                    if bg != Color::Reset {
                        style = style.bg(bg);
                    }
                }
            }
            Some(style)
        }
        other => parse_color_value(other).and_then(|c| {
            if c != Color::Reset {
                Some(Style::default().fg(c))
            } else {
                None
            }
        }),
    }
}

impl ResolvedThemeStyles {
    /// Resolves theme palette from Theme struct
    pub fn from_theme(theme: &Theme, use_nerd_fonts: bool) -> Self {
        let mut resolved = Self::default();

        // Indicator symbol
        if let Some(ind) = &theme.indicator {
            let resolved_sym = bashmenu_core::resolve_glyph(ind, use_nerd_fonts);
            if !resolved_sym.is_empty() {
                resolved.indicator_symbol = format!("{} ", resolved_sym);
            }
        }

        // Prefer 256 colors, fallback to 16
        let palette = theme.colors_256.as_ref().or(theme.colors_16.as_ref());

        if let Some(p) = palette {
            if let Some(bg) = p.get("background").and_then(parse_color_value) {
                resolved.background = bg;
            }
            if let Some(st) = parse_style_pair(p.get("border")) {
                resolved.border = st.fg.unwrap_or(Color::Cyan);
            }
            if let Some(st) = parse_style_pair(p.get("title")) {
                resolved.title = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair(p.get("text")) {
                resolved.text = st;
            }
            if let Some(st) = parse_style_pair(p.get("highlight")) {
                resolved.highlight = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair(p.get("accent")) {
                resolved.accent = st;
            }
            if let Some(st) = parse_style_pair(p.get("footer")) {
                resolved.footer = st;
            }
            if let Some(st) = parse_style_pair(p.get("gutter")) {
                resolved.gutter = st;
            }
            if let Some(st) = parse_style_pair(p.get("help_text")) {
                resolved.help_text = st;
            }
            if let Some(st) = parse_style_pair(p.get("selection")) {
                resolved.selection = st;
            }
            if let Some(st) = parse_style_pair(p.get("shortcut_key")) {
                resolved.shortcut_key = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair(p.get("divider")) {
                resolved.divider = st;
            }
            if let Some(st) = parse_style_pair(p.get("window_close_button")) {
                resolved.window_close_button = st.add_modifier(Modifier::BOLD);
            }
        }

        resolved.borders = ResolvedWindowBorders::from_theme_borders(theme.window.as_ref());

        resolved
    }
}
