use bashmenu_tui::{
    AsciiGridModal, ColorGridModal, ConfirmChoice, ConfirmModal, FilePickerModal, InputModal,
    MessageModal, NoteEditorAction, NoteEditorModal, NoteViewerAction, NoteViewerModal, RadioModal,
    ResolvedThemeStyles, ToggleChoice, ToggleModal,
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

    // Initial state is Yes (index 0)
    assert!(modal.selected_yes);
    assert_eq!(modal.selected_index, 0);

    // Press Right cycles to No (index 1)
    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert!(!modal.selected_yes);
    assert_eq!(modal.selected_index, 1);

    // Press Right cycles to Cancel (index 2)
    assert_eq!(modal.handle_key(KeyCode::Right), None);
    assert_eq!(modal.selected_index, 2);

    // Press Enter on Cancel returns Some(ConfirmChoice::Cancel)
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        Some(ConfirmChoice::Cancel)
    );

    // Reset to index 1 (No) and press Enter
    modal.selected_index = 1;
    assert_eq!(modal.handle_key(KeyCode::Enter), Some(ConfirmChoice::No));

    // Press 'y' immediately confirms Yes
    assert_eq!(
        modal.handle_key(KeyCode::Char('y')),
        Some(ConfirmChoice::Yes)
    );

    // Press 'n' immediately confirms No
    assert_eq!(
        modal.handle_key(KeyCode::Char('n')),
        Some(ConfirmChoice::No)
    );

    // Press 'c' or Esc immediately confirms Cancel
    assert_eq!(
        modal.handle_key(KeyCode::Char('c')),
        Some(ConfirmChoice::Cancel)
    );
    assert_eq!(modal.handle_key(KeyCode::Esc), Some(ConfirmChoice::Cancel));

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

    // Verify 3-digit decimal badge rendering in buffers
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 30);
    let mut color_buf = Buffer::empty(area);
    color_modal.render(area, &styles, &mut color_buf);
    let color_text: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| color_buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        color_text.contains("Color: 017"),
        "Expected color modal to display 3-digit decimal badge 'Color: 017', got: {}",
        color_text
    );
    assert!(
        color_text.contains(" X"),
        "Expected color modal to display uppercase 'X' indicator for selected color, got: {}",
        color_text
    );

    let mut ascii_buf = Buffer::empty(area);
    ascii_modal.render(area, &styles, &mut ascii_buf);
    let ascii_text: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| ascii_buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        ascii_text.contains("ASCII: 196"),
        "Expected ASCII modal to display 3-digit decimal badge 'ASCII: 196', got: {}",
        ascii_text
    );
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
    use ratatui::style::{Color, Modifier, Style};

    // Simulate a dark theme like alaska-mountain with background 235
    let styles = ResolvedThemeStyles {
        background: Color::Indexed(235),
        highlight: Style::default().bg(Color::Cyan),
        ..Default::default()
    };

    // 1. HelpModal ## and ### headings and text must match styles.background (not Reset/Black or Cyan)
    let help_text = "## Secondary Section\n### Sub Section\nRegular help content line";
    let help = HelpModal::new("Help Test", help_text);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    help.render(area, &styles, &mut buf);

    // Check cells: any cell with letters must have bg == Color::Indexed(235)
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            let sym = cell.symbol();
            if sym == "S" || sym == "R" {
                assert_eq!(
                    cell.style().bg,
                    Some(Color::Indexed(235)),
                    "Text must have popup window background (235), not black/Reset"
                );
            }
        }
    }

    // 2. NoteViewerModal: no '►' indicator, selected item is inverted with Modifier::REVERSED
    let note = "## Task Heading\n- [ ] Yellow item with text";
    let mut viewer = NoteViewerModal::new("Viewer Test", "/tmp/v.yml", note);
    viewer.selected_line = 1;
    let mut buf2 = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf2);

    let content = format!("{:?}", buf2);
    assert!(!content.contains('►'), "► indicator must not be rendered");

    // The selected line 1 ('Yellow item with text') must have REVERSED modifier
    let mut found_reversed = false;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf2.cell((x, y)).unwrap();
            if cell.symbol() == "Y" {
                assert_eq!(
                    cell.style().bg,
                    Some(Color::Indexed(235)),
                    "Background must match styles.background"
                );
                assert!(
                    cell.style().add_modifier.contains(Modifier::REVERSED),
                    "Selected item text must be inverted (REVERSED)"
                );
                found_reversed = true;
            }
        }
    }
    assert!(found_reversed, "Must find selected inverted item cell");
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

