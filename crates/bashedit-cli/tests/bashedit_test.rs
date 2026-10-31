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
