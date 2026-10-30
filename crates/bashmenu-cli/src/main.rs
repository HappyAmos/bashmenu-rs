//! Main executable entrypoint for BashMenu in Rust.

use anyhow::Result;
use bashmenu_core::{
    interpolate_placeholders, load_config, load_menu, load_themes, save_config, BashMenuConfig,
    MacroContext, MenuItemType, MenuSchema, PluginEngine,
};
use bashmenu_tui::{
    install_panic_hook, HelpModal, InputModal, MenuAction, MenuView, ResolvedThemeStyles,
    TerminalGuard, ThemePickerModal,
};
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

enum ActiveModal {
    None,
    Help(HelpModal),
    ThemePicker(ThemePickerModal),
    Input(InputModal),
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

fn locate_tool(name: &str) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        let candidate = PathBuf::from(&dir).join("target/release").join(name);
        if candidate.is_file() {
            return candidate;
        }
        let script_cand = PathBuf::from(&dir)
            .join("scripts")
            .join(format!("{}.sh", name));
        if script_cand.is_file() {
            return script_cand;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin").join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(name)
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
    let mut themes = if themes_path.exists() {
        load_themes(&themes_path).unwrap_or_default()
    } else {
        Default::default()
    };

    let use_nerd = config.settings.use_nerd_fonts.unwrap_or(false);
    let active_theme = themes.get(&config.theme);
    let mut styles = if let Some(t) = active_theme {
        ResolvedThemeStyles::from_theme(t, use_nerd)
    } else {
        ResolvedThemeStyles::default()
    };

    // 4. Macro Context & Status Gutter
    let status_gutter = {
        let initial_ctx = MacroContext::new(&config);
        config
            .settings
            .status_gutter
            .as_deref()
            .map(|tmpl| interpolate_placeholders(tmpl, &initial_ctx))
            .unwrap_or_default()
    };

    // 5. Initialize Plugin Engine
    let (plugin_tx, plugin_rx) = mpsc::channel();
    let plugin_engine = PluginEngine::new().with_sender(plugin_tx);
    plugin_engine.trigger_refresh(&config, false);

    // 6. Initialize Terminal Guard
    let mut guard = TerminalGuard::init()?;
    let mut menu_view = MenuView::new(menu, styles.clone(), use_nerd, status_gutter);
    if let Some(show) = config.settings.show_menu_shortcuts {
        menu_view.set_show_shortcuts(show);
    }
    let mut active_modal = ActiveModal::None;
    let mut last_plugin_check = Instant::now();

    // 7. Interactive Event Loop
    loop {
        let size = guard.terminal_mut().size()?;
        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);

        // Drain plugin background updates
        let mut plugins_updated = false;
        while plugin_rx.try_recv().is_ok() {
            plugins_updated = true;
        }
        if plugins_updated {
            let avail_w = size.width.saturating_sub(6) as usize;
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
            match &mut active_modal {
                ActiveModal::None => {}
                ActiveModal::Help(modal) => {
                    modal.render(f.area(), &styles, f.buffer_mut());
                }
                ActiveModal::ThemePicker(modal) => {
                    modal.render(f.area(), &styles, f.buffer_mut());
                }
                ActiveModal::Input(modal) => {
                    modal.render(f.area(), &styles, f.buffer_mut());
                }
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            match &mut active_modal {
                ActiveModal::Help(modal) => {
                    let dismiss = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => false,
                    };
                    if dismiss {
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::ThemePicker(modal) => {
                    let chosen = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_theme) = chosen {
                        if let Some(new_theme) = opt_theme {
                            config.theme = new_theme;
                            let _ = save_config(&config_path, &config);
                            if let Some(t) = themes.get(&config.theme) {
                                styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                menu_view.set_styles(styles.clone());
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::Input(modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_gutter) = opt_val {
                            config.settings.status_gutter = Some(new_gutter.clone());
                            let _ = save_config(&config_path, &config);
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            let gutter_str = interpolate_placeholders(&new_gutter, &ctx);
                            menu_view.set_status_gutter(gutter_str);
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::None => {
                    let action = match ev {
                        Event::Key(key) => menu_view.handle_key(key.code),
                        Event::Mouse(mouse) => menu_view.handle_mouse(mouse, area),
                        Event::Resize(_, _) => MenuAction::None,
                        _ => MenuAction::None,
                    };

                    match action {
                        MenuAction::Help => {
                            let doc_path = resolve_file_path(Path::new("bashmenu.md"));
                            let help_content = std::fs::read_to_string(&doc_path).unwrap_or_else(|_| {
                                "BashMenu Documentation\n\nNavigation:\n  Up/Down, k/j: Navigate items\n  Enter: Select item\n  Esc/q: Back or exit\n\nFunction Keys:\n  F1: Help manual\n  F2: Theme picker\n  F3: Text editor\n  F4: Menu editor\n  F5: Reload config and menus\n  F6: Toggle shortcut numbers\n  F7: Configure status gutter\n  F8: Theme editor".to_string()
                            });
                            active_modal =
                                ActiveModal::Help(HelpModal::new("BashMenu Help", &help_content));
                        }
                        MenuAction::ThemeSelector => {
                            let theme_names: Vec<String> = themes.keys().cloned().collect();
                            active_modal = ActiveModal::ThemePicker(ThemePickerModal::new(
                                theme_names,
                                &config.theme,
                            ));
                        }
                        MenuAction::LaunchEditor => {
                            let editor_bin = locate_tool("bashedit");
                            let _ = guard.suspend(|| {
                                let _ = Command::new(&editor_bin).arg(&config_path).status();
                                Ok(())
                            });
                        }
                        MenuAction::EditMenu => {
                            let menuedit_bin = locate_tool("menuedit");
                            let _ = guard.suspend(|| {
                                let _ = Command::new(&menuedit_bin)
                                    .arg("-f")
                                    .arg(&menu_path)
                                    .status();
                                Ok(())
                            });
                            if let Ok(new_menu) = load_menu(&menu_path) {
                                menu_view.reload_menu(new_menu);
                            }
                        }
                        MenuAction::Reload => {
                            if let Ok(new_config) = load_config(&config_path) {
                                config = new_config;
                            }
                            if let Ok(new_menu) = load_menu(&menu_path) {
                                menu_view.reload_menu(new_menu);
                            }
                            if let Ok(new_themes) = load_themes(&themes_path) {
                                themes = new_themes;
                            }
                            if let Some(t) = themes.get(&config.theme) {
                                styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                menu_view.set_styles(styles.clone());
                            }
                            plugin_engine.trigger_refresh(&config, true);
                        }
                        MenuAction::ToggleShortcuts => {
                            let show = menu_view.toggle_shortcuts();
                            config.settings.show_menu_shortcuts = Some(show);
                            let _ = save_config(&config_path, &config);
                        }
                        MenuAction::ConfigureGutter => {
                            let current_gutter = config
                                .settings
                                .status_gutter
                                .as_deref()
                                .unwrap_or("{user} | {battery} | {date_time_24}");
                            active_modal = ActiveModal::Input(InputModal::new(
                                "Configure Status Gutter",
                                "Enter status gutter template:",
                                current_gutter,
                            ));
                        }
                        MenuAction::LaunchThemeEditor => {
                            let themeedit_bin = locate_tool("themeedit");
                            let _ = guard.suspend(|| {
                                let _ = Command::new(&themeedit_bin)
                                    .arg("-f")
                                    .arg(&themes_path)
                                    .status();
                                Ok(())
                            });
                            if let Ok(new_themes) = load_themes(&themes_path) {
                                themes = new_themes;
                                if let Some(t) = themes.get(&config.theme) {
                                    styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                    menu_view.set_styles(styles.clone());
                                }
                            }
                        }
                        MenuAction::Execute(item) => {
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            if item.r#type == MenuItemType::Editor {
                                let target_raw = item
                                    .action
                                    .or(item.command)
                                    .or(item.script)
                                    .unwrap_or_default();
                                let target = interpolate_placeholders(&target_raw, &ctx);
                                let editor_bin = locate_tool("bashedit");
                                let _ = guard.suspend(|| {
                                    let _ = Command::new(&editor_bin).arg(&target).status();
                                    Ok(())
                                });
                            } else {
                                let cmd_to_run = item.command.or(item.action).or(item.script);
                                if let Some(raw_cmd) = cmd_to_run {
                                    let final_cmd = interpolate_placeholders(&raw_cmd, &ctx);
                                    let _ = guard.suspend(|| {
                                        let _ = Command::new("/bin/bash")
                                            .arg("-c")
                                            .arg(&final_cmd)
                                            .status();
                                        Ok(())
                                    });
                                }
                            }
                        }
                        MenuAction::Back | MenuAction::Exit => {
                            break;
                        }
                        MenuAction::None => {}
                    }
                }
            }
        }
    }

    Ok(())
}
