//! ymlcheck - Theme and YAML Configuration Validator for BashMenu.
//!
//! Validates YAML files for syntax or specific schema rules (themes, config, menu).

use bashmenu_core::{
    activity_override_tokens, load_config, load_menu, load_shortcuts, load_themes,
    parse_and_flatten_yaml, validate_key_spec, LocaleCatalog, ACTIVITY_OVERRIDE_KEYS,
};
use clap::{ArgGroup, Parser};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// Canonical names for 16-color ANSI terminal palette colors.
const VALID_16_COLORS: &[&str] = &[
    "COLOR_BLACK",
    "COLOR_RED",
    "COLOR_GREEN",
    "COLOR_YELLOW",
    "COLOR_BLUE",
    "COLOR_MAGENTA",
    "COLOR_CYAN",
    "COLOR_WHITE",
    "COLOR_GREY",
    "COLOR_GRAY",
    "COLOR_BRIGHT_BLACK",
    "COLOR_BRIGHT_RED",
    "COLOR_BRIGHT_GREEN",
    "COLOR_BRIGHT_YELLOW",
    "COLOR_BRIGHT_BLUE",
    "COLOR_BRIGHT_MAGENTA",
    "COLOR_BRIGHT_CYAN",
    "COLOR_BRIGHT_WHITE",
    "COLOR_ORANGE",
    "COLOR_PURPLE",
    "COLOR_PINK",
    "COLOR_BROWN",
];

/// Canonical names for 8-color ANSI terminal palette colors.
const VALID_8_COLORS: &[&str] = &[
    "COLOR_BLACK",
    "COLOR_RED",
    "COLOR_GREEN",
    "COLOR_YELLOW",
    "COLOR_BLUE",
    "COLOR_MAGENTA",
    "COLOR_CYAN",
    "COLOR_WHITE",
];

/// Canonical top-level color tokens supported inside each palette tier.
const VALID_BASE_COLOR_TOKENS: &[&str] = &[
    "background",
    "title",
    "text",
    "highlight",
    "highlight_text",
    "border",
    "divider",
    "accent",
    "selection",
    "whitespace",
    "shortcut_key",
    "help_text",
    "plugin",
    "gutter",
    "footer",
    "window_close_button",
    "scrollbar",
    "shadow",
    "toast_info",
    "toast_warning",
    "toast_error",
];

/// Valid field keys permitted in a theme's `window:` border configuration map.
const VALID_WINDOW_BORDER_KEYS: &[&str] = &[
    "border_horizontal",
    "border_vertical",
    "border_horizontal_top",
    "border_horizontal_bottom",
    "border_vertical_left",
    "border_vertical_right",
    "border_top_left",
    "border_top_right",
    "border_bottom_left",
    "border_bottom_right",
    "left_tee",
    "right_tee",
    "top_tee",
    "bottom_tee",
    "title_left_cap",
    "title_right_cap",
    "shadow_char",
    "gutter_border",
    "gutter_separator",
    "gutter_left_tee",
    "gutter_right_tee",
    "gutter_top_tee",
    "gutter_bottom_tee",
];

/// Valid option type string descriptors permitted in menu items and shortcuts.
const VALID_OPTION_TYPES: &[&str] = &[
    "command",
    "script",
    "submenu",
    "config",
    "toggle",
    "editor",
    "radio",
    "config_radio",
    "confirm",
    "message",
    "popup",
    "info",
    "python",
    "inject_block",
    "theme_selector",
    "notebook",
    "toast",
    "{divider}",
    "back",
    "exit",
    "action",
];

/// Command-line argument configuration for the `ymlcheck` schema linter.
#[derive(Parser, Debug)]
#[command(
    name = "ymlcheck",
    version,
    about = "YAML validator and schema linter for BashMenu"
)]
#[command(group(
    ArgGroup::new("mode")
        .args(["themes", "config", "menu", "shortcuts", "locale", "yaml"])
))]
struct Cli {
    /// Path to one or more YAML files to validate
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Validate as a theme palette file (bashmenu.themes)
    #[arg(short = 't', long = "themes", alias = "theme")]
    themes: bool,

