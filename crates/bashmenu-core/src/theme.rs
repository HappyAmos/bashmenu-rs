//! Theme definitions and palette mapping for `bashmenu.themes`.

use crate::error::{BashMenuError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Canonical activity override section keys stored inside a theme palette.
pub const ACTIVITY_OVERRIDE_KEYS: [&str; 6] = [
    "bashedit",
    "modals",
    "menuedit",
    "toasts",
    "themeedit",
    "shortcutedit",
];

/// Returns `true` if `token` stores a single scalar color rather than a `[fg, bg]` pair.
pub fn is_scalar_color_token(token: &str) -> bool {
    matches!(token, "background" | "highlight_text")
}

/// Returns the supported color override token keys for a given activity section name.
pub fn activity_override_tokens(activity_key: &str) -> &'static [&'static str] {
    match activity_key {
        "bashedit" => &[
            "background",
            "title",
            "text",
            "border",
            "divider",
            "accent",
            "selection",
            "selected_tab",
            "unselected_tab",
            "line_numbers",
            "whitespace",
            "gutter",
            "help_text",
            "footer",
            "shortcut_key",
            "scrollbar",
            "window_close_button",
        ],
        "modals" => &[
            "background",
            "title",
            "text",
            "highlight",
            "highlight_text",
            "border",
            "divider",
            "accent",
            "selection",
            "shortcut_key",
            "help_text",
            "gutter",
            "footer",
            "scrollbar",
            "shadow",
            "modal_yes_button",
            "modal_no_button",
            "modal_cancel_button",
            "window_close_button",
        ],
        "menuedit" | "shortcutedit" => &[
            "background",
            "title",
            "text",
            "highlight",
            "highlight_text",
            "border",
            "divider",
            "accent",
            "selection",
            "shortcut_key",
            "help_text",
            "gutter",
            "footer",
            "scrollbar",
            "window_close_button",
        ],
        "themeedit" => &[
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
            "unselected_tab",
            "shortcut_key",
            "help_text",
            "gutter",
            "footer",
            "scrollbar",
            "window_close_button",
        ],
        "toasts" => &[
            "background",
            "title",
            "text",
            "gutter",
            "help_text",
            "toast_info",
            "toast_warning",
            "toast_error",
        ],
        _ => &[],
    }
}

/// Builds default `-1` (inherit from base theme) activity override sub-maps for
/// `bashedit`, `modals`, `menuedit`, `toasts`, `themeedit`, and `shortcutedit`.
pub fn default_activity_overrides_map() -> HashMap<String, serde_json::Value> {
    let inherit_val = || serde_json::Value::Number((-1).into());
    let inherit_pair = || {
        serde_json::Value::Array(vec![
            serde_json::Value::Number((-1).into()),
            serde_json::Value::Number((-1).into()),
        ])
    };

    let mut overrides = HashMap::new();
    for &activity in &ACTIVITY_OVERRIDE_KEYS {
        let mut sub_map = serde_json::Map::new();
        for &tok in activity_override_tokens(activity) {
            let val = if is_scalar_color_token(tok) {
                inherit_val()
            } else {
                inherit_pair()
            };
            sub_map.insert(tok.to_string(), val);
        }
        overrides.insert(activity.to_string(), serde_json::Value::Object(sub_map));
    }
    overrides
}

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

/// Default separator glyph for marquee scrolling (`"•"`).
fn default_marquee_char() -> String {
    "•".to_string()
}

/// Default spacing count between marquee repetitions (5).
fn default_marquee_gap() -> usize {
    5
}

impl Default for MarqueeConfig {
    /// Returns default marquee configuration with bullet separator and 5-column gap.
    fn default() -> Self {
        Self {
            char: default_marquee_char(),
            gap: default_marquee_gap(),
        }
    }
}

/// Window border and box drawing character definitions.
///
/// Each optional field allows overriding specific border characters (such as
/// single-line, double-line, or rounded corners) for window frames, gutters, and tees.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WindowBorders {
    /// Horizontal border character (e.g., '─' or '═')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal: Option<String>,
    /// Vertical border character (e.g., '│' or '║')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical: Option<String>,
    /// Top horizontal border override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal_top: Option<String>,
    /// Bottom horizontal border override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_horizontal_bottom: Option<String>,
    /// Left vertical border override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical_left: Option<String>,
    /// Right vertical border override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_vertical_right: Option<String>,
    /// Top-left corner character (e.g., '┌')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_left: Option<String>,
    /// Top-right corner character (e.g., '┐')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_right: Option<String>,
    /// Bottom-left corner character (e.g., '└')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_left: Option<String>,
    /// Bottom-right corner character (e.g., '┘')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_right: Option<String>,
    /// Left T-junction connector ('├')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_tee: Option<String>,
    /// Right T-junction connector ('┤')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_tee: Option<String>,
    /// Top T-junction connector ('┬')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_tee: Option<String>,
    /// Bottom T-junction connector ('┴')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom_tee: Option<String>,
    /// Left title decoration cap bracket (e.g., '[')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_left_cap: Option<String>,
    /// Right title decoration cap bracket (e.g., ']')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_right_cap: Option<String>,
    /// Horizontal border character for status/help gutter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_border: Option<String>,
    /// Vertical separator character within gutter columns
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_separator: Option<String>,
    /// Left T-junction connector connecting gutter border to window frame
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_left_tee: Option<String>,
    /// Right T-junction connector connecting gutter border to window frame
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_right_tee: Option<String>,
    /// Top T-junction connector on gutter border
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_top_tee: Option<String>,
    /// Bottom T-junction connector on gutter border
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter_bottom_tee: Option<String>,
    /// Shadow character used for modal backdrop effects (e.g., '▒' or '░')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_char: Option<String>,
}

