//! Library module for ThemeEdit core state, actions, and theme manipulation.

pub mod preview;
pub use preview::{
    extract_preview_cell_color, extract_preview_color_at, format_color_code, render_preview_pane,
};

use anyhow::Result;
use bashmenu_core::{
    load_themes, save_themes, DividerConfig, MarqueeConfig, Theme, ThemeCollection,
    WallpaperSetting, WindowBorders,
};
use bashmenu_tui::{ResolvedThemeStyles, TextInput};
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Category grouping for theme attributes in the editor sidebar tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenCategory {
    /// Global theme metadata, wallpaper, and marquee settings
    Theme,
    /// Color token palette mappings
    Colors,
    /// Box drawing and window frame border glyphs
    Borders,
}

impl TokenCategory {
    /// Array containing all three category variants.
    pub const ALL: [TokenCategory; 3] = [
        TokenCategory::Theme,
        TokenCategory::Colors,
        TokenCategory::Borders,
    ];

    /// Display header label for this category.
    pub fn label(self) -> &'static str {
        match self {
            TokenCategory::Theme => "Theme:",
            TokenCategory::Colors => "Colors:",
            TokenCategory::Borders => "Borders:",
        }
    }
}

/// Represents a row in the theme editor's sidebar hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeRow {
    /// Category header row (expandable/collapsible)
    Category(TokenCategory),
    /// Individual token item row referencing an index in `tokens`
    Token(usize),
}

/// The specific theme token or attribute being edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// Color palette token name (e.g., "background", "text", "highlight")
    Color(String),
    /// Border glyph field name (e.g., "border_horizontal", "border_top_left")
    Border(String),
    /// Active selection indicator symbol
    Indicator,
    /// Section divider rule character
    DividerChar,
    /// Section divider rule length expression
    DividerLength,
    /// Horizontal marquee ticker scroll delimiter character
    MarqueeChar,
    /// Marquee scroll repeat gap in spaces
    MarqueeGap,
    /// Toast notification vertical indicator glyph
    ToastChar,
    /// Background wallpaper image path
    WallpaperImage,
    /// Background wallpaper opacity factor
    WallpaperOpacity,
    /// Background wallpaper scaling algorithm
    WallpaperScaling,
    /// Background wallpaper rendering mode
    WallpaperMode,
    /// Background wallpaper color clustering distance
    WallpaperClustering,
    /// Background wallpaper palette color quantization step
    WallpaperQuantization,
}

/// An individual editable theme token entry in the sidebar.
#[derive(Debug, Clone)]
pub struct TokenItem {
    /// Display label in the tree list
    pub label: String,
    /// Classification kind of the token
    pub kind: TokenKind,
}

impl TokenItem {
    /// Returns the category under which this token is grouped.
    pub fn category(&self) -> TokenCategory {
        match &self.kind {
            TokenKind::Color(_) => TokenCategory::Colors,
            TokenKind::Border(_) => TokenCategory::Borders,
            TokenKind::WallpaperImage
            | TokenKind::WallpaperOpacity
            | TokenKind::WallpaperScaling
            | TokenKind::WallpaperMode
            | TokenKind::WallpaperClustering
            | TokenKind::WallpaperQuantization
            | TokenKind::Indicator
            | TokenKind::DividerChar
            | TokenKind::DividerLength
            | TokenKind::MarqueeChar
            | TokenKind::MarqueeGap
            | TokenKind::ToastChar => TokenCategory::Theme,
        }
    }
}

/// Active preview mode displaying different BashMenu screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    /// Main menu interactive preview
    #[default]
    Menu,
    /// Text editor (`bashedit`) preview
    TextEditor,
    /// Modal dialogs and alert popups preview
    Modals,
    /// Visual menu hierarchy editor (`menuedit`) preview
    MenuEditor,
    /// Toast notification popups preview
    Toast,
}

impl PreviewMode {
    /// Cycles to the next preview mode screen.
    pub fn next(self) -> Self {
        match self {
            PreviewMode::Menu => PreviewMode::TextEditor,
            PreviewMode::TextEditor => PreviewMode::Modals,
            PreviewMode::Modals => PreviewMode::MenuEditor,
            PreviewMode::MenuEditor => PreviewMode::Toast,
            PreviewMode::Toast => PreviewMode::Menu,
        }
    }

    /// Title string of the current preview screen mode.
    pub fn title(self) -> &'static str {
        match self {
            PreviewMode::Menu => "Main Menu",
            PreviewMode::TextEditor => "Text Editor",
            PreviewMode::Modals => "Popup Windows / Modals",
            PreviewMode::MenuEditor => "Visual Menu Editor",
            PreviewMode::Toast => "Toast Notifications",
        }
    }
}

/// Interactive state for the `themeedit` TUI.
pub struct ThemeEditorState {
    /// All loaded themes mapped by theme name
    pub themes: ThemeCollection,
    /// File path to `bashmenu.themes`
    pub themes_path: PathBuf,
    /// Name of currently active theme
    pub current_name: String,
    /// Active theme definition schema
    pub current_theme: Theme,
    /// Dirty flag indicating unsaved modifications
    pub is_modified: bool,
    /// List of all discoverable token items
    pub tokens: Vec<TokenItem>,
    /// Currently highlighted token index
    pub selected_idx: usize,
    /// Currently highlighted visual tree row index
    pub selected_row: usize,
    /// Set of expanded sidebar categories
    pub expanded_categories: HashSet<TokenCategory>,
    /// Vertical scroll offset in lines for sidebar tree
    pub scroll_offset: usize,
    /// Transient status notification message
    pub status_message: String,
    /// Active preview pane mode
    pub preview_mode: PreviewMode,
    /// Text input buffer for theme name editing
    pub name_input: TextInput,
}

/// Converts a Ratatui Color to an ANSI 256 color index (0..=255)
pub fn color_to_u8(color: ratatui::style::Color) -> u8 {
    match color {
        ratatui::style::Color::Reset => 0,
        ratatui::style::Color::Black => 0,
        ratatui::style::Color::Red => 1,
        ratatui::style::Color::Green => 2,
        ratatui::style::Color::Yellow => 3,
        ratatui::style::Color::Blue => 4,
        ratatui::style::Color::Magenta => 5,
        ratatui::style::Color::Cyan => 6,
        ratatui::style::Color::Gray => 7,
        ratatui::style::Color::DarkGray => 8,
        ratatui::style::Color::LightRed => 9,
        ratatui::style::Color::LightGreen => 10,
        ratatui::style::Color::LightYellow => 11,
        ratatui::style::Color::LightBlue => 12,
        ratatui::style::Color::LightMagenta => 13,
        ratatui::style::Color::LightCyan => 14,
        ratatui::style::Color::White => 15,
        ratatui::style::Color::Indexed(code) => code,
        ratatui::style::Color::Rgb(r, g, b) => {
            if r == g && g == b {
                if r < 8 {
                    16
                } else if r > 248 {
                    231
                } else {
                    ((r as u16 - 8) * 24 / 247) as u8 + 232
                }
            } else {
                let qr = (r as u16 * 5 / 255) as u8;
                let qg = (g as u16 * 5 / 255) as u8;
                let qb = (b as u16 * 5 / 255) as u8;
                16 + 36 * qr + 6 * qg + qb
            }
        }
    }
}

