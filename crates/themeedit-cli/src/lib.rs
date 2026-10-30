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

pub struct ThemeEditorState {
    pub themes: ThemeCollection,
    pub themes_path: PathBuf,
    pub current_name: String,
    pub current_theme: Theme,
    pub is_modified: bool,
    pub tokens: Vec<TokenItem>,
    pub selected_idx: usize,
    pub status_message: String,
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
            .or_else(|| themes.keys().next().cloned())
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
