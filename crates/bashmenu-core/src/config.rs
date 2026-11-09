//! Configuration definitions and persistence for `bashmenu.yml`.

use crate::error::{BashMenuError, Result};
use crate::macro_engine::resolve_dev_dir;
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

    /// Default toast notification timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toast_timeout: Option<f64>,

    /// Delay in seconds before stationary mouse hover expands the help gutter (default: 4.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_gutter_delay: Option<f64>,

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

    /// Menu return transition effect ("instant", "fade", "curtain", "none")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,

    /// Animation transition speed ("fast", "normal", "slow", or milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_speed: Option<String>,

    /// Modal popup animation effect ("zoom", "none")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modal_animation: Option<String>,

    /// Modal popup animation speed ("fast", "normal", "slow", or milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modal_animation_speed: Option<String>,

    /// Direct Linux framebuffer (/dev/fb0) rendering ("auto", "true", "false")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framebuffer: Option<String>,

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

/// Default configuration schema version string (`"0.0.1"`).
fn default_version() -> String {
    "0.0.1".to_string()
}

/// Default active theme name (`"alaska-mountain"`).
fn default_theme() -> String {
    "alaska-mountain".to_string()
}

impl Default for BashMenuConfig {
    /// Returns default application configuration with default theme.
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
    /// Returns effective wallpaper config, combining settings with theme.
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

    /// Returns the effective help gutter hover delay in seconds (defaulting to 4.0s).
    pub fn help_gutter_delay(&self) -> f64 {
        self.settings.help_gutter_delay.unwrap_or(4.0)
    }

