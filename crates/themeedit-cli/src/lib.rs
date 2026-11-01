//! Library module for ThemeEdit core state, actions, and theme manipulation.

use anyhow::Result;
use bashmenu_core::{
    load_themes, save_themes, DividerConfig, MarqueeConfig, Theme, ThemeCollection,
    WallpaperSetting, WindowBorders,
};
use bashmenu_tui::ResolvedThemeStyles;
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Color(String),
    Border(String),
    Indicator,
    DividerChar,
    DividerLength,
    MarqueeChar,
    MarqueeGap,
    ToastChar,
    ToastTimeout,
    WallpaperImage,
}

#[derive(Debug, Clone)]
pub struct TokenItem {
    pub label: String,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    #[default]
    Menu,
    TextEditor,
    Modals,
    MenuEditor,
    Toast,
}

impl PreviewMode {
    pub fn next(self) -> Self {
        match self {
            PreviewMode::Menu => PreviewMode::TextEditor,
            PreviewMode::TextEditor => PreviewMode::Modals,
            PreviewMode::Modals => PreviewMode::MenuEditor,
            PreviewMode::MenuEditor => PreviewMode::Toast,
            PreviewMode::Toast => PreviewMode::Menu,
        }
    }

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

pub struct ThemeEditorState {
    pub themes: ThemeCollection,
    pub themes_path: PathBuf,
    pub current_name: String,
    pub current_theme: Theme,
    pub is_modified: bool,
    pub tokens: Vec<TokenItem>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub status_message: String,
    pub preview_mode: PreviewMode,
}

impl ThemeEditorState {
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
                    sorted.sort();
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
                label: "Toast Default Timeout".to_string(),
                kind: TokenKind::ToastTimeout,
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

