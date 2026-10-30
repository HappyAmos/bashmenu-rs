use bashmenu_core::{MenuItem, MenuItemType, MenuSchema, Submenu};
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
