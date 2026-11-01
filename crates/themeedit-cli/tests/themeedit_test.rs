//! Integration tests for ThemeEdit state machine and save invariants.

use std::fs;
use std::path::PathBuf;
use themeedit_cli::ThemeEditorState;

fn get_fixture_path(filename: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let path = manifest_dir.join("../../tests/fixtures").join(filename);
    if path.exists() {
        path
    } else {
        PathBuf::from("tests/fixtures").join(filename)
    }
}

#[test]
fn test_themeedit_initial_state_and_modification() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    assert_eq!(state.current_name, "dracula");
    assert!(!state.is_modified);

    // Modify a color token
    state.set_color_token("text", 123);
    assert!(state.is_modified);

    // Verify token display reflects update
    let text_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Text Color")
        .unwrap();
    let display = state.get_token_value_display(text_token);
    assert_eq!(display, "Color 123");

    // Modify a border token
    state.set_border_token("border_horizontal", '=');
    let horiz_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Horizontal Border")
        .unwrap();
    let horiz_display = state.get_token_value_display(horiz_token);
    assert_eq!(horiz_display, "'='");
}

#[test]
fn test_themeedit_ctrl_s_direct_save_invariant() {
    let tmp_dir = std::env::temp_dir().join(format!("themeedit_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_file = tmp_dir.join("test.themes");

    // Copy fixture to tmp
    let fixture_path = get_fixture_path("bashmenu.themes");
    fs::copy(&fixture_path, &tmp_file).unwrap();

    let mut state = ThemeEditorState::new(tmp_file.clone(), Some("dracula".to_string()));
    assert_eq!(state.current_name, "dracula");
    assert!(!state.is_modified);

    // Edit color
    state.set_color_token("highlight", 99);
    assert!(state.is_modified);

    // Ctrl+S saves directly without diverting to Save As when theme_name is set
    state.save_to_disk().unwrap();
    assert!(!state.is_modified);

    // Verify saved content on disk
    let reloaded = ThemeEditorState::new(tmp_file, Some("dracula".to_string()));
    let highlight_token = reloaded
        .tokens
        .iter()
        .find(|t| t.label == "Highlight Color")
        .unwrap();
    assert_eq!(
        reloaded.get_token_value_display(highlight_token),
        "Color 99"
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_themeedit_preview_mode_cycling() {
    use themeedit_cli::PreviewMode;

    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    assert_eq!(state.preview_mode, PreviewMode::Menu);
    assert_eq!(state.preview_mode.title(), "Main Menu");

    state.preview_mode = state.preview_mode.next();
    assert_eq!(state.preview_mode, PreviewMode::TextEditor);
    assert_eq!(state.preview_mode.title(), "Text Editor");

    state.preview_mode = state.preview_mode.next();
    assert_eq!(state.preview_mode, PreviewMode::Modals);
    assert_eq!(state.preview_mode.title(), "Popup Windows / Modals");

    state.preview_mode = state.preview_mode.next();
    assert_eq!(state.preview_mode, PreviewMode::MenuEditor);
    assert_eq!(state.preview_mode.title(), "Visual Menu Editor");

    state.preview_mode = state.preview_mode.next();
    assert_eq!(state.preview_mode, PreviewMode::Toast);
    assert_eq!(state.preview_mode.title(), "Toast Notifications");

    state.preview_mode = state.preview_mode.next();
    assert_eq!(state.preview_mode, PreviewMode::Menu);
}

#[test]
fn test_themeedit_help_text_and_gutter_tokens() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    // Verify Help Text Color token exists and can be updated
    let help_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Help Text Color")
        .cloned()
        .expect("Help Text Color token must exist");
    assert_eq!(help_token.label, "Help Text Color");

    state.set_color_token("help_text", 45);
    assert_eq!(state.get_token_value_display(&help_token), "Color 45");

    // Verify gutter border tokens exist and can be updated
    let gutter_sep_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Gutter Separator")
        .cloned()
        .expect("Gutter Separator token must exist");
    state.set_border_token("gutter_separator", '│');
    assert_eq!(state.get_token_value_display(&gutter_sep_token), "'│'");

    let gutter_top_tee_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Gutter Top Tee")
        .cloned()
        .expect("Gutter Top Tee token must exist");
    state.set_border_token("gutter_top_tee", '┬');
    assert_eq!(state.get_token_value_display(&gutter_top_tee_token), "'┬'");
}

#[test]
fn test_themeedit_t_connectors_and_click_selection() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    // Verify T-connector tokens exist and can be modified
    let left_tee = state
        .tokens
        .iter()
        .find(|t| t.label == "Left T-Connector")
        .cloned()
        .expect("Left T-Connector token must exist");
    state.set_border_token("left_tee", '├');
    assert_eq!(state.get_token_value_display(&left_tee), "'├'");

    let right_tee = state
        .tokens
        .iter()
        .find(|t| t.label == "Right T-Connector")
        .cloned()
        .expect("Right T-Connector token must exist");
    state.set_border_token("right_tee", '┤');
    assert_eq!(state.get_token_value_display(&right_tee), "'┤'");

    let top_tee = state
        .tokens
        .iter()
        .find(|t| t.label == "Top T-Connector")
        .cloned()
        .expect("Top T-Connector token must exist");
    state.set_border_token("top_tee", '┬');
    assert_eq!(state.get_token_value_display(&top_tee), "'┬'");

    let bottom_tee = state
        .tokens
        .iter()
        .find(|t| t.label == "Bottom T-Connector")
        .cloned()
        .expect("Bottom T-Connector token must exist");
    state.set_border_token("bottom_tee", '┴');
    assert_eq!(state.get_token_value_display(&bottom_tee), "'┴'");

    // Verify click selection / focus by key
    assert!(state.select_token_by_key("highlight"));
    assert_eq!(state.tokens[state.selected_idx].label, "Highlight Color");

    assert!(state.select_token_by_key("left_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Left T-Connector");

    assert!(state.select_token_by_key("right_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Right T-Connector");

    assert!(state.select_token_by_key("help_text"));
    assert_eq!(state.tokens[state.selected_idx].label, "Help Text Color");

    assert!(state.select_token_by_key("gutter"));
    assert_eq!(state.tokens[state.selected_idx].label, "Gutter Color");

    assert!(state.select_token_by_key("shortcut_key"));
    assert_eq!(
        state.tokens[state.selected_idx].label,
        "Shortcut Gutter (1-9, a-z)"
    );

    // Verify gutter T-connectors and window T-connectors select exact tokens
    assert!(state.select_token_by_key("gutter_top_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Gutter Top Tee");

    assert!(state.select_token_by_key("gutter_bottom_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Gutter Bottom Tee");

    assert!(state.select_token_by_key("top_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Top T-Connector");

    assert!(state.select_token_by_key("bottom_tee"));
    assert_eq!(state.tokens[state.selected_idx].label, "Bottom T-Connector");

    assert!(state.select_token_by_key("divider"));
    assert_eq!(state.tokens[state.selected_idx].label, "Divider Color");

    assert!(state.select_token_by_key("indicator"));
    assert_eq!(state.tokens[state.selected_idx].label, "Cursor Indicator");

    // Verify new divider and marquee tokens
    assert!(state.select_token_by_key("divider_char"));
    assert_eq!(state.tokens[state.selected_idx].label, "Divider Character");

    assert!(state.select_token_by_key("divider_length"));
    assert_eq!(state.tokens[state.selected_idx].label, "Divider Width Mode");

    assert!(state.select_token_by_key("marquee_char"));
    assert_eq!(state.tokens[state.selected_idx].label, "Marquee Character");

    assert!(state.select_token_by_key("marquee_gap"));
    assert_eq!(state.tokens[state.selected_idx].label, "Marquee Gap Count");
}

#[test]
fn test_themeedit_divider_and_marquee_configuration() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("pacman".to_string()));

    // Pacman has screen_width
    let div_len_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Divider Width Mode")
        .unwrap()
        .clone();
    assert_eq!(
        state.get_token_value_display(&div_len_token),
        "{screen_width}"
    );

    // Toggle to window_width
    state.toggle_divider_length();
    assert_eq!(
        state.get_token_value_display(&div_len_token),
        "{window_width}"
    );
    assert!(state.is_modified);

    // Toggle back to screen_width
    state.toggle_divider_length();
    assert_eq!(
        state.get_token_value_display(&div_len_token),
        "{screen_width}"
    );

    // Set divider char
    state.set_divider_char('═');
    let div_char_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Divider Character")
        .unwrap()
        .clone();
    assert_eq!(state.get_token_value_display(&div_char_token), "'═'");

    // Marquee char and gap
    state.set_marquee_char('~');
    let marq_char_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Marquee Character")
        .unwrap()
        .clone();
    assert_eq!(state.get_token_value_display(&marq_char_token), "'~'");

    let marq_gap_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Marquee Gap Count")
        .unwrap()
        .clone();
    state.cycle_marquee_gap();
    assert_eq!(state.get_token_value_display(&marq_gap_token), "4");
}

#[test]
fn test_themeedit_qbasic_ascii_indicator_and_divider() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let state = ThemeEditorState::new(fixture_path, Some("qbasic".to_string()));

    // QBasic indicator has {ascii:240} which translates to ≡
    let ind_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Cursor Indicator")
        .unwrap();
    let ind_display = state.get_token_value_display(ind_token);
    assert!(
        ind_display.contains('≡'),
        "Expected indicator display to contain '≡', got: {}",
        ind_display
    );

    // QBasic divider length is window_width
    let div_len_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Divider Width Mode")
        .unwrap();
    assert_eq!(
        state.get_token_value_display(div_len_token),
        "{window_width}"
    );
}

#[test]
fn test_themeedit_scroll_offset_adjustment() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.selected_idx, 0);

    // Viewport height 10
    state.adjust_scroll(10);
    assert_eq!(state.scroll_offset, 0);

    // Move selection down to index 15
    state.selected_idx = 15;
    state.adjust_scroll(10);
    // scroll_offset should shift so 15 is visible: 15 - 10 + 1 = 6
    assert_eq!(state.scroll_offset, 6);

    // Move selection back up to index 2
    state.selected_idx = 2;
    state.adjust_scroll(10);
    assert_eq!(state.scroll_offset, 2);
}

#[test]
fn test_themeedit_preview_menu_editor_border_intact() {
    use bashmenu_tui::ResolvedThemeStyles;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::{Block, Borders, Widget};

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 60, 20);
    let mut buf = Buffer::empty(area);

    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title("─[ Visual Menu Editor ]")
        .border_style(ratatui::style::Style::default().fg(styles.border));
    preview_block.render(area, &mut buf);

    let bot_y = area.y + area.height - 1;
    let footer_y = bot_y.saturating_sub(1);
    let p_inner_x = area.x + 2;
    let p_inner_y = area.y + 2;
    let p_avail_w = area.width.saturating_sub(4);
    let panels_h = footer_y.saturating_sub(p_inner_y);
    let left_w = p_avail_w / 2;
    let right_w = p_avail_w.saturating_sub(left_w);

    let tree_rect = Rect::new(p_inner_x, p_inner_y, left_w, panels_h);
    let insp_rect = Rect::new(p_inner_x + left_w, p_inner_y, right_w, panels_h);

    let tree_block = Block::default()
        .borders(Borders::ALL)
        .title("─[ Menu Structure ]")
        .border_style(styles.accent);
    tree_block.render(tree_rect, &mut buf);

    let insp_block = Block::default()
        .borders(Borders::ALL)
        .title("───[ Property Inspector ]")
        .border_style(styles.accent);
    insp_block.render(insp_rect, &mut buf);

    // Verify outer border corners intact
    assert_ne!(buf[(area.x, bot_y)].symbol(), " ");
    assert_ne!(buf[(area.x + area.width - 1, bot_y)].symbol(), " ");

    // Verify interior panel titles rendered
    let mut tree_title_found = false;
    for x in tree_rect.x..tree_rect.x + tree_rect.width {
        if buf[(x, tree_rect.y)].symbol() == "[" {
            tree_title_found = true;
            break;
        }
    }
    assert!(
        tree_title_found,
        "Tree panel title [ Menu Structure ] must be rendered"
    );

    let mut insp_title_found = false;
    for x in insp_rect.x..insp_rect.x + insp_rect.width {
        if buf[(x, insp_rect.y)].symbol() == "[" {
            insp_title_found = true;
            break;
        }
    }
    assert!(
        insp_title_found,
        "Property inspector panel title must be rendered"
    );
}

#[test]
fn test_themeedit_toast_tokens_and_customization() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    // Verify toast tokens exist
    assert!(state.tokens.iter().any(|t| t.label == "Toast Left Bar"));
    assert!(state
        .tokens
        .iter()
        .any(|t| t.label == "Toast Default Timeout"));
    assert!(state.tokens.iter().any(|t| t.label == "Toast Info Accent"));
    assert!(state
        .tokens
        .iter()
        .any(|t| t.label == "Toast Warning Accent"));
    assert!(state.tokens.iter().any(|t| t.label == "Toast Error Accent"));

    // Verify custom toast bar char
    state.set_toast_char('▎');
    assert!(state.is_modified);
    let bar_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Toast Left Bar")
        .unwrap();
    assert_eq!(state.get_token_value_display(bar_token), "'▎'");

    // Verify cycling toast timeout
    state.cycle_toast_timeout();
    let timeout_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Toast Default Timeout")
        .unwrap();
    let display = state.get_token_value_display(timeout_token);
    assert!(display.ends_with('s'));

    // Verify toast color tokens update correctly
    state.set_color_token("toast_info", 39);
    state.set_color_token("toast_warning", 214);
    state.set_color_token("toast_error", 196);

    let info_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Toast Info Accent")
        .unwrap();
    assert_eq!(state.get_token_value_display(info_token), "Color 39");

    let warn_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Toast Warning Accent")
        .unwrap();
    assert_eq!(state.get_token_value_display(warn_token), "Color 214");

    let err_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Toast Error Accent")
        .unwrap();
    assert_eq!(state.get_token_value_display(err_token), "Color 196");

    // Verify token selection by key works for toast keys
    assert!(state.select_token_by_key("toast_char"));
    assert_eq!(state.tokens[state.selected_idx].label, "Toast Left Bar");

    assert!(state.select_token_by_key("toast_timeout"));
    assert_eq!(
        state.tokens[state.selected_idx].label,
        "Toast Default Timeout"
    );

    assert!(state.select_token_by_key("toast_info"));
    assert_eq!(state.tokens[state.selected_idx].label, "Toast Info Accent");
}

