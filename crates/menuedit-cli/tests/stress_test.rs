//! Stress tests for `menuedit` state transitions, deep hierarchies,
//! item insertions, and rapid reordering.

use bashmenu_core::load_menu;
use bashmenu_core::menu::{MenuItem, MenuItemType};
use menuedit_cli::MenuEditorState;
use std::fs;

#[test]
fn test_stress_menuedit_scale_and_reorder() {
    let tmp_dir = std::env::temp_dir().join(format!("menuedit_stress_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let mnu_path = tmp_dir.join("stress.mnu");

    // Initialize with a simple menu
    fs::write(
        &mnu_path,
        "items:\n  - Initial Item:\n      command: echo init\n",
    )
    .unwrap();

    let mut state = MenuEditorState::new(mnu_path.clone());

    // Stress add 500 items
    for i in 0..500 {
        let item = MenuItem {
            label: format!("Item {:04}", i),
            r#type: MenuItemType::Command,
            command: Some(format!("echo {}", i)),
            ..Default::default()
        };
        state.add_item(item);
    }
    assert_eq!(state.current_items().len(), 501);

    // Stress rapid reordering
    for _ in 0..500 {
        state.swap_down();
    }
    for _ in 0..500 {
        state.swap_up();
    }

    // Stress saving to disk
    state.save_to_disk().expect("Save must succeed");
    assert!(mnu_path.exists());

    // Load back in fresh state
    let state_reloaded = load_menu(&mnu_path).expect("Load must succeed");
    assert_eq!(state_reloaded.items.len(), 501);

    let _ = fs::remove_dir_all(&tmp_dir);
}