#[test]
fn test_help_modal_renders_markdown_table() {
    use bashmenu_tui::HelpModal;
    use ratatui::style::{Color, Style};

    let styles = ResolvedThemeStyles {
        background: Color::Indexed(236),
        text: Style::default().fg(Color::White),
        accent: Style::default().fg(Color::Yellow),
        ..Default::default()
    };
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    let md_table = "\
# Table Title

| Syntax | Action |
| :--- | :--- |
| `F1` | Help Manual |
| `^S` | Save File |
";
    let help = HelpModal::new("Table Test", md_table);
    help.render(area, &styles, &mut buf);

    // Look for box drawing characters ┌, ┬, ┐, │, ├, ┼, ┤, └, ┴, ┘
    let mut found_top_left = false;
    let mut found_pipe = false;
    let mut found_header = false;
    let mut found_cell = false;

    for y in 0..area.height {
        let mut row_str = String::new();
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            row_str.push_str(cell.symbol());
            if cell.symbol() == "┌" {
                found_top_left = true;
            }
            if cell.symbol() == "│" {
                found_pipe = true;
            }
        }
        if row_str.contains("Syntax") && row_str.contains("Action") {
            found_header = true;
        }
        if row_str.contains("`F1`") && row_str.contains("Help Manual") {
            found_cell = true;
        }
    }

    assert!(found_top_left, "Must render box-drawing top border ┌");
    assert!(found_pipe, "Must render vertical column divider │");
    assert!(
        found_header,
        "Must render table header row with Syntax and Action"
    );
    assert!(
        found_cell,
        "Must render table data row with `F1` and Help Manual"
    );
}

#[test]
fn test_modal_frame_close_button_rendering_and_clicking() {
    use bashmenu_tui::{is_close_button_clicked, render_modal_frame, HelpModal};
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::style::{Color, Style};

    let styles = ResolvedThemeStyles {
        border: Color::Cyan,
        window_close_button: Style::default().fg(Color::Red),
        ..Default::default()
    };
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    let inner = render_modal_frame(area, 60, 15, "Close Test", &styles, &mut buf);

    // Verify top border title format: ┌─[ Close Test ]─────...
    let top_y = inner.y - 1;
    let corner_x = inner.x - 1;
    assert_eq!(buf[(corner_x, top_y)].symbol(), "┌");
    assert_eq!(buf[(corner_x + 1, top_y)].symbol(), "─");
    assert_eq!(buf[(corner_x + 2, top_y)].symbol(), "[");

    // Top-right corner character is at inner.x + inner.width.
    // Close button [X] is at inner.x + inner.width - 3 .. -1.
    let close_start = inner.x + inner.width - 3;

    let cell_bracket_open = buf.cell((close_start, top_y)).unwrap();
    assert_eq!(cell_bracket_open.symbol(), "[");
    assert_eq!(cell_bracket_open.style().fg, Some(Color::Cyan));

    let cell_x = buf.cell((close_start + 1, top_y)).unwrap();
    assert_eq!(cell_x.symbol(), "X");
    assert_eq!(cell_x.style().fg, Some(Color::Red));

    let cell_bracket_close = buf.cell((close_start + 2, top_y)).unwrap();
    assert_eq!(cell_bracket_close.symbol(), "]");
    assert_eq!(cell_bracket_close.style().fg, Some(Color::Cyan));

    // Verify click detection
    assert!(is_close_button_clicked(inner, top_y, close_start + 1));
    assert!(!is_close_button_clicked(inner, top_y + 1, close_start + 1));

    // Verify HelpModal mouse click on close button dismisses modal
    let mut help = HelpModal::new("Help", "Content");
    let help_inner = help.render(area, &styles, &mut buf);
    let help_close_x = help_inner.x + help_inner.width - 2; // 'X' position
    let help_top_y = help_inner.y - 1;
    let mouse_ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: help_close_x,
        row: help_top_y,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    assert!(help.handle_mouse(mouse_ev, area));
}

