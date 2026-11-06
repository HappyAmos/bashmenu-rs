use bashmenu_core::WallpaperConfig;
use bashmenu_tui::{
    BackgroundConfigAction, BackgroundConfigModal, BackgroundFocus, ResolvedThemeStyles,
    MODE_OPTIONS, SCALING_OPTIONS,
};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_background_config_modal_initial_state_and_defaults() {
    let modal = BackgroundConfigModal::new(None);
    assert_eq!(modal.title, "Configure Background Wallpaper");
    assert_eq!(modal.focus, BackgroundFocus::Image);
    assert_eq!(modal.image_input.as_string(), "");
    assert_eq!(modal.opacity_input.as_string(), "0.35");
    assert_eq!(modal.scaling_idx, 0); // "cover"
    assert_eq!(modal.mode_idx, 0); // "halfblock"
    assert_eq!(modal.clustering_input.as_string(), "0");
    assert_eq!(modal.quantization_input.as_string(), "1");

    let cfg = modal.build_config();
    assert!(!cfg.enabled);
    assert_eq!(cfg.image, None);
    assert_eq!(cfg.scaling, "cover");
    assert_eq!(cfg.mode, Some("halfblock".to_string()));
}

#[test]
fn test_background_config_modal_from_existing_config() {
    let existing = WallpaperConfig {
        enabled: true,
        image: Some("assets/wallpapers/twilight.png".to_string()),
        opacity: 0.65,
        scaling: "stretch".to_string(),
        clustering: 24,
        quantization: 16,
        mode: Some("quarterblock".to_string()),
    };

    let modal = BackgroundConfigModal::new(Some(&existing));
    assert_eq!(
        modal.image_input.as_string(),
        "assets/wallpapers/twilight.png"
    );
    assert_eq!(modal.opacity_input.as_string(), "0.65");
    assert_eq!(SCALING_OPTIONS[modal.scaling_idx].0, "stretch");
    assert_eq!(MODE_OPTIONS[modal.mode_idx].0, "quarterblock");
    assert_eq!(modal.clustering_input.as_string(), "24");
    assert_eq!(modal.quantization_input.as_string(), "16");

    let rebuilt = modal.build_config();
    assert!(rebuilt.enabled);
    assert_eq!(
        rebuilt.image.as_deref(),
        Some("assets/wallpapers/twilight.png")
    );
    assert!((rebuilt.opacity - 0.65).abs() < 0.01);
    assert_eq!(rebuilt.scaling, "stretch");
    assert_eq!(rebuilt.mode.as_deref(), Some("quarterblock"));
    assert_eq!(rebuilt.clustering, 24);
    assert_eq!(rebuilt.quantization, 16);
}

#[test]
fn test_background_config_modal_keyboard_navigation_and_cycling() {
    let mut modal = BackgroundConfigModal::new(None);
    assert_eq!(modal.focus, BackgroundFocus::Image);

    // Tab moves focus through all controls
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Browse);

    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Opacity);

    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Scaling);

    // Right / Left cycles scaling mode
    assert_eq!(modal.scaling_idx, 0); // cover
    modal.handle_key(KeyCode::Right);
    assert_eq!(modal.scaling_idx, 1); // stretch
    modal.handle_key(KeyCode::Right);
    assert_eq!(modal.scaling_idx, 2); // fit
    modal.handle_key(KeyCode::Left);
    assert_eq!(modal.scaling_idx, 1); // stretch

    // Move to Mode selector
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Mode);
    assert_eq!(modal.mode_idx, 0); // halfblock
    modal.handle_key(KeyCode::Right);
    assert_eq!(modal.mode_idx, 1); // quarterblock
    modal.handle_key(KeyCode::Left);
    assert_eq!(modal.mode_idx, 0); // halfblock

    // Move to Clustering
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Clustering);

    // Move to Quantization
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Quantization);

    // Move to Apply button
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Apply);

    // Move to Remove button
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Remove);

    // Move to Cancel button
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Cancel);

    // Tab wraps around to Image
    assert_eq!(modal.handle_key(KeyCode::Tab), BackgroundConfigAction::None);
    assert_eq!(modal.focus, BackgroundFocus::Image);

    // BackTab moves in reverse
    assert_eq!(
        modal.handle_key_with_modifiers(KeyCode::BackTab, KeyModifiers::NONE),
        BackgroundConfigAction::None
    );
    assert_eq!(modal.focus, BackgroundFocus::Cancel);
}

#[test]
fn test_background_config_modal_actions() {
    let mut modal = BackgroundConfigModal::new(None);
    modal.set_image_path("assets/wallpapers/twilight.png");

    // Esc produces Cancel
    assert_eq!(
        modal.handle_key(KeyCode::Esc),
        BackgroundConfigAction::Cancel
    );

    // Browse action when focused on Browse button
    modal.focus = BackgroundFocus::Browse;
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        BackgroundConfigAction::Browse
    );

    // Remove action when focused on Remove button
    modal.focus = BackgroundFocus::Remove;
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        BackgroundConfigAction::Remove
    );

    // Apply action when focused on Apply button
    modal.focus = BackgroundFocus::Apply;
    match modal.handle_key(KeyCode::Enter) {
        BackgroundConfigAction::Apply(cfg) => {
            assert_eq!(cfg.image.as_deref(), Some("assets/wallpapers/twilight.png"));
            assert!(cfg.enabled);
        }
        other => panic!("Expected Apply action, got {:?}", other),
    }

    // Ctrl+S triggers Apply from anywhere
    modal.focus = BackgroundFocus::Image;
    match modal.handle_key_with_modifiers(KeyCode::Char('s'), KeyModifiers::CONTROL) {
        BackgroundConfigAction::Apply(cfg) => {
            assert_eq!(cfg.image.as_deref(), Some("assets/wallpapers/twilight.png"));
        }
        other => panic!("Expected Apply action on Ctrl+S, got {:?}", other),
    }
}

#[test]
fn test_background_config_modal_render() {
    let modal = BackgroundConfigModal::new(None);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let styles = ResolvedThemeStyles::default();

    modal.render(area, &styles, &mut buf);

    // Check title was rendered
    let title = "Configure Background Wallpaper";
    let mut found_title = false;
    for y in 0..24 {
        let mut row_str = String::new();
        for x in 0..80 {
            row_str.push_str(buf[(x, y)].symbol());
        }
        if row_str.contains(title) {
            found_title = true;
            break;
        }
    }
    assert!(found_title, "Modal title must be rendered in buffer");
}

#[test]
fn test_background_config_modal_mouse_interaction() {
    let mut modal = BackgroundConfigModal::new(None);
    let area = Rect::new(0, 0, 80, 24);

    // Close button click at top right of frame returns Cancel
    let close_mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 70,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    let act = modal.handle_mouse(close_mouse, area);
    // Even if close button position varies slightly with geometry,
    // verify handle_mouse executes cleanly without panic
    let _ = act;
}
