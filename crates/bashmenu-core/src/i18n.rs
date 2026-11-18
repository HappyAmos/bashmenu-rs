//! Internationalization (i18n) and localized string table engine.
//!
//! Provides compile-time embedded English (US) fallback string catalogs,
//! runtime locale loading from YAML files, transparent missing-key fallback,
//! and variable interpolation via the [`t!`] macro.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

/// Compile-time embedded canonical English (US) string table.
pub const DEFAULT_EN_US_YAML: &str = include_str!("../../../locales/en_us.yml");

/// Flattened string catalog mapping dot-separated keys to translated values.
#[derive(Debug, Clone)]
pub struct LocaleCatalog {
    active_locale: String,
    active_strings: HashMap<String, String>,
    default_strings: HashMap<String, String>,
}

impl LocaleCatalog {
    /// Create a new catalog initialized with the embedded English fallback.
    pub fn new() -> Self {
        let default_strings = parse_and_flatten_yaml(DEFAULT_EN_US_YAML).unwrap_or_default();
        Self {
            active_locale: "en_US".to_string(),
            active_strings: HashMap::new(),
            default_strings,
        }
    }

    /// Returns the currently active locale identifier (e.g. "en_US", "es_ES").
    pub fn active_locale(&self) -> &str {
        &self.active_locale
    }

    /// Load and activate a custom locale from a YAML string.
    pub fn load_yaml_str(&mut self, locale_code: &str, yaml_content: &str) -> Result<(), String> {
        let parsed = parse_and_flatten_yaml(yaml_content)
            .map_err(|e| format!("Failed to parse locale YAML for '{}': {}", locale_code, e))?;
        self.active_locale = locale_code.to_string();
        self.active_strings = parsed;
        Ok(())
    }

    /// Load and activate a custom locale from a file on disk.
    pub fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read locale file '{}': {}", path.display(), e))?;
        let code = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom");
        self.load_yaml_str(code, &content)
    }

    /// Reset the catalog back to the default English (US) locale.
    pub fn reset_to_default(&mut self) {
        self.active_locale = "en_US".to_string();
        self.active_strings.clear();
    }

    /// Resolve a string key, falling back to embedded default English if missing.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(val) = self.active_strings.get(key) {
            return val.as_str();
        }
        if let Some(val) = self.default_strings.get(key) {
            return val.as_str();
        }
        key
    }

    /// Translate a key and interpolate dynamic variables (e.g. `{line}`, `{file}`).
    pub fn get_with(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let raw = self.get(key);
        interpolate_string(raw, vars)
    }
}

impl Default for LocaleCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Global thread-safe locale catalog instance.
static GLOBAL_CATALOG: LazyLock<RwLock<LocaleCatalog>> =
    LazyLock::new(|| RwLock::new(LocaleCatalog::new()));

/// Translate a key into the active locale.
pub fn translate(key: &str) -> String {
    let catalog = GLOBAL_CATALOG.read().unwrap();
    catalog.get(key).to_string()
}

/// Translate a key into the active locale with variable interpolation.
pub fn translate_with(key: &str, vars: &[(&str, &str)]) -> String {
    let catalog = GLOBAL_CATALOG.read().unwrap();
    catalog.get_with(key, vars)
}

/// Get the active locale code (e.g. "en_US", "es_ES").
pub fn current_locale() -> String {
    let catalog = GLOBAL_CATALOG.read().unwrap();
    catalog.active_locale().to_string()
}

/// Set the active locale by code or file path.
///
/// Searches `$BASHMENU_DIR/locales/`, `~/.config/bashmenu/locales/`,
/// `$REPO_DIR/locales/`, and local working directory for `<locale>.yml`.
pub fn set_locale(locale_or_path: &str) -> Result<(), String> {
    if locale_or_path.is_empty()
        || locale_or_path.eq_ignore_ascii_case("en")
        || locale_or_path.eq_ignore_ascii_case("en_us")
    {
        let mut catalog = GLOBAL_CATALOG.write().unwrap();
        catalog.reset_to_default();
        return Ok(());
    }

    let candidate_path = Path::new(locale_or_path);
    if candidate_path.is_file() {
        let mut catalog = GLOBAL_CATALOG.write().unwrap();
        return catalog.load_file(candidate_path);
    }

    // Try resolving from standard locale search directories
    if let Some(resolved) = resolve_locale_path(locale_or_path) {
        let mut catalog = GLOBAL_CATALOG.write().unwrap();
        return catalog.load_file(&resolved);
    }

    Err(format!(
        "Locale '{}' could not be resolved from standard search paths",
        locale_or_path
    ))
}