#[test]
fn test_note_viewer_hidden_completed_items_not_selectable() {
    use bashmenu_tui::NoteViewerModal;
    use crossterm::event::KeyCode;

    let note = "- [x] Done item 1\n- [ ] Todo item 2\n- [x] Done item 3\n- [ ] Todo item 4";
    let mut viewer = NoteViewerModal::new("Viewer", "/tmp/v.yml", note);
    // Initially show_completed is false, so items 0 and 2 are hidden
    assert!(!viewer.show_completed);

    // Initial navigation should only step between index 1 and 3
    viewer.handle_key(KeyCode::Home);
    assert_eq!(viewer.selected_line, 1, "First visible item is index 1");

    viewer.handle_key(KeyCode::Down);
    assert_eq!(
        viewer.selected_line, 3,
        "Next visible item is index 3, skipping hidden index 2"
    );

    viewer.handle_key(KeyCode::Up);
    assert_eq!(viewer.selected_line, 1, "Previous visible item is index 1");
}

#[test]
fn test_note_viewer_and_editor_checkbox_margin_wrapping() {
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 40, 20);

    // 1. NoteViewerModal wraps checkbox items with margin
    let note =
        "- [ ] This is a very long checkbox note item that will wrap onto multiple visual lines";
    let mut viewer = NoteViewerModal::new("Viewer", "/tmp/v.yml", note);
    let mut buf = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf);

    // Line 1 should have "- [ ] This is a"
    // Line 2 should have "      very long" (prefixed with 6 spaces margin)
    let mut found_continuation_with_margin = false;
    for y in 0..area.height {
        let line_chars: String = (0..area.width)
            .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        if line_chars.contains("      ")
            && (line_chars.contains("very")
                || line_chars.contains("checkbox")
                || line_chars.contains("wrap"))
        {
            found_continuation_with_margin = true;
            break;
        }
    }
    assert!(
        found_continuation_with_margin,
        "NoteViewer must prepend continuation lines with margin spaces"
    );

    // 2. NoteEditorModal compute_visual_lines wraps checkbox items with margin
    let editor = NoteEditorModal::new("Editor", "/tmp/e.yml", note);
    let (vlines, _) = editor.compute_visual_lines(30);
    assert!(
        vlines.len() >= 2,
        "Long line must wrap into multiple visual lines"
    );
    assert_eq!(
        vlines[0].indent_margin, 0,
        "First chunk has no extra indent margin"
    );
    assert_eq!(
        vlines[1].indent_margin, 6,
        "Continuation chunk must have 6 spaces margin"
    );
}

#[test]
fn test_note_viewer_and_editor_unordered_list_margin_wrapping() {
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 40, 20);

    // 1. NoteViewerModal wraps bullet items with indent and margin
    let note =
        "  - This is an indented bullet item that will wrap onto multiple visual lines cleanly";
    let mut viewer = NoteViewerModal::new("Viewer", "/tmp/v.yml", note);
    let mut buf = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf);

    // First visual line should have "  • " (2 spaces indent)
    // Continuation line should have "    " (2 indent + 2 marker = 4 spaces margin)
    let mut found_bullet_continuation = false;
    for y in 0..area.height {
        let line_chars: String = (0..area.width)
            .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        if line_chars.contains("    ")
            && (line_chars.contains("bullet")
                || line_chars.contains("wrap")
                || line_chars.contains("lines"))
        {
            found_bullet_continuation = true;
            break;
        }
    }
    assert!(
        found_bullet_continuation,
        "NoteViewer must prepend unordered list continuation lines with margin spaces matching indentation"
    );

    // 2. NoteEditorModal compute_visual_lines wraps bullet items with margin
    let editor = NoteEditorModal::new("Editor", "/tmp/e.yml", note);
    let (vlines, _) = editor.compute_visual_lines(30);
    assert!(
        vlines.len() >= 2,
        "Long bullet item must wrap into multiple visual lines"
    );
    assert_eq!(
        vlines[0].indent_margin, 0,
        "First chunk has no extra indent margin"
    );
    assert_eq!(
        vlines[1].indent_margin, 4,
        "Continuation chunk must have 4 spaces margin (2 indent + 2 marker)"
    );
}

