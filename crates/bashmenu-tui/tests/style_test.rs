use bashmenu_core::Theme;
use bashmenu_tui::{
    draw_box_drop_shadow, rgb_to_ansi16_name, ColorTier, ResolvedThemeStyles, ThemeActivity,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde_yml::{Mapping, Value};

fn set_activity_override(theme: &mut Theme, activity: ThemeActivity, key: &str, val: Value) {
    let palette = theme.colors_256.get_or_insert_with(Default::default);
    let act_key = activity.yaml_key();
    let entry = palette
        .entry(act_key.to_string())
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    if let Value::Mapping(map) = entry {
        map.insert(Value::String(key.to_string()), val);
    }
}

fn pair(fg: i64, bg: i64) -> Value {
    Value::Sequence(vec![Value::Number(fg.into()), Value::Number(bg.into())])
}

#[test]
fn test_activity_override_and_inherit_resolution() {
    let mut theme = Theme::monochrome();

    // Partial & full overrides per activity
    set_activity_override(
        &mut theme,
        ThemeActivity::BashEdit,
        "background",
        Value::Number(19.into()),
    );
    set_activity_override(
        &mut theme,
        ThemeActivity::BashEdit,
        "help_text",
        pair(220, -1),
    );
    set_activity_override(
        &mut theme,
        ThemeActivity::BashEdit,
        "selection",
        pair(-1, 52),
    );
    set_activity_override(&mut theme, ThemeActivity::Modals, "border", pair(81, -1));
    set_activity_override(&mut theme, ThemeActivity::Modals, "shadow", pair(240, -1));
    set_activity_override(&mut theme, ThemeActivity::MenuEdit, "accent", pair(46, -1));
    set_activity_override(
        &mut theme,
        ThemeActivity::Toasts,
        "toast_info",
        pair(39, -1),
    );
    set_activity_override(&mut theme, ThemeActivity::ThemeEdit, "title", pair(201, -1));
    set_activity_override(
        &mut theme,
        ThemeActivity::ShortcutEdit,
        "shortcut_key",
        pair(226, -1),
    );

    let base = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::MainMenu,
    );
    let expected_base =
        ResolvedThemeStyles::from_theme_with_tier(&theme, false, ColorTier::Color256);
    assert_eq!(base.background, expected_base.background);
    assert_eq!(base.shadow, expected_base.shadow);

    // BashEdit overrides apply without disturbing base tokens or MainMenu
    let editor = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::BashEdit,
    );
    assert_eq!(editor.background, Color::Indexed(19));
    assert_eq!(editor.help_text.fg, Some(Color::Indexed(220)));
    assert_eq!(editor.help_text.bg, base.help_text.bg);
    assert_eq!(editor.selection.fg, base.selection.fg);
    assert_eq!(editor.selection.bg, Some(Color::Indexed(52)));
    assert_eq!(editor.text, base.text);

    // Modals, MenuEdit, Toasts, ThemeEdit, ShortcutEdit each resolve their own overrides
    let modals = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::Modals,
    );
    assert_eq!(modals.border, Color::Indexed(81));
    assert_eq!(modals.shadow.fg, Some(Color::Indexed(240)));
    assert_eq!(modals.shadow.bg, Some(Color::Indexed(240)));
    assert_eq!(modals.background, base.background);

    let menuedit = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::MenuEdit,
    );
    assert_eq!(menuedit.accent.fg, Some(Color::Indexed(46)));

    let toasts = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::Toasts,
    );
    assert_eq!(toasts.toast_info, Color::Indexed(39));
    assert_eq!(toasts.toast_error, base.toast_error);

    let themeedit = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::ThemeEdit,
    );
    assert_eq!(themeedit.title.fg, Some(Color::Indexed(201)));

    let shortcutedit = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::Color256,
        ThemeActivity::ShortcutEdit,
    );
    assert_eq!(shortcutedit.shortcut_key.fg, Some(Color::Indexed(226)));
}

#[test]
fn test_shadow_fg_bg_sync_and_drop_shadow_rendering() {
    let mut theme = Theme::monochrome();
    set_activity_override(
        &mut theme,
        ThemeActivity::Modals,
        "shadow",
        Value::Sequence(vec![
            Value::String("#445566".to_string()),
            Value::Number((-1).into()),
        ]),
    );

    let mut modal_styles = ResolvedThemeStyles::from_theme_with_tier_for_activity(
        &theme,
        false,
        ColorTier::TrueColor,
        ThemeActivity::Modals,
    );
    assert_eq!(modal_styles.shadow.fg, Some(Color::Rgb(0x44, 0x55, 0x66)));
    assert_eq!(modal_styles.shadow.bg, Some(Color::Rgb(0x44, 0x55, 0x66)));

    // Confirm ANSI-16 mapping treats mid/dark neutral grays as COLOR_GREY
    assert_eq!(rgb_to_ansi16_name(80, 80, 80), "COLOR_GREY");

    modal_styles.borders.shadow_char = "▒".to_string();
    let area = Rect::new(0, 0, 20, 10);
    let box_rect = Rect::new(2, 1, 10, 4);
    let mut buf = Buffer::empty(area);
    draw_box_drop_shadow(area, box_rect, &modal_styles, &mut buf);

    // Right shadow column and bottom shadow row use shadow_char + matched fg/bg
    let right_cell = &buf[(box_rect.right(), box_rect.y + 1)];
    assert_eq!(right_cell.symbol(), "▒");
    assert_eq!(right_cell.fg, Color::Rgb(0x44, 0x55, 0x66));
    assert_eq!(right_cell.bg, Color::Rgb(0x44, 0x55, 0x66));

    let bottom_cell = &buf[(box_rect.x + 2, box_rect.bottom())];
    assert_eq!(bottom_cell.symbol(), "▒");
    assert_eq!(bottom_cell.fg, Color::Rgb(0x44, 0x55, 0x66));
    assert_eq!(bottom_cell.bg, Color::Rgb(0x44, 0x55, 0x66));
}
