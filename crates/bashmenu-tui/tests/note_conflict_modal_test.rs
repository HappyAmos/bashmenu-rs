use bashmenu_tui::{NoteConflictChoice, NoteConflictModal, ResolvedThemeStyles};
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_note_conflict_modal_navigation_and_selection() {
    let mut modal = NoteConflictModal::new(
        "Meeting Notes",
        "notes.yml",
        "Local Line 1\nLocal Line 2",
        "Disk Line 1\nDisk Line 2",
        "Base Line 1",
    );

    assert_eq!(modal.selected_index, 0); // [ Merge ]

    // Cycle right
    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 1); // [ Overwrite Disk ]

    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 2); // [ Reload Disk ]

    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 3); // [ Save As Copy ]

    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 4); // [ Cancel ]

    // Wraparound
    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 0);

    // Enter confirms Merge
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        Some(NoteConflictChoice::Merge)
    );

    // Direct shortcut keys
    assert_eq!(
        modal.handle_key(KeyCode::Char('o')),
        Some(NoteConflictChoice::Overwrite)
    );
    assert_eq!(
        modal.handle_key(KeyCode::Char('r')),
        Some(NoteConflictChoice::Reload)
    );
    assert_eq!(
        modal.handle_key(KeyCode::Char('s')),
        Some(NoteConflictChoice::SaveAsCopy)
    );
    assert_eq!(
        modal.handle_key(KeyCode::Esc),
        Some(NoteConflictChoice::Cancel)
    );
}

#[test]
fn test_note_conflict_modal_render() {
    let modal = NoteConflictModal::new(
        "Meeting Notes",
        "notes.yml",
        "Local Line 1",
        "Disk Line 1",
        "Base Line 1",
    );
    let area = Rect::new(0, 0, 80, 24);
    let styles = ResolvedThemeStyles::default();
    let mut buf = Buffer::empty(area);

    modal.render(area, &styles, &mut buf);

    // Ensure buttons are rendered in the buffer
    let mut buf_str = String::new();
    for y in 0..24 {
        for x in 0..80 {
            buf_str.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(buf_str.contains("Merge"));
    assert!(buf_str.contains("Overwrite Disk"));
    assert!(buf_str.contains("Reload Disk"));
    assert!(buf_str.contains("Save As Copy"));
    assert!(buf_str.contains("Cancel"));
}
