//! Theme color mapping and Ratatui style conversion.

use crate::borders::ResolvedWindowBorders;
use crate::wallpaper::{color_to_rgb, rgb_to_ansi16_name, rgb_to_ansi8_name, rgb_to_xterm256};
use bashmenu_core::{activity_override_tokens, Theme};
use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

/// Logical UI activity or program context for per-activity theme overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeActivity {
    /// Primary `bashmenu` interactive menu screen (base theme palette).
    #[default]
    MainMenu,
    /// Built-in `bashedit` text editor.
    BashEdit,
    /// Modal dialog overlays across all BashMenu apps.
    Modals,
    /// Visual `menuedit` menu tree editor.
    MenuEdit,
    /// Popup toast notification banners.
    Toasts,
    /// Visual `themeedit` theme and palette editor.
    ThemeEdit,
    /// Custom keybinding and `shortcutedit` shortcuts screen.
    ShortcutEdit,
}

impl ThemeActivity {
    /// Canonical YAML sub-map key for this activity inside a tier palette.
    pub fn yaml_key(self) -> &'static str {
        match self {
            Self::MainMenu => "bashmenu",
            Self::BashEdit => "bashedit",
            Self::Modals => "modals",
            Self::MenuEdit => "menuedit",
            Self::Toasts => "toasts",
            Self::ThemeEdit => "themeedit",
            Self::ShortcutEdit => "shortcutedit",
        }
    }

    /// Legacy YAML sub-map aliases checked as read fallbacks when migrating themes.
    pub fn legacy_yaml_keys(self) -> &'static [&'static str] {
        match self {
            Self::BashEdit => &["editor"],
            Self::MenuEdit => &["menu_editor"],
            _ => &[],
        }
    }

    /// Returns all six override activity variants excluding `MainMenu`.
    pub fn all_override_activities() -> &'static [ThemeActivity] {
        &[
            Self::BashEdit,
            Self::Modals,
            Self::MenuEdit,
            Self::Toasts,
            Self::ThemeEdit,
            Self::ShortcutEdit,
        ]
    }

    /// Returns the supported color token names for this activity.
    pub fn supported_tokens(self) -> &'static [&'static str] {
        match self {
            Self::MainMenu => &[
                "background",
                "title",
                "text",
                "highlight",
                "highlight_text",
                "border",
                "divider",
                "accent",
                "selection",
                "selected_tab",
                "selected_tab_text",
                "unselected_tab",
                "line_numbers",
                "whitespace",
                "shortcut_key",
                "key_color",
                "help_text",
                "plugin",
                "gutter",
                "footer",
                "window_close_button",
                "scrollbar",
                "shadow",
                "modal_yes_button",
                "modal_no_button",
                "modal_cancel_button",
                "input_box",
                "input_box_text",
                "input_box_unfocused",
                "input_box_unfocused_text",
                "toast_info",
                "toast_warning",
                "toast_error",
            ],
            _ => activity_override_tokens(self.yaml_key()),
        }
    }
}

/// Returns `true` if a YAML color token or `[fg, bg]` pair represents `-1` inheritance.
pub fn is_inherited_color_value(val: &serde_json::Value) -> bool {
    match val {
        serde_json::Value::Null => true,
        serde_json::Value::Number(n) => n.as_i64().is_some_and(|i| i < 0),
        serde_json::Value::String(s) => s.trim().parse::<i64>().is_ok_and(|i| i < 0),
        serde_json::Value::Array(seq) => {
            !seq.is_empty() && seq.iter().all(is_inherited_color_value)
        }
        _ => false,
    }
}

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
        // 1. Explicit user override takes highest precedence
        if let Ok(tier) = std::env::var("BASHMENU_COLOR_TIER") {
            match tier.trim().to_ascii_lowercase().as_str() {
                "8" | "color8" | "basic" => return Self::Color8,
                "16" | "color16" => return Self::Color16,
                "256" | "color256" => return Self::Color256,
                "truecolor" | "24bit" | "rgb" => return Self::TrueColor,
                _ => {}
            }
        }

        // 2. Linux VT consoles lack TrueColor; check linux/console before COLORTERM
        // so inherited COLORTERM variables do not override basic Linux console.
        if let Ok(term) = std::env::var("TERM") {
            let t = term.trim().to_ascii_lowercase();
            if t == "linux" || t == "console" {
                return Self::Color8;
            }
            if t == "linux-16color" {
                return Self::Color16;
            }
        }

        // 3. Check COLORTERM for TrueColor support in modern terminal emulators
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let ct = colorterm.trim().to_ascii_lowercase();
            if ct == "truecolor" || ct == "24bit" {
                return Self::TrueColor;
            }
        }

        // 4. Inspect remaining TERM capabilities
        if let Ok(term) = std::env::var("TERM") {
            let t = term.trim().to_ascii_lowercase();
            if t == "vt100" || t == "vt102" || t == "dumb" {
                return Self::Color8;
            }
            if t.contains("16color") {
                return Self::Color16;
            }
            if t.contains("kitty")
                || t.contains("alacritty")
                || t.contains("ghostty")
                || t.contains("foot")
                || t.contains("wezterm")
            {
                return Self::TrueColor;
            }
            if t.contains("256color") {
                return Self::Color256;
            }
        }

        Self::TrueColor
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

