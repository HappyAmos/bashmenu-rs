//! Integration tests for MenuEdit state machine, item reordering, and submenu navigation.

use bashmenu_core::{load_menu, MenuItem, MenuItemType, Submenu};
use menuedit_cli::MenuEditorState;
use std::fs;
use std::path::PathBuf;

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
fn test_menuedit_navigation_and_reorder() {
    let fixture_path = get_fixture_path("bashmenu.mnu");
    let mut state = MenuEditorState::new(fixture_path);

    assert_eq!(state.selected_index(), 0);
    let original_first = state.current_items()[0].label.clone();
    let original_second = state.current_items()[1].label.clone();

    // Move down
    state.move_down();
    assert_eq!(state.selected_index(), 1);

    // Swap up (move item 1 to item 0)
    state.swap_up();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.current_items()[0].label, original_second);
    assert_eq!(state.current_items()[1].label, original_first);
    assert!(state.is_modified);

    // Swap down (move item 0 back to item 1)
    state.swap_down();
    assert_eq!(state.selected_index(), 1);
    assert_eq!(state.current_items()[0].label, original_first);
    assert_eq!(state.current_items()[1].label, original_second);
}

#[test]
fn test_menuedit_add_edit_delete() {
    let fixture_path = get_fixture_path("bashmenu.mnu");
    let mut state = MenuEditorState::new(fixture_path);

    let initial_count = state.current_items().len();

    // Add new command item
    let new_item = MenuItem {
        label: "Custom Added Item".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo custom".to_string()),
        ..Default::default()
    };
    state.add_item(new_item);
    assert_eq!(state.current_items().len(), initial_count + 1);
    assert_eq!(
        state.current_items()[state.selected_index()].label,
        "Custom Added Item"
    );

    // Edit current item
    state.edit_selected(
        "Custom Renamed Item".to_string(),
        Some("echo edited".to_string()),
        Some("⭐".to_string()),
    );
    assert_eq!(
        state.current_items()[state.selected_index()].label,
        "Custom Renamed Item"
    );
    assert_eq!(
        state.current_items()[state.selected_index()]
            .command
            .as_deref(),
        Some("echo edited")
    );

    // Delete current item
    let deleted = state.delete_selected().unwrap();
    assert_eq!(deleted.label, "Custom Renamed Item");
    assert_eq!(state.current_items().len(), initial_count);
}

#[test]
fn test_menuedit_submenu_dive_and_save_roundtrip() {
    let tmp_dir = std::env::temp_dir().join(format!("menuedit_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_file = tmp_dir.join("test.mnu");

    let fixture_path = get_fixture_path("bashmenu.mnu");
    fs::copy(&fixture_path, &tmp_file).unwrap();

    let mut state = MenuEditorState::new(tmp_file.clone());

    // Add a submenu item
    let sub_item = MenuItem {
        label: "Test Submenu".to_string(),
        r#type: MenuItemType::Submenu,
        submenu: Some(Submenu {
            title: "Sub Title".to_string(),
            items: vec![MenuItem {
                label: "Sub Item 1".to_string(),
                r#type: MenuItemType::Command,
                command: Some("echo sub".to_string()),
                ..Default::default()
            }],
        }),
        ..Default::default()
    };
    state.add_item(sub_item);

    // Enter submenu
    let entered = state.enter_submenu();
    assert!(entered);
    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.current_level().title, "Sub Title");
    assert_eq!(state.current_items().len(), 1);

    // Save to disk (which automatically unwinds stack and saves root)
    state.save_to_disk().unwrap();
    assert!(!state.is_modified);

    // Reload from disk and verify
    let reloaded = load_menu(&tmp_file).unwrap();
    let found = reloaded.items.iter().find(|i| i.label == "Test Submenu");
    assert!(found.is_some());
    let sub = found.unwrap().submenu.as_ref().unwrap();
    assert_eq!(sub.title, "Sub Title");
    assert_eq!(sub.items[0].label, "Sub Item 1");

    let _ = fs::remove_dir_all(&tmp_dir);
}