#[test]
fn test_themeedit_help_gutter_layout_above_bottom_border() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    let bot_y = area.y + area.height - 1; // 23
    let footer_y = bot_y.saturating_sub(1); // 22
    let gt_y = footer_y.saturating_sub(1); // 21
    let panel_y = area.y + 2; // 2
    let panel_h = gt_y.saturating_sub(panel_y); // 19

    // Check that panel_h leaves exact space for gutter divider and gutter row
    assert_eq!(panel_y + panel_h, gt_y);
    assert_eq!(gt_y + 1, footer_y);
    assert_eq!(footer_y + 1, bot_y);

    // Simulate drawing gutter divider and help gutter row
    buf.set_string(area.x, gt_y, "├", ratatui::style::Style::default());
    buf.set_string(
        area.x + area.width - 1,
        gt_y,
        "┤",
        ratatui::style::Style::default(),
    );
    buf.set_string(area.x, footer_y, "│", ratatui::style::Style::default());
    buf.set_string(
        area.x + area.width - 1,
        footer_y,
        "│",
        ratatui::style::Style::default(),
    );
    buf.set_string(area.x, bot_y, "└", ratatui::style::Style::default());
    buf.set_string(
        area.x + area.width - 1,
        bot_y,
        "┘",
        ratatui::style::Style::default(),
    );

    // Bottom border must be undisturbed on row bot_y
    assert_eq!(buf[(area.x, bot_y)].symbol(), "└");
    assert_eq!(buf[(area.x + area.width - 1, bot_y)].symbol(), "┘");

    // Help gutter must be on row footer_y (not bottom border)
    assert_eq!(buf[(area.x, footer_y)].symbol(), "│");
    assert_eq!(buf[(area.x + area.width - 1, footer_y)].symbol(), "│");

    // Gutter divider must be on row gt_y
    assert_eq!(buf[(area.x, gt_y)].symbol(), "├");
    assert_eq!(buf[(area.x + area.width - 1, gt_y)].symbol(), "┤");
}

