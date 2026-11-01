//! Integration tests for BashEdit text buffer, swap file lifecycle, and recovery invariants.

use bashedit_cli::{
    compute_file_sha256, get_swap_path, is_pid_running, read_swap_file, remove_swap_file,
    write_swap_file, EditorBuffer, SWAP_MAGIC,
};
use std::fs;
use std::path::PathBuf;

#[test]
fn test_compute_file_sha256() {
    assert_eq!(
        compute_file_sha256(&PathBuf::from("/nonexistent/file.txt")),
        ""
    );

    let tmp_dir = std::env::temp_dir().join(format!("bashedit_sha_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let sample = tmp_dir.join("sample.txt");
    fs::write(&sample, "Hello World\n").unwrap();

    let sha = compute_file_sha256(&sample);
    assert_eq!(sha.len(), 64);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_get_swap_path_colocated() {
    let target = PathBuf::from("/tmp/my_project/code.sh");
    let swp = get_swap_path(&target);
    assert_eq!(swp, PathBuf::from("/tmp/my_project/.code.sh.swp"));
}

#[test]
fn test_is_pid_running() {
    assert!(!is_pid_running(0));
    assert!(is_pid_running(std::process::id()));
    assert!(!is_pid_running(99999999));
}

#[test]
fn test_write_and_read_swap_file() {
    let tmp_dir = std::env::temp_dir().join(format!("bashedit_swp_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let target = tmp_dir.join("test.txt");
    fs::write(&target, "Original Line 1\nOriginal Line 2\n").unwrap();

    let swp_path = get_swap_path(&target);
    let lines = vec![
        "Recovered Line A".to_string(),
        "Recovered Line B".to_string(),
    ];

    let ok = write_swap_file(&swp_path, &target, &lines, 12345.0, "dummy_sha");
    assert!(ok);
    assert!(swp_path.exists());

    let (header, read_lines) = read_swap_file(&swp_path).expect("Must read swap file");
    assert_eq!(header.magic, SWAP_MAGIC);
    assert_eq!(header.pid, std::process::id());
    assert_eq!(header.line_count, 2);
    assert_eq!(read_lines, lines);

    // Remove swap
    assert!(remove_swap_file(&swp_path));
    assert!(!swp_path.exists());
    assert!(!remove_swap_file(&swp_path));

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_editor_buffer_typing_and_save() {
    let tmp_dir = std::env::temp_dir().join(format!("bashedit_buf_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let target = tmp_dir.join("buffer_test.txt");

    let mut buf = EditorBuffer::open(&target).unwrap();
    assert_eq!(buf.lines.len(), 1);
    assert_eq!(buf.lines[0], "");

    // Type "echo hello"
    for ch in "echo hello".chars() {
        buf.insert_char(ch);
    }
    assert_eq!(buf.lines[0], "echo hello");
    assert!(buf.is_modified);

    // Insert newline and type "exit 0"
    buf.insert_newline();
    for ch in "exit 0".chars() {
        buf.insert_char(ch);
    }
    assert_eq!(buf.lines.len(), 2);
    assert_eq!(buf.lines[1], "exit 0");

    // Save to disk
    buf.save_to_disk().unwrap();
    assert!(!buf.is_modified);

    // Verify content on disk
    let disk_content = fs::read_to_string(&target).unwrap();
    assert_eq!(disk_content, "echo hello\nexit 0");

    buf.cleanup_swap();
    assert!(!buf.swap_path.exists());

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_read_only_mode_prevents_saving() {
    let tmp_dir = std::env::temp_dir().join(format!("bashedit_ro_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let target = tmp_dir.join("ro_test.txt");
    fs::write(&target, "Read only content\n").unwrap();

    let mut buf = EditorBuffer::open(&target).unwrap();
    buf.is_read_only = true;

    // Typing in read only mode has no effect
    buf.insert_char('x');
    assert_eq!(buf.lines[0], "Read only content");

    // Saving in read only mode returns Err
    let save_res = buf.save_to_disk();
    assert!(save_res.is_err());

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_editor_buffer_empty_and_save_as() {
    let mut buf = EditorBuffer::new_empty();
    assert!(buf.file_path.is_none());
    assert_eq!(buf.lines.len(), 1);
    assert_eq!(buf.lines[0], "");

    // Saving without path fails
    assert!(buf.save_to_disk().is_err());

    // Type text
    for ch in "hello empty buffer".chars() {
        buf.insert_char(ch);
    }
    assert!(buf.is_modified);

    let tmp_dir = std::env::temp_dir().join(format!("bashedit_empty_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let target = tmp_dir.join("save_as.txt");

    buf.set_file_path(target.clone());
    assert!(buf.file_path.is_some());
    assert!(buf.save_to_disk().is_ok());

    let disk_content = fs::read_to_string(&target).unwrap();
    assert_eq!(disk_content, "hello empty buffer");

    buf.cleanup_swap();
    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_editor_buffer_word_navigation() {
    let mut buf = EditorBuffer::new_empty();
    buf.lines = vec![
        "The quick brown fox jumps.".to_string(),
        "Second line here.".to_string(),
    ];

    // Initial position: row 0, col 0
    assert_eq!(buf.cursor_row, 0);
    assert_eq!(buf.cursor_col, 0);

    // Move word right -> "quick" (index 4)
    buf.move_word_right();
    assert_eq!(buf.cursor_col, 4);

    // Move word right -> "brown" (index 10)
    buf.move_word_right();
    assert_eq!(buf.cursor_col, 10);

    // Move word left -> "quick" (index 4)
    buf.move_word_left();
    assert_eq!(buf.cursor_col, 4);

    // Move word left -> "The" (index 0)
    buf.move_word_left();
    assert_eq!(buf.cursor_col, 0);
}

#[test]
fn test_editor_buffer_tilde_expansion() {
    let home = std::env::var("HOME").unwrap();
    let buf = EditorBuffer::open("~/.bashrc").expect("open ~/.bashrc");
    assert!(buf.file_path.is_some());
    let path = buf.file_path.unwrap();
    assert_eq!(path, PathBuf::from(&home).join(".bashrc"));
}

#[test]
fn test_editor_buffer_undo_redo() {
    let mut buf = EditorBuffer::new_empty();
    buf.insert_char('a');
    buf.insert_char('b');
    buf.insert_char('c');
    assert_eq!(buf.lines[0], "abc");

    assert!(buf.undo());
    assert_eq!(buf.lines[0], "ab");

    assert!(buf.undo());
    assert_eq!(buf.lines[0], "a");

    assert!(buf.redo());
    assert_eq!(buf.lines[0], "ab");

    assert!(buf.redo());
    assert_eq!(buf.lines[0], "abc");
}

#[test]
fn test_editor_buffer_cut_copy_paste() {
    let mut buf = EditorBuffer::new_empty();
    for ch in "hello world".chars() {
        buf.insert_char(ch);
    }
    assert_eq!(buf.lines[0], "hello world");

    let mut cutbuffer = Vec::new();
    // Mark selection of "world" (col 6..11)
    buf.cursor_col = 6;
    buf.toggle_mark();
    buf.cursor_col = 11;
    assert!(buf.get_selection_range().is_some());

    // Cut selection
    assert!(buf.cut(&mut cutbuffer));
    assert_eq!(cutbuffer, vec!["world"]);
    assert_eq!(buf.lines[0], "hello ");

    // Move to beginning and paste
    buf.cursor_col = 0;
    assert!(buf.paste(&cutbuffer));
    assert_eq!(buf.lines[0], "worldhello ");
}

#[test]
fn test_editor_buffer_search() {
    let mut buf = EditorBuffer::new_empty();
    buf.lines = vec![
        "First line".to_string(),
        "Second target line".to_string(),
        "Third line".to_string(),
    ];
    buf.cursor_row = 0;
    buf.cursor_col = 0;

    assert!(buf.search("target"));
    assert_eq!(buf.cursor_row, 1);
    assert_eq!(buf.cursor_col, 7);

    assert!(!buf.search("nonexistent"));
}

#[test]
fn test_editor_buffer_paragraph_navigation() {
    let mut buf = EditorBuffer::new_empty();
    buf.lines = vec![
        "Para 1 line 1".to_string(),
        "Para 1 line 2".to_string(),
        "".to_string(),
        "Para 2 line 1".to_string(),
    ];
    buf.cursor_row = 0;
    buf.cursor_col = 0;

    buf.move_paragraph_down();
    assert_eq!(buf.cursor_row, 3);

    buf.move_paragraph_up();
    assert_eq!(buf.cursor_row, 0);
}

#[test]
fn test_editor_legend_row_spread() {
    let items_r1 = [
        "F1 Help",
        "F2 Whitespace",
        "F3 ASCII",
        "F4 Colors",
        "F5 Macros",
        "F6 MD",
        "F7 Lineno",
        "F8 MD Ref",
    ];
    let items_r2 = [
        "^N Tab",
        "^O Open",
        "^S Save",
        "^A SaveAs",
        "^W Search",
        "^Z Undo",
        "Alt+←/→ Tab",
        "^Q/ESC Exit",
    ];

    let avail_w = 110;
    let (line_r1, ranges_r1) = bashmenu_tui::spread_gutter_line_layout(&items_r1, avail_w);
    let (line_r2, ranges_r2) = bashmenu_tui::spread_gutter_line_layout(&items_r2, avail_w);

    assert_eq!(bashmenu_tui::visible_text_len(&line_r1), avail_w);
    assert_eq!(bashmenu_tui::visible_text_len(&line_r2), avail_w);

    assert_eq!(ranges_r1.len(), 8);
    assert_eq!(ranges_r2.len(), 8);

    assert_eq!(ranges_r1[0].0, 0);
    assert_eq!(ranges_r1[7].1, avail_w);
    assert_eq!(ranges_r2[0].0, 0);
    assert_eq!(ranges_r2[7].1, avail_w);
}