impl ThemeEditorState {
    /// Initializes theme editor state from `bashmenu.themes` targeting the selected or default theme.
    pub fn new(themes_path: PathBuf, target_theme: Option<String>) -> Self {
        let themes = if themes_path.exists() {
            load_themes(&themes_path).unwrap_or_default()
        } else {
            ThemeCollection::default()
        };

        let initial_name = target_theme
            .filter(|n| !n.trim().is_empty())
            .or_else(|| {
                // Try reading active theme from bashmenu.yml if available
                if let Ok(home) = std::env::var("HOME") {
                    let cfg_path = PathBuf::from(home).join(".config/bashmenu/bashmenu.yml");
                    if let Ok(cfg) = bashmenu_core::load_config(&cfg_path) {
                        if themes.contains_key(&cfg.theme) {
                            return Some(cfg.theme);
                        }
                    }
                }
                let cur_cfg = PathBuf::from("bashmenu.yml");
                if let Ok(cfg) = bashmenu_core::load_config(&cur_cfg) {
                    if themes.contains_key(&cfg.theme) {
                        return Some(cfg.theme);
                    }
                }
                if themes.contains_key("dracula") {
                    Some("dracula".to_string())
                } else {
                    let mut sorted: Vec<_> = themes.keys().cloned().collect();
                    sorted.sort_by(|a, b| {
                        a.to_ascii_lowercase()
                            .cmp(&b.to_ascii_lowercase())
                            .then_with(|| a.cmp(b))
                    });
                    sorted.into_iter().next()
                }
            })
            .unwrap_or_else(|| "dracula".to_string());

        let current_theme = themes
            .get(&initial_name)
            .cloned()
            .unwrap_or_else(Self::create_default_theme);

        let tokens = vec![
            // Background & Wallpaper
            TokenItem {
                label: "Background Color".to_string(),
                kind: TokenKind::Color("background".to_string()),
            },
            TokenItem {
                label: "Wallpaper Image".to_string(),
                kind: TokenKind::WallpaperImage,
            },
            TokenItem {
                label: "Wallpaper Opacity".to_string(),
                kind: TokenKind::WallpaperOpacity,
            },
            TokenItem {
                label: "Wallpaper Scaling".to_string(),
                kind: TokenKind::WallpaperScaling,
            },
            TokenItem {
                label: "Wallpaper Mode".to_string(),
                kind: TokenKind::WallpaperMode,
            },
            TokenItem {
                label: "Wallpaper Clustering".to_string(),
                kind: TokenKind::WallpaperClustering,
            },
            TokenItem {
                label: "Wallpaper Quantization".to_string(),
                kind: TokenKind::WallpaperQuantization,
            },
            // Colors
            TokenItem {
                label: "Text Color".to_string(),
                kind: TokenKind::Color("text".to_string()),
            },
            TokenItem {
                label: "Highlight Color".to_string(),
                kind: TokenKind::Color("highlight".to_string()),
            },
            TokenItem {
                label: "Border Color".to_string(),
                kind: TokenKind::Color("border".to_string()),
            },
            TokenItem {
                label: "Divider Color".to_string(),
                kind: TokenKind::Color("divider".to_string()),
            },
            TokenItem {
                label: "Accent Color".to_string(),
                kind: TokenKind::Color("accent".to_string()),
            },
            TokenItem {
                label: "Shortcut Gutter (1-9, a-z)".to_string(),
                kind: TokenKind::Color("shortcut_key".to_string()),
            },
            TokenItem {
                label: "Help Text Color".to_string(),
                kind: TokenKind::Color("help_text".to_string()),
            },
            TokenItem {
                label: "Plugin Text Color".to_string(),
                kind: TokenKind::Color("plugin".to_string()),
            },
            TokenItem {
                label: "Gutter Color".to_string(),
                kind: TokenKind::Color("gutter".to_string()),
            },
            TokenItem {
                label: "Scrollbar Color".to_string(),
                kind: TokenKind::Color("scrollbar".to_string()),
            },
            // Indicator
            TokenItem {
                label: "Cursor Indicator".to_string(),
                kind: TokenKind::Indicator,
            },
            // Divider
            TokenItem {
                label: "Divider Character".to_string(),
                kind: TokenKind::DividerChar,
            },
            TokenItem {
                label: "Divider Width Mode".to_string(),
                kind: TokenKind::DividerLength,
            },
            // Marquee
            TokenItem {
                label: "Marquee Character".to_string(),
                kind: TokenKind::MarqueeChar,
            },
            TokenItem {
                label: "Marquee Gap Count".to_string(),
                kind: TokenKind::MarqueeGap,
            },
            // Borders
            TokenItem {
                label: "Horizontal Border".to_string(),
                kind: TokenKind::Border("border_horizontal".to_string()),
            },
            TokenItem {
                label: "Vertical Border".to_string(),
                kind: TokenKind::Border("border_vertical".to_string()),
            },
            TokenItem {
                label: "Top Left Corner".to_string(),
                kind: TokenKind::Border("border_top_left".to_string()),
            },
            TokenItem {
                label: "Top Right Corner".to_string(),
                kind: TokenKind::Border("border_top_right".to_string()),
            },
            TokenItem {
                label: "Bottom Left Corner".to_string(),
                kind: TokenKind::Border("border_bottom_left".to_string()),
            },
            TokenItem {
                label: "Bottom Right Corner".to_string(),
                kind: TokenKind::Border("border_bottom_right".to_string()),
            },
            TokenItem {
                label: "Left T-Connector".to_string(),
                kind: TokenKind::Border("left_tee".to_string()),
            },
            TokenItem {
                label: "Right T-Connector".to_string(),
                kind: TokenKind::Border("right_tee".to_string()),
            },
            TokenItem {
                label: "Top T-Connector".to_string(),
                kind: TokenKind::Border("top_tee".to_string()),
            },
            TokenItem {
                label: "Bottom T-Connector".to_string(),
                kind: TokenKind::Border("bottom_tee".to_string()),
            },
            TokenItem {
                label: "Gutter Border".to_string(),
                kind: TokenKind::Border("gutter_border".to_string()),
            },
            TokenItem {
                label: "Gutter Separator".to_string(),
                kind: TokenKind::Border("gutter_separator".to_string()),
            },
            TokenItem {
                label: "Gutter Left Tee".to_string(),
                kind: TokenKind::Border("gutter_left_tee".to_string()),
            },
            TokenItem {
                label: "Gutter Right Tee".to_string(),
                kind: TokenKind::Border("gutter_right_tee".to_string()),
            },
            TokenItem {
                label: "Gutter Top Tee".to_string(),
                kind: TokenKind::Border("gutter_top_tee".to_string()),
            },
            TokenItem {
                label: "Gutter Bottom Tee".to_string(),
                kind: TokenKind::Border("gutter_bottom_tee".to_string()),
            },
            // Toast Notifications
            TokenItem {
                label: "Toast Left Bar".to_string(),
                kind: TokenKind::ToastChar,
            },
            TokenItem {
                label: "Toast Info Accent".to_string(),
                kind: TokenKind::Color("toast_info".to_string()),
            },
            TokenItem {
                label: "Toast Warning Accent".to_string(),
                kind: TokenKind::Color("toast_warning".to_string()),
            },
            TokenItem {
                label: "Toast Error Accent".to_string(),
                kind: TokenKind::Color("toast_error".to_string()),
            },
        ];

        let name_input = TextInput::new(&initial_name);

        Self {
            themes,
            themes_path,
            current_name: initial_name,
            current_theme,
            is_modified: false,
            tokens,
            selected_idx: 0,
            selected_row: 0,
            expanded_categories: HashSet::new(),
            scroll_offset: 0,
            status_message: "Ready".to_string(),
            preview_mode: PreviewMode::default(),
            name_input,
        }
    }