/// Attempt to find a locale file in known bashmenu search locations.
pub fn resolve_locale_path(code: &str) -> Option<PathBuf> {
    let clean_code = code.trim().replace('-', "_").to_lowercase();
    let filenames = [
        format!("{}.yml", code),
        format!("{}.yml", clean_code),
        format!("{}.yaml", code),
        format!("{}.yaml", clean_code),
    ];

    let mut search_dirs = Vec::new();

    // 1. $BASHMENU_DIR/locales
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        search_dirs.push(PathBuf::from(dir).join("locales"));
    }

    // 2. ~/.config/bashmenu/locales
    if let Ok(home) = std::env::var("HOME") {
        search_dirs.push(PathBuf::from(home).join(".config/bashmenu/locales"));
    }

    // 3. Current working directory ./locales
    search_dirs.push(PathBuf::from("locales"));

    for dir in search_dirs {
        for fname in &filenames {
            let p = dir.join(fname);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    None
}

/// Helper function to interpolate `{variable}` placeholders inside a string.
pub fn interpolate_string(template: &str, vars: &[(&str, &str)]) -> String {
    if !template.contains('{') || vars.is_empty() {
        return template.to_string();
    }
    let mut result = template.to_string();
    for (key, val) in vars {
        let placeholder = format!("{{{}}}", key);
        result = result.replace(&placeholder, val);
    }
    result
}

/// Parse a YAML string and recursively flatten mappings into dot-delimited paths.
fn parse_and_flatten_yaml(content: &str) -> Result<HashMap<String, String>, serde_yml::Error> {
    let value: serde_yml::Value = serde_yml::from_str(content)?;
    let mut map = HashMap::new();
    if let serde_yml::Value::Mapping(root) = value {
        for (k, v) in root {
            if let Some(prefix) = k.as_str() {
                flatten_recursive(prefix, &v, &mut map);
            }
        }
    }
    Ok(map)
}

fn flatten_recursive(prefix: &str, val: &serde_yml::Value, out: &mut HashMap<String, String>) {
    match val {
        serde_yml::Value::Mapping(m) => {
            for (k, v) in m {
                if let Some(subkey) = k.as_str() {
                    let next_prefix = format!("{}.{}", prefix, subkey);
                    flatten_recursive(&next_prefix, v, out);
                }
            }
        }
        serde_yml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        serde_yml::Value::Number(n) => {
            out.insert(prefix.to_string(), n.to_string());
        }
        serde_yml::Value::Bool(b) => {
            out.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }
}

/// Convenience macro for translating keys into the active locale.
///
/// # Examples
/// ```ignore
/// use bashmenu_core::t;
///
/// let yes = t!("common.buttons.yes");
/// let legend = t!("bashedit.status_legend", line => 42, col => 10, char => 'a');
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $($k:ident => $v:expr),+ $(,)?) => {
        $crate::i18n::translate_with($key, &[
            $( (stringify!($k), &$v.to_string()) ),+
        ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_default_en_us_catalog() {
        let catalog = LocaleCatalog::new();
        assert_eq!(catalog.active_locale(), "en_US");
        assert_eq!(catalog.get("common.buttons.yes"), "Yes");
        assert_eq!(catalog.get("common.buttons.cancel"), "Cancel");
        assert_eq!(catalog.get("common.gutters.help"), "Help");
        assert_eq!(catalog.get("bashedit.status_read_only"), "[RO]");
    }

    #[test]
    fn test_variable_interpolation() {
        let catalog = LocaleCatalog::new();
        let interpolated = catalog.get_with(
            "bashedit.status_legend",
            &[("line", "12"), ("col", "34"), ("char", "X")],
        );
        assert_eq!(interpolated, "Line 12, Col 34 | Char X");
    }

    #[test]
    fn test_custom_locale_overlay_and_fallback() {
        let mut catalog = LocaleCatalog::new();
        let custom_es = r#"
common:
  buttons:
    yes: "Sí"
    cancel: "Cancelar"
bashedit:
  saved_file: "Guardado {file}"
"#;
        catalog.load_yaml_str("es_ES", custom_es).unwrap();
        assert_eq!(catalog.active_locale(), "es_ES");
        // Translated keys
        assert_eq!(catalog.get("common.buttons.yes"), "Sí");
        assert_eq!(catalog.get("common.buttons.cancel"), "Cancelar");
        assert_eq!(
            catalog.get_with("bashedit.saved_file", &[("file", "test.sh")]),
            "Guardado test.sh"
        );
        // Fallback to default en_US for untranslated keys
        assert_eq!(catalog.get("common.buttons.no"), "No");
        assert_eq!(catalog.get("common.gutters.quit"), "Quit");
    }

    #[test]
    fn test_global_translate_macro() {
        let yes = translate("common.buttons.yes");
        assert_eq!(yes, "Yes");

        let legend = translate_with(
            "bashedit.status_legend",
            &[("line", "1"), ("col", "2"), ("char", "A")],
        );
        assert_eq!(legend, "Line 1, Col 2 | Char A");
    }

    #[test]
    fn test_es_es_locale_completeness() {
        let en_map = parse_and_flatten_yaml(DEFAULT_EN_US_YAML).unwrap();
        let es_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../locales/es_es.yml");
        assert!(es_path.is_file(), "locales/es_es.yml must exist");
        let es_content = std::fs::read_to_string(&es_path).unwrap();
        let es_map = parse_and_flatten_yaml(&es_content).unwrap();

        for key in en_map.keys() {
            if key == "meta.locale" || key == "meta.name" {
                continue;
            }
            assert!(
                es_map.contains_key(key),
                "Missing key in locales/es_es.yml: {}",
                key
            );
        }
    }
}