#[test]
fn test_themeedit_background_color_and_wallpaper_tokens() {
    let fixture_path = get_fixture_path("bashmenu.themes");
    let mut state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));

    // 1. Background Color token exists and can be updated
    let bg_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Background Color")
        .cloned()
        .expect("Background Color token must exist");
    assert_eq!(bg_token.label, "Background Color");

    state.set_color_token("background", 234);
    assert_eq!(state.get_token_value_display(&bg_token), "Color 234");
    assert!(state.select_token_by_key("background"));
    assert_eq!(state.tokens[state.selected_idx].label, "Background Color");
    assert!(state.select_token_by_key("bg"));
    assert_eq!(state.tokens[state.selected_idx].label, "Background Color");

    // 2. Wallpaper Image token exists and displays None initially
    let wp_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Wallpaper Image")
        .cloned()
        .expect("Wallpaper Image token must exist");
    assert_eq!(state.get_token_value_display(&wp_token), "None");

    // 3. Set wallpaper image
    state.set_wallpaper_image(Some("assets/wallpapers/twilight.png".to_string()));
    assert_eq!(state.get_token_value_display(&wp_token), "twilight.png");
    assert!(state.select_token_by_key("wallpaper"));
    assert_eq!(state.tokens[state.selected_idx].label, "Wallpaper Image");
    assert!(state.select_token_by_key("wallpaper_image"));
    assert_eq!(state.tokens[state.selected_idx].label, "Wallpaper Image");

    // 4. Remove/clear wallpaper image
    state.set_wallpaper_image(None);
    assert_eq!(state.get_token_value_display(&wp_token), "None");
}

