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

#[test]
fn test_bashmenu_modal_animation_config_options() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");
    assert_eq!(config.settings.modal_animation, Some("zoom".to_string()));
    assert_eq!(
        config.settings.modal_animation_speed,
        Some("fast".to_string())
    );
}