    /// Creates a base fallback theme with standard borders and color mappings.
    pub fn create_default_theme() -> Theme {
        let mut t = Theme::default();
        let borders = WindowBorders {
            border_horizontal: Some("─".to_string()),
            border_vertical: Some("│".to_string()),
            border_top_left: Some("┌".to_string()),
            border_top_right: Some("┐".to_string()),
            border_bottom_left: Some("└".to_string()),
            border_bottom_right: Some("┘".to_string()),
            left_tee: Some("├".to_string()),
            right_tee: Some("┤".to_string()),
            top_tee: Some("┬".to_string()),
            bottom_tee: Some("┴".to_string()),
            ..Default::default()
        };
        t.window = Some(borders);
        t.indicator = Some("> ".to_string());

        let mut colors = HashMap::new();
        colors.insert(
            "text".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(255.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "highlight".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(255.into()),
                serde_yml::Value::Number(201.into()),
            ]),
        );
        colors.insert(
            "border".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(61.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "divider".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(61.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "accent".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(117.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "shortcut_key".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(201.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "gutter".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(239.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "plugin".to_string(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(212.into()),
                serde_yml::Value::Number((-1).into()),
            ]),
        );
        t.colors_16 = Some(bashmenu_tui::downsample_palette_to_16(&colors));
        t.colors_8 = Some(bashmenu_tui::downsample_palette_to_8(&colors));
        t.colors_256 = Some(colors);
        t
    }

    /// Returns a human-readable display string representing the token's current value.
    pub fn get_token_value_display(&self, token: &TokenItem) -> String {
        match &token.kind {
            TokenKind::Color(key) => {
                if let Some(c_map) = &self.current_theme.colors_256 {
                    if let Some(val) = c_map.get(key) {
                        if let Some(seq) = val.as_sequence() {
                            if let Some(fg) = seq.first().and_then(|v| v.as_i64()) {
                                return format!("Color {}", fg);
                            } else if let Some(s) = seq.first().and_then(|v| v.as_str()) {
                                return s.to_string();
                            }
                        } else if let Some(n) = val.as_i64() {
                            return format!("Color {}", n);
                        } else if let Some(s) = val.as_str() {
                            return s.to_string();
                        }
                    }
                }
                "Default".to_string()
            }
            TokenKind::Border(key) => {
                if let Some(win) = &self.current_theme.window {
                    let s = match key.as_str() {
                        "border_horizontal" => win.border_horizontal.as_deref(),
                        "border_vertical" => win.border_vertical.as_deref(),
                        "border_top_left" => win.border_top_left.as_deref(),
                        "border_top_right" => win.border_top_right.as_deref(),
                        "border_bottom_left" => win.border_bottom_left.as_deref(),
                        "border_bottom_right" => win.border_bottom_right.as_deref(),
                        "left_tee" => win.left_tee.as_deref(),
                        "right_tee" => win.right_tee.as_deref(),
                        "top_tee" => win.top_tee.as_deref(),
                        "bottom_tee" => win.bottom_tee.as_deref(),
                        "gutter_border" => win.gutter_border.as_deref(),
                        "gutter_separator" => win.gutter_separator.as_deref(),
                        "gutter_left_tee" => win.gutter_left_tee.as_deref(),
                        "gutter_right_tee" => win.gutter_right_tee.as_deref(),
                        "gutter_top_tee" => win.gutter_top_tee.as_deref(),
                        "gutter_bottom_tee" => win.gutter_bottom_tee.as_deref(),
                        _ => None,
                    };
                    if let Some(ch) = s {
                        return format!("'{}'", ch);
                    }
                }
                "Default".to_string()
            }
            TokenKind::Indicator => {
                let raw = self.current_theme.indicator.as_deref().unwrap_or("> ");
                let parsed = bashmenu_core::parse_ascii_val(raw);
                if parsed != raw {
                    format!("'{}' ({})", parsed, raw)
                } else {
                    format!("'{}'", raw)
                }
            }
            TokenKind::DividerChar => {
                let raw = self
                    .current_theme
                    .divider
                    .as_ref()
                    .and_then(|d| d.char.as_deref())
                    .unwrap_or("─");
                let parsed = bashmenu_core::parse_ascii_val(raw);
                format!("'{}'", parsed)
            }
            TokenKind::DividerLength => self
                .current_theme
                .divider
                .as_ref()
                .and_then(|d| d.length.as_deref())
                .unwrap_or("{window_width}")
                .to_string(),
            TokenKind::MarqueeChar => {
                let raw = self
                    .current_theme
                    .marquee
                    .as_ref()
                    .map(|m| m.char.as_str())
                    .unwrap_or("•");
                let parsed = bashmenu_core::parse_ascii_val(raw);
                format!("'{}'", parsed)
            }
            TokenKind::MarqueeGap => {
                let gap = self
                    .current_theme
                    .marquee
                    .as_ref()
                    .map(|m| m.gap)
                    .unwrap_or(4);
                format!("{}", gap)
            }
            TokenKind::ToastChar => {
                let c = self
                    .current_theme
                    .toast
                    .as_ref()
                    .and_then(|t| t.char.clone())
                    .unwrap_or_else(|| "▌".to_string());
                let parsed = bashmenu_core::parse_ascii_val(&c);
                format!("'{}'", if parsed.is_empty() { c } else { parsed })
            }
            TokenKind::WallpaperImage => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    if let Some(img) = &wp.image {
                        if !img.is_empty() {
                            let p = std::path::Path::new(img);
                            return p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(img)
                                .to_string();
                        }
                    }
                }
                "None".to_string()
            }
            TokenKind::WallpaperOpacity => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    format!("{:.2}", wp.opacity)
                } else {
                    "0.85".to_string()
                }
            }
            TokenKind::WallpaperScaling => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    wp.scaling.clone()
                } else {
                    "cover".to_string()
                }
            }
            TokenKind::WallpaperMode => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    wp.mode.clone().unwrap_or_else(|| "halfblock".to_string())
                } else {
                    "halfblock".to_string()
                }
            }
            TokenKind::WallpaperClustering => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    format!("{}", wp.clustering)
                } else {
                    "0".to_string()
                }
            }
            TokenKind::WallpaperQuantization => {
                if let Some(wp) = self.current_theme.wallpaper_config() {
                    format!("{}", wp.quantization)
                } else {
                    "1".to_string()
                }
            }
        }
    }

    /// Updates a 256-color palette token entry with the specified color index.
    pub fn set_color_token(&mut self, key: &str, color_idx: u8) {
        self.set_color_token_value(key, Color::Indexed(color_idx));
    }

    /// Updates a color token entry with a specified Ratatui Color (Indexed, RGB, or named).
    pub fn set_color_token_value(&mut self, key: &str, color: Color) {
        if self.current_theme.colors_256.is_none() {
            self.current_theme.colors_256 = Some(HashMap::new());
        }
        if let Some(c_map) = &mut self.current_theme.colors_256 {
            let val = match color {
                Color::Indexed(code) => serde_yml::Value::Number((code as i64).into()),
                Color::Rgb(r, g, b) => serde_yml::Value::String(format!("#{r:02x}{g:02x}{b:02x}")),
                _ => {
                    let (r, g, b) = bashmenu_tui::color_to_rgb(color);
                    serde_yml::Value::String(format!("#{r:02x}{g:02x}{b:02x}"))
                }
            };
            if key == "background" {
                c_map.insert(key.to_string(), val.clone());
            } else {
                c_map.insert(
                    key.to_string(),
                    serde_yml::Value::Sequence(vec![
                        val.clone(),
                        serde_yml::Value::Number((-1).into()),
                    ]),
                );
            }

            // Keep 16-color fallback palette tier in sync if present
            if let Some(c16) = &mut self.current_theme.colors_16 {
                let val_16 = bashmenu_tui::downsample_color_value_to_16(&val);
                if key == "background" {
                    c16.insert(key.to_string(), val_16);
                } else {
                    c16.insert(
                        key.to_string(),
                        serde_yml::Value::Sequence(vec![
                            val_16,
                            serde_yml::Value::Number((-1).into()),
                        ]),
                    );
                }
            }

            // Keep 8-color fallback palette tier in sync if present
            if let Some(c8) = &mut self.current_theme.colors_8 {
                let val_8 = bashmenu_tui::downsample_color_value_to_8(&val);
                if key == "background" {
                    c8.insert(key.to_string(), val_8);
                } else {
                    c8.insert(
                        key.to_string(),
                        serde_yml::Value::Sequence(vec![
                            val_8,
                            serde_yml::Value::Number((-1).into()),
                        ]),
                    );
                }
            }
            self.is_modified = true;
            let display_str = match color {
                Color::Indexed(code) => format!("color {}", code),
                Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
                _ => format_color_code(color),
            };
            self.status_message = format!("Updated {} to {}", key, display_str);
        }
    }

    /// Retrieves the current Ratatui Color for a palette token key.
    pub fn get_color_token_color(&self, key: &str) -> Color {
        if let Some(c_map) = &self.current_theme.colors_256 {
            if let Some(val) = c_map.get(key) {
                if let Some(c) = bashmenu_tui::parse_color_value(val) {
                    return c;
                }
            }
        }
        let styles = ResolvedThemeStyles::from_theme(&self.current_theme, true);
        match key {
            "background" => styles.background,
            "text" => styles.text.fg.unwrap_or(Color::White),
            "highlight" => styles.highlight.bg.unwrap_or(Color::Cyan),
            "border" => styles.border,
            "divider" => styles.divider.fg.unwrap_or(Color::Cyan),
            "accent" => styles.accent.fg.unwrap_or(Color::LightCyan),
            "shortcut_key" => styles.shortcut_key.fg.unwrap_or(Color::Yellow),
            "help_text" => styles.help_text.fg.unwrap_or(Color::Cyan),
            "plugin" => styles.plugin.fg.unwrap_or(Color::Cyan),
            "gutter" => styles.gutter.fg.unwrap_or(Color::DarkGray),
            "window_close_button" => styles.window_close_button.fg.unwrap_or(Color::Red),
            "scrollbar" => styles.scrollbar.fg.unwrap_or(Color::Cyan),
            "toast_info" => styles.toast_info,
            "toast_warning" => styles.toast_warning,
            "toast_error" => styles.toast_error,
            _ => Color::White,
        }
    }

    /// Applies or clears full background wallpaper configuration.
    pub fn set_wallpaper_config(&mut self, config: Option<bashmenu_core::WallpaperConfig>) {
        match config {
            Some(mut cfg) => {
                let has_img = cfg
                    .image
                    .as_deref()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false);
                if has_img {
                    cfg.enabled = true;
                    let display_name = cfg
                        .image
                        .as_deref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("image")
                        .to_string();
                    self.current_theme.background = Some(WallpaperSetting::Config(cfg));
                    self.is_modified = true;
                    self.status_message = format!("Configured wallpaper: {}", display_name);
                } else {
                    self.current_theme.background = None;
                    self.current_theme.extra.remove("wallpaper");
                    self.current_theme.extra.remove("theme_background");
                    self.is_modified = true;
                    self.status_message = "Removed wallpaper image".to_string();
                }
            }
            None => {
                self.current_theme.background = None;
                self.current_theme.extra.remove("wallpaper");
                self.current_theme.extra.remove("theme_background");
                self.is_modified = true;
                self.status_message = "Removed wallpaper image".to_string();
            }
        }
    }

    /// Configures or removes the background wallpaper image file path.
    pub fn set_wallpaper_image(&mut self, path: Option<String>) {
        match path {
            Some(p) => {
                let mut cfg = self.current_theme.wallpaper_config().unwrap_or_default();
                cfg.image = Some(p);
                self.set_wallpaper_config(Some(cfg));
            }
            None => {
                self.set_wallpaper_config(None);
            }
        }
    }

    /// Auto-derives theme colors based on the colors of the configured background wallpaper image.
    pub fn derive_colors_from_wallpaper(&mut self) -> std::result::Result<String, String> {
        let img_str = self
            .current_theme
            .wallpaper_config()
            .and_then(|wp| wp.image)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No background wallpaper image configured on this theme".to_string())?;

        let resolved_path = bashmenu_tui::resolve_wallpaper_path(&img_str)
            .ok_or_else(|| format!("Wallpaper image file not found: {}", img_str))?;

        let derived = bashmenu_tui::derive_palette_from_image(&resolved_path)?;
        let c_map = self
            .current_theme
            .colors_256
            .get_or_insert_with(HashMap::new);

        for (k, v) in derived {
            c_map.insert(k, v);
        }
        // Clear view-specific sub-table overrides so derived colors apply uniformly
        c_map.remove("menu_editor");
        c_map.remove("editor");
        c_map.remove("modals");

        // Also derive harmonious 16-color and 8-color fallback palette tiers
        let c_map_16 = bashmenu_tui::downsample_palette_to_16(c_map);
        let c_map_8 = bashmenu_tui::downsample_palette_to_8(c_map);
        self.current_theme.colors_16 = Some(c_map_16);
        self.current_theme.colors_8 = Some(c_map_8);

        self.is_modified = true;
        let fname = resolved_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&img_str)
            .to_string();
        self.status_message = format!("Derived theme colors from '{}'", fname);
        Ok(fname)
    }

    /// Retrieves the current 8-bit color index for a palette token key.
    pub fn get_color_token_u8(&self, key: &str) -> u8 {
        if let Some(c_map) = &self.current_theme.colors_256 {
            if let Some(val) = c_map.get(key) {
                if let Some(seq) = val.as_sequence() {
                    if let Some(fg) = seq.first().and_then(|v| v.as_i64()) {
                        if (0..=255).contains(&fg) {
                            return fg as u8;
                        }
                    }
                } else if let Some(n) = val.as_i64() {
                    if (0..=255).contains(&n) {
                        return n as u8;
                    }
                }
                if let Some(c) = bashmenu_tui::parse_color_value(val) {
                    return color_to_u8(c);
                }
            }
        }
        // Fallback to resolved styles from active theme
        let styles = ResolvedThemeStyles::from_theme(&self.current_theme, true);
        match key {
            "background" => color_to_u8(styles.background),
            "text" => styles.text.fg.map(color_to_u8).unwrap_or(255),
            "highlight" => styles.highlight.bg.map(color_to_u8).unwrap_or(255),
            "border" => color_to_u8(styles.border),
            "divider" => styles.divider.fg.map(color_to_u8).unwrap_or(255),
            "accent" => styles.accent.fg.map(color_to_u8).unwrap_or(255),
            "shortcut_key" => styles.shortcut_key.fg.map(color_to_u8).unwrap_or(255),
            "help_text" => styles.help_text.fg.map(color_to_u8).unwrap_or(255),
            "plugin" => styles.plugin.fg.map(color_to_u8).unwrap_or(255),
            "gutter" => styles.gutter.fg.map(color_to_u8).unwrap_or(255),
            "window_close_button" => styles
                .window_close_button
                .fg
                .map(color_to_u8)
                .unwrap_or(255),
            "scrollbar" => styles.scrollbar.fg.map(color_to_u8).unwrap_or(255),
            "toast_info" => color_to_u8(styles.toast_info),
            "toast_warning" => color_to_u8(styles.toast_warning),
            "toast_error" => color_to_u8(styles.toast_error),
            _ => 255,
        }
    }

    /// Sets the border glyph character for a specific window frame element.
    pub fn set_border_token(&mut self, key: &str, ch: char) {
        if self.current_theme.window.is_none() {
            self.current_theme.window = Some(WindowBorders::default());
        }
        if let Some(win) = &mut self.current_theme.window {
            let str_val = Some(ch.to_string());
            match key {
                "border_horizontal" => win.border_horizontal = str_val,
                "border_vertical" => win.border_vertical = str_val,
                "border_top_left" => win.border_top_left = str_val,
                "border_top_right" => win.border_top_right = str_val,
                "border_bottom_left" => win.border_bottom_left = str_val,
                "border_bottom_right" => win.border_bottom_right = str_val,
                "left_tee" => win.left_tee = str_val,
                "right_tee" => win.right_tee = str_val,
                "top_tee" => win.top_tee = str_val,
                "bottom_tee" => win.bottom_tee = str_val,
                "gutter_border" => win.gutter_border = str_val,
                "gutter_separator" => win.gutter_separator = str_val,
                "gutter_left_tee" => win.gutter_left_tee = str_val,
                "gutter_right_tee" => win.gutter_right_tee = str_val,
                "gutter_top_tee" => win.gutter_top_tee = str_val,
                "gutter_bottom_tee" => win.gutter_bottom_tee = str_val,
                _ => {}
            }
            self.is_modified = true;
            self.status_message = format!("Updated {} to '{}'", key, ch);
        }
    }

    /// Updates the character glyph used for divider rules.
    pub fn set_divider_char(&mut self, ch: char) {
        if self.current_theme.divider.is_none() {
            self.current_theme.divider = Some(DividerConfig::default());
        }
        if let Some(div) = &mut self.current_theme.divider {
            div.char = Some(ch.to_string());
            self.is_modified = true;
            self.status_message = format!("Updated divider character to '{}'", ch);
        }
    }

    /// Toggles divider length setting between `{window_width}` and `{screen_width}`.
    pub fn toggle_divider_length(&mut self) {
        if self.current_theme.divider.is_none() {
            self.current_theme.divider = Some(DividerConfig::default());
        }
        if let Some(div) = &mut self.current_theme.divider {
            let current = div.length.as_deref().unwrap_or("{window_width}");
            let new_len = if current == "{screen_width}" {
                "{window_width}"
            } else {
                "{screen_width}"
            };
            div.length = Some(new_len.to_string());
            self.is_modified = true;
            self.status_message = format!("Updated divider length to {}", new_len);
        }
    }

    /// Updates delimiter glyph character used in marquee tickers.
    pub fn set_marquee_char(&mut self, ch: char) {
        if self.current_theme.marquee.is_none() {
            self.current_theme.marquee = Some(MarqueeConfig::default());
        }
        if let Some(marq) = &mut self.current_theme.marquee {
            marq.char = ch.to_string();
            self.is_modified = true;
            self.status_message = format!("Updated marquee character to '{}'", ch);
        }
    }

    /// Cycles the marquee repetition spacing gap (1..10 spaces).
    pub fn cycle_marquee_gap(&mut self) {
        if self.current_theme.marquee.is_none() {
            self.current_theme.marquee = Some(MarqueeConfig::default());
        }
        if let Some(marq) = &mut self.current_theme.marquee {
            let gaps = [1, 2, 3, 4, 5, 6, 8, 10];
            let next_gap = match gaps.iter().position(|&g| g == marq.gap) {
                Some(idx) => gaps[(idx + 1) % gaps.len()],
                None => 4,
            };
            marq.gap = next_gap;
            self.is_modified = true;
            self.status_message = format!("Updated marquee gap to {}", next_gap);
        }
    }

    /// Updates left vertical accent bar character for toast popups.
    pub fn set_toast_char(&mut self, ch: char) {
        if self.current_theme.toast.is_none() {
            self.current_theme.toast = Some(bashmenu_core::ToastConfig::default());
        }
        if let Some(t) = &mut self.current_theme.toast {
            t.char = Some(ch.to_string());
            self.is_modified = true;
            self.status_message = format!("Updated toast left bar character to '{}'", ch);
        }
    }

    /// Computes the list of currently visible sidebar rows based on expanded categories.
    pub fn visible_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        for cat in TokenCategory::ALL {
            rows.push(TreeRow::Category(cat));
            if self.expanded_categories.contains(&cat) {
                for (idx, token) in self.tokens.iter().enumerate() {
                    if token.category() == cat {
                        rows.push(TreeRow::Token(idx));
                    }
                }
            }
        }
        rows
    }

    /// Returns `true` if the specified category section is currently expanded.
    pub fn is_category_expanded(&self, cat: TokenCategory) -> bool {
        self.expanded_categories.contains(&cat)
    }

    /// Expands the specified category in the sidebar tree.
    pub fn expand_category(&mut self, cat: TokenCategory) {
        self.expanded_categories.insert(cat);
    }

    /// Collapses the specified category in the sidebar tree.
    pub fn collapse_category(&mut self, cat: TokenCategory) {
        self.expanded_categories.remove(&cat);
        let rows = self.visible_rows();
        if self.selected_row >= rows.len() {
            self.selected_row = rows.len().saturating_sub(1);
        }
        self.sync_selected_idx_from_row(&rows);
    }

    /// Toggles the expansion state of the specified category.
    pub fn toggle_category(&mut self, cat: TokenCategory) {
        if self.expanded_categories.contains(&cat) {
            self.collapse_category(cat);
        } else {
            self.expand_category(cat);
        }
    }

    /// Synchronizes `selected_idx` to the token pointed to by the currently selected tree row.
    fn sync_selected_idx_from_row(&mut self, rows: &[TreeRow]) {
        if let Some(TreeRow::Token(idx)) = rows.get(self.selected_row) {
            self.selected_idx = *idx;
        }
    }

    /// Selects the visible row at the given index, synchronizing selected token index.
    pub fn select_visible_row(&mut self, row_idx: usize) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        self.selected_row = row_idx.min(rows.len().saturating_sub(1));
        self.sync_selected_idx_from_row(&rows);
    }

    /// Returns the currently selected TokenItem, if the active tree row points to a token
    /// (or falls back to `self.tokens[self.selected_idx]`).
    pub fn current_selected_token(&self) -> Option<&TokenItem> {
        let rows = self.visible_rows();
        if let Some(TreeRow::Token(idx)) = rows.get(self.selected_row) {
            self.tokens.get(*idx)
        } else {
            self.tokens.get(self.selected_idx)
        }
    }

    /// Handles Right arrow key navigation (expanding categories or descending into tokens).
    pub fn handle_right_key(&mut self) -> bool {
        let rows = self.visible_rows();
        if let Some(&TreeRow::Category(cat)) = rows.get(self.selected_row) {
            if !self.expanded_categories.contains(&cat) {
                self.expand_category(cat);
                return true;
            } else if self.selected_row + 1 < self.visible_rows().len() {
                self.select_visible_row(self.selected_row + 1);
                return true;
            }
        }
        false
    }

    /// Handles Left arrow key navigation (collapsing categories or ascending to header).
    pub fn handle_left_key(&mut self) -> bool {
        let rows = self.visible_rows();
        match rows.get(self.selected_row).copied() {
            Some(TreeRow::Category(cat)) => {
                if self.expanded_categories.contains(&cat) {
                    self.collapse_category(cat);
                    return true;
                }
            }
            Some(TreeRow::Token(idx)) => {
                let cat = self.tokens[idx].category();
                self.collapse_category(cat);
                let new_rows = self.visible_rows();
                if let Some(cat_pos) = new_rows.iter().position(|r| *r == TreeRow::Category(cat)) {
                    self.selected_row = cat_pos;
                }
                return true;
            }
            None => {}
        }
        false
    }

    /// Adjusts viewport scroll offset so selected row remains visible.
    pub fn adjust_scroll(&mut self, viewport_h: usize) {
        if viewport_h == 0 {
            return;
        }
        let rows = self.visible_rows();
        let active_pos = if self.expanded_categories.is_empty() && self.selected_idx > 0 {
            // Backward compatibility when tests set selected_idx directly without expanding tree
            self.selected_idx
        } else {
            self.selected_row.min(rows.len().saturating_sub(1))
        };
        if active_pos < self.scroll_offset {
            self.scroll_offset = active_pos;
        } else if active_pos >= self.scroll_offset + viewport_h {
            self.scroll_offset = active_pos.saturating_sub(viewport_h - 1);
        }
    }

    /// Saves all loaded themes to `bashmenu.themes` on disk in alphabetical order.
    pub fn save_to_disk(&mut self) -> Result<()> {
        self.themes
            .insert(self.current_name.clone(), self.current_theme.clone());
        save_themes(&self.themes_path, &self.themes)?;
        self.is_modified = false;
        self.status_message = format!("Saved theme '{}' directly to disk", self.current_name);
        Ok(())
    }

    /// Constructs a clean monochrome baseline theme.
    pub fn create_monochrome_theme() -> Theme {
        Theme::monochrome()
    }

    /// Creates a new theme using the given name and monochrome baseline.
    pub fn create_theme(&mut self, name: &str) -> std::result::Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Theme name cannot be empty".to_string());
        }
        if self.themes.contains_key(trimmed) {
            return Err(format!("Theme '{}' already exists", trimmed));
        }
        if self.is_modified {
            self.themes
                .insert(self.current_name.clone(), self.current_theme.clone());
        }
        let new_theme = Theme::monochrome();
        self.themes.insert(trimmed.to_string(), new_theme.clone());
        self.current_name = trimmed.to_string();
        self.current_theme = new_theme;
        self.name_input.set_text(&self.current_name);
        self.is_modified = true;
        self.status_message = format!(
            "Created new theme '{}' (monochrome base)",
            self.current_name
        );
        Ok(())
    }

    /// Deletes the named theme from the collection and updates disk storage.
    pub fn delete_theme(&mut self, name: &str) -> std::result::Result<(), String> {
        self.themes.remove(name);
        if self.themes.is_empty() {
            self.themes
                .insert("default".to_string(), Theme::monochrome());
        }
        if self.current_name == name {
            let next_name = if self.themes.contains_key("dracula") {
                "dracula".to_string()
            } else {
                let mut sorted: Vec<_> = self.themes.keys().cloned().collect();
                sorted.sort_by(|a, b| {
                    a.to_ascii_lowercase()
                        .cmp(&b.to_ascii_lowercase())
                        .then_with(|| a.cmp(b))
                });
                sorted
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "default".to_string())
            };
            self.current_name = next_name.clone();
            self.current_theme = self
                .themes
                .get(&next_name)
                .cloned()
                .unwrap_or_else(Theme::monochrome);
            self.name_input.set_text(&self.current_name);
        }
        save_themes(&self.themes_path, &self.themes)
            .map_err(|e| format!("Failed to save themes after deletion: {}", e))?;
        self.is_modified = false;
        self.status_message = format!("Deleted theme '{}'", name);
        Ok(())
    }

    /// Renames the active theme in memory.
    pub fn rename_theme(&mut self, new_name: &str) -> std::result::Result<(), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Theme name cannot be empty".to_string());
        }
        if trimmed == self.current_name {
            self.name_input.set_text(&self.current_name);
            return Ok(());
        }
        if self.themes.contains_key(trimmed) {
            return Err(format!("Theme '{}' already exists", trimmed));
        }
        self.themes.remove(&self.current_name);
        self.themes
            .insert(trimmed.to_string(), self.current_theme.clone());
        self.current_name = trimmed.to_string();
        self.name_input.set_text(&self.current_name);
        self.is_modified = true;
        self.status_message = format!("Renamed theme to '{}'", trimmed);
        Ok(())
    }

    /// Saves the active theme under a new name and commits to disk.
    pub fn save_as_theme(&mut self, new_name: &str) -> std::result::Result<(), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Theme name cannot be empty".to_string());
        }
        self.themes
            .insert(trimmed.to_string(), self.current_theme.clone());
        self.current_name = trimmed.to_string();
        self.name_input.set_text(&self.current_name);
        save_themes(&self.themes_path, &self.themes)
            .map_err(|e| format!("Failed to save theme: {}", e))?;
        self.is_modified = false;
        self.status_message = format!("Saved as new theme '{}'", trimmed);
        Ok(())
    }

    /// Switches the active editing target to the specified theme.
    pub fn switch_theme(&mut self, name: &str) -> bool {
        if let Some(th) = self.themes.get(name).cloned() {
            if self.is_modified {
                self.themes
                    .insert(self.current_name.clone(), self.current_theme.clone());
            }
            self.current_name = name.to_string();
            self.current_theme = th;
            self.name_input.set_text(&self.current_name);
            self.is_modified = false;
            self.status_message = format!("Switched to theme '{}'", name);
            true
        } else {
            false
        }
    }

    /// Cycles forward or backward through alphabetically sorted themes.
    pub fn cycle_theme(&mut self, forward: bool) {
        if self.themes.is_empty() {
            return;
        }
        let mut sorted: Vec<_> = self.themes.keys().cloned().collect();
        sorted.sort_by(|a, b| {
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
                .then_with(|| a.cmp(b))
        });
        if let Some(pos) = sorted.iter().position(|k| k == &self.current_name) {
            let next_idx = if forward {
                (pos + 1) % sorted.len()
            } else if pos == 0 {
                sorted.len().saturating_sub(1)
            } else {
                pos - 1
            };
            self.switch_theme(&sorted[next_idx]);
        } else if let Some(first) = sorted.first() {
            self.switch_theme(first);
        }
    }

    /// Focuses/selects the token corresponding to the given style or border key,
    /// automatically expanding its parent category node and focusing the row in the tree.
    pub fn select_token_by_key(&mut self, key: &str) -> bool {
        // 1. Exact match first
        let matched_idx = self
            .tokens
            .iter()
            .position(|t| match &t.kind {
                TokenKind::Color(k) => k == key,
                TokenKind::Border(k) => k == key,
                TokenKind::Indicator => key == "indicator",
                TokenKind::DividerChar => key == "divider_char",
                TokenKind::DividerLength => key == "divider_length",
                TokenKind::MarqueeChar => key == "marquee_char",
                TokenKind::MarqueeGap => key == "marquee_gap",
                TokenKind::ToastChar => key == "toast_char",
                TokenKind::WallpaperImage => key == "wallpaper" || key == "wallpaper_image",
                TokenKind::WallpaperOpacity => key == "wallpaper_opacity" || key == "opacity",
                TokenKind::WallpaperScaling => key == "wallpaper_scaling" || key == "scaling",
                TokenKind::WallpaperMode => key == "wallpaper_mode" || key == "mode",
                TokenKind::WallpaperClustering => {
                    key == "wallpaper_clustering" || key == "clustering"
                }
                TokenKind::WallpaperQuantization => {
                    key == "wallpaper_quantization" || key == "quantization"
                }
            })
            .or_else(|| {
                // 2. Fallback aliases
                self.tokens.iter().position(|t| match &t.kind {
                    TokenKind::Color(k) => {
                        (k == "shortcut_key" && (key == "shortcut_gutter" || key == "shortcut"))
                            || (k == "text" && key == "content")
                            || (k == "plugin" && (key == "plugin_text" || key == "plugins"))
                            || (k == "background"
                                && (key == "bg" || key == "bg_color" || key == "background_color"))
                    }
                    TokenKind::Border(k) => {
                        (k == "top_tee" && key == "gutter_top_tee")
                            || (k == "bottom_tee" && key == "gutter_bottom_tee")
                            || (k == "gutter_top_tee" && key == "top_tee")
                            || (k == "gutter_bottom_tee" && key == "bottom_tee")
                            || (k == "left_tee" && key == "gutter_left_tee")
                            || (k == "right_tee" && key == "gutter_right_tee")
                            || (k == "gutter_left_tee" && key == "left_tee")
                            || (k == "gutter_right_tee" && key == "right_tee")
                            || (k == "border_horizontal"
                                && (key == "border_bottom" || key == "border_top"))
                    }
                    TokenKind::DividerChar => key == "divider",
                    TokenKind::MarqueeChar => key == "marquee",
                    TokenKind::ToastChar => {
                        key == "toast_char" || key == "toast_bar" || key == "toast"
                    }
                    TokenKind::WallpaperImage => {
                        key == "wallpaper" || key == "wallpaper_image" || key == "bg_image"
                    }
                    _ => false,
                })
            });

        if let Some(idx) = matched_idx {
            let cat = self.tokens[idx].category();
            self.expanded_categories.insert(cat);
            self.selected_idx = idx;
            let rows = self.visible_rows();
            if let Some(r_idx) = rows.iter().position(|r| *r == TreeRow::Token(idx)) {
                self.selected_row = r_idx;
            }
            self.status_message.clear();
            return true;
        }
        false
    }
}

