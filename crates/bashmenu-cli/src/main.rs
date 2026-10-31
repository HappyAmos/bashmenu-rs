//! Main executable entrypoint for BashMenu in Rust.

use anyhow::Result;
use bashmenu_core::{
    add_or_update_note, build_notebook_submenu, delete_note, get_note_by_title,
    interpolate_placeholders, load_config, load_menu, load_notebook, load_themes, rename_note,
    resolve_notebook_path, save_config, save_notebook, BashMenuConfig, MacroContext, MenuItemType,
    MenuSchema, NoteItem, PluginEngine,
};
use bashmenu_tui::{
    install_panic_hook, ConfirmModal, FilePickerModal, HelpModal, InputModal, MenuAction, MenuView,
    NoteEditorAction, NoteEditorModal, NoteViewerAction, NoteViewerModal, RadioModal,
    ResolvedThemeStyles, TerminalGuard, ThemePickerModal, ToggleChoice, ToggleModal,
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
    NoteViewer(NoteViewerModal),
    NoteEditor(NoteEditorModal),
    NewNoteTitle(String, InputModal),
    RenameNoteTitle(String, String, InputModal),
    ConfirmDeleteNote(String, String, ConfirmModal),
    Toggle(String, ToggleModal),
    ConfigInput(String, InputModal),
    Radio(String, RadioModal),
    EditorFilePicker(Box<bashmenu_core::MenuItem>, FilePickerModal),
}

