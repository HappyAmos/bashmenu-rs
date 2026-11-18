use std::process::Command;

#[test]
fn test_ymlcheck_help_contains_locale_flag() {
    let bin = env!("CARGO_BIN_EXE_ymlcheck");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("ymlcheck --help must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("-l, --locale") || stdout.contains("--locale"),
        "Help must document -l / --locale flag: {}",
        stdout
    );
}

#[test]
fn test_ymlcheck_locale_validation_success() {
    let bin = env!("CARGO_BIN_EXE_ymlcheck");
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let en_path = repo_root.join("locales/en_us.yml");
    let es_path = repo_root.join("locales/es_es.yml");

    let output = Command::new(bin)
        .arg("-l")
        .arg(&en_path)
        .arg(&es_path)
        .output()
        .expect("ymlcheck -l must run");
    assert!(
        output.status.success(),
        "Both en_us.yml and es_es.yml must pass validation. Output: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("100% key parity"));
}

#[test]
fn test_ymlcheck_locale_validation_missing_key() {
    let bin = env!("CARGO_BIN_EXE_ymlcheck");
    let tmp_dir = std::env::temp_dir().join(format!("ymlcheck_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let incomplete_locale = tmp_dir.join("incomplete.yml");
    std::fs::write(&incomplete_locale, "common:\n  buttons:\n    ok: 'OK'\n")
        .expect("must write incomplete locale");

    let output = Command::new(bin)
        .arg("-l")
        .arg(&incomplete_locale)
        .output()
        .expect("ymlcheck -l must run");
    assert!(
        !output.status.success(),
        "Incomplete locale must fail validation"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("missing"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
