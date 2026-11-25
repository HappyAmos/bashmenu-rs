//! Library module for ThemeEdit core state, actions, and theme manipulation.

pub mod preview;
pub use preview::{
    extract_preview_cell_color, extract_preview_color_at, format_color_code,
    preview_tab_bar_layout, render_preview_pane, render_preview_pane_with_base,
    toast_preview_boxes, PreviewTabSpan,
};

use anyhow::Result;
use bashmenu_core::{
    default_activity_overrides_map, is_scalar_color_token, load_themes, save_themes, DividerConfig,
    MarqueeConfig, Theme, ThemeCollection, ToastStyle, WallpaperSetting, WindowBorders,
};
use bashmenu_tui::{is_inherited_color_value, ResolvedThemeStyles, TextInput, ThemeActivity};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

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
    /// Default menu item icon glyph definition (e.g., "{nf:>:#f105}")
    DefaultIcon,
    /// Section divider rule character
    DividerChar,
    /// Section divider rule length expression
    DividerLength,
    /// Horizontal marquee ticker scroll delimiter character
    MarqueeChar,
    /// Marquee scroll repeat gap in spaces
    MarqueeGap,
    /// Visual layout style for toast notifications ("Bar" or "Framed")
    ToastStyle,
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
            | TokenKind::DefaultIcon
            | TokenKind::DividerChar
            | TokenKind::DividerLength
            | TokenKind::MarqueeChar
            | TokenKind::MarqueeGap
            | TokenKind::ToastStyle
            | TokenKind::ToastChar => TokenCategory::Theme,
        }
    }
}

/// Active preview mode displaying different BashMenu screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    /// Main menu interactive preview (edits base theme palette)
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
    /// Visual theme editor (`themeedit`) preview
    ThemeEditor,
    /// Shortcuts editor (`shortcutedit`) preview
    ShortcutEditor,
}

impl PreviewMode {
    /// Ordered list of all seven preview tabs (`1` through `7`).
    pub const ALL: [PreviewMode; 7] = [
        PreviewMode::Menu,
        PreviewMode::TextEditor,
        PreviewMode::Modals,
        PreviewMode::MenuEditor,
        PreviewMode::Toast,
        PreviewMode::ThemeEditor,
        PreviewMode::ShortcutEditor,
    ];

    /// Cycles to the next preview mode screen.
    pub fn next(self) -> Self {
        match self {
            PreviewMode::Menu => PreviewMode::TextEditor,
            PreviewMode::TextEditor => PreviewMode::Modals,
            PreviewMode::Modals => PreviewMode::MenuEditor,
            PreviewMode::MenuEditor => PreviewMode::Toast,
            PreviewMode::Toast => PreviewMode::ThemeEditor,
            PreviewMode::ThemeEditor => PreviewMode::ShortcutEditor,
            PreviewMode::ShortcutEditor => PreviewMode::Menu,
        }
    }

    /// Cycles to the previous preview mode screen (`Shift+Tab`).
    pub fn prev(self) -> Self {
        match self {
            PreviewMode::Menu => PreviewMode::ShortcutEditor,
            PreviewMode::TextEditor => PreviewMode::Menu,
            PreviewMode::Modals => PreviewMode::TextEditor,
            PreviewMode::MenuEditor => PreviewMode::Modals,
            PreviewMode::Toast => PreviewMode::MenuEditor,
            PreviewMode::ThemeEditor => PreviewMode::Toast,
            PreviewMode::ShortcutEditor => PreviewMode::ThemeEditor,
        }
    }

    /// Returns the `ThemeActivity` associated with this preview tab.
    pub fn activity(self) -> ThemeActivity {
        match self {
            PreviewMode::Menu => ThemeActivity::MainMenu,
            PreviewMode::TextEditor => ThemeActivity::BashEdit,
            PreviewMode::Modals => ThemeActivity::Modals,
            PreviewMode::MenuEditor => ThemeActivity::MenuEdit,
            PreviewMode::Toast => ThemeActivity::Toasts,
            PreviewMode::ThemeEditor => ThemeActivity::ThemeEdit,
            PreviewMode::ShortcutEditor => ThemeActivity::ShortcutEdit,
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
            PreviewMode::ThemeEditor => "Theme Editor",
            PreviewMode::ShortcutEditor => "Shortcuts Editor",
        }
    }