#[test]
fn test_note_viewer_full_edit_action() {
    use crossterm::event::KeyCode;

    let mut viewer = NoteViewerModal::new("Test Note", "/tmp/notes.yml", "# Content");

    // Pressing 'f' triggers FullEdit
    assert_eq!(
        viewer.handle_key(KeyCode::Char('f')),
        NoteViewerAction::FullEdit
    );
    // Pressing 'F' triggers FullEdit
    assert_eq!(
        viewer.handle_key(KeyCode::Char('F')),
        NoteViewerAction::FullEdit
    );
    // Pressing F3 triggers FullEdit
    assert_eq!(viewer.handle_key(KeyCode::F(3)), NoteViewerAction::FullEdit);

    // Render should include [F] Full Edit in footer
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf);

    let mut found_full_edit = false;
    for y in 0..area.height {
        let line_chars: String = (0..area.width)
            .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        if line_chars.contains("[F] Full Edit") {
            found_full_edit = true;
            break;
        }
    }
    assert!(found_full_edit, "Footer must contain '[F] Full Edit'");
}

#[test]
fn test_modal_no_triangle_indicator() {
    use bashmenu_tui::{ColorGridModal, FilePickerModal, ThemePickerModal};

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // 1. ThemePickerModal
    let mut theme_picker =
        ThemePickerModal::new(vec!["theme1".to_string(), "theme2".to_string()], "theme1");
    let mut buf1 = Buffer::empty(area);
    theme_picker.render(area, &styles, &mut buf1);
    let s1 = format!("{:?}", buf1);
    assert!(!s1.contains('►'), "ThemePickerModal must not render '►'");

    // 2. FilePickerModal
    let mut file_picker = FilePickerModal::new("Pick File", &std::path::PathBuf::from("/"), false);
    let mut buf2 = Buffer::empty(area);
    file_picker.render(area, &styles, &mut buf2);
    let s2 = format!("{:?}", buf2);
    assert!(!s2.contains('►'), "FilePickerModal must not render '►'");

    // 3. ColorGridModal
    let grid_picker = ColorGridModal::new("Color Grid", 10);
    let mut buf3 = Buffer::empty(area);
    grid_picker.render(area, &styles, &mut buf3);
    let s3 = format!("{:?}", buf3);
    assert!(!s3.contains('►'), "ColorGridModal must not render '►'");
}

#[test]
fn test_toggle_modal_interaction() {
    let mut modal = ToggleModal::new(
        "Toggle Nerd Fonts",
        "Enable colorful menu emojis and Nerd Fonts?",
    );
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Initial focus is True (0)
    assert_eq!(modal.focused_idx, 0);

    // Tab moves to False (1)
    modal.handle_key(KeyCode::Tab);
    assert_eq!(modal.focused_idx, 1);

    // Tab moves to Cancel (2)
    modal.handle_key(KeyCode::Tab);
    assert_eq!(modal.focused_idx, 2);

    // Tab wraps to True (0)
    modal.handle_key(KeyCode::Tab);
    assert_eq!(modal.focused_idx, 0);

    // Enter returns Some(ToggleChoice::True)
    assert_eq!(modal.handle_key(KeyCode::Enter), Some(ToggleChoice::True));

    // Direct key shortcuts: 'f' returns False, 't' returns True, 'c' returns Cancel
    assert_eq!(
        modal.handle_key(KeyCode::Char('f')),
        Some(ToggleChoice::False)
    );
    assert_eq!(
        modal.handle_key(KeyCode::Char('t')),
        Some(ToggleChoice::True)
    );
    assert_eq!(
        modal.handle_key(KeyCode::Char('c')),
        Some(ToggleChoice::Cancel)
    );
    assert_eq!(modal.handle_key(KeyCode::Esc), Some(ToggleChoice::Cancel));

    // Rendering test
    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let s = format!("{:?}", buf);
    assert!(s.contains("Toggle Nerd Fonts"));
    assert!(s.contains("True"));
    assert!(s.contains("False"));
    assert!(s.contains("Cancel"));
}

