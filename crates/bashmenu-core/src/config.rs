//! Configuration definitions and persistence for `bashmenu.yml`.

use crate::error::{BashMenuError, Result};
use crate::theme::{Theme, WallpaperConfig, WallpaperSetting};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Canonical header comments preserved at the top of bashmenu.yml.
pub const CONFIG_HEADER: &str = r#"# ==============================================================================
# BashMenu Configuration (bashmenu.yml)
# All keys defined in this file can be referenced via placeholders using the
# '{app.<key>}' namespace (e.g. {app.theme}, {app.settings.tabstop}, {app.user.postal_code}),
# or shorthand shortcuts ({theme}, {user.<key>}, {settings.<key>}).
# ==============================================================================
"#;

/// Inactivity timeout settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InactivityTimeout {
    /// Inactivity threshold in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milliseconds: Option<u64>,
    /// Shell command to execute when idle threshold is reached
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Application settings section in bashmenu.yml
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Settings {
    /// Directory for cache files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<String>,

    /// Whether to check for upstream updates on launch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_for_updates: Option<bool>,

    /// Templates directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub templates_dir: Option<String>,

    /// Extension scripts directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts_dir: Option<String>,

    /// Target host or IP for ping checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping_target: Option<String>,

    /// Convert tabs to spaces in text editor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_to_spaces: Option<bool>,

    /// Tab stop width in spaces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabstop: Option<usize>,

    /// Show menu hotkey shortcuts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_menu_shortcuts: Option<bool>,

    /// Enable Nerd Fonts rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_nerd_fonts: Option<bool>,

    /// Nerd font width calculation mode (e.g. "auto", "fixed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nerd_font_width: Option<String>,

    /// Template string for bottom status gutter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_gutter: Option<String>,

    /// Screensaver / idle timeout configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inactivity_timeout: Option<InactivityTimeout>,

    /// Periodic refresh rate in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval: Option<serde_yml::Value>,

    /// Pager / editor executable (e.g. "vim", "nano")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pager_editor: Option<String>,

    /// Plugin configuration table or mappings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<serde_yml::Value>,

    /// Background wallpaper configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<WallpaperSetting>,

    /// Additional arbitrary user-defined settings preserved across saves
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

/// Root configuration structure corresponding to `bashmenu.yml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BashMenuConfig {
    /// Schema / configuration version
    #[serde(default = "default_version")]
    pub version: String,

    /// Active theme name
    #[serde(default = "default_theme")]
    pub theme: String,

    /// User-defined custom variables accessible via {user.<key>}
    #[serde(default)]
    pub user: HashMap<String, serde_yml::Value>,

    /// Global application settings
    #[serde(default)]
    pub settings: Settings,

    /// Additional arbitrary top-level fields preserved across saves
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

fn default_version() -> String {
    "0.0.1".to_string()
}

fn default_theme() -> String {
    "alaska-mountain".to_string()
}

impl Default for BashMenuConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            theme: default_theme(),
            user: HashMap::new(),
            settings: Settings::default(),
            extra: HashMap::new(),
        }
    }
}

impl BashMenuConfig {
    /// Returns the effective wallpaper config, combining user settings override with theme config.
    pub fn resolve_wallpaper(&self, theme: Option<&Theme>) -> Option<WallpaperConfig> {
        let user_bg = self.settings.background.as_ref().map(|s| s.to_config());
        let theme_bg = theme.and_then(|t| t.wallpaper_config());

        match (user_bg, theme_bg) {
            (Some(user), Some(theme)) => {
                if !user.enabled {
                    return None;
                }
                Some(WallpaperConfig {
                    enabled: user.enabled,
                    image: user.image.or(theme.image),
                    opacity: if (user.opacity - 0.35).abs() > f32::EPSILON {
                        user.opacity
                    } else {
                        theme.opacity
                    },
                    scaling: if user.scaling != "cover" {
                        user.scaling
                    } else {
                        theme.scaling
                    },
                    clustering: if user.clustering != 0 {
                        user.clustering
                    } else {
                        theme.clustering
                    },
                    quantization: if user.quantization != 1 {
                        user.quantization
                    } else {
                        theme.quantization
                    },
                    mode: user.mode.or(theme.mode),
                })
            }
            (Some(user), None) => {
                if !user.enabled {
                    None
                } else {
                    Some(user)
                }
            }
            (None, Some(theme)) => {
                if !theme.enabled {
                    None
                } else {
                    Some(theme)
                }
            }
            (None, None) => None,
        }
    }
}

/// Loads and parses `bashmenu.yml` from the specified path.
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<BashMenuConfig> {
    let p = path.as_ref();
    let content = fs::read_to_string(p).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    let config: BashMenuConfig = serde_yml::from_str(&content)?;
    Ok(config)
}

/// Persists configuration to `bashmenu.yml` with the top-level comment header preserved.
pub fn save_config<P: AsRef<Path>>(path: P, config: &BashMenuConfig) -> Result<()> {
    let p = path.as_ref();
    let yaml_body = serde_yml::to_string(config)?;
    let output = format!("{}\n{}", CONFIG_HEADER.trim(), yaml_body);
    fs::write(p, output).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_and_save_config_roundtrip() {
        let fixture_path = Path::new("../../tests/fixtures/bashmenu.yml");
        if fixture_path.exists() {
            let config = load_config(fixture_path).expect("Failed to load bashmenu.yml fixture");
            assert_eq!(config.version, "0.0.1");
            assert_eq!(config.theme, "alaska-mountain");
            assert_eq!(config.settings.tabstop, Some(4));
            assert_eq!(config.settings.use_nerd_fonts, Some(true));

            // Verify serialization retains the header
            let temp_dir = std::env::temp_dir();
            let test_out = temp_dir.join("test_save_bashmenu.yml");
            save_config(&test_out, &config).expect("Failed to save config");
            let saved_content = fs::read_to_string(&test_out).expect("Read saved");
            assert!(saved_content.starts_with("# ===================================="));
            let _ = fs::remove_file(test_out);
        }
    }
}
