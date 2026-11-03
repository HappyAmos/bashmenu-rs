//! Main executable entrypoint for BashMenu in Rust.
//!
//! # Architecture & Event Loop
//! The primary application loop in `bashmenu` coordinates between:
//! 1. [`MenuView`]: Renders the menu item hierarchy, 4-column icon slots, status gutters,
//!    and background wallpaper composites.
//! 2. [`ActiveModal`]: A state machine managing modal popups (theme selectors, help dialogs,
//!    confirm prompts, parameter inputs, markdown note viewers, and live streaming outputs).
//! 3. [`ShortcutDispatcher`]: Evaluates global `<leader>` key chords (e.g. `<Ctrl+Space>`),
//!    direct shortcuts, and renders the compact *which-key* overlay.
//! 4. [`PluginEngine`]: Periodically runs background shell scripts without blocking the UI,
//!    signaling the main thread over an MPSC channel on output arrival.
//! 5. [`TerminalGuard`]: Safely suspends raw mode when running external shell commands
//!    or launching companion tools (`bashedit`, `menuedit`, `themeedit`, `bmpager`).
//! 6. [`ToastRack`]: Displays and auto-dismisses non-blocking bottom-right notifications.

use anyhow::Result;
use bashmenu_core::{
    add_or_update_note, build_notebook_submenu, clear_command_cache, delete_note, extract_toasts,
    get_note_by_title, interpolate_placeholders, load_config, load_menu, load_notebook,
    load_shortcuts, load_themes, rename_note, resolve_notebook_path, resolve_shortcuts_path,
    save_config, save_notebook, BashMenuConfig, ConfirmAction, ExecutionMode, MacroContext,
    MenuItem, MenuItemType, MenuSchema, NoteItem, PluginEngine, ShortcutsConfig, ToastNotification,
};
use bashmenu_tui::{
    install_panic_hook, ConfirmChoice, ConfirmModal, FilePickerModal, HelpModal, InputModal,
    MenuAction, MenuView, MessageModal, NoteEditorAction, NoteEditorModal, NoteViewerAction,
    NoteViewerModal, RadioModal, ResolvedThemeStyles, ShortcutDispatchResult, ShortcutDispatcher,
    StreamOutputAction, StreamOutputModal, TerminalGuard, ThemePickerModal, ToastRack,
    ToggleChoice, ToggleModal,
};
use clap::Parser;
use crossterm::event::{self, Event};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Command-line argument schema for the main `bashmenu` executable.
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

    /// Path to custom keyboard shortcuts file (shortcuts.yml)
    #[arg(short, long)]
    shortcuts: Option<PathBuf>,

    /// Load and save configuration files in current working directory
    #[arg(long)]
    local: bool,
}

/// Active popup modal dialog currently being presented in the main menu loop.
enum ActiveModal {
    /// No modal active; main menu receives keyboard and mouse input.
    None,
    /// Contextual help overlay dialog.
    Help(HelpModal),
    /// Interactive theme selection dialog.
    ThemePicker(ThemePickerModal),
    /// Single-line generic text input modal.
    Input(InputModal),
    /// Notebook note viewer modal with interactive markdown checklist support.
    NoteViewer(NoteViewerModal),
    /// Notebook note text editor modal.
    NoteEditor(NoteEditorModal),
    /// Input prompt modal for creating a new note title.
    NewNoteTitle(String, InputModal),
    /// Input prompt modal for renaming an existing note title.
    RenameNoteTitle(String, String, InputModal),
    /// Confirmation dialog for deleting an existing note.
    ConfirmDeleteNote(String, String, ConfirmModal),
    /// Confirmation dialog before executing a menu item action.
    ConfirmItem(Box<bashmenu_core::MenuItem>, ConfirmModal),
    /// Informational message popup dialog.
    Message(MessageModal),
    /// Toggle setting value selection dialog.
    Toggle(String, ToggleModal),
    /// Config value text input dialog.
    ConfigInput(String, InputModal),
    /// Parameter input dialog for parameterized menu command execution.
    ParamInput(Box<bashmenu_core::MenuItem>, InputModal),
    /// Radio setting choice selection dialog.
    Radio(String, RadioModal),
    /// File or directory browser modal for items requiring path selection.
    ItemFilePicker(Box<bashmenu_core::MenuItem>, bool, FilePickerModal),
    /// Live process streaming output modal.
    StreamOutput(StreamOutputModal),
}