#[test]
fn test_radio_modal_interaction() {
    let options = vec![
        ("default".to_string(), "Default ($EDITOR)".to_string()),
        ("builtin".to_string(), "Built-in (BashEdit)".to_string()),
        ("vim".to_string(), "Vim".to_string()),
    ];
    let mut modal = RadioModal::new(
        "Pager Text Editor",
        "Select editor launched when pressing [e] in pager:",
        options,
        "default",
    );
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    assert_eq!(modal.selected_val, "default");
    assert_eq!(modal.cursor_idx, 0);

    // Down key moves cursor to option 1 ("builtin")
    modal.handle_key(KeyCode::Down);
    assert_eq!(modal.cursor_idx, 1);

    // Space key selects current cursor option ("builtin")
    modal.handle_key(KeyCode::Char(' '));
    assert_eq!(modal.selected_val, "builtin");

    // Number key '3' immediately selects option 3 ("vim")
    modal.handle_key(KeyCode::Char('3'));
    assert_eq!(modal.selected_val, "vim");

    // Enter confirms selection
    assert_eq!(
        modal.handle_key(KeyCode::Enter),
        Some(Some("vim".to_string()))
    );

    // Esc cancels
    assert_eq!(modal.handle_key(KeyCode::Esc), Some(None));

    // Rendering test
    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let s = format!("{:?}", buf);
    assert!(s.contains("Pager Text Editor"));
    assert!(s.contains("Default ($EDITOR)"));
    assert!(s.contains("Built-in (BashEdit)"));
    assert!(s.contains("Vim"));
    assert!(s.contains("●"));
}