/// Theme divider configuration for separating menu sections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DividerConfig {
    /// Character repeated across the divider rule (e.g., '─' or '=')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    /// Length of divider: either a fixed number or dynamic macro like `"{window_width}"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
}

/// Visual layout style for toast notification popups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastStyle {
    /// Classic toast with solid or glyph vertical left accent bar.
    #[default]
    Bar,
    /// Framed toast with double left border and single top/right/bottom frame.
    Framed,
}

impl ToastStyle {
    /// Parses a toast style name (defaults to `ToastStyle::Bar`).
    pub fn from_str_name(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "framed" | "box" | "border" => Self::Framed,
            _ => Self::Bar,
        }
    }

    /// Returns canonical lowercase YAML string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Framed => "framed",
        }
    }

    /// Returns user-facing display label for theme editor UI.
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Framed => "Framed",
        }
    }

    /// Toggles between `Bar` and `Framed` toast styles.
    pub fn toggle(self) -> Self {
        match self {
            Self::Bar => Self::Framed,
            Self::Framed => Self::Bar,
        }
    }
}

/// Theme toast notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToastConfig {
    /// Visual layout style (`"bar"` or `"framed"`, defaults to `"bar"`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Left accent indicator glyph (defaults to left half block `▌`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    /// Optional legacy theme display duration in seconds before auto-dismissal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

/// Default boolean true for serde field defaults.
fn default_true() -> bool {
    true
}

/// Default wallpaper blending opacity (0.35).
fn default_opacity() -> f32 {
    0.35
}

/// Default wallpaper scaling mode (`"cover"`).
fn default_scaling() -> String {
    "cover".to_string()
}

/// Default wallpaper color quantization step (1).
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
    /// Returns default wallpaper configuration enabled with 0.35 opacity and cover scaling.
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

    /// Default menu item icon for items without an explicitly configured icon
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_icon: Option<String>,

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
    pub colors_256: Option<HashMap<String, serde_json::Value>>,

    /// 16-color ANSI fallback palette tier
    #[serde(rename = "16", skip_serializing_if = "Option::is_none")]
    pub colors_16: Option<HashMap<String, serde_json::Value>>,

    /// 8-color ANSI fallback palette tier
    #[serde(rename = "8", skip_serializing_if = "Option::is_none")]
    pub colors_8: Option<HashMap<String, serde_json::Value>>,

    /// Background wallpaper configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<WallpaperSetting>,

    /// Additional arbitrary theme fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
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
            if let Ok(setting) = serde_json::from_value::<WallpaperSetting>(val.clone()) {
                return Some(setting.to_config());
            }
        }
        None
    }

    /// Resolves the default icon for menu items, falling back to
    /// `DEFAULT_ITEM_ICON`.
    pub fn resolved_default_icon(&self) -> &str {
        self.default_icon
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_ITEM_ICON)
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
        t.default_icon = Some(DEFAULT_ITEM_ICON.to_string());
        t.divider = Some(DividerConfig {
            char: Some("─".to_string()),
            length: Some("{window_width}".to_string()),
        });
        t.marquee = Some(MarqueeConfig {
            char: "•".to_string(),
            gap: 4,
        });
        t.toast = Some(ToastConfig {
            style: None,
            char: Some("▌".to_string()),
            timeout: Some(2.0),
        });

        let mut colors = HashMap::new();
        let make_pair = |fg: i64, bg: i64| -> serde_json::Value {
            serde_json::Value::Array(vec![
                serde_json::Value::Number(fg.into()),
                serde_json::Value::Number(bg.into()),
            ])
        };
        colors.insert(
            "background".to_string(),
            serde_json::Value::Number(0.into()),
        );
        colors.insert("title".to_string(), make_pair(255, -1));
        colors.insert("text".to_string(), make_pair(255, -1));
        colors.insert("highlight".to_string(), make_pair(0, 255));
        colors.insert(
            "highlight_text".to_string(),
            serde_json::Value::Number(0.into()),
        );
        colors.insert("border".to_string(), make_pair(250, -1));
        colors.insert("divider".to_string(), make_pair(245, -1));
        colors.insert("accent".to_string(), make_pair(255, -1));
        colors.insert("shortcut_key".to_string(), make_pair(255, -1));
        colors.insert("selection".to_string(), make_pair(0, 255));
        colors.insert("selected_tab".to_string(), make_pair(0, 255));
        colors.insert("unselected_tab".to_string(), make_pair(245, -1));
        colors.insert("line_numbers".to_string(), make_pair(242, -1));
        colors.insert("whitespace".to_string(), make_pair(242, -1));
        colors.insert("gutter".to_string(), make_pair(242, -1));
        colors.insert("footer".to_string(), make_pair(245, -1));
        colors.insert("help_text".to_string(), make_pair(250, -1));
        colors.insert("plugin".to_string(), make_pair(250, -1));
        colors.insert("window_close_button".to_string(), make_pair(255, -1));
        colors.insert("scrollbar".to_string(), make_pair(245, -1));
        colors.insert("shadow".to_string(), make_pair(236, -1));
        colors.insert("modal_yes_button".to_string(), make_pair(255, -1));
        colors.insert("modal_no_button".to_string(), make_pair(203, -1));
        colors.insert("modal_cancel_button".to_string(), make_pair(245, -1));
        colors.insert("toast_info".to_string(), make_pair(255, -1));
        colors.insert("toast_warning".to_string(), make_pair(250, -1));
        colors.insert("toast_error".to_string(), make_pair(245, -1));
        colors.extend(default_activity_overrides_map());
        t.colors_256 = Some(colors);
        t
    }
}

