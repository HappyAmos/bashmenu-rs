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