#[test]
fn test_note_editor_initial_cursor_and_ctrl_word_navigation() {
    use crossterm::event::KeyModifiers;

    let content = "First line content.\nSecond line here.\nThird line ends.";
    let mut editor = NoteEditorModal::new("Test Note", "/tmp/n.yml", content).with_cursor(1, 0);

    // Initial position matches view mode selected_line (row 1, col 0)
    assert_eq!(editor.cursor_row, 1);
    assert_eq!(editor.cursor_col, 0);

    // Ctrl+Right moves word right -> "line" (col 7)
    editor.handle_key(KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(editor.cursor_col, 7);

    // Ctrl+Right moves word right -> "here." (col 12)
    editor.handle_key(KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(editor.cursor_col, 12);

    // Ctrl+Left moves word left -> "line" (col 7)
    editor.handle_key(KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(editor.cursor_col, 7);

    // Ctrl+Left moves word left -> "Second" (col 0)
    editor.handle_key(KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(editor.cursor_col, 0);
}

#[test]
fn test_input_modal_status_gutter_no_overflow_and_word_navigation() {
    use crossterm::event::KeyModifiers;

    let long_gutter = "| Theme: {theme} | {user} | Battery: {battery} ({battery_stat}) | {user.localip} | [b]{date_time_24_short}[/b]";
    let prompt = "Enter status gutter format. Available options: {user}, {battery}, {date}, {date_time_24}, {date_time_24_short}, {date_time_12}, {date_time_12_short}, {host}, {user-mode}, {version}, {utc_seconds}";
    let mut modal = InputModal::new("Configure Status Gutter", prompt, long_gutter);

    let area = Rect::new(0, 0, 80, 24);
    let styles = ResolvedThemeStyles::default();
    let mut buf = Buffer::empty(area);

    let inner = modal.render(area, &styles, &mut buf);
    assert!(inner.width > 0);

    // Verify right border is not overwritten by text:
    // Any cell to the right of the dialog right frame (inner.x + inner.width) should NOT contain input chars
    let right_boundary = inner.x + inner.width;
    for y in inner.y..inner.y + inner.height {
        for x in (right_boundary + 1)..area.width {
            let ch = buf[(x, y)].symbol();
            // Should be space or empty, not letters from input or prompt
            assert!(
                ch == " " || ch == "░",
                "Cell at ({}, {}) has unexpected character '{}' exceeding modal width",
                x,
                y,
                ch
            );
        }
    }

    // Test Ctrl+Left and Ctrl+Right word navigation in InputModal
    modal.cursor_pos = 0;
    modal.handle_key_with_modifiers(KeyCode::Right, KeyModifiers::CONTROL);
    assert!(modal.cursor_pos > 0);

    let next_pos = modal.cursor_pos;
    modal.handle_key_with_modifiers(KeyCode::Right, KeyModifiers::CONTROL);
    assert!(modal.cursor_pos > next_pos);

    modal.handle_key_with_modifiers(KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(modal.cursor_pos, next_pos);
}

#[test]
fn test_note_viewer_and_editor_scrollbar_when_lines_exceed_window_height() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // 1. Short note: does NOT have more lines than window height -> NO scrollbar
    let short_note = "Short line 1\nShort line 2\n";
    let mut viewer_short = NoteViewerModal::new("Short Note", "/tmp/short.yml", short_note);
    let mut buf = Buffer::empty(area);
    let inner_short = viewer_short.render(area, &styles, &mut buf);
    let sb_x_short = inner_short.x + inner_short.width - 1;
    // Right border is at sb_x_short + 1
    assert_eq!(buf[(sb_x_short + 1, inner_short.y + 1)].symbol(), "│");
    // Column sb_x_short should NOT have scrollbar arrow '▲'
    assert_ne!(
        buf[(sb_x_short, inner_short.y + 1)].symbol(),
        "▲",
        "Short note should not render scrollbar arrow"
    );

    // 2. Long note: 50 lines -> HAS more lines than window height -> Renders scrollbar
    let mut long_lines = Vec::new();
    for i in 1..=50 {
        long_lines.push(format!("Note entry line number {}", i));
    }
    let long_note = long_lines.join("\n");
    let mut viewer_long = NoteViewerModal::new("Long Note", "/tmp/long.yml", &long_note);
    let mut buf_long = Buffer::empty(area);
    let inner_long = viewer_long.render(area, &styles, &mut buf_long);

    let sb_x = inner_long.x + inner_long.width - 1;
    // Right border must be at sb_x + 1
    assert_eq!(
        buf_long[(sb_x + 1, inner_long.y + 1)].symbol(),
        "│",
        "Right border must be at sb_x + 1"
    );

    // Scrollbar top arrow must be at sb_x, 1 character left of right border
    let content_start_y = inner_long.y + 1;
    assert_eq!(
        buf_long[(sb_x, content_start_y)].symbol(),
        "▲",
        "Top arrow '▲' must be rendered 1 char left of right border"
    );

    // Scrollbar thumb '█' must be rendered in column sb_x
    let mut found_thumb = false;
    let mut found_track = false;
    let mut down_arrow_y = 0;
    for y in content_start_y..inner_long.y + inner_long.height {
        let sym = buf_long[(sb_x, y)].symbol();
        if sym == "█" {
            found_thumb = true;
        } else if sym == "░" {
            found_track = true;
        } else if sym == "▼" {
            down_arrow_y = y;
        }
    }
    assert!(found_thumb, "Scrollbar thumb '█' must be rendered");
    assert!(found_track, "Scrollbar track '░' must be rendered");
    assert!(
        down_arrow_y > 0,
        "Scrollbar bottom arrow '▼' must be rendered"
    );

    // Test mouse interaction on scrollbar:
    // Clicking ▼ scrolls down
    let click_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: sb_x,
        row: down_arrow_y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    viewer_long.handle_mouse(click_down, area);
    assert!(
        viewer_long.scroll_offset > 0,
        "Clicking scrollbar down arrow must advance scroll_offset"
    );

    // Clicking ▲ scrolls back up
    let click_up = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: sb_x,
        row: content_start_y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    viewer_long.handle_mouse(click_up, area);
    assert_eq!(
        viewer_long.scroll_offset, 0,
        "Clicking scrollbar up arrow must reduce scroll_offset"
    );

    // 3. NoteEditorModal: long note also renders scrollbar
    let mut editor_long = NoteEditorModal::new("Edit Long Note", "/tmp/long.yml", &long_note);
    let mut buf_ed = Buffer::empty(area);
    let inner_ed = editor_long.render(area, &styles, &mut buf_ed);
    let sb_x_ed = inner_ed.x + inner_ed.width - 1;
    assert_eq!(buf_ed[(sb_x_ed + 1, inner_ed.y + 1)].symbol(), "│");
    assert_eq!(buf_ed[(sb_x_ed, inner_ed.y + 1)].symbol(), "▲");
}

#[test]
fn test_note_viewer_scrolling_when_show_done_enabled() {
    use bashmenu_tui::{NoteViewerAction, NoteViewerModal};
    use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);

    // Create a note with 30 items: odd are completed [x], even are todo [ ]
    let mut note_lines = Vec::new();
    for i in 0..30 {
        if i % 2 == 1 {
            note_lines.push(format!("- [x] Completed task {}", i));
        } else {
            note_lines.push(format!("- [ ] Pending task {}", i));
        }
    }
    let note_content = note_lines.join("\n");
    let mut viewer = NoteViewerModal::new("Tasks Note", "/tmp/tasks.yml", &note_content);

    // Initial state: show_completed is false, only 15 pending tasks
    let mut buf = Buffer::empty(area);
    viewer.render(area, &styles, &mut buf);
    assert!(!viewer.show_completed);
    assert_eq!(viewer.scroll_offset, 0);

    // Press 'h' to enable "Show Done"
    viewer.handle_key(KeyCode::Char('h'));
    assert!(viewer.show_completed);

    // Re-render to update visible entries with completed tasks included
    viewer.render(area, &styles, &mut buf);
    assert_eq!(viewer.last_visible_raw_indices.len(), 30);

    // Arrow down through all 30 items
    for _ in 0..29 {
        viewer.handle_key(KeyCode::Down);
        viewer.render(area, &styles, &mut buf);
    }
    assert_eq!(viewer.selected_line, 29);
    assert!(
        viewer.scroll_offset > 0,
        "View window must scroll down with selection when show done is enabled, scroll_offset was {}",
        viewer.scroll_offset
    );

    // Arrow up back to the first item
    for _ in 0..29 {
        viewer.handle_key(KeyCode::Up);
        viewer.render(area, &styles, &mut buf);
    }
    assert_eq!(viewer.selected_line, 0);
    assert_eq!(
        viewer.scroll_offset, 0,
        "View window must scroll back up to 0 when navigating to the top"
    );

    // Test mouse clicking on footer
    let footer_start_y = 24 - 5;
    let click_footer = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 25,
        row: footer_start_y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    viewer.handle_mouse(click_footer, area);

    // Clicking ESC exits
    let close_action = viewer.handle_key(KeyCode::Esc);
    assert_eq!(close_action, NoteViewerAction::Close);
}

#[test]
fn test_stream_output_modal_lifecycle_and_rendering() {
    use bashmenu_tui::{StreamOutputAction, StreamOutputModal};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut modal = StreamOutputModal::new(
        "Stream Output Test".to_string(),
        "echo 'Hello Streaming World'; echo 'Line Two'".to_string(),
        65,
        false,
    );

    assert_eq!(modal.window_percent, 65);
    assert!(!modal.is_running);

    modal.start();
    assert!(modal.is_running);

    // Poll until process completes or timeout
    let start = std::time::Instant::now();
    while modal.is_running && start.elapsed().as_secs() < 3 {
        modal.poll_output();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    assert!(!modal.is_running);
    assert_eq!(modal.exit_code, Some(0));
    assert!(modal
        .lines
        .iter()
        .any(|l| l.contains("Hello Streaming World")));
    assert!(modal.lines.iter().any(|l| l.contains("Line Two")));

    // Rendering test
    let styles = ResolvedThemeStyles::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let s = format!("{:?}", buf);
    assert!(s.contains("Stream Output Test"));
    assert!(s.contains("Hello Streaming World"));
    assert!(s.contains("Close"));

    // Enter closes when finished
    let act = modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(act, StreamOutputAction::Close);
}

#[test]
fn test_input_modal_selection_and_clipboard() {
    bashmenu_tui::clipboard::set_test_mode(true);
    use bashmenu_tui::InputModal;
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut modal = InputModal::new("Test Input", "Enter value:", "hello world");

    // Initially cursor at end (11)
    assert_eq!(modal.cursor_pos, 11);
    assert_eq!(modal.selection_range(), None);

    // Shift+Left 5 times selects "world"
    for _ in 0..5 {
        modal.handle_key_with_modifiers(KeyCode::Left, KeyModifiers::SHIFT);
    }
    assert_eq!(modal.cursor_pos, 6);
    assert_eq!(modal.selection_range(), Some((6, 11)));

    // Ctrl+C copies "world"
    modal.handle_key_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("world")
    );

    // Typing 'earth' overwrites selection
    for ch in "earth".chars() {
        modal.handle_key_with_modifiers(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    assert_eq!(modal.value, "hello earth");
    assert_eq!(modal.selection_range(), None);

    // Ctrl+A selects all
    modal.handle_key_with_modifiers(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(modal.selection_range(), Some((0, 11)));

    // Ctrl+X cuts all
    modal.handle_key_with_modifiers(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(modal.value, "");
    assert_eq!(modal.cursor_pos, 0);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("hello earth")
    );

    // Ctrl+V pastes "hello earth"
    modal.handle_key_with_modifiers(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(modal.value, "hello earth");
    assert_eq!(modal.cursor_pos, 11);

    // Home clears selection and jumps to 0
    modal.handle_key_with_modifiers(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(modal.cursor_pos, 0);

    // Shift+End selects to end
    modal.handle_key_with_modifiers(KeyCode::End, KeyModifiers::SHIFT);
    assert_eq!(modal.selection_range(), Some((0, 11)));

    // Backspace removes selection
    modal.handle_key_with_modifiers(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(modal.value, "");
    assert_eq!(modal.cursor_pos, 0);
}

#[test]
fn test_note_editor_selection_and_clipboard() {
    bashmenu_tui::clipboard::set_test_mode(true);
    use bashmenu_tui::NoteEditorModal;
    use crossterm::event::{KeyCode, KeyModifiers};

    let text = "Line 1: Alpha\nLine 2: Beta\nLine 3: Gamma";
    let mut editor = NoteEditorModal::new("Editor Test", "/tmp/test.md", text);

    // Move to 0,0
    editor.cursor_row = 0;
    editor.cursor_col = 0;

    // Shift+Down selects line 0 and first character of line 1
    editor.handle_key(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(editor.cursor_row, 1);
    assert_eq!(editor.cursor_col, 0);
    assert_eq!(editor.selection_range(), Some(((0, 0), (1, 0))));

    // Ctrl+C copies selected text
    editor.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("Line 1: Alpha\n")
    );

    // Ctrl+A selects all
    editor.handle_key(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(
        editor.selection_range(),
        Some(((0, 0), (2, "Line 3: Gamma".len())))
    );

    // Ctrl+X cuts all
    editor.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(editor.lines, vec![""]);
    assert_eq!(editor.cursor_row, 0);
    assert_eq!(editor.cursor_col, 0);

    // Paste back via Ctrl+V
    editor.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(
        editor.lines,
        vec!["Line 1: Alpha", "Line 2: Beta", "Line 3: Gamma"]
    );
}

#[test]
fn test_input_modal_masked_rendering() {
    use bashmenu_tui::InputModal;

    let modal = InputModal::new("Password Input", "Enter secret:", "secret123").with_masked(true);
    assert!(modal.masked);
    assert_eq!(modal.value, "secret123");

    let area = Rect::new(0, 0, 80, 24);
    let styles = ResolvedThemeStyles::default();
    let mut buf = Buffer::empty(area);
    modal.render(area, &styles, &mut buf);

    let mut rendered_text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            rendered_text.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(
        !rendered_text.contains("secret123"),
        "Plaintext value must not appear in rendered buffer when masked"
    );
    assert!(
        rendered_text.contains("*********"),
        "Asterisks matching input length must appear in rendered buffer when masked"
    );
}

#[test]
fn test_theme_picker_alphabetical_order() {
    use bashmenu_tui::ThemePickerModal;

    let unsorted = vec![
        "synthwave".to_string(),
        "Test".to_string(),
        "alaska-mountain".to_string(),
        "dracula".to_string(),
        "Cyberpunk".to_string(),
        "nord".to_string(),
        "tangyle".to_string(),
        "tokyo_night".to_string(),
    ];
    let picker = ThemePickerModal::new(unsorted, "dracula");
    assert_eq!(
        picker.themes,
        vec![
            "alaska-mountain",
            "Cyberpunk",
            "dracula",
            "nord",
            "synthwave",
            "tangyle",
            "Test",
            "tokyo_night",
        ]
    );
    assert_eq!(picker.selected_idx, 2);
}
