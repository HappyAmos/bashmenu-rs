//! Theme color mapping and Ratatui style conversion.

use crate::borders::ResolvedWindowBorders;
use crate::wallpaper::{color_to_rgb, rgb_to_ansi16_name, rgb_to_ansi8_name};
use bashmenu_core::Theme;
use ratatui::style::{Color, Modifier, Style};

/// Terminal color depth capability tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ColorTier {
    /// 8 standard basic ANSI colors (e.g. Linux VT console TERM=linux)
    Color8,
    /// 16 high-intensity ANSI colors
    Color16,
    /// 256 indexed colors (xterm-256color)
    Color256,
    /// 24-bit TrueColor / RGB
    #[default]
    TrueColor,
}

impl ColorTier {
    /// Detects the active terminal color capability tier from environment
    /// variables.
    pub fn detect() -> Self {
        if let Ok(tier) = std::env::var("BASHMENU_COLOR_TIER") {
            match tier.trim().to_ascii_lowercase().as_str() {
                "8" | "color8" | "basic" => return Self::Color8,
                "16" | "color16" => return Self::Color16,
                "256" | "color256" => return Self::Color256,
                "truecolor" | "24bit" | "rgb" => return Self::TrueColor,
                _ => {}
            }
        }

        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let ct = colorterm.trim().to_ascii_lowercase();
            if ct == "truecolor" || ct == "24bit" {
                return Self::TrueColor;
            }
        }

        if let Ok(term) = std::env::var("TERM") {
            let t = term.trim().to_ascii_lowercase();
            if t == "linux" || t == "console" || t == "vt100" || t == "vt102" || t == "dumb" {
                return Self::Color8;
            }
            if t.contains("256color")
                || t.contains("kitty")
                || t.contains("alacritty")
                || t.contains("ghostty")
                || t.contains("foot")
                || t.contains("wezterm")
            {
                return Self::Color256;
            }
            if t.contains("16color") {
                return Self::Color16;
            }
        }

        Self::Color16
    }
}

/// Downsamples a Ratatui `Color` to the closest standard ANSI 8-color.
pub fn downsample_color_to_8(color: Color) -> Color {
    if color == Color::Reset {
        return Color::Reset;
    }
    match color {
        Color::LightRed => Color::Red,
        Color::LightGreen => Color::Green,
        Color::LightYellow => Color::Yellow,
        Color::LightBlue => Color::Blue,
        Color::LightMagenta => Color::Magenta,
        Color::LightCyan => Color::Cyan,
        Color::DarkGray => Color::Black,
        Color::Gray => Color::White,
        Color::Black
        | Color::Red
        | Color::Green
        | Color::Yellow
        | Color::Blue
        | Color::Magenta
        | Color::Cyan
        | Color::White => color,
        _ => {
            let (r, g, b) = color_to_rgb(color);
            let name = rgb_to_ansi8_name(r, g, b);
            ansi_name_to_color(name)
        }
    }
}

/// Downsamples a Ratatui `Color` to the closest standard ANSI 16-color.
pub fn downsample_color_to_16(color: Color) -> Color {
    if color == Color::Reset {
        return Color::Reset;
    }
    match color {
        Color::Black
        | Color::Red
        | Color::Green
        | Color::Yellow
        | Color::Blue
        | Color::Magenta
        | Color::Cyan
        | Color::Gray
        | Color::DarkGray
        | Color::LightRed
        | Color::LightGreen
        | Color::LightYellow
        | Color::LightBlue
        | Color::LightMagenta
        | Color::LightCyan
        | Color::White => color,
        Color::Indexed(idx) if idx < 16 => ansi_index_to_color(idx),
        _ => {
            let (r, g, b) = color_to_rgb(color);
            let name = rgb_to_ansi16_name(r, g, b);
            ansi_name_to_color(name)
        }
    }
}

