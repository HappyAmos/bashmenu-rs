use bashmenu_core::{MenuItem, MenuItemType, MenuSchema, PluginDisplayRow, Submenu};
use bashmenu_tui::{MenuAction, MenuView, ResolvedThemeStyles};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

fn create_sample_menu() -> MenuSchema {
    MenuSchema {
        version: "0.0.1".to_string(),
        title: "Test Menu".to_string(),
        items: vec![
            MenuItem::divider(),
            MenuItem {
                label: "First Command".to_string(),
                r#type: MenuItemType::Command,
                command: Some("echo 1".to_string()),
                icon: Some("🚀".to_string()),
                ..Default::default()
            },
            MenuItem::divider(),
            MenuItem {
                label: "Submenu Test".to_string(),
                r#type: MenuItemType::Submenu,
                submenu: Some(Submenu {
                    title: "Sub Title".to_string(),
                    items: vec![MenuItem {
                        label: "Child Action".to_string(),
                        r#type: MenuItemType::Command,
                        command: Some("echo child".to_string()),
                        ..Default::default()
                    }],
                }),
                ..Default::default()
            },
            MenuItem {
                label: "Exit Menu".to_string(),
                r#type: MenuItemType::Exit,
                ..Default::default()
            },
        ],
        extra: Default::default(),
    }
}

#[test]
fn test_menu_skips_dividers_on_navigation() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    // Initial item should skip the leading divider (index 0) and land on index 1
    assert_eq!(view.selected_index(), 1);
    assert_eq!(
        view.current_items()[view.selected_index()].label,
        "First Command"
    );

    // Move down should skip divider (index 2) and land on index 3 (Submenu Test)
    view.move_down();
    assert_eq!(view.selected_index(), 3);
    assert_eq!(
        view.current_items()[view.selected_index()].label,
        "Submenu Test"
    );

    // Move up should skip divider back to index 1
    view.move_up();
    assert_eq!(view.selected_index(), 1);
}

#[test]
fn test_submenu_push_and_pop() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    // Navigate to Submenu Test (index 3)
    view.move_down();
    assert_eq!(view.selected_index(), 3);

    // Press Enter to dive into submenu
    let action = view.handle_key(KeyCode::Enter);
    assert_eq!(action, MenuAction::None);
    assert_eq!(view.current_items().len(), 1);
    assert_eq!(view.current_items()[0].label, "Child Action");

    // Press Esc to return to root menu
    let action = view.handle_key(KeyCode::Esc);
    assert_eq!(action, MenuAction::None);
    assert_eq!(view.current_items().len(), 5);
}

#[test]
fn test_immediate_mouse_click_activation() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    let area = Rect::new(0, 0, 80, 24);
    // Row 0 = top border, Row 1 = blank, Row 2 = first item (divider), Row 3 = "First Command"
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 3,
        modifiers: KeyModifiers::NONE,
    };

    let action = view.handle_mouse(mouse, area);
    match action {
        MenuAction::Execute(item) => {
            assert_eq!(item.label, "First Command");
            assert_eq!(item.command, Some("echo 1".to_string()));
        }
        other => panic!(
            "Expected Execute action on immediate MouseDown, got {:?}",
            other
        ),
    }
}

#[test]
fn test_render_into_buffer() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);

    // Verify title was rendered
    let content = format!("{:?}", buf);
    assert!(content.contains("Test Menu"));
}