    /// Validate as a settings configuration file (bashmenu.yml)
    #[arg(short = 'c', long = "config")]
    config: bool,

    /// Validate as a menu structure file (bashmenu.mnu)
    #[arg(short = 'm', long = "menu")]
    menu: bool,

    /// Validate as a custom keyboard shortcuts file (shortcuts.yml)
    #[arg(short = 's', long = "shortcuts", alias = "shortcut")]
    shortcuts: bool,

    /// Validate as a translation locale string table (e.g. locales/es_es.yml)
    #[arg(short = 'l', long = "locale")]
    locale: bool,

    /// Validate YAML syntax only with no schema enforcement (default)
    #[arg(short = 'y', long = "yaml")]
    yaml: bool,
}

/// Validates pure YAML parsing syntax without evaluating BashMenu-specific schemas.
fn validate_yaml_syntax(path: &Path) -> bool {
    println!("Checking YAML syntax for {}...", path.display());
    match fs::read_to_string(path) {
        Ok(content) => match serde_saphyr::from_str::<serde_json::Value>(&content) {
            Ok(_) => {
                println!("SUCCESS: '{}' contains valid YAML syntax!", path.display());
                true
            }
            Err(e) => {
                println!(
                    "\n[ERROR] YAML Syntax Error in '{}':\n{}",
                    path.display(),
                    e
                );
                false
            }
        },
        Err(e) => {
            println!("\n[ERROR] Could not read file '{}': {}", path.display(), e);
            false
        }
    }
}

/// Validates a single palette color element (number, `-1` inherit, hex, or ANSI constant).
fn validate_palette_scalar(
    elem: &serde_json::Value,
    tier_name: &str,
    valid_constants: Option<&HashSet<&str>>,
    ctx: &str,
    errors: &mut Vec<String>,
) {
    match elem {
        serde_json::Value::Number(n) => {
            let Some(i) = n.as_i64() else {
                errors.push(format!("{}: Color value must be an integer", ctx));
                return;
            };
            if i == -1 {
                return;
            }
            let max_idx = match tier_name {
                "8" => 7,
                "16" => 15,
                _ => 255,
            };
            if !(0..=max_idx).contains(&i) {
                errors.push(format!(
                    "{}: Color index {} out of range (expected -1 or 0..={})",
                    ctx, i, max_idx
                ));
            }
        }
        serde_json::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim == "-1" {
                return;
            }
            if tier_name == "256" {
                let is_hex = s_trim
                    .strip_prefix('#')
                    .is_some_and(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
                let is_idx = s_trim
                    .parse::<i64>()
                    .is_ok_and(|i| i == -1 || (0..=255).contains(&i));
                if !is_hex && !is_idx {
                    if let Some(valid16) = valid_constants {
                        if !valid16.contains(s_trim) {
                            errors.push(format!("{}: Invalid 256/TrueColor token '{}'", ctx, s));
                        }
                    } else {
                        errors.push(format!("{}: Invalid 256/TrueColor token '{}'", ctx, s));
                    }
                }
            } else if let Some(allowed) = valid_constants {
                if !allowed.contains(s_trim) {
                    errors.push(format!("{}: Invalid color constant '{}'", ctx, s));
                }
            }
        }
        _ => {
            errors.push(format!(
                "{}: Expected color integer or string constant",
                ctx
            ));
        }
    }
}