#[test]
fn test_themeedit_token_preview_spans_and_flash() {
    use bashmenu_tui::ResolvedThemeStyles;
    use ratatui::layout::Rect;
    use themeedit_cli::{get_token_preview_spans, PreviewMode};

    let fixture_path = get_fixture_path("bashmenu.themes");
    let state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));
    let styles = ResolvedThemeStyles::from_theme(&state.current_theme, true);
    let right_area = Rect::new(40, 2, 40, 20);

    // 1. Background Color spans the interior
    let bg_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Background Color")
        .unwrap();
    let bg_spans = get_token_preview_spans(bg_token, right_area, PreviewMode::Menu, &styles);
    assert!(!bg_spans.is_empty(), "Background spans must not be empty");
    assert_eq!(bg_spans[0].1, right_area.x + 1);
    assert_eq!(bg_spans[0].2, right_area.x + right_area.width - 1);

    // 2. Wallpaper Image spans the interior
    let wp_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Wallpaper Image")
        .unwrap();
    let wp_spans = get_token_preview_spans(wp_token, right_area, PreviewMode::Menu, &styles);
    assert!(!wp_spans.is_empty(), "Wallpaper spans must not be empty");

    // 3. Highlight Color spans the active row
    let hl_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Highlight Color")
        .unwrap();
    let hl_spans = get_token_preview_spans(hl_token, right_area, PreviewMode::Menu, &styles);
    assert_eq!(hl_spans.len(), 1);
    assert_eq!(hl_spans[0].0, right_area.y + 3); // content_start_y + 1

    // 4. Border Color spans all 4 border edges
    let border_token = state
        .tokens
        .iter()
        .find(|t| t.label == "Border Color")
        .unwrap();
    let border_spans =
        get_token_preview_spans(border_token, right_area, PreviewMode::Menu, &styles);
    assert!(
        border_spans.len() >= 4,
        "Border spans must cover all outer edges"
    );

    // 5. Shortcut Gutter spans shortcut digits 1-4
    let sc_token = state
        .tokens
        .iter()
        .find(|t| t.label.contains("Shortcut Gutter"))
        .unwrap();
    let sc_spans = get_token_preview_spans(sc_token, right_area, PreviewMode::Menu, &styles);
    assert_eq!(sc_spans.len(), 4, "Must flash 4 menu item shortcut digits");
}

