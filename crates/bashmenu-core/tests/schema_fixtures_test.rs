use bashmenu_core::{load_config, load_menu, load_themes, save_config, save_menu, MenuItemType};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path(filename: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("../../tests/fixtures").join(filename);
    if path.exists() {
        path
    } else {
        PathBuf::from("tests/fixtures").join(filename)
    }
}

#[test]
fn test_parse_production_bashmenu_yml() {
    let fixture = fixture_path("bashmenu.yml");
    assert!(
        fixture.exists(),
        "bashmenu.yml fixture must exist at {:?}",
        fixture
    );

    let config = load_config(&fixture).expect("Failed to parse production bashmenu.yml");
    assert_eq!(config.version, "0.0.1");
    assert_eq!(config.theme, "alaska-mountain");
    assert_eq!(config.settings.tabstop, Some(4));
    assert_eq!(config.settings.use_nerd_fonts, Some(true));
    assert_eq!(
        config.settings.scripts_dir,
        Some("{bashmenu_dir}/scripts".to_string())
    );

    // Verify roundtrip save preserves the header
    let tmp = std::env::temp_dir().join("test_bashmenu_roundtrip.yml");
    save_config(&tmp, &config).expect("Failed to save config");
    let written = fs::read_to_string(&tmp).expect("Failed to read back saved config");
    assert!(written.starts_with(
        "# =============================================================================="
    ));
    assert!(written.contains("BashMenu Configuration (bashmenu.yml)"));
    assert!(written.contains("theme: alaska-mountain"));
    let _ = fs::remove_file(tmp);
}