/// Maps a canonical ANSI color token name to a Ratatui `Color`.
pub fn ansi_name_to_color(name: &str) -> Color {
    match name {
        "COLOR_BLACK" | "BLACK" => Color::Black,
        "COLOR_RED" | "RED" => Color::Red,
        "COLOR_GREEN" | "GREEN" => Color::Green,
        "COLOR_YELLOW" | "YELLOW" => Color::Yellow,
        "COLOR_BLUE" | "BLUE" => Color::Blue,
        "COLOR_MAGENTA" | "MAGENTA" => Color::Magenta,
        "COLOR_CYAN" | "CYAN" => Color::Cyan,
        "COLOR_WHITE" | "WHITE" => Color::White,
        "COLOR_GREY" | "COLOR_GRAY" | "GRAY" => Color::DarkGray,
        "COLOR_BRIGHT_BLACK" => Color::DarkGray,
        "COLOR_BRIGHT_RED" => Color::LightRed,
        "COLOR_BRIGHT_GREEN" => Color::LightGreen,
        "COLOR_BRIGHT_YELLOW" => Color::LightYellow,
        "COLOR_BRIGHT_BLUE" => Color::LightBlue,
        "COLOR_BRIGHT_MAGENTA" => Color::LightMagenta,
        "COLOR_BRIGHT_CYAN" => Color::LightCyan,
        "COLOR_BRIGHT_WHITE" => Color::White,
        _ => Color::White,
    }
}

/// Maps an ANSI 0..15 index to a Ratatui `Color`.
pub fn ansi_index_to_color(idx: u8) -> Color {
    match idx {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        _ => Color::White,
    }
}

/// Resolved active theme styles for TUI rendering
#[derive(Debug, Clone)]
pub struct ResolvedThemeStyles {
    /// Window background color fill
    pub background: Color,
    /// Window frame and border line color
    pub border: Color,
    /// Header/title bar style
    pub title: Style,
    /// Standard body text style
    pub text: Style,
    /// Highlighted/selected item style
    pub highlight: Style,
    /// Accent color style (e.g., active elements or key markers)
    pub accent: Style,
    /// Footer bar text style
    pub footer: Style,
    /// Status/help gutter bar style
    pub gutter: Style,
    /// Contextual help text style
    pub help_text: Style,
    /// Plugin status display row style
    pub plugin: Style,
    /// Text selection highlight style
    pub selection: Style,
    /// Shortcut key indicator style (e.g., [1] or <F1>)
    pub shortcut_key: Style,
    /// Divider rule style
    pub divider: Style,
    /// Divider rule character
    pub divider_char: String,
    /// Whether divider spans the full screen width
    pub divider_is_screen_width: bool,
    /// Character used as delimiter in horizontal marquee tickers
    pub marquee_char: String,
    /// Space padding gap between marquee repetitions
    pub marquee_gap: usize,
    /// Window close button indicator style (e.g. `[X]`)
    pub window_close_button: Style,
    /// Selection indicator symbol (e.g. `> `)
    pub indicator_symbol: String,
    /// Resolved box-drawing glyphs for window borders
    pub borders: ResolvedWindowBorders,
    /// Toast notification vertical accent indicator bar glyph
    pub toast_bar_char: String,
    /// Default display duration in seconds for toast popups
    pub toast_timeout: f64,
    /// Toast informational severity color
    pub toast_info: Color,
    /// Toast warning severity color
    pub toast_warning: Color,
    /// Toast error severity color
    pub toast_error: Color,
    /// Scrollbar track and thumb styling
    pub scrollbar: Style,
}

impl Default for ResolvedThemeStyles {
    /// Returns default fallback TUI styles when no custom theme is loaded.
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
            plugin: Style::default().fg(Color::Cyan),
            selection: Style::default().fg(Color::Black).bg(Color::Cyan),
            shortcut_key: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            divider: Style::default().fg(Color::Cyan),
            divider_char: "─".to_string(),
            divider_is_screen_width: false,
            marquee_char: "•".to_string(),
            marquee_gap: 4,
            window_close_button: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            indicator_symbol: "► ".to_string(),
            borders: ResolvedWindowBorders::default(),
            toast_bar_char: "▌".to_string(),
            toast_timeout: 2.0,
            toast_info: Color::Rgb(78, 191, 113),
            toast_warning: Color::Rgb(255, 184, 108),
            toast_error: Color::Rgb(255, 85, 85),
            scrollbar: Style::default().fg(Color::Cyan),
        }
    }
}