/// Validates a palette entry inside a tier (`256`, `16`, or `8`), including nested
/// activity override sub-maps.
#[allow(clippy::too_many_arguments)]
fn validate_palette_entry(
    key: &str,
    val: &serde_json::Value,
    theme_name: &str,
    tier_name: &str,
    activity_scope: Option<&str>,
    valid_base: &HashSet<&str>,
    valid_activities: &HashSet<&str>,
    valid_16: &HashSet<&str>,
    valid_8: &HashSet<&str>,
    errors: &mut Vec<String>,
) {
    let tier_constants = match tier_name {
        "8" => Some(valid_8),
        "16" => Some(valid_16),
        _ => Some(valid_16),
    };
    let path_ctx = match activity_scope {
        Some(act) => format!("'{}.{}.{}.{}'", theme_name, tier_name, act, key),
        None => format!("'{}.{}.{}'", theme_name, tier_name, key),
    };

    if let Some(act) = activity_scope {
        let allowed_sub = activity_override_tokens(act);
        if !allowed_sub.contains(&key) {
            errors.push(format!(
                "{}: Unknown or unsupported token '{}' in activity '{}'",
                path_ctx, key, act
            ));
            return;
        }
        if matches!(val, serde_json::Value::Object(_)) {
            errors.push(format!(
                "{}: Nested mappings are not allowed inside activity overrides",
                path_ctx
            ));
            return;
        }
    } else if valid_activities.contains(key) {
        let Some(sub_map) = val.as_object() else {
            errors.push(format!(
                "{}: Activity override section must be a YAML map",
                path_ctx
            ));
            return;
        };
        for (sk, sv) in sub_map {
            validate_palette_entry(
                sk.as_str(),
                sv,
                theme_name,
                tier_name,
                Some(key),
                valid_base,
                valid_activities,
                valid_16,
                valid_8,
                errors,
            );
        }
        return;
    } else if !valid_base.contains(key) {
        errors.push(format!(
            "{}: Unknown or legacy palette key '{}'",
            path_ctx, key
        ));
        return;
    }

    match val {
        serde_json::Value::Array(seq) => {
            if seq.len() != 2 {
                errors.push(format!("{}: Must be a 2-element list [fg, bg]", path_ctx));
                return;
            }
            for (i, elem) in seq.iter().enumerate() {
                let layer = if i == 0 { "fg" } else { "bg" };
                let elem_ctx = format!(
                    "'{}.{}.{}.{}.{}'",
                    theme_name,
                    tier_name,
                    activity_scope.unwrap_or_default(),
                    key,
                    layer
                )
                .replace("..", ".");
                validate_palette_scalar(elem, tier_name, tier_constants, &elem_ctx, errors);
            }
        }
        serde_json::Value::Number(_) | serde_json::Value::String(_) => {
            validate_palette_scalar(val, tier_name, tier_constants, &path_ctx, errors);
        }
        _ => {
            errors.push(format!("{}: Invalid color value type", path_ctx));
        }
    }
}

