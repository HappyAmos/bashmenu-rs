use bashmenu_core::{format_icon_slot, interpolate_placeholders, load_config, MacroContext};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

#[test]
fn test_status_gutter_interpolation() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/bashmenu.yml");
    let config = load_config(fixture).expect("load config");

    let mut ctx = MacroContext::new(&config);
    ctx.window_width = 100;
    ctx.screen_width = 100;

    let template = config
        .settings
        .status_gutter
        .as_deref()
        .expect("status gutter template");
    let interpolated = interpolate_placeholders(template, &ctx);

    // Gutter template: '| Theme: {theme} | {user} | Battery: ... | {user.localip} | [b]{date_time_24_short}[/b]'
    assert!(
        interpolated.contains("Theme: alaska-mountain"),
        "Theme should be interpolated: {}",
        interpolated
    );
    assert!(
        !interpolated.contains("{theme}"),
        "Raw placeholder should be resolved: {}",
        interpolated
    );
}

#[test]
fn test_icon_slot_4_columns_strict_invariant() {
    // 2-column emoji (rocket, game, globe)
    for emoji in &["🚀", "🎮", "🌐"] {
        let slot = format_icon_slot(Some(emoji), true);
        assert_eq!(
            UnicodeWidthStr::width(slot.as_str()),
            4,
            "Emoji slot must occupy exactly 4 columns: {:?}",
            slot
        );
        assert!(
            slot.ends_with("  "),
            "Emoji slot must have 2 trailing spaces"
        );
    }

    // 1-column symbols (gear, info, hash)
    for sym in &["⚙", "ℹ", "#"] {
        let slot = format_icon_slot(Some(sym), true);
        assert_eq!(
            UnicodeWidthStr::width(slot.as_str()),
            4,
            "1-col symbol slot must occupy exactly 4 columns: {:?}",
            slot
        );
        assert!(
            slot.ends_with("   "),
            "Symbol slot must have 3 trailing spaces"
        );
    }

    // No icon
    let empty_slot = format_icon_slot(None, true);
    assert_eq!(
        UnicodeWidthStr::width(empty_slot.as_str()),
        4,
        "Empty icon slot must occupy exactly 4 columns"
    );
    assert_eq!(empty_slot, "    ");
}

#[test]
fn test_right_aligned_bracket_placeholders_expansion() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/bashmenu.yml");
    let mut config = load_config(fixture).expect("load config");
    config.settings.ping_target = Some("1.1.1.1".to_string());
    config.settings.check_for_updates = Some(false);
    config.settings.nerd_font_width = Some("auto".to_string());
    config.settings.tab_to_spaces = Some(true);
    config.settings.tabstop = Some(4);
    config.settings.pager_editor = Some("vim".to_string());
    config.settings.refresh_interval = Some(serde_yml::Value::Number(serde_yml::Number::from(10)));
    config.user.insert(
        "postal_code".to_string(),
        serde_yml::Value::String("49079".to_string()),
    );

    let mut ctx = MacroContext::new(&config);
    ctx.bashmenu_dir = std::path::PathBuf::from("/home/user/.config/bashmenu");

    let test_cases = vec![
        (
            "Configure Ping Target [{settings.ping_target}]",
            "Configure Ping Target [1.1.1.1]",
        ),
        (
            "Enable Automatic Updates (git pull) [{settings.check_for_updates}]",
            "Enable Automatic Updates (git pull) [false]",
        ),
        (
            "Nerd Font Display Width [{settings.nerd_font_width}]",
            "Nerd Font Display Width [auto]",
        ),
        (
            "Editor Tab to Spaces Mode [{settings.tab_to_spaces}]",
            "Editor Tab to Spaces Mode [true]",
        ),
        (
            "Editor Tabstop Distance [{settings.tabstop}]",
            "Editor Tabstop Distance [4]",
        ),
        (
            "Pager Text Editor [{settings.pager_editor}]",
            "Pager Text Editor [vim]",
        ),
        (
            "Status Gutter Refresh Rate [[{app.settings.refresh_interval}] seconds]",
            "Status Gutter Refresh Rate [[10] seconds]",
        ),
        (
            "Weather Location [{user.postal_code}]",
            "Weather Location [49079]",
        ),
        (
            "Notes [{notebooks_dir}/notes.yml]",
            "Notes [/home/user/.config/bashmenu/notebooks/notes.yml]",
        ),
    ];

    for (input, expected) in test_cases {
        let expanded = interpolate_placeholders(input, &ctx);
        assert_eq!(expanded, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_profile_and_editor_placeholders_expansion() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/bashmenu.yml");
    let config = load_config(fixture).expect("load config");

    let mut ctx = MacroContext::new(&config);
    ctx.home_dir = std::path::PathBuf::from("/home/testuser");

    let test_cases = vec![
        ("{bashrc}", "/home/testuser/.bashrc"),
        ("{bash_aliases}", "/home/testuser/.bash_aliases"),
        ("{vimrc}", "/home/testuser/.vimrc"),
        ("{nanorc}", "/home/testuser/.nanorc"),
        ("{zshrc}", "/home/testuser/.zshrc"),
        ("{zprofile}", "/home/testuser/.zprofile"),
        (
            "Edit .bashrc [{bashrc}]",
            "Edit .bashrc [/home/testuser/.bashrc]",
        ),
        (
            "Edit .bash_aliases [{bash_aliases}]",
            "Edit .bash_aliases [/home/testuser/.bash_aliases]",
        ),
        (
            "Edit .vimrc [{vimrc}]",
            "Edit .vimrc [/home/testuser/.vimrc]",
        ),
        (
            "{termux_properties}",
            "/home/testuser/.termux/termux.properties",
        ),
        ("{termux_storage}", "/home/testuser/storage"),
    ];

    for (input, expected) in test_cases {
        let expanded = interpolate_placeholders(input, &ctx);
        assert_eq!(expanded, expected, "Failed for input: {}", input);
    }

    // Verify 12-hour time placeholders are non-empty and expand
    let t12 = interpolate_placeholders("{time_12}", &ctx);
    assert!(
        t12.contains("AM") || t12.contains("PM"),
        "time_12 must include AM/PM: {}",
        t12
    );

    let dt12 = interpolate_placeholders("{date_time_12_short}", &ctx);
    assert!(
        dt12.contains("AM") || dt12.contains("PM"),
        "date_time_12_short must include AM/PM: {}",
        dt12
    );
}