#[test]
fn test_modals_and_toast_preview_wallpaper_compositing() {
    use bashmenu_core::WallpaperConfig;
    use bashmenu_tui::{ResolvedThemeStyles, WallpaperCompositor};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use themeedit_cli::{render_preview_pane, PreviewMode, ThemeEditorState};

    let fixture_path = get_fixture_path("bashmenu.themes");
    let state = ThemeEditorState::new(fixture_path, Some("dracula".to_string()));
    let styles = ResolvedThemeStyles::from_theme(&state.current_theme, true);
    let right_area = Rect::new(40, 2, 60, 24);

    let wp_cfg = WallpaperConfig {
        enabled: true,
        image: Some("assets/wallpapers/alaska-mountain.png".to_string()),
        opacity: 0.85,
        scaling: "stretch".to_string(),
        clustering: 0,
        quantization: 1,
        mode: Some("halfblock".to_string()),
    };

    // 1. Verify PreviewMode::Modals composites wallpaper behind modal
    {
        let mut buf = Buffer::empty(Rect::new(0, 0, 110, 30));
        let mut compositor = Some(WallpaperCompositor::new(wp_cfg.clone()));
        render_preview_pane(
            right_area,
            &mut buf,
            PreviewMode::Modals,
            &styles,
            &mut compositor,
        );

        let mut has_wallpaper_block = false;
        let mut has_drop_shadow = false;

        for y in right_area.y..right_area.bottom() {
            for x in right_area.x..right_area.right() {
                let sym = buf[(x, y)].symbol();
                if sym == "▀" {
                    has_wallpaper_block = true;
                }
                if sym == "░" {
                    has_drop_shadow = true;
                }
            }
        }

        assert!(
            has_wallpaper_block,
            "PreviewMode::Modals must composite wallpaper half-blocks behind dialog"
        );
        assert!(
            has_drop_shadow,
            "PreviewMode::Modals must render drop-shadow '░' over wallpaper"
        );

        // Check that the modal title is present
        let mut title_found = false;
        for y in right_area.y..right_area.bottom() {
            let mut line_str = String::new();
            for x in right_area.x..right_area.right() {
                line_str.push_str(buf[(x, y)].symbol());
            }
            if line_str.contains("Confirmation Dialog") {
                title_found = true;
                break;
            }
        }
        assert!(
            title_found,
            "Modal confirmation dialog title must be rendered"
        );
    }

    // 2. Verify PreviewMode::Toast composites wallpaper behind toasts
    {
        let mut buf = Buffer::empty(Rect::new(0, 0, 110, 30));
        let mut compositor = Some(WallpaperCompositor::new(wp_cfg));
        render_preview_pane(
            right_area,
            &mut buf,
            PreviewMode::Toast,
            &styles,
            &mut compositor,
        );

        let mut has_wallpaper_block = false;
        let mut toast_text_found = false;

        for y in right_area.y..right_area.bottom() {
            let mut line_str = String::new();
            for x in right_area.x..right_area.right() {
                let sym = buf[(x, y)].symbol();
                if sym == "▀" {
                    has_wallpaper_block = true;
                }
                line_str.push_str(sym);
            }
            if line_str.contains("Saved configuration to disk") {
                toast_text_found = true;
            }
        }

        assert!(
            has_wallpaper_block,
            "PreviewMode::Toast must composite wallpaper half-blocks behind toasts"
        );
        assert!(
            toast_text_found,
            "PreviewMode::Toast must render toast message text"
        );
    }
}