/// Validates a `bashmenu.themes` theme file against schema rules, palette color ranges, and border keys.
fn validate_theme_file(path: &Path) -> bool {
    println!("Validating theme file {}...", path.display());
    let themes = match load_themes(path) {
        Ok(t) => t,
        Err(e) => {
            println!("\n[ERROR] Theme File is Malformed!\n{}", e);
            return false;
        }
    };

    let mut errors = Vec::new();
    let valid_16: HashSet<&str> = VALID_16_COLORS.iter().copied().collect();
    let valid_8: HashSet<&str> = VALID_8_COLORS.iter().copied().collect();
    let valid_base: HashSet<&str> = VALID_BASE_COLOR_TOKENS.iter().copied().collect();
    let valid_activities: HashSet<&str> = ACTIVITY_OVERRIDE_KEYS.iter().copied().collect();
    let valid_borders: HashSet<&str> = VALID_WINDOW_BORDER_KEYS.iter().copied().collect();

    for (name, theme) in &themes {
        for (tier_name, palette_opt) in [
            ("256", &theme.colors_256),
            ("16", &theme.colors_16),
            ("8", &theme.colors_8),
        ] {
            if let Some(pal) = palette_opt {
                for (key, val) in pal {
                    validate_palette_entry(
                        key,
                        val,
                        name,
                        tier_name,
                        None,
                        &valid_base,
                        &valid_activities,
                        &valid_16,
                        &valid_8,
                        &mut errors,
                    );
                }
            }
        }

        // Validate window borders
        if let Some(borders) = &theme.window {
            if let Some(horiz) = &borders.border_horizontal {
                if horiz.is_empty() {
                    errors.push(format!(
                        "'{}.window.border_horizontal': cannot be empty",
                        name
                    ));
                }
            }
        }

        // Validate extra unknown border keys
        for key in theme.extra.keys() {
            if key.starts_with("border_") && !valid_borders.contains(key.as_str()) {
                errors.push(format!(
                    "'{}.window': Unknown border property '{}'",
                    name, key
                ));
            }
        }

        // Validate toast style if configured
        if let Some(toast) = &theme.toast {
            if let Some(style) = &toast.style {
                let norm = style.trim().to_ascii_lowercase();
                if !matches!(
                    norm.as_str(),
                    "bar" | "classic" | "default" | "framed" | "box" | "border"
                ) {
                    errors.push(format!(
                        "'{}.toast.style': Unknown toast style '{}' (expected 'bar' or 'framed')",
                        name, style
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        println!("\n[ERROR] Found {} validation issue(s):\n", errors.len());
        for e in &errors {
            println!("  - {}", e);
        }
        return false;
    }

    println!("SUCCESS: Theme file is valid!");
    true
}

/// Validates a `bashmenu.yml` configuration file against schema fields and setting bounds.
fn validate_config_file(path: &Path) -> bool {
    println!("Validating configuration file {}...", path.display());
    let config = match load_config(path) {
        Ok(c) => c,
        Err(e) => {
            println!("\n[ERROR] Configuration File is Malformed: {}", e);
            return false;
        }
    };

    let mut errors = Vec::new();
    if config.theme.is_empty() {
        errors.push("'theme' cannot be empty".to_string());
    }

    if let Some(tabstop) = config.settings.tabstop {
        if tabstop == 0 || tabstop > 16 {
            errors.push(format!(
                "'settings.tabstop' must be between 1 and 16, got {}",
                tabstop
            ));
        }
    }

    if !errors.is_empty() {
        println!(
            "\n[ERROR] Found {} config validation issue(s):\n",
            errors.len()
        );
        for e in &errors {
            println!("  - {}", e);
        }
        return false;
    }

    println!("SUCCESS: Configuration file is valid!");
    true
}

/// Validates a `bashmenu.mnu` menu schema file, recursively checking items and submenus.
fn validate_menu_file(path: &Path) -> bool {
    println!("Validating menu file {}...", path.display());
    let menu = match load_menu(path) {
        Ok(m) => m,
        Err(e) => {
            println!("\n[ERROR] Menu File is Malformed: {}", e);
            return false;
        }
    };

    let mut errors = Vec::new();
    let valid_types: HashSet<&str> = VALID_OPTION_TYPES.iter().copied().collect();

    if menu.items.is_empty() {
        errors.push("Menu definition has no items".to_string());
    }

    /// Recursively checks each menu item for type validity and submenu structure.
    fn check_items(
        items: &[bashmenu_core::MenuItem],
        path_prefix: &str,
        valid_types: &HashSet<&str>,
        errors: &mut Vec<String>,
    ) {
        for (idx, item) in items.iter().enumerate() {
            let item_path = format!("{}[{}]", path_prefix, idx);
            if item.is_divider() {
                continue;
            }

            // Check item type
            match &item.r#type {
                bashmenu_core::MenuItemType::Other(s) => {
                    if !valid_types.contains(s.as_str()) {
                        errors.push(format!(
                            "'{}.{}': Unknown option type '{}'",
                            item_path, item.label, s
                        ));
                    }
                }
                bashmenu_core::MenuItemType::Radio => {
                    if !item.extra.contains_key("key") && item.action.is_none() {
                        errors.push(format!(
                            "'{}.{}': Radio item requires a 'key' property",
                            item_path, item.label
                        ));
                    }
                }
                bashmenu_core::MenuItemType::Submenu => {
                    if let Some(sub) = &item.submenu {
                        check_items(
                            &sub.items,
                            &format!("{}.submenu", item_path),
                            valid_types,
                            errors,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    check_items(&menu.items, "items", &valid_types, &mut errors);

    if !errors.is_empty() {
        println!(
            "\n[ERROR] Found {} menu validation issue(s):\n",
            errors.len()
        );
        for e in &errors {
            println!("  - {}", e);
        }
        return false;
    }

    println!("SUCCESS: Menu file is valid!");
    true
}

/// Validates a `shortcuts.yml` keyboard shortcut definitions file.
fn validate_shortcuts_file(path: &Path) -> bool {
    println!("Validating shortcuts file {}...", path.display());
    let cfg = match load_shortcuts(path) {
        Ok(c) => c,
        Err(e) => {
            println!(
                "[ERROR] Shortcuts schema error in '{}': {}",
                path.display(),
                e
            );
            return false;
        }
    };

    let mut errors = Vec::new();
    if let Err(err) = validate_key_spec(&cfg.leader_key, true) {
        errors.push(format!("Invalid leader_key '{}': {}", cfg.leader_key, err));
    }

    let mut seen_keys = HashSet::new();
    for (idx, entry) in cfg.shortcuts.iter().enumerate() {
        let loc = format!("shortcuts[{}] ('{}')", idx, entry.resolved_description());
        if entry.description.trim().is_empty() && entry.builtin_macro().is_none() {
            errors.push(format!("{}: 'description' must not be empty.", loc));
        }
        match validate_key_spec(&entry.key, false) {
            Ok(norm) => {
                if entry.enabled && !seen_keys.insert(norm.clone()) {
                    errors.push(format!("{}: Duplicate active key binding '{}'.", loc, norm));
                }
            }
            Err(err) => {
                errors.push(format!("{}: Invalid key '{}': {}", loc, entry.key, err));
            }
        }
        if let Some(pct) = entry.window_percent {
            if !(20..=100).contains(&pct) {
                errors.push(format!(
                    "{}: 'window_percent' ({}) must be between 20 and 100.",
                    loc, pct
                ));
            }
        }
    }

    if !errors.is_empty() {
        println!(
            "\n[ERROR] Found {} shortcuts validation issue(s):\n",
            errors.len()
        );
        for e in &errors {
            println!("  - {}", e);
        }
        return false;
    }

    println!(
        "SUCCESS: Shortcuts file is valid ({} shortcut(s), leader_key: '{}')!",
        cfg.shortcuts.len(),
        cfg.leader_key
    );
    true
}

/// Validates a locale string table YAML against the canonical English schema and variable placeholders.
fn validate_locale_file(path: &Path) -> bool {
    println!("Validating locale string table: {}...", path.display());
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            println!("\n[ERROR] Could not read file '{}': {}", path.display(), e);
            return false;
        }
    };

    let target_map = match parse_and_flatten_yaml(&content) {
        Ok(m) => m,
        Err(e) => {
            println!(
                "\n[ERROR] YAML Syntax Error in '{}':\n{}",
                path.display(),
                e
            );
            return false;
        }
    };

    let catalog = LocaleCatalog::new();
    let default_map = catalog.default_strings();

    let mut missing_keys: Vec<&String> = Vec::new();
    let mut variable_mismatches: Vec<String> = Vec::new();

    let extract_vars = |s: &str| -> HashSet<String> {
        let mut vars = HashSet::new();
        let mut chars = s.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '{' {
                let mut var_name = String::new();
                for inner in chars.by_ref() {
                    if inner == '}' {
                        if !var_name.is_empty() {
                            vars.insert(var_name);
                        }
                        break;
                    }
                    var_name.push(inner);
                }
            }
        }
        vars
    };

    for (key, default_val) in default_map {
        if let Some(target_val) = target_map.get(key) {
            let def_vars = extract_vars(default_val);
            let target_vars = extract_vars(target_val);
            if def_vars != target_vars {
                variable_mismatches.push(format!(
                    "Key '{}': expected placeholder variables {:?}, found {:?}",
                    key, def_vars, target_vars
                ));
            }
        } else {
            missing_keys.push(key);
        }
    }

    missing_keys.sort();
    variable_mismatches.sort();

    let mut passed = true;
    if !missing_keys.is_empty() {
        println!(
            "\n[ERROR] Locale file '{}' is missing {} canonical key(s):",
            path.display(),
            missing_keys.len()
        );
        for k in &missing_keys {
            println!("  - {}", k);
        }
        passed = false;
    }

    if !variable_mismatches.is_empty() {
        println!(
            "\n[ERROR] Locale file '{}' has {} variable interpolation mismatch(es):",
            path.display(),
            variable_mismatches.len()
        );
        for mismatch in &variable_mismatches {
            println!("  - {}", mismatch);
        }
        passed = false;
    }

    let extra_keys: Vec<&String> = target_map
        .keys()
        .filter(|k| !default_map.contains_key(*k))
        .collect();
    if !extra_keys.is_empty() {
        println!(
            "\n[WARNING] Locale file '{}' defines {} unknown key(s) not in canonical en_us.yml:",
            path.display(),
            extra_keys.len()
        );
        for k in extra_keys {
            println!("  + {}", k);
        }
    }

    if passed {
        println!(
            "SUCCESS: '{}' is a valid locale string table with 100% key parity ({} keys validated)!",
            path.display(),
            target_map.len()
        );
    }

    passed
}

/// CLI entrypoint for `ymlcheck`.
fn main() {
    let cli = Cli::parse();

    let mut all_passed = true;
    for file in &cli.files {
        if !file.exists() {
            println!("[ERROR] File not found: '{}'", file.display());
            all_passed = false;
            continue;
        }

        let passed = if cli.themes {
            validate_theme_file(file)
        } else if cli.config {
            validate_config_file(file)
        } else if cli.menu {
            validate_menu_file(file)
        } else if cli.shortcuts {
            validate_shortcuts_file(file)
        } else if cli.locale {
            validate_locale_file(file)
        } else {
            validate_yaml_syntax(file)
        };

        if !passed {
            all_passed = false;
        }
    }

    if !all_passed {
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_option_types_contains_toast_and_inject_block() {
        assert!(
            VALID_OPTION_TYPES.contains(&"toast"),
            "VALID_OPTION_TYPES must contain toast"
        );
        assert!(
            VALID_OPTION_TYPES.contains(&"inject_block"),
            "VALID_OPTION_TYPES must contain inject_block"
        );
        assert!(
            VALID_OPTION_TYPES.contains(&"message"),
            "VALID_OPTION_TYPES must contain message"
        );
    }

    #[test]
    fn test_validate_menu_schema_with_toast_and_inject_block() {
        let raw_yaml = r#"
title: Test Menu
items:
  - label: Show Toast
    type: toast
    message: "Operation completed successfully"
  - label: Setup Environment
    type: inject_block
    template: "{templates_dir}/env.tmpl"
    target: "~/.bashrc"
    block_id: "my_env"
"#;
        let menu: bashmenu_core::MenuSchema =
            serde_saphyr::from_str(raw_yaml).expect("Must parse test menu");
        let valid_types: std::collections::HashSet<&str> =
            VALID_OPTION_TYPES.iter().copied().collect();
        let mut errors = Vec::new();

        for (idx, item) in menu.items.iter().enumerate() {
            if let bashmenu_core::MenuItemType::Other(ref s) = item.r#type {
                if !valid_types.contains(s.as_str()) {
                    errors.push(format!("Unknown option type: {s} at index {idx}"));
                }
            }
        }
        assert!(
            errors.is_empty(),
            "Validation must pass without unknown option type errors: {:?}",
            errors
        );
    }
}
