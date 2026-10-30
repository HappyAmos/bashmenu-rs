//! Main executable entrypoint for BashMenu in Rust.

use anyhow::Result;
use bashmenu_core::{
    interpolate_placeholders, load_config, load_menu, load_themes, BashMenuConfig, MacroContext,
    MenuSchema, PluginEngine,
};
use bashmenu_tui::{install_panic_hook, MenuAction, MenuView, ResolvedThemeStyles, TerminalGuard};
use clap::Parser;
use crossterm::event::{self, Event};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    name = "bashmenu",
    author,
    version,
    about = "Modern, high-performance terminal menu system in Rust"
)]
struct Cli {
    /// Path to menu definition file
    #[arg(short, long, default_value = "bashmenu.mnu")]
    menu: PathBuf,

    /// Path to configuration file
    #[arg(short, long, default_value = "bashmenu.yml")]
    config: PathBuf,

    /// Override active theme
    #[arg(short, long)]
    theme: Option<String>,
}

fn resolve_file_path(target: &Path) -> PathBuf {
    if target.exists() {
        return target.to_path_buf();
    }
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        let candidate = PathBuf::from(dir).join(target);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let user_cfg = PathBuf::from(home).join(".config/bashmenu").join(target);
        if user_cfg.exists() {
            return user_cfg;
        }
    }
    // Check fixtures directory fallback
    let fixture_path = PathBuf::from("tests/fixtures").join(target);
    if fixture_path.exists() {
        return fixture_path;
    }
    target.to_path_buf()
}

fn main() -> Result<()> {
    // 1. Terminal Panic Recovery Hook Invariant
    install_panic_hook();

    let cli = Cli::parse();

    // 2. Load Configuration and Menus
    let config_path = resolve_file_path(&cli.config);
    let mut config = if config_path.exists() {
        load_config(&config_path).unwrap_or_default()
    } else {
        BashMenuConfig::default()
    };

    if let Some(t) = cli.theme {
        config.theme = t;
    }

    let menu_path = resolve_file_path(&cli.menu);
    let menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        MenuSchema::default()
    };

    // 3. Load Themes
    let themes_path = resolve_file_path(Path::new("bashmenu.themes"));
    let themes = if themes_path.exists() {
        load_themes(&themes_path).unwrap_or_default()
    } else {
        Default::default()
    };

    let use_nerd = config.settings.use_nerd_fonts.unwrap_or(false);
    let active_theme = themes.get(&config.theme);
    let styles = if let Some(t) = active_theme {
        ResolvedThemeStyles::from_theme(t, use_nerd)
    } else {
        ResolvedThemeStyles::default()
    };

    // 4. Macro Context & Status Gutter
    let mut macro_ctx = MacroContext::new(&config);
    let status_gutter = config
        .settings
        .status_gutter
        .as_deref()
        .map(|tmpl| interpolate_placeholders(tmpl, &macro_ctx))
        .unwrap_or_default();

    // 5. Initialize Plugin Engine
    let (plugin_tx, plugin_rx) = mpsc::channel();
    let plugin_engine = PluginEngine::new().with_sender(plugin_tx);
    plugin_engine.trigger_refresh(&config, false);

    // 6. Initialize Terminal Guard
    let mut guard = TerminalGuard::init()?;
    let mut menu_view = MenuView::new(menu, styles, use_nerd, status_gutter);
    let mut last_plugin_check = Instant::now();

    // 7. Interactive Event Loop
    loop {
        let size = guard.terminal_mut().size()?;
        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
        macro_ctx.window_width = size.width as usize;
        macro_ctx.window_height = size.height as usize;
        macro_ctx.screen_width = size.width as usize;

        // Drain plugin background updates
        let mut plugins_updated = false;
        while plugin_rx.try_recv().is_ok() {
            plugins_updated = true;
        }
        if plugins_updated {
            let avail_w = size.width.saturating_sub(4) as usize;
            let rows = plugin_engine.format_display_rows(&config, avail_w, '─');
            menu_view.set_plugin_rows(rows);
        }

        // Periodic check to trigger expired plugins
        if last_plugin_check.elapsed() >= Duration::from_secs(1) {
            plugin_engine.trigger_refresh(&config, false);
            last_plugin_check = Instant::now();
        }

        guard.terminal_mut().draw(|f| {
            menu_view.render(f.area(), f.buffer_mut());
        })?;

        if event::poll(Duration::from_millis(50))? {
            let action = match event::read()? {
                Event::Key(key) => menu_view.handle_key(key.code),
                Event::Mouse(mouse) => menu_view.handle_mouse(mouse, area),
                Event::Resize(_, _) => MenuAction::None,
                _ => MenuAction::None,
            };

            match action {
                MenuAction::Execute(item) => {
                    let cmd_to_run = item.command.or(item.action).or(item.script);
                    if let Some(raw_cmd) = cmd_to_run {
                        let final_cmd = interpolate_placeholders(&raw_cmd, &macro_ctx);
                        let _ = guard.suspend(|| {
                            let _ = Command::new("/bin/bash").arg("-c").arg(&final_cmd).status();
                            Ok(())
                        });
                    }
                }
                MenuAction::Back | MenuAction::Exit => {
                    break;
                }
                MenuAction::Reload => {
                    // Hot reload menu and configs
                }
                _ => {}
            }
        }
    }

    Ok(())
}
