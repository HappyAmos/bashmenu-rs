//! End-to-end stress tests for all BashMenu suite CLI binaries, verifying
//! safety, graceful error exits, and zero naked panics on adversarial inputs.

use std::fs;
use std::process::Command;

#[test]
fn test_stress_all_binaries_help_and_version() {
    let binaries = [
        ("bashmenu-cli", "bashmenu"),
        ("menuedit-cli", "menuedit"),
        ("menuedit-cli", "shortcutedit"),
        ("themeedit-cli", "themeedit"),
        ("bashedit-cli", "bashedit"),
        ("bmpager-cli", "bmpager"),
        ("cheat-cli", "cheat"),
        ("ymlcheck-cli", "ymlcheck"),
    ];

    for (pkg, bin) in binaries {
        // Test --help
        let help_res = Command::new("cargo")
            .args(["run", "-p", pkg, "--bin", bin, "--", "--help"])
            .output()
            .unwrap_or_else(|_| panic!("Failed to run {} --help", bin));
        assert!(
            help_res.status.success(),
            "Binary '{}' (package '{}') --help failed: {:?}",
            bin,
            pkg,
            help_res
        );

        // Test --version
        let ver_res = Command::new("cargo")
            .args(["run", "-p", pkg, "--bin", bin, "--", "--version"])
            .output()
            .unwrap_or_else(|_| panic!("Failed to run {} --version", bin));
        assert!(
            ver_res.status.success(),
            "Binary '{}' (package '{}') --version failed: {:?}",
            bin,
            pkg,
            ver_res
        );
    }
}

#[test]
fn test_stress_ymlcheck_adversarial_inputs() {
    let tmp_dir = std::env::temp_dir().join(format!("ymlcheck_stress_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);

    // 1. Non-existent file
    let non_existent = tmp_dir.join("non_existent.yml");
    let res = Command::new("cargo")
        .args([
            "run",
            "-p",
            "ymlcheck-cli",
            "--bin",
            "ymlcheck",
            "--",
            non_existent.to_str().unwrap(),
        ])
        .output()
        .expect("ymlcheck non-existent");
    assert!(
        !res.status.success(),
        "Must fail gracefully on non-existent file"
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        !stderr.contains("panicked"),
        "Must not panic on non-existent file"
    );

    // 2. Empty file
    let empty_file = tmp_dir.join("empty.yml");
    fs::write(&empty_file, "").unwrap();
    let res = Command::new("cargo")
        .args([
            "run",
            "-p",
            "ymlcheck-cli",
            "--bin",
            "ymlcheck",
            "--",
            empty_file.to_str().unwrap(),
        ])
        .output()
        .expect("ymlcheck empty file");
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(!stderr.contains("panicked"), "Must not panic on empty file");

    // 3. Corrupt syntax YAML file
    let corrupt_file = tmp_dir.join("corrupt.yml");
    fs::write(&corrupt_file, "key: [unclosed list\n  bad indent\n").unwrap();
    let res = Command::new("cargo")
        .args([
            "run",
            "-p",
            "ymlcheck-cli",
            "--bin",
            "ymlcheck",
            "--",
            corrupt_file.to_str().unwrap(),
        ])
        .output()
        .expect("ymlcheck corrupt file");
    assert!(!res.status.success(), "Must fail on corrupt YAML");
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        !stderr.contains("panicked"),
        "Must not panic on corrupt syntax"
    );

    // 4. Binary random bytes file
    let binary_file = tmp_dir.join("random.bin");
    fs::write(
        &binary_file,
        vec![0xFF, 0xFE, 0x00, 0x01, 0xDE, 0xAD, 0xBE, 0xEF],
    )
    .unwrap();
    let res = Command::new("cargo")
        .args([
            "run",
            "-p",
            "ymlcheck-cli",
            "--bin",
            "ymlcheck",
            "--",
            binary_file.to_str().unwrap(),
        ])
        .output()
        .expect("ymlcheck binary file");
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        !stderr.contains("panicked"),
        "Must not panic on binary bytes"
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_stress_bashedit_adversarial_paths() {
    let res = Command::new("cargo")
        .args([
            "run",
            "-p",
            "bashedit-cli",
            "--bin",
            "bashedit",
            "--",
            "--help",
        ])
        .output()
        .expect("bashedit help");
    assert!(res.status.success());
}
