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
fn test_repo_mode_notebook_isolation() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let notes_path = repo_root.join("defaults/notes.yml");
    assert!(
        notes_path.is_file(),
        "notes.yml must exist in repo defaults/"
    );
    let nb = bashmenu_core::load_notebook(&notes_path).expect("notes.yml must load");
    assert_eq!(nb.notebook.title, "Notes");
    let notes = bashmenu_core::get_notes(&nb);
    assert!(
        !notes.is_empty(),
        "Sample notes in repository must not be empty"
    );

    // Verify resolve_notebook_path in repo mode
    let cfg =
        bashmenu_core::load_config(repo_root.join("defaults/bashmenu.yml")).unwrap_or_default();
    let ctx = bashmenu_core::MacroContext::new(&cfg)
        .with_bashmenu_dir(repo_root.clone())
        .with_repo_mode(true);
    let resolved = bashmenu_core::resolve_notebook_path(Some("{notebooks_dir}/notes.yml"), &ctx);
    assert_eq!(
        resolved.canonicalize().unwrap_or(resolved.clone()),
        notes_path.canonicalize().unwrap_or(notes_path.clone()),
        "In repo mode, {{notebooks_dir}}/notes.yml must resolve to repo defaults/notes.yml"
    );
}

#[test]
fn test_bashmenu_modal_animation_config_options() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("defaults/bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");
    assert_eq!(config.settings.modal_animation, Some("zoom".to_string()));
    assert_eq!(
        config.settings.modal_animation_speed,
        Some("fast".to_string())
    );
}

#[test]
fn test_bashmenu_marquee_speed_config_options() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("defaults/bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");
    assert_eq!(config.settings.marquee_speed, Some("normal".to_string()));
    assert_eq!(
        config.settings.marquee_speed_duration(),
        std::time::Duration::from_millis(250)
    );
}

#[test]
fn test_bashmenu_locale_config_options() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("defaults/bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");
    assert_eq!(config.settings.locale, Some("auto".to_string()));

    let mnu_path = repo_root.join("defaults/bashmenu.mnu");
    let content = std::fs::read_to_string(&mnu_path).expect("bashmenu.mnu must exist");
    assert!(
        content.contains("Language / Locale [{settings.locale}]"),
        "bashmenu.mnu must contain Language / Locale radio setting"
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
fn test_bashmenu_locale_cli_flag_in_help() {
    let bin = env!("CARGO_BIN_EXE_bashmenu");
    let output = Command::new(bin)
        .arg("--help")
        .output()
        .expect("bashmenu --help must run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("-l, --locale") || stdout.contains("--locale <LOCALE>"),
        "Help output must document --locale / -l flag: {}",
        stdout
    );
}

#[test]
fn test_makefile_targets_and_dry_run() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let makefile_path = repo_root.join("Makefile");
    assert!(makefile_path.is_file(), "Makefile must exist at repo root");

    // Run 'make help'
    let output = Command::new("make")
        .current_dir(&repo_root)
        .arg("help")
        .output()
        .expect("make help must execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("make check"));
    assert!(stdout.contains("make verify"));
    assert!(stdout.contains("make locale-check"));
    assert!(stdout.contains("make fmt"));
    assert!(stdout.contains("make clippy"));

    // Dry-run checks for key targets to ensure no makefile syntax errors
    for target in &[
        "check",
        "verify",
        "locale-check",
        "fmt",
        "clippy",
        "shellcheck",
        "build",
    ] {
        let dry_run = Command::new("make")
            .current_dir(&repo_root)
            .arg("-n")
            .arg(target)
            .output()
            .unwrap_or_else(|_| panic!("make -n {} must run", target));
        assert!(
            dry_run.status.success(),
            "make -n {} failed: {}",
            target,
            String::from_utf8_lossy(&dry_run.stderr)
        );
    }
}

#[test]
fn test_search_nerd_font_and_emoji_menu_item_streaming() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let menu_path = repo_root.join("defaults/bashmenu.mnu");
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
fn test_inactivity_timeout_configured_in_default_config() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("defaults/bashmenu.yml");
    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");

    let timeout = config
        .settings
        .inactivity_timeout
        .as_ref()
        .expect("inactivity_timeout must be configured in bashmenu.yml");
    assert!(
        timeout.milliseconds.unwrap_or(0) > 0,
        "Inactivity timeout ms must be greater than 0"
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
    assert!(
        stdout.contains("-i, --image"),
        "Help output must document -i, --image flag: {}",
        stdout
    );
}

#[test]
fn test_env_environment_info_modal_formatting() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let yml_path = repo_root.join("defaults/bashmenu.yml");
    let mnu_path = repo_root.join("defaults/bashmenu.mnu");
    let thm_path = repo_root.join("defaults/bashmenu.themes");
    let shc_path = repo_root.join("defaults/shortcuts.yml");
    let cht_path = repo_root.join("defaults/cheat.yml");

    let config = bashmenu_core::load_config(&yml_path).expect("bashmenu.yml must load");
    let ctx = bashmenu_core::MacroContext::new(&config)
        .with_paths(
            yml_path.clone(),
            mnu_path.clone(),
            thm_path.clone(),
            shc_path.clone(),
            cht_path.clone(),
        )
        .with_color_mode("truecolor")
        .with_tty_mode("pty");

    let (env_title, env_text) = ctx.render_env_about();
    assert_eq!(env_title, "Current Bashmenu Environment");
    assert_eq!(
        bashmenu_core::interpolate_placeholders("{version}", &ctx),
        ctx.version
    );
    // {app.version} was removed to prevent confusion; it does not resolve
    assert_eq!(
        bashmenu_core::interpolate_placeholders("{app.version}", &ctx),
        "{app.version}"
    );
    assert!(!env_text.contains("title:"));
    assert!(env_text.contains("| Information | Value |"));
    assert!(env_text.contains(&format!("| Version | {} |", ctx.version)));
    assert!(env_text.contains("| Color Mode | truecolor |"));
    assert!(env_text.contains("| TTY Mode | pty |"));
    assert!(env_text.contains(&format!(
        "| bashmenu.yml | {} |",
        bashmenu_core::full_file_location(&yml_path)
    )));
    assert!(env_text.contains(&format!(
        "| bashmenu.mnu | {} |",
        bashmenu_core::full_file_location(&mnu_path)
    )));
    assert!(env_text.contains(&format!(
        "| bashmenu.themes | {} |",
        bashmenu_core::full_file_location(&thm_path)
    )));
    assert!(env_text.contains(&format!(
        "| shortcuts.yml | {} |",
        bashmenu_core::full_file_location(&shc_path)
    )));
    assert!(env_text.contains(&format!(
        "| cheat.yml | {} |",
        bashmenu_core::full_file_location(&cht_path)
    )));

    // Verify modal dimensions and wrapped formatting does not split table rows
    let (w, h) = bashmenu_tui::modals::MessageModal::new(env_title, &env_text)
        .dialog_dimensions(ratatui::layout::Rect::new(0, 0, 100, 30));
    assert!(
        w >= 70,
        "Dialog width should accommodate table without line truncation: {}",
        w
    );
    assert!(
        h >= 10,
        "Dialog height should accommodate header + table rows: {}",
        h
    );
}

