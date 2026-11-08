use bashmenu_core::{load_config, load_menu, load_themes, save_config, save_menu, MenuItemType};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path(filename: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("../..").join(filename);
    if path.exists() {
        path
    } else {
        PathBuf::from(filename)
    }
}

#[test]
fn test_libyml_string_join_overflow_bug_fixed() {
    let input = "description: 'This is a long string that spans across multiple lines\n\nand has trailing breaks and spaces to ensure JOIN macro extends capacity without panic'\n";
    let val: serde_yml::Value =
        serde_yml::from_str(input).expect("Should parse multiline flow scalar");
    assert!(val.get("description").is_some());
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
    assert_eq!(config.theme, "canopy");
    assert_eq!(config.settings.tabstop, Some(4));
    assert_eq!(config.settings.use_nerd_fonts, Some(true));
    assert_eq!(
        config.settings.scripts_dir,
        Some("{bashmenu_dir}/scripts".to_string())
    );
    assert_eq!(config.settings.transition, Some("blinds".to_string()));
    assert_eq!(config.settings.transition_speed, Some("normal".to_string()));
    assert_eq!(config.settings.modal_animation, Some("zoom".to_string()));
    assert_eq!(
        config.settings.modal_animation_speed,
        Some("fast".to_string())
    );

    // Verify roundtrip save preserves the header
    let tmp = std::env::temp_dir().join("test_bashmenu_roundtrip.yml");
    save_config(&tmp, &config).expect("Failed to save config");
    let written = fs::read_to_string(&tmp).expect("Failed to read back saved config");
    assert!(written.starts_with(
        "# =============================================================================="
    ));
    assert!(written.contains("BashMenu Configuration (bashmenu.yml)"));
    assert!(written.contains("theme: canopy"));
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

    // Item 1 is notebook
    let notes_item = &menu.items[1];
    assert_eq!(notes_item.label, "Notes [{notebooks_dir}/notes.yml]");
    assert_eq!(notes_item.r#type, MenuItemType::Notebook);

    // Item 3 is submenu
    let app_submenu = &menu.items[3];
    assert_eq!(app_submenu.label, "Applications");
    assert_eq!(app_submenu.r#type, MenuItemType::Submenu);
    let sub = app_submenu
        .submenu
        .as_ref()
        .expect("Submenu must be present");
    assert!(!sub.items.is_empty(), "Submenu items must not be empty");

    // Check script item inside submenu
    let script_item = sub
        .items
        .iter()
        .find(|i| i.label.starts_with("Search Nerd Font & Emoji"));
    assert!(
        script_item.is_some(),
        "Search Nerd Font & Emoji item must exist"
    );
    let item = script_item.unwrap();
    assert_eq!(item.r#type, MenuItemType::Script);
    assert_eq!(
        item.action,
        Some("{scripts_dir}/glyphs.sh {param}".to_string())
    );

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
    assert_eq!(config.get_value("theme"), Some("canopy".to_string()));
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

#[test]
fn test_parse_production_shortcuts_yml_and_inline_param() {
    use bashmenu_core::{load_shortcuts, save_shortcuts, ExecutionMode};

    let fixture = fixture_path("shortcuts.yml");
    assert!(
        fixture.exists(),
        "shortcuts.yml fixture must exist at {:?}",
        fixture
    );

    let cfg = load_shortcuts(&fixture).expect("Failed to parse shortcuts.yml fixture");
    assert_eq!(cfg.leader_key, "ctrl+space");
    assert_eq!(cfg.leader_timeout_ms, 1500);
    assert!(cfg.show_leader_hint);
    assert!(!cfg.shortcuts.is_empty());

    // Check inline {param:prompt=...} on the Ping shortcut
    let ping_entry = cfg
        .shortcuts
        .iter()
        .find(|s| s.key == "<leader> p")
        .expect("<leader> p shortcut must exist");
    let mut ping_item = ping_entry.to_menu_item();
    assert_eq!(ping_item.execution_mode(), ExecutionMode::Streaming);
    assert!(ping_item.has_param_placeholder());
    assert_eq!(ping_item.param_modal_prompt(), "Enter Host or IP:");
    ping_item.apply_param_placeholder("1.1.1.1");
    assert!(!ping_item.has_param_placeholder());
    assert_eq!(ping_item.action.as_deref(), Some("ping -c 4 1.1.1.1"));

    // Roundtrip save preserves SHORTCUTS_HEADER
    let tmp = std::env::temp_dir().join("test_shortcuts_roundtrip.yml");
    save_shortcuts(&tmp, &cfg).expect("Failed to save shortcuts.yml");
    let written = fs::read_to_string(&tmp).expect("Failed to read saved shortcuts.yml");
    assert!(written.starts_with(
        "# =============================================================================="
    ));
    assert!(written.contains("BashMenu Custom Keyboard Shortcuts (shortcuts.yml)"));
    let reloaded = load_shortcuts(&tmp).expect("Failed to reload saved shortcuts.yml");
    assert_eq!(reloaded.shortcuts.len(), cfg.shortcuts.len());
    let _ = fs::remove_file(tmp);
}

#[test]
fn test_inject_block_marker_injection_and_removal_cycle() {
    let raw_yaml = r#"
title: Test Inject Block
items:
  - label: Configure Sudo Prompt
    type: inject_block
    template: "{templates_dir}/sudo_lecture.tmpl"
    target: "/tmp/test_target_profile"
    block_id: "sudo_lecture_setting"
    user_mode: "root"
"#;
    let schema: bashmenu_core::MenuSchema =
        serde_yml::from_str(raw_yaml).expect("Must parse inject_block");
    let item = &schema.items[0];
    assert_eq!(item.label, "Configure Sudo Prompt");
    assert_eq!(item.r#type, MenuItemType::Other("inject_block".to_string()));
    assert_eq!(
        item.extra.get("template").and_then(|v| v.as_str()),
        Some("{templates_dir}/sudo_lecture.tmpl")
    );
    assert_eq!(
        item.extra.get("target").and_then(|v| v.as_str()),
        Some("/tmp/test_target_profile")
    );
    let block_id = item
        .extra
        .get("block_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    assert_eq!(block_id, "sudo_lecture_setting");
    assert_eq!(
        item.extra.get("user_mode").and_then(|v| v.as_str()),
        Some("root")
    );

    // Verify boundary markers format
    let start_marker = format!("# CODEBLOCK:{}:START", block_id);
    let end_marker = format!("# CODEBLOCK:{}:END", block_id);
    assert_eq!(start_marker, "# CODEBLOCK:sudo_lecture_setting:START");
    assert_eq!(end_marker, "# CODEBLOCK:sudo_lecture_setting:END");

    // Simulate injection into target file
    let initial_content = "# Existing profile header\nexport FOO=1\n";
    let template_body = "Defaults lecture=always\n";
    let block_to_inject = format!(
        "\n{}\n{}\n{}\n",
        start_marker,
        template_body.trim(),
        end_marker
    );

    let mut injected = initial_content.to_string();
    injected.push_str(&block_to_inject);
    assert!(injected.contains(&start_marker));
    assert!(injected.contains(&end_marker));
    assert!(injected.contains("Defaults lecture=always"));

    // Simulate update/reinstallation
    let updated_template_body = "Defaults lecture=never\n";
    let updated_block = format!(
        "\n{}\n{}\n{}\n",
        start_marker,
        updated_template_body.trim(),
        end_marker
    );
    let s_idx = injected.find(&start_marker).unwrap();
    let e_idx = injected.find(&end_marker).unwrap() + end_marker.len();
    let updated = format!(
        "{}{}{}",
        &injected[..s_idx],
        updated_block.trim(),
        &injected[e_idx..]
    );
    assert!(updated.contains("Defaults lecture=never"));
    assert!(!updated.contains("Defaults lecture=always"));

    // Simulate uninstallation (clean removal)
    let rem_s = updated.find(&start_marker).unwrap();
    let rem_e = updated.find(&end_marker).unwrap() + end_marker.len();
    let mut s_cut = rem_s;
    if s_cut > 0 && updated.as_bytes()[s_cut - 1] == b'\n' {
        s_cut -= 1;
    }
    let mut e_cut = rem_e;
    if e_cut < updated.len() && updated.as_bytes()[e_cut] == b'\n' {
        e_cut += 1;
    }
    let uninstalled = format!("{}{}", &updated[..s_cut], &updated[e_cut..]);
    assert!(!uninstalled.contains(&start_marker));
    assert!(!uninstalled.contains(&end_marker));
    assert_eq!(uninstalled.trim(), initial_content.trim());
}

#[test]
fn test_inactivity_timeout_and_root_elevation_schema() {
    let raw_config = r#"
version: "0.0.1"
theme: "classic"
settings:
  inactivity_timeout:
    milliseconds: 300000
    command: "{scripts_dir}/screensaver.sh"
"#;
    let config: bashmenu_core::BashMenuConfig =
        serde_yml::from_str(raw_config).expect("Must parse config with inactivity_timeout");
    let timeout = config
        .settings
        .inactivity_timeout
        .expect("Timeout must be present");
    assert_eq!(timeout.milliseconds, Some(300000));
    assert_eq!(
        timeout.command.as_deref(),
        Some("{scripts_dir}/screensaver.sh")
    );
}
