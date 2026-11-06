//! Integration tests for `cheat-cli`.

use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_cheat_cli_directory_and_listing() {
    let dir = tempdir().expect("tempdir");
    let cheats_dir = dir.path().join("cheats");
    fs::create_dir_all(&cheats_dir).expect("create cheats dir");

    let config_file = dir.path().join("cheat.yml");
    fs::write(&config_file, format!("dir: {}\n", cheats_dir.display())).expect("write config");

    let cheat_md = cheats_dir.join("test-tool.md");
    fs::write(
        &cheat_md,
        "---\ntitle: Test Tool\ntags: [tool, automation]\n---\n# Test Tool\nUsage guide.",
    )
    .expect("write markdown");

    // Build path to binary
    let bin_path = env!("CARGO_BIN_EXE_cheat");

    // Test -d / --directory
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .arg("-d")
        .output()
        .expect("run -d");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().ends_with("cheats"));

    // Test -l (unfiltered list)
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .arg("-l")
        .output()
        .expect("run -l");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-tool.md"));
    assert!(stdout.contains("[tool, automation]"));

    // Test -l with query matching tag
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .args(["-l", "automation"])
        .output()
        .expect("run -l automation");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-tool.md"));

    // Test -s with query matching full text
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .args(["-s", "guide"])
        .output()
        .expect("run -s guide");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-tool.md"));

    // Test non-existent search exits 1
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .args(["-l", "nonexistent"])
        .output()
        .expect("run -l nonexistent");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No markdown file matched 'nonexistent'"));

    // Test no arguments exits 1 and prints help
    let output = Command::new(bin_path)
        .env("CHEAT_CONFIG", &config_file)
        .output()
        .expect("run with no args");

    assert!(!output.status.success());
}