/// Parses a color identifier (number, hex string, or named constant) into a
/// Ratatui Color respecting the given terminal color tier.
pub fn parse_color_value_with_tier(val: &serde_yml::Value, tier: ColorTier) -> Option<Color> {
    let raw_color = match val {
        serde_yml::Value::Sequence(seq) => seq
            .first()
            .and_then(|v| parse_color_value_with_tier(v, tier)),
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
                        return Some(match tier {
                            ColorTier::Color8 => downsample_color_to_8(Color::Rgb(r, g, b)),
                            ColorTier::Color16 => downsample_color_to_16(Color::Rgb(r, g, b)),
                            ColorTier::Color256 | ColorTier::TrueColor => Color::Rgb(r, g, b),
                        });
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
    }?;

    Some(match tier {
        ColorTier::Color8 => downsample_color_to_8(raw_color),
        ColorTier::Color16 => downsample_color_to_16(raw_color),
        ColorTier::Color256 | ColorTier::TrueColor => raw_color,
    })
}

/// Parses a color identifier (number, hex string, or named constant) into a
/// Ratatui Color using the detected environment color tier.
pub fn parse_color_value(val: &serde_yml::Value) -> Option<Color> {
    parse_color_value_with_tier(val, ColorTier::detect())
}

/// Resolves a pair [fg, bg] into a Ratatui Style respecting the color tier.
pub fn parse_style_pair_with_tier(
    val: Option<&serde_yml::Value>,
    tier: ColorTier,
) -> Option<Style> {
    let v = val?;
    match v {
        serde_yml::Value::Sequence(seq) => {
            let mut style = Style::default();
            if let Some(fg_val) = seq.first() {
                if let Some(fg) = parse_color_value_with_tier(fg_val, tier) {
                    if fg != Color::Reset {
                        style = style.fg(fg);
                    }
                }
            }
            if let Some(bg_val) = seq.get(1) {
                if let Some(bg) = parse_color_value_with_tier(bg_val, tier) {
                    if bg != Color::Reset {
                        style = style.bg(bg);
                    }
                }
            }
            Some(style)
        }
        other => parse_color_value_with_tier(other, tier).and_then(|c| {
            if c != Color::Reset {
                Some(Style::default().fg(c))
            } else {
                None
            }
        }),
    }
}

/// Resolves a pair [fg, bg] into a Ratatui Style using detected environment
/// color tier.
pub fn parse_style_pair(val: Option<&serde_yml::Value>) -> Option<Style> {
    parse_style_pair_with_tier(val, ColorTier::detect())
}