#[test]
fn test_parse_production_bashmenu_mnu() {
    let fixture = fixture_path("bashmenu.mnu");
    assert!(
        fixture.exists(),
        "bashmenu.mnu fixture must exist at {:?}",
        fixture
    );

    let menu = load_menu(&fixture).expect("Failed to parse production bashmenu.mnu");
    assert_eq!(menu.version, "0.0.1");
    assert_eq!(menu.title, "Bash Menu");
    assert!(!menu.items.is_empty(), "Items list must not be empty");

    // Item 0 is divider
    assert!(menu.items[0].is_divider(), "First item must be a divider");

    // Item 1 is submenu
    let dev_shortcuts = &menu.items[1];
    assert_eq!(dev_shortcuts.label, "BM Developer Shortcuts");
    assert_eq!(dev_shortcuts.r#type, MenuItemType::Submenu);
    let sub = dev_shortcuts
        .submenu
        .as_ref()
        .expect("Submenu must be present");
    assert!(!sub.items.is_empty(), "Submenu items must not be empty");

    // Check an editor item inside submenu
    let edit_item = sub
        .items
        .iter()
        .find(|i| i.label.starts_with("Edit bashmenu.md"));
    assert!(edit_item.is_some(), "Edit bashmenu.md item must exist");
    let item = edit_item.unwrap();
    assert_eq!(item.r#type, MenuItemType::Editor);
    assert_eq!(item.file, Some("{bashmenu_dir}/bashmenu.md".to_string()));
    assert_eq!(item.tabstop, Some(4));

    // Verify roundtrip save
    let tmp = std::env::temp_dir().join("test_bashmenu_roundtrip.mnu");
    save_menu(&tmp, &menu).expect("Failed to save menu");
    let reloaded = load_menu(&tmp).expect("Failed to reload saved menu");
    assert_eq!(reloaded.title, menu.title);
    assert_eq!(reloaded.items.len(), menu.items.len());
    let _ = fs::remove_file(tmp);
}

#[test]
fn test_menu_item_execution_mode_and_window_percent() {
    use bashmenu_core::{ExecutionMode, MenuItem};

    // 1. Streaming via stream: true
    let mut item_stream = MenuItem::default();
    item_stream
        .extra
        .insert("stream".to_string(), serde_yml::Value::Bool(true));
    assert_eq!(item_stream.execution_mode(), ExecutionMode::Streaming);
    assert!(item_stream.is_streaming());
    assert_eq!(item_stream.window_percent(), 65);

    // 2. Streaming via interactive: false
    let item_non_interactive = MenuItem {
        interactive: Some(false),
        ..Default::default()
    };
    assert_eq!(
        item_non_interactive.execution_mode(),
        ExecutionMode::Streaming
    );
    assert!(item_non_interactive.is_streaming());

    // 3. Interactive via exec_mode: "interactive" and window_percent: 80
    let mut item_interact = MenuItem::default();
    item_interact.extra.insert(
        "exec_mode".to_string(),
        serde_yml::Value::String("interactive".to_string()),
    );
    item_interact.extra.insert(
        "window_percent".to_string(),
        serde_yml::Value::Number(80.into()),
    );
    assert_eq!(item_interact.execution_mode(), ExecutionMode::Interactive);
    assert_eq!(item_interact.window_percent(), 80);

    // 4. Console mode (default or interactive: true)
    let item_console = MenuItem {
        interactive: Some(true),
        ..Default::default()
    };
    assert_eq!(item_console.execution_mode(), ExecutionMode::Console);
    assert_eq!(item_console.window_percent(), 65);
}

#[test]
fn test_parse_production_bashmenu_themes() {
    let fixture = fixture_path("bashmenu.themes");
    assert!(
        fixture.exists(),
        "bashmenu.themes fixture must exist at {:?}",
        fixture
    );

    let themes = load_themes(&fixture).expect("Failed to parse production bashmenu.themes");
    assert!(themes.len() >= 25, "Must parse all production themes");

    // Verify key themes
    for theme_name in &[
        "dracula",
        "nord",
        "cyberpunk",
        "gruvbox",
        "matrix",
        "alaska-mountain",
    ] {
        assert!(
            themes.contains_key(*theme_name),
            "Theme '{}' must exist",
            theme_name
        );
        let t = &themes[*theme_name];
        assert!(
            t.colors_256.is_some(),
            "Theme '{}' must have 256 color palette",
            theme_name
        );
        assert!(
            t.window.is_some(),
            "Theme '{}' must have window borders",
            theme_name
        );
    }
}

#[test]
fn test_config_get_set_value_and_menu_item_helpers() {
    let fixture = fixture_path("bashmenu.yml");
    let mut config = load_config(&fixture).expect("Failed to parse production bashmenu.yml");

    // Test get_value
    assert_eq!(
        config.get_value("theme"),
        Some("alaska-mountain".to_string())
    );
    assert_eq!(
        config.get_value("settings.use_nerd_fonts"),
        Some("true".to_string())
    );
    assert_eq!(
        config.get_value("app.settings.tabstop"),
        Some("4".to_string())
    );

    // Test set_value
    config.set_value("settings.use_nerd_fonts", serde_yml::Value::Bool(false));
    assert_eq!(config.settings.use_nerd_fonts, Some(false));
    assert_eq!(
        config.get_value("settings.use_nerd_fonts"),
        Some("false".to_string())
    );

    config.set_value(
        "settings.ping_target",
        serde_yml::Value::String("8.8.4.4".to_string()),
    );
    assert_eq!(config.settings.ping_target, Some("8.8.4.4".to_string()));
    assert_eq!(
        config.get_value("settings.ping_target"),
        Some("8.8.4.4".to_string())
    );

    config.set_value(
        "settings.pager_editor",
        serde_yml::Value::String("nano".to_string()),
    );
    assert_eq!(config.settings.pager_editor, Some("nano".to_string()));

    // Test MenuItem helpers
    let menu = load_menu(fixture_path("bashmenu.mnu")).expect("load menu");
    let mut found_nerd = false;
    let mut found_pager = false;

    fn check_items(
        items: &[bashmenu_core::MenuItem],
        found_nerd: &mut bool,
        found_pager: &mut bool,
    ) {
        for it in items {
            if let Some(key) = it.key() {
                if key == "settings.use_nerd_fonts" {
                    *found_nerd = true;
                    assert!(it.title().is_some() || it.message().is_some());
                } else if key == "settings.pager_editor" {
                    *found_pager = true;
                    let opts = it.options();
                    assert!(
                        !opts.is_empty(),
                        "Options must not be empty for pager_editor"
                    );
                }
            }
            if let Some(sub) = &it.submenu {
                check_items(&sub.items, found_nerd, found_pager);
            }
        }
    }

    check_items(&menu.items, &mut found_nerd, &mut found_pager);
    assert!(found_nerd, "Enable Nerd Fonts item with key must be found");
    assert!(found_pager, "Pager editor item with options must be found");
}

#[test]
fn test_menu_item_param_placeholder_and_masked() {
    use bashmenu_core::MenuItem;

    let mut item = MenuItem {
        label: "Which Command [which [param]]".to_string(),
        action: Some("which {param}".to_string()),
        command: Some("which {param}".to_string()),
        prompt: Some("Enter command name to locate:".to_string()),
        ..Default::default()
    };
    item.extra
        .insert("masked".to_string(), serde_yml::Value::Bool(true));

    assert!(item.has_param_placeholder());
    assert!(item.masked());
    assert_eq!(item.param_modal_title(), "Which Command [which [param]]");
    assert_eq!(item.param_modal_prompt(), "Enter command name to locate:");

    item.apply_param_placeholder("cargo");
    assert!(!item.has_param_placeholder());
    assert_eq!(item.action.as_deref(), Some("which cargo"));
    assert_eq!(item.command.as_deref(), Some("which cargo"));
}