    /// Sanitizes configuration by migrating legacy or flattened settings keys.
    pub fn sanitize(&mut self) {
        if let Some(val) = self
            .settings
            .extra
            .remove("inactivity_timeout.milliseconds")
        {
            let ms = match &val {
                serde_yml::Value::Number(n) => n
                    .as_u64()
                    .or_else(|| {
                        n.as_i64()
                            .and_then(|v| if v >= 0 { Some(v as u64) } else { None })
                    })
                    .or_else(|| {
                        n.as_f64()
                            .and_then(|v| if v >= 0.0 { Some(v as u64) } else { None })
                    }),
                serde_yml::Value::String(s) => s.trim().parse::<u64>().ok(),
                _ => None,
            };
            if let Some(ms_val) = ms {
                self.settings
                    .inactivity_timeout
                    .get_or_insert_with(Default::default)
                    .milliseconds = Some(ms_val);
            }
        }
        if let Some(val) = self.settings.extra.remove("inactivity_timeout.command") {
            let cmd = match &val {
                serde_yml::Value::String(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                }
                serde_yml::Value::Number(n) => Some(n.to_string()),
                _ => None,
            };
            if cmd.is_some() {
                self.settings
                    .inactivity_timeout
                    .get_or_insert_with(Default::default)
                    .command = cmd;
            }
        }
        self.extra
            .remove("settings.inactivity_timeout.milliseconds");
        self.extra.remove("settings.inactivity_timeout.command");
        self.extra.remove("inactivity_timeout.milliseconds");
        self.extra.remove("inactivity_timeout.command");

        // Migrate any stray dotted flat keys in settings.extra into nested mappings
        let stray_settings_keys: Vec<(String, serde_yml::Value)> = self
            .settings
            .extra
            .iter()
            .filter(|(k, _)| k.contains('.'))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in stray_settings_keys {
            self.settings.extra.remove(&k);
            insert_nested_yaml_value(&mut self.settings.extra, &k, v);
        }

        // Migrate any stray dotted flat keys in self.extra into nested mappings
        let stray_extra_keys: Vec<(String, serde_yml::Value)> = self
            .extra
            .iter()
            .filter(|(k, _)| k.contains('.'))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in stray_extra_keys {
            self.extra.remove(&k);
            insert_nested_yaml_value(&mut self.extra, &k, v);
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
            "settings.toast_timeout" | "settings.toast.timeout" => {
                self.settings.toast_timeout = match &value {
                    serde_yml::Value::Number(n) => {
                        n.as_f64().or_else(|| n.as_u64().map(|v| v as f64))
                    }
                    serde_yml::Value::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                };
            }
            "settings.help_gutter_delay" | "settings.help_gutter_hover_delay" => {
                self.settings.help_gutter_delay = match &value {
                    serde_yml::Value::Number(n) => {
                        n.as_f64().or_else(|| n.as_u64().map(|v| v as f64))
                    }
                    serde_yml::Value::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                };
            }
            "settings.status_gutter" => {
                self.settings.status_gutter = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.inactivity_timeout.milliseconds" | "inactivity_timeout.milliseconds" => {
                let ms = match &value {
                    serde_yml::Value::Number(n) => n
                        .as_u64()
                        .or_else(|| {
                            n.as_i64()
                                .and_then(|v| if v >= 0 { Some(v as u64) } else { None })
                        })
                        .or_else(|| {
                            n.as_f64()
                                .and_then(|v| if v >= 0.0 { Some(v as u64) } else { None })
                        }),
                    serde_yml::Value::String(s) => s.trim().parse::<u64>().ok(),
                    _ => None,
                };
                let timeout = self
                    .settings
                    .inactivity_timeout
                    .get_or_insert_with(Default::default);
                timeout.milliseconds = ms;
                self.settings
                    .extra
                    .remove("inactivity_timeout.milliseconds");
                self.extra
                    .remove("settings.inactivity_timeout.milliseconds");
                self.extra.remove("inactivity_timeout.milliseconds");
            }
            "settings.inactivity_timeout.command" | "inactivity_timeout.command" => {
                let cmd = match &value {
                    serde_yml::Value::String(s) => {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    }
                    serde_yml::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
                let timeout = self
                    .settings
                    .inactivity_timeout
                    .get_or_insert_with(Default::default);
                timeout.command = cmd;
                self.settings.extra.remove("inactivity_timeout.command");
                self.extra.remove("settings.inactivity_timeout.command");
                self.extra.remove("inactivity_timeout.command");
            }
            "settings.inactivity_timeout" | "inactivity_timeout" => {
                if let Ok(timeout) = serde_yml::from_value::<InactivityTimeout>(value) {
                    self.settings.inactivity_timeout = Some(timeout);
                    self.settings.extra.remove("inactivity_timeout");
                    self.extra.remove("settings.inactivity_timeout");
                    self.extra.remove("inactivity_timeout");
                }
            }
            "settings.transition" => {
                self.settings.transition = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            "settings.transition_speed" => {
                self.settings.transition_speed = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    serde_yml::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
            }
            "settings.modal_animation" => {
                self.settings.modal_animation = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    serde_yml::Value::Bool(b) => Some(if *b { "zoom" } else { "none" }.to_string()),
                    _ => None,
                };
            }
            "settings.modal_animation_speed" => {
                self.settings.modal_animation_speed = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    serde_yml::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
            }
            "settings.framebuffer" => {
                self.settings.framebuffer = match &value {
                    serde_yml::Value::String(s) => Some(s.clone()),
                    serde_yml::Value::Bool(b) => Some(b.to_string()),
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
                    insert_nested_yaml_value(&mut self.settings.extra, key, value);
                } else if let Some(key) = norm.strip_prefix("user.") {
                    insert_nested_yaml_value(&mut self.user, key, value);
                } else {
                    insert_nested_yaml_value(&mut self.extra, norm, value);
                }
            }
        }
        true
    }
}

/// Inserts a value into a YAML map, decomposing dot-separated paths into nested mappings
/// to guarantee that dotted flat keys are never emitted in configuration YAML.
fn insert_nested_yaml_value(
    map: &mut HashMap<String, serde_yml::Value>,
    path: &str,
    value: serde_yml::Value,
) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }

    let top_key = parts[0].to_string();
    let entry = map
        .entry(top_key)
        .or_insert_with(|| serde_yml::Value::Mapping(serde_yml::Mapping::new()));

    let mut curr = entry;
    for &part in &parts[1..parts.len() - 1] {
        if !curr.is_mapping() {
            *curr = serde_yml::Value::Mapping(serde_yml::Mapping::new());
        }
        let mapping = curr.as_mapping_mut().unwrap();
        let key = serde_yml::Value::String(part.to_string());
        if !mapping.contains_key(&key) || !mapping.get(&key).is_some_and(|v| v.is_mapping()) {
            mapping.insert(
                key.clone(),
                serde_yml::Value::Mapping(serde_yml::Mapping::new()),
            );
        }
        curr = mapping.get_mut(&key).unwrap();
    }

    if !curr.is_mapping() {
        *curr = serde_yml::Value::Mapping(serde_yml::Mapping::new());
    }
    if let Some(mapping) = curr.as_mapping_mut() {
        let leaf_key = serde_yml::Value::String(parts.last().unwrap().to_string());
        mapping.insert(leaf_key, value);
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
/// 2. If `target` contains directory components (e.g. `./local.yml`),
///    treat it as an explicit relative path.
/// 3. Standard User Configuration Path:
///    Look in `get_user_config_dir().join(target)`. If it exists, return it.
/// 4. Environment override (`$BASHMENU_DIR`):
///    If set and `$BASHMENU_DIR/<target>` exists, return it.
/// 5. Repository root fallback:
///    If the file exists in the repository root directory (`resolve_dev_dir`), return it.
/// 6. Default fallback:
///    Return `get_user_config_dir().join(target)`. This ensures saved files
///    always persist to `~/.config/bashmenu/` regardless of working directory.
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

    if target.exists() {
        return target.to_path_buf();
    }

    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));
    let dev_dir = resolve_dev_dir(&home, &user_cfg_dir);
    let dev_path = dev_dir.join(target);
    if dev_path.exists() {
        return dev_path;
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
    let mut config: BashMenuConfig = serde_yml::from_str(&content)?;
    config.sanitize();
    Ok(config)
}

