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

/// Theme toast notification configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToastConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

fn default_true() -> bool {
    true
}

fn default_opacity() -> f32 {
    0.35
}

fn default_scaling() -> String {
    "cover".to_string()
}

fn default_quantization() -> usize {
    1
}

/// Configuration for background wallpapers (theme or user settings)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WallpaperConfig {
    /// Master switch for background wallpaper compositing
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Image path (e.g. assets/wallpapers/alaska-mountain.png or {bashmenu_dir}/...)
    #[serde(default)]
    pub image: Option<String>,

    /// Opacity factor between 0.0 and 1.0 (blends wallpaper with theme background)
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    /// Scaling mode: "cover", "stretch", "fit", "center", "tile"
    #[serde(default = "default_scaling")]
    pub scaling: String,

    /// Horizontal delta clustering threshold (0..128)
    #[serde(default)]
    pub clustering: usize,

    /// Color channel quantization (>= 1)
    #[serde(default = "default_quantization")]
    pub quantization: usize,

    /// Rendering mode (e.g. "halfblock", "quarterblock")
    #[serde(default)]
    pub mode: Option<String>,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            image: None,
            opacity: 0.35,
            scaling: "cover".to_string(),
            clustering: 0,
            quantization: 1,
            mode: Some("halfblock".to_string()),
        }
    }
}

/// Wallpaper configuration setting representation in YAML (string path or full config map)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WallpaperSetting {
    /// Direct string path to image
    Simple(String),
    /// Structured configuration map
    Config(WallpaperConfig),
}

impl WallpaperSetting {
    /// Convert setting to concrete `WallpaperConfig`
    pub fn to_config(&self) -> WallpaperConfig {
        match self {
            WallpaperSetting::Simple(path) => WallpaperConfig {
                enabled: true,
                image: Some(path.clone()),
                ..WallpaperConfig::default()
            },
            WallpaperSetting::Config(cfg) => cfg.clone(),
        }
    }
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

    /// Toast notification settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toast: Option<ToastConfig>,

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
    pub background: Option<WallpaperSetting>,

    /// Additional arbitrary theme fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

impl Theme {
    /// Resolves wallpaper configuration from `background`, `theme_background`, or `wallpaper`
    pub fn wallpaper_config(&self) -> Option<WallpaperConfig> {
        if let Some(bg) = &self.background {
            return Some(bg.to_config());
        }
        if let Some(val) = self
            .extra
            .get("theme_background")
            .or_else(|| self.extra.get("wallpaper"))
        {
            if let Ok(setting) = serde_yml::from_value::<WallpaperSetting>(val.clone()) {
                return Some(setting.to_config());
            }
        }
        None
    }

    /// Creates a base monochrome theme with clean single-line borders,
    /// white-on-black/transparent text, and standard indicators.
    pub fn monochrome() -> Self {
        let mut t = Self::default();
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
        t.divider = Some(DividerConfig {
            char: Some("─".to_string()),
            length: Some("{window_width}".to_string()),
        });
        t.marquee = Some(MarqueeConfig {
            char: "•".to_string(),
            gap: 4,
        });
        t.toast = Some(ToastConfig {
            char: Some("▌".to_string()),
            timeout: Some(2.0),
        });

        let mut colors = HashMap::new();
        let make_pair = |fg: i64, bg: i64| -> serde_yml::Value {
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(fg.into()),
                serde_yml::Value::Number(bg.into()),
            ])
        };
        colors.insert("background".to_string(), serde_yml::Value::Number(0.into()));
        colors.insert("text".to_string(), make_pair(255, -1));
        colors.insert("highlight".to_string(), make_pair(0, 255));
        colors.insert("border".to_string(), make_pair(250, -1));
        colors.insert("divider".to_string(), make_pair(245, -1));
        colors.insert("accent".to_string(), make_pair(255, -1));
        colors.insert("shortcut_key".to_string(), make_pair(255, -1));
        colors.insert("gutter".to_string(), make_pair(242, -1));
        colors.insert("help_text".to_string(), make_pair(250, -1));
        colors.insert("plugin".to_string(), make_pair(250, -1));
        colors.insert("scrollbar".to_string(), make_pair(245, -1));
        colors.insert("toast_info".to_string(), make_pair(255, -1));
        colors.insert("toast_warning".to_string(), make_pair(250, -1));
        colors.insert("toast_error".to_string(), make_pair(245, -1));
        t.colors_256 = Some(colors);
        t
    }
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

/// Saves theme collection to disk in alphabetical order.
pub fn save_themes<P: AsRef<Path>>(path: P, themes: &ThemeCollection) -> Result<()> {
    let p = path.as_ref();
    let mut entries: Vec<(&String, &Theme)> = themes.iter().collect();
    entries.sort_by(|(a, _), (b, _)| {
        a.to_ascii_lowercase()
            .cmp(&b.to_ascii_lowercase())
            .then_with(|| a.cmp(b))
    });
    let mut map = serde_yml::Mapping::new();
    for (k, v) in entries {
        map.insert(serde_yml::Value::String(k.clone()), serde_yml::to_value(v)?);
    }
    let yaml_body = serde_yml::to_string(&map)?.replace(": =\n", ": '='\n");
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
