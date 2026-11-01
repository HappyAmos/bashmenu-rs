//! Configuration definitions and persistence for `bashmenu.yml`.

use crate::error::{BashMenuError, Result};
use crate::theme::{Theme, WallpaperConfig, WallpaperSetting};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Retrieve value from nested configuration using dot-notation string.
    pub fn get_value(&self, key_path: &str) -> Option<String> {
        let val = serde_json::to_value(self).ok()?;
        crate::macro_engine::resolve_config_dot_path(&val, key_path)
    }

    /// Set value in configuration using dot-notation string.
    pub fn set_value(&mut self, key_path: &str, value: serde_yml::Value) -> bool {
        let norm = if let Some(stripped) = key_path.strip_prefix("app.") {
            stripped
        } else if let Some(stripped) = key_path.strip_prefix("bashmenu.") {
            stripped
        } else {
            key_path
        };

        match norm {
            "settings.use_nerd_fonts" => {
                self.settings.use_nerd_fonts = match &value {
                    serde_yml::Value::Bool(b) => Some(*b),
                    serde_yml::Value::String(s) => Some(s.eq_ignore_ascii_case("true")),
                    _ => None,
                };
            }
            "settings.check_for_updates" => {
                self.settings.check_for_updates = match &value {
                    serde_yml::Value::Bool(b) => Some(*b),
                    serde_yml::Value::String(s) => Some(s.eq_ignore_ascii_case("true")),
                    _ => None,
                };
            }
            "settings.tab_to_spaces" => {
                self.settings.tab_to_spaces = match &value {
                    serde_yml::Value::Bool(b) => Some(*b),
                    serde_yml::Value::String(s) => Some(s.eq_ignore_ascii_case("true")),
                    _ => None,
                };
            }
            "settings.show_menu_shortcuts" => {
                self.settings.show_menu_shortcuts = match &value {
                    serde_yml::Value::Bool(b) => Some(*b),
                    serde_yml::Value::String(s) => Some(s.eq_ignore_ascii_case("true")),
                    _ => None,
                };
            }
            "settings.nerd_font_width" => {
                self.settings.nerd_font_width = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    serde_yml::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
            }
            "settings.tabstop" => {
                self.settings.tabstop = match &value {
                    serde_yml::Value::Number(n) => n.as_u64().map(|v| v as usize),
                    serde_yml::Value::String(s) => s.parse::<usize>().ok(),
                    _ => None,
                };
            }
            "settings.pager_editor" => {
                self.settings.pager_editor = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.ping_target" => {
                self.settings.ping_target = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.status_gutter" => {
                self.settings.status_gutter = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.refresh_interval" => {
                self.settings.refresh_interval = Some(value);
            }
            "settings.cache_dir" => {
                self.settings.cache_dir = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.scripts_dir" => {
                self.settings.scripts_dir = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.templates_dir" => {
                self.settings.templates_dir = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "theme" => {
                if let serde_yml::Value::String(s) = value {
                    self.theme = s;
                }
            }
            _ => {
                if let Some(key) = norm.strip_prefix("settings.") {
                    self.settings.extra.insert(key.to_string(), value);
                } else if let Some(key) = norm.strip_prefix("user.") {
                    self.user.insert(key.to_string(), value);
                } else {
                    self.extra.insert(norm.to_string(), value);
                }
            }
        }
        true
    }
}

/// Returns the canonical BashMenu user configuration directory:
/// `~/.config/bashmenu` (or `$XDG_CONFIG_HOME/bashmenu`).
pub fn get_user_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("bashmenu");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/bashmenu");
    }
    PathBuf::from(".config/bashmenu")
}

/// Resolves a configuration, menu, or theme file path according to the standard
/// BashMenu single-source-of-truth rules:
///
/// 1. If `target` is absolute, return it directly.
/// 2. If `target` contains directory components (e.g. `./local.yml` or `sub/menu.mnu`),
///    treat it as an explicit relative path.
/// 3. Standard User Configuration Path:
///    Look in `get_user_config_dir().join(target)`. If it exists, return it.
/// 4. Environment override (`$BASHMENU_DIR`):
///    If set and `$BASHMENU_DIR/<target>` exists, return it.
/// 5. Test Fixtures fallback (for unit tests running inside workspace crates):
///    If `tests/fixtures/<target>` exists, return it.
/// 6. Default fallback:
///    Return `get_user_config_dir().join(target)`. This ensures newly created/saved
///    files always persist to the single source of truth in `~/.config/bashmenu/`
///    regardless of which directory the command was launched from.
pub fn resolve_app_path<P: AsRef<Path>>(target: P) -> PathBuf {
    let target = target.as_ref();
    if target.is_absolute() {
        return target.to_path_buf();
    }
    if target.components().count() > 1 && target.exists() {
        return target.to_path_buf();
    }

    let user_cfg_dir = get_user_config_dir();
    let user_path = user_cfg_dir.join(target);
    if user_path.exists() {
        return user_path;
    }

    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        let candidate = PathBuf::from(dir).join(target);
        if candidate.exists() {
            return candidate;
        }
    }

    let fixture_paths = [
        PathBuf::from("tests/fixtures").join(target),
        PathBuf::from("../../tests/fixtures").join(target),
    ];
    for fix in fixture_paths {
        if fix.exists() {
            return fix;
        }
    }

    // Default to the single source of truth in ~/.config/bashmenu/
    user_path
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
