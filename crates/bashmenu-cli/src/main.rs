//! Main executable entrypoint for BashMenu in Rust.
//!
//! # Architecture & Event Loop
//! The primary application loop in `bashmenu` coordinates between:
//! 1. [`MenuView`]: Renders the menu item hierarchy, 4-column icon slots,
//!    status gutters, and background wallpaper composites.
//! 2. [`ActiveModal`]: A state machine managing modal popups (theme
//!    selectors, help dialogs, confirm prompts, parameter inputs, markdown
//!    note viewers, and live streaming outputs).
//! 3. [`ShortcutDispatcher`]: Evaluates global `<leader>` key chords
//!    (e.g. `<Ctrl+Space>`), direct shortcuts, and renders the compact
//!    *which-key* overlay.
//! 4. [`PluginEngine`]: Periodically runs background shell scripts without
//!    blocking the UI, signaling the main thread over an MPSC channel on
//!    output arrival.
//! 5. [`TerminalGuard`]: Safely suspends raw mode when running external
//!    shell commands or launching companion tools (`bashedit`, `menuedit`,
//!    `themeedit`, `bmpager`).
//! 6. [`ToastRack`]: Displays and auto-dismisses non-blocking bottom-right
//!    notifications.

mod ui_rpc;

use anyhow::Result;
use bashmenu_core::pick_random_theme;
use bashmenu_core::{
    add_or_update_note, build_notebook_submenu, clear_command_cache, delete_note, extract_toasts,
    get_note_by_title, interpolate_placeholders, load_config, load_menu, load_notebook,
    load_shortcuts, load_themes, markdown_reference_text, merge_note_content, rename_note,
    resolve_notebook_path, resolve_shortcuts_path, save_config, save_notebook, t, BashMenuConfig,
    ConfirmAction, ExecutionMode, MacroContext, MenuItem, MenuItemType, MenuSchema, NoteItem,
    NoteSwapFile, PluginEngine, ShortcutsConfig, ToastNotification, ToastSeverity,
};
use bashmenu_tui::{
    install_panic_hook, is_framebuffer_available, ColorTier, ConfirmChoice, ConfirmModal,
    FilePickerModal, FramebufferDevice, HelpModal, InputModal, MenuAction, MenuView, MessageModal,
    ModalAnimation, NoteConflictChoice, NoteConflictModal, NoteEditorAction, NoteEditorModal,
    NoteViewerAction, NoteViewerModal, RadioModal, ResolvedThemeStyles, ShortcutDispatchResult,
    ShortcutDispatcher, StreamOutputAction, StreamOutputModal, TerminalGuard, ThemeActivity,
    ThemePickerModal, ToastRack, ToggleChoice, ToggleModal, TransitionEffect, TransitionSpeed,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Adds a red error toast notification to `toast_rack`.
fn push_error_toast(toast_rack: &mut ToastRack, title: &str, message: String, timeout_secs: f64) {
    toast_rack.add(ToastNotification {
        message,
        title: Some(title.to_string()),
        severity: ToastSeverity::Error,
        timeout_secs,
    });
}

/// Persists `config` to `path`, surfacing any save block or I/O failure as an
/// error toast.
fn save_config_or_toast(path: &Path, config: &BashMenuConfig, toast_rack: &mut ToastRack) {
    if let Err(e) = save_config(path, config) {
        let title = t!("bashmenu.toast_config_save_blocked");
        push_error_toast(toast_rack, &title, e.to_string(), 6.0);
    }
}

/// Builds the list of radio modal options for language / locale selection,
/// combining system auto-detection with dynamically discovered locale files.
fn build_locale_radio_options() -> Vec<(String, String)> {
    let mut options = Vec::new();
    let current = bashmenu_core::current_locale();
    options.push((
        "auto".to_string(),
        format!("Auto (System Default) [{}]", current),
    ));

    for loc in bashmenu_core::discover_locales() {
        options.push((loc.code.clone(), format!("{} [{}]", loc.name, loc.code)));
    }

    options
}

/// Resolves `MainMenu`, `Modals`, and `Toasts` styles from an optional theme.
fn resolve_bashmenu_styles(
    theme: Option<&bashmenu_core::Theme>,
    use_nerd: bool,
    toast_timeout: Option<f64>,
) -> (
    ResolvedThemeStyles,
    ResolvedThemeStyles,
    ResolvedThemeStyles,
) {
    if let Some(t) = theme {
        (
            ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::MainMenu)
                .with_toast_timeout(toast_timeout),
            ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::Modals)
                .with_toast_timeout(toast_timeout),
            ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::Toasts)
                .with_toast_timeout(toast_timeout),
        )
    } else {
        (
            ResolvedThemeStyles::default().with_toast_timeout(toast_timeout),
            ResolvedThemeStyles::default().with_toast_timeout(toast_timeout),
            ResolvedThemeStyles::default().with_toast_timeout(toast_timeout),
        )
    }
}

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
    #[arg(short, long)]
    menu: Option<PathBuf>,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Override active theme
    #[arg(short, long)]
    theme: Option<String>,

    /// Path to custom keyboard shortcuts file (shortcuts.yml)
    #[arg(short, long)]
    shortcuts: Option<PathBuf>,

    /// Load and save configuration files in current working directory
    #[arg(long)]
    local: bool,

    /// Load configuration files from the repository defaults directory
    #[arg(short, long)]
    repo: bool,

    /// Render full-resolution TrueColor background images directly via framebuffer
    #[arg(short, long)]
    image: bool,

    /// Override active locale (e.g. "en_US", "es_ES", or path to YAML locale file)
    #[arg(short, long)]
    locale: Option<String>,
}

/// State preserved while an interactive Markdown form's reserved script runs.
struct SuspendedInteractiveForm {
    modal: Box<bashmenu_tui::InteractiveMarkdownModal>,
    reserved_scripts: std::collections::HashMap<String, bashmenu_core::ScriptBlock>,
    frontmatter: bashmenu_core::MarkdownFrontmatter,
    fallback_title: String,
    close_after_action: bool,
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
    /// Markdown syntax reference overlay opened from the note editor.
    NoteMarkdownRef {
        /// Suspended note editor state restored when the reference closes.
        editor: Box<NoteEditorModal>,
        /// Markdown reference help modal.
        help: HelpModal,
    },
    /// Input prompt modal for creating a new note title.
    NewNoteTitle(String, InputModal),
    /// Input prompt modal for renaming an existing note title.
    RenameNoteTitle(String, String, InputModal),
    /// Confirmation dialog for deleting an existing note.
    ConfirmDeleteNote(String, String, ConfirmModal),
    /// Confirmation dialog before executing a menu item action.
    ConfirmItem(Box<bashmenu_core::MenuItem>, ConfirmModal),
    /// Confirmation dialog for modular code block injection/removal.
    ConfirmInjectBlock {
        /// Confirmation dialog modal widget.
        modal: ConfirmModal,
        /// Absolute destination path of the target profile or script.
        target_path: PathBuf,
        /// Formatted code block with delimiters to inject.
        block_to_inject: String,
        /// Start delimiter string.
        start_marker: String,
        /// End delimiter string.
        end_marker: String,
        /// Unique block identifier.
        block_id: String,
        /// Whether the code block is already present in target file.
        block_present: bool,
    },
    /// Informational message popup dialog.
    Message(MessageModal),
    /// Interactive `{message:"..."}` popup dialog with optional remaining item continuation.
    ItemMessage(Box<bashmenu_core::MenuItem>, MessageModal),
    /// Live UI-RPC `{param:...}` input modal triggered by an interactive Markdown script.
    RpcParam(
        std::sync::mpsc::Sender<bashmenu_core::UiRpcResponse>,
        InputModal,
    ),
    /// Live UI-RPC `{message:...}` popup modal triggered by an interactive Markdown script.
    RpcMessage(
        std::sync::mpsc::Sender<bashmenu_core::UiRpcResponse>,
        MessageModal,
    ),
    /// Interactive Markdown form modal with embedded `{input:...}` and `{button:...}` widgets.
    InteractiveForm {
        modal: Box<bashmenu_tui::InteractiveMarkdownModal>,
        reserved_scripts: std::collections::HashMap<String, bashmenu_core::ScriptBlock>,
        frontmatter: bashmenu_core::MarkdownFrontmatter,
        fallback_title: String,
    },
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
    /// Concurrent editing conflict resolution modal.
    NoteConflict(NoteConflictModal),
}

impl ActiveModal {
    /// Calculates the outer frame rectangle for this modal if one is active.
    fn dialog_rect(&self, area: ratatui::layout::Rect) -> Option<ratatui::layout::Rect> {
        let (w, h) = match self {
            ActiveModal::None => return None,
            ActiveModal::Help(m) => m.dialog_dimensions(area),
            ActiveModal::ThemePicker(m) => m.dialog_dimensions(area),
            ActiveModal::Input(m)
            | ActiveModal::NewNoteTitle(_, m)
            | ActiveModal::RenameNoteTitle(_, _, m)
            | ActiveModal::ConfigInput(_, m)
            | ActiveModal::ParamInput(_, m)
            | ActiveModal::RpcParam(_, m) => m.dialog_dimensions(area),
            ActiveModal::NoteViewer(m) => m.dialog_dimensions(area),
            ActiveModal::NoteEditor(m) => m.dialog_dimensions(area),
            ActiveModal::NoteMarkdownRef { help, .. } => help.dialog_dimensions(area),
            ActiveModal::ConfirmDeleteNote(_, _, m)
            | ActiveModal::ConfirmItem(_, m)
            | ActiveModal::ConfirmInjectBlock { modal: m, .. } => m.dialog_dimensions(area),
            ActiveModal::Message(m)
            | ActiveModal::ItemMessage(_, m)
            | ActiveModal::RpcMessage(_, m) => m.dialog_dimensions(area),
            ActiveModal::InteractiveForm { modal: m, .. } => m.dialog_dimensions(area),
            ActiveModal::Toggle(_, m) => m.dialog_dimensions(area),
            ActiveModal::Radio(_, m) => m.dialog_dimensions(area),
            ActiveModal::ItemFilePicker(_, _, m) => m.dialog_dimensions(area),
            ActiveModal::StreamOutput(m) => m.dialog_dimensions(area),
            ActiveModal::NoteConflict(m) => m.dialog_dimensions(area),
        };
        Some(bashmenu_tui::compute_modal_rect(area, w, h))
    }
}

/// Resolves a configuration or asset file path, choosing between local
/// directory or `~/.config/bashmenu/`.
fn resolve_file_path(target: &Path, local: bool) -> PathBuf {
    bashmenu_core::resolve_path_with_local(target, local)
}

