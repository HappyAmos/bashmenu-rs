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

#[test]
fn test_note_viewer_footer_wrapping() {
    use bashmenu_tui::{NoteViewerAction, NoteViewerModal};

    let mut viewer = NoteViewerModal::new(
        "Meeting Notes",
        "/tmp/notes.yml",
        "Discussion on architecture and next steps for BashMenu.",
    );
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 70, 20);

    // Render into narrow area to trigger wrapping
    let mut buf = Buffer::empty(area);
    let inner = viewer.render(area, &styles, &mut buf);
    assert!(inner.width > 0);
    assert!(inner.height > 0);

    // Verify key bindings
    assert_eq!(
        viewer.handle_key(KeyCode::Char('e')),
        NoteViewerAction::Edit
    );
    assert_eq!(
        viewer.handle_key(KeyCode::Char('m')),
        NoteViewerAction::Rename
    );
    assert_eq!(
        viewer.handle_key(KeyCode::Char('d')),
        NoteViewerAction::Delete
    );
    assert_eq!(viewer.handle_key(KeyCode::Esc), NoteViewerAction::Close);
}

#[test]
fn test_note_editor_modal_typing_and_saving() {
    use bashmenu_tui::{NoteEditorAction, NoteEditorModal};
    use crossterm::event::KeyModifiers;

    let mut editor = NoteEditorModal::new("Shopping List", "/tmp/shopping.yml", "- [ ] Apples");

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Move cursor to end of first line
    editor.cursor_col = editor.lines[0].len();

    // Press Enter: should auto-continue bullet "- [ ] " on next line
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(editor.lines.len(), 2);
    assert_eq!(editor.lines[1], "- [ ] ");

    // Type "Bananas"
    for c in "Bananas".chars() {
        editor.handle_key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(editor.lines[1], "- [ ] Bananas");

    // Render editor modal
    let mut buf = Buffer::empty(area);
    let inner = editor.render(area, &styles, &mut buf);
    assert!(inner.width > 0);

    // Save with Ctrl+S
    let act = editor.handle_key(KeyCode::Char('s'), KeyModifiers::CONTROL);
    match act {
        NoteEditorAction::Save(content) => {
            assert_eq!(editor.title, "Shopping List");
            assert!(content.contains("- [ ] Apples"));
            assert!(content.contains("- [ ] Bananas"));
        }
        _ => panic!("Expected Save action on Ctrl+S"),
    }
}

#[test]
fn test_note_editor_soft_wrapping_and_no_line_numbers() {
    use bashmenu_tui::NoteEditorModal;
    use crossterm::event::KeyModifiers;

    // Create a note with a long line (70 characters)
    let long_line =
        "This is a very long line in a notebook note that should wrap across lines cleanly.";
    let mut editor = NoteEditorModal::new("Wrapped Note", "/tmp/wrapped.yml", long_line);
    editor.last_inner_w = 30;

    // Compute visual lines with a narrow width of 30
    let (vlines, cur_vidx) = editor.compute_visual_lines(30);
    assert!(
        vlines.len() >= 3,
        "70 chars wrapped at 30 should yield at least 3 visual lines"
    );
    assert_eq!(cur_vidx, 0);

    // Press Down key: cursor should move to second visual line within the same logical line
    editor.handle_key(KeyCode::Down, KeyModifiers::NONE);
    let (_, next_vidx) = editor.compute_visual_lines(30);
    assert_eq!(next_vidx, 1);
    assert_eq!(editor.cursor_row, 0, "Still on logical line 0");
    assert!(
        editor.cursor_col >= 30,
        "Cursor column advanced into second chunk"
    );

    // Render into buffer and verify line numbers (e.g. ' 1 ') are not rendered
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    editor.render(area, &styles, &mut buf);
    let content = format!("{:?}", buf);
    assert!(
        !content.contains(" 1  ") && !content.contains(" 1 │"),
        "Line numbers must not be rendered"
    );
    assert!(content.contains("This is a very long"));
}

#[test]
fn test_note_viewer_markdown_code_rendering() {
    use bashmenu_tui::NoteViewerModal;

    let md_note = "\
# BashMenu Release

Run the test suite with `cargo test`:

```bash
cargo test --workspace
```

- [ ] Unfinished task
- [x] Finished task

> Documentation note
";

    let mut viewer = NoteViewerModal::new("Preview Test", "/tmp/test.yml", md_note);
    viewer.show_completed = true;
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 70, 25);
    let mut buf = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf);

    let content = format!("{:?}", buf);
    // Header rendered
    assert!(content.contains("# BashMenu Release"));
    // Code block frame and content rendered
    assert!(content.contains("┌─ [bash]"));
    assert!(content.contains("│ cargo test --workspace"));
    assert!(content.contains("└─"));
    // Checkboxes rendered
    assert!(content.contains("- [ ] Unfinished task"));
    assert!(content.contains("- [x] Finished task"));
    // Blockquote rendered
    assert!(content.contains("│ Documentation note"));
}

