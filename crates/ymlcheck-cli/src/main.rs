//! ymlcheck - Theme and YAML Configuration Validator for BashMenu.
//!
//! Validates YAML files for syntax or specific schema rules (themes, config, menu).

use bashmenu_core::{load_config, load_menu, load_themes};
use clap::{ArgGroup, Parser};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

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

#[derive(Parser, Debug)]
#[command(
    name = "ymlcheck",
    version,
    about = "YAML validator and schema linter for BashMenu"
)]
#[command(group(
    ArgGroup::new("mode")
        .args(["themes", "config", "menu", "yaml"])
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

    /// Validate YAML syntax only with no schema enforcement (default)
    #[arg(short = 'y', long = "yaml")]
    yaml: bool,
}

fn validate_yaml_syntax(path: &Path) -> bool {
    println!("Checking YAML syntax for {}...", path.display());
    match fs::read_to_string(path) {
        Ok(content) => match serde_yml::from_str::<serde_yml::Value>(&content) {
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
    let valid_borders: HashSet<&str> = VALID_WINDOW_BORDER_KEYS.iter().copied().collect();

    for (name, theme) in &themes {
        // Validate 256 colors
        if let Some(c256) = &theme.colors_256 {
            for (key, val) in c256 {
                if let serde_yml::Value::Sequence(seq) = val {
                    if seq.len() != 2 {
                        errors.push(format!(
                            "'{}.256.{}': Must be a 2-element list [fg, bg]",
                            name, key
                        ));
                    }
                }
            }
        }

        // Validate 16 colors
        if let Some(c16) = &theme.colors_16 {
            for (key, val) in c16 {
                if let serde_yml::Value::Sequence(seq) = val {
                    for (i, elem) in seq.iter().enumerate() {
                        let layer = if i == 0 { "fg" } else { "bg" };
                        if let serde_yml::Value::String(s) = elem {
                            if s != "-1" && !valid_16.contains(s.as_str()) {
                                errors.push(format!(
                                    "'{}.16.{}.{}': Invalid color constant '{}'",
                                    name, key, layer, s
                                ));
                            }
                        }
                    }
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