        Self {
            themes,
            themes_path,
            current_name: initial_name,
            current_theme,
            is_modified: false,
            tokens,
            selected_idx: 0,
            scroll_offset: 0,
            status_message: "Ready".to_string(),
            preview_mode: PreviewMode::default(),
        }
    }

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
        t.colors_256 = Some(colors);
        t
    }

    pub fn get_token_value_display(&self, token: &TokenItem) -> String {
        match &token.kind {
            TokenKind::Color(key) => {
                if let Some(c_map) = &self.current_theme.colors_256 {
                    if let Some(val) = c_map.get(key) {
                        if let Some(seq) = val.as_sequence() {
                            if let Some(fg) = seq.first().and_then(|v| v.as_i64()) {
                                return format!("Color {}", fg);
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
            TokenKind::ToastTimeout => {
                let t = self
                    .current_theme
                    .toast
                    .as_ref()
                    .and_then(|t| t.timeout)
                    .unwrap_or(2.0);
                format!("{:.1}s", t)
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
        }
    }

    pub fn set_color_token(&mut self, key: &str, color_idx: u8) {
        if self.current_theme.colors_256.is_none() {
            self.current_theme.colors_256 = Some(HashMap::new());
        }
        if let Some(c_map) = &mut self.current_theme.colors_256 {
            if key == "background" {
                c_map.insert(
                    key.to_string(),
                    serde_yml::Value::Number((color_idx as i64).into()),
                );
            } else {
                c_map.insert(
                    key.to_string(),
                    serde_yml::Value::Sequence(vec![
                        serde_yml::Value::Number((color_idx as i64).into()),
                        serde_yml::Value::Number((-1).into()),
                    ]),
                );
            }
            self.is_modified = true;
            self.status_message = format!("Updated {} to color {}", key, color_idx);
        }
    }

    pub fn set_wallpaper_image(&mut self, path: Option<String>) {
        match path {
            Some(p) => {
                let mut cfg = self.current_theme.wallpaper_config().unwrap_or_default();
                cfg.image = Some(p.clone());
                cfg.enabled = true;
                self.current_theme.background = Some(WallpaperSetting::Config(cfg));
                self.is_modified = true;
                self.status_message = format!("Set wallpaper image to {}", p);
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
            }
        }
        255
    }

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

    pub fn cycle_toast_timeout(&mut self) {
        if self.current_theme.toast.is_none() {
            self.current_theme.toast = Some(bashmenu_core::ToastConfig::default());
        }
        if let Some(t) = &mut self.current_theme.toast {
            let timeouts = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0];
            let curr = t.timeout.unwrap_or(2.0);
            let next_t = match timeouts.iter().position(|&v| (v - curr).abs() < 0.05) {
                Some(idx) => timeouts[(idx + 1) % timeouts.len()],
                None => 2.0,
            };
            t.timeout = Some(next_t);
            self.is_modified = true;
            self.status_message = format!("Updated toast timeout to {:.1}s", next_t);
        }
    }

    pub fn adjust_scroll(&mut self, viewport_h: usize) {
        if viewport_h == 0 {
            return;
        }
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + viewport_h {
            self.scroll_offset = self.selected_idx.saturating_sub(viewport_h - 1);
        }
    }

    pub fn save_to_disk(&mut self) -> Result<()> {
        self.themes
            .insert(self.current_name.clone(), self.current_theme.clone());
        save_themes(&self.themes_path, &self.themes)?;
        self.is_modified = false;
        self.status_message = format!("Saved theme '{}' directly to disk", self.current_name);
        Ok(())
    }

    /// Focuses/selects the token corresponding to the given style or border key
    pub fn select_token_by_key(&mut self, key: &str) -> bool {
        // 1. Exact match first
        if let Some(idx) = self.tokens.iter().position(|t| match &t.kind {
            TokenKind::Color(k) => k == key,
            TokenKind::Border(k) => k == key,
            TokenKind::Indicator => key == "indicator",
            TokenKind::DividerChar => key == "divider_char",
            TokenKind::DividerLength => key == "divider_length",
            TokenKind::MarqueeChar => key == "marquee_char",
            TokenKind::MarqueeGap => key == "marquee_gap",
            TokenKind::ToastChar => key == "toast_char",
            TokenKind::ToastTimeout => key == "toast_timeout",
            TokenKind::WallpaperImage => key == "wallpaper" || key == "wallpaper_image",
        }) {
            self.selected_idx = idx;
            self.status_message.clear();
            return true;
        }

        // 2. Fallback aliases
        if let Some(idx) = self.tokens.iter().position(|t| match &t.kind {
            TokenKind::Color(k) => {
                (k == "shortcut_key" && (key == "shortcut_gutter" || key == "shortcut"))
                    || (k == "text" && key == "content")
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
            }
            TokenKind::DividerChar => key == "divider",
            TokenKind::MarqueeChar => key == "marquee",
            TokenKind::ToastChar => key == "toast_char" || key == "toast_bar" || key == "toast",
            TokenKind::ToastTimeout => key == "toast_timeout" || key == "toast_duration",
            TokenKind::WallpaperImage => {
                key == "wallpaper" || key == "wallpaper_image" || key == "bg_image"
            }
            _ => false,
        }) {
            self.selected_idx = idx;
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
        TokenKind::WallpaperImage => {
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
            spans.push((content_start_y + 1, p_inner_x, p_inner_x + p_avail_w));
        }
        TokenKind::Color(key) if key == "text" => {
            spans.push((
                content_start_y,
                p_inner_x + 6,
                right_area.x + right_area.width.saturating_sub(2),
            ));
            spans.push((
                content_start_y + 2,
                p_inner_x + 6,
                right_area.x + right_area.width.saturating_sub(18),
            ));
            spans.push((
                content_start_y + 4,
                p_inner_x + 6,
                right_area.x + right_area.width.saturating_sub(2),
            ));
        }
        TokenKind::Color(key) if key == "accent" => {
            if content_avail_h >= 8 {
                spans.push((plugin_start_y + 1, p_inner_x, p_inner_x + p_avail_w));
            }
            spans.push((
                content_start_y + 2,
                right_area.x + right_area.width.saturating_sub(18),
                right_area.x + right_area.width.saturating_sub(2),
            ));
        }
        TokenKind::Color(key) if key == "shortcut_key" => {
            spans.push((content_start_y, p_inner_x + 7, p_inner_x + 10));
            spans.push((content_start_y + 1, p_inner_x + 7, p_inner_x + 10));
            spans.push((content_start_y + 2, p_inner_x + 7, p_inner_x + 10));
            spans.push((content_start_y + 4, p_inner_x + 7, p_inner_x + 10));
        }
        TokenKind::Indicator => {
            spans.push((content_start_y + 1, p_inner_x + 2, p_inner_x + 7));
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
        TokenKind::ToastChar | TokenKind::ToastTimeout => {
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