impl ResolvedThemeStyles {
    /// Resolves theme palette from Theme struct using a specific color tier.
    pub fn from_theme_with_tier(theme: &Theme, use_nerd_fonts: bool, tier: ColorTier) -> Self {
        let mut resolved = Self::default();
        if tier == ColorTier::Color8 {
            resolved.border = Color::Cyan;
            resolved.toast_info = Color::Green;
            resolved.toast_warning = Color::Yellow;
            resolved.toast_error = Color::Red;
            resolved.accent = Style::default().fg(Color::Cyan);
        } else if tier == ColorTier::Color16 {
            resolved.toast_info = Color::LightGreen;
            resolved.toast_warning = Color::LightYellow;
            resolved.toast_error = Color::LightRed;
        }

        // Indicator symbol
        if let Some(ind) = &theme.indicator {
            let resolved_sym = bashmenu_core::resolve_glyph(ind, use_nerd_fonts);
            if !resolved_sym.is_empty() {
                resolved.indicator_symbol = if resolved_sym.ends_with(' ') {
                    resolved_sym
                } else {
                    format!("{} ", resolved_sym)
                };
            }
        }

        // Divider configuration
        if let Some(div) = &theme.divider {
            if let Some(ch) = &div.char {
                let parsed = bashmenu_core::parse_ascii_val(ch);
                if !parsed.is_empty() {
                    resolved.divider_char = parsed;
                }
            }
            if let Some(len) = &div.length {
                resolved.divider_is_screen_width = len.contains("screen_width");
            }
        }

        // Marquee configuration
        if let Some(marq) = &theme.marquee {
            let parsed = bashmenu_core::parse_ascii_val(&marq.char);
            if !parsed.is_empty() {
                resolved.marquee_char = parsed;
            }
            resolved.marquee_gap = marq.gap;
        }

        // Palette resolution respecting terminal color tier
        let palette = match tier {
            ColorTier::Color8 => theme
                .colors_8
                .as_ref()
                .or(theme.colors_16.as_ref())
                .or(theme.colors_256.as_ref()),
            ColorTier::Color16 => theme
                .colors_16
                .as_ref()
                .or(theme.colors_256.as_ref())
                .or(theme.colors_8.as_ref()),
            ColorTier::Color256 | ColorTier::TrueColor => theme
                .colors_256
                .as_ref()
                .or(theme.colors_16.as_ref())
                .or(theme.colors_8.as_ref()),
        };

        if let Some(p) = palette {
            if let Some(bg) = p
                .get("background")
                .and_then(|v| parse_color_value_with_tier(v, tier))
            {
                resolved.background = bg;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("border"), tier) {
                resolved.border = st.fg.unwrap_or(Color::Cyan);
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("title"), tier) {
                resolved.title = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("text"), tier) {
                resolved.text = st;
                resolved.plugin = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("highlight"), tier) {
                resolved.highlight = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("accent"), tier) {
                resolved.accent = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("footer"), tier) {
                resolved.footer = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("gutter"), tier) {
                resolved.gutter = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("help_text"), tier) {
                resolved.help_text = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("plugin"), tier) {
                resolved.plugin = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("selection"), tier) {
                resolved.selection = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("shortcut_key"), tier) {
                resolved.shortcut_key = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("divider"), tier) {
                resolved.divider = st;
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("window_close_button"), tier) {
                resolved.window_close_button = st.add_modifier(Modifier::BOLD);
            }
            if let Some(st) = parse_style_pair_with_tier(p.get("scrollbar"), tier) {
                resolved.scrollbar = st;
            }
            if let Some(c) = p
                .get("toast_info")
                .and_then(|v| parse_color_value_with_tier(v, tier))
            {
                if c != Color::Reset {
                    resolved.toast_info = c;
                }
            }
            if let Some(c) = p
                .get("toast_warning")
                .and_then(|v| parse_color_value_with_tier(v, tier))
            {
                if c != Color::Reset {
                    resolved.toast_warning = c;
                }
            }
            if let Some(c) = p
                .get("toast_error")
                .and_then(|v| parse_color_value_with_tier(v, tier))
            {
                if c != Color::Reset {
                    resolved.toast_error = c;
                }
            }
        }

        // Toast configuration
        if let Some(t_cfg) = &theme.toast {
            if let Some(ch) = &t_cfg.char {
                let parsed = bashmenu_core::parse_ascii_val(ch);
                if !parsed.is_empty() {
                    resolved.toast_bar_char = parsed;
                }
            }
            if let Some(timeout) = t_cfg.timeout {
                if timeout > 0.0 {
                    resolved.toast_timeout = timeout;
                }
            }
        }

        resolved.borders = ResolvedWindowBorders::from_theme_borders(theme.window.as_ref());

        resolved
    }

    /// Resolves theme palette from Theme struct using detected environment
    /// color tier.
    pub fn from_theme(theme: &Theme, use_nerd_fonts: bool) -> Self {
        Self::from_theme_with_tier(theme, use_nerd_fonts, ColorTier::detect())
    }

    /// Sets the runtime toast notification timeout (from global settings).
    pub fn with_toast_timeout(mut self, timeout: Option<f64>) -> Self {
        if let Some(t) = timeout {
            if t > 0.0 {
                self.toast_timeout = t;
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_tier_fallback_mapping() {
        // RGB truecolor downsampled to 8-color
        let bright_red = Color::Rgb(255, 30, 30);
        let c8 = downsample_color_to_8(bright_red);
        assert_eq!(c8, Color::Red);

        // LightCyan downsampled to 8-color
        let c8_cyan = downsample_color_to_8(Color::LightCyan);
        assert_eq!(c8_cyan, Color::Cyan);

        // 16-color downsampling
        let c16 = downsample_color_to_16(Color::Rgb(255, 30, 30));
        assert_eq!(c16, Color::LightRed);

        // Reset preserved
        assert_eq!(downsample_color_to_8(Color::Reset), Color::Reset);
        assert_eq!(downsample_color_to_16(Color::Reset), Color::Reset);
    }

    #[test]
    fn test_from_theme_with_8_color_tier() {
        let mut theme = Theme::default();
        let mut c8_map = std::collections::HashMap::new();
        c8_map.insert(
            "title".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::String("COLOR_MAGENTA".to_string()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        theme.colors_8 = Some(c8_map);

        let styles = ResolvedThemeStyles::from_theme_with_tier(&theme, false, ColorTier::Color8);
        assert_eq!(styles.title.fg, Some(Color::Magenta));
        assert_eq!(styles.border, Color::Cyan);
    }
}