/// Resolves a configuration or asset file path, choosing between local directory or `~/.config/bashmenu/`.
fn resolve_file_path(target: &Path, local: bool) -> PathBuf {
    if local {
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            PathBuf::from(".").join(target)
        }
    } else {
        bashmenu_core::resolve_app_path(target)
    }
}

/// Discovers the path to companion tools (`menuedit`, `themeedit`, `bashedit`, `bmpager`)
/// by inspecting target directories, `BASHMENU_DIR`, and `~/.local/bin`.
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

/// Main application entrypoint for `bashmenu`.
fn main() -> Result<()> {
    // 1. Terminal Panic Recovery Hook Invariant
    install_panic_hook();

    let cli = Cli::parse();

    // 2. Load Configuration and Menus
    let config_path = resolve_file_path(&cli.config, cli.local);
    let mut config = if config_path.exists() {
        load_config(&config_path).unwrap_or_default()
    } else {
        BashMenuConfig::default()
    };

    if let Some(t) = cli.theme {
        config.theme = t;
    }

    let menu_path = resolve_file_path(&cli.menu, cli.local);
    let menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        MenuSchema::default()
    };

    let shortcuts_path = if let Some(ref sp) = cli.shortcuts {
        resolve_file_path(sp, cli.local)
    } else {
        resolve_shortcuts_path(cli.local)
    };
    let shortcuts_cfg = if shortcuts_path.exists() {
        load_shortcuts(&shortcuts_path).unwrap_or_default()
    } else {
        ShortcutsConfig::default()
    };
    let mut shortcut_dispatcher = ShortcutDispatcher::new(shortcuts_cfg);

    // 3. Load Themes
    let themes_path = resolve_file_path(Path::new("bashmenu.themes"), cli.local);
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
    let mut pending_action: Option<MenuAction> = None;
    let mut last_plugin_check = Instant::now();
    let mut last_marquee_tick = Instant::now();
    let mut toast_rack = ToastRack::new();

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
            let div_ch = styles.divider_char.chars().next().unwrap_or('─');
            let rows = plugin_engine.format_display_rows(&config, avail_w, div_ch);
            menu_view.set_plugin_rows(rows);
            need_redraw = true;
        }

        // Periodic check to trigger expired plugins and refresh status gutter
        if last_plugin_check.elapsed() >= Duration::from_secs(1) {
            plugin_engine.trigger_refresh(&config, false);
            if let Some(ref gutter_tmpl) = config.settings.status_gutter {
                let mut ctx = MacroContext::new(&config);
                ctx.window_width = size.width as usize;
                ctx.window_height = size.height as usize;
                ctx.screen_width = size.width as usize;
                let gutter_str = interpolate_placeholders(gutter_tmpl, &ctx);
                menu_view.set_status_gutter(gutter_str);
            }
            last_plugin_check = Instant::now();
        }

        // Periodic check to advance marquee ticker (250ms)
        if last_marquee_tick.elapsed() >= Duration::from_millis(250) {
            if matches!(active_modal, ActiveModal::None)
                && !shortcut_dispatcher.is_leader_pending()
                && menu_view.tick_marquee()
            {
                need_redraw = true;
            }
            last_marquee_tick = Instant::now();
        }

        // Periodic check to expire leader-key pending timeout
        if shortcut_dispatcher.poll_timeout() {
            need_redraw = true;
        }

        // Periodic check to advance toast rack expiration
        if toast_rack.tick() {
            need_redraw = true;
        }

        // Periodic poll for streaming command output
        if let ActiveModal::StreamOutput(ref mut modal) = active_modal {
            if modal.poll_output() {
                need_redraw = true;
            }
        }

        if need_redraw {
            guard.draw(|f| {
                menu_view.render(f.area(), f.buffer_mut());
                match &mut active_modal {
                    ActiveModal::None => {
                        shortcut_dispatcher.render_leader_popup(f.area(), &styles, f.buffer_mut());
                    }
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
                    ActiveModal::ConfirmItem(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::Message(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::Toggle(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::ConfigInput(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::ParamInput(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::Radio(_, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::ItemFilePicker(_, _, modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                    ActiveModal::StreamOutput(modal) => {
                        modal.render(f.area(), &styles, f.buffer_mut());
                    }
                }
                // Render non-blocking toast notifications on top
                toast_rack.render(f.area(), &styles, f.buffer_mut());
            })?;
            need_redraw = false;
        }

        if pending_action.is_some() || event::poll(Duration::from_millis(50))? {
            let ev = if pending_action.is_some() {
                Event::FocusGained
            } else {
                event::read()?
            };
            if let Event::Mouse(m) = &ev {
                if m.kind == crossterm::event::MouseEventKind::Moved {
                    continue;
                }
                if m.kind
                    == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                    && toast_rack.handle_mouse(m.column, m.row)
                {
                    need_redraw = true;
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
                        NoteViewerAction::FullEdit => {
                            let editor_bin = locate_tool("bashedit");
                            let temp_dir = std::env::temp_dir();
                            let sanitized_title: String = modal
                                .title
                                .chars()
                                .map(|c| {
                                    if c.is_alphanumeric() || c == '-' || c == '_' {
                                        c
                                    } else {
                                        '_'
                                    }
                                })
                                .collect();
                            let temp_file = temp_dir.join(format!(
                                "note_{}_{}.md",
                                std::process::id(),
                                sanitized_title
                            ));

                            if fs::write(&temp_file, &modal.content).is_ok() {
                                let original_content = modal.content.clone();
                                let _ = guard.suspend(|| {
                                    let mut cmd = Command::new(&editor_bin);
                                    cmd.arg(&temp_file);
                                    let _ = cmd.status();
                                    Ok(())
                                });

                                if let Ok(new_content) = fs::read_to_string(&temp_file) {
                                    if new_content != original_content {
                                        if let Ok(mut nb) = load_notebook(&modal.filepath) {
                                            add_or_update_note(
                                                &mut nb,
                                                NoteItem {
                                                    title: modal.title.clone(),
                                                    content: new_content.clone(),
                                                    ..Default::default()
                                                },
                                            );
                                            let _ = save_notebook(&modal.filepath, &nb);
                                            let sub = build_notebook_submenu(&modal.filepath, &nb);
                                            menu_view.reload_current_notebook(
                                                sub.items,
                                                Some(modal.title.as_str()),
                                            );
                                        }
                                        modal.content = new_content;
                                        modal.selected_line = 0;
                                        modal.scroll_offset = 0;
                                        toast_rack.add(ToastNotification::new(
                                            "Notebook saved from editor",
                                            Some("Note Updated".to_string()),
                                            3.0,
                                        ));
                                    }
                                }
                                let _ = fs::remove_file(&temp_file);
                                if let Some(parent) = temp_file.parent() {
                                    if let Some(fname) = temp_file.file_name() {
                                        let swap_file = parent
                                            .join(format!(".{}.swp", fname.to_string_lossy()));
                                        let _ = fs::remove_file(swap_file);
                                    }
                                }
                            }
                            need_redraw = true;
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
                    if let Some(choice) = confirmed {
                        if choice.is_yes() {
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
                ActiveModal::ConfirmItem(item, modal) => {
                    let choice = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(c) = choice {
                        let selected_action = match c {
                            ConfirmChoice::Yes => item.on_yes_action().cloned(),
                            ConfirmChoice::No => item.on_no_action().cloned(),
                            ConfirmChoice::Cancel => None,
                        };
                        let item_label = item.label.clone();
                        let item_interactive = item.interactive;
                        let item_quiet = item.quiet;
                        let item_alt_buffer = item.alt_buffer;
                        let item_start_dir = item.start_dir.clone();
                        active_modal = ActiveModal::None;
                        if let Some(action_obj) = selected_action {
                            match action_obj {
                                ConfirmAction::Simple(cmd) => {
                                    let trimmed = cmd.trim();
                                    if !trimmed.is_empty() {
                                        let next_item = MenuItem {
                                            label: item_label,
                                            action: Some(trimmed.to_string()),
                                            command: Some(trimmed.to_string()),
                                            r#type: MenuItemType::Command,
                                            interactive: item_interactive,
                                            quiet: item_quiet,
                                            alt_buffer: item_alt_buffer,
                                            start_dir: item_start_dir,
                                            ..Default::default()
                                        };
                                        pending_action =
                                            Some(MenuAction::Execute(Box::new(next_item)));
                                    }
                                }
                                ConfirmAction::Structured(details) => {
                                    let act_type = details.r#type.as_deref().unwrap_or("command");
                                    if act_type == "message" {
                                        let msg_title =
                                            details.title.unwrap_or_else(|| "Message".to_string());
                                        let msg_body =
                                            details.message.or(details.prompt).unwrap_or_default();
                                        active_modal = ActiveModal::Message(MessageModal::new(
                                            &msg_title, &msg_body,
                                        ));
                                    } else {
                                        let cmd = details
                                            .action
                                            .or(details.command)
                                            .or(details.script)
                                            .unwrap_or_default();
                                        let trimmed = cmd.trim();
                                        if !trimmed.is_empty() {
                                            let next_item = MenuItem {
                                                label: details.title.unwrap_or(item_label),
                                                action: Some(trimmed.to_string()),
                                                command: Some(trimmed.to_string()),
                                                r#type: if act_type == "script" {
                                                    MenuItemType::Script
                                                } else {
                                                    MenuItemType::Command
                                                },
                                                interactive: item_interactive,
                                                quiet: item_quiet,
                                                alt_buffer: item_alt_buffer,
                                                start_dir: item_start_dir,
                                                extra: details.extra,
                                                ..Default::default()
                                            };
                                            pending_action =
                                                Some(MenuAction::Execute(Box::new(next_item)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ActiveModal::Message(modal) => {
                    let dismiss = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => false,
                    };
                    if dismiss {
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
                ActiveModal::ParamInput(item, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(val) = opt_val {
                            let mut resolved_item = item.clone();
                            resolved_item.apply_param_placeholder(&val);
                            active_modal = ActiveModal::None;
                            pending_action = Some(MenuAction::Execute(resolved_item));
                        } else {
                            active_modal = ActiveModal::None;
                        }
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
                ActiveModal::ItemFilePicker(item, is_dir, modal) => {
                    let chosen = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(opt_path) = chosen {
                        if let Some(target_path) = opt_path {
                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            let mut resolved_item = item.clone();
                            resolved_item.apply_picker_result(&target_path, *is_dir, Some(&ctx));
                            active_modal = ActiveModal::None;
                            pending_action = Some(MenuAction::Execute(resolved_item));
                        } else {
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::StreamOutput(modal) => {
                    let action = match ev {
                        Event::Key(key) => modal.handle_key(key),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => StreamOutputAction::None,
                    };
                    if action == StreamOutputAction::Close {
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::None => {
                    let action = if let Some(act) = pending_action.take() {
                        act
                    } else {
                        match ev {
                            Event::Key(key) => match shortcut_dispatcher.handle_key(key) {
                                ShortcutDispatchResult::PassThrough => {
                                    menu_view.handle_key(key.code)
                                }
                                ShortcutDispatchResult::LeaderActivated
                                | ShortcutDispatchResult::LeaderCancelled => MenuAction::None,
                                ShortcutDispatchResult::LeaderUnbound(unbound) => {
                                    toast_rack.add(ToastNotification::new(
                                        format!("Unbound leader key: '<leader> {}'", unbound),
                                        Some("Leader Shortcut".to_string()),
                                        2.0,
                                    ));
                                    MenuAction::None
                                }
                                ShortcutDispatchResult::Matched(entry) => {
                                    let item = entry.to_menu_item();
                                    if item.r#type == MenuItemType::Notebook {
                                        MenuAction::OpenNotebook(Box::new(item))
                                    } else {
                                        MenuAction::Execute(Box::new(item))
                                    }
                                }
                            },
                            Event::Mouse(mouse) => {
                                if shortcut_dispatcher.is_leader_pending() {
                                    match shortcut_dispatcher.handle_mouse(mouse, area) {
                                        ShortcutDispatchResult::Matched(entry) => {
                                            let item = entry.to_menu_item();
                                            if item.r#type == MenuItemType::Notebook {
                                                MenuAction::OpenNotebook(Box::new(item))
                                            } else {
                                                MenuAction::Execute(Box::new(item))
                                            }
                                        }
                                        _ => MenuAction::None,
                                    }
                                } else {
                                    menu_view.handle_mouse(mouse, area)
                                }
                            }
                            Event::Resize(_, _) => MenuAction::None,
                            _ => MenuAction::None,
                        }
                    };

                    match action {
                        MenuAction::Help => {
                            let ctx = MacroContext::new(&config);
                            let cand = ctx.bashmenu_dir.join("bashmenu.md");
                            let doc_path = if cand.exists() {
                                cand
                            } else {
                                let dev_cand = ctx.dev_dir.join("bashmenu.md");
                                if dev_cand.exists() {
                                    dev_cand
                                } else {
                                    resolve_file_path(Path::new("bashmenu.md"), cli.local)
                                }
                            };
                            if doc_path.exists() {
                                let pager_bin = locate_tool("bmpager");
                                let _ = guard.suspend_keep_alt(|| {
                                    let _ = Command::new(&pager_bin)
                                        .arg("--markdown")
                                        .arg(&doc_path)
                                        .status();
                                    Ok(())
                                });
                                need_redraw = true;
                            } else {
                                let help_content = "BashMenu Documentation\n\nNavigation:\n  Up/Down, k/j: Navigate items\n  Enter: Select item\n  Esc/q: Back or exit\n\nFunction Keys:\n  F1: Help manual\n  F2: Theme picker\n  F3: Text editor\n  F4: Menu editor\n  F5: Reload config and menus\n  F6: Toggle shortcut numbers\n  F7: Configure status gutter\n  F8: Theme editor\n  F9: Keyboard shortcut editor".to_string();
                                active_modal = ActiveModal::Help(HelpModal::new(
                                    "BashMenu Help",
                                    &help_content,
                                ));
                            }
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
                            let _ = guard.suspend_keep_alt(|| {
                                let _ = Command::new(&editor_bin).status();
                                Ok(())
                            });
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::EditMenu => {
                            let menuedit_bin = locate_tool("menuedit");
                            let path_indices = menu_view.selected_path();
                            let path_str = path_indices
                                .iter()
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join(",");
                            let sel_title = menu_view.selected_item().map(|it| it.label.clone());
                            let title_chain = menu_view.active_title_chain().join(" > ");
                            let _ = guard.suspend_keep_alt(|| {
                                let mut cmd = Command::new(&menuedit_bin);
                                cmd.arg("-f").arg(&menu_path);
                                if !path_str.is_empty() {
                                    cmd.arg("--select-path").arg(&path_str);
                                }
                                if let Some(ref t) = sel_title {
                                    cmd.arg("--select-title").arg(t);
                                }
                                if !title_chain.is_empty() {
                                    cmd.arg("--title-chain").arg(&title_chain);
                                }
                                let _ = cmd.status();
                                Ok(())
                            });
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::Reload => {
                            clear_command_cache();

                            if let Ok(new_config) = load_config(&config_path) {
                                config = new_config;
                            }
                            use_nerd = config.settings.use_nerd_fonts.unwrap_or(false);
                            menu_view.set_use_nerd_fonts(use_nerd);
                            menu_view.set_config(config.clone());
                            menu_view.set_show_shortcuts(
                                config.settings.show_menu_shortcuts.unwrap_or(true),
                            );

                            if let Ok(new_menu) = load_menu(&menu_path) {
                                menu_view.reload_menu(new_menu);
                            }

                            if shortcuts_path.exists() {
                                if let Ok(new_shortcuts) = load_shortcuts(&shortcuts_path) {
                                    shortcut_dispatcher.set_config(new_shortcuts);
                                }
                            }

                            if let Ok(new_themes) = load_themes(&themes_path) {
                                themes = new_themes;
                            }
                            let active_theme = themes.get(&config.theme);
                            styles = if let Some(t) = active_theme {
                                ResolvedThemeStyles::from_theme(t, use_nerd)
                            } else {
                                ResolvedThemeStyles::default()
                            };
                            menu_view.set_styles(styles.clone());

                            let wp_config = config.resolve_wallpaper(active_theme);
                            menu_view.set_wallpaper(wp_config);

                            let mut ctx = MacroContext::new(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let gutter_str = config
                                .settings
                                .status_gutter
                                .as_deref()
                                .map(|tmpl| interpolate_placeholders(tmpl, &ctx))
                                .unwrap_or_default();
                            menu_view.set_status_gutter(gutter_str);

                            let avail_w = size.width.saturating_sub(6) as usize;
                            let div_ch = styles.divider_char.chars().next().unwrap_or('─');
                            let rows = plugin_engine.format_display_rows(&config, avail_w, div_ch);
                            menu_view.set_plugin_rows(rows);
                            plugin_engine.trigger_refresh(&config, true);

                            guard.request_clear();
                            need_redraw = true;
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
                            let _ = guard.suspend_keep_alt(|| {
                                let _ = Command::new(&themeedit_bin)
                                    .arg("-f")
                                    .arg(&themes_path)
                                    .arg("-t")
                                    .arg(&config.theme)
                                    .status();
                                Ok(())
                            });
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::LaunchShortcutEditor => {
                            let shortcutedit_bin = locate_tool("shortcutedit");
                            let menuedit_bin = locate_tool("menuedit");
                            let _ = guard.suspend_keep_alt(|| {
                                let status = Command::new(&shortcutedit_bin)
                                    .arg("-f")
                                    .arg(&shortcuts_path)
                                    .status();
                                if status.is_err() {
                                    let _ = Command::new(&menuedit_bin)
                                        .arg("--shortcuts")
                                        .arg("-f")
                                        .arg(&shortcuts_path)
                                        .status();
                                }
                                Ok(())
                            });
                            pending_action = Some(MenuAction::Reload);
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
                                        InputModal::new(title, prompt, &curr_val)
                                            .with_masked(item.masked()),
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
                            } else if item.r#type == MenuItemType::Confirm
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "confirm")
                            {
                                let raw_title = item.title().unwrap_or("Confirm");
                                let raw_msg = item
                                    .message()
                                    .or(item.prompt.as_deref())
                                    .unwrap_or("Are you sure?");
                                let title = interpolate_placeholders(raw_title, &ctx);
                                let msg = interpolate_placeholders(raw_msg, &ctx);
                                active_modal =
                                    ActiveModal::ConfirmItem(item, ConfirmModal::new(title, msg));
                            } else if item.has_param_placeholder() {
                                let raw_title = item.param_modal_title();
                                let clean_title = if item.extra.contains_key("title") {
                                    raw_title.to_string()
                                } else {
                                    let (main_lbl, _) =
                                        bashmenu_tui::split_label_brackets(raw_title);
                                    let trimmed = main_lbl.trim();
                                    if trimmed.is_empty() {
                                        raw_title.to_string()
                                    } else {
                                        trimmed.to_string()
                                    }
                                };
                                let title = interpolate_placeholders(&clean_title, &ctx);
                                let prompt =
                                    interpolate_placeholders(&item.param_modal_prompt(), &ctx);
                                let modal =
                                    InputModal::new(title, prompt, "").with_masked(item.masked());
                                active_modal = ActiveModal::ParamInput(item, modal);
                            } else if let Some(picker_spec) = item.first_picker_spec(&ctx) {
                                let default_title = if picker_spec.is_dir {
                                    "Select Directory"
                                } else {
                                    "Select File"
                                };
                                let raw_title = item.title().unwrap_or(default_title);
                                let title = interpolate_placeholders(raw_title, &ctx);
                                let picker = FilePickerModal::new(
                                    title,
                                    &picker_spec.start_dir,
                                    picker_spec.is_dir,
                                )
                                .with_allow_new(picker_spec.is_new);
                                active_modal =
                                    ActiveModal::ItemFilePicker(item, picker_spec.is_dir, picker);
                            } else if item.r#type == MenuItemType::Editor {
                                let target_raw = item
                                    .file
                                    .as_ref()
                                    .or(item.action.as_ref())
                                    .or(item.command.as_ref())
                                    .or(item.script.as_ref())
                                    .map(|s| s.as_str())
                                    .unwrap_or_default();
                                let target = interpolate_placeholders(target_raw, &ctx);
                                let editor_bin = locate_tool("bashedit");
                                let _ = guard.suspend_keep_alt(|| {
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
                                pending_action = Some(MenuAction::Reload);
                            } else {
                                let action_str = item.action.as_deref().unwrap_or_default();
                                let command_str = item.command.as_deref().unwrap_or_default();
                                if action_str == "themeedit"
                                    || action_str == "action_theme_editor"
                                    || command_str == "themeedit"
                                    || command_str == "action_theme_editor"
                                {
                                    let themeedit_bin = locate_tool("themeedit");
                                    let _ = guard.suspend_keep_alt(|| {
                                        let _ = Command::new(&themeedit_bin)
                                            .arg("-f")
                                            .arg(&themes_path)
                                            .arg("-t")
                                            .arg(&config.theme)
                                            .status();
                                        Ok(())
                                    });
                                    pending_action = Some(MenuAction::Reload);
                                } else if action_str == "menuedit"
                                    || action_str == "menuedit.py"
                                    || command_str == "menuedit"
                                    || command_str == "menuedit.py"
                                {
                                    let menuedit_bin = locate_tool("menuedit");
                                    let path_indices = menu_view.selected_path();
                                    let path_str = path_indices
                                        .iter()
                                        .map(|i| i.to_string())
                                        .collect::<Vec<_>>()
                                        .join(",");
                                    let sel_title =
                                        menu_view.selected_item().map(|it| it.label.clone());
                                    let title_chain = menu_view.active_title_chain().join(" > ");
                                    let _ = guard.suspend_keep_alt(|| {
                                        let mut cmd = Command::new(&menuedit_bin);
                                        cmd.arg("-f").arg(&menu_path);
                                        if !path_str.is_empty() {
                                            cmd.arg("--select-path").arg(&path_str);
                                        }
                                        if let Some(ref t) = sel_title {
                                            cmd.arg("--select-title").arg(t);
                                        }
                                        if !title_chain.is_empty() {
                                            cmd.arg("--title-chain").arg(&title_chain);
                                        }
                                        let _ = cmd.status();
                                        Ok(())
                                    });
                                    pending_action = Some(MenuAction::Reload);
                                } else if action_str == "shortcutedit"
                                    || action_str == "shortcuts_editor"
                                    || command_str == "shortcutedit"
                                    || command_str == "shortcuts_editor"
                                    || matches!(
                                        item.r#type,
                                        MenuItemType::Other(ref s)
                                            if s == "shortcuts_editor" || s == "shortcutedit"
                                    )
                                {
                                    pending_action = Some(MenuAction::LaunchShortcutEditor);
                                } else if item.r#type == MenuItemType::Other("toast".to_string())
                                    || matches!(item.r#type, MenuItemType::Other(ref s) if s == "toast")
                                {
                                    let timeout = item
                                        .extra
                                        .get("timeout")
                                        .or_else(|| item.extra.get("seconds"))
                                        .or_else(|| item.extra.get("duration"))
                                        .and_then(|v| {
                                            v.as_f64().or_else(|| v.as_i64().map(|i| i as f64))
                                        })
                                        .unwrap_or(2.0);
                                    let raw_msg = item
                                        .message()
                                        .or(item.action.as_deref())
                                        .or(item.script.as_deref())
                                        .or(item.command.as_deref())
                                        .unwrap_or_default();
                                    let (clean, toasts) = extract_toasts(raw_msg, &ctx);
                                    for t in toasts {
                                        toast_rack.add(t);
                                    }
                                    if !clean.trim().is_empty() {
                                        let title =
                                            item.title().map(|t| interpolate_placeholders(t, &ctx));
                                        let interp_msg = interpolate_placeholders(&clean, &ctx);
                                        toast_rack.add(ToastNotification::new(
                                            interp_msg, title, timeout,
                                        ));
                                    }
                                    need_redraw = true;
                                } else {
                                    let cmd_to_run = item
                                        .command
                                        .as_deref()
                                        .or(item.action.as_deref())
                                        .or(item.script.as_deref());
                                    if let Some(raw_cmd) = cmd_to_run {
                                        let (clean_cmd, toasts) = extract_toasts(raw_cmd, &ctx);
                                        for t in toasts {
                                            toast_rack.add(t);
                                            need_redraw = true;
                                        }
                                        let final_cmd = interpolate_placeholders(&clean_cmd, &ctx);
                                        if !final_cmd.trim().is_empty() {
                                            let mode = item.execution_mode();
                                            match mode {
                                                ExecutionMode::Streaming
                                                | ExecutionMode::Interactive => {
                                                    let is_interactive =
                                                        mode == ExecutionMode::Interactive;
                                                    let title = item
                                                        .title()
                                                        .map(|t| interpolate_placeholders(t, &ctx))
                                                        .unwrap_or_else(|| clean_cmd.clone());
                                                    let mut modal = StreamOutputModal::new(
                                                        title,
                                                        final_cmd,
                                                        item.window_percent(),
                                                        is_interactive,
                                                    );
                                                    modal.start();
                                                    active_modal = ActiveModal::StreamOutput(modal);
                                                    need_redraw = true;
                                                }
                                                ExecutionMode::Console => {
                                                    let is_quiet = item.quiet.unwrap_or(false);
                                                    let use_alt = item.alt_buffer.unwrap_or(true);
                                                    let run_console = || -> std::io::Result<()> {
                                                        if use_alt {
                                                            let mut stdout = std::io::stdout();
                                                            let _ = stdout
                                                                .write_all(b"\x1b[0m\x1b[H\x1b[2J");
                                                            let _ = stdout.flush();
                                                        }
                                                        if !is_quiet {
                                                            println!(
                                                                "\n--- Running Command: {} ---\n",
                                                                final_cmd
                                                            );
                                                        }
                                                        let _ = Command::new("/bin/bash")
                                                            .arg("-c")
                                                            .arg(&final_cmd)
                                                            .status();
                                                        if !is_quiet {
                                                            println!("\n--------------------------------------------------");
                                                            print!("Press [ENTER] to return to menu...");
                                                            let _ = std::io::stdout().flush();
                                                            let mut input = String::new();
                                                            let _ = std::io::stdin()
                                                                .read_line(&mut input);
                                                        }
                                                        Ok(())
                                                    };
                                                    if use_alt {
                                                        let _ = guard.suspend_keep_alt(run_console);
                                                    } else {
                                                        let _ = guard.suspend(run_console);
                                                    }
                                                    need_redraw = true;
                                                }
                                            }
                                        }
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