#[test]
fn test_menu_priority_over_plugins_rendering() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    view.set_plugin_rows(vec![
        PluginDisplayRow {
            text: "Plugin Row 1".to_string(),
            is_divider: false,
        },
        PluginDisplayRow {
            text: "Plugin Row 2".to_string(),
            is_divider: false,
        },
    ]);

    // Case 1: Large terminal (24 rows) -> space for menu items and plugins
    let area_large = Rect::new(0, 0, 80, 24);
    let mut buf_large = Buffer::empty(area_large);
    view.render(area_large, &mut buf_large);

    let mut found_plugin = false;
    for y in 0..24 {
        let mut line_str = String::new();
        for x in 0..80 {
            line_str.push(buf_large[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if line_str.contains("Plugin Row 1") {
            found_plugin = true;
            break;
        }
    }
    assert!(
        found_plugin,
        "Plugin Row 1 should be rendered in large terminal"
    );

    // Case 2: Constrained terminal (6 rows) -> all rows taken by menu/borders, plugins dropped
    let area_small = Rect::new(0, 0, 80, 6);
    let mut buf_small = Buffer::empty(area_small);
    view.render(area_small, &mut buf_small);

    let mut found_plugin_small = false;
    for y in 0..6 {
        let mut line_str = String::new();
        for x in 0..80 {
            line_str.push(buf_small[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if line_str.contains("Plugin Row 1") {
            found_plugin_small = true;
            break;
        }
    }
    assert!(
        !found_plugin_small,
        "Plugin should be dropped when terminal height is constrained"
    );
}

#[test]
fn test_function_key_actions() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    assert_eq!(view.handle_key(KeyCode::F(1)), MenuAction::Help);
    assert_eq!(view.handle_key(KeyCode::F(2)), MenuAction::ThemeSelector);
    assert_eq!(view.handle_key(KeyCode::F(3)), MenuAction::LaunchEditor);
    assert_eq!(view.handle_key(KeyCode::F(4)), MenuAction::EditMenu);
    assert_eq!(view.handle_key(KeyCode::F(5)), MenuAction::Reload);
    assert_eq!(view.handle_key(KeyCode::F(6)), MenuAction::ToggleShortcuts);
    assert_eq!(view.handle_key(KeyCode::F(7)), MenuAction::ConfigureGutter);
    assert_eq!(
        view.handle_key(KeyCode::F(8)),
        MenuAction::LaunchThemeEditor
    );
}

#[test]
fn test_gutter_border_rendering() {
    let schema = create_sample_menu();
    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "user | 100% | 12:00".to_string(),
    );

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);

    // Bottom border is at row 23
    let mut bot_line = String::new();
    for x in 0..80 {
        bot_line.push(buf[(x, 23)].symbol().chars().next().unwrap_or(' '));
    }
    assert!(
        bot_line.starts_with('└'),
        "Bottom border must start with └, got: {}",
        bot_line
    );
    assert!(
        bot_line.ends_with('┘'),
        "Bottom border must end with ┘, got: {}",
        bot_line
    );
    assert!(
        bot_line.contains('┴'),
        "Bottom border must contain bottom tee ┴, got: {}",
        bot_line
    );

    // Gutter top border should be at row 20 or 21 (with 1 or 2 gutter content rows)
    let mut found_gutter_top_border = false;
    for y in 19..23 {
        let mut line_str = String::new();
        for x in 0..80 {
            line_str.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if line_str.starts_with('├') && line_str.ends_with('┤') && line_str.contains('┬') {
            found_gutter_top_border = true;
            break;
        }
    }
    assert!(
        found_gutter_top_border,
        "Gutter top border (├ ... ┬ ... ┤) must be rendered above gutter content"
    );
}

#[test]
fn test_bbcode_placeholder_and_right_aligned_brackets() {
    let mut schema = create_sample_menu();
    schema.items.push(MenuItem {
        label: "Notes [{notebooks_dir}/notes.yml]".to_string(),
        r#type: MenuItemType::Command,
        ..Default::default()
    });
    schema.items.push(MenuItem {
        label: "[b]Styled Option[/b] [tag]".to_string(),
        r#type: MenuItemType::Command,
        ..Default::default()
    });

    let mut view = MenuView::new(
        schema,
        ResolvedThemeStyles::default(),
        true,
        "Status".to_string(),
    );

    view.set_plugin_rows(vec![PluginDisplayRow {
        text: "[b]Alienware[/b] [green]ONLINE[/green]".to_string(),
        is_divider: false,
    }]);

    std::env::set_var("BASHMENU_DIR", "/opt/bashmenu");

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);

    // Verify row with Notes [{notebooks_dir}/notes.yml]
    let mut found_notes_line = false;
    for y in 2..20 {
        let mut line_str = String::new();
        for x in 0..80 {
            line_str.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if line_str.contains("Notes") {
            // Verify placeholder {notebooks_dir} was expanded (does NOT contain '{notebooks_dir}')
            assert!(
                !line_str.contains("{notebooks_dir}"),
                "Placeholder {{notebooks_dir}} must be expanded in label: {}",
                line_str
            );
            assert!(
                line_str.contains("/opt/bashmenu/notebooks/notes.yml]"),
                "Expanded path must be present in label: {}",
                line_str
            );
            // Verify bracket is right-aligned: ends near column 78 (before border at 79)
            let trimmed = line_str[..79].trim_end();
            assert!(
                trimmed.ends_with(']'),
                "Square bracket must be right-aligned near the right margin: {}",
                line_str
            );
            found_notes_line = true;
            break;
        }
    }
    assert!(found_notes_line, "Notes line must be rendered");

    // Verify BBCode in plugin row: raw tags [b], [/b], [green], [/green] stripped, content rendered
    let mut found_plugin_line = false;
    for y in 2..20 {
        let mut line_str = String::new();
        for x in 0..80 {
            line_str.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if line_str.contains("Alienware") {
            assert!(
                !line_str.contains("[b]") && !line_str.contains("[green]"),
                "BBCode tags must be parsed and stripped from display: {}",
                line_str
            );
            assert!(
                line_str.contains("ONLINE"),
                "Plugin text must be present: {}",
                line_str
            );
            found_plugin_line = true;
            break;
        }
    }
    assert!(found_plugin_line, "Plugin row must be rendered");
}

#[test]
fn test_menu_highlight_bar_unbroken_full_width() {
    let schema = create_sample_menu();
    let styles = ResolvedThemeStyles {
        background: Color::Indexed(235),
        highlight: ratatui::style::Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(ratatui::style::Modifier::BOLD),
        shortcut_key: ratatui::style::Style::default()
            .fg(Color::Yellow)
            .add_modifier(ratatui::style::Modifier::BOLD),
        ..Default::default()
    };

    let mut view = MenuView::new(schema, styles, true, "Status".to_string());

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);

    // In create_sample_menu(), item 0 is a divider, so item 1 ("First Command") is the selected active item at row 3
    let row_y = 3;
    let inner_x = 3;
    let avail_w = 80 - 6; // 74

    // 1. Margins outside inner_x must not have highlight background
    assert_ne!(
        buf[(1, row_y)].bg,
        Color::Cyan,
        "Left margin must not have highlight bg"
    );
    assert_ne!(
        buf[(2, row_y)].bg,
        Color::Cyan,
        "Left margin must not have highlight bg"
    );
    assert_ne!(
        buf[(77, row_y)].bg,
        Color::Cyan,
        "Right margin must not have highlight bg"
    );
    assert_ne!(
        buf[(78, row_y)].bg,
        Color::Cyan,
        "Right margin must not have highlight bg"
    );

    // 2. The highlight bar must extend entirely across inner_x to inner_x + avail_w (unbroken)
    for x in inner_x..(inner_x + avail_w) {
        assert_eq!(
            buf[(x, row_y)].bg,
            Color::Cyan,
            "Cell at x={} on active row must have highlight background (unbroken full-width highlight bar)",
            x
        );
    }

    // 3. Specifically verify shortcut keys (e.g. x=9..11, "1. ") are highlighted and not broken
    assert_eq!(
        buf[(9, row_y)].bg,
        Color::Cyan,
        "Shortcut key number '1' must be highlighted"
    );
    assert_eq!(
        buf[(10, row_y)].bg,
        Color::Cyan,
        "Shortcut key dot '.' must be highlighted"
    );
    assert_eq!(
        buf[(11, row_y)].bg,
        Color::Cyan,
        "Shortcut key space ' ' must be highlighted"
    );

    // 4. Verify padding cells all the way to right margin are highlighted
    assert_eq!(
        buf[(inner_x + avail_w - 1, row_y)].bg,
        Color::Cyan,
        "Last cell within right margin must be highlighted"
    );
}
