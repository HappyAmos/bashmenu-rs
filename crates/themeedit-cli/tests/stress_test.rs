//! Stress tests for `themeedit` state management, tree navigation,
//! and adversarial theme mutations.

use std::path::PathBuf;
use themeedit_cli::{ThemeEditorState, TokenCategory};

#[test]
fn test_stress_themeedit_adversarial_paths_and_targets() {
    let dummy_path = PathBuf::from("/nonexistent/path/never/exists/bashmenu.themes");

    // Must gracefully fall back to default theme without panicking
    let mut state = ThemeEditorState::new(dummy_path, Some("nonexistent_theme".to_string()));
    assert!(!state.current_name.is_empty());

    // Stress visible rows and row selection
    for _ in 0..100 {
        let rows = state.visible_rows();
        for (i, _) in rows.iter().enumerate() {
            state.select_visible_row(i);
        }
    }

    // Stress toggle expand/collapse on all categories
    for _ in 0..100 {
        state.toggle_category(TokenCategory::Theme);
        state.toggle_category(TokenCategory::Colors);
        state.toggle_category(TokenCategory::Borders);
    }

    // Stress modifying arbitrary color tokens across range
    for i in 0..256 {
        state.set_color_token("text", i as u8);
        state.set_color_token("background", (255 - i) as u8);
    }
    assert!(state.is_modified);
}