/// Downsamples a Ratatui `Color` according to the specified terminal `ColorTier`.
pub fn downsample_color_to_tier(color: Color, tier: ColorTier) -> Color {
    match tier {
        ColorTier::Color8 => downsample_color_to_8(color),
        ColorTier::Color16 => downsample_color_to_16(color),
        ColorTier::Color256 => {
            if color == Color::Reset {
                return Color::Reset;
            }
            match color {
                Color::Indexed(_) => color,
                Color::Rgb(r, g, b) => Color::Indexed(rgb_to_xterm256(r, g, b)),
                other => other,
            }
        }
        ColorTier::TrueColor => color,
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
    /// Key assignment badge color in shortcuts lists
    pub key_color: Style,
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
    /// Default menu item icon glyph definition (e.g. `"{nf:>:#f105}"`)
    pub default_icon: String,
    /// Resolved box-drawing glyphs for window borders
    pub borders: ResolvedWindowBorders,
    /// Visual layout style for toast notification popups
    pub toast_style: bashmenu_core::ToastStyle,
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
    /// Modal dialog drop-shadow style
    pub shadow: Style,
    /// Affirmative / Yes button style in modal dialogs
    pub modal_yes_button: Style,
    /// Negative / No button style in modal dialogs
    pub modal_no_button: Style,
    /// Cancel button style in modal dialogs
    pub modal_cancel_button: Style,
    /// Focused text input box style (fg = input_box_text, bg = input_box)
    pub input_box: Style,
    /// Unfocused text input box style (fg = input_box_unfocused_text, bg = input_box_unfocused)
    pub input_box_unfocused: Style,
    /// Active / selected tab style in tabbed interfaces
    pub selected_tab: Style,
    /// Inactive / unselected tab style in tabbed interfaces
    pub unselected_tab: Style,
    /// Line number gutter style in text editors
    pub line_numbers: Style,
    /// Visible whitespace glyph style in text editors
    pub whitespace: Style,
    /// Active terminal color capability tier
    pub tier: ColorTier,
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
            key_color: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            divider: Style::default().fg(Color::Cyan),
            divider_char: "─".to_string(),
            divider_is_screen_width: false,
            marquee_char: "•".to_string(),
            marquee_gap: 4,
            window_close_button: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            indicator_symbol: "► ".to_string(),
            default_icon: bashmenu_core::DEFAULT_ITEM_ICON.to_string(),
            borders: ResolvedWindowBorders::default(),
            toast_style: bashmenu_core::ToastStyle::default(),
            toast_bar_char: "▌".to_string(),
            toast_timeout: 2.0,
            toast_info: Color::Rgb(78, 191, 113),
            toast_warning: Color::Rgb(255, 184, 108),
            toast_error: Color::Rgb(255, 85, 85),
            scrollbar: Style::default().fg(Color::Cyan),
            shadow: Style::default().fg(Color::DarkGray).bg(Color::Black),
            modal_yes_button: Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
            modal_no_button: Style::default().fg(Color::LightRed),
            modal_cancel_button: Style::default().fg(Color::DarkGray),
            input_box: Style::default().fg(Color::White).bg(Color::Indexed(236)),
            input_box_unfocused: Style::default().fg(Color::Gray).bg(Color::Indexed(234)),
            selected_tab: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            unselected_tab: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::DIM),
            line_numbers: Style::default().fg(Color::DarkGray),
            whitespace: Style::default().fg(Color::DarkGray),
            tier: ColorTier::TrueColor,
        }
    }
}