/// Persists configuration to `bashmenu.yml` preserving comment header.
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
        let fixture_path = Path::new("../../bashmenu.yml");
        if fixture_path.exists() {
            let config = load_config(fixture_path).expect("Failed to load bashmenu.yml");
            assert!(!config.version.is_empty());
            assert_eq!(config.theme, "canopy");
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

    #[test]
    fn test_help_gutter_delay_config_and_set_value() {
        let mut config = BashMenuConfig::default();
        assert_eq!(config.help_gutter_delay(), 4.0);

        // Test set_value with float number
        assert!(config.set_value(
            "settings.help_gutter_delay",
            serde_yml::to_value(2.5).unwrap(),
        ));
        assert_eq!(config.settings.help_gutter_delay, Some(2.5));
        assert_eq!(config.help_gutter_delay(), 2.5);

        // Test set_value with string
        assert!(config.set_value(
            "app.settings.help_gutter_delay",
            serde_yml::Value::String("0.0".to_string()),
        ));
        assert_eq!(config.settings.help_gutter_delay, Some(0.0));
        assert_eq!(config.help_gutter_delay(), 0.0);

        // Test get_value via dot-path
        let val_str = config.get_value("settings.help_gutter_delay");
        assert_eq!(val_str, Some("0.0".to_string()));
    }

    #[test]
    fn test_inactivity_timeout_set_value_and_sanitize() {
        let mut config = BashMenuConfig::default();

        // Initially inactivity_timeout is None
        assert_eq!(config.settings.inactivity_timeout, None);

        // Set milliseconds via app.settings.inactivity_timeout.milliseconds
        assert!(config.set_value(
            "app.settings.inactivity_timeout.milliseconds",
            serde_yml::Value::Number(serde_yml::Number::from(60000)),
        ));
        assert_eq!(
            config
                .settings
                .inactivity_timeout
                .as_ref()
                .and_then(|t| t.milliseconds),
            Some(60000)
        );
        assert_eq!(
            config.get_value("app.settings.inactivity_timeout.milliseconds"),
            Some("60000".to_string())
        );

        // Set command via settings.inactivity_timeout.command
        assert!(config.set_value(
            "settings.inactivity_timeout.command",
            serde_yml::Value::String("kcchase".to_string()),
        ));
        assert_eq!(
            config
                .settings
                .inactivity_timeout
                .as_ref()
                .and_then(|t| t.command.as_deref()),
            Some("kcchase")
        );
        assert_eq!(
            config.get_value("settings.inactivity_timeout.command"),
            Some("kcchase".to_string())
        );

        // Ensure extra maps are clean (no long flattened string keys)
        assert!(!config
            .settings
            .extra
            .contains_key("inactivity_timeout.milliseconds"));
        assert!(!config
            .settings
            .extra
            .contains_key("inactivity_timeout.command"));

        // Verify save_config does not output flattened extra keys
        let temp_dir = std::env::temp_dir();
        let test_out = temp_dir.join("test_save_inactivity_timeout.yml");
        save_config(&test_out, &config).expect("Save config");
        let saved_str = fs::read_to_string(&test_out).expect("Read config");
        assert!(
            saved_str.contains("milliseconds: 60000") && saved_str.contains("command: kcchase")
        );
        assert!(!saved_str.contains("inactivity_timeout.milliseconds:"));
        assert!(!saved_str.contains("inactivity_timeout.command:"));
        let _ = fs::remove_file(test_out);

        // Test sanitize migration from legacy/stray flattened keys in extra
        let mut legacy_config = BashMenuConfig::default();
        legacy_config.settings.extra.insert(
            "inactivity_timeout.milliseconds".to_string(),
            serde_yml::Value::Number(serde_yml::Number::from(120000)),
        );
        legacy_config.settings.extra.insert(
            "inactivity_timeout.command".to_string(),
            serde_yml::Value::String("cmatrix -s".to_string()),
        );
        legacy_config.sanitize();
        assert_eq!(
            legacy_config
                .settings
                .inactivity_timeout
                .as_ref()
                .and_then(|t| t.milliseconds),
            Some(120000)
        );
        assert_eq!(
            legacy_config
                .settings
                .inactivity_timeout
                .as_ref()
                .and_then(|t| t.command.as_deref()),
            Some("cmatrix -s")
        );
        assert!(!legacy_config
            .settings
            .extra
            .contains_key("inactivity_timeout.milliseconds"));
        assert!(!legacy_config
            .settings
            .extra
            .contains_key("inactivity_timeout.command"));
    }

    #[test]
    fn test_arbitrary_nested_dot_paths_never_flatten() {
        let mut config = BashMenuConfig::default();

        // Set arbitrary nested dotted path under user
        assert!(config.set_value(
            "user.profile.contact.email",
            serde_yml::Value::String("user@example.com".to_string()),
        ));

        // Set arbitrary nested dotted path under settings
        assert!(config.set_value(
            "settings.custom_feature.sub_option.enabled",
            serde_yml::Value::Bool(true),
        ));

        // Set arbitrary nested dotted path in extra
        assert!(config.set_value(
            "experimental.flags.speed",
            serde_yml::Value::String("ludicrous".to_string()),
        ));

        // Verify none of the maps contain literal keys with dots
        assert!(!config.user.contains_key("profile.contact.email"));
        assert!(!config
            .settings
            .extra
            .contains_key("custom_feature.sub_option.enabled"));
        assert!(!config.extra.contains_key("experimental.flags.speed"));

        // Verify get_value resolves them via dot path navigation
        assert_eq!(
            config.get_value("user.profile.contact.email"),
            Some("user@example.com".to_string())
        );
        assert_eq!(
            config.get_value("settings.custom_feature.sub_option.enabled"),
            Some("true".to_string())
        );
        assert_eq!(
            config.get_value("experimental.flags.speed"),
            Some("ludicrous".to_string())
        );

        // Verify YAML output is properly structured and contains no flat dotted keys
        let temp_dir = std::env::temp_dir();
        let test_out = temp_dir.join("test_save_nested_paths.yml");
        save_config(&test_out, &config).expect("Save config");
        let saved_str = fs::read_to_string(&test_out).expect("Read config");

        assert!(!saved_str.contains("profile.contact.email:"));
        assert!(!saved_str.contains("custom_feature.sub_option.enabled:"));
        assert!(!saved_str.contains("flags.speed:"));
        assert!(saved_str.contains("email: user@example.com"));
        let _ = fs::remove_file(test_out);
    }
}