/// Calculates visual rectangular spans `(y, x_start, x_end)` in the preview pane corresponding
/// to a theme token so the UI can flash them when selected or clicked.
pub fn get_token_preview_spans(
    token: &TokenItem,
    right_area: Rect,
    _mode: PreviewMode,
    _preview_styles: &ResolvedThemeStyles,
) -> Vec<(u16, u16, u16)> {
    let mut spans = Vec::new();
    if right_area.width < 6 || right_area.height < 6 {
        return spans;
    }
    let w = right_area.width as usize;
    let avail_both = (w.saturating_sub(6)).max(4);
    let left_gutter_w = avail_both / 2;
    let sep_x = (right_area.x + 2 + left_gutter_w as u16)
        .min(right_area.x + right_area.width.saturating_sub(2));
    let gt_y = right_area.y + right_area.height.saturating_sub(3);
    let p_inner_x = right_area.x + 2;
    let p_avail_w = right_area.width.saturating_sub(4);
    let content_start_y = right_area.y + 2;
    let content_avail_h = gt_y.saturating_sub(content_start_y);
    let plugin_start_y = gt_y.saturating_sub(if content_avail_h >= 8 { 3 } else { 1 });
    let bot_y = right_area.y + right_area.height.saturating_sub(1);

    match &token.kind {
        TokenKind::Color(key) if key == "background" => {
            for y in (right_area.y + 1)..bot_y {
                spans.push((
                    y,
                    right_area.x + 1,
                    right_area.x + right_area.width.saturating_sub(1),
                ));
            }
        }
        TokenKind::WallpaperImage
        | TokenKind::WallpaperOpacity
        | TokenKind::WallpaperScaling
        | TokenKind::WallpaperMode
        | TokenKind::WallpaperClustering
        | TokenKind::WallpaperQuantization => {
            for y in (right_area.y + 1)..bot_y {
                spans.push((
                    y,
                    right_area.x + 1,
                    right_area.x + right_area.width.saturating_sub(1),
                ));
            }
        }
        TokenKind::Color(key) if key == "border" => {
            spans.push((right_area.y, right_area.x, right_area.x + right_area.width));
            spans.push((bot_y, right_area.x, right_area.x + right_area.width));
            for y in (right_area.y + 1)..bot_y {
                spans.push((y, right_area.x, right_area.x + 1));
                spans.push((
                    y,
                    right_area.x + right_area.width.saturating_sub(1),
                    right_area.x + right_area.width,
                ));
            }
        }
        TokenKind::Border(key) if key == "border_horizontal" => {
            spans.push((
                right_area.y,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ));
            spans.push((
                bot_y,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ));
        }
        TokenKind::Border(key) if key == "border_vertical" => {
            for y in (right_area.y + 1)..bot_y {
                spans.push((y, right_area.x, right_area.x + 1));
                spans.push((
                    y,
                    right_area.x + right_area.width.saturating_sub(1),
                    right_area.x + right_area.width,
                ));
            }
        }
        TokenKind::Border(key) if key == "border_top_left" => {
            spans.push((right_area.y, right_area.x, right_area.x + 1));
        }
        TokenKind::Border(key) if key == "border_top_right" => {
            spans.push((
                right_area.y,
                right_area.x + right_area.width.saturating_sub(1),
                right_area.x + right_area.width,
            ));
        }
        TokenKind::Border(key) if key == "border_bottom_left" => {
            spans.push((bot_y, right_area.x, right_area.x + 1));
        }
        TokenKind::Border(key) if key == "border_bottom_right" => {
            spans.push((
                bot_y,
                right_area.x + right_area.width.saturating_sub(1),
                right_area.x + right_area.width,
            ));
        }
        TokenKind::Border(key) if key == "left_tee" || key == "gutter_left_tee" => {
            spans.push((gt_y, right_area.x, right_area.x + 1));
        }
        TokenKind::Border(key) if key == "right_tee" || key == "gutter_right_tee" => {
            spans.push((
                gt_y,
                right_area.x + right_area.width.saturating_sub(1),
                right_area.x + right_area.width,
            ));
        }
        TokenKind::Border(key) if key == "top_tee" || key == "gutter_top_tee" => {
            spans.push((gt_y, sep_x, sep_x + 1));
        }
        TokenKind::Border(key) if key == "bottom_tee" || key == "gutter_bottom_tee" => {
            spans.push((bot_y, sep_x, sep_x + 1));
        }
        TokenKind::Border(key) if key == "gutter_border" => {
            spans.push((
                gt_y,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ));
        }
        TokenKind::Border(key) if key == "gutter_separator" => {
            spans.push((gt_y + 1, sep_x, sep_x + 1));
        }
        TokenKind::Color(key) if key == "title" => {
            spans.push((
                right_area.y,
                right_area.x + 2,
                right_area.x + right_area.width.saturating_sub(2),
            ));
        }
        TokenKind::Color(key) if key == "highlight" => {
            spans.push((content_start_y, p_inner_x, p_inner_x + p_avail_w));
        }
        TokenKind::Color(key) if key == "text" => {
            spans.push((content_start_y + 1, p_inner_x + 2, p_inner_x + 22));
            spans.push((content_start_y + 2, p_inner_x + 2, p_inner_x + 28));
            spans.push((content_start_y + 4, p_inner_x + 2, p_inner_x + 24));
        }
        TokenKind::Color(key) if key == "accent" => {
            // Accent is the submenu chevron indicator glyph '»' on inactive submenu item 2
            spans.push((content_start_y + 1, p_inner_x + 22, p_inner_x + 26));
        }
        TokenKind::Color(key) if key == "shortcut_key" => {
            // Shortcut keys in help gutter
            spans.push((gt_y + 1, right_area.x + 2, sep_x));
        }
        TokenKind::Indicator => {
            spans.push((content_start_y, p_inner_x, p_inner_x + 2));
        }
        TokenKind::Color(key) if key == "divider" => {
            spans.push((content_start_y + 3, p_inner_x, p_inner_x + p_avail_w));
            if content_avail_h >= 8 {
                spans.push((plugin_start_y, p_inner_x, p_inner_x + p_avail_w));
            }
        }
        TokenKind::DividerChar | TokenKind::DividerLength => {
            spans.push((content_start_y + 3, p_inner_x, p_inner_x + p_avail_w));
        }
        TokenKind::MarqueeChar | TokenKind::MarqueeGap => {
            spans.push((
                content_start_y + 1,
                p_inner_x + 10,
                right_area.x + right_area.width.saturating_sub(6),
            ));
        }
        TokenKind::Color(key) if key == "help_text" => {
            spans.push((gt_y + 1, right_area.x + 2, sep_x));
        }
        TokenKind::Color(key) if key == "plugin" => {
            if content_avail_h >= 8 {
                spans.push((plugin_start_y + 1, p_inner_x, p_inner_x + p_avail_w));
                spans.push((plugin_start_y + 2, p_inner_x, p_inner_x + p_avail_w));
            } else if content_avail_h >= 6 {
                spans.push((gt_y.saturating_sub(1), p_inner_x, p_inner_x + p_avail_w));
            }
        }
        TokenKind::Color(key) if key == "gutter" => {
            spans.push((
                gt_y + 1,
                sep_x + 1,
                right_area.x + right_area.width.saturating_sub(2),
            ));
        }
        TokenKind::Color(key) if key == "scrollbar" => {
            let top_sb = (right_area.y + 3).min(bot_y);
            let bot_sb = (right_area.y + 6).min(bot_y);
            for y in top_sb..=bot_sb {
                spans.push((
                    y,
                    right_area.x + right_area.width.saturating_sub(1),
                    right_area.x + right_area.width,
                ));
            }
        }
        TokenKind::ToastChar => {
            let toast_w = 40_u16.min(right_area.width.saturating_sub(4));
            let toast_h = 4_u16.min(right_area.height.saturating_sub(2));
            let toast_x = right_area.x + right_area.width.saturating_sub(toast_w + 2);
            let toast_y = bot_y.saturating_sub(toast_h + 1);
            for y in toast_y..toast_y + toast_h {
                spans.push((y, toast_x, toast_x + 2));
            }
        }
        TokenKind::Color(key) if key.starts_with("toast_") => {
            let toast_w = 40_u16.min(right_area.width.saturating_sub(4));
            let toast_h = 4_u16.min(right_area.height.saturating_sub(2));
            let toast_x = right_area.x + right_area.width.saturating_sub(toast_w + 2);
            let toast_y = bot_y.saturating_sub(toast_h + 1);
            for y in toast_y..toast_y + toast_h {
                spans.push((y, toast_x, toast_x + toast_w));
            }
        }
        _ => {
            spans.push((content_start_y, p_inner_x, p_inner_x + p_avail_w));
        }
    }
    spans
}

