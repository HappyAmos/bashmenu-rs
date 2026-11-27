//! Standalone CLI executable for configuring BashMenu custom keyboard shortcuts (`shortcuts.yml`).

use anyhow::Result;
use bashmenu_core::resolve_shortcuts_path;
use bashmenu_tui::install_panic_hook;
use clap::Parser;
use menuedit_cli::run_shortcut_editor;
use std::path::PathBuf;

/// Command-line argument configuration for the standalone keyboard shortcuts editor (`shortcutedit`).
#[derive(Parser, Debug)]
#[command(
    name = "shortcutedit",
    author,
    version,
    about = "Interactive visual keyboard shortcuts editor for shortcuts.yml"
)]
struct Cli {
    /// Path to shortcuts.yml configuration file
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Load and save shortcuts.yml in the current working directory
    #[arg(long)]
    local: bool,

    /// Override interface display locale
    #[arg(short = 'L', long = "locale")]
    locale: Option<String>,
}

/// Builds the localized `clap::Command` for `shortcutedit`.
fn build_cli_command() -> clap::Command {
    use bashmenu_core::t;
    use clap::CommandFactory;
    Cli::command()
        .about(t!("cli.shortcutedit.about"))
        .help_template(t!("cli.help_template"))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .mut_arg("file", |a| a.help(t!("cli.shortcutedit.file")))
        .mut_arg("local", |a| a.help(t!("cli.shortcutedit.local")))
        .mut_arg("locale", |a| a.help(t!("cli.shortcutedit.locale")))
        .arg(
            clap::Arg::new("help")
                .short('h')
                .long("help")
                .action(clap::ArgAction::Help)
                .help(t!("cli.common.help")),
        )
        .arg(
            clap::Arg::new("version")
                .short('V')
                .long("version")
                .action(clap::ArgAction::Version)
                .help(t!("cli.common.version")),
        )
}

/// CLI entry point for the standalone keyboard shortcuts editor (`shortcutedit`).
fn main() -> Result<()> {
    install_panic_hook();
    bashmenu_core::preinit_locale_from_args(false);
    let matches = build_cli_command().get_matches();
    let cli =
        <Cli as clap::FromArgMatches>::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    let cfg_path = if cli.local {
        PathBuf::from("./bashmenu.yml")
    } else {
        bashmenu_core::resolve_app_path("bashmenu.yml")
    };
    let cfg = bashmenu_core::load_config(&cfg_path).ok();

    bashmenu_core::init_locale_from_env(
        cli.locale.as_deref(),
        cfg.as_ref().and_then(|c| c.settings.locale.as_deref()),
    );

    let shortcuts_path = if let Some(ref f) = cli.file {
        if cli.local && !f.is_absolute() {
            PathBuf::from(".").join(f)
        } else if f.exists() || f.is_absolute() {
            f.clone()
        } else {
            bashmenu_core::resolve_app_path(f)
        }
    } else {
        resolve_shortcuts_path(cli.local)
    };

    run_shortcut_editor(shortcuts_path)
}
