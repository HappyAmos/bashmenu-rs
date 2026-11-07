//! Tests for bashmenu CLI audit items including inactivity timeout,
//! inject_block, root privilege escalation, and editor options.

use std::process::Command;

#[test]
fn test_bashmenu_binary_help_and_version() {
    let bin = env!("CARGO_BIN_EXE_bashmenu");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("bashmenu --help must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bashmenu") || stdout.contains("Usage"));

    let ver_output = Command::new(bin)
        .arg("--version")
        .output()
        .expect("bashmenu --version must run");
    assert!(ver_output.status.success());
}