#[test]
fn test_note_editor_word_wrapping_and_end_key_enter() {
    use bashmenu_tui::NoteEditorModal;
    use crossterm::event::KeyModifiers;

    let text = "- [ ] Appears to be some conflict with the autocomplete in the notes editors. Pressing enter at the end of an existing checkbox line, seems to grab characters from the beginning of the line and appending them to the new autocreated checkbox";
    let mut editor = NoteEditorModal::new("Bug Repro", "/tmp/repro.yml", text);
    editor.last_inner_w = 40;

    // Word wrap check: no word split
    let (vlines, _) = editor.compute_visual_lines(40);
    for vl in &vlines {
        let chunk: String = vl.chars.iter().collect();
        assert!(chunk.len() <= 40);
    }

    // Press End key: cursor MUST jump to the logical end of the line
    editor.handle_key(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(editor.cursor_col, text.chars().count());

    // Press Enter at the end of the line: must create clean empty new checkbox
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(editor.lines.len(), 2);
    assert_eq!(editor.lines[0], text);
    assert_eq!(editor.lines[1], "- [ ] ");
    assert_eq!(editor.cursor_col, 6);
}

#[test]
fn test_note_editor_mouse_click_places_cursor() {
    use bashmenu_tui::NoteEditorModal;
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let text = "Line 0: First note row\nLine 1: Second note row with some words";
    let mut editor = NoteEditorModal::new("Mouse Test", "/tmp/mouse.yml", text);
    editor.last_inner_w = 60;

    let area = Rect::new(0, 0, 80, 24);
    let styles = ResolvedThemeStyles::default();
    let mut buf = Buffer::empty(area);
    let inner = editor.render(area, &styles, &mut buf);

    // Initial cursor at (0, 0)
    assert_eq!(editor.cursor_row, 0);
    assert_eq!(editor.cursor_col, 0);

    // Click on row 1, col 10
    let click_ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inner.x + 2 + 10,
        row: inner.y + 1 + 1,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    editor.handle_mouse(click_ev, area);
    assert_eq!(editor.cursor_row, 1);
    assert_eq!(editor.cursor_col, 10);
}

#[test]
fn test_headings_and_viewer_no_background_artifact() {
    use bashmenu_tui::{HelpModal, NoteViewerModal};
    use ratatui::style::Color;

    let styles = ResolvedThemeStyles::default();
    // Default highlight style has bg: Cyan
    assert_eq!(styles.highlight.bg, Some(Color::Cyan));

    // 1. HelpModal ## and ### headings must NOT have highlight background
    let help_text = "## Secondary Section\n### Sub Section";
    let help = HelpModal::new("Help Test", help_text);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    help.render(area, &styles, &mut buf);

    // Search cells for 'Secondary Section' and verify none have bg: Cyan
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            if cell.symbol() == "S" && cell.style().bg == Some(Color::Cyan) {
                // If this cell is part of "Secondary Section", fail
                let mut row_chars = String::new();
                for dx in 0..20 {
                    if x + dx < area.width {
                        row_chars.push_str(buf.cell((x + dx, y)).unwrap().symbol());
                    }
                }
                assert!(
                    !row_chars.starts_with("Secondary"),
                    "Header text must not have background highlight"
                );
            }
        }
    }

    // 2. NoteViewerModal ## headings and selected line must NOT wash background over text
    let note = "## Task Heading\n- [ ] Yellow item with text";
    let mut viewer = NoteViewerModal::new("Viewer Test", "/tmp/v.yml", note);
    viewer.selected_line = 1;
    let mut buf2 = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf2);

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf2.cell((x, y)).unwrap();
            if cell.symbol() == "Y" {
                assert_ne!(
                    cell.style().bg,
                    Some(Color::Cyan),
                    "Item text must not have background patched"
                );
            }
        }
    }
}

#[test]
fn test_help_modal_renders_at_90_percent_width() {
    use bashmenu_tui::HelpModal;

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 120, 30);
    let mut buf = Buffer::empty(area);

    let help = HelpModal::new("Wide Help", "Sample help text line.");
    let inner = help.render(area, &styles, &mut buf);

    // 90% of 120 is 108. Inner width is 108 - 2 borders = 106.
    assert_eq!(inner.width, 106);
}
