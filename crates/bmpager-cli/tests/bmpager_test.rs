use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_bmpager_help_and_version() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "--help",
        ])
        .output()
        .expect("failed to execute bmpager --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: command | bmpager [options]"));
    assert!(stdout.contains("-m, --markdown"));
    assert!(stdout.contains("+N"));
    assert!(stdout.contains("+/PATTERN"));

    let ver_output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "--version",
        ])
        .output()
        .expect("failed to execute bmpager --version");
    assert!(ver_output.status.success());
    let ver_stdout = String::from_utf8_lossy(&ver_output.stdout);
    assert!(ver_stdout.contains("bmpager"));
}

#[test]
fn test_bmpager_passthrough_non_tty() {
    let mut child = Command::new("cargo")
        .args(["run", "-p", "bmpager-cli", "--bin", "bmpager"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let test_input = "Line 1: Hello World\nLine 2: Second Line\nLine 3: Final Line\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(test_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("Line 1: Hello World"));
    assert!(out_str.contains("Line 2: Second Line"));
    assert!(out_str.contains("Line 3: Final Line"));
}

#[test]
fn test_bmpager_markdown_flag_passthrough() {
    let mut child = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "--markdown",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let md_input =
        "# Document Title\n\nWelcome to **Markdown** in `bmpager`.\n\n* Item 1\n* Item 2\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(md_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    // H1 heavy box panel borders
    assert!(out_str.contains('┏') && out_str.contains('┗'));
    assert!(out_str.contains("Document Title"));
    // Unordered bullet
    assert!(out_str.contains('•'));
    assert!(out_str.contains("Item 1"));
}

#[test]
fn test_bmpager_table_rendering_and_wrapping() {
    let mut child = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "--markdown",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let table_input = "| Script | Description |\n| :--- | :--- |\n| `scripts/ascii.sh` | Standard and Extended ASCII viewing utility with comprehensive character maps. |\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(table_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains('┌') && out_str.contains('┐'));
    assert!(out_str.contains('├') && out_str.contains('┤'));
    assert!(out_str.contains('└') && out_str.contains('┘'));
    assert!(out_str.contains("scripts/ascii.sh"));
    assert!(out_str.contains("Standard and Extended"));
}

#[test]
fn test_bmpager_file_argument() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "Cargo.toml",
        ])
        .stdout(Stdio::piped())
        .output()
        .expect("failed to run bmpager with Cargo.toml");

    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("[package]") || out_str.contains("[workspace]"));
}

#[test]
fn test_bmpager_link_extraction_and_anchor_resolution() {
    let mut child = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bmpager-cli",
            "--bin",
            "bmpager",
            "--",
            "--markdown",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let doc = "\
- [3.1 Main Menu Engine (`bashmenu.py`)](#31-main-menu-engine-bashmenupy)

### 3.1 Main Menu Engine (`bashmenu.py`)

Here is section 3.1 content.
";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(doc.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("3.1 Main Menu Engine"));
}
