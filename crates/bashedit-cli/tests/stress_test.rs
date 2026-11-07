//! Stress tests for `bashedit` editor buffer, large files, and rapid undo/redo.

use bashedit_cli::EditorBuffer;

#[test]
fn test_stress_bashedit_buffer_large_file_and_navigation() {
    let mut buf = EditorBuffer::new_empty();

    // Populate with 10,000 lines
    buf.lines.clear();
    for i in 0..10_000 {
        buf.lines.push(format!(
            "Line {:05}: The quick brown fox jumps over the lazy dog.",
            i
        ));
    }
    assert_eq!(buf.lines.len(), 10_000);

    // Stress navigation across 10,000 lines
    buf.cursor_row = 0;
    buf.cursor_col = 0;

    for _ in 0..500 {
        buf.move_paragraph_down();
    }
    for _ in 0..500 {
        buf.move_paragraph_up();
    }
    assert_eq!(buf.cursor_row, 0);

    // Stress search across 10,000 lines
    assert!(buf.search("Line 09999"));
    assert_eq!(buf.cursor_row, 9999);

    assert!(buf.search("Line 00050"));
    assert_eq!(buf.cursor_row, 50);

    assert!(!buf.search("Nonexistent String"));
}

#[test]
fn test_stress_bashedit_rapid_undo_redo_and_capacity_bound() {
    let mut buf = EditorBuffer::new_empty();

    // Perform 80 sequential character insertions
    for i in 0..80 {
        buf.insert_char((b'a' + (i % 26) as u8) as char);
    }
    assert_eq!(buf.lines[0].len(), 80);

    // Undo 40 times
    for _ in 0..40 {
        assert!(buf.undo());
    }
    assert_eq!(buf.lines[0].len(), 40);

    // Redo 30 times
    for _ in 0..30 {
        assert!(buf.redo());
    }
    assert_eq!(buf.lines[0].len(), 70);

    // Now test 150 insertions exceeding the 100-snapshot bounded capacity
    let mut buf_bounded = EditorBuffer::new_empty();
    for _ in 0..150 {
        buf_bounded.insert_char('x');
    }
    assert_eq!(buf_bounded.lines[0].len(), 150);

    // Exactly 100 undos must succeed
    for _ in 0..100 {
        assert!(buf_bounded.undo());
    }
    // The 101st undo must gracefully return false without crashing
    assert!(!buf_bounded.undo());
}