#[test]
fn test_theme_picker_includes_random_option() {
    use bashmenu_tui::ThemePickerModal;

    let themes = vec!["nord".to_string(), "dracula".to_string()];
    let picker = ThemePickerModal::new(themes.clone(), "nord").with_random_option();
    // [ Random ] must be at index 0 (top of the list) in brackets
    assert_eq!(
        picker.themes[0], "[ Random ]",
        "ThemePickerModal with_random_option must place '[ Random ]' at top of list"
    );
    assert_eq!(picker.themes[1], "dracula");
    assert_eq!(picker.themes[2], "nord");
    assert_eq!(picker.selected_idx, 2); // "nord" selected

    // When initialized with "[ Random ]", selected_idx should be 0
    let picker_random = ThemePickerModal::new(themes, "[ Random ]").with_random_option();
    assert_eq!(picker_random.selected_idx, 0);
}

#[test]
fn test_all_themes_define_default_icon_and_menu_items_have_icons() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let themes_path = repo_root.join("defaults/bashmenu.themes");
    let themes = bashmenu_core::load_themes(&themes_path).expect("bashmenu.themes must load");

    assert!(!themes.is_empty(), "Themes collection must not be empty");
    for (name, theme) in &themes {
        assert_eq!(
            theme.default_icon.as_deref(),
            Some(bashmenu_core::DEFAULT_ITEM_ICON),
            "Theme '{}' must explicitly define default_icon matching DEFAULT_ITEM_ICON",
            name
        );
        assert_eq!(
            theme.resolved_default_icon(),
            bashmenu_core::DEFAULT_ITEM_ICON,
            "Theme '{}' resolved_default_icon must match DEFAULT_ITEM_ICON",
            name
        );
    }

    let menu_path = repo_root.join("defaults/bashmenu.mnu");
    let menu = bashmenu_core::load_menu(&menu_path).expect("bashmenu.mnu must load");
    fn assert_items_have_icons(items: &[bashmenu_core::MenuItem]) {
        for it in items {
            if !it.is_divider() {
                assert!(
                    it.icon.is_some() && !it.icon.as_deref().unwrap().trim().is_empty(),
                    "Menu item '{}' must have an explicitly configured icon",
                    it.label
                );
            }
            if let Some(ref sub) = it.submenu {
                assert_items_have_icons(&sub.items);
            }
        }
    }
    assert_items_have_icons(&menu.items);
}

#[test]
fn test_random_theme_resolution_in_bashmenu() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let themes_path = repo_root.join("defaults/bashmenu.themes");
    let themes = bashmenu_core::load_themes(&themes_path).expect("bashmenu.themes must load");

    let cfg_theme = "[ Random ]";
    assert!(bashmenu_core::is_random_theme_spec(cfg_theme));

    let (active_name, active_theme) = if bashmenu_core::is_random_theme_spec(cfg_theme) {
        if let Some((name, t)) = bashmenu_core::pick_random_theme(&themes) {
            (name.to_string(), Some(t))
        } else {
            (cfg_theme.to_string(), None)
        }
    } else {
        (cfg_theme.to_string(), themes.get(cfg_theme))
    };

    assert_ne!(active_name, "[ Random ]");
    assert_ne!(active_name, "Random");
    assert!(themes.contains_key(&active_name));
    assert!(active_theme.is_some());
}

#[test]
fn test_installer_help_flags() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let install_sh = repo_root.join("install.sh");
    assert!(install_sh.is_file(), "install.sh must exist at repo root");

    let output = Command::new("sh")
        .arg(&install_sh)
        .arg("--help")
        .output()
        .expect("install.sh --help must run");

    assert!(output.status.success(), "install.sh --help must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--merge"));
    assert!(stdout.contains("--update-menu"));

    let script = std::fs::read_to_string(&install_sh).expect("install.sh must be readable");
    assert!(
        !script.contains(
            "if [ ! -f \"$SCRIPT_DIR/bin/$bin\" ] && [ ! -f \"$SCRIPT_DIR/target/release/$bin\" ]; then"
        ),
        "install.sh must not skip cargo build when only stale target/release binaries exist"
    );
}
