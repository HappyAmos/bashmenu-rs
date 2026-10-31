//! Library module for ThemeEdit core state, actions, and theme manipulation.

use anyhow::Result;
use bashmenu_core::{load_themes, save_themes, Theme, ThemeCollection, WindowBorders};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Color(String),
    Border(String),
    Indicator,
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
}

impl PreviewMode {
    pub fn next(self) -> Self {
        match self {
            PreviewMode::Menu => PreviewMode::TextEditor,
            PreviewMode::TextEditor => PreviewMode::Modals,
            PreviewMode::Modals => PreviewMode::MenuEditor,
            PreviewMode::MenuEditor => PreviewMode::Menu,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PreviewMode::Menu => "Main Menu",
            PreviewMode::TextEditor => "Text Editor",
            PreviewMode::Modals => "Popup Windows / Modals",
            PreviewMode::MenuEditor => "Visual Menu Editor",
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
                label: "Shortcut Color".to_string(),
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
            // Indicator
            TokenItem {
                label: "Cursor Indicator".to_string(),
                kind: TokenKind::Indicator,
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
        ];

        Self {
            themes,
            themes_path,
            current_name: initial_name,
            current_theme,
            is_modified: false,
            tokens,
            selected_idx: 0,
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
            TokenKind::Indicator => self
                .current_theme
                .indicator
                .as_deref()
                .unwrap_or("> ")
                .to_string(),
        }
    }

    pub fn set_color_token(&mut self, key: &str, color_idx: u8) {
        if self.current_theme.colors_256.is_none() {
            self.current_theme.colors_256 = Some(HashMap::new());
        }
        if let Some(c_map) = &mut self.current_theme.colors_256 {
            c_map.insert(
                key.to_string(),
                serde_yml::Value::Sequence(vec![
                    serde_yml::Value::Number((color_idx as i64).into()),
                    serde_yml::Value::Number((-1).into()),
                ]),
            );
            self.is_modified = true;
            self.status_message = format!("Updated {} to color {}", key, color_idx);
        }
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

    pub fn save_to_disk(&mut self) -> Result<()> {
        self.themes
            .insert(self.current_name.clone(), self.current_theme.clone());
        save_themes(&self.themes_path, &self.themes)?;
        self.is_modified = false;
        self.status_message = format!("Saved theme '{}' directly to disk", self.current_name);
        Ok(())
    }
}