/// Discovers the path to companion tools (`menuedit`, `themeedit`, `bashedit`,
/// `bmpager`) across executable directory, `BASHMENU_DIR`, and `~/.local/bin`.
fn locate_tool(name: &str) -> PathBuf {
    let found = bashmenu_core::locate_companion_tool(name);
    if found.is_file() {
        return found;
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
    found
}

/// Builds the localized `clap::Command` for `bashmenu`.
fn build_cli_command() -> clap::Command {
    use clap::CommandFactory;
    Cli::command()
        .about(t!("cli.bashmenu.about"))
        .help_template(t!("cli.help_template"))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .mut_arg("menu", |a| a.help(t!("cli.bashmenu.menu")))
        .mut_arg("config", |a| a.help(t!("cli.bashmenu.config")))
        .mut_arg("theme", |a| a.help(t!("cli.bashmenu.theme")))
        .mut_arg("shortcuts", |a| a.help(t!("cli.bashmenu.shortcuts")))
        .mut_arg("local", |a| a.help(t!("cli.bashmenu.local")))
        .mut_arg("repo", |a| a.help(t!("cli.bashmenu.repo")))
        .mut_arg("image", |a| a.help(t!("cli.bashmenu.image")))
        .mut_arg("locale", |a| a.help(t!("cli.bashmenu.locale")))
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

/// Main application entrypoint for `bashmenu`.
fn main() -> Result<()> {
    // 0. Fast-path UI-RPC client mode (invoked by interactive Markdown scripts)
    let raw_args: Vec<String> = std::env::args().collect();
    if let Some(code) = ui_rpc::maybe_run_ui_rpc_client(&raw_args) {
        std::process::exit(code);
    }

    // 1. Terminal Panic Recovery Hook Invariant
    install_panic_hook();

    bashmenu_core::preinit_locale_from_args(true);
    let matches = build_cli_command().get_matches();
    let cli =
        <Cli as clap::FromArgMatches>::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    let repo_dir = if cli.repo {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"));
        let cfg_dir = bashmenu_core::get_user_config_dir();
        Some(bashmenu_core::resolve_dev_dir(&home, &cfg_dir))
    } else {
        None
    };

    // 2. Load Configuration and Menus
    let config_path = if let Some(ref cp) = cli.config {
        resolve_file_path(cp, cli.local)
    } else if let Some(ref rdir) = repo_dir {
        rdir.join("defaults/bashmenu.yml")
    } else if cli.local {
        PathBuf::from("./bashmenu.yml")
    } else {
        bashmenu_core::get_user_config_dir().join("bashmenu.yml")
    };
    let mut startup_config_err: Option<String> = None;
    let mut config = if config_path.exists() {
        match load_config(&config_path) {
            Ok(c) => c,
            Err(e) => {
                startup_config_err =
                    Some(format!("Failed to parse {}: {}", config_path.display(), e));
                let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
                let dev_cfg = repo_dir
                    .clone()
                    .unwrap_or_else(|| {
                        bashmenu_core::resolve_dev_dir(&home, &bashmenu_core::get_user_config_dir())
                    })
                    .join("defaults/bashmenu.yml");
                if dev_cfg.exists() && dev_cfg != config_path {
                    load_config(&dev_cfg).unwrap_or_default()
                } else {
                    BashMenuConfig::default()
                }
            }
        }
    } else {
        let vanilla_cfg = bashmenu_core::resolve_app_path("bashmenu.yml");
        if vanilla_cfg.exists() && vanilla_cfg != config_path {
            if let Ok(c) = load_config(&vanilla_cfg) {
                let _ = bashmenu_core::save_config(&config_path, &c);
                c
            } else {
                BashMenuConfig::default()
            }
        } else {
            BashMenuConfig::default()
        }
    };

    bashmenu_core::init_locale_from_env(cli.locale.as_deref(), config.settings.locale.as_deref());

    if let Some(t) = cli.theme {
        config.theme = t;
    }

    let menu_path = if let Some(ref mp) = cli.menu {
        resolve_file_path(mp, cli.local)
    } else if let Some(ref rdir) = repo_dir {
        rdir.join("defaults/bashmenu.mnu")
    } else if cli.local {
        PathBuf::from("./bashmenu.mnu")
    } else {
        bashmenu_core::get_user_config_dir().join("bashmenu.mnu")
    };
    let menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        let vanilla_menu = bashmenu_core::resolve_app_path("bashmenu.mnu");
        if vanilla_menu.exists() && vanilla_menu != menu_path {
            if let Ok(m) = load_menu(&vanilla_menu) {
                let _ = bashmenu_core::save_menu(&menu_path, &m);
                m
            } else {
                MenuSchema::default()
            }
        } else {
            MenuSchema::default()
        }
    };

    let shortcuts_path = if let Some(ref sp) = cli.shortcuts {
        resolve_file_path(sp, cli.local)
    } else if let Some(ref rdir) = repo_dir {
        rdir.join("defaults/shortcuts.yml")
    } else {
        resolve_shortcuts_path(cli.local)
    };
    let shortcuts_cfg = if shortcuts_path.exists() {
        load_shortcuts(&shortcuts_path).unwrap_or_default()
    } else {
        ShortcutsConfig::default()
    };
    let mut shortcut_dispatcher = ShortcutDispatcher::new(shortcuts_cfg.clone());

    // 3. Load Themes
    let themes_path = if let Some(ref rdir) = repo_dir {
        rdir.join("defaults/bashmenu.themes")
    } else if cli.local {
        PathBuf::from("./bashmenu.themes")
    } else {
        let user_themes = bashmenu_core::get_user_config_dir().join("bashmenu.themes");
        if user_themes.exists() {
            user_themes
        } else {
            resolve_file_path(Path::new("bashmenu.themes"), false)
        }
    };
    let mut themes = if themes_path.exists() {
        load_themes(&themes_path).unwrap_or_default()
    } else {
        Default::default()
    };

    let cheat_path = if let Some(ref rdir) = repo_dir {
        rdir.join("defaults/cheat.yml")
    } else if cli.local {
        PathBuf::from("./cheat.yml")
    } else {
        let user_cheat = bashmenu_core::get_user_config_dir().join("cheat.yml");
        if user_cheat.exists() {
            user_cheat
        } else {
            bashmenu_core::resolve_app_path(Path::new("cheat.yml"))
        }
    };

    // Direct Linux Framebuffer (/dev/fb0) detection, desktop terminal guard, & setup
    let fb_mode = config.settings.framebuffer.as_deref().unwrap_or("auto");
    let on_linux_console = bashmenu_tui::is_linux_console();
    let in_desktop_term = bashmenu_tui::is_desktop_terminal();
    let force_fb = std::env::var("BASHMENU_FORCE_FB").as_deref() == Ok("1")
        || (fb_mode == "true" && !in_desktop_term);
    let use_fb_image_mode = cli.image || fb_mode == "image";
    let want_fb =
        fb_mode != "false" && is_framebuffer_available() && (force_fb || on_linux_console);

    let mut fb_perm_denied = false;
    let mut fb_device = if want_fb {
        if let Some(fb) = FramebufferDevice::open() {
            Some(fb)
        } else if bashmenu_tui::is_framebuffer_permission_denied() {
            if bashmenu_tui::ensure_framebuffer_access_interactive() {
                FramebufferDevice::open()
            } else {
                fb_perm_denied = true;
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if (fb_device.is_some() || cli.image) && std::env::var("BASHMENU_COLOR_TIER").is_err() {
        std::env::set_var("BASHMENU_COLOR_TIER", "truecolor");
    }

    let mut use_nerd =
        config.settings.use_nerd_fonts.unwrap_or(true) && !on_linux_console && fb_device.is_none();
    let (mut active_theme_name, active_theme) =
        if bashmenu_core::is_random_theme_spec(&config.theme) {
            if let Some((name, t)) = pick_random_theme(&themes) {
                (name.to_string(), Some(t))
            } else {
                (config.theme.clone(), None)
            }
        } else {
            (config.theme.clone(), themes.get(&config.theme))
        };
    let (mut styles, mut modal_styles, mut toast_styles) =
        resolve_bashmenu_styles(active_theme, use_nerd, config.settings.toast_timeout);

    let active_color_mode = match styles.tier {
        ColorTier::Color8 => "3-bit (8 color)",
        ColorTier::Color16 => "16 color",
        ColorTier::Color256 => "256 color",
        ColorTier::TrueColor => "truecolor",
    };
    let active_tty_mode = if fb_device.is_some() || on_linux_console {
        "virtual console (tty)".to_string()
    } else {
        bashmenu_core::detect_tty_mode()
    };

    // 4. Macro Context & Status Gutter
    let active_bashmenu_dir = if let Some(ref rdir) = repo_dir {
        rdir.clone()
    } else if cli.local {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        bashmenu_core::get_user_config_dir()
    };

    macro_rules! make_macro_ctx {
        ($cfg:expr) => {
            MacroContext::new($cfg)
                .with_bashmenu_dir(active_bashmenu_dir.clone())
                .with_repo_mode(cli.repo)
                .with_local_mode(cli.local)
                .with_paths(
                    config_path.clone(),
                    menu_path.clone(),
                    themes_path.clone(),
                    shortcuts_path.clone(),
                    cheat_path.clone(),
                )
                .with_color_mode(active_color_mode)
                .with_tty_mode(active_tty_mode.clone())
        };
    }
    let status_gutter = {
        let initial_ctx = make_macro_ctx!(&config);
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
    if let Some(ref mut fb) = fb_device {
        fb.set_image_mode(use_fb_image_mode);
    }
    let mut guard = TerminalGuard::init_with_framebuffer(fb_device)?;
    let mut menu_view = MenuView::new(menu, styles.clone(), use_nerd, status_gutter)
        .with_config(config.clone())
        .with_shortcuts(shortcuts_cfg.shortcuts.clone())
        .with_leader_key(shortcuts_cfg.leader_key.clone())
        .with_allow_command_macros(!cli.local);
    let initial_wallpaper = config.resolve_wallpaper(active_theme);
    menu_view.set_wallpaper(initial_wallpaper.clone());
    if let Some(show) = config.settings.show_menu_shortcuts {
        menu_view.set_show_shortcuts(show);
    }
    let mut active_modal = ActiveModal::None;
    let mut active_interactive_md: Option<ui_rpc::InteractiveMdSession> = None;
    let mut suspended_interactive_form: Option<SuspendedInteractiveForm> = None;
    let mut pending_action: Option<MenuAction> = None;
    let mut last_plugin_check = Instant::now();
    let mut last_marquee_tick = Instant::now();
    let mut last_interaction = Instant::now();
    let mut toast_rack = ToastRack::new();
    if let Some(err_msg) = startup_config_err {
        let title = t!("bashmenu.toast_config_syntax_title");
        let msg = t!("bashmenu.toast_config_syntax_msg", err => &err_msg);
        push_error_toast(&mut toast_rack, &title, msg, 8.0);
    }

    if let Some(fb) = guard.framebuffer_mut() {
        fb.set_image_mode(use_fb_image_mode);
        if use_fb_image_mode {
            fb.set_wallpaper(initial_wallpaper.as_ref(), styles.background);
            menu_view.set_framebuffer_image_mode(true);
        }
    } else if fb_perm_denied {
        toast_rack.add(ToastNotification::new(
            t!("bashmenu.toast_fb_perm_msg"),
            Some(t!("bashmenu.toast_fb_perm_title")),
            6.0,
        ));
    } else if use_fb_image_mode && in_desktop_term {
        toast_rack.add(ToastNotification::new(
            t!("bashmenu.toast_fb_desktop_msg"),
            Some(t!("bashmenu.toast_fb_desktop_title")),
            4.5,
        ));
    }
    guard.set_default_bg(styles.background);

    // 7. Interactive Event Loop
    let mut need_redraw = true;
    let mut pending_transition = true;
    let mut modal_anchor: (u16, u16) = (0, 0);
    let mut pending_modal_grow = false;
    loop {
        let size = guard.terminal_mut().size()?;
        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
        let trans_speed = TransitionSpeed::parse(config.settings.transition_speed.as_deref());
        let modal_speed_str = config
            .settings
            .modal_animation_speed
            .as_deref()
            .or(config.settings.transition_speed.as_deref())
            .unwrap_or("fast");
        let modal_speed = TransitionSpeed::parse(Some(modal_speed_str));
        let modal_anim = ModalAnimation::parse(config.settings.modal_animation.as_deref());
        let modal_anim_enabled =
            modal_anim == ModalAnimation::Zoom && modal_speed != TransitionSpeed::Custom(0);

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

        // Poll active interactive Markdown runner for UI-RPC modal requests or completion
        if let Some(ref session) = active_interactive_md {
            if matches!(active_modal, ActiveModal::None) {
                while let Ok(pending) = session.rpc_rx.try_recv() {
                    match pending.request {
                        bashmenu_core::UiRpcRequest::Param {
                            title,
                            text,
                            default_value,
                            masked,
                        } => {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let t = if title.is_empty() {
                                String::new()
                            } else {
                                interpolate_placeholders(&title, &ctx)
                            };
                            let p = if text.is_empty() {
                                String::new()
                            } else {
                                interpolate_placeholders(&text, &ctx)
                            };
                            let d = if default_value.is_empty() {
                                String::new()
                            } else {
                                interpolate_placeholders(&default_value, &ctx)
                            };
                            active_modal = ActiveModal::RpcParam(
                                pending.reply_tx,
                                InputModal::new(t, p, &d).with_masked(masked),
                            );
                            need_redraw = true;
                            break;
                        }
                        bashmenu_core::UiRpcRequest::Message { title, text } => {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let t = if title.is_empty() {
                                String::new()
                            } else {
                                interpolate_placeholders(&title, &ctx)
                            };
                            let body = interpolate_placeholders(&text, &ctx);
                            active_modal = ActiveModal::RpcMessage(
                                pending.reply_tx,
                                MessageModal::new(t, body),
                            );
                            need_redraw = true;
                            break;
                        }
                        bashmenu_core::UiRpcRequest::Toast {
                            text,
                            duration_secs,
                        } => {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let msg = interpolate_placeholders(&text, &ctx);
                            toast_rack.add(ToastNotification::new(
                                msg,
                                None,
                                duration_secs.unwrap_or(styles.toast_timeout),
                            ));
                            let _ = pending.reply_tx.send(bashmenu_core::UiRpcResponse {
                                ok: true,
                                value: String::new(),
                                cancelled: false,
                            });
                            need_redraw = true;
                        }
                        bashmenu_core::UiRpcRequest::Placeholder { text } => {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;
                            let expanded = interpolate_placeholders(&text, &ctx);
                            let _ = pending.reply_tx.send(bashmenu_core::UiRpcResponse {
                                ok: true,
                                value: expanded,
                                cancelled: false,
                            });
                        }
                    }
                }
            }
        }
        let finished_md = active_interactive_md
            .as_ref()
            .and_then(|s| s.done_rx.try_recv().ok());
        if let Some(done) = finished_md {
            active_interactive_md = None;
            if let Some(mut suspended) = suspended_interactive_form.take() {
                if !suspended.close_after_action && suspended.modal.advance_page() {
                    active_modal = ActiveModal::InteractiveForm {
                        modal: suspended.modal,
                        reserved_scripts: suspended.reserved_scripts,
                        frontmatter: suspended.frontmatter,
                        fallback_title: suspended.fallback_title,
                    };
                } else {
                    let mut ctx = make_macro_ctx!(&config).with_allow_command_macros(false);
                    ctx.window_width = size.width as usize;
                    ctx.window_height = size.height as usize;
                    ctx.screen_width = size.width as usize;
                    let title = done
                        .frontmatter
                        .title
                        .as_deref()
                        .map(|t| interpolate_placeholders(t, &ctx))
                        .unwrap_or(done.fallback_title);
                    let rendered_body = interpolate_placeholders(&done.markdown_body, &ctx);
                    if !rendered_body.trim().is_empty() {
                        active_modal = ActiveModal::Message(
                            MessageModal::new(title, rendered_body)
                                .with_dimensions(done.frontmatter.width, done.frontmatter.height),
                        );
                    }
                }
            } else {
                let mut ctx = make_macro_ctx!(&config).with_allow_command_macros(false);
                ctx.window_width = size.width as usize;
                ctx.window_height = size.height as usize;
                ctx.screen_width = size.width as usize;
                let title = done
                    .frontmatter
                    .title
                    .as_deref()
                    .map(|t| interpolate_placeholders(t, &ctx))
                    .unwrap_or(done.fallback_title);
                let rendered_body = interpolate_placeholders(&done.markdown_body, &ctx);
                if !rendered_body.trim().is_empty() {
                    active_modal = ActiveModal::Message(
                        MessageModal::new(title, rendered_body)
                            .with_dimensions(done.frontmatter.width, done.frontmatter.height),
                    );
                }
            }
            need_redraw = true;
        }

        // Periodic check to trigger expired plugins and refresh status gutter
        let refresh_sec = config
            .settings
            .refresh_interval
            .as_ref()
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1);
        let has_seconds = config
            .settings
            .status_gutter
            .as_deref()
            .map(|s| {
                s.contains("{date_time_24}")
                    || s.contains("{date_time_12}")
                    || s.contains("{utc_seconds}")
            })
            .unwrap_or(false);
        let gutter_interval = if has_seconds {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(refresh_sec.min(5))
        };

        if last_plugin_check.elapsed() >= gutter_interval {
            plugin_engine.trigger_refresh(&config, false);
            if let Some(ref gutter_tmpl) = config.settings.status_gutter {
                let mut ctx = make_macro_ctx!(&config);
                ctx.window_width = size.width as usize;
                ctx.window_height = size.height as usize;
                ctx.screen_width = size.width as usize;
                let gutter_str = interpolate_placeholders(gutter_tmpl, &ctx);
                if menu_view.update_status_gutter(gutter_str) {
                    need_redraw = true;
                }
            }
            last_plugin_check = Instant::now();
        }

        // Periodic check to advance marquee ticker (only when a visible row overflows)
        let marquee_interval = config.settings.marquee_speed_duration();
        let marquee_active = matches!(active_modal, ActiveModal::None)
            && !shortcut_dispatcher.is_leader_pending()
            && menu_view.has_active_marquee();
        if marquee_active && last_marquee_tick.elapsed() >= marquee_interval {
            if menu_view.tick_marquee() {
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

        // Periodic check for help gutter hover tooltip expansion (4 seconds)
        if matches!(active_modal, ActiveModal::None)
            && !shortcut_dispatcher.is_leader_pending()
            && menu_view.check_hover_timeout(area)
        {
            need_redraw = true;
        }

        if need_redraw {
            let transition_effect = if pending_transition {
                pending_transition = false;
                TransitionEffect::parse(config.settings.transition.as_deref())
            } else {
                TransitionEffect::None
            };

            let modal_grow_rect = if pending_modal_grow && modal_anim_enabled {
                active_modal.dialog_rect(area)
            } else {
                None
            };

            if let Some(target_rect) = modal_grow_rect {
                pending_modal_grow = false;
                let _ = guard.animate_modal_grow(
                    modal_anchor,
                    target_rect,
                    &styles,
                    modal_speed,
                    |f| menu_view.render(f.area(), f.buffer_mut()),
                );
            }

            let render_fn = |f: &mut ratatui::Frame<'_>| {
                menu_view.render(f.area(), f.buffer_mut());
                match &mut active_modal {
                    ActiveModal::None => {
                        shortcut_dispatcher.render_leader_popup(
                            f.area(),
                            &modal_styles,
                            f.buffer_mut(),
                        );
                    }
                    ActiveModal::Help(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ThemePicker(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::Input(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::NoteViewer(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::NoteEditor(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::NoteMarkdownRef { help, .. } => {
                        help.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::NewNoteTitle(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::RenameNoteTitle(_, _, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ConfirmDeleteNote(_, _, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ConfirmItem(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ConfirmInjectBlock { modal, .. } => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::Message(modal)
                    | ActiveModal::ItemMessage(_, modal)
                    | ActiveModal::RpcMessage(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::InteractiveForm { modal, .. } => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::Toggle(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ConfigInput(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ParamInput(_, modal) | ActiveModal::RpcParam(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::Radio(_, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::ItemFilePicker(_, _, modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::StreamOutput(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                    ActiveModal::NoteConflict(modal) => {
                        modal.render(f.area(), &modal_styles, f.buffer_mut());
                    }
                }
                // Render non-blocking toast notifications on top
                toast_rack.render(f.area(), &toast_styles, f.buffer_mut());
            };

            if transition_effect != TransitionEffect::None {
                guard.draw_with_transition(transition_effect, trans_speed, render_fn)?;
            } else {
                guard.draw(render_fn)?;
            }
            need_redraw = false;
        }

        // Periodic check for inactivity timeout
        if let Some(ref timeout) = config.settings.inactivity_timeout {
            if let (Some(ms), Some(cmd_raw)) = (timeout.milliseconds, timeout.command.as_deref()) {
                if ms > 0 && last_interaction.elapsed().as_millis() >= ms as u128 {
                    last_interaction = Instant::now();
                    let mut ctx = make_macro_ctx!(&config);
                    ctx.window_width = size.width as usize;
                    ctx.window_height = size.height as usize;
                    let cmd = interpolate_placeholders(cmd_raw, &ctx);
                    if !cmd.trim().is_empty() {
                        let eff = TransitionEffect::parse(config.settings.transition.as_deref());
                        if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                            let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                menu_view.render(f.area(), f.buffer_mut());
                            });
                        }
                        let _ = guard.suspend_keep_alt(|| {
                            let _ = Command::new("sh").arg("-c").arg(&cmd).status();
                            Ok(())
                        });
                        last_interaction = Instant::now();
                        pending_transition = true;
                        need_redraw = true;
                    }
                }
            }
        }

        let gutter_remaining = gutter_interval.saturating_sub(last_plugin_check.elapsed());
        let marquee_remaining = if matches!(active_modal, ActiveModal::None)
            && !shortcut_dispatcher.is_leader_pending()
            && menu_view.has_active_marquee()
        {
            Some(marquee_interval.saturating_sub(last_marquee_tick.elapsed()))
        } else {
            None
        };
        let leader_remaining = shortcut_dispatcher.leader_timeout_remaining();
        let hover_remaining = if matches!(active_modal, ActiveModal::None)
            && !shortcut_dispatcher.is_leader_pending()
        {
            menu_view.hover_timeout_remaining()
        } else {
            None
        };
        let inactivity_remaining = config
            .settings
            .inactivity_timeout
            .as_ref()
            .and_then(|t| t.milliseconds.filter(|&ms| ms > 0))
            .map(|ms| Duration::from_millis(ms).saturating_sub(last_interaction.elapsed()));
        let has_fast_activity = !toast_rack.is_empty()
            || matches!(active_modal, ActiveModal::StreamOutput(_))
            || active_interactive_md.is_some();
        let has_plugins = config.settings.plugins.as_ref().is_some_and(|v| match v {
            serde_json::Value::Array(arr) => !arr.is_empty(),
            serde_json::Value::Object(map) => !map.is_empty(),
            serde_json::Value::Null => false,
            _ => true,
        });
        let poll_timeout = compute_idle_poll_timeout(
            gutter_remaining,
            marquee_remaining,
            leader_remaining,
            hover_remaining,
            inactivity_remaining,
            has_fast_activity,
            has_plugins,
        );

        if pending_action.is_some() || event::poll(poll_timeout)? {
            let ev = if pending_action.is_some() {
                Event::FocusGained
            } else {
                event::read()?
            };
            if matches!(ev, Event::Key(_) | Event::Mouse(_) | Event::FocusGained) {
                last_interaction = Instant::now();
            }
            if let Event::Mouse(m) = &ev {
                if m.kind == crossterm::event::MouseEventKind::Moved {
                    if matches!(active_modal, ActiveModal::None)
                        && !shortcut_dispatcher.is_leader_pending()
                        && menu_view.handle_mouse_move(m.column, m.row, area)
                    {
                        need_redraw = true;
                    }
                    continue;
                }
                menu_view.handle_interaction();
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
                let is_ctrl_g = matches!(active_modal, ActiveModal::None)
                    && !shortcut_dispatcher.is_leader_pending()
                    && matches!(k.code, KeyCode::Char('g') | KeyCode::Char('G'))
                    && k.modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL);
                if !is_ctrl_g {
                    menu_view.handle_interaction();
                }
            }
            need_redraw = true;
            let prev_levels = menu_view.menu_level_titles();
            let prev_scroll = menu_view.current_scroll_offset();
            let prev_in_modal = !matches!(active_modal, ActiveModal::None);
            let prev_modal_rect = active_modal.dialog_rect(area);

            if !prev_in_modal {
                match ev {
                    Event::Key(key) => {
                        if key.code == KeyCode::F(1) || key.code == KeyCode::F(2) {
                            modal_anchor = (area.x + area.width / 2, area.y + area.height / 2);
                        } else {
                            modal_anchor = menu_view.selected_item_anchor(area);
                        }
                    }
                    Event::Mouse(mouse) => {
                        let gutter_border_y = area.y + area.height.saturating_sub(4);
                        if mouse.row >= gutter_border_y {
                            modal_anchor = (area.x + area.width / 2, area.y + area.height / 2);
                        } else {
                            modal_anchor = menu_view.selected_item_anchor(area);
                        }
                    }
                    _ => {}
                }
            }

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
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                            let (resolved_name, resolved_t) =
                                if bashmenu_core::is_random_theme_spec(&config.theme) {
                                    if let Some((name, t)) = pick_random_theme(&themes) {
                                        (name.to_string(), Some(t))
                                    } else {
                                        (config.theme.clone(), None)
                                    }
                                } else {
                                    (config.theme.clone(), themes.get(&config.theme))
                                };
                            active_theme_name = resolved_name;
                            if let Some(t) = resolved_t {
                                (styles, modal_styles, toast_styles) = resolve_bashmenu_styles(
                                    Some(t),
                                    use_nerd,
                                    config.settings.toast_timeout,
                                );
                                menu_view.set_styles(styles.clone());
                                guard.set_default_bg(styles.background);
                                let wp_config = config.resolve_wallpaper(Some(t));
                                if let Some(fb) = guard.framebuffer_mut() {
                                    fb.set_wallpaper(wp_config.as_ref(), styles.background);
                                }
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
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_gutter) = opt_val {
                            config.settings.status_gutter = Some(new_gutter.clone());
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                            let mut ctx = make_macro_ctx!(&config);
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
                            let p = Path::new(&modal.filepath);
                            if let Some(existing_swp) = NoteSwapFile::load(p, &modal.title) {
                                if existing_swp.pid != std::process::id() {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_swap_detected", pid => existing_swp.pid),
                                        Some(t!("common.dialogs.warning_title")),
                                        4.0,
                                    ));
                                }
                            }
                            NoteSwapFile::create(p, &modal.title, &modal.content);
                            active_modal = ActiveModal::NoteEditor(
                                NoteEditorModal::new(&modal.title, &modal.filepath, &modal.content)
                                    .with_cursor(modal.selected_line, 0),
                            );
                        }
                        NoteViewerAction::FullEdit => {
                            let p = Path::new(&modal.filepath);
                            if let Some(existing_swp) = NoteSwapFile::load(p, &modal.title) {
                                if existing_swp.pid != std::process::id() {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_swap_detected", pid => existing_swp.pid),
                                        Some(t!("common.dialogs.warning_title")),
                                        4.0,
                                    ));
                                }
                            }
                            NoteSwapFile::create(p, &modal.title, &modal.content);
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
                                let eff =
                                    TransitionEffect::parse(config.settings.transition.as_deref());
                                if eff != TransitionEffect::None && eff != TransitionEffect::Instant
                                {
                                    let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                        menu_view.render(f.area(), f.buffer_mut());
                                    });
                                }
                                let _ = guard.suspend(|| {
                                    let mut cmd = Command::new(&editor_bin);
                                    cmd.arg(&temp_file);
                                    let _ = cmd.status();
                                    Ok(())
                                });
                                last_interaction = Instant::now();
                                pending_transition = true;

                                if let Ok(new_content) = fs::read_to_string(&temp_file) {
                                    if new_content != original_content {
                                        let disk_content = load_notebook(&modal.filepath)
                                            .ok()
                                            .and_then(|nb| get_note_by_title(&nb, &modal.title))
                                            .map(|n| n.content)
                                            .unwrap_or_default();

                                        if disk_content != original_content {
                                            active_modal =
                                                ActiveModal::NoteConflict(NoteConflictModal::new(
                                                    &modal.title,
                                                    &modal.filepath,
                                                    &new_content,
                                                    &disk_content,
                                                    &original_content,
                                                ));
                                        } else {
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
                                                NoteSwapFile::remove(p, &modal.title);
                                                let sub =
                                                    build_notebook_submenu(&modal.filepath, &nb);
                                                menu_view.reload_current_notebook(
                                                    sub.items,
                                                    Some(modal.title.as_str()),
                                                );
                                            }
                                            modal.content = new_content;
                                            modal.selected_line = 0;
                                            modal.scroll_offset = 0;
                                            toast_rack.add(ToastNotification::new(
                                                t!("bashmenu.notes_saved_from_editor"),
                                                Some(t!("common.dialogs.info_title")),
                                                3.0,
                                            ));
                                        }
                                    } else {
                                        NoteSwapFile::remove(p, &modal.title);
                                    }
                                } else {
                                    NoteSwapFile::remove(p, &modal.title);
                                }
                                let _ = fs::remove_file(&temp_file);
                            } else {
                                NoteSwapFile::remove(p, &modal.title);
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
                        Event::Paste(text) => {
                            modal.paste_text(text.as_str());
                            NoteEditorAction::None
                        }
                        _ => NoteEditorAction::None,
                    };
                    match action {
                        NoteEditorAction::Save(saved_content) => {
                            let p = Path::new(&modal.filepath);
                            let disk_content = load_notebook(modal.filepath.as_str())
                                .ok()
                                .and_then(|nb| get_note_by_title(&nb, &modal.title))
                                .map(|n| n.content)
                                .unwrap_or_default();

                            if !modal.base_content.is_empty() && disk_content != modal.base_content
                            {
                                active_modal = ActiveModal::NoteConflict(NoteConflictModal::new(
                                    &modal.title,
                                    &modal.filepath,
                                    &saved_content,
                                    &disk_content,
                                    &modal.base_content,
                                ));
                            } else {
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
                                    NoteSwapFile::remove(p, &modal.title);
                                    let sub = build_notebook_submenu(modal.filepath.as_str(), &nb);
                                    menu_view.reload_current_notebook(
                                        sub.items,
                                        Some(modal.title.as_str()),
                                    );
                                }
                                active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                    &modal.title,
                                    &modal.filepath,
                                    &saved_content,
                                ));
                            }
                        }
                        NoteEditorAction::Cancel => {
                            let p = Path::new(&modal.filepath);
                            NoteSwapFile::remove(p, &modal.title);
                            active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                &modal.title,
                                &modal.filepath,
                                &modal.lines.join("\n"),
                            ));
                        }
                        NoteEditorAction::MarkdownReference => {
                            let editor_snapshot = modal.clone();
                            active_modal = ActiveModal::NoteMarkdownRef {
                                editor: Box::new(editor_snapshot),
                                help: HelpModal::new(
                                    t!("bashedit.markdown_ref_title"),
                                    &markdown_reference_text(),
                                ),
                            };
                        }
                        NoteEditorAction::None => {}
                    }
                }
                ActiveModal::NoteMarkdownRef { editor, help } => {
                    let dismissed = match ev {
                        Event::Key(key) => help.handle_key(key.code),
                        Event::Mouse(mouse) => help.handle_mouse(mouse, area),
                        _ => false,
                    };
                    if dismissed {
                        let restored = (**editor).clone();
                        active_modal = ActiveModal::NoteEditor(restored);
                    }
                }
                ActiveModal::NewNoteTitle(filepath, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
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
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
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
                ActiveModal::ConfirmInjectBlock {
                    modal,
                    target_path,
                    block_to_inject,
                    start_marker,
                    end_marker,
                    block_id,
                    block_present,
                } => {
                    let choice = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(c) = choice {
                        let target_path_clone = target_path.clone();
                        let block_to_inject_clone = block_to_inject.clone();
                        let start_marker_clone = start_marker.clone();
                        let end_marker_clone = end_marker.clone();
                        let block_id_clone = block_id.clone();
                        let is_present = *block_present;
                        active_modal = ActiveModal::None;
                        match c {
                            ConfirmChoice::Yes => {
                                let target_content = if target_path_clone.exists() {
                                    std::fs::read_to_string(&target_path_clone).unwrap_or_default()
                                } else {
                                    String::new()
                                };

                                let new_content = if is_present {
                                    if let (Some(s_idx), Some(e_idx)) = (
                                        target_content.find(start_marker_clone.as_str()),
                                        target_content.find(end_marker_clone.as_str()),
                                    ) {
                                        let e_end = e_idx + end_marker_clone.len();
                                        format!(
                                            "{}{}{}",
                                            &target_content[..s_idx],
                                            block_to_inject_clone.trim(),
                                            &target_content[e_end..]
                                        )
                                    } else {
                                        target_content
                                    }
                                } else {
                                    let mut s = target_content;
                                    if !s.is_empty() && !s.ends_with('\n') {
                                        s.push('\n');
                                    }
                                    s.push_str(block_to_inject_clone.as_str());
                                    s
                                };

                                if let Some(parent) = target_path_clone.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                match std::fs::write(&target_path_clone, new_content) {
                                    Ok(_) => {
                                        #[cfg(unix)]
                                        if target_path_clone.extension().and_then(|e| e.to_str())
                                            == Some("sh")
                                            || target_path_clone
                                                .to_string_lossy()
                                                .contains("autoexec.sh")
                                        {
                                            use std::os::unix::fs::PermissionsExt;
                                            let _ = std::fs::set_permissions(
                                                &target_path_clone,
                                                std::fs::Permissions::from_mode(0o755),
                                            );
                                        }
                                        active_modal = ActiveModal::Message(MessageModal::new(
                                            "Success",
                                            format!(
                                                "Code block '{}' installed in:\n{}",
                                                block_id_clone,
                                                target_path_clone.display()
                                            ),
                                        ));
                                    }
                                    Err(e) => {
                                        active_modal = ActiveModal::Message(MessageModal::new(
                                            "Write Error",
                                            format!("Failed to write target file:\n{}", e),
                                        ));
                                    }
                                }
                            }
                            ConfirmChoice::No => {
                                if is_present {
                                    let target_content = if target_path_clone.exists() {
                                        std::fs::read_to_string(&target_path_clone)
                                            .unwrap_or_default()
                                    } else {
                                        String::new()
                                    };
                                    if let (Some(s_idx), Some(e_idx)) = (
                                        target_content.find(start_marker_clone.as_str()),
                                        target_content.find(end_marker_clone.as_str()),
                                    ) {
                                        let e_end = e_idx + end_marker_clone.len();
                                        let mut s_cut = s_idx;
                                        if s_cut > 0
                                            && target_content.as_bytes()[s_cut - 1] == b'\n'
                                        {
                                            s_cut -= 1;
                                        }
                                        let mut e_cut = e_end;
                                        if e_cut < target_content.len()
                                            && target_content.as_bytes()[e_cut] == b'\n'
                                        {
                                            e_cut += 1;
                                        }
                                        let new_content = format!(
                                            "{}{}",
                                            &target_content[..s_cut],
                                            &target_content[e_cut..]
                                        );
                                        match std::fs::write(&target_path_clone, new_content) {
                                            Ok(_) => {
                                                active_modal =
                                                    ActiveModal::Message(MessageModal::new(
                                                        "Success",
                                                        format!(
                                                            "Code block '{}' removed from:\n{}",
                                                            block_id_clone,
                                                            target_path_clone.display()
                                                        ),
                                                    ));
                                            }
                                            Err(e) => {
                                                active_modal =
                                                    ActiveModal::Message(MessageModal::new(
                                                        "Write Error",
                                                        format!(
                                                            "Failed to write target file:\n{}",
                                                            e
                                                        ),
                                                    ));
                                            }
                                        }
                                    }
                                }
                            }
                            ConfirmChoice::Cancel => {}
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
                ActiveModal::ItemMessage(item, modal) => {
                    let is_esc = matches!(ev, Event::Key(key) if key.code == KeyCode::Esc);
                    let dismiss = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => false,
                    };
                    if dismiss {
                        let next_item = item.clone();
                        active_modal = ActiveModal::None;
                        if !is_esc && next_item.has_remaining_payload() {
                            pending_action = Some(MenuAction::Execute(next_item));
                        }
                    }
                }
                ActiveModal::RpcMessage(reply_tx, modal) => {
                    let is_esc = matches!(ev, Event::Key(key) if key.code == KeyCode::Esc);
                    let dismiss = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => false,
                    };
                    if dismiss {
                        let _ = reply_tx.send(bashmenu_core::UiRpcResponse {
                            ok: !is_esc,
                            value: String::new(),
                            cancelled: is_esc,
                        });
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::RpcParam(reply_tx, modal) => {
                    let submitted = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        let resp = match opt_val {
                            Some(val) => bashmenu_core::UiRpcResponse {
                                ok: true,
                                value: val,
                                cancelled: false,
                            },
                            None => bashmenu_core::UiRpcResponse {
                                ok: false,
                                value: String::new(),
                                cancelled: true,
                            },
                        };
                        let _ = reply_tx.send(resp);
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::InteractiveForm { modal, .. } => {
                    let action = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(act) = action {
                        if let ActiveModal::InteractiveForm {
                            mut modal,
                            reserved_scripts,
                            frontmatter,
                            fallback_title,
                        } = std::mem::replace(&mut active_modal, ActiveModal::None)
                        {
                            match act {
                                bashmenu_tui::InteractiveFormAction::Continue { action, vars } => {
                                    let script_opt = match action.as_deref() {
                                        Some(id) => match reserved_scripts.get(id).cloned() {
                                            Some(s) => Some(s),
                                            None => {
                                                push_error_toast(
                                                    &mut toast_rack,
                                                    "Interactive Markdown",
                                                    format!("Unknown action block '{id}'"),
                                                    5.0,
                                                );
                                                None
                                            }
                                        },
                                        None => None,
                                    };
                                    if let Some(script) = script_opt {
                                        match ui_rpc::spawn_reserved_script_block(
                                            script,
                                            frontmatter.clone(),
                                            fallback_title.clone(),
                                            ui_rpc::current_bashmenu_exe(),
                                            vars,
                                        ) {
                                            Ok(session) => {
                                                let close_after_action = !modal.has_next_page();
                                                active_interactive_md = Some(session);
                                                suspended_interactive_form =
                                                    Some(SuspendedInteractiveForm {
                                                        modal,
                                                        reserved_scripts,
                                                        frontmatter,
                                                        fallback_title,
                                                        close_after_action,
                                                    });
                                            }
                                            Err(err) => {
                                                active_modal =
                                                    ActiveModal::Message(MessageModal::new(
                                                        "Interactive Markdown Error",
                                                        err.to_string(),
                                                    ));
                                            }
                                        }
                                    } else if modal.advance_page() {
                                        active_modal = ActiveModal::InteractiveForm {
                                            modal,
                                            reserved_scripts,
                                            frontmatter,
                                            fallback_title,
                                        };
                                    }
                                }
                                bashmenu_tui::InteractiveFormAction::Cancel { action, vars } => {
                                    if let Some(id) = action.as_deref() {
                                        if let Some(script) = reserved_scripts.get(id).cloned() {
                                            match ui_rpc::spawn_reserved_script_block(
                                                script,
                                                frontmatter.clone(),
                                                fallback_title.clone(),
                                                ui_rpc::current_bashmenu_exe(),
                                                vars,
                                            ) {
                                                Ok(session) => {
                                                    active_interactive_md = Some(session);
                                                    suspended_interactive_form =
                                                        Some(SuspendedInteractiveForm {
                                                            modal,
                                                            reserved_scripts,
                                                            frontmatter,
                                                            fallback_title,
                                                            close_after_action: true,
                                                        });
                                                }
                                                Err(err) => {
                                                    active_modal =
                                                        ActiveModal::Message(MessageModal::new(
                                                            "Interactive Markdown Error",
                                                            err.to_string(),
                                                        ));
                                                }
                                            }
                                        } else {
                                            push_error_toast(
                                                &mut toast_rack,
                                                "Interactive Markdown",
                                                format!("Unknown action block '{id}'"),
                                                5.0,
                                            );
                                        }
                                    }
                                }
                            }
                        }
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
                            config.set_value(key_path.as_str(), serde_json::Value::Bool(val));
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                            if key_path == "settings.use_nerd_fonts" {
                                let unf = config.settings.use_nerd_fonts.unwrap_or(true)
                                    && !on_linux_console
                                    && !guard.has_framebuffer();
                                use_nerd = unf;
                                menu_view.set_use_nerd_fonts(unf);
                                let resolved_t = themes.get(&active_theme_name);
                                if let Some(t) = resolved_t {
                                    (styles, modal_styles, toast_styles) = resolve_bashmenu_styles(
                                        Some(t),
                                        use_nerd,
                                        config.settings.toast_timeout,
                                    );
                                    menu_view.set_styles(styles.clone());
                                }
                            }
                            if key_path == "settings.status_gutter" {
                                let new_gutter =
                                    config.settings.status_gutter.clone().unwrap_or_default();
                                let mut ctx = make_macro_ctx!(&config);
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
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(new_val) = opt_val {
                            let trimmed = new_val.trim();
                            let parsed: serde_json::Value = if trimmed.eq_ignore_ascii_case("true")
                            {
                                serde_json::Value::Bool(true)
                            } else if trimmed.eq_ignore_ascii_case("false") {
                                serde_json::Value::Bool(false)
                            } else if let Ok(num) = trimmed.parse::<i64>() {
                                serde_json::Value::Number(serde_json::Number::from(num))
                            } else if let Ok(f) = trimmed.parse::<f64>() {
                                serde_json::to_value(f).unwrap_or_else(|_| {
                                    serde_json::Value::String(trimmed.to_string())
                                })
                            } else {
                                serde_json::Value::String(trimmed.to_string())
                            };
                            config.set_value(key_path.as_str(), parsed);
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                            if key_path == "settings.toast_timeout"
                                || key_path == "settings.toast.timeout"
                                || key_path == "app.settings.toast_timeout"
                                || key_path == "app.settings.toast.timeout"
                            {
                                let resolved_t = themes.get(&active_theme_name);
                                if let Some(t) = resolved_t {
                                    (styles, modal_styles, toast_styles) = resolve_bashmenu_styles(
                                        Some(t),
                                        use_nerd,
                                        config.settings.toast_timeout,
                                    );
                                    menu_view.set_styles(styles.clone());
                                }
                            }
                            if key_path == "settings.status_gutter" {
                                let new_gutter =
                                    config.settings.status_gutter.clone().unwrap_or_default();
                                let mut ctx = make_macro_ctx!(&config);
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
                        Event::Paste(text) => {
                            modal.paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(opt_val) = submitted {
                        if let Some(val) = opt_val {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            let mut resolved_item = item.clone();
                            resolved_item.interpolate_template_macros(&ctx);
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
                            config.set_value(
                                key_path.as_str(),
                                serde_json::Value::String(selected.clone()),
                            );
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                            if key_path == "settings.locale"
                                || key_path == "app.settings.locale"
                                || key_path == "locale"
                            {
                                let clean = if selected.eq_ignore_ascii_case("auto") {
                                    "auto"
                                } else {
                                    &selected
                                };
                                let _ = bashmenu_core::set_locale(clean);
                                if let Ok(new_menu) = load_menu(&menu_path) {
                                    menu_view.reload_menu(new_menu);
                                }
                                toast_rack.add(ToastNotification {
                                    title: Some(t!("common.status.saved")),
                                    message: format!("Locale: {}", clean),
                                    severity: ToastSeverity::Information,
                                    timeout_secs: 2.0,
                                });
                            }
                            if key_path == "settings.framebuffer"
                                || key_path == "app.settings.framebuffer"
                            {
                                let fb_m = config.settings.framebuffer.as_deref().unwrap_or("auto");
                                let use_img = cli.image || fb_m == "image";
                                let force_fb_now = std::env::var("BASHMENU_FORCE_FB").as_deref()
                                    == Ok("1")
                                    || (fb_m == "true" && !in_desktop_term);
                                let want_fb = fb_m != "false"
                                    && is_framebuffer_available()
                                    && (force_fb_now || on_linux_console);
                                if want_fb {
                                    if !guard.has_framebuffer() {
                                        if let Some(mut new_fb) = FramebufferDevice::open() {
                                            new_fb.set_image_mode(use_img);
                                            let wp_cfg = config
                                                .resolve_wallpaper(themes.get(&active_theme_name));
                                            new_fb
                                                .set_wallpaper(wp_cfg.as_ref(), styles.background);
                                            guard.set_framebuffer(Some(new_fb));
                                        } else if bashmenu_tui::is_framebuffer_permission_denied() {
                                            toast_rack.add(ToastNotification::new(
                                                t!("bashmenu.toast_fb_perm_msg"),
                                                Some(t!("bashmenu.toast_fb_perm_title")),
                                                5.0,
                                            ));
                                        }
                                    } else if let Some(fb) = guard.framebuffer_mut() {
                                        fb.set_image_mode(use_img);
                                        let wp_cfg = config
                                            .resolve_wallpaper(themes.get(&active_theme_name));
                                        fb.set_wallpaper(wp_cfg.as_ref(), styles.background);
                                    }
                                    menu_view.set_framebuffer_image_mode(
                                        guard.has_framebuffer() && use_img,
                                    );
                                } else {
                                    guard.set_framebuffer(None);
                                    menu_view.set_framebuffer_image_mode(false);
                                    guard.request_clear();
                                    if (fb_m == "true" || fb_m == "image") && in_desktop_term {
                                        toast_rack.add(ToastNotification::new(
                                            t!("bashmenu.toast_fb_desktop_guard_msg"),
                                            Some(t!("bashmenu.toast_fb_desktop_guard_title")),
                                            4.5,
                                        ));
                                    }
                                }
                            }
                        }
                        active_modal = ActiveModal::None;
                    }
                }
                ActiveModal::ItemFilePicker(item, is_dir, modal) => {
                    let chosen = match ev {
                        Event::Key(key) => modal.handle_key_with_modifiers(key.code, key.modifiers),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        Event::Paste(text) => {
                            modal.handle_paste(text.as_str());
                            None
                        }
                        _ => None,
                    };
                    if let Some(opt_path) = chosen {
                        if let Some(target_path) = opt_path {
                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            let mut resolved_item = item.clone();
                            resolved_item.interpolate_template_macros(&ctx);
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
                        Event::Paste(text) => {
                            modal.handle_paste(&text);
                            StreamOutputAction::None
                        }
                        Event::Resize(w, h) => {
                            modal.resize_for_area(ratatui::layout::Rect::new(0, 0, w, h));
                            StreamOutputAction::None
                        }
                        _ => StreamOutputAction::None,
                    };
                    if action == StreamOutputAction::Close {
                        active_modal = ActiveModal::None;
                        last_interaction = Instant::now();
                    }
                }
                ActiveModal::NoteConflict(modal) => {
                    let choice = match ev {
                        Event::Key(key) => modal.handle_key(key.code),
                        Event::Mouse(mouse) => modal.handle_mouse(mouse, area),
                        _ => None,
                    };
                    if let Some(ch) = choice {
                        match ch {
                            NoteConflictChoice::Merge => {
                                let merge_res = merge_note_content(
                                    &modal.base_content,
                                    &modal.disk_content,
                                    &modal.local_content,
                                );
                                let mut ed = NoteEditorModal::new(
                                    &modal.note_title,
                                    &modal.filepath,
                                    &merge_res.content,
                                );
                                ed.is_modified = true;
                                if merge_res.has_conflicts {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_merged_conflicts"),
                                        Some(t!("common.dialogs.warning_title")),
                                        4.0,
                                    ));
                                } else {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_merged_clean"),
                                        Some(t!("common.dialogs.info_title")),
                                        3.0,
                                    ));
                                }
                                active_modal = ActiveModal::NoteEditor(ed);
                            }
                            NoteConflictChoice::Overwrite => {
                                let mut saved_ok = false;
                                if let Ok(mut nb) = load_notebook(&modal.filepath) {
                                    add_or_update_note(
                                        &mut nb,
                                        NoteItem {
                                            title: modal.note_title.clone(),
                                            content: modal.local_content.clone(),
                                            ..Default::default()
                                        },
                                    );
                                    match save_notebook(&modal.filepath, &nb) {
                                        Ok(()) => {
                                            saved_ok = true;
                                            NoteSwapFile::remove(
                                                Path::new(&modal.filepath),
                                                &modal.note_title,
                                            );
                                            let sub = build_notebook_submenu(&modal.filepath, &nb);
                                            menu_view.reload_current_notebook(
                                                sub.items,
                                                Some(modal.note_title.as_str()),
                                            );
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some(t!("common.dialogs.error_title")),
                                                message: format!("Failed to save notebook: {}", e),
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 4.0,
                                            });
                                        }
                                    }
                                }
                                if saved_ok {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_overwritten"),
                                        Some(t!("common.dialogs.info_title")),
                                        3.0,
                                    ));
                                }
                                active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                    &modal.note_title,
                                    &modal.filepath,
                                    &modal.local_content,
                                ));
                            }
                            NoteConflictChoice::Reload => {
                                NoteSwapFile::remove(Path::new(&modal.filepath), &modal.note_title);
                                toast_rack.add(ToastNotification::new(
                                    t!("bashmenu.notes_reloaded"),
                                    Some(t!("common.dialogs.info_title")),
                                    3.0,
                                ));
                                active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                    &modal.note_title,
                                    &modal.filepath,
                                    &modal.disk_content,
                                ));
                            }
                            NoteConflictChoice::SaveAsCopy => {
                                let copy_title = format!("{} (User Copy)", modal.note_title);
                                let mut saved_ok = false;
                                if let Ok(mut nb) = load_notebook(&modal.filepath) {
                                    add_or_update_note(
                                        &mut nb,
                                        NoteItem {
                                            title: copy_title.clone(),
                                            content: modal.local_content.clone(),
                                            ..Default::default()
                                        },
                                    );
                                    match save_notebook(&modal.filepath, &nb) {
                                        Ok(()) => {
                                            saved_ok = true;
                                            NoteSwapFile::remove(
                                                Path::new(&modal.filepath),
                                                &modal.note_title,
                                            );
                                            let sub = build_notebook_submenu(&modal.filepath, &nb);
                                            menu_view.reload_current_notebook(
                                                sub.items,
                                                Some(copy_title.as_str()),
                                            );
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some(t!("common.dialogs.error_title")),
                                                message: format!("Failed to save notebook: {}", e),
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 4.0,
                                            });
                                        }
                                    }
                                }
                                if saved_ok {
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.notes_saved_copy", title => &copy_title),
                                        Some(t!("common.dialogs.info_title")),
                                        3.0,
                                    ));
                                }
                                active_modal = ActiveModal::NoteViewer(NoteViewerModal::new(
                                    &copy_title,
                                    &modal.filepath,
                                    &modal.local_content,
                                ));
                            }
                            NoteConflictChoice::Cancel => {
                                let mut ed = NoteEditorModal::new(
                                    &modal.note_title,
                                    &modal.filepath,
                                    &modal.local_content,
                                );
                                ed.is_modified = true;
                                active_modal = ActiveModal::NoteEditor(ed);
                            }
                        }
                        need_redraw = true;
                    }
                }
                ActiveModal::None => {
                    let action = if let Some(act) = pending_action.take() {
                        act
                    } else {
                        match ev {
                            Event::Key(key) => match shortcut_dispatcher.handle_key(key) {
                                ShortcutDispatchResult::PassThrough => {
                                    let will_transition = match key.code {
                                        KeyCode::Enter => {
                                            if let Some(item) = menu_view.selected_item() {
                                                item.r#type == MenuItemType::Submenu
                                                    || item.submenu.is_some()
                                                    || item.r#type == MenuItemType::Notebook
                                            } else {
                                                false
                                            }
                                        }
                                        KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => {
                                            menu_view.stack_depth() > 1
                                        }
                                        KeyCode::PageUp | KeyCode::PageDown => true,
                                        _ => false,
                                    };
                                    if will_transition {
                                        let eff = TransitionEffect::parse(
                                            config.settings.transition.as_deref(),
                                        );
                                        if eff != TransitionEffect::None
                                            && eff != TransitionEffect::Instant
                                        {
                                            let _ = guard.draw_with_transition_out(
                                                eff,
                                                trans_speed,
                                                |f| {
                                                    menu_view.render(f.area(), f.buffer_mut());
                                                },
                                            );
                                        }
                                    }
                                    if key.code == KeyCode::Char('c')
                                        && key
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::CONTROL)
                                    {
                                        MenuAction::Exit
                                    } else if !menu_view.is_current_notebook()
                                        && ((key.code == KeyCode::Enter
                                            && shortcut_dispatcher.has_active_builtin("{select}"))
                                            || (matches!(
                                                key.code,
                                                KeyCode::Esc | KeyCode::Backspace
                                            ) && shortcut_dispatcher
                                                .has_active_builtin("{back}")))
                                    {
                                        MenuAction::None
                                    } else {
                                        menu_view.handle_key(key.code)
                                    }
                                }
                                ShortcutDispatchResult::LeaderActivated
                                | ShortcutDispatchResult::LeaderCancelled => MenuAction::None,
                                ShortcutDispatchResult::LeaderUnbound(unbound) => {
                                    let unbound_str = unbound.to_string();
                                    toast_rack.add(ToastNotification::new(
                                        t!("bashmenu.toast_unbound_leader_msg", key => &unbound_str),
                                        Some(t!("bashmenu.toast_unbound_leader_title")),
                                        2.0,
                                    ));
                                    MenuAction::None
                                }
                                ShortcutDispatchResult::Matched(entry) => {
                                    if let Some(m) = entry.builtin_macro() {
                                        let will_transition = match m {
                                            "{select}" => {
                                                if let Some(item) = menu_view.selected_item() {
                                                    item.r#type == MenuItemType::Submenu
                                                        || item.submenu.is_some()
                                                        || item.r#type == MenuItemType::Notebook
                                                } else {
                                                    false
                                                }
                                            }
                                            "{back}" => menu_view.stack_depth() > 1,
                                            _ => false,
                                        };
                                        if will_transition {
                                            let eff = TransitionEffect::parse(
                                                config.settings.transition.as_deref(),
                                            );
                                            if eff != TransitionEffect::None
                                                && eff != TransitionEffect::Instant
                                            {
                                                let _ = guard.draw_with_transition_out(
                                                    eff,
                                                    trans_speed,
                                                    |f| {
                                                        menu_view.render(f.area(), f.buffer_mut());
                                                    },
                                                );
                                            }
                                        }
                                        menu_view
                                            .action_for_builtin_macro(m)
                                            .unwrap_or(MenuAction::None)
                                    } else {
                                        let item = entry.to_menu_item();
                                        if item.r#type == MenuItemType::Notebook {
                                            MenuAction::OpenNotebook(Box::new(item))
                                        } else {
                                            MenuAction::Execute(Box::new(item))
                                        }
                                    }
                                }
                            },
                            Event::Mouse(mouse) => {
                                if shortcut_dispatcher.is_leader_pending() {
                                    match shortcut_dispatcher.handle_mouse(mouse, area) {
                                        ShortcutDispatchResult::Matched(entry) => {
                                            if let Some(m) = entry.builtin_macro() {
                                                menu_view
                                                    .action_for_builtin_macro(m)
                                                    .unwrap_or(MenuAction::None)
                                            } else {
                                                let item = entry.to_menu_item();
                                                if item.r#type == MenuItemType::Notebook {
                                                    MenuAction::OpenNotebook(Box::new(item))
                                                } else {
                                                    MenuAction::Execute(Box::new(item))
                                                }
                                            }
                                        }
                                        _ => MenuAction::None,
                                    }
                                } else {
                                    let prev_levels = menu_view.menu_level_titles();
                                    let act = menu_view.handle_mouse(mouse, area);
                                    let curr_levels = menu_view.menu_level_titles();
                                    if prev_levels != curr_levels {
                                        let eff = TransitionEffect::parse(
                                            config.settings.transition.as_deref(),
                                        );
                                        if eff != TransitionEffect::None
                                            && eff != TransitionEffect::Instant
                                        {
                                            let _ = guard.draw_with_transition_out(
                                                eff,
                                                trans_speed,
                                                |f| {
                                                    menu_view.render(f.area(), f.buffer_mut());
                                                },
                                            );
                                        }
                                    }
                                    act
                                }
                            }
                            Event::Resize(_, _) => MenuAction::None,
                            _ => MenuAction::None,
                        }
                    };

                    match action {
                        MenuAction::Help => {
                            let ctx = make_macro_ctx!(&config);
                            let asset_cand = ctx.bashmenu_dir.join("assets/bashmenu.md");
                            let cand = ctx.bashmenu_dir.join("bashmenu.md");
                            let doc_path = if asset_cand.exists() {
                                asset_cand
                            } else if cand.exists() {
                                cand
                            } else {
                                let dev_cand = ctx.dev_dir.join("assets/bashmenu.md");
                                if dev_cand.exists() {
                                    dev_cand
                                } else {
                                    resolve_file_path(Path::new("assets/bashmenu.md"), cli.local)
                                }
                            };
                            if doc_path.exists() {
                                let pager_bin = locate_tool("bmpager");
                                let eff =
                                    TransitionEffect::parse(config.settings.transition.as_deref());
                                if eff != TransitionEffect::None && eff != TransitionEffect::Instant
                                {
                                    let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                        menu_view.render(f.area(), f.buffer_mut());
                                    });
                                }
                                let _ = guard.suspend_keep_alt(|| {
                                    let _ = Command::new(&pager_bin)
                                        .arg("--markdown")
                                        .arg(&doc_path)
                                        .status();
                                    Ok(())
                                });
                                last_interaction = Instant::now();
                                pending_transition = true;
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
                            active_modal = ActiveModal::ThemePicker(
                                ThemePickerModal::new(theme_names, &config.theme)
                                    .with_random_option(),
                            );
                        }
                        MenuAction::LaunchEditor => {
                            let editor_bin = locate_tool("bashedit");
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
                            let _ = guard.suspend_keep_alt(|| {
                                let _ = Command::new(&editor_bin).status();
                                Ok(())
                            });
                            last_interaction = Instant::now();
                            pending_transition = true;
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
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
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
                            last_interaction = Instant::now();
                            pending_transition = true;
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::Reload => {
                            last_interaction = Instant::now();
                            clear_command_cache();

                            match load_config(&config_path) {
                                Ok(new_config) => {
                                    config = new_config;
                                }
                                Err(e) => {
                                    let title = t!("bashmenu.toast_config_reload_title");
                                    let err_str = e.to_string();
                                    let msg =
                                        t!("bashmenu.toast_config_reload_msg", err => &err_str);
                                    push_error_toast(&mut toast_rack, &title, msg, 8.0);
                                }
                            }
                            use_nerd = config.settings.use_nerd_fonts.unwrap_or(true)
                                && !on_linux_console
                                && !guard.has_framebuffer();
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
                                    menu_view.set_shortcuts_config(&new_shortcuts);
                                    shortcut_dispatcher.set_config(new_shortcuts);
                                }
                            }

                            if let Ok(new_themes) = load_themes(&themes_path) {
                                themes = new_themes;
                            }
                            let (resolved_name, active_theme) =
                                if bashmenu_core::is_random_theme_spec(&config.theme) {
                                    if let Some((name, t)) = pick_random_theme(&themes) {
                                        (name.to_string(), Some(t))
                                    } else {
                                        (config.theme.clone(), None)
                                    }
                                } else {
                                    (config.theme.clone(), themes.get(&config.theme))
                                };
                            active_theme_name = resolved_name;
                            (styles, modal_styles, toast_styles) = resolve_bashmenu_styles(
                                active_theme,
                                use_nerd,
                                config.settings.toast_timeout,
                            );
                            menu_view.set_styles(styles.clone());
                            guard.set_default_bg(styles.background);

                            let wp_config = config.resolve_wallpaper(active_theme);
                            if let Some(fb) = guard.framebuffer_mut() {
                                fb.set_wallpaper(wp_config.as_ref(), styles.background);
                            }
                            menu_view.set_wallpaper(wp_config);

                            let mut ctx = make_macro_ctx!(&config);
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

                            if !pending_transition {
                                guard.request_clear();
                            }
                            need_redraw = true;
                        }
                        MenuAction::ToggleShortcuts => {
                            let show = menu_view.toggle_shortcuts();
                            config.settings.show_menu_shortcuts = Some(show);
                            save_config_or_toast(&config_path, &config, &mut toast_rack);
                            menu_view.set_config(config.clone());
                        }
                        MenuAction::ConfigureGutter => {
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
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
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
                            let _ = guard.suspend_keep_alt(|| {
                                let _ = Command::new(&themeedit_bin)
                                    .arg("-f")
                                    .arg(&themes_path)
                                    .arg("-t")
                                    .arg(&active_theme_name)
                                    .status();
                                Ok(())
                            });
                            last_interaction = Instant::now();
                            pending_transition = true;
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::LaunchShortcutEditor => {
                            let shortcutedit_bin = locate_tool("shortcutedit");
                            let menuedit_bin = locate_tool("menuedit");
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
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
                            last_interaction = Instant::now();
                            pending_transition = true;
                            pending_action = Some(MenuAction::Reload);
                        }
                        MenuAction::OpenNotebook(item) => {
                            let eff =
                                TransitionEffect::parse(config.settings.transition.as_deref());
                            if eff != TransitionEffect::None && eff != TransitionEffect::Instant {
                                let _ = guard.draw_with_transition_out(eff, trans_speed, |f| {
                                    menu_view.render(f.area(), f.buffer_mut());
                                });
                            }
                            let mut ctx = make_macro_ctx!(&config);
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
                            let title = bashmenu_core::t!("bashmenu.new_note_title");
                            let prompt = bashmenu_core::t!("bashmenu.new_note_prompt");
                            active_modal = ActiveModal::NewNoteTitle(
                                filepath,
                                InputModal::new(&title, &prompt, ""),
                            );
                        }
                        MenuAction::RenameNote(old_title) => {
                            if let Some(filepath) = menu_view.current_notebook_file() {
                                let title = bashmenu_core::t!("bashmenu.rename_note_title");
                                let prompt = bashmenu_core::t!("bashmenu.rename_note_prompt");
                                active_modal = ActiveModal::RenameNoteTitle(
                                    filepath.to_string(),
                                    old_title.clone(),
                                    InputModal::new(&title, &prompt, &old_title),
                                );
                            }
                        }
                        MenuAction::DeleteNote(note_title) => {
                            if let Some(filepath) = menu_view.current_notebook_file() {
                                let title = bashmenu_core::t!("bashmenu.delete_note_title");
                                let msg = bashmenu_core::t!(
                                    "bashmenu.delete_note_confirm",
                                    title => &note_title
                                );
                                active_modal = ActiveModal::ConfirmDeleteNote(
                                    filepath.to_string(),
                                    note_title.clone(),
                                    ConfirmModal::new(&title, msg),
                                );
                            }
                        }
                        MenuAction::ToggleHelpGutter => {
                            menu_view.toggle_help_gutter(area);
                        }
                        MenuAction::ActivateLeader => {
                            shortcut_dispatcher.activate_leader();
                        }
                        MenuAction::Execute(mut item) => {
                            let builtin_target = item
                                .action
                                .as_deref()
                                .or(item.command.as_deref())
                                .or(item.script.as_deref())
                                .map(str::trim)
                                .filter(|s| bashmenu_core::builtin_action_locale_key(s).is_some());
                            if let Some(m) = builtin_target {
                                pending_action = menu_view.action_for_builtin_macro(m);
                                continue;
                            }

                            let mut ctx = make_macro_ctx!(&config);
                            ctx.window_width = size.width as usize;
                            ctx.window_height = size.height as usize;
                            ctx.screen_width = size.width as usize;

                            let has_env = item
                                .action
                                .as_deref()
                                .map(|s| s.contains("{env}") || s.contains("{environment}"))
                                .unwrap_or(false)
                                || item
                                    .command
                                    .as_deref()
                                    .map(|s| s.contains("{env}") || s.contains("{environment}"))
                                    .unwrap_or(false)
                                || item
                                    .script
                                    .as_deref()
                                    .map(|s| s.contains("{env}") || s.contains("{environment}"))
                                    .unwrap_or(false)
                                || item
                                    .message()
                                    .map(|s| s.contains("{env}") || s.contains("{environment}"))
                                    .unwrap_or(false)
                                || item
                                    .prompt
                                    .as_deref()
                                    .map(|s| s.contains("{env}") || s.contains("{environment}"))
                                    .unwrap_or(false)
                                || matches!(item.r#type, MenuItemType::Other(ref s) if s == "env" || s == "environment");

                            let info_macro_path = item
                                .action
                                .as_deref()
                                .and_then(bashmenu_core::extract_info_macro_path)
                                .or_else(|| {
                                    item.command
                                        .as_deref()
                                        .and_then(bashmenu_core::extract_info_macro_path)
                                })
                                .or_else(|| {
                                    item.script
                                        .as_deref()
                                        .and_then(bashmenu_core::extract_info_macro_path)
                                })
                                .or_else(|| {
                                    item.message()
                                        .and_then(bashmenu_core::extract_info_macro_path)
                                })
                                .or_else(|| {
                                    item.prompt
                                        .as_deref()
                                        .and_then(bashmenu_core::extract_info_macro_path)
                                });

                            if has_env {
                                let (fm, env_info) = ctx.render_env_about_full();
                                let env_title = fm
                                    .title
                                    .unwrap_or_else(|| "Current Bashmenu Environment".to_string());
                                active_modal = ActiveModal::Message(
                                    MessageModal::new(env_title, env_info)
                                        .with_dimensions(fm.width, fm.height),
                                );
                            } else if let Some(info_path) = info_macro_path {
                                let resolved_info = ctx.resolve_info_file_path(&info_path);
                                let interactive_launch = if ctx.allow_command_macros {
                                    std::fs::read_to_string(&resolved_info)
                                        .ok()
                                        .and_then(|raw| {
                                            let (fm, body) =
                                                bashmenu_core::parse_markdown_frontmatter_full(
                                                    &raw,
                                                );
                                            if fm.interactive {
                                                let fallback_title = resolved_info
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or("Info")
                                                    .to_string();
                                                Some((fm, fallback_title, body.to_string()))
                                            } else {
                                                None
                                            }
                                        })
                                } else {
                                    None
                                };
                                if let Some((fm, fallback_title, body)) = interactive_launch {
                                    if bashmenu_core::has_interactive_markdown_form(&body) {
                                        let safe_ctx = ctx.clone().with_allow_command_macros(false);
                                        let form_title = fm
                                            .title
                                            .as_deref()
                                            .map(|t| interpolate_placeholders(t, &safe_ctx))
                                            .unwrap_or_else(|| fallback_title.clone());
                                        let doc =
                                            bashmenu_core::parse_interactive_markdown_doc(&body);
                                        let interpolated_pages: Vec<
                                            Vec<bashmenu_core::MarkdownFormBlock>,
                                        > = doc
                                            .pages
                                            .into_iter()
                                            .map(|page| {
                                                page.into_iter()
                                                    .map(|b| match b {
                                                        bashmenu_core::MarkdownFormBlock::Text(
                                                            t,
                                                        ) => {
                                                            bashmenu_core::MarkdownFormBlock::Text(
                                                                interpolate_placeholders(
                                                                    &t, &safe_ctx,
                                                                ),
                                                            )
                                                        }
                                                        bashmenu_core::MarkdownFormBlock::Input {
                                                            prefix,
                                                            mut spec,
                                                        } => {
                                                            let prefix = prefix.map(|p| {
                                                                interpolate_placeholders(
                                                                    &p, &safe_ctx,
                                                                )
                                                            });
                                                            if spec.default_value.contains('{') {
                                                                spec.default_value =
                                                                    interpolate_placeholders(
                                                                        &spec.default_value,
                                                                        &safe_ctx,
                                                                    );
                                                            }
                                                            bashmenu_core::MarkdownFormBlock::Input {
                                                                prefix,
                                                                spec,
                                                            }
                                                        }
                                                        bashmenu_core::MarkdownFormBlock::Buttons(
                                                            mut btns,
                                                        ) => {
                                                            for btn in &mut btns {
                                                                if let Some(ref mut lbl) = btn.label
                                                                {
                                                                    if lbl.contains('{') {
                                                                        *lbl =
                                                                            interpolate_placeholders(
                                                                                lbl, &safe_ctx,
                                                                            );
                                                                    }
                                                                }
                                                            }
                                                            bashmenu_core::MarkdownFormBlock::Buttons(
                                                                btns,
                                                            )
                                                        }
                                                    })
                                                    .collect()
                                            })
                                            .collect();
                                        let form_modal =
                                            bashmenu_tui::InteractiveMarkdownModal::from_pages(
                                                form_title,
                                                interpolated_pages,
                                            )
                                            .with_dimensions(fm.width, fm.height);
                                        active_modal = ActiveModal::InteractiveForm {
                                            modal: Box::new(form_modal),
                                            reserved_scripts: doc.reserved_scripts,
                                            frontmatter: fm,
                                            fallback_title,
                                        };
                                    } else {
                                        match ui_rpc::spawn_interactive_markdown(
                                            fm,
                                            fallback_title,
                                            body,
                                            ui_rpc::current_bashmenu_exe(),
                                            std::collections::HashMap::new(),
                                        ) {
                                            Ok(session) => {
                                                active_interactive_md = Some(session);
                                            }
                                            Err(err) => {
                                                active_modal =
                                                    ActiveModal::Message(MessageModal::new(
                                                        "Interactive Markdown Error",
                                                        err.to_string(),
                                                    ));
                                            }
                                        }
                                    }
                                } else {
                                    match ctx.render_info_file(&info_path) {
                                        Ok((fm, body)) => {
                                            let title =
                                                fm.title.unwrap_or_else(|| "Info".to_string());
                                            active_modal = ActiveModal::Message(
                                                MessageModal::new(title, body)
                                                    .with_dimensions(fm.width, fm.height),
                                            );
                                        }
                                        Err(err) => {
                                            active_modal = ActiveModal::Message(MessageModal::new(
                                                "Info Error",
                                                err,
                                            ));
                                        }
                                    }
                                }
                            } else if item.r#type == MenuItemType::Toggle
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
                                    let mut options = item.options();
                                    if key_path == "settings.locale"
                                        || key_path == "app.settings.locale"
                                        || key_path == "locale"
                                    {
                                        options = build_locale_radio_options();
                                    }
                                    let curr_val = config
                                        .get_value(key_path)
                                        .unwrap_or_else(|| "auto".to_string());
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
                                active_modal = ActiveModal::ThemePicker(
                                    ThemePickerModal::new(theme_names, &config.theme)
                                        .with_random_option(),
                                );
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
                            } else if matches!(
                                item.r#type,
                                MenuItemType::Other(ref s)
                                    if s == "message" || s == "popup" || s == "info"
                            ) {
                                let raw_title = item.title().unwrap_or("Message");
                                let raw_msg = item
                                    .message()
                                    .or(item.prompt.as_deref())
                                    .or(item.action.as_deref())
                                    .or(item.command.as_deref())
                                    .or(item.script.as_deref())
                                    .unwrap_or_default();
                                let title = interpolate_placeholders(raw_title, &ctx);
                                let msg = interpolate_placeholders(raw_msg, &ctx);
                                active_modal =
                                    ActiveModal::Message(MessageModal::new(&title, &msg));
                            } else if matches!(
                                item.r#type,
                                MenuItemType::Other(ref s) if s == "inject_block"
                            ) {
                                let target_raw = item
                                    .extra
                                    .get("target")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let template_raw = item
                                    .extra
                                    .get("template")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let block_id_raw = item
                                    .extra
                                    .get("block_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("default");
                                let target_str = interpolate_placeholders(target_raw, &ctx);
                                let template_str = interpolate_placeholders(template_raw, &ctx);
                                let block_id = interpolate_placeholders(block_id_raw, &ctx);

                                if target_str.is_empty() || template_str.is_empty() {
                                    active_modal = ActiveModal::Message(MessageModal::new(
                                        "Configuration Error",
                                        "Both 'target' and 'template' paths must be specified.",
                                    ));
                                } else {
                                    let target_path =
                                        if let Some(stripped) = target_str.strip_prefix("~/") {
                                            ctx.home_dir.join(stripped)
                                        } else if target_str == "~" {
                                            ctx.home_dir.clone()
                                        } else {
                                            PathBuf::from(&target_str)
                                        };

                                    let template_path =
                                        if let Some(stripped) = template_str.strip_prefix("~/") {
                                            ctx.home_dir.join(stripped)
                                        } else if Path::new(&template_str).is_absolute() {
                                            PathBuf::from(&template_str)
                                        } else {
                                            ctx.bashmenu_dir.join(&template_str)
                                        };

                                    if !template_path.exists() {
                                        active_modal = ActiveModal::Message(MessageModal::new(
                                            "Template Error",
                                            format!(
                                                "Template file not found:\n{}",
                                                template_path.display()
                                            ),
                                        ));
                                    } else {
                                        match std::fs::read_to_string(&template_path) {
                                            Ok(tmpl_content) => {
                                                let interpolated =
                                                    interpolate_placeholders(&tmpl_content, &ctx);
                                                let start_marker =
                                                    format!("# CODEBLOCK:{}:START", block_id);
                                                let end_marker =
                                                    format!("# CODEBLOCK:{}:END", block_id);
                                                let block_to_inject = format!(
                                                    "\n{}\n{}\n{}\n",
                                                    start_marker,
                                                    interpolated.trim(),
                                                    end_marker
                                                );

                                                let target_content = if target_path.exists() {
                                                    std::fs::read_to_string(&target_path)
                                                        .unwrap_or_default()
                                                } else {
                                                    String::new()
                                                };
                                                let block_present = target_content
                                                    .contains(&start_marker)
                                                    && target_content.contains(&end_marker);

                                                let (title, msg) = if block_present {
                                                    (
                                                        "Update or Uninstall Block",
                                                        format!(
                                                            "Code block '{}' is already present in:\n{}\n\n- Press YES to reinstall/update.\n- Press NO to uninstall/remove.\n- Press [C / ESC] Cancel to abort.",
                                                            block_id,
                                                            target_path.display()
                                                        ),
                                                    )
                                                } else {
                                                    (
                                                        "Install Code Block",
                                                        format!(
                                                            "Would you like to inject code block '{}' into:\n{}?",
                                                            block_id,
                                                            target_path.display()
                                                        ),
                                                    )
                                                };

                                                active_modal = ActiveModal::ConfirmInjectBlock {
                                                    modal: ConfirmModal::new(title, msg),
                                                    target_path,
                                                    block_to_inject,
                                                    start_marker,
                                                    end_marker,
                                                    block_id,
                                                    block_present,
                                                };
                                            }
                                            Err(e) => {
                                                active_modal =
                                                    ActiveModal::Message(MessageModal::new(
                                                        "Read Error",
                                                        format!("Failed to read template:\n{}", e),
                                                    ));
                                            }
                                        }
                                    }
                                }
                            } else if let Some(msg_data) = item.take_first_message_macro(&ctx) {
                                let title = msg_data
                                    .title
                                    .or_else(|| {
                                        item.extra
                                            .get("title")
                                            .and_then(|v| v.as_str())
                                            .filter(|s| !s.trim().is_empty())
                                            .map(|t| interpolate_placeholders(t, &ctx))
                                    })
                                    .unwrap_or_default();
                                active_modal = ActiveModal::ItemMessage(
                                    item,
                                    MessageModal::new(title, msg_data.text),
                                );
                            } else if item.has_param_placeholder() {
                                let raw_title = item.param_modal_title();
                                let title = if raw_title.is_empty() {
                                    String::new()
                                } else {
                                    interpolate_placeholders(&raw_title, &ctx)
                                };
                                let raw_prompt = item.param_modal_prompt();
                                let prompt = if raw_prompt.is_empty() {
                                    String::new()
                                } else {
                                    interpolate_placeholders(&raw_prompt, &ctx)
                                };
                                let raw_default = item.param_modal_default();
                                let default_val = if raw_default.is_empty() {
                                    String::new()
                                } else {
                                    interpolate_placeholders(&raw_default, &ctx)
                                };
                                let modal = InputModal::new(title, prompt, &default_val)
                                    .with_masked(item.masked());
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
                                let target = if item.macros_interpolated {
                                    target_raw.to_string()
                                } else {
                                    interpolate_placeholders(target_raw, &ctx)
                                };
                                let editor_bin = locate_tool("bashedit");
                                let trans_enabled = item.transition_enabled();
                                if trans_enabled {
                                    let eff = TransitionEffect::parse(
                                        config.settings.transition.as_deref(),
                                    );
                                    if eff != TransitionEffect::None
                                        && eff != TransitionEffect::Instant
                                    {
                                        let _ =
                                            guard.draw_with_transition_out(eff, trans_speed, |f| {
                                                menu_view.render(f.area(), f.buffer_mut());
                                            });
                                    }
                                }
                                let _ = guard.suspend_keep_alt(|| {
                                    let mut cmd = Command::new(&editor_bin);
                                    let trimmed = target.trim();
                                    if !trimmed.is_empty() {
                                        cmd.arg(trimmed);
                                    }
                                    if let Some(ts) = item.tabstop {
                                        cmd.arg("--tabstop").arg(ts.to_string());
                                    }
                                    if let Some(sw) =
                                        item.extra.get("show_whitespace").and_then(|v| v.as_bool())
                                    {
                                        if sw {
                                            cmd.arg("--show-whitespace");
                                        }
                                    }
                                    if let Some(tts) =
                                        item.extra.get("tab_to_spaces").and_then(|v| v.as_bool())
                                    {
                                        cmd.arg(format!("--tab-to-spaces={tts}"));
                                    }
                                    let _ = cmd.status();
                                    Ok(())
                                });
                                last_interaction = Instant::now();
                                if trans_enabled {
                                    pending_transition = true;
                                }
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
                                    let trans_enabled = item.transition_enabled();
                                    if trans_enabled {
                                        let eff = TransitionEffect::parse(
                                            config.settings.transition.as_deref(),
                                        );
                                        if eff != TransitionEffect::None
                                            && eff != TransitionEffect::Instant
                                        {
                                            let _ = guard.draw_with_transition_out(
                                                eff,
                                                trans_speed,
                                                |f| {
                                                    menu_view.render(f.area(), f.buffer_mut());
                                                },
                                            );
                                        }
                                    }
                                    let _ = guard.suspend_keep_alt(|| {
                                        let _ = Command::new(&themeedit_bin)
                                            .arg("-f")
                                            .arg(&themes_path)
                                            .arg("-t")
                                            .arg(&active_theme_name)
                                            .status();
                                        Ok(())
                                    });
                                    last_interaction = Instant::now();
                                    if trans_enabled {
                                        pending_transition = true;
                                    }
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
                                    let trans_enabled = item.transition_enabled();
                                    if trans_enabled {
                                        let eff = TransitionEffect::parse(
                                            config.settings.transition.as_deref(),
                                        );
                                        if eff != TransitionEffect::None
                                            && eff != TransitionEffect::Instant
                                        {
                                            let _ = guard.draw_with_transition_out(
                                                eff,
                                                trans_speed,
                                                |f| {
                                                    menu_view.render(f.area(), f.buffer_mut());
                                                },
                                            );
                                        }
                                    }
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
                                    last_interaction = Instant::now();
                                    pending_transition = true;
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
                                        .unwrap_or(styles.toast_timeout);
                                    let raw_msg = item
                                        .message()
                                        .or(item.action.as_deref())
                                        .or(item.script.as_deref())
                                        .or(item.command.as_deref())
                                        .unwrap_or_default();
                                    let (clean, toasts) = if item.macros_interpolated {
                                        (raw_msg.to_string(), item.pending_toasts.clone())
                                    } else {
                                        extract_toasts(raw_msg, &ctx)
                                    };
                                    for t in toasts {
                                        toast_rack.add(t);
                                    }
                                    if !clean.trim().is_empty() {
                                        let title =
                                            item.title().map(|t| interpolate_placeholders(t, &ctx));
                                        let interp_msg = if item.macros_interpolated {
                                            clean
                                        } else {
                                            interpolate_placeholders(&clean, &ctx)
                                        };
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
                                        let (clean_cmd, toasts) = if item.macros_interpolated {
                                            (raw_cmd.to_string(), item.pending_toasts.clone())
                                        } else {
                                            extract_toasts(raw_cmd, &ctx)
                                        };
                                        for t in toasts {
                                            toast_rack.add(t);
                                            need_redraw = true;
                                        }
                                        let final_cmd = if item.macros_interpolated {
                                            clean_cmd.clone()
                                        } else {
                                            interpolate_placeholders(&clean_cmd, &ctx)
                                        };
                                        if !final_cmd.trim().is_empty() {
                                            let requires_root = item
                                                .extra
                                                .get("user_mode")
                                                .and_then(|v| v.as_str())
                                                == Some("root");
                                            #[cfg(unix)]
                                            let is_root = unsafe { libc::geteuid() } == 0;
                                            #[cfg(not(unix))]
                                            let is_root = false;
                                            let effective_cmd = if requires_root && !is_root {
                                                let trimmed = final_cmd.trim();
                                                if trimmed.starts_with("sudo ")
                                                    || trimmed.starts_with("doas ")
                                                {
                                                    final_cmd
                                                } else {
                                                    format!("sudo {}", final_cmd)
                                                }
                                            } else {
                                                final_cmd
                                            };
                                            let mode = item.execution_mode();
                                            match mode {
                                                ExecutionMode::Streaming
                                                | ExecutionMode::Interactive => {
                                                    let is_interactive =
                                                        mode == ExecutionMode::Interactive;
                                                    let title = item
                                                        .title()
                                                        .map(|t| interpolate_placeholders(t, &ctx))
                                                        .unwrap_or_else(|| {
                                                            let (main_lbl, _) =
                                                                bashmenu_tui::split_label_brackets(
                                                                    &item.label,
                                                                );
                                                            let trimmed = main_lbl.trim();
                                                            if !trimmed.is_empty() {
                                                                interpolate_placeholders(
                                                                    trimmed, &ctx,
                                                                )
                                                            } else {
                                                                clean_cmd.clone()
                                                            }
                                                        });
                                                    let mut modal = StreamOutputModal::new(
                                                        title,
                                                        effective_cmd,
                                                        item.window_percent(),
                                                        is_interactive,
                                                    );
                                                    modal.set_show_command(item.show_command());
                                                    let screen_rect = ratatui::layout::Rect::new(
                                                        0,
                                                        0,
                                                        size.width,
                                                        size.height,
                                                    );
                                                    modal.resize_for_area(screen_rect);
                                                    modal.start();
                                                    active_modal = ActiveModal::StreamOutput(modal);
                                                    need_redraw = true;
                                                }
                                                ExecutionMode::Console => {
                                                    let is_quiet = item.quiet.unwrap_or(false);
                                                    let use_alt = item.alt_buffer.unwrap_or(true);
                                                    let trans_enabled = item.transition_enabled();
                                                    if trans_enabled {
                                                        let eff = TransitionEffect::parse(
                                                            config.settings.transition.as_deref(),
                                                        );
                                                        if eff != TransitionEffect::None
                                                            && eff != TransitionEffect::Instant
                                                        {
                                                            let _ = guard.draw_with_transition_out(
                                                                eff,
                                                                trans_speed,
                                                                |f| {
                                                                    menu_view.render(
                                                                        f.area(),
                                                                        f.buffer_mut(),
                                                                    );
                                                                },
                                                            );
                                                        }
                                                    }
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
                                                                effective_cmd
                                                            );
                                                        }
                                                        let _ = Command::new("/bin/bash")
                                                            .arg("-c")
                                                            .arg(&effective_cmd)
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
                                                    last_interaction = Instant::now();
                                                    if trans_enabled {
                                                        pending_transition = true;
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

            let curr_levels = menu_view.menu_level_titles();
            let curr_scroll = menu_view.current_scroll_offset();
            let curr_in_modal = !matches!(active_modal, ActiveModal::None);

            let page_changed = prev_levels != curr_levels
                || (curr_scroll as isize - prev_scroll as isize).abs() > 1;

            if page_changed {
                pending_transition = true;
                need_redraw = true;
            }

            if prev_in_modal && !curr_in_modal {
                if let Some(source_rect) = prev_modal_rect {
                    if modal_anim_enabled {
                        let _ = guard.animate_modal_shrink(
                            modal_anchor,
                            source_rect,
                            &styles,
                            modal_speed,
                            |f| menu_view.render(f.area(), f.buffer_mut()),
                        );
                    }
                }
                need_redraw = true;
            }

            if !prev_in_modal && curr_in_modal {
                pending_modal_grow = true;
                need_redraw = true;
            }
        }
    }

    Ok(())
}

/// Computes the next `event::poll` timeout from all active timers so an idle
/// menu sleeps until the earliest scheduled wakeup instead of polling at 20 Hz.
fn compute_idle_poll_timeout(
    gutter_remaining: Duration,
    marquee_remaining: Option<Duration>,
    leader_remaining: Option<Duration>,
    hover_remaining: Option<Duration>,
    inactivity_remaining: Option<Duration>,
    has_fast_activity: bool,
    has_plugins: bool,
) -> Duration {
    if has_fast_activity {
        return Duration::from_millis(50);
    }
    let mut timeout = gutter_remaining;
    if has_plugins {
        timeout = timeout.min(Duration::from_millis(500));
    }
    if let Some(rem) = marquee_remaining {
        timeout = timeout.min(rem);
    }
    if let Some(rem) = leader_remaining {
        timeout = timeout.min(rem);
    }
    if let Some(rem) = hover_remaining {
        timeout = timeout.min(rem);
    }
    if let Some(rem) = inactivity_remaining {
        timeout = timeout.min(rem);
    }
    timeout.max(Duration::from_millis(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_idle_poll_timeout_deadlines() {
        // Completely idle with 5s gutter interval -> sleeps full 5s (<= 1/sec)
        assert_eq!(
            compute_idle_poll_timeout(Duration::from_secs(5), None, None, None, None, false, false,),
            Duration::from_secs(5)
        );

        // Active toast / streaming modal -> snappy 50ms poll
        assert_eq!(
            compute_idle_poll_timeout(Duration::from_secs(5), None, None, None, None, true, false,),
            Duration::from_millis(50)
        );

        // Active marquee overrides long gutter wait
        assert_eq!(
            compute_idle_poll_timeout(
                Duration::from_secs(5),
                Some(Duration::from_millis(250)),
                None,
                None,
                None,
                false,
                false,
            ),
            Duration::from_millis(250)
        );

        // Pending leader or hover timer picks earliest deadline
        assert_eq!(
            compute_idle_poll_timeout(
                Duration::from_secs(5),
                Some(Duration::from_millis(250)),
                Some(Duration::from_millis(120)),
                Some(Duration::from_secs(3)),
                None,
                false,
                false,
            ),
            Duration::from_millis(120)
        );

        // Background plugins bound idle sleep to 500ms
        assert_eq!(
            compute_idle_poll_timeout(Duration::from_secs(5), None, None, None, None, false, true,),
            Duration::from_millis(500)
        );

        // Zero remaining duration is clamped to 10ms floor to avoid busy-spin
        assert_eq!(
            compute_idle_poll_timeout(Duration::ZERO, None, None, None, None, false, false,),
            Duration::from_millis(10)
        );
    }
}
