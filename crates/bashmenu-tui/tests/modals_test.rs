use bashmenu_tui::{
    AsciiGridModal, ColorGridModal, ConfirmModal, FilePickerModal, InputModal, MessageModal,
    ResolvedThemeStyles,
};
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::path::Path;

#[test]
fn test_confirm_modal_interaction() {
    let mut modal = ConfirmModal::new("Exit Prompt", "Are you sure you want to exit?");
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Initial state is Yes
    assert!(modal.selected_yes);

    // Press Right toggles to No
    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert!(!modal.selected_yes);

    // Press Enter confirms No
    assert_eq!(modal.handle_key(KeyCode::Enter), Some(false));

    // Press 'y' immediately confirms Yes
    assert_eq!(modal.handle_key(KeyCode::Char('y')), Some(true));

    // Buffer rendering and drop shadow verification
    let mut buf = Buffer::empty(area);
    let inner = modal.render(area, &styles, &mut buf);

    // Verify dialog content is rendered inside
    let content = format!("{:?}", buf);
    assert!(content.contains("Exit Prompt"));
    assert!(content.contains("Are you sure you want to exit?"));

    // Verify drop shadow character '░' exists outside inner rect
    let shadow_x = inner.x + inner.width + 1;
    let shadow_y = inner.y + 2;
    assert_eq!(buf[(shadow_x, shadow_y)].symbol(), "░");
}

#[test]
fn test_input_modal_typing_and_submit() {
    let mut modal = InputModal::new("Enter Name", "Your name:", "Alice");
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    assert_eq!(modal.value, "Alice");

    // Type "!" at end
    modal.handle_key(KeyCode::Char('!'));
    assert_eq!(modal.value, "Alice!");

    // Backspace removes "!"
    modal.handle_key(KeyCode::Backspace);
    assert_eq!(modal.value, "Alice");

    // Submit with Enter
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        Some(Some("Alice".to_string()))
    );

    // Render into buffer
    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let content = format!("{:?}", buf);
    assert!(content.contains("Enter Name"));
    assert!(content.contains("Your name:"));
}

#[test]
fn test_message_modal_dismiss() {
    let modal = MessageModal::new("Alert", "File saved successfully.");
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Enter dismisses
    assert!(modal.handle_key(KeyCode::Enter));

    // Space dismisses
    assert!(modal.handle_key(KeyCode::Char(' ')));

    // Esc dismisses
    assert!(modal.handle_key(KeyCode::Esc));

    // Arrow keys do not dismiss
    assert!(!modal.handle_key(KeyCode::Left));

    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let content = format!("{:?}", buf);
    assert!(content.contains("Alert"));
    assert!(content.contains("File saved successfully."));
}

#[test]
fn test_grid_pickers_navigation() {
    // 256 Color grid (initial 0 = black)
    let mut color_modal = ColorGridModal::new("Select Color", 0);
    assert_eq!(color_modal.selected_code, 0);

    // Down moves +16
    color_modal.handle_key(KeyCode::Down);
    assert_eq!(color_modal.selected_code, 16);

    // Right moves +1
    color_modal.handle_key(KeyCode::Right);
    assert_eq!(color_modal.selected_code, 17);

    // Enter submits
    assert_eq!(color_modal.handle_key(KeyCode::Enter), Some(Some(17)));

    // CP437 ASCII grid
    let mut ascii_modal = AsciiGridModal::new("Select Character", 196); // 196 = '─'
    assert_eq!(ascii_modal.selected_code, 196);

    // Enter submits CP437 character
    assert_eq!(ascii_modal.handle_key(KeyCode::Enter), Some(Some('─')));
}

#[test]
fn test_file_picker_navigation() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut picker = FilePickerModal::new("Pick Configuration", &fixture_dir, false);
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Render picker
    let mut buf = Buffer::empty(area);
    picker.render(area, &styles, &mut buf);
    let content = format!("{:?}", buf);

    assert!(content.contains("Pick Configuration"));
    assert!(content.contains("bashmenu.yml"));
}
