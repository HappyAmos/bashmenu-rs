//! Stress tests for `bashmenu-tui` widgets, layouts, boundary viewports,
//! and screen transition color calculations.

use bashmenu_core::menu::{MenuItem, MenuItemType, MenuSchema};
use bashmenu_core::theme::Theme;
use bashmenu_tui::modals::frame::render_modal_frame;
use bashmenu_tui::style::ResolvedThemeStyles;
use bashmenu_tui::transition::{dim_color, TransitionEffect, BAYER_4X4};
use bashmenu_tui::MenuView;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[test]
fn test_stress_modal_frame_boundary_dimensions() {
    let theme = Theme::default();
    let styles = ResolvedThemeStyles::from_theme(&theme, true);

    let test_dimensions = [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 3),
        (4, 2),
        (4, 3),
        (5, 5),
        (80, 1),
        (1, 24),
        (10, 10),
        (80, 24),
        (300, 200),
    ];

    for (w, h) in test_dimensions {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        // Must never panic on any viewport dimension, including 0x0
        let inner = render_modal_frame(area, 60, 20, "Stress Test Modal", &styles, &mut buf);
        assert!(inner.width <= area.width);
        assert!(inner.height <= area.height);
    }
}

#[test]
fn test_stress_transition_dim_color_extremes() {
    let colors = [
        Color::Reset,
        Color::Black,
        Color::White,
        Color::Red,
        Color::Rgb(0, 0, 0),
        Color::Rgb(255, 255, 255),
        Color::Rgb(128, 64, 32),
        Color::Indexed(0),
        Color::Indexed(15),
        Color::Indexed(16),
        Color::Indexed(231),
        Color::Indexed(255),
    ];

    let factors = [
        -100.0, -1.0, -0.001, 0.0, 0.0001, 0.1, 0.25, 0.5, 0.75, 1.0, 1.001, 2.0, 100.0,
    ];

    for color in colors {
        for factor in factors {
            // Must not panic on negative, zero, or overflow factors
            let dimmed = dim_color(color, factor);
            match dimmed {
                Color::Rgb(r, g, b) => {
                    let _ = (r, g, b);
                }
                Color::Indexed(idx) => {
                    let _ = idx;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn test_stress_bayer_matrix_bounds() {
    for y in 0..1000 {
        for x in 0..1000 {
            let val = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
            assert!(val <= 15, "Bayer 4x4 dither values must be in range 0..=15");
        }
    }
}

#[test]
fn test_stress_transition_effect_fuzz_parsing() {
    let test_strings = [
        "",
        "   ",
        "fade",
        "FADE",
        "  FaDe  ",
        "curtain",
        "CURTAIN",
        "dissolve",
        "dither",
        "pixel",
        "matrix",
        "rain",
        "cyber",
        "slide",
        "slide_left",
        "slideleft",
        "slide_right",
        "slideright",
        "blinds",
        "venetian",
        "slats",
        "iris",
        "zoom",
        "box",
        "wipedown",
        "wipe_down",
        "blind",
        "roll",
        "instant",
        "none",
        "disabled",
        "off",
        "0",
        "false",
        "unknown_random_gibberish",
        "123456",
        "!@#$%^&*()",
    ];

    for s in test_strings {
        let eff = TransitionEffect::parse(Some(s));
        let _ = format!("{:?}", eff);
    }
    assert_eq!(TransitionEffect::parse(None), TransitionEffect::Fade);
}

#[test]
fn test_stress_menu_view_large_scale_and_navigation() {
    let mut items = Vec::new();
    for i in 0..1000 {
        let item = if i % 10 == 0 {
            MenuItem {
                label: format!("Item {:04}", i),
                r#type: MenuItemType::Divider,
                ..Default::default()
            }
        } else {
            MenuItem {
                label: format!("Item {:04}", i),
                r#type: MenuItemType::Command,
                command: Some(format!("echo {}", i)),
                ..Default::default()
            }
        };
        items.push(item);
    }

    let schema = MenuSchema {
        version: "1.0".to_string(),
        title: "Large Menu".to_string(),
        items,
        extra: Default::default(),
    };

    let mut menu_view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status Stress".to_string(),
    );

    // Stress navigation
    for _ in 0..2000 {
        menu_view.move_down();
    }
    assert!(menu_view.selected_index() > 0);

    for _ in 0..2000 {
        menu_view.move_up();
    }
    // Must remain clamped within valid indices without underflow
    assert_eq!(menu_view.selected_index(), 1); // 0 is divider

    // Stress page jumps
    for _ in 0..100 {
        menu_view.page_down(20);
    }
    for _ in 0..100 {
        menu_view.page_up(20);
    }

    // Stress rendering across multiple terminal sizes
    let sizes = [(0, 0), (1, 1), (10, 5), (80, 24), (200, 60)];
    for (w, h) in sizes {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        // Must never panic on rendering into any sized buffer
        menu_view.render(area, &mut buf);
    }
}
