//! Standalone CLI executable for configuring BashMenu custom keyboard shortcuts (`shortcuts.yml`).

use anyhow::Result;
use bashmenu_core::resolve_shortcuts_path;
use bashmenu_tui::install_panic_hook;
use clap::Parser;
use menuedit_cli::run_shortcut_editor;
use std::path::PathBuf;

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
}

fn main() -> Result<()> {
    install_panic_hook();
    let cli = Cli::parse();

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