/// Parses a color identifier (number, hex string, or named constant) into a
/// Ratatui Color respecting the given terminal color tier.
pub fn parse_color_value_with_tier(val: &serde_json::Value, tier: ColorTier) -> Option<Color> {
    let raw_color = match val {
        serde_json::Value::Array(seq) => seq
            .first()
            .and_then(|v| parse_color_value_with_tier(v, tier)),
        serde_json::Value::Number(n) => {
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
        serde_json::Value::String(s) => {
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
            if let Some(hex_rgb) = s_trim
                .strip_prefix('#')
                .filter(|h| h.len() == 6)
                .and_then(|h| u32::from_str_radix(h, 16).ok())
            {
                let r = ((hex_rgb >> 16) & 0xFF) as u8;
                let g = ((hex_rgb >> 8) & 0xFF) as u8;
                let b = (hex_rgb & 0xFF) as u8;
                Some(Color::Rgb(r, g, b))
            } else {
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
        }
        _ => None,
    }?;

    Some(downsample_color_to_tier(raw_color, tier))
}

/// Parses a color identifier (number, hex string, or named constant) into a
/// Ratatui Color using the detected environment color tier.
pub fn parse_color_value(val: &serde_json::Value) -> Option<Color> {
    parse_color_value_with_tier(val, ColorTier::detect())
}

/// Resolves a pair [fg, bg] into a Ratatui Style respecting the color tier.
pub fn parse_style_pair_with_tier(
    val: Option<&serde_json::Value>,
    tier: ColorTier,
) -> Option<Style> {
    let v = val?;
    match v {
        serde_json::Value::Array(seq) => {
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
pub fn parse_style_pair(val: Option<&serde_json::Value>) -> Option<Style> {
    parse_style_pair_with_tier(val, ColorTier::detect())
}

/// Finds the override sub-map matching `activity`'s canonical key or legacy alias.
fn find_activity_map(
    p: &HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    p.get(activity.yaml_key())
        .and_then(|v| v.as_object())
        .or_else(|| {
            activity
                .legacy_yaml_keys()
                .iter()
                .find_map(|k| p.get(*k).and_then(|v| v.as_object()))
        })
}

/// Merges a resolved style pair onto `base`, keeping `base.fg` when `st.fg` is `-1`.
fn merge_style_pair(base: Style, st: Style, extra_mod: Option<Modifier>) -> Style {
    let fg = st.fg.or(base.fg);
    let mut out = Style::default();
    if let Some(c) = fg {
        out = out.fg(c);
    }
    if let Some(c) = st.bg {
        out = out.bg(c);
    }
    if let Some(m) = extra_mod {
        out = out.add_modifier(m);
    }
    out
}

impl ResolvedThemeStyles {
    /// Applies palette token entries from `p` onto `self`.
    ///
    /// When `is_override` is `true`, `-1` / `[-1, -1]` values leave the inherited base
    /// styles unchanged and automatic fallback propagation (`text` -> `plugin`,
    /// `gutter` -> `whitespace`) is skipped.
    fn apply_palette_map(
        &mut self,
        p: &HashMap<String, serde_json::Value>,
        tier: ColorTier,
        is_override: bool,
    ) {
        if let Some(bg) = p
            .get("background")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            if bg != Color::Reset {
                self.background = bg;
            }
        }
        if let Some(st) = p
            .get("border")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            if let Some(fg) = st.fg {
                self.border = fg;
            }
        }
        if let Some(st) = p
            .get("title")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.title = merge_style_pair(self.title, st, Some(Modifier::BOLD));
        }
        if let Some(st) = p
            .get("text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.text, st, None);
            self.text = merged;
            if !is_override && !p.contains_key("plugin") {
                self.plugin = merged;
            }
            if !is_override && !p.contains_key("unselected_tab") {
                self.unselected_tab = merged.add_modifier(Modifier::DIM);
            }
        }
        if let Some(st) = p
            .get("highlight")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.highlight = merge_style_pair(self.highlight, st, Some(Modifier::BOLD));
        }
        if let Some(st) = p
            .get("highlight_text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            if let Some(fg) = st.fg {
                self.highlight = self.highlight.fg(fg);
            }
        }
        if let Some(st) = p
            .get("accent")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.accent, st, None);
            self.accent = merged;
            if !is_override && !p.contains_key("modal_yes_button") {
                self.modal_yes_button = merged.add_modifier(Modifier::BOLD);
            }
        }
        if let Some(st) = p
            .get("footer")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.footer = merge_style_pair(self.footer, st, None);
        }
        if let Some(st) = p
            .get("gutter")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.gutter, st, None);
            self.gutter = merged;
            if !is_override && !p.contains_key("whitespace") {
                self.whitespace = merged;
            }
            if !is_override && !p.contains_key("line_numbers") {
                self.line_numbers = merged;
            }
        }
        if let Some(st) = p
            .get("line_numbers")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.line_numbers = merge_style_pair(self.line_numbers, st, None);
        }
        if let Some(st) = p
            .get("whitespace")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.whitespace = merge_style_pair(self.whitespace, st, None);
        }
        if let Some(st) = p
            .get("help_text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.help_text = merge_style_pair(self.help_text, st, None);
        }
        if let Some(st) = p
            .get("plugin")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.plugin = merge_style_pair(self.plugin, st, None);
        }
        if let Some(st) = p
            .get("selection")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.selection, st, None);
            self.selection = merged;
            if !is_override && !p.contains_key("selected_tab") {
                self.selected_tab = merged.add_modifier(Modifier::BOLD);
            }
        }
        if let Some(st) = p
            .get("selected_tab")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.selected_tab = merge_style_pair(self.selected_tab, st, Some(Modifier::BOLD));
        }
        if let Some(fg) = p
            .get("selected_tab_text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            self.selected_tab = self.selected_tab.fg(fg);
        }
        if let Some(st) = p
            .get("input_box")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.input_box, st, None);
            self.input_box = merged;
            if !is_override && !p.contains_key("input_box_unfocused") {
                self.input_box_unfocused = merged.add_modifier(Modifier::DIM);
            }
        }
        if let Some(fg) = p
            .get("input_box_text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            self.input_box = self.input_box.fg(fg);
            if !is_override && !p.contains_key("input_box_unfocused_text") {
                self.input_box_unfocused =
                    self.input_box_unfocused.fg(fg).add_modifier(Modifier::DIM);
            }
        }
        if let Some(st) = p
            .get("input_box_unfocused")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.input_box_unfocused = merge_style_pair(self.input_box_unfocused, st, None);
        }
        if let Some(fg) = p
            .get("input_box_unfocused_text")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            self.input_box_unfocused = self.input_box_unfocused.fg(fg);
        }
        if let Some(st) = p
            .get("unselected_tab")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.unselected_tab = merge_style_pair(self.unselected_tab, st, None);
        }
        if let Some(st) = p
            .get("shortcut_key")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let merged = merge_style_pair(self.shortcut_key, st, Some(Modifier::BOLD));
            self.shortcut_key = merged;
            if !is_override && !p.contains_key("key_color") {
                self.key_color = merged;
            }
        }
        if let Some(st) = p
            .get("key_color")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.key_color = merge_style_pair(self.key_color, st, Some(Modifier::BOLD));
        }
        if let Some(st) = p
            .get("divider")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.divider = merge_style_pair(self.divider, st, None);
        }
        if let Some(st) = p
            .get("window_close_button")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.window_close_button =
                merge_style_pair(self.window_close_button, st, Some(Modifier::BOLD));
        }
        if let Some(st) = p
            .get("scrollbar")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.scrollbar = merge_style_pair(self.scrollbar, st, None);
        }
        if let Some(st) = p
            .get("shadow")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            let fg_color = st
                .fg
                .unwrap_or_else(|| self.shadow.fg.unwrap_or(Color::DarkGray));
            let bg_color = st.bg.unwrap_or(fg_color);
            self.shadow = Style::default().fg(fg_color).bg(bg_color);
        }
        if let Some(st) = p
            .get("modal_yes_button")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.modal_yes_button =
                merge_style_pair(self.modal_yes_button, st, Some(Modifier::BOLD));
        }
        if let Some(st) = p
            .get("modal_no_button")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.modal_no_button = merge_style_pair(self.modal_no_button, st, None);
        }
        if let Some(st) = p
            .get("modal_cancel_button")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_style_pair_with_tier(Some(v), tier))
        {
            self.modal_cancel_button = merge_style_pair(self.modal_cancel_button, st, None);
        }
        if let Some(c) = p
            .get("toast_info")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            if c != Color::Reset {
                self.toast_info = c;
            }
        }
        if let Some(c) = p
            .get("toast_warning")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            if c != Color::Reset {
                self.toast_warning = c;
            }
        }
        if let Some(c) = p
            .get("toast_error")
            .filter(|v| !is_inherited_color_value(v))
            .and_then(|v| parse_color_value_with_tier(v, tier))
        {
            if c != Color::Reset {
                self.toast_error = c;
            }
        }
    }

    /// Resolves theme palette from Theme struct using a specific color tier.
    pub fn from_theme_with_tier(theme: &Theme, use_nerd_fonts: bool, tier: ColorTier) -> Self {
        let mut resolved = Self {
            tier,
            ..Default::default()
        };
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

        resolved.default_icon = theme.resolved_default_icon().to_string();

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
            resolved.apply_palette_map(p, tier, false);
        }

        // Toast configuration
        if let Some(t_cfg) = &theme.toast {
            if let Some(style_str) = &t_cfg.style {
                resolved.toast_style = bashmenu_core::ToastStyle::from_str_name(style_str);
            }
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

    /// Resolves theme palette for a specific UI activity using a explicit color tier.
    pub fn from_theme_with_tier_for_activity(
        theme: &Theme,
        use_nerd_fonts: bool,
        tier: ColorTier,
        activity: ThemeActivity,
    ) -> Self {
        let mut resolved = Self::from_theme_with_tier(theme, use_nerd_fonts, tier);
        if activity == ThemeActivity::MainMenu {
            return resolved;
        }

        let palette_candidates: [Option<&HashMap<String, serde_json::Value>>; 3] = match tier {
            ColorTier::Color8 => [
                theme.colors_8.as_ref(),
                theme.colors_16.as_ref(),
                theme.colors_256.as_ref(),
            ],
            ColorTier::Color16 => [
                theme.colors_16.as_ref(),
                theme.colors_256.as_ref(),
                theme.colors_8.as_ref(),
            ],
            ColorTier::Color256 | ColorTier::TrueColor => [
                theme.colors_256.as_ref(),
                theme.colors_16.as_ref(),
                theme.colors_8.as_ref(),
            ],
        };

        if let Some(sub_mapping) = palette_candidates
            .into_iter()
            .flatten()
            .find_map(|p| find_activity_map(p, activity))
        {
            let mut override_map = HashMap::new();
            for (k, v) in sub_mapping {
                override_map.insert(k.clone(), v.clone());
            }
            resolved.apply_palette_map(&override_map, tier, true);
        }

        resolved
    }

    /// Resolves theme palette from Theme struct using detected environment
    /// color tier.
    pub fn from_theme(theme: &Theme, use_nerd_fonts: bool) -> Self {
        Self::from_theme_with_tier(theme, use_nerd_fonts, ColorTier::detect())
    }

    /// Resolves theme palette for a specific UI activity using the detected color tier.
    pub fn from_theme_for_activity(
        theme: &Theme,
        use_nerd_fonts: bool,
        activity: ThemeActivity,
    ) -> Self {
        Self::from_theme_with_tier_for_activity(
            theme,
            use_nerd_fonts,
            ColorTier::detect(),
            activity,
        )
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
            serde_json::Value::Array(vec![
                serde_json::Value::String("COLOR_MAGENTA".to_string()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        theme.colors_8 = Some(c8_map);

        let styles = ResolvedThemeStyles::from_theme_with_tier(&theme, false, ColorTier::Color8);
        assert_eq!(styles.title.fg, Some(Color::Magenta));
        assert_eq!(styles.border, Color::Cyan);
    }

    #[test]
    fn test_resolved_theme_styles_default_icon() {
        let styles = ResolvedThemeStyles::default();
        assert_eq!(styles.default_icon, bashmenu_core::DEFAULT_ITEM_ICON);

        let theme = Theme {
            default_icon: Some("{nf:*:#f005}".to_string()),
            ..Default::default()
        };
        let resolved = ResolvedThemeStyles::from_theme(&theme, true);
        assert_eq!(resolved.default_icon, "{nf:*:#f005}");
    }
}
