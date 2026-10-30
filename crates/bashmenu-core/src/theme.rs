//! Theme definitions and palette mapping for `bashmenu.themes`.

use crate::error::{BashMenuError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Theme marquee scroll configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarqueeConfig {
    /// Separator character or string between scroll cycles
    #[serde(default = "default_marquee_char")]
    pub char: String,
    /// Number of separator repetitions
    #[serde(default = "default_marquee_gap")]
    pub gap: usize,
}

fn default_marquee_char() -> String {
    "•".to_string()
}

fn default_marquee_gap() -> usize {
    5
}

impl Default for MarqueeConfig {
    fn default() -> Self {
        Self {
            char: default_marquee_char(),
            gap: default_marquee_gap(),
        }
    }
}

/// Window border and box drawing character definitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WindowBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal_top: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal_bottom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical_left: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical_right: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_left: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_right: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_left: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_right: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_left_cap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_right_cap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_left_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_right_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_top_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_bottom_tee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_char: Option<String>,
}

/// Theme divider configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DividerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
}

/// A single theme palette and border definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Theme {
    /// Marquee scroll configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marquee: Option<MarqueeConfig>,

    /// Selection indicator symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indicator: Option<String>,

    /// Window borders and frame glyphs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowBorders>,

    /// Divider styling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divider: Option<DividerConfig>,

    /// 256-color palette tier mapping token names to [fg, bg] or color identifiers
    #[serde(rename = "256", skip_serializing_if = "Option::is_none")]
    pub colors_256: Option<HashMap<String, serde_yml::Value>>,

    /// 16-color ANSI fallback palette tier
    #[serde(rename = "16", skip_serializing_if = "Option::is_none")]
    pub colors_16: Option<HashMap<String, serde_yml::Value>>,

    /// 8-color ANSI fallback palette tier
    #[serde(rename = "8", skip_serializing_if = "Option::is_none")]
    pub colors_8: Option<HashMap<String, serde_yml::Value>>,

    /// Background wallpaper configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<serde_yml::Value>,

    /// Additional arbitrary theme fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

/// Type alias for all loaded themes mapped by theme name
pub type ThemeCollection = HashMap<String, Theme>;

/// Loads and parses theme definitions from `bashmenu.themes`.
pub fn load_themes<P: AsRef<Path>>(path: P) -> Result<ThemeCollection> {
    let p = path.as_ref();
    let content = fs::read_to_string(p).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    let themes: ThemeCollection = serde_yml::from_str(&content)?;
    Ok(themes)
}

/// Saves theme collection to disk.
pub fn save_themes<P: AsRef<Path>>(path: P, themes: &ThemeCollection) -> Result<()> {
    let p = path.as_ref();
    let yaml_body = serde_yml::to_string(themes)?;
    fs::write(p, yaml_body).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_themes_fixture() {
        let fixture_path = Path::new("../../tests/fixtures/bashmenu.themes");
        if fixture_path.exists() {
            let themes = load_themes(fixture_path).expect("Failed to load bashmenu.themes");
            assert!(themes.contains_key("dracula"), "dracula theme must exist");
            assert!(
                themes.contains_key("alaska-mountain"),
                "alaska-mountain theme must exist"
            );

            let dracula = &themes["dracula"];
            assert_eq!(dracula.marquee.as_ref().map(|m| m.gap), Some(4));
            assert!(dracula.colors_256.is_some());
            assert!(dracula.window.is_some());
        }
    }
}
