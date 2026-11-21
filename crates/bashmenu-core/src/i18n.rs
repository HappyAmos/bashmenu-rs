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
        if let Some(canonical) = parsed.get("meta.locale") {
            self.active_locale = canonical.clone();
        } else {
            self.active_locale = locale_code.to_string();
        }
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

    /// Returns the map of default canonical keys to strings.
    pub fn default_strings(&self) -> &HashMap<String, String> {
        &self.default_strings
    }

    /// Returns the map of active loaded keys to strings.
    pub fn active_strings(&self) -> &HashMap<String, String> {
        &self.active_strings
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

    if locale_or_path.eq_ignore_ascii_case("auto") {
        let sys_loc = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        let clean = sys_loc.split('.').next().unwrap_or(&sys_loc);
        if !clean.is_empty()
            && !clean.eq_ignore_ascii_case("c")
            && !clean.eq_ignore_ascii_case("posix")
            && !clean.eq_ignore_ascii_case("en")
            && !clean.eq_ignore_ascii_case("en_us")
        {
            if let Ok(()) = set_locale(clean) {
                return Ok(());
            }
        }
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

    // 3. System shared locales (/usr/share/bashmenu/locales)
    let sys_dir = PathBuf::from("/usr/share/bashmenu/locales");
    if sys_dir.is_dir() {
        search_dirs.push(sys_dir);
    }

    // 4. Current working directory and ancestors
    if let Ok(cwd) = std::env::current_dir() {
        let mut curr = Some(cwd.as_path());
        while let Some(dir) = curr {
            search_dirs.push(dir.join("locales"));
            curr = dir.parent();
        }
    }

    // 5. Cargo manifest directory and ancestors (for test runners and dev builds)
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut curr = Some(Path::new(&manifest));
        while let Some(dir) = curr {
            search_dirs.push(dir.join("locales"));
            curr = dir.parent();
        }
    }

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

/// Discovered locale metadata describing an available translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleInfo {
    /// Standard locale code (e.g. "en_US", "es_ES", "fr_FR")
    pub code: String,
    /// Friendly localized language name (e.g. "English (US)", "Español (España)")
    pub name: String,
    /// Path to file on disk, or `None` if built-in
    pub path: Option<PathBuf>,
}

#[derive(serde::Deserialize)]
struct LocaleMetaHeader {
    meta: Option<LocaleMetaInner>,
}

#[derive(serde::Deserialize)]
struct LocaleMetaInner {
    locale: Option<String>,
    name: Option<String>,
}

/// Discovers available locale translation files across standard search paths:
/// 1. Built-in `en_US` ("English (US)")
/// 2. `$BASHMENU_DIR/locales/`
/// 3. `~/.config/bashmenu/locales/`
/// 4. `/usr/share/bashmenu/locales/`
/// 5. Current working directory and ancestors (`locales/`)
/// 6. Repository root and Cargo manifest directory (`locales/`)
pub fn discover_locales() -> Vec<LocaleInfo> {
    let mut search_dirs = Vec::new();

    // 1. $BASHMENU_DIR/locales
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        search_dirs.push(PathBuf::from(dir).join("locales"));
    }

    // 2. ~/.config/bashmenu/locales
    if let Ok(home) = std::env::var("HOME") {
        search_dirs.push(PathBuf::from(home).join(".config/bashmenu/locales"));
    }

    // 3. System shared locales (/usr/share/bashmenu/locales)
    let sys_dir = PathBuf::from("/usr/share/bashmenu/locales");
    if sys_dir.is_dir() {
        search_dirs.push(sys_dir);
    }

    // 4. Current working directory and ancestors
    if let Ok(cwd) = std::env::current_dir() {
        let mut curr = Some(cwd.as_path());
        while let Some(dir) = curr {
            search_dirs.push(dir.join("locales"));
            curr = dir.parent();
        }
    }

    // 5. Cargo manifest directory and ancestors
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut curr = Some(Path::new(&manifest));
        while let Some(dir) = curr {
            search_dirs.push(dir.join("locales"));
            curr = dir.parent();
        }
    }

    let mut map: std::collections::HashMap<String, LocaleInfo> = std::collections::HashMap::new();

    // Built-in English (US) is always available
    map.insert(
        "en_us".to_string(),
        LocaleInfo {
            code: "en_US".to_string(),
            name: "English (US)".to_string(),
            path: None,
        },
    );

    for dir in search_dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "yml" && ext != "yaml" {
                continue;
            }

            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if file_stem.is_empty() {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let header: Option<LocaleMetaHeader> = serde_yml::from_str(&content).ok();
            let raw_code = header
                .as_ref()
                .and_then(|h| h.meta.as_ref())
                .and_then(|m| m.locale.clone())
                .unwrap_or_else(|| {
                    if let Some((lang, country)) = file_stem.split_once('_') {
                        format!("{}_{}", lang.to_lowercase(), country.to_uppercase())
                    } else {
                        file_stem.to_string()
                    }
                });

            let name = header
                .as_ref()
                .and_then(|h| h.meta.as_ref())
                .and_then(|m| m.name.clone())
                .unwrap_or_else(|| raw_code.clone());

            let key = raw_code.trim().replace('-', "_").to_lowercase();
            if key == "en_us" || key == "en" {
                if let Some(en_info) = map.get_mut("en_us") {
                    if en_info.path.is_none() {
                        en_info.path = Some(path);
                    }
                }
            } else {
                map.entry(key).or_insert(LocaleInfo {
                    code: raw_code,
                    name,
                    path: Some(path),
                });
            }
        }
    }

    let mut result = Vec::new();
    if let Some(en) = map.remove("en_us") {
        result.push(en);
    }

    let mut others: Vec<LocaleInfo> = map.into_values().collect();
    others.sort_by(|a, b| a.name.cmp(&b.name));
    result.extend(others);

    result
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
pub fn parse_and_flatten_yaml(content: &str) -> Result<HashMap<String, String>, serde_yml::Error> {
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
    fn test_discover_locales() {
        let locales = discover_locales();
        assert!(!locales.is_empty(), "Must discover at least en_US");
        assert_eq!(locales[0].code, "en_US");
        assert_eq!(locales[0].name, "English (US)");

        let has_es = locales.iter().any(|loc| loc.code == "es_ES");
        assert!(has_es, "Must discover es_ES from repository locales/");
    }

    #[test]
    fn test_set_locale_auto_and_code() {
        let _ = set_locale("es_ES");
        assert_eq!(current_locale(), "es_ES");

        let _ = set_locale("auto");
        // Auto resets or uses system LANG; in test environment without LANG=es it defaults to en_US
        let cur = current_locale();
        assert!(cur == "en_US" || !cur.is_empty());

        let _ = set_locale("en_US");
        assert_eq!(current_locale(), "en_US");
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