    /// Compact numbered tab label for the preview pane header bar.
    pub fn tab_label(self) -> &'static str {
        match self {
            PreviewMode::Menu => "1:Menu",
            PreviewMode::TextEditor => "2:Editor",
            PreviewMode::Modals => "3:Modals",
            PreviewMode::MenuEditor => "4:MenuEdit",
            PreviewMode::Toast => "5:Toasts",
            PreviewMode::ThemeEditor => "6:ThemeEdit",
            PreviewMode::ShortcutEditor => "7:Shortcuts",
        }
    }

    /// Ultra-compact numbered label used on narrow split panes.
    pub fn short_label(self) -> &'static str {
        match self {
            PreviewMode::Menu => "1:Menu",
            PreviewMode::TextEditor => "2:Edit",
            PreviewMode::Modals => "3:Mod",
            PreviewMode::MenuEditor => "4:MEd",
            PreviewMode::Toast => "5:Tst",
            PreviewMode::ThemeEditor => "6:Thm",
            PreviewMode::ShortcutEditor => "7:Keys",
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

        let target_is_random = target_theme
            .as_deref()
            .map(bashmenu_core::is_random_theme_spec)
            .unwrap_or(false);

        let initial_name = if target_is_random {
            bashmenu_core::pick_random_theme(&themes)
                .map(|(n, _)| n.to_string())
                .or_else(|| {
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
        } else {
            target_theme.filter(|n| !n.trim().is_empty())
        }
        .or_else(|| {
            let check_cfg = |cfg: &bashmenu_core::BashMenuConfig| -> Option<String> {
                if bashmenu_core::is_random_theme_spec(&cfg.theme) {
                    bashmenu_core::pick_random_theme(&themes).map(|(n, _)| n.to_string())
                } else if themes.contains_key(&cfg.theme) {
                    Some(cfg.theme.clone())
                } else {
                    None
                }
            };

            // Try reading active theme from bashmenu.yml if available
            let cfg_path = bashmenu_core::resolve_app_path(Path::new("bashmenu.yml"));
            if let Ok(cfg) = bashmenu_core::load_config(&cfg_path) {
                if let Some(name) = check_cfg(&cfg) {
                    return Some(name);
                }
            }
            if let Ok(home) = std::env::var("HOME") {
                let cfg_path = PathBuf::from(home).join(".config/bashmenu/bashmenu.yml");
                if let Ok(cfg) = bashmenu_core::load_config(&cfg_path) {
                    if let Some(name) = check_cfg(&cfg) {
                        return Some(name);
                    }
                }
            }
            let cur_cfg = PathBuf::from("bashmenu.yml");
            if let Ok(cfg) = bashmenu_core::load_config(&cur_cfg) {
                if let Some(name) = check_cfg(&cfg) {
                    return Some(name);
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
            // Theme Attributes: Indicator, Wallpaper, Divider, Marquee
            TokenItem {
                label: "Indicator".to_string(),
                kind: TokenKind::Indicator,
            },
            TokenItem {
                label: "Default Icon".to_string(),
                kind: TokenKind::DefaultIcon,
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
            // Colors
            TokenItem {
                label: "Background Color".to_string(),
                kind: TokenKind::Color("background".to_string()),
            },
            TokenItem {
                label: "Title Color".to_string(),
                kind: TokenKind::Color("title".to_string()),
            },
            TokenItem {
                label: "Text Color".to_string(),
                kind: TokenKind::Color("text".to_string()),
            },
            TokenItem {
                label: "Highlight Color".to_string(),
                kind: TokenKind::Color("highlight".to_string()),
            },
            TokenItem {
                label: "Highlight Text Color".to_string(),
                kind: TokenKind::Color("highlight_text".to_string()),
            },
            TokenItem {
                label: "Selection Color".to_string(),
                kind: TokenKind::Color("selection".to_string()),
            },
            TokenItem {
                label: "Selected Tab Color".to_string(),
                kind: TokenKind::Color("selected_tab".to_string()),
            },
            TokenItem {
                label: "Unselected Tab Color".to_string(),
                kind: TokenKind::Color("unselected_tab".to_string()),
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
                label: "Shortcut Gutter".to_string(),
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
                label: "Footer Color".to_string(),
                kind: TokenKind::Color("footer".to_string()),
            },
            TokenItem {
                label: "Window Close Button".to_string(),
                kind: TokenKind::Color("window_close_button".to_string()),
            },
            TokenItem {
                label: "Scrollbar Color".to_string(),
                kind: TokenKind::Color("scrollbar".to_string()),
            },
            TokenItem {
                label: "Shadow Color".to_string(),
                kind: TokenKind::Color("shadow".to_string()),
            },
            TokenItem {
                label: "Modal Yes Button".to_string(),
                kind: TokenKind::Color("modal_yes_button".to_string()),
            },
            TokenItem {
                label: "Modal No Button".to_string(),
                kind: TokenKind::Color("modal_no_button".to_string()),
            },
            TokenItem {
                label: "Modal Cancel Button".to_string(),
                kind: TokenKind::Color("modal_cancel_button".to_string()),
            },
            TokenItem {
                label: "Line Numbers Color".to_string(),
                kind: TokenKind::Color("line_numbers".to_string()),
            },
            TokenItem {
                label: "Whitespace Color".to_string(),
                kind: TokenKind::Color("whitespace".to_string()),
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
                label: "Toast Style".to_string(),
                kind: TokenKind::ToastStyle,
            },
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
        t.default_icon = Some(bashmenu_core::DEFAULT_ITEM_ICON.to_string());

        let mut colors = HashMap::new();
        colors.insert(
            "title".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(201.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "text".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(255.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "highlight".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(255.into()),
                serde_json::Value::Number(201.into()),
            ]),
        );
        colors.insert(
            "border".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(61.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "divider".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(61.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "accent".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(117.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "shortcut_key".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(201.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "gutter".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(239.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "plugin".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(212.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "footer".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(61.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "window_close_button".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(203.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "shadow".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(236.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "modal_yes_button".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(117.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "modal_no_button".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(203.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "modal_cancel_button".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(245.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "selection".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(255.into()),
                serde_json::Value::Number(201.into()),
            ]),
        );
        colors.insert(
            "selected_tab".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(255.into()),
                serde_json::Value::Number(201.into()),
            ]),
        );
        colors.insert(
            "unselected_tab".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(245.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "line_numbers".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(242.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "whitespace".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(239.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "toast_info".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(117.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "toast_warning".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(214.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.insert(
            "toast_error".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(196.into()),
                serde_json::Value::Number((-1).into()),
            ]),
        );
        colors.extend(default_activity_overrides_map());
        t.colors_16 = Some(bashmenu_tui::downsample_palette_to_16(&colors));
        t.colors_8 = Some(bashmenu_tui::downsample_palette_to_8(&colors));
        t.colors_256 = Some(colors);
        t
    }

    /// Returns a human-readable display string representing the token's current value.
    pub fn get_token_value_display(&self, token: &TokenItem) -> String {
        match &token.kind {
            TokenKind::Color(key) => {
                if self.is_color_token_inherited(key) {
                    let base =
                        resolve_activity_display(&self.current_theme, ThemeActivity::MainMenu, key);
                    return format!("Inherit ({})", base);
                }
                resolve_activity_display(&self.current_theme, self.active_activity(), key)
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
                let resolved = bashmenu_core::resolve_glyph(raw, true);
                if resolved != raw {
                    format!("'{}' ({})", resolved, raw)
                } else {
                    format!("'{}'", raw)
                }
            }
            TokenKind::DefaultIcon => {
                let raw = self.current_theme.resolved_default_icon();
                let resolved = bashmenu_core::resolve_glyph(raw, true);
                if resolved != raw {
                    format!("'{}' ({})", resolved, raw)
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
            TokenKind::ToastStyle => self.get_toast_style().display_label().to_string(),
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

    /// Returns the `ThemeActivity` edited on the current preview tab.
    pub fn active_activity(&self) -> ThemeActivity {
        self.preview_mode.activity()
    }

    /// Returns `true` if `key` currently inherits from `MainMenu` (`-1`).
    ///
    /// Always returns `false` on `PreviewMode::Menu` because tab 1 edits the
    /// base `bashmenu` palette itself.
    pub fn is_color_token_inherited(&self, key: &str) -> bool {
        let activity = self.active_activity();
        if activity == ThemeActivity::MainMenu {
            return false;
        }
        let Some(c_map) = &self.current_theme.colors_256 else {
            return true;
        };
        match find_tier_token_value(c_map, activity, key) {
            None => true,
            Some(val) => is_token_value_inherited_for_key(key, val),
        }
    }

    /// Resets an activity override token to `-1` (or `[-1, -1]`) across tiers.
    ///
    /// Returns `true` when an override was cleared; returns `false` on Tab 1
    /// (`MainMenu`) or if the token already inherits from the base palette.
    pub fn reset_color_token_to_inherit(&mut self, key: &str) -> bool {
        let activity = self.active_activity();
        if activity == ThemeActivity::MainMenu || self.is_color_token_inherited(key) {
            return false;
        }
        let inherit_val = if is_scalar_color_token(key) {
            serde_json::Value::Number((-1).into())
        } else {
            serde_json::Value::Array(vec![
                serde_json::Value::Number((-1).into()),
                serde_json::Value::Number((-1).into()),
            ])
        };

        if let Some(c_map) = &mut self.current_theme.colors_256 {
            reset_tier_token_entry(c_map, activity, key, &inherit_val);
        }
        if let Some(c16) = &mut self.current_theme.colors_16 {
            reset_tier_token_entry(c16, activity, key, &inherit_val);
        }
        if let Some(c8) = &mut self.current_theme.colors_8 {
            reset_tier_token_entry(c8, activity, key, &inherit_val);
        }

        self.is_modified = true;
        self.status_message = format!(
            "Reset {} ({}) to inherit (-1)",
            key,
            self.preview_mode.title()
        );
        true
    }

    /// Updates a color token entry with a specified Ratatui Color (Indexed, RGB, or named).
    pub fn set_color_token_value(&mut self, key: &str, color: Color) {
        let activity = self.active_activity();
        if self.current_theme.colors_256.is_none() {
            self.current_theme.colors_256 = Some(HashMap::new());
        }
        let raw_val = match color {
            Color::Reset => serde_json::Value::Number((-1).into()),
            Color::Indexed(code) => serde_json::Value::Number((code as i64).into()),
            Color::Rgb(r, g, b) => serde_json::Value::String(format!("#{r:02x}{g:02x}{b:02x}")),
            _ => {
                let (r, g, b) = bashmenu_tui::color_to_rgb(color);
                serde_json::Value::String(format!("#{r:02x}{g:02x}{b:02x}"))
            }
        };

        if let Some(c_map) = &mut self.current_theme.colors_256 {
            write_color_token_to_tier(
                c_map,
                activity,
                key,
                color,
                &raw_val,
                PaletteTierTarget::Color256,
            );
        }
        if let Some(c16) = &mut self.current_theme.colors_16 {
            write_color_token_to_tier(
                c16,
                activity,
                key,
                color,
                &raw_val,
                PaletteTierTarget::Color16,
            );
        }
        if let Some(c8) = &mut self.current_theme.colors_8 {
            write_color_token_to_tier(
                c8,
                activity,
                key,
                color,
                &raw_val,
                PaletteTierTarget::Color8,
            );
        }

        self.is_modified = true;
        let display_str = match color {
            Color::Reset => "inherit (-1)".to_string(),
            Color::Indexed(code) => format!("color {}", code),
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => format_color_code(color),
        };
        if activity == ThemeActivity::MainMenu {
            self.status_message = format!("Updated {} to {}", key, display_str);
        } else {
            self.status_message = format!(
                "Updated {} ({}) to {}",
                key,
                self.preview_mode.title(),
                display_str
            );
        }
    }

    /// Retrieves the current Ratatui Color for a palette token key.
    pub fn get_color_token_color(&self, key: &str) -> Color {
        resolve_activity_color(&self.current_theme, self.active_activity(), key)
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
        // Clear legacy sub-table keys and reset all 6 activity override maps to -1
        for legacy_key in [
            "menu_editor",
            "editor",
            "modals",
            "button_primary",
            "button_error",
            "button_cancel",
            "button_success",
        ] {
            c_map.remove(legacy_key);
        }
        for (act_key, act_map) in default_activity_overrides_map() {
            c_map.insert(act_key, act_map);
        }

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
        color_to_u8(self.get_color_token_color(key))
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

    /// Updates the selection indicator string (e.g., `"> "`, `"▶ "`, `"{ascii:240}"`, `"{nf:...}"`).
    pub fn set_indicator(&mut self, indicator: String) {
        let val = if indicator.is_empty() {
            "> ".to_string()
        } else {
            indicator
        };
        self.current_theme.indicator = Some(val.clone());
        self.is_modified = true;
        self.status_message = format!("Updated indicator symbol to '{}'", val);
    }

    /// Updates default menu item icon (e.g. `"{nf:>:#f105}"`).
    pub fn set_default_icon(&mut self, icon: String) {
        let trimmed = icon.trim().to_string();
        self.current_theme.default_icon = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.clone())
        };
        self.is_modified = true;
        self.status_message = format!("Updated default icon to '{}'", trimmed);
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

    /// Returns the configured toast layout style for the current theme.
    pub fn get_toast_style(&self) -> ToastStyle {
        self.current_theme
            .toast
            .as_ref()
            .and_then(|t| t.style.as_deref())
            .map(ToastStyle::from_str_name)
            .unwrap_or_default()
    }

    /// Sets the toast layout style for the current theme.
    pub fn set_toast_style(&mut self, style: ToastStyle) {
        if self.current_theme.toast.is_none() {
            self.current_theme.toast = Some(bashmenu_core::ToastConfig::default());
        }
        if let Some(t) = &mut self.current_theme.toast {
            t.style = Some(style.as_str().to_string());
            self.is_modified = true;
            self.status_message = format!("Updated toast style to '{}'", style.display_label());
        }
    }

    /// Toggles the toast layout style between Bar and Framed.
    pub fn toggle_toast_style(&mut self) -> ToastStyle {
        let next = self.get_toast_style().toggle();
        self.set_toast_style(next);
        next
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
        if bashmenu_core::is_random_theme_spec(trimmed) {
            return Err("Theme name cannot be reserved keyword 'Random'".to_string());
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
        if bashmenu_core::is_random_theme_spec(trimmed) {
            return Err("Theme name cannot be reserved keyword 'Random'".to_string());
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
                TokenKind::Indicator => {
                    key == "indicator" || key == "selection_indicator" || key == "cursor_indicator"
                }
                TokenKind::DefaultIcon => {
                    key == "default_icon" || key == "icon" || key == "menu_icon"
                }
                TokenKind::DividerChar => key == "divider_char",
                TokenKind::DividerLength => key == "divider_length",
                TokenKind::MarqueeChar => key == "marquee_char",
                TokenKind::MarqueeGap => key == "marquee_gap",
                TokenKind::ToastStyle => key == "toast_style",
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
                            || (k == "highlight_text"
                                && (key == "highlight_fg" || key == "highlight_foreground"))
                            || (k == "background"
                                && (key == "bg" || key == "bg_color" || key == "background_color"))
                            || (k == "window_close_button"
                                && (key == "close_button" || key == "close_btn" || key == "close"))
                            || (k == "title" && key == "header")
                            || (k == "footer" && key == "bottom_bar")
                            || (k == "selection"
                                && (key == "sel" || key == "selection_color" || key == "select"))
                            || (k == "selected_tab"
                                && (key == "active_tab" || key == "selected_tab_color"))
                            || (k == "unselected_tab"
                                && (key == "inactive_tab" || key == "unselected_tab_color"))
                            || (k == "line_numbers"
                                && (key == "line_number"
                                    || key == "linenumbers"
                                    || key == "line_numbers_color"))
                            || (k == "whitespace"
                                && (key == "ws"
                                    || key == "whitespace_color"
                                    || key == "indent_guide"))
                            || (k == "shadow" && (key == "shadow_color" || key == "drop_shadow"))
                            || (k == "modal_yes_button"
                                && (key == "yes_button" || key == "modal_yes"))
                            || (k == "modal_no_button" && (key == "no_button" || key == "modal_no"))
                            || (k == "modal_cancel_button"
                                && (key == "cancel_button" || key == "modal_cancel"))
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
                    TokenKind::ToastStyle => key == "toast_style" || key == "toast_mode",
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

/// Internal tier target used when writing palette overrides uniformly.
#[derive(Clone, Copy)]
enum PaletteTierTarget {
    Color256,
    Color16,
    Color8,
}

/// Mutable view into either a top-level palette map or an activity sub-map.
enum TierMapMut<'a> {
    Top(&'a mut HashMap<String, serde_json::Value>),
    Sub(&'a mut serde_json::Map<String, serde_json::Value>),
}

impl<'a> TierMapMut<'a> {
    fn get(&self, key: &str) -> Option<&serde_json::Value> {
        match self {
            Self::Top(m) => m.get(key),
            Self::Sub(m) => m.get(key),
        }
    }

    fn insert(&mut self, key: &str, val: serde_json::Value) {
        match self {
            Self::Top(m) => {
                m.insert(key.to_string(), val);
            }
            Self::Sub(m) => {
                m.insert(key.to_string(), val);
            }
        }
    }
}

/// Looks up a token's raw YAML value for `activity` inside a tier map.
fn find_tier_token_value<'a>(
    c_map: &'a HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
    key: &str,
) -> Option<&'a serde_json::Value> {
    if activity == ThemeActivity::MainMenu {
        return c_map.get(key);
    }
    if let Some(val) = c_map
        .get(activity.yaml_key())
        .and_then(|v| v.as_object())
        .and_then(|m| m.get(key))
    {
        return Some(val);
    }
    for leg in activity.legacy_yaml_keys() {
        if let Some(val) = c_map
            .get(*leg)
            .and_then(|v| v.as_object())
            .and_then(|m| m.get(key))
        {
            return Some(val);
        }
    }
    None
}

/// Returns `true` when `val` represents `-1` inheritance for `key`.
fn is_token_value_inherited_for_key(key: &str, val: &serde_json::Value) -> bool {
    if is_inherited_color_value(val) {
        return true;
    }
    if key == "highlight" || key == "selection" {
        if let Some(seq) = val.as_array() {
            return seq.get(1).is_none_or(is_inherited_color_value);
        }
    }
    false
}

/// Formats a scalar YAML color value (`i64` or string) for display.
fn format_yaml_scalar_value(val: &serde_json::Value) -> Option<String> {
    if is_inherited_color_value(val) {
        return None;
    }
    if let Some(n) = val.as_i64() {
        return Some(format!("Color {}", n));
    }
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    None
}

/// Extracts the display string for `key` within `activity` in `c_map`.
fn format_yaml_token_display(
    c_map: &HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
    key: &str,
) -> Option<String> {
    if let Some(val) = find_tier_token_value(c_map, activity, key) {
        if let Some(seq) = val.as_array() {
            if key == "highlight" || key == "selection" {
                if let Some(bg_val) = seq.get(1) {
                    if let Some(s) = format_yaml_scalar_value(bg_val) {
                        return Some(s);
                    }
                }
                if key == "highlight" {
                    return None;
                }
            }
            if let Some(fg_val) = seq.first() {
                if let Some(s) = format_yaml_scalar_value(fg_val) {
                    return Some(s);
                }
            }
        } else if let Some(s) = format_yaml_scalar_value(val) {
            return Some(s);
        }
    }
    if key == "highlight_text" {
        if let Some(hl_val) = find_tier_token_value(c_map, activity, "highlight") {
            if let Some(fg_val) = hl_val.as_array().and_then(|s| s.first()) {
                if let Some(s) = format_yaml_scalar_value(fg_val) {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Resolves the display string for `key` under `activity`.
fn resolve_activity_display(theme: &Theme, activity: ThemeActivity, key: &str) -> String {
    if let Some(c_map) = &theme.colors_256 {
        if let Some(disp) = format_yaml_token_display(c_map, activity, key) {
            return disp;
        }
        if activity != ThemeActivity::MainMenu {
            if let Some(disp) = format_yaml_token_display(c_map, ThemeActivity::MainMenu, key) {
                return disp;
            }
        }
    }
    let col = resolve_activity_color(theme, activity, key);
    if col != Color::Reset {
        format!("Color {}", color_to_u8(col))
    } else {
        "Default".to_string()
    }
}

/// Resolves the Ratatui `Color` for `key` under `activity`.
fn resolve_activity_color(theme: &Theme, activity: ThemeActivity, key: &str) -> Color {
    let styles = ResolvedThemeStyles::from_theme_for_activity(theme, true, activity);
    if let Some(c_map) = &theme.colors_256 {
        let check_act = |act: ThemeActivity| -> Option<Color> {
            if key == "highlight_text" {
                if let Some(val) = find_tier_token_value(c_map, act, key) {
                    if !is_inherited_color_value(val) {
                        if let Some(c) = bashmenu_tui::parse_color_value(val) {
                            if c != Color::Reset {
                                return Some(c);
                            }
                        }
                    }
                }
                if let Some(val) = find_tier_token_value(c_map, act, "highlight") {
                    if let Some(fg) = val.as_array().and_then(|s| s.first()) {
                        if !is_inherited_color_value(fg) {
                            if let Some(c) = bashmenu_tui::parse_color_value(fg) {
                                if c != Color::Reset {
                                    return Some(c);
                                }
                            }
                        }
                    }
                }
                return None;
            }
            if key == "highlight" || key == "selection" || key == "selected_tab" {
                if let Some(val) = find_tier_token_value(c_map, act, key) {
                    if let Some(seq) = val.as_array() {
                        if let Some(bg) = seq.get(1) {
                            if !is_inherited_color_value(bg) {
                                if let Some(c) = bashmenu_tui::parse_color_value(bg) {
                                    if c != Color::Reset {
                                        return Some(c);
                                    }
                                }
                            }
                        }
                        if key == "selection" || key == "selected_tab" {
                            if let Some(fg) = seq.first() {
                                if !is_inherited_color_value(fg) {
                                    if let Some(c) = bashmenu_tui::parse_color_value(fg) {
                                        if c != Color::Reset {
                                            return Some(c);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                return None;
            }
            if let Some(val) = find_tier_token_value(c_map, act, key) {
                if !is_inherited_color_value(val) {
                    if let Some(c) = bashmenu_tui::parse_color_value(val) {
                        if c != Color::Reset {
                            return Some(c);
                        }
                    }
                }
            }
            None
        };

        if let Some(c) = check_act(activity) {
            return c;
        }
        if activity != ThemeActivity::MainMenu {
            if let Some(c) = check_act(ThemeActivity::MainMenu) {
                return c;
            }
        }
    }

    match key {
        "background" => styles.background,
        "title" => styles.title.fg.unwrap_or(Color::White),
        "text" => styles.text.fg.unwrap_or(Color::White),
        "highlight" => styles.highlight.bg.unwrap_or(Color::Cyan),
        "highlight_text" => styles.highlight.fg.unwrap_or(Color::Black),
        "selection" => styles
            .selection
            .bg
            .or(styles.selection.fg)
            .unwrap_or(Color::Cyan),
        "selected_tab" => styles
            .selected_tab
            .bg
            .or(styles.selected_tab.fg)
            .unwrap_or(Color::Cyan),
        "unselected_tab" => styles.unselected_tab.fg.unwrap_or(Color::DarkGray),
        "line_numbers" => styles.line_numbers.fg.unwrap_or(Color::DarkGray),
        "whitespace" => styles.whitespace.fg.unwrap_or(Color::DarkGray),
        "border" => styles.border,
        "divider" => styles.divider.fg.unwrap_or(Color::Cyan),
        "accent" => styles.accent.fg.unwrap_or(Color::LightCyan),
        "shortcut_key" => styles.shortcut_key.fg.unwrap_or(Color::Yellow),
        "help_text" => styles.help_text.fg.unwrap_or(Color::Cyan),
        "plugin" => styles.plugin.fg.unwrap_or(Color::Cyan),
        "gutter" => styles.gutter.fg.unwrap_or(Color::DarkGray),
        "footer" => styles.footer.fg.unwrap_or(Color::DarkGray),
        "window_close_button" => styles.window_close_button.fg.unwrap_or(Color::Red),
        "scrollbar" => styles.scrollbar.fg.unwrap_or(Color::Cyan),
        "shadow" => styles.shadow.fg.unwrap_or(Color::DarkGray),
        "modal_yes_button" => styles.modal_yes_button.fg.unwrap_or(Color::LightCyan),
        "modal_no_button" => styles.modal_no_button.fg.unwrap_or(Color::LightRed),
        "modal_cancel_button" => styles.modal_cancel_button.fg.unwrap_or(Color::DarkGray),
        "toast_info" => styles.toast_info,
        "toast_warning" => styles.toast_warning,
        "toast_error" => styles.toast_error,
        _ => Color::White,
    }
}

/// Computes a high-contrast foreground color YAML value for a background color.
fn contrast_fg_value(color: Color, tier: PaletteTierTarget) -> serde_json::Value {
    let (r, g, b) = bashmenu_tui::color_to_rgb(color);
    let lum = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
    match tier {
        PaletteTierTarget::Color256 => {
            serde_json::Value::Number(if lum > 128 { 16 } else { 255 }.into())
        }
        PaletteTierTarget::Color16 | PaletteTierTarget::Color8 => serde_json::Value::String(
            if lum > 128 {
                "COLOR_BLACK"
            } else {
                "COLOR_WHITE"
            }
            .to_string(),
        ),
    }
}

/// Gets or initializes the activity override YAML mapping inside `tier_map`.
fn get_or_init_activity_mapping(
    tier_map: &mut HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
    let key = activity.yaml_key();
    let needs_seed = !matches!(tier_map.get(key), Some(serde_json::Value::Object(_)));
    if needs_seed {
        let mut migrated = None;
        for leg in activity.legacy_yaml_keys() {
            if let Some(val) = tier_map.remove(*leg) {
                if val.is_object() {
                    migrated = Some(val);
                    break;
                }
            }
        }
        let val = migrated.unwrap_or_else(|| {
            let mut m = serde_json::Map::new();
            for tok in bashmenu_core::activity_override_tokens(key) {
                let default_v = if is_scalar_color_token(tok) {
                    serde_json::Value::Number((-1).into())
                } else {
                    serde_json::Value::Array(vec![
                        serde_json::Value::Number((-1).into()),
                        serde_json::Value::Number((-1).into()),
                    ])
                };
                m.insert((*tok).to_string(), default_v);
            }
            serde_json::Value::Object(m)
        });
        tier_map.insert(key.to_string(), val);
    }
    match tier_map.get_mut(key) {
        Some(serde_json::Value::Object(m)) => Some(m),
        _ => None,
    }
}

/// Writes `key` with `raw_val` (downsampled for `tier`) into top-level or activity map.
fn write_color_token_to_tier(
    tier_map: &mut HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
    key: &str,
    color: Color,
    raw_val: &serde_json::Value,
    tier: PaletteTierTarget,
) {
    let val = match tier {
        PaletteTierTarget::Color256 => raw_val.clone(),
        PaletteTierTarget::Color16 => bashmenu_tui::downsample_color_value_to_16(raw_val),
        PaletteTierTarget::Color8 => bashmenu_tui::downsample_color_value_to_8(raw_val),
    };
    let is_override = activity != ThemeActivity::MainMenu;
    let mut target = if is_override {
        match get_or_init_activity_mapping(tier_map, activity) {
            Some(m) => TierMapMut::Sub(m),
            None => return,
        }
    } else {
        TierMapMut::Top(tier_map)
    };

    if key == "background" {
        target.insert(key, val);
    } else if key == "highlight" {
        let existing_fg = target
            .get("highlight_text")
            .filter(|v| !is_inherited_color_value(v))
            .cloned()
            .or_else(|| {
                target
                    .get("highlight")
                    .and_then(|v| v.as_array())
                    .and_then(|seq| seq.first())
                    .filter(|v| !is_inherited_color_value(v))
                    .cloned()
            });
        let fg_val = if let Some(efg) = existing_fg {
            if bashmenu_tui::parse_color_value(&efg).is_some_and(|c| c != Color::Reset) {
                efg
            } else if is_override {
                serde_json::Value::Number((-1).into())
            } else {
                contrast_fg_value(color, tier)
            }
        } else if is_override {
            serde_json::Value::Number((-1).into())
        } else {
            contrast_fg_value(color, tier)
        };
        target.insert(key, serde_json::Value::Array(vec![fg_val, val]));
    } else if key == "selection" || key == "selected_tab" {
        let fg_val = contrast_fg_value(color, tier);
        target.insert(key, serde_json::Value::Array(vec![fg_val, val]));
    } else if key == "highlight_text" {
        if is_override {
            target.insert(key, val);
        } else {
            let existing_bg = target
                .get("highlight")
                .and_then(|v| v.as_array())
                .and_then(|seq| seq.get(1))
                .cloned()
                .unwrap_or_else(|| match tier {
                    PaletteTierTarget::Color256 => serde_json::Value::Number(201.into()),
                    PaletteTierTarget::Color16 | PaletteTierTarget::Color8 => {
                        serde_json::Value::String("COLOR_BLUE".to_string())
                    }
                });
            target.insert(
                "highlight",
                serde_json::Value::Array(vec![val.clone(), existing_bg]),
            );
            target.insert(key, val);
        }
    } else {
        target.insert(
            key,
            serde_json::Value::Array(vec![val, serde_json::Value::Number((-1).into())]),
        );
    }
}

/// Resets an override token inside `tier_map` to `-1` / `[-1, -1]`.
fn reset_tier_token_entry(
    tier_map: &mut HashMap<String, serde_json::Value>,
    activity: ThemeActivity,
    key: &str,
    inherit_val: &serde_json::Value,
) {
    let Some(sub) = get_or_init_activity_mapping(tier_map, activity) else {
        return;
    };
    sub.insert(key.to_string(), inherit_val.clone());
    if key == "highlight_text" {
        if let Some(serde_json::Value::Array(seq)) = sub.get_mut("highlight") {
            if !seq.is_empty() {
                seq[0] = serde_json::Value::Number((-1).into());
            }
        }
    }
}

/// Calculates visual rectangular spans `(y, x_start, x_end)` in the preview pane corresponding
/// to a theme token so the UI can flash them when selected or clicked.
pub fn get_token_preview_spans(
    token: &TokenItem,
    right_area: Rect,
    mode: PreviewMode,
    preview_styles: &ResolvedThemeStyles,
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
            if mode == PreviewMode::Toast {
                let (toast_x, toast_w, boxes) =
                    preview::toast_preview_boxes(right_area, preview_styles.toast_style);
                let is_framed = preview_styles.toast_style == ToastStyle::Framed;
                let end_x = if is_framed {
                    toast_x + toast_w.saturating_sub(1)
                } else {
                    toast_x + toast_w
                };
                for (ty, th) in boxes {
                    let (y_start, y_end) = if is_framed {
                        (ty + 1, ty + th.saturating_sub(1))
                    } else {
                        (ty, ty + th)
                    };
                    for y in y_start..y_end {
                        spans.push((y, toast_x + 1, end_x));
                    }
                }
            } else {
                for y in (right_area.y + 1)..bot_y {
                    spans.push((
                        y,
                        right_area.x + 1,
                        right_area.x + right_area.width.saturating_sub(1),
                    ));
                }
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
        TokenKind::Color(key) if key == "title" => match mode {
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                spans.push((mod_y, mod_x + 2, (mod_x + 27).min(mod_x + mod_w - 4)));
            }
            PreviewMode::MenuEditor => {
                let left_w = p_avail_w / 2;
                spans.push((content_start_y + 1, p_inner_x + 1, p_inner_x + left_w - 1));
            }
            PreviewMode::ThemeEditor => {
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                let right_w = p_avail_w.saturating_sub(left_w + 1);
                let sample_x = p_inner_x + left_w + 1;
                spans.push((
                    content_start_y,
                    p_inner_x + 2,
                    (p_inner_x + 12).min(p_inner_x + left_w - 1),
                ));
                if right_w >= 10 {
                    spans.push((
                        content_start_y,
                        sample_x + 2,
                        (sample_x + 18).min(sample_x + right_w - 1),
                    ));
                    spans.push((
                        content_start_y + 1,
                        sample_x + 2,
                        (sample_x + 15).min(sample_x + right_w - 1),
                    ));
                }
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                spans.push((
                    content_start_y,
                    p_inner_x + 2,
                    (p_inner_x + 24).min(p_inner_x + left_w - 1),
                ));
                if right_w >= 6 {
                    spans.push((
                        content_start_y,
                        insp_x + 2,
                        (insp_x + 24).min(insp_x + right_w - 1),
                    ));
                }
            }
            _ => {
                spans.push((
                    right_area.y,
                    right_area.x + 2,
                    right_area.x + right_area.width.saturating_sub(2),
                ));
            }
        },
        TokenKind::Color(key) if key == "highlight" || key == "highlight_text" => match mode {
            PreviewMode::TextEditor => {
                let max_x = p_inner_x + p_avail_w;
                let status_y = bot_y.saturating_sub(2);
                spans.push((
                    content_start_y + 4,
                    p_inner_x + 7,
                    (p_inner_x + 23).min(max_x),
                ));
                spans.push((status_y, p_inner_x, max_x));
            }
            PreviewMode::MenuEditor => {
                let left_w = p_avail_w / 2;
                spans.push((content_start_y + 3, p_inner_x + 1, p_inner_x + left_w - 1));
            }
            PreviewMode::ThemeEditor => {
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                let right_w = p_avail_w.saturating_sub(left_w + 1);
                let sample_x = p_inner_x + left_w + 1;
                spans.push((content_start_y + 3, p_inner_x + 1, p_inner_x + left_w - 1));
                if right_w >= 10 {
                    spans.push((
                        content_start_y + 2,
                        sample_x + 2,
                        (sample_x + 18).min(sample_x + right_w - 1),
                    ));
                }
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                spans.push((content_start_y + 1, p_inner_x + 1, p_inner_x + left_w - 1));
            }
            _ => {
                if key == "highlight_text" {
                    spans.push((content_start_y, p_inner_x + 2, p_inner_x + 28));
                } else {
                    spans.push((content_start_y, p_inner_x, p_inner_x + p_avail_w));
                }
            }
        },
        TokenKind::Color(key) if key == "selection" => {
            if mode == PreviewMode::TextEditor {
                let max_x = p_inner_x + p_avail_w;
                spans.push((
                    content_start_y + 5,
                    p_inner_x + 24,
                    (p_inner_x + 28).min(max_x),
                ));
            } else {
                spans.push((content_start_y, p_inner_x, p_inner_x + p_avail_w));
            }
        }
        TokenKind::Color(key) if key == "selected_tab" => {
            if let Some(active_tab) = preview_tab_bar_layout(right_area, mode)
                .into_iter()
                .find(|t| t.mode == mode)
            {
                spans.push((right_area.y, active_tab.x_start, active_tab.x_end));
            }
            if mode == PreviewMode::TextEditor {
                spans.push((content_start_y, p_inner_x, p_inner_x + 17));
            }
        }
        TokenKind::Color(key) if key == "unselected_tab" => {
            for tab in preview_tab_bar_layout(right_area, mode) {
                if tab.mode != mode {
                    spans.push((right_area.y, tab.x_start, tab.x_end));
                }
            }
            if mode == PreviewMode::TextEditor {
                spans.push((content_start_y, p_inner_x + 17, p_inner_x + 30));
            }
        }
        TokenKind::Color(key) if key == "line_numbers" => {
            if mode == PreviewMode::TextEditor {
                let rows_end = right_area.y + right_area.height.saturating_sub(3);
                for row in 0..7_u16 {
                    let y = content_start_y + 2 + row;
                    if y >= rows_end {
                        break;
                    }
                    spans.push((y, p_inner_x, p_inner_x + 5));
                }
            }
        }
        TokenKind::Color(key) if key == "whitespace" => {
            if mode == PreviewMode::TextEditor {
                let rows_end = right_area.y + right_area.height.saturating_sub(3);
                for row in [2_u16, 3, 5] {
                    let y = content_start_y + 2 + row;
                    if y < rows_end {
                        spans.push((y, p_inner_x + 5, p_inner_x + 7));
                    }
                }
            } else {
                spans.push((
                    gt_y + 1,
                    sep_x + 1,
                    right_area.x + right_area.width.saturating_sub(2),
                ));
            }
        }
        TokenKind::Color(key) if key == "text" => match mode {
            PreviewMode::TextEditor => {
                let max_x = p_inner_x + p_avail_w;
                let rows_end = right_area.y + right_area.height.saturating_sub(3);
                let text_rows: [(u16, u16, u16); 5] =
                    [(0, 5, 21), (1, 5, 14), (3, 7, 24), (4, 5, 10), (5, 7, 30)];
                for (r, x0, x1) in text_rows {
                    let y = content_start_y + 2 + r;
                    if y < rows_end {
                        spans.push((y, p_inner_x + x0, (p_inner_x + x1).min(max_x)));
                    }
                }
            }
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                spans.push((mod_y + 2, mod_x + 2, (mod_x + 35).min(mod_x + mod_w - 1)));
                spans.push((mod_y + 5, mod_x + 3, mod_x + mod_w.saturating_sub(3)));
            }
            PreviewMode::MenuEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                for r in [2_u16, 4, 5] {
                    spans.push((content_start_y + r, p_inner_x + 1, p_inner_x + left_w - 1));
                }
                if right_w > 2 {
                    for r in [2_u16, 5] {
                        spans.push((content_start_y + r, insp_x + 1, insp_x + right_w - 1));
                    }
                }
            }
            PreviewMode::Toast => {
                let (toast_x, toast_w, boxes) =
                    preview::toast_preview_boxes(right_area, preview_styles.toast_style);
                let is_framed = preview_styles.toast_style == ToastStyle::Framed;
                for (ty, _th) in boxes {
                    let msg_y = if is_framed { ty + 2 } else { ty + 1 };
                    let end_x = if is_framed {
                        toast_x + toast_w.saturating_sub(1)
                    } else {
                        toast_x + toast_w
                    };
                    spans.push((msg_y, toast_x + 2, end_x));
                }
            }
            PreviewMode::ThemeEditor => {
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                for r in [2_u16, 4, 5] {
                    spans.push((content_start_y + r, p_inner_x + 1, p_inner_x + left_w - 1));
                }
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                for r in 2..=4_u16 {
                    let start = (p_inner_x + 10).min(p_inner_x + left_w - 1);
                    spans.push((content_start_y + r, start, p_inner_x + left_w - 1));
                }
                if right_w > 4 {
                    for r in [2_u16, 4] {
                        spans.push((content_start_y + r, insp_x + 2, insp_x + right_w - 2));
                    }
                }
            }
            PreviewMode::Menu => {
                spans.push((content_start_y + 1, p_inner_x + 2, p_inner_x + 22));
                spans.push((content_start_y + 2, p_inner_x + 2, p_inner_x + 28));
                spans.push((content_start_y + 4, p_inner_x + 2, p_inner_x + 24));
            }
        },
        TokenKind::Color(key) if key == "accent" => match mode {
            PreviewMode::TextEditor => {
                let max_x = p_inner_x + p_avail_w;
                spans.push((content_start_y, p_inner_x + 30, (p_inner_x + 35).min(max_x)));
            }
            PreviewMode::MenuEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                if right_w > 2 {
                    spans.push((content_start_y + 1, insp_x + 1, insp_x + right_w - 1));
                    spans.push((content_start_y + 4, insp_x + 1, insp_x + right_w - 1));
                }
            }
            PreviewMode::ThemeEditor => {
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                spans.push((content_start_y + 1, p_inner_x + 1, p_inner_x + left_w - 1));
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                if right_w > 4 {
                    spans.push((content_start_y + 1, insp_x + 2, insp_x + right_w - 2));
                }
            }
            _ => {
                // Accent is the submenu chevron indicator glyph '»' on inactive submenu item 2
                spans.push((content_start_y + 1, p_inner_x + 25, p_inner_x + 28));
            }
        },
        TokenKind::Color(key) if key == "shortcut_key" => match mode {
            PreviewMode::TextEditor => {
                let footer_y = bot_y.saturating_sub(1);
                spans.push((footer_y, p_inner_x, p_inner_x + 3));
                spans.push((footer_y, p_inner_x + 9, p_inner_x + 12));
                spans.push((footer_y, p_inner_x + 18, p_inner_x + 21));
                spans.push((footer_y, p_inner_x + 27, p_inner_x + 30));
            }
            PreviewMode::MenuEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                if right_w > 2 {
                    spans.push((content_start_y + 3, insp_x + 1, insp_x + right_w - 1));
                }
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                let right_w = p_avail_w.saturating_sub(left_w);
                let insp_x = p_inner_x + left_w;
                for r in 2..=4_u16 {
                    spans.push((
                        content_start_y + r,
                        p_inner_x + 1,
                        (p_inner_x + 9).min(p_inner_x + left_w - 1),
                    ));
                }
                if right_w > 4 {
                    spans.push((content_start_y + 3, insp_x + 2, insp_x + right_w - 2));
                }
            }
            _ => {
                // Shortcut keys in inactive menu items (active item shortcut adopts highlight_text)
                spans.push((content_start_y + 1, p_inner_x + 6, p_inner_x + 9));
                spans.push((content_start_y + 2, p_inner_x + 6, p_inner_x + 9));
                spans.push((content_start_y + 4, p_inner_x + 6, p_inner_x + 9));
            }
        },
        TokenKind::Indicator => {
            let ind_w = preview_styles.indicator_symbol.width().max(2) as u16;
            spans.push((content_start_y, p_inner_x, p_inner_x + ind_w));
        }
        TokenKind::DefaultIcon => {
            spans.push((content_start_y + 2, p_inner_x + 2, p_inner_x + 6));
        }
        TokenKind::Color(key) if key == "divider" => match mode {
            PreviewMode::TextEditor => {
                spans.push((content_start_y + 1, p_inner_x, p_inner_x + p_avail_w));
            }
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                spans.push((
                    mod_y + mod_h.saturating_sub(3),
                    mod_x + 1,
                    mod_x + mod_w - 1,
                ));
            }
            PreviewMode::ThemeEditor => {
                let div_y = bot_y.saturating_sub(2);
                spans.push((div_y, p_inner_x, p_inner_x + p_avail_w));
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                let right_w = p_avail_w.saturating_sub(left_w + 1);
                if right_w >= 10 {
                    let sx = p_inner_x + left_w + 3;
                    spans.push((content_start_y + 3, sx, (sx + 8).min(p_inner_x + p_avail_w)));
                }
            }
            PreviewMode::ShortcutEditor => {
                let div_y = bot_y.saturating_sub(2);
                spans.push((div_y, p_inner_x, p_inner_x + p_avail_w));
            }
            _ => {
                spans.push((content_start_y + 3, p_inner_x, p_inner_x + p_avail_w));
                if content_avail_h >= 8 {
                    spans.push((plugin_start_y, p_inner_x, p_inner_x + p_avail_w));
                }
            }
        },
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
        TokenKind::Color(key) if key == "help_text" => match mode {
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                spans.push((mod_y + 3, mod_x + 2, (mod_x + 36).min(mod_x + mod_w - 1)));
                spans.push((
                    mod_y + mod_h.saturating_sub(2),
                    mod_x + 2,
                    mod_x + mod_w.saturating_sub(12),
                ));
            }
            PreviewMode::TextEditor
            | PreviewMode::MenuEditor
            | PreviewMode::ThemeEditor
            | PreviewMode::ShortcutEditor => {
                let footer_y = bot_y.saturating_sub(1);
                spans.push((footer_y, p_inner_x, p_inner_x + p_avail_w));
            }
            _ => {
                spans.push((gt_y + 1, right_area.x + 2, sep_x));
            }
        },
        TokenKind::Color(key) if key == "plugin" => {
            if content_avail_h >= 8 {
                spans.push((plugin_start_y + 1, p_inner_x, p_inner_x + p_avail_w));
                spans.push((plugin_start_y + 2, p_inner_x, p_inner_x + p_avail_w));
            } else if content_avail_h >= 6 {
                spans.push((gt_y.saturating_sub(1), p_inner_x, p_inner_x + p_avail_w));
            }
        }
        TokenKind::Color(key) if key == "gutter" => match mode {
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                spans.push((
                    mod_y + mod_h.saturating_sub(2),
                    mod_x + mod_w.saturating_sub(12),
                    mod_x + mod_w - 1,
                ));
            }
            _ => {
                spans.push((
                    gt_y + 1,
                    sep_x + 1,
                    right_area.x + right_area.width.saturating_sub(2),
                ));
            }
        },
        TokenKind::Color(key) if key == "footer" => {
            spans.push((
                bot_y,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ));
        }
        TokenKind::Color(key) if key == "window_close_button" => {
            if mode == PreviewMode::Modals {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                let close_x = mod_x + mod_w.saturating_sub(4);
                spans.push((mod_y, close_x, close_x + 3));
            } else {
                let close_x = right_area.x + right_area.width.saturating_sub(4);
                spans.push((right_area.y, close_x, close_x + 3));
            }
        }
        TokenKind::Color(key) if key == "scrollbar" => match mode {
            PreviewMode::Modals => {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                for y in (mod_y + 3)..=(mod_y + 5) {
                    spans.push((y, mod_x + mod_w - 1, mod_x + mod_w));
                }
            }
            PreviewMode::ThemeEditor => {
                let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
                spans.push((
                    content_start_y + 2,
                    p_inner_x + left_w - 1,
                    p_inner_x + left_w,
                ));
            }
            PreviewMode::ShortcutEditor => {
                let left_w = p_avail_w / 2;
                spans.push((
                    content_start_y + 1,
                    p_inner_x + left_w - 1,
                    p_inner_x + left_w,
                ));
            }
            _ => {
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
        },
        TokenKind::Color(key)
            if key == "modal_yes_button"
                || key == "modal_no_button"
                || key == "modal_cancel_button" =>
        {
            if mode == PreviewMode::Modals {
                let p_avail_h = right_area.height.saturating_sub(4);
                let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                let mod_h = 12_u16.min(p_avail_h);
                let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
                let btn_y = mod_y + 7;
                if key == "modal_yes_button" {
                    spans.push((btn_y, mod_x + 3, mod_x + 10));
                } else if key == "modal_no_button" {
                    spans.push((btn_y, mod_x + 12, mod_x + 18));
                } else {
                    spans.push((btn_y, mod_x + 20, mod_x + 30));
                }
            }
        }
        TokenKind::Color(key) if key == "shadow" => {
            let p_avail_h = right_area.height.saturating_sub(4);
            let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
            let mod_h = 12_u16.min(p_avail_h);
            let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
            let mod_y = content_start_y + (p_avail_h.saturating_sub(mod_h)) / 2;
            let max_x = right_area.x + right_area.width.saturating_sub(1);
            let max_y = right_area.y + right_area.height.saturating_sub(1);
            for y in (mod_y + 1)..(mod_y + mod_h + 1) {
                if y < max_y && mod_x + mod_w < max_x {
                    spans.push((y, mod_x + mod_w, (mod_x + mod_w + 2).min(max_x)));
                }
            }
            let bottom_y = mod_y + mod_h;
            if bottom_y < max_y && mod_x + 2 < max_x {
                spans.push((bottom_y, mod_x + 2, (mod_x + mod_w + 2).min(max_x)));
            }
        }
        TokenKind::ToastStyle => {
            let (toast_x, toast_w, boxes) =
                preview::toast_preview_boxes(right_area, preview_styles.toast_style);
            for (ty, th) in boxes {
                for y in ty..(ty + th) {
                    spans.push((y, toast_x, toast_x + toast_w));
                }
            }
        }
        TokenKind::ToastChar => {
            let (toast_x, _toast_w, boxes) =
                preview::toast_preview_boxes(right_area, preview_styles.toast_style);
            for (ty, th) in boxes {
                for y in ty..(ty + th) {
                    spans.push((y, toast_x, toast_x + 2));
                }
            }
        }
        TokenKind::Color(key) if key.starts_with("toast_") => {
            let (toast_x, toast_w, boxes) =
                preview::toast_preview_boxes(right_area, preview_styles.toast_style);
            for (ty, th) in boxes {
                for y in ty..(ty + th) {
                    spans.push((y, toast_x, toast_x + toast_w));
                }
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

/// Renders the root window frame and background fill for the Theme Editor.
///
/// Fills all cells in `area` with `styles.background` and renders the outer
/// border block styled with the theme's background color.
pub fn render_root_frame(
    area: Rect,
    buf: &mut Buffer,
    theme_name: &str,
    is_modified: bool,
    styles: &ResolvedThemeStyles,
) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(styles.background);
        }
    }

    let border_style = Style::default().fg(styles.border);
    let root_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::styled("─", border_style),
            Span::styled(
                format!(
                    "[ ThemeEdit: {}{} ]",
                    theme_name,
                    if is_modified { " (Modified)" } else { "" }
                ),
                styles.title,
            ),
        ]))
        .border_style(border_style)
        .style(Style::default().bg(styles.background));
    root_block.render(area, buf);
}