/// Returns the default directory when looking for wallpaper images.
///
/// Prioritizes the repository's `assets/wallpapers` directory across multiple discovery
/// strategies:
/// 1. `$BASHMENU_DIR/assets/wallpapers` if `$BASHMENU_DIR` environment variable is set.
/// 2. Ancestor traversal from the current working directory (`CWD`) looking for `assets/wallpapers`.
/// 3. Ancestor traversal from the current executable looking for `assets/wallpapers`.
/// 4. Compile-time `$CARGO_MANIFEST_DIR` ancestor traversal.
/// 5. Standard user config (`~/.config/bashmenu/assets/wallpapers` or `~/.config/bashmenu/wallpapers`)
///    and system share directories (`/usr/local/share/bashmenu/assets/wallpapers`, `/opt/bashmenu/assets/wallpapers`).
/// 6. Falls back to CWD if no wallpapers directory is found.
pub fn default_wallpaper_dir() -> PathBuf {
    use std::path::Path;

    // 1. Check $BASHMENU_DIR/assets/wallpapers
    if let Ok(bdir) = std::env::var("BASHMENU_DIR") {
        let p = PathBuf::from(bdir).join("assets/wallpapers");
        if p.is_dir() {
            return p;
        }
    }

    // 2. Search upward from current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur: Option<&Path> = Some(&cwd);
        while let Some(dir) = cur {
            let p = dir.join("assets/wallpapers");
            if p.is_dir() {
                return p;
            }
            cur = dir.parent();
        }
    }

    // 3. Search upward from current executable directory
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent();
        while let Some(dir) = cur {
            let p = dir.join("assets/wallpapers");
            if p.is_dir() {
                return p;
            }
            cur = dir.parent();
        }
    }

    // 4. Compile-time CARGO_MANIFEST_DIR fallback (for developer builds / checkouts)
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let mut cur = Path::new(manifest).parent();
        while let Some(dir) = cur {
            let p = dir.join("assets/wallpapers");
            if p.is_dir() {
                return p;
            }
            cur = dir.parent();
        }
    }

    // 5. User config & system installation directories
    let user_wp = bashmenu_core::get_user_config_dir().join("assets/wallpapers");
    if user_wp.is_dir() {
        return user_wp;
    }
    let alt_user_wp = bashmenu_core::get_user_config_dir().join("wallpapers");
    if alt_user_wp.is_dir() {
        return alt_user_wp;
    }
    let share_wp = PathBuf::from("/usr/local/share/bashmenu/assets/wallpapers");
    if share_wp.is_dir() {
        return share_wp;
    }
    let opt_wp = PathBuf::from("/opt/bashmenu/assets/wallpapers");
    if opt_wp.is_dir() {
        return opt_wp;
    }

    // 6. Final fallback to current directory
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Formats a picked image path into an appropriate representation for storing in a theme.
///
/// If the file is inside the repository's `assets/wallpapers` directory (or any ancestor
/// containing `assets/wallpapers`), formats it as `assets/wallpapers/<filename>`.
/// If the file is inside CWD, strips CWD prefix.
/// Otherwise returns the path as a string.
pub fn relativize_wallpaper_path(path: &std::path::Path) -> String {
    // Check if path is inside assets/wallpapers
    let default_dir = default_wallpaper_dir();
    if let Ok(rel) = path.strip_prefix(&default_dir) {
        return format!("assets/wallpapers/{}", rel.to_string_lossy());
    }
    if let Some(repo_root) = default_dir.parent() {
        if let Ok(rel) = path.strip_prefix(repo_root) {
            return rel.to_string_lossy().to_string();
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rel) = path.strip_prefix(&cwd) {
            return rel.to_string_lossy().to_string();
        }
    }
    path.to_string_lossy().to_string()
}
