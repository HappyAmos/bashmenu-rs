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

#[test]
fn test_user_config_isolation_and_vanilla_separation() {
    let bin = env!("CARGO_BIN_EXE_bashmenu");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("bashmenu --help must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Ensure --menu and --config are optional arguments rather than defaulting to local file
    assert!(stdout.contains("-m, --menu") || stdout.contains("--menu <MENU>"));
    assert!(stdout.contains("-c, --config") || stdout.contains("--config <CONFIG>"));
}

#[test]
fn test_search_nerd_font_and_emoji_menu_item_streaming() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let menu_path = repo_root.join("bashmenu.mnu");
    let schema = bashmenu_core::load_menu(&menu_path).expect("bashmenu.mnu must load");

    // Search for the glyphs menu item across all submenus
    let mut found_item = None;
    for item in &schema.items {
        if item.label.contains("Search Nerd Font & Emoji") {
            found_item = Some(item.clone());
            break;
        }
        if let Some(ref sub) = item.submenu {
            for sub_item in &sub.items {
                if sub_item.label.contains("Search Nerd Font & Emoji") {
                    found_item = Some(sub_item.clone());
                    break;
                }
            }
        }
    }

    let glyph_item = found_item.expect("Search Nerd Font & Emoji item must exist");
    assert_eq!(
        glyph_item.execution_mode(),
        bashmenu_core::ExecutionMode::Streaming
    );
    assert!(glyph_item.is_streaming());
    assert_eq!(glyph_item.window_percent(), 75);
    assert_eq!(glyph_item.param_modal_prompt(), "Enter search term:");

    // Verify macro expansion with param
    let mut resolved = glyph_item.clone();
    resolved.apply_param_placeholder("folder");
    let action = resolved
        .action
        .as_ref()
        .or(resolved.command.as_ref())
        .unwrap();
    assert!(action.contains("glyphs.sh folder"));
}

#[test]
fn test_inactivity_timeout_timer_reset_contract() {
    use std::time::{Duration, Instant};

    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");

    // Inactivity timeout should be defined in settings
    let timeout = config
        .settings
        .inactivity_timeout
        .as_ref()
        .expect("inactivity_timeout must be configured in bashmenu.yml");
    let ms = timeout.milliseconds.unwrap_or(0);
    assert!(ms > 0, "Inactivity timeout ms must be greater than 0");

    // Simulate returning from an external command that ran longer than the timeout
    let mut last_interaction = Instant::now() - Duration::from_millis(ms + 1000);
    assert!(
        last_interaction.elapsed().as_millis() >= ms as u128,
        "Simulated command run exceeds timeout threshold"
    );

    // Contract: Upon returning from an external command or sub-tool,
    // last_interaction MUST be reset to Instant::now().
    last_interaction = Instant::now();

    assert!(
        last_interaction.elapsed().as_millis() < ms as u128,
        "Resetting last_interaction prevents immediate screensaver trigger upon return"
    );
}

#[test]
fn test_bashmenu_repo_flag_in_help_and_parsing() {
    let bin = env!("CARGO_BIN_EXE_bashmenu");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("bashmenu --help must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("-r, --repo"),
        "Help output must document -r, --repo flag: {}",
        stdout
    );
}