/// Default menu item glyph for items without an explicitly configured icon.
pub const DEFAULT_ITEM_ICON: &str = "{nf:>:#f105}";

/// Special identifier constant for pseudo-random theme selection.
pub const RANDOM_THEME_LABEL: &str = "[ Random ]";

/// Checks if a theme string specifies random theme selection.
/// Matches "[ Random ]", "[Random]", or "Random" (case-insensitively).
pub fn is_random_theme_spec(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.eq_ignore_ascii_case("[ Random ]")
        || trimmed.eq_ignore_ascii_case("[Random]")
        || trimmed.eq_ignore_ascii_case("Random")
}

/// Type alias for all loaded themes mapped by theme name
pub type ThemeCollection = HashMap<String, Theme>;

/// Selects a pseudo-random theme from available themes, skipping random specs.
pub fn pick_random_theme(themes: &ThemeCollection) -> Option<(&str, &Theme)> {
    let eligible: Vec<(&str, &Theme)> = themes
        .iter()
        .filter(|(k, _)| !is_random_theme_spec(k))
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    if eligible.is_empty() {
        return None;
    }
    let seed = if let Ok(mut f) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        let mut buf = [0u8; 8];
        if f.read_exact(&mut buf).is_ok() {
            u64::from_ne_bytes(buf)
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42)
        }
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    };
    let idx = (seed as usize) % eligible.len();
    Some(eligible[idx])
}

/// Loads and parses theme definitions from `bashmenu.themes`.
pub fn load_themes<P: AsRef<Path>>(path: P) -> Result<ThemeCollection> {
    let p = path.as_ref();
    let content = fs::read_to_string(p).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    let themes: ThemeCollection = serde_saphyr::from_str(&content)?;
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
    let mut map = serde_json::Map::new();
    for (k, v) in entries {
        map.insert(k.clone(), serde_json::to_value(v)?);
    }
    let yaml_body = serde_saphyr::to_string(&map)?.replace(": =\n", ": '='\n");
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
        let fixture_path = Path::new("../../defaults/bashmenu.themes");
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

    #[test]
    fn test_theme_default_icon_resolution() {
        let mut theme = Theme::default();
        assert_eq!(theme.default_icon, None);
        assert_eq!(theme.resolved_default_icon(), DEFAULT_ITEM_ICON);
        assert_eq!(DEFAULT_ITEM_ICON, "{nf:>:#f105}");

        theme.default_icon = Some("{nf:*:#f005}".to_string());
        assert_eq!(theme.resolved_default_icon(), "{nf:*:#f005}");

        // Empty or whitespace falls back to default
        theme.default_icon = Some("   ".to_string());
        assert_eq!(theme.resolved_default_icon(), DEFAULT_ITEM_ICON);

        let mono = Theme::monochrome();
        assert_eq!(mono.resolved_default_icon(), DEFAULT_ITEM_ICON);
        assert_eq!(mono.default_icon.as_deref(), Some(DEFAULT_ITEM_ICON));
    }

    #[test]
    fn test_pick_random_theme_skips_random_specs() {
        let mut themes = ThemeCollection::new();
        themes.insert("[ Random ]".to_string(), Theme::default());
        themes.insert("Random".to_string(), Theme::default());
        themes.insert("[Random]".to_string(), Theme::default());

        // When only random specs are present, none should be eligible
        assert!(pick_random_theme(&themes).is_none());

        themes.insert("dracula".to_string(), Theme::default());
        let picked = pick_random_theme(&themes);
        assert!(picked.is_some());
        let (name, _) = picked.unwrap();
        assert_eq!(name, "dracula");
    }
}
