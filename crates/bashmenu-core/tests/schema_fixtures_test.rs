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
