use bashmenu_core::{MenuItem, MenuItemType, MenuSchema, PluginDisplayRow, Submenu};
use bashmenu_tui::{MenuAction, MenuView, ResolvedThemeStyles};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

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
