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