fn resolve_file_path(target: &Path) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }
    if target.components().count() > 1 && target.exists() {
        return target.to_path_buf();
    }
    // 1st Priority: ~/.config/bashmenu/
    if let Ok(home) = std::env::var("HOME") {
        let user_cfg = PathBuf::from(home).join(".config/bashmenu").join(target);
        if user_cfg.exists() {
            return user_cfg;
        }
    }
    // 2nd Priority: {bashmenu_dir}/
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        let candidate = PathBuf::from(dir).join(target);
        if candidate.exists() {
            return candidate;
        }
    }
    // 3rd Priority: current working directory
    if target.exists() {
        return target.to_path_buf();
    }
    // 4th Priority: fixtures fallback
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

    let mut use_nerd = config.settings.use_nerd_fonts.unwrap_or(false);
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
    let mut menu_view =
        MenuView::new(menu, styles.clone(), use_nerd, status_gutter).with_config(config.clone());
    let initial_wallpaper = config.resolve_wallpaper(active_theme);
    menu_view.set_wallpaper(initial_wallpaper);
    if let Some(show) = config.settings.show_menu_shortcuts {
        menu_view.set_show_shortcuts(show);
    }
    let mut active_modal = ActiveModal::None;
    let mut last_plugin_check = Instant::now();

    // 7. Interactive Event Loop
    let mut need_redraw = true;
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
            need_redraw = true;
        }

        // Periodic check to trigger expired plugins
        if last_plugin_check.elapsed() >= Duration::from_secs(1) {
            plugin_engine.trigger_refresh(&config, false);
            last_plugin_check = Instant::now();
        }

        if need_redraw {
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
                    ActiveModal::NoteViewer(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::NoteEditor(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::NewNoteTitle(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::RenameNoteTitle(_, _, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::ConfirmDeleteNote(_, _, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::Toggle(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::ConfigInput(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::Radio(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::EditorFilePicker(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                }
            })?;
            need_redraw = false;
        }

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;
            if let Event::Mouse(m) = &ev {
                if m.kind == crossterm::event::MouseEventKind::Moved {
                    continue;
                }
            }
            if let Event::Key(k) = &ev {
                if k.kind == crossterm::event::KeyEventKind::Release {
                    continue;
                }
            }
            need_redraw = true;

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
                            menu_view.set_config(config.clone());
                            if let Some(t) = themes.get(&config.theme) {
                                styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                menu_view.set_styles(styles.clone());
                                let wp_config = config.resolve_wallpaper(Some(t));
                                menu_view.set_wallpaper(wp_config);
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::Input(modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_gutter) = opt_val {
                            config.settings.status_gutter = Some(new_gutter.clone());
                            let _ = save_config(&config_path, &config);
                            menu_view.set_config(config.clone());
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            let gutter_str = interpolate_placeholders(&new_gutter, &ctx);
                            menu_view.set_status_gutter(gutter_str);
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::NoteViewer(modal) => {
                    let action = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => NoteViewerAction::None,
                    };
                    match action {
                        NoteViewerAction::Close => {
                            active_modal = ActiveModal::None;
                        }
                        NoteViewerAction::ContentModified(new_content) => {
                            if let Ok(mut nb) = load_notebook(&modal.filepath) {
                                add_or_update_note(
                                    &mut nb,
                                    NoteItem {
                                        title: modal.title.clone(),
                                        content: new_content,
                                        ..Default::default()
                                    },
                                );
                                let _ = save_notebook(&modal.filepath, &nb);
                            }
                        }
                        NoteViewerAction::Edit => {
                            active_modal = ActiveModal::NoteEditor(
                                NoteEditorModal::new(&modal.title, &modal.filepath, &modal.content)
                                    .with_cursor(modal.selected_line, 0),
                            );
                        }
                        NoteViewerAction::Rename => {
                            active_modal = ActiveModal::RenameNoteTitle(
                                modal.filepath.clone(),
                                modal.title.clone(),
                                InputModal::new(
                                    "Rename Note",
                                    "Enter new note title:",
                                    &modal.title,
                                ),
                            );
                        }
                        NoteViewerAction::Delete => {
                            active_modal = ActiveModal::ConfirmDeleteNote(
                                modal.filepath.clone(),
                                modal.title.clone(),
                                ConfirmModal::new(
                                    "Delete Note",
                                    format!(
                                        "Are you sure you want to delete note '{}'?",
                                        modal.title
                                    ),
                                ),
                            );
                        }
                        NoteViewerAction::None => {}
                    }
                }
                ActiveModal::NoteEditor(modal) => {
                    let action = match ev {
                        Event::Key(key) => modal.handle_key(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => NoteEditorAction::None,
                    };
                    match action {
                        NoteEditorAction::Save(saved_content) => {
                            if let Ok(mut nb) = load_notebook(modal.filepath.as_str()) {
                                add_or_update_note(
                                    &mut nb,
                                    NoteItem {
                                        title: modal.title.clone(),
                                        content: saved_content.clone(),
                                        ..Default::default()
                                    },
                                );
                                let _ = save_notebook(modal.filepath.as_str(), &nb);
                                let sub = build_notebook_submenu(modal.filepath.as_str(), &nb);
                                menu_view
                                    .reload_current_notebook(sub.items, Some(modal.title.as_str()));
                            }
                            active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                &modal.title,
                                &modal.filepath,
                                &saved_content,
                            ));
                        }
                        NoteEditorAction::Cancel => {
                            active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                &modal.title,
                                &modal.filepath,
                                &modal.lines.join("\n"),
                            ));
                        }
                        NoteEditorAction::None => {}
                    }
                }
                ActiveModal::NewNoteTitle(filepath, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(title) = opt_val {
                            let trimmed = title.trim();
                            if !trimmed.is_empty() {
                                active_modal = ActiveModal::NoteEditor(NoteEditorModal::new(
                                    trimmed,
                                    filepath.as_str(),
                                    "",
                                ));
                                continue;
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::RenameNoteTitle(filepath, old_title, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_title) = opt_val {
                            let trimmed = new_title.trim();
                            if !trimmed.is_empty() && trimmed != old_title.as_str() {
                                if let Ok(mut nb) = load_notebook(filepath.as_str()) {
                                    if rename_note(&mut nb, old_title.as_str(), trimmed) {
                                        let _ = save_notebook(filepath.as_str(), &nb);
                                        let sub = build_notebook_submenu(filepath.as_str(), &nb);
                                        menu_view.reload_current_notebook(sub.items, Some(trimmed));
                                    }
                                }
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::ConfirmDeleteNote(filepath, note_title, modal) => {
                    let confirmed = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(yes) = confirmed {
                        if yes {
                            if let Ok(mut nb) = load_notebook(filepath.as_str()) {
                                if delete_note(&mut nb, note_title.as_str()) {
                                    let _ = save_notebook(filepath.as_str(), &nb);
                                    let sub = build_notebook_submenu(filepath.as_str(), &nb);
                                    menu_view.reload_current_notebook(sub.items, None);
                                }
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::Toggle(key_path, modal) => {
                    let choice = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(c) = choice {
                        if c == ToggleChoice::True || c == ToggleChoice::False {
                            let val = c == ToggleChoice::True;
                            config.set_value(key_path.as_str(), serde_yml::Value::Bool(val));
                            let _ = save_config(&config_path, &config);
                            menu_view.set_config(config.clone());
                            if key_path == "settings.use_nerd_fonts" {
                                let unf = config.settings.use_nerd_fonts.unwrap_or(false);
                                use_nerd = unf;
                                menu_view.set_use_nerd_fonts(unf);
                                if let Some(t) = themes.get(&config.theme) {
                                    styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                    menu_view.set_styles(styles.clone());
                                }
                            }
                            if key_path == "settings.status_gutter" {
                                let new_gutter =
                                    config.settings.status_gutter.clone().unwrap_or_default();
                                let mut ctx = MacroContext::new(&config);
                                ctx.window_width = size.width as usize;
                                ctx.window_height = size.height as usize;
                                let gutter_str = interpolate_placeholders(&new_gutter, &ctx);
                                menu_view.set_status_gutter(gutter_str);
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::ConfigInput(key_path, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_val) = opt_val {
                            let trimmed = new_val.trim();
                            let parsed: serde_yml::Value = if trimmed.eq_ignore_ascii_case("true") {
                                serde_yml::Value::Bool(true)
                            } else if trimmed.eq_ignore_ascii_case("false") {
                                serde_yml::Value::Bool(false)
                            } else if let Ok(num) = trimmed.parse::<i64>() {
                                serde_yml::Value::Number(serde_yml::Number::from(num))
                            } else {
                                serde_yml::Value::String(trimmed.to_string())
                            };
                            config.set_value(key_path.as_str(), parsed);
                            let _ = save_config(&config_path, &config);
                            menu_view.set_config(config.clone());
                            if key_path == "settings.status_gutter" {
                                let new_gutter =
                                    config.settings.status_gutter.clone().unwrap_or_default();
                                let mut ctx = MacroContext::new(&config);
                                ctx.window_width = size.width as usize;
                                ctx.window_height = size.height as usize;
                                let gutter_str = interpolate_placeholders(&new_gutter, &ctx);
                                menu_view.set_status_gutter(gutter_str);
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::Radio(key_path, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(selected) = opt_val {
                            config.set_value(key_path.as_str(), serde_yml::Value::String(selected));
                            let _ = save_config(&config_path, &config);
                            menu_view.set_config(config.clone());
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::EditorFilePicker(item, modal) => {
                    let chosen = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_path) = chosen {
                        if let Some(target_path) = opt_path {
                            let editor_bin = locate_tool("bashedit");
                            let _ = guard.suspend(|| {
                                let mut cmd = Command::new(&editor_bin);
                                cmd.arg(target_path.to_string_lossy().as_ref());
                                if let Some(ts) = item.tabstop {
                                    cmd.arg("--tabstop").arg(ts.to_string());
                                }
                                let _ = cmd.status();
                                Ok(())
                            });
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
                                let _ = Command::new(&editor_bin).status();
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
                                menu_view.set_config(config.clone());
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
                                let wp_config = config.resolve_wallpaper(Some(t));
                                menu_view.set_wallpaper(wp_config);
                            }
                            plugin_engine.trigger_refresh(&config, true);
                        }
                        MenuAction::ToggleShortcuts => {
                            let show = menu_view.toggle_shortcuts();
                            config.settings.show_menu_shortcuts = Some(show);
                            let _ = save_config(&config_path, &config);
                            menu_view.set_config(config.clone());
                        }
                        MenuAction::ConfigureGutter => {
                            let current_gutter = config
                                .settings
                                .status_gutter
                                .as_deref()
                                .unwrap_or("{user} | {battery} | {date_time_24}");
                            let prompt = "Enter status gutter format. Available options: {user}, {battery}, {date}, {date_time_24}, {date_time_24_short}, {date_time_12}, {date_time_12_short}, {host}, {user-mode}, {version}, {utc_seconds}";
                            active_modal = ActiveModal::Input(InputModal::new(
                                "Configure Status Gutter",
                                prompt,
                                current_gutter,
                            ));
                        }
                        MenuAction::LaunchThemeEditor => {
                            let themeedit_bin = locate_tool("themeedit");
                            let _ = guard.suspend(|| {
                                let _ = Command::new(&themeedit_bin)
                                    .arg("-f")
                                    .arg(&themes_path)
                                    .arg("-t")
                                    .arg(&config.theme)
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
                        MenuAction::OpenNotebook(item) => {
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let raw_path = item.file.as_deref().or(item.action.as_deref());
                            let resolved = resolve_notebook_path(raw_path, &ctx);
                            match load_notebook(&resolved) {
                                Ok(nb) => {
                                    let sub =
                                        build_notebook_submenu(&resolved.to_string_lossy(), &nb);
                                    menu_view.push_notebook(
                                        sub.title,
                                        sub.items,
                                        resolved.to_string_lossy().to_string(),
                                    );
                                }
                                Err(e) => {
                                    active_modal = ActiveModal::Help(HelpModal::new(
                                        "Notebook Error",
                                        &format!(
                                            "Failed to load notebook '{}':\n\n{}",
                                            resolved.display(),
                                            e
                                        ),
                                    ));
                                }
                            }
                        }
                        MenuAction::ViewNote {
                            filepath,
                            note_title,
                        } => match load_notebook(&filepath) {
                            Ok(nb) => {
                                if let Some(note) = get_note_by_title(&nb, &note_title) {
                                    active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                        &note.title,
                                        &filepath,
                                        &note.content,
                                    ));
                                }
                            }
                            Err(e) => {
                                active_modal = ActiveModal::Help(HelpModal::new(
                                    "Note Error",
                                    &format!("Failed to load note file '{}':\n\n{}", filepath, e),
                                ));
                            }
                        },
                        MenuAction::NewNote(filepath) => {
                            active_modal = ActiveModal::NewNoteTitle(
                                filepath,
                                InputModal::new("New Note", "Enter note title:", ""),
                            );
                        }
                        MenuAction::RenameNote(old_title) => {
                            if let Some(filepath) = menu_view.current_notebook_file() {
                                active_modal = ActiveModal::RenameNoteTitle(
                                    filepath.to_string(),
                                    old_title.clone(),
                                    InputModal::new(
                                        "Rename Note",
                                        "Enter new note title:",
                                        &old_title,
                                    ),
                                );
                            }
                        }
                        MenuAction::DeleteNote(note_title) => {
                            if let Some(filepath) = menu_view.current_notebook_file() {
                                active_modal = ActiveModal::ConfirmDeleteNote(
                                    filepath.to_string(),
                                    note_title.clone(),
                                    ConfirmModal::new(
                                        "Delete Note",
                                        format!(
                                            "Are you sure you want to delete note '{}'?",
                                            note_title
                                        ),
                                    ),
                                );
                            }
                        }
                        MenuAction::Execute(item) => {
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            if item.r#type == MenuItemType::Toggle
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "toggle" || s == "config_toggle")
                            {
                                if let Some(key_path) = item.key() {
                                    let raw_title = item.title().unwrap_or("Toggle Setting");
                                    let raw_msg = item.message().unwrap_or("Configure option:");
                                    let title = interpolate_placeholders(raw_title, &ctx);
                                    let msg = interpolate_placeholders(raw_msg, &ctx);
                                    active_modal = ActiveModal::Toggle(
                                        key_path.to_string(),
                                        ToggleModal::new(title, msg),
                                    );
                                }
                            } else if item.r#type == MenuItemType::Radio
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "radio" || s == "config_radio")
                            {
                                if let Some(key_path) = item.key() {
                                    let raw_title = item.title().unwrap_or("Choose Setting");
                                    let raw_msg = item.message().unwrap_or("Configure option:");
                                    let title = interpolate_placeholders(raw_title, &ctx);
                                    let msg = interpolate_placeholders(raw_msg, &ctx);
                                    let options = item.options();
                                    let curr_val = config.get_value(key_path).unwrap_or_default();
                                    active_modal = ActiveModal::Radio(
                                        key_path.to_string(),
                                        RadioModal::new(title, msg, options, curr_val),
                                    );
                                }
                            } else if item.r#type == MenuItemType::Config
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "config")
                            {
                                if let Some(key_path) = item.key() {
                                    let raw_title = item.title().unwrap_or("Edit Setting");
                                    let raw_prompt = item
                                        .prompt
                                        .as_deref()
                                        .or_else(|| item.message())
                                        .unwrap_or("Enter new value:");
                                    let title = interpolate_placeholders(raw_title, &ctx);
                                    let prompt = interpolate_placeholders(raw_prompt, &ctx);
                                    let curr_val = config.get_value(key_path).unwrap_or_default();
                                    active_modal = ActiveModal::ConfigInput(
                                        key_path.to_string(),
                                        InputModal::new(title, prompt, &curr_val),
                                    );
                                }
                            } else if item.r#type == MenuItemType::ThemeSelector
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "theme_selector")
                            {
                                let theme_names: Vec<String> = themes.keys().cloned().collect();
                                active_modal = ActiveModal::ThemePicker(ThemePickerModal::new(
                                    theme_names,
                                    &config.theme,
                                ));
                            } else if item.r#type == MenuItemType::Editor {
                                let target_raw = item
                                    .file
                                    .as_ref()
                                    .or(item.action.as_ref())
                                    .or(item.command.as_ref())
                                    .or(item.script.as_ref())
                                    .map(|s| s.as_str())
                                    .unwrap_or_default();
                                if target_raw.contains("{file_picker")
                                    || target_raw.contains("{dir_picker")
                                {
                                    let is_dir_picker = target_raw.contains("dir_picker");
                                    let allow_new = target_raw.contains("file_picker_new")
                                        || target_raw.contains("dir_picker_new");
                                    let start_dir = item.start_dir.as_deref().unwrap_or(".");
                                    let exp_start = interpolate_placeholders(start_dir, &ctx);
                                    let raw_title = item.title().unwrap_or("Select File");
                                    let title = interpolate_placeholders(raw_title, &ctx);
                                    let picker = FilePickerModal::new(
                                        title,
                                        &PathBuf::from(exp_start),
                                        is_dir_picker,
                                    )
                                    .with_allow_new(allow_new);
                                    active_modal = ActiveModal::EditorFilePicker(item, picker);
                                } else {
                                    let target = interpolate_placeholders(target_raw, &ctx);
                                    let editor_bin = locate_tool("bashedit");
                                    let _ = guard.suspend(|| {
                                        let mut cmd = Command::new(&editor_bin);
                                        let trimmed = target.trim();
                                        if !trimmed.is_empty() {
                                            cmd.arg(trimmed);
                                        }
                                        if let Some(ts) = item.tabstop {
                                            cmd.arg("--tabstop").arg(ts.to_string());
                                        }
                                        let _ = cmd.status();
                                        Ok(())
                                    });
                                }
                            } else {
                                let action_str = item.action.as_deref().unwrap_or_default();
                                let command_str = item.command.as_deref().unwrap_or_default();
                                if action_str == "themeedit"
                                    || action_str == "action_theme_editor"
                                    || command_str == "themeedit"
                                    || command_str == "action_theme_editor"
                                {
                                    let themeedit_bin = locate_tool("themeedit");
                                    let _ = guard.suspend(|| {
                                        let _ = Command::new(&themeedit_bin)
                                            .arg("-f")
                                            .arg(&themes_path)
                                            .arg("-t")
                                            .arg(&config.theme)
                                            .status();
                                        Ok(())
                                    });
                                    if let Ok(new_themes) = load_themes(&themes_path) {
                                        themes = new_themes;
                                    }
                                    if let Some(t) = themes.get(&config.theme) {
                                        styles = ResolvedThemeStyles::from_theme(t, use_nerd);
                                        menu_view.set_styles(styles.clone());
                                    }
                                } else if action_str == "menuedit"
                                    || action_str == "menuedit.py"
                                    || command_str == "menuedit"
                                    || command_str == "menuedit.py"
                                {
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
