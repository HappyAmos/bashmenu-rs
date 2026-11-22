//! Visual split-screen menu tree editor for `bashmenu.mnu`.

use anyhow::Result;
use bashmenu_core::{
    interpolate_placeholders, load_config, load_menu, run_glyphs_search, save_menu, t,
    MacroContext, MenuItem, MenuSchema, ToastNotification, ToastSeverity,
};
use bashmenu_tui::{
    compute_modal_inner, get_marquee_slice, install_panic_hook, AsciiGridModal, ConfirmModal,
    FilePickerModal, HelpModal, InputModal, MarqueeTracker, ResolvedThemeStyles, TerminalGuard,
    ThemeActivity, ToastRack,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use menuedit_cli::{
    delete_item, find_item_mut, find_node_path, flatten_tree, format_inspector_text,
    get_type_badge, indent_item, insert_item_after, move_down_sibling, move_up_sibling,
    mutate_item_type, outdent_item, toggle_all_nodes, BrowseTarget, ItemEditModal, ItemEditResult,
    ItemTypePickerModal, ITEM_EDIT_HELP_TEXT, PLACEHOLDER_HELP_TEXT,
};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// Action identifiers triggered via keyboard shortcuts or mouse clicks on the footer gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuEditFooterAction {
    /// Show contextual help dialog
    Help,
    /// Toggle expansion of all submenu containers
    ToggleAll,
    /// Open ASCII table symbol picker
    Ascii,
    /// Open glyphs / Nerd Fonts search dialog
    Glyphs,
    /// Open dynamic macro placeholders reference
    Placeholders,
    /// Add a new menu item
    Add,
    /// Duplicate selected menu item in-place below it
    Duplicate,
    /// Edit the selected menu item properties
    Edit,
    /// Change selected menu item type
    Type,
    /// Expand or collapse current submenu node
    Toggle,
    /// Delete selected menu item
    Delete,
    /// Move selected item downward in sibling order
    MoveDown,
    /// Move selected item upward in sibling order
    MoveUp,
    /// Indent selected item (nest under previous sibling)
    Indent,
    /// Outdent selected item (promote to parent container)
    Outdent,
    /// Save menu configuration to disk
    Save,
    /// Exit the menu editor
    Exit,
}

/// Hit-testing region for mouse clicks on the footer help gutter.
#[derive(Clone, Copy)]
struct FooterHitRegion {
    /// Zero-based vertical row index
    row: u16,
    /// Starting terminal column index (inclusive)
    start_x: u16,
    /// Ending terminal column index (exclusive)
    end_x: u16,
    /// Target action triggered by clicking inside this region
    action: MenuEditFooterAction,
}

/// Command-line argument configuration for the Menu Editor (`menuedit`).
#[derive(Parser, Debug)]
#[command(
    name = "menuedit",
    author,
    version,
    about = "Interactive visual menu editor with side-by-side tree and property inspector"
)]
struct Cli {
    /// Path to menu definition file
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Load and save menu files in current working directory
    #[arg(long)]
    local: bool,

    /// Selected item path (comma-separated indices, e.g. "0,2")
    #[arg(long)]
    select_path: Option<String>,

    /// Selected item label or title to focus
    #[arg(long)]
    select_title: Option<String>,

    /// Hierarchical title chain for deep node resolution (comma or > separated)
    #[arg(long)]
    title_chain: Option<String>,

    /// Launch in Keyboard Shortcuts editor mode (`shortcuts.yml`)
    #[arg(long)]
    shortcuts: bool,

    /// Interactively merge a user menu with an upstream vanilla template
    #[arg(long)]
    merge: bool,

    /// User menu file for merge mode (defaults to user's bashmenu.mnu)
    #[arg(long)]
    user_menu: Option<PathBuf>,

    /// Vanilla / upstream template menu file for merge mode (defaults to repo or installed vanilla bashmenu.mnu)
    #[arg(long)]
    vanilla_menu: Option<PathBuf>,

    /// Destination file for merged menu (defaults to user_menu)
    #[arg(long)]
    output_menu: Option<PathBuf>,

    /// Create a backup file at this path before writing merged menu
    #[arg(long)]
    backup: Option<PathBuf>,

    /// Automatically accept defaults in non-interactive mode
    #[arg(short = 'y', long = "yes")]
    yes: bool,

    /// Override interface display locale
    #[arg(short = 'L', long = "locale")]
    locale: Option<String>,
}

/// Resolves the filesystem path for the target menu file.
/// If `local` is true, interprets non-absolute paths relative to the current working directory;
/// otherwise delegates to [`bashmenu_core::resolve_app_path`] to locate user/system configuration.
fn resolve_menu_path(target: &Path, local: bool) -> PathBuf {
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

/// Represents the active child modal spawned within an item editor modal.
enum ChildModal {
    /// Item type selection modal.
    TypePicker(ItemTypePickerModal),
    /// File / directory picker modal with target field indicator.
    FilePicker(FilePickerModal, BrowseTarget),
    /// Contextual help display modal.
    Help(HelpModal),
    /// ASCII table character picker modal.
    AsciiPicker(AsciiGridModal),
    /// Search modal for glyphs / nerd fonts.
    GlyphSearch(InputModal),
}

/// Represents the top-level active modal dialog currently being presented in the menu editor.
enum ActiveModal {
    /// No modal active; tree editor interaction enabled.
    None,
    /// Type picker modal with optional target item path.
    TypePicker(ItemTypePickerModal, Option<Vec<usize>>), // (modal, target_path)
    /// Full item configuration and editing modal.
    ItemEdit(Box<ItemEditModal>, Vec<usize>, Option<Box<ChildModal>>),
    /// Delete item confirmation modal.
    ConfirmDelete(ConfirmModal, Vec<usize>), // (modal, target_path)
    /// Unsaved changes exit confirmation modal.
    ConfirmExit(ConfirmModal),
    /// Global help dialog modal.
    Help(HelpModal),
    /// ASCII symbol picker modal.
    AsciiPicker(AsciiGridModal),
    /// Glyph search input modal.
    GlyphSearch(InputModal),
}

fn get_footer_items() -> [(String, MenuEditFooterAction); 17] {
    [
        (
            format!("[F1] {}", t!("common.gutters.help")),
            MenuEditFooterAction::Help,
        ),
        (
            format!("[F2] {}", t!("menuedit.gutter_toggle_all")),
            MenuEditFooterAction::ToggleAll,
        ),
        (
            format!("[F3] {}", t!("menuedit.gutter_ascii")),
            MenuEditFooterAction::Ascii,
        ),
        (
            format!("[F4] {}", t!("menuedit.gutter_glyphs")),
            MenuEditFooterAction::Glyphs,
        ),
        (
            format!("[F5] {}", t!("menuedit.gutter_placeholders")),
            MenuEditFooterAction::Placeholders,
        ),
        (
            format!("[a] {}", t!("menuedit.gutter_add")),
            MenuEditFooterAction::Add,
        ),
        (
            format!("[d] {}", t!("menuedit.gutter_duplicate")),
            MenuEditFooterAction::Duplicate,
        ),
        (
            format!("[e/ENTER] {}", t!("common.gutters.edit")),
            MenuEditFooterAction::Edit,
        ),
        (
            format!("[t] {}", t!("menuedit.gutter_type")),
            MenuEditFooterAction::Type,
        ),
        (
            format!("[SPACE] {}", t!("menuedit.gutter_toggle")),
            MenuEditFooterAction::Toggle,
        ),
        (
            format!("[DEL] {}", t!("common.buttons.delete")),
            MenuEditFooterAction::Delete,
        ),
        (
            format!("[m] {}", t!("menuedit.gutter_move_down")),
            MenuEditFooterAction::MoveDown,
        ),
        (
            format!("[M] {}", t!("menuedit.gutter_move_up")),
            MenuEditFooterAction::MoveUp,
        ),
        (
            format!("[>] {}", t!("menuedit.gutter_indent")),
            MenuEditFooterAction::Indent,
        ),
        (
            format!("[<] {}", t!("menuedit.gutter_outdent")),
            MenuEditFooterAction::Outdent,
        ),
        (
            format!("[s] {}", t!("common.buttons.save")),
            MenuEditFooterAction::Save,
        ),
        (
            format!("[ESC/q] {}", t!("common.gutters.quit")),
            MenuEditFooterAction::Exit,
        ),
    ]
}

/// Entry point for the Menu Editor TUI application (`menuedit`).
fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let cfg_path = resolve_menu_path(Path::new("bashmenu.yml"), cli.local);
    let cfg = load_config(&cfg_path).ok();

    let locale_target = cli
        .locale
        .clone()
        .or_else(|| cfg.as_ref().and_then(|c| c.settings.locale.clone()))
        .or_else(|| std::env::var("LC_ALL").ok())
        .or_else(|| std::env::var("LC_MESSAGES").ok())
        .or_else(|| std::env::var("LANG").ok());

    if let Some(ref loc) = locale_target {
        let clean_loc = loc.split('.').next().unwrap_or(loc);
        let _ = bashmenu_core::set_locale(clean_loc);
    }

    if cli.merge {
        let user_menu = if let Some(ref u) = cli.user_menu {
            resolve_menu_path(u, cli.local)
        } else if let Some(ref f) = cli.file {
            resolve_menu_path(f, cli.local)
        } else if cli.local {
            PathBuf::from("./bashmenu.mnu")
        } else {
            bashmenu_core::get_user_config_dir().join("bashmenu.mnu")
        };

        let vanilla_menu = if let Some(ref v) = cli.vanilla_menu {
            resolve_menu_path(v, cli.local)
        } else {
            bashmenu_core::resolve_app_path("bashmenu.mnu")
        };

        let output_menu = cli
            .output_menu
            .as_ref()
            .map(|o| resolve_menu_path(o, cli.local))
            .unwrap_or_else(|| user_menu.clone());

        let backup_menu = cli.backup.as_ref().map(|b| resolve_menu_path(b, cli.local));

        return menuedit_cli::run_merge_wizard(
            &user_menu,
            &vanilla_menu,
            &output_menu,
            backup_menu.as_deref(),
            cli.yes,
        );
    }

    if cli.shortcuts {
        let sp = if let Some(ref f) = cli.file {
            resolve_menu_path(f, cli.local)
        } else {
            bashmenu_core::resolve_shortcuts_path(cli.local)
        };
        return menuedit_cli::run_shortcut_editor(sp);
    }
    let menu_path = if let Some(ref f) = cli.file {
        resolve_menu_path(f, cli.local)
    } else if cli.local {
        PathBuf::from("./bashmenu.mnu")
    } else {
        bashmenu_core::get_user_config_dir().join("bashmenu.mnu")
    };
    let mut menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        let vanilla_menu = bashmenu_core::resolve_app_path("bashmenu.mnu");
        if vanilla_menu.exists() && vanilla_menu != menu_path {
            load_menu(&vanilla_menu).unwrap_or_default()
        } else {
            MenuSchema::default()
        }
    };

    let themes_path = resolve_menu_path(Path::new("bashmenu.themes"), cli.local);
    let themes = bashmenu_core::load_themes(&themes_path).ok();
    let use_nerd = cfg
        .as_ref()
        .and_then(|c| c.settings.use_nerd_fonts)
        .unwrap_or(true);
    let active_theme = cfg.as_ref().and_then(|c| themes.as_ref()?.get(&c.theme));
    let styles = active_theme
        .map(|t| ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::MenuEdit))
        .unwrap_or_default();
    let modal_styles = active_theme
        .map(|t| ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::Modals))
        .unwrap_or_default();
    let toast_styles = active_theme
        .map(|t| ResolvedThemeStyles::from_theme_for_activity(t, use_nerd, ThemeActivity::Toasts))
        .unwrap_or_default();
    let mut guard = TerminalGuard::init()?;

    let mut toast_rack = ToastRack::new();
    let mut expanded_paths = HashSet::new();
    let mut selected_node_idx = 0;

    // Resolve initial focus if specified via CLI flags
    if let Some(target_path) = find_node_path(
        &menu,
        cli.select_path.as_deref(),
        cli.select_title.as_deref(),
        cli.title_chain.as_deref(),
    ) {
        for len in 1..target_path.len() {
            expanded_paths.insert(target_path[..len].to_vec());
        }
        let init_nodes = flatten_tree(&menu, &expanded_paths);
        if let Some(pos) = init_nodes.iter().position(|n| n.path == target_path) {
            selected_node_idx = pos;
        }
    }
    let mut tree_scroll = 0;
    let mut inspector_scroll = 0;
    let mut is_modified = false;
    let mut active_modal = ActiveModal::None;
    let hover_duration = std::time::Duration::from_secs_f64(
        cfg.as_ref()
            .map(|c| c.help_gutter_delay())
            .unwrap_or(4.0)
            .max(0.0),
    );
    let mut footer_hover = bashmenu_tui::HelpGutterHover::with_duration(hover_duration);
    let marquee_interval = cfg
        .as_ref()
        .map(|c| c.settings.marquee_speed_duration())
        .unwrap_or_else(|| Duration::from_millis(250));
    let mut last_marquee_tick = std::time::Instant::now();
    let mut marquee_tracker = MarqueeTracker::default();
    let mut last_selected_idx = selected_node_idx;

    loop {
        if selected_node_idx != last_selected_idx {
            marquee_tracker.reset();
            last_marquee_tick = std::time::Instant::now();
            last_selected_idx = selected_node_idx;
        }
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        let visible_nodes = flatten_tree(&menu, &expanded_paths);
        if !visible_nodes.is_empty() && selected_node_idx >= visible_nodes.len() {
            selected_node_idx = visible_nodes.len() - 1;
        }

        // Keep tree scroll visible (shrink available height by 1 row to prevent bottom border overlap)
        let max_tree_rows = area.height.saturating_sub(7) as usize;
        if selected_node_idx < tree_scroll {
            tree_scroll = selected_node_idx;
        } else if selected_node_idx >= tree_scroll + max_tree_rows && max_tree_rows > 0 {
            tree_scroll = selected_node_idx.saturating_sub(max_tree_rows - 1);
        }

        let mut footer_hit_regions: Vec<FooterHitRegion> = Vec::new();

        guard.draw(|f| {
            let buf = f.buffer_mut();

            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    buf[(x, y)].reset();
                }
            }

            // Root window title
            let file_name = menu_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "bashmenu.mnu".to_string());
            let mod_str = if is_modified { " (Modified)" } else { "" };
            let title_raw = format!("─[ Visual Menu Editor - {}{} ]", file_name, mod_str);
            let title_max_w = (area.width as usize).saturating_sub(6);
            let title_vis = UnicodeWidthStr::width(title_raw.as_str());
            let title = if title_vis > title_max_w && title_max_w > 0 {
                get_marquee_slice(
                    &title_raw,
                    title_max_w,
                    marquee_tracker.offset,
                    &styles.marquee_char,
                    styles.marquee_gap,
                )
            } else {
                title_raw
            };

            let root_block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(styles.border))
                .style(Style::default().bg(styles.background));
            root_block.render(area, buf);

            // Universal [X] close button on top right of the root editor window
            if area.width > 6 {
                let close_x = area.x + area.width - 4;
                buf.set_string(close_x, area.y, "[X]", styles.window_close_button);
            }

            if area.width < 40 || area.height < 10 {
                return;
            }

            let inner_x = area.x + 2;
            let inner_y = area.y + 1;
            let avail_w = area.width.saturating_sub(4);
            let footer_h: u16 = if footer_hover.is_expanded() { 3 } else { 2 };
            let avail_h = area.height.saturating_sub(2 + footer_h);

            let left_w = avail_w / 2;
            let right_w = avail_w.saturating_sub(left_w);

            // Left Panel: Tree
            let tree_block = Block::default()
                .borders(Borders::ALL)
                .title("─[ Menu Structure ]")
                .border_style(styles.accent);
            let tree_rect = Rect::new(inner_x, inner_y, left_w, avail_h.saturating_sub(1));
            tree_block.render(tree_rect, buf);

            let tree_content_w = tree_rect.width.saturating_sub(2);
            for (idx, node) in visible_nodes
                .iter()
                .skip(tree_scroll)
                .take(max_tree_rows)
                .enumerate()
            {
                let global_idx = tree_scroll + idx;
                let row_y = tree_rect.y + 1 + idx as u16;
                let is_sel = global_idx == selected_node_idx;

                let indent = "  ".repeat(node.depth);
                let expand_icon = if node.has_children {
                    if node.is_expanded {
                        "▼ "
                    } else {
                        "▶ "
                    }
                } else {
                    "  "
                };

                let badge = get_type_badge(&node.item);
                let row_style = if is_sel {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                };

                let (icon_span, label_text) = if node.item.is_divider() {
                    (Span::raw(""), "──────────────── [Divider]".to_string())
                } else {
                    let default_icon_str = styles.default_icon.as_str();
                    let raw_icon = node
                        .item
                        .icon
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(default_icon_str);
                    let exp_icon = if let Some(ref c) = cfg {
                        let ctx = MacroContext::new(c);
                        interpolate_placeholders(raw_icon, &ctx)
                    } else {
                        raw_icon.to_string()
                    };
                    let icon_slot = bashmenu_core::format_icon_slot(Some(&exp_icon), use_nerd);
                    (
                        Span::styled(icon_slot, if is_sel { row_style } else { styles.accent }),
                        node.item.label.clone(),
                    )
                };

                let prefix_vis =
                    node.depth * 2 + 2 + 6 + if node.item.is_divider() { 0 } else { 4 };
                let avail_for_label = (tree_content_w as usize).saturating_sub(prefix_vis);
                let label_vis = UnicodeWidthStr::width(label_text.as_str());
                let disp_label = if is_sel && label_vis > avail_for_label && avail_for_label > 0 {
                    get_marquee_slice(
                        &label_text,
                        avail_for_label,
                        marquee_tracker.offset,
                        &styles.marquee_char,
                        styles.marquee_gap,
                    )
                } else {
                    label_text
                };

                let line = Line::from(vec![
                    Span::styled(indent, row_style),
                    Span::styled(expand_icon, if is_sel { row_style } else { styles.accent }),
                    Span::styled(
                        format!("{:<5} ", badge),
                        if is_sel {
                            row_style
                        } else {
                            styles.shortcut_key
                        },
                    ),
                    icon_span,
                    Span::styled(disp_label, row_style),
                ]);

                buf.set_line(tree_rect.x + 1, row_y, &line, tree_content_w);
            }

            // Right Panel: Property Inspector
            let insp_x = inner_x + left_w;
            let insp_rect = Rect::new(insp_x, inner_y, right_w, avail_h.saturating_sub(1));
            let insp_title = format!("───[ {} ]", t!("menuedit.inspector_title"));
            let insp_block = Block::default()
                .borders(Borders::ALL)
                .title(insp_title.as_str())
                .border_style(styles.accent);
            insp_block.render(insp_rect, buf);

            let insp_content_w = insp_rect.width.saturating_sub(2);
            let insp_content_h = insp_rect.height.saturating_sub(2) as usize;

            if let Some(selected_node) = visible_nodes.get(selected_node_idx) {
                let insp_lines = format_inspector_text(
                    &selected_node.item,
                    cfg.as_ref(),
                    insp_content_w as usize,
                );
                for (i, line) in insp_lines
                    .iter()
                    .skip(inspector_scroll)
                    .take(insp_content_h)
                    .enumerate()
                {
                    let y = insp_rect.y + 1 + i as u16;
                    let line_style = if line.starts_with('[') {
                        styles.title.add_modifier(Modifier::BOLD)
                    } else if line.starts_with("Title/Label:")
                        || line.starts_with("Expanded Title:")
                    {
                        styles.text
                    } else if line.starts_with("Icon/Glyph:") || line.starts_with("Expanded Icon:")
                    {
                        styles.accent
                    } else if line.starts_with("Action/Target:")
                        || line.starts_with("Expanded Action:")
                    {
                        styles.shortcut_key
                    } else if line.starts_with("Config Key:") || line.starts_with("Message/Prompt:")
                    {
                        styles.help_text
                    } else if line.starts_with("Preview:") {
                        styles.accent.add_modifier(Modifier::BOLD)
                    } else {
                        styles.text
                    };

                    let line_vis = UnicodeWidthStr::width(line.as_str());
                    let disp_line = if line_vis > insp_content_w as usize && insp_content_w > 0 {
                        get_marquee_slice(
                            line,
                            insp_content_w as usize,
                            marquee_tracker.offset,
                            &styles.marquee_char,
                            styles.marquee_gap,
                        )
                    } else {
                        line.chars().take(insp_content_w as usize).collect()
                    };
                    buf.set_string(insp_rect.x + 1, y, &disp_line, line_style);
                }
            } else {
                buf.set_string(
                    insp_rect.x + 2,
                    insp_rect.y + 2,
                    t!("menuedit.no_item_selected"),
                    styles.text,
                );
            }

            // Footer (2 or 3 lines depending on hover expansion)
            let items_all = get_footer_items();
            let all_labels: Vec<&str> = items_all.iter().map(|(lbl, _)| lbl.as_str()).collect();
            let has_hidden = bashmenu_tui::has_hidden_gutter_text(&all_labels, avail_w as usize, 2);
            let is_expanded = footer_hover.is_expanded() && has_hidden && area.height >= 12;
            let footer_rows: usize = if is_expanded { 3 } else { 2 };

            let gutter_layouts = bashmenu_tui::wrap_spread_help_gutter_layout(
                &all_labels,
                avail_w as usize,
                footer_rows,
            );

            for (r_idx, (line_str, ranges)) in gutter_layouts.into_iter().enumerate() {
                let row_y = (area.y + area.height).saturating_sub(1 + (footer_rows - r_idx) as u16);
                let clipped_line = if line_str.len() > avail_w as usize {
                    line_str.chars().take(avail_w as usize).collect::<String>()
                } else {
                    line_str
                };
                buf.set_string(inner_x, row_y, &clipped_line, styles.footer);
                for (tok_label, start, end) in ranges {
                    if let Some((_, action)) =
                        items_all.iter().find(|(lbl, _)| lbl.as_str() == tok_label)
                    {
                        let screen_start = inner_x + start as u16;
                        let screen_end = inner_x + end as u16;
                        if screen_start < inner_x + avail_w {
                            let max_tok_w = (inner_x + avail_w - screen_start) as usize;
                            let display_tok = if tok_label.chars().count() > max_tok_w {
                                tok_label.chars().take(max_tok_w).collect::<String>()
                            } else {
                                tok_label.to_string()
                            };
                            buf.set_string(screen_start, row_y, &display_tok, styles.help_text);
                            footer_hit_regions.push(FooterHitRegion {
                                row: row_y,
                                start_x: screen_start,
                                end_x: screen_end.min(inner_x + avail_w),
                                action: *action,
                            });
                        }
                    }
                }
            }

            // Render Active Modals
            match &mut active_modal {
                ActiveModal::None => {}
                ActiveModal::TypePicker(modal, _) => {
                    modal.render(area, &modal_styles, buf);
                }
                ActiveModal::ItemEdit(modal, _, child_opt) => {
                    modal.render(area, &modal_styles, buf);
                    if let Some(child) = child_opt {
                        match child.as_mut() {
                            ChildModal::TypePicker(m) => {
                                m.render(area, &modal_styles, buf);
                            }
                            ChildModal::FilePicker(m, _) => {
                                m.render(area, &modal_styles, buf);
                            }
                            ChildModal::Help(m) => {
                                m.render(area, &modal_styles, buf);
                            }
                            ChildModal::AsciiPicker(m) => {
                                m.render(area, &modal_styles, buf);
                            }
                            ChildModal::GlyphSearch(m) => {
                                m.render(area, &modal_styles, buf);
                            }
                        }
                    }
                }
                ActiveModal::ConfirmDelete(modal, _) => {
                    modal.render(area, &modal_styles, buf);
                }
                ActiveModal::ConfirmExit(modal) => {
                    modal.render(area, &modal_styles, buf);
                }
                ActiveModal::Help(modal) => {
                    modal.render(area, &modal_styles, buf);
                }
                ActiveModal::AsciiPicker(modal) => {
                    modal.render(area, &modal_styles, buf);
                }
                ActiveModal::GlyphSearch(modal) => {
                    modal.render(area, &modal_styles, buf);
                }
            }

            // Render floating toast notifications atop everything
            toast_rack.render(area, &toast_styles, buf);
        })?;

        toast_rack.tick();

        let footer_items_outer = get_footer_items();
        let all_labels: Vec<&str> = footer_items_outer
            .iter()
            .map(|(lbl, _)| lbl.as_str())
            .collect();
        let avail_w = area.width.saturating_sub(4);
        let has_hidden = bashmenu_tui::has_hidden_gutter_text(&all_labels, avail_w as usize, 2);

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;
            if let Event::Mouse(m) = &ev {
                if m.kind == MouseEventKind::Moved {
                    let footer_rows: u16 = if footer_hover.is_expanded() { 3 } else { 2 };
                    let in_footer = m.row >= (area.y + area.height).saturating_sub(1 + footer_rows)
                        && m.row < area.y + area.height - 1;
                    if matches!(active_modal, ActiveModal::None) {
                        footer_hover.handle_mouse_move(m.column, m.row, in_footer && has_hidden);
                    }
                    continue;
                }
                footer_hover.handle_interaction();
            }
            if let Event::Key(_) = &ev {
                footer_hover.handle_interaction();
            }

            match &mut active_modal {
                ActiveModal::TypePicker(modal, target_path_opt) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(chosen_type) = res {
                                if let Some(path) = target_path_opt.take() {
                                    if let Some(item) = find_item_mut(&mut menu, &path) {
                                        mutate_item_type(item, &chosen_type);
                                        is_modified = true;
                                        toast_rack.add(ToastNotification {
                                            title: Some("Menu Editor".to_string()),
                                            message: format!("Mutated type to {}", chosen_type),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 1.5,
                                        });
                                    }
                                } else {
                                    let new_item = if chosen_type == "{divider}" {
                                        MenuItem::divider()
                                    } else {
                                        let mut it = MenuItem {
                                            label: format!("New {}", chosen_type),
                                            icon: Some(styles.default_icon.clone()),
                                            ..Default::default()
                                        };
                                        mutate_item_type(&mut it, &chosen_type);
                                        it
                                    };

                                    let cur_path = visible_nodes
                                        .get(selected_node_idx)
                                        .map(|n| n.path.clone())
                                        .unwrap_or_default();
                                    let new_path =
                                        insert_item_after(&mut menu, &cur_path, new_item.clone());
                                    is_modified = true;
                                    toast_rack.add(ToastNotification {
                                        title: Some("Menu Editor".to_string()),
                                        message: format!("Added new {}", chosen_type),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                    active_modal = ActiveModal::ItemEdit(
                                        Box::new(ItemEditModal::new(&new_item)),
                                        new_path,
                                        None,
                                    );
                                    continue;
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, area) {
                            if res.is_none() {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    _ => {}
                },
                ActiveModal::ItemEdit(modal, path, child_opt) => {
                    if let Some(child) = child_opt {
                        // Handle events for stacked child modal
                        match child.as_mut() {
                            ChildModal::TypePicker(m) => match ev {
                                Event::Key(k) => {
                                    if let Some(res) = m.handle_key(k.code) {
                                        if let Some(chosen) = res {
                                            modal.apply_type_change(&chosen);
                                        }
                                        *child_opt = None;
                                    }
                                }
                                Event::Mouse(mouse) => {
                                    if let Some(res) = m.handle_mouse(mouse, area) {
                                        if let Some(chosen) = res {
                                            modal.apply_type_change(&chosen);
                                        }
                                        *child_opt = None;
                                    }
                                }
                                _ => {}
                            },
                            ChildModal::FilePicker(m, target) => match ev {
                                Event::Key(k) => {
                                    if let Some(res) = m.handle_key(k.code) {
                                        if let Some(picked) = res {
                                            modal.set_browse_result(
                                                *target,
                                                &picked.to_string_lossy(),
                                            );
                                        }
                                        *child_opt = None;
                                    }
                                }
                                Event::Mouse(mouse) => {
                                    if let Some(res) = m.handle_mouse(mouse, area) {
                                        if let Some(picked) = res {
                                            modal.set_browse_result(
                                                *target,
                                                &picked.to_string_lossy(),
                                            );
                                        }
                                        *child_opt = None;
                                    }
                                }
                                _ => {}
                            },
                            ChildModal::Help(m) => match ev {
                                Event::Key(k) if m.handle_key(k.code) => {
                                    *child_opt = None;
                                }
                                Event::Mouse(mouse) if m.handle_mouse(mouse, area) => {
                                    *child_opt = None;
                                }
                                _ => {}
                            },
                            ChildModal::AsciiPicker(m) => match ev {
                                Event::Key(k) => {
                                    if let Some(opt_char) = m.handle_key(k.code) {
                                        if opt_char.is_some() {
                                            let s = format!("{{ascii:{}}}", m.selected_code);
                                            modal.char_input.set_text(&s);
                                        }
                                        *child_opt = None;
                                    }
                                }
                                Event::Mouse(mouse) => {
                                    let inner = compute_modal_inner(area, 56, 22);
                                    let grid_start = (inner.x + 2, inner.y + 1);
                                    if let Some(opt_char) = m.handle_mouse(mouse, grid_start) {
                                        if opt_char.is_some() {
                                            let s = format!("{{ascii:{}}}", m.selected_code);
                                            modal.char_input.set_text(&s);
                                        }
                                        *child_opt = None;
                                    }
                                }
                                _ => {}
                            },
                            ChildModal::GlyphSearch(m) => match ev {
                                Event::Key(k) => {
                                    if let Some(res) =
                                        m.handle_key_with_modifiers(k.code, k.modifiers)
                                    {
                                        if let Some(term) = res {
                                            let output_text = run_glyphs_search(&term);
                                            *child_opt =
                                                Some(Box::new(ChildModal::Help(HelpModal::new(
                                                    format!("Nerd Font & Emoji: {}", term),
                                                    &output_text,
                                                ))));
                                        } else {
                                            *child_opt = None;
                                        }
                                    }
                                }
                                Event::Mouse(mouse) => {
                                    if let Some(res) = m.handle_mouse(mouse, area) {
                                        if let Some(term) = res {
                                            let output_text = run_glyphs_search(&term);
                                            *child_opt =
                                                Some(Box::new(ChildModal::Help(HelpModal::new(
                                                    format!("Nerd Font & Emoji: {}", term),
                                                    &output_text,
                                                ))));
                                        } else {
                                            *child_opt = None;
                                        }
                                    }
                                }
                                _ => {}
                            },
                        }
                    } else {
                        // Handle events for ItemEditModal itself
                        let edit_res = match ev {
                            Event::Key(k) => modal.handle_key(k.code, k.modifiers),
                            Event::Mouse(m) => modal.handle_mouse(m, area),
                            _ => ItemEditResult::Continue,
                        };

                        match edit_res {
                            ItemEditResult::Continue => {}
                            ItemEditResult::Save(updated) => {
                                if let Some(item_slot) = find_item_mut(&mut menu, path) {
                                    *item_slot = *updated;
                                    is_modified = true;
                                    toast_rack.add(ToastNotification {
                                        title: Some("Menu Editor".to_string()),
                                        message: "Updated item properties".to_string(),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                }
                                active_modal = ActiveModal::None;
                            }
                            ItemEditResult::Cancel => {
                                active_modal = ActiveModal::None;
                            }
                            ItemEditResult::ChangeType => {
                                *child_opt = Some(Box::new(ChildModal::TypePicker(
                                    ItemTypePickerModal::new(&modal.current_type),
                                )));
                            }
                            ItemEditResult::ShowHelp => {
                                *child_opt = Some(Box::new(ChildModal::Help(HelpModal::new(
                                    "Item Properties Guide",
                                    ITEM_EDIT_HELP_TEXT,
                                ))));
                            }
                            ItemEditResult::ShowAscii => {
                                *child_opt =
                                    Some(Box::new(ChildModal::AsciiPicker(AsciiGridModal::new(
                                        t!("common.dialogs.ascii_table_title"),
                                        196,
                                    ))));
                            }
                            ItemEditResult::ShowGlyphs => {
                                *child_opt =
                                    Some(Box::new(ChildModal::GlyphSearch(InputModal::new(
                                        t!("common.dialogs.search_glyphs_title"),
                                        t!("common.dialogs.search_glyphs_prompt"),
                                        "",
                                    ))));
                            }
                            ItemEditResult::ShowPlaceholders => {
                                *child_opt = Some(Box::new(ChildModal::Help(HelpModal::new(
                                    "Placeholders & Macros",
                                    PLACEHOLDER_HELP_TEXT,
                                ))));
                            }
                            ItemEditResult::Browse(target) => {
                                let (title, only_dirs) = match target {
                                    BrowseTarget::Action => ("Select Action File", false),
                                    BrowseTarget::Template => ("Select Template File", false),
                                    BrowseTarget::Target => ("Select Target File", false),
                                    BrowseTarget::StartDir => ("Select Start Directory", true),
                                    BrowseTarget::OnYes => ("Select On-Yes File/Script", false),
                                    BrowseTarget::OnNo => ("Select On-No File/Script", false),
                                };
                                *child_opt = Some(Box::new(ChildModal::FilePicker(
                                    FilePickerModal::new(title, &PathBuf::from("."), only_dirs),
                                    target,
                                )));
                            }
                        }
                    }
                }
                ActiveModal::ConfirmDelete(modal, path) => match ev {
                    Event::Key(k) => {
                        if let Some(choice) = modal.handle_key(k.code) {
                            if choice.is_yes() {
                                if let Some(deleted) = delete_item(&mut menu, path) {
                                    is_modified = true;
                                    toast_rack.add(ToastNotification {
                                        title: Some("Menu Editor".to_string()),
                                        message: format!("Deleted '{}'", deleted.label),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                    selected_node_idx = selected_node_idx.saturating_sub(1);
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(choice) = modal.handle_mouse(m, area) {
                            if choice.is_yes() {
                                if let Some(deleted) = delete_item(&mut menu, path) {
                                    is_modified = true;
                                    toast_rack.add(ToastNotification {
                                        title: Some("Menu Editor".to_string()),
                                        message: format!("Deleted '{}'", deleted.label),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                    selected_node_idx = selected_node_idx.saturating_sub(1);
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::ConfirmExit(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(choice) = modal.handle_key(k.code) {
                            match choice {
                                bashmenu_tui::ConfirmChoice::Yes => {
                                    let _ = save_menu(&menu_path, &menu);
                                    break;
                                }
                                bashmenu_tui::ConfirmChoice::No => {
                                    break;
                                }
                                bashmenu_tui::ConfirmChoice::Cancel => {
                                    active_modal = ActiveModal::None;
                                }
                            }
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(choice) = modal.handle_mouse(m, area) {
                            match choice {
                                bashmenu_tui::ConfirmChoice::Yes => {
                                    let _ = save_menu(&menu_path, &menu);
                                    break;
                                }
                                bashmenu_tui::ConfirmChoice::No => {
                                    break;
                                }
                                bashmenu_tui::ConfirmChoice::Cancel => {
                                    active_modal = ActiveModal::None;
                                }
                            }
                        }
                    }
                    _ => {}
                },
                ActiveModal::Help(modal) => match ev {
                    Event::Key(k) if modal.handle_key(k.code) => {
                        active_modal = ActiveModal::None;
                    }
                    Event::Mouse(m) if modal.handle_mouse(m, area) => {
                        active_modal = ActiveModal::None;
                    }
                    _ => {}
                },
                ActiveModal::AsciiPicker(modal) => match ev {
                    Event::Key(k) => {
                        if modal.handle_key(k.code).is_some() {
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        let inner = compute_modal_inner(area, 56, 22);
                        let grid_start = (inner.x + 2, inner.y + 1);
                        if modal.handle_mouse(m, grid_start).is_some() {
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::GlyphSearch(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key_with_modifiers(k.code, k.modifiers) {
                            if let Some(term) = res {
                                let output_text = run_glyphs_search(&term);
                                active_modal = ActiveModal::Help(HelpModal::new(
                                    format!("Nerd Font & Emoji: {}", term),
                                    &output_text,
                                ));
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, area) {
                            if let Some(term) = res {
                                let output_text = run_glyphs_search(&term);
                                active_modal = ActiveModal::Help(HelpModal::new(
                                    format!("Nerd Font & Emoji: {}", term),
                                    &output_text,
                                ));
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    _ => {}
                },
                ActiveModal::None => {
                    match ev {
                        Event::Mouse(m) => {
                            if m.kind == MouseEventKind::Down(MouseButton::Left) {
                                // Dismiss active toast on direct click
                                if toast_rack.handle_mouse(m.column, m.row) {
                                    continue;
                                }

                                // Close button [X] at top right
                                if m.row == area.y && m.column >= area.x + area.width - 5 {
                                    if is_modified {
                                        active_modal = ActiveModal::ConfirmExit(
                                            ConfirmModal::new(
                                                t!("common.dialogs.unsaved_changes_title"),
                                                t!("common.dialogs.unsaved_changes_msg"),
                                            )
                                            .with_default_cancel(),
                                        );
                                    } else {
                                        break;
                                    }
                                    continue;
                                }

                                // Interactive footer clicks
                                if let Some(hit) = footer_hit_regions.iter().find(|r| {
                                    r.row == m.row && m.column >= r.start_x && m.column < r.end_x
                                }) {
                                    match hit.action {
                                        MenuEditFooterAction::Help => {
                                            active_modal = ActiveModal::Help(HelpModal::new(
                                                "Visual Menu Editor Guide",
                                                ITEM_EDIT_HELP_TEXT,
                                            ));
                                        }
                                        MenuEditFooterAction::ToggleAll => {
                                            toggle_all_nodes(&menu, &mut expanded_paths);
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Toggled all branches".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                        }
                                        MenuEditFooterAction::Ascii => {
                                            active_modal =
                                                ActiveModal::AsciiPicker(AsciiGridModal::new(
                                                    t!("common.dialogs.ascii_table_title"),
                                                    196,
                                                ));
                                        }
                                        MenuEditFooterAction::Glyphs => {
                                            active_modal =
                                                ActiveModal::GlyphSearch(InputModal::new(
                                                    t!("common.dialogs.search_glyphs_title"),
                                                    t!("common.dialogs.search_glyphs_prompt"),
                                                    "",
                                                ));
                                        }
                                        MenuEditFooterAction::Placeholders => {
                                            active_modal = ActiveModal::Help(HelpModal::new(
                                                "Placeholders & Macros",
                                                PLACEHOLDER_HELP_TEXT,
                                            ));
                                        }
                                        MenuEditFooterAction::Add => {
                                            active_modal = ActiveModal::TypePicker(
                                                ItemTypePickerModal::new("command"),
                                                None,
                                            );
                                        }
                                        MenuEditFooterAction::Edit => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                active_modal = ActiveModal::ItemEdit(
                                                    Box::new(ItemEditModal::new(&node.item)),
                                                    node.path.clone(),
                                                    None,
                                                );
                                            }
                                        }
                                        MenuEditFooterAction::Type => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                active_modal = ActiveModal::TypePicker(
                                                    ItemTypePickerModal::new("command"),
                                                    Some(node.path.clone()),
                                                );
                                            }
                                        }
                                        MenuEditFooterAction::Toggle => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if node.has_children {
                                                    if expanded_paths.contains(&node.path) {
                                                        expanded_paths.remove(&node.path);
                                                    } else {
                                                        expanded_paths.insert(node.path.clone());
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::Duplicate => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if let Some(new_p) = menuedit_cli::duplicate_item(
                                                    &mut menu,
                                                    &node.path,
                                                    &mut expanded_paths,
                                                ) {
                                                    is_modified = true;
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Menu Editor".to_string()),
                                                        message: "Duplicated item".to_string(),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 1.5,
                                                    });
                                                    let new_vis =
                                                        flatten_tree(&menu, &expanded_paths);
                                                    if let Some(pos) =
                                                        new_vis.iter().position(|n| n.path == new_p)
                                                    {
                                                        selected_node_idx = pos;
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::Delete => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                active_modal = ActiveModal::ConfirmDelete(
                                                    ConfirmModal::new(
                                                        t!("menuedit.delete_item_title"),
                                                        t!(
                                                            "menuedit.delete_item_confirm",
                                                            label => &node.item.label
                                                        ),
                                                    ),
                                                    node.path.clone(),
                                                );
                                            }
                                        }
                                        MenuEditFooterAction::MoveDown => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if let Some(new_p) =
                                                    move_down_sibling(&mut menu, &node.path)
                                                {
                                                    is_modified = true;
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Menu Editor".to_string()),
                                                        message: "Moved item down".to_string(),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 1.5,
                                                    });
                                                    let new_vis =
                                                        flatten_tree(&menu, &expanded_paths);
                                                    if let Some(pos) =
                                                        new_vis.iter().position(|n| n.path == new_p)
                                                    {
                                                        selected_node_idx = pos;
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::MoveUp => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if let Some(new_p) =
                                                    move_up_sibling(&mut menu, &node.path)
                                                {
                                                    is_modified = true;
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Menu Editor".to_string()),
                                                        message: "Moved item up".to_string(),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 1.5,
                                                    });
                                                    let new_vis =
                                                        flatten_tree(&menu, &expanded_paths);
                                                    if let Some(pos) =
                                                        new_vis.iter().position(|n| n.path == new_p)
                                                    {
                                                        selected_node_idx = pos;
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::Indent => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if let Some(new_p) = indent_item(
                                                    &mut menu,
                                                    &node.path,
                                                    &mut expanded_paths,
                                                ) {
                                                    is_modified = true;
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Menu Editor".to_string()),
                                                        message: "Indented item into submenu"
                                                            .to_string(),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 1.5,
                                                    });
                                                    let new_vis =
                                                        flatten_tree(&menu, &expanded_paths);
                                                    if let Some(pos) =
                                                        new_vis.iter().position(|n| n.path == new_p)
                                                    {
                                                        selected_node_idx = pos;
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::Outdent => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                if let Some(new_p) =
                                                    outdent_item(&mut menu, &node.path)
                                                {
                                                    is_modified = true;
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Menu Editor".to_string()),
                                                        message: "Outdented item to parent level"
                                                            .to_string(),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 1.5,
                                                    });
                                                    let new_vis =
                                                        flatten_tree(&menu, &expanded_paths);
                                                    if let Some(pos) =
                                                        new_vis.iter().position(|n| n.path == new_p)
                                                    {
                                                        selected_node_idx = pos;
                                                    }
                                                }
                                            }
                                        }
                                        MenuEditFooterAction::Save => {
                                            if let Err(e) = save_menu(&menu_path, &menu) {
                                                toast_rack.add(ToastNotification {
                                                    title: Some("Error".to_string()),
                                                    message: format!("Failed to save menu: {}", e),
                                                    severity: ToastSeverity::Error,
                                                    timeout_secs: 3.0,
                                                });
                                            } else {
                                                is_modified = false;
                                                toast_rack.add(ToastNotification {
                                                    title: Some("Save Menu".to_string()),
                                                    message: "Saved menu to disk".to_string(),
                                                    severity: ToastSeverity::Information,
                                                    timeout_secs: 2.0,
                                                });
                                            }
                                        }
                                        MenuEditFooterAction::Exit => {
                                            if is_modified {
                                                active_modal = ActiveModal::ConfirmExit(
                                                    ConfirmModal::new(
                                                        t!("common.dialogs.unsaved_changes_title"),
                                                        t!("common.dialogs.unsaved_changes_msg"),
                                                    )
                                                    .with_default_cancel(),
                                                );
                                            } else {
                                                break;
                                            }
                                        }
                                    }
                                    continue;
                                }

                                // Tree panel clicks
                                let inner_x = area.x + 2;
                                let inner_y = area.y + 1;
                                let avail_w = area.width.saturating_sub(4);
                                let avail_h = area.height.saturating_sub(4);
                                let left_w = avail_w / 2;
                                if m.column >= inner_x
                                    && m.column < inner_x + left_w
                                    && m.row > inner_y
                                    && m.row < inner_y + avail_h.saturating_sub(1)
                                {
                                    let clicked_row = (m.row - inner_y - 1) as usize;
                                    let global_idx = tree_scroll + clicked_row;
                                    if global_idx < visible_nodes.len() {
                                        selected_node_idx = global_idx;
                                        if let Some(node) = visible_nodes.get(global_idx) {
                                            if node.has_children {
                                                if expanded_paths.contains(&node.path) {
                                                    expanded_paths.remove(&node.path);
                                                } else {
                                                    expanded_paths.insert(node.path.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Event::Key(k) => {
                            if k.modifiers.contains(KeyModifiers::CONTROL) {
                                match k.code {
                                    KeyCode::Char('s') => {
                                        if let Err(e) = save_menu(&menu_path, &menu) {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Error".to_string()),
                                                message: format!("Failed to save menu: {}", e),
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 3.0,
                                            });
                                        } else {
                                            is_modified = false;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Save Menu".to_string()),
                                                message: "Saved menu to disk".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.0,
                                            });
                                        }
                                        continue;
                                    }
                                    KeyCode::Char('a') => {
                                        active_modal =
                                            ActiveModal::AsciiPicker(AsciiGridModal::new(
                                                t!("common.dialogs.ascii_table_title"),
                                                196,
                                            ));
                                        continue;
                                    }
                                    KeyCode::Char('n') => {
                                        active_modal = ActiveModal::GlyphSearch(InputModal::new(
                                            t!("common.dialogs.search_glyphs_title"),
                                            t!("common.dialogs.search_glyphs_prompt"),
                                            "",
                                        ));
                                        continue;
                                    }
                                    KeyCode::Char('p') => {
                                        active_modal = ActiveModal::Help(HelpModal::new(
                                            "Placeholders & Macros",
                                            PLACEHOLDER_HELP_TEXT,
                                        ));
                                        continue;
                                    }
                                    KeyCode::Char('t') => {
                                        if let Some(node) = visible_nodes.get(selected_node_idx) {
                                            active_modal = ActiveModal::TypePicker(
                                                ItemTypePickerModal::new("command"),
                                                Some(node.path.clone()),
                                            );
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            match k.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if selected_node_idx > 0 {
                                        selected_node_idx -= 1;
                                        inspector_scroll = 0;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if selected_node_idx + 1 < visible_nodes.len() {
                                        selected_node_idx += 1;
                                        inspector_scroll = 0;
                                    }
                                }
                                KeyCode::Char(' ') | KeyCode::Right => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if node.has_children {
                                            if expanded_paths.contains(&node.path) {
                                                expanded_paths.remove(&node.path);
                                            } else {
                                                expanded_paths.insert(node.path.clone());
                                            }
                                        }
                                    }
                                }
                                KeyCode::Left => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if node.has_children && expanded_paths.contains(&node.path)
                                        {
                                            expanded_paths.remove(&node.path);
                                        } else if node.path.len() > 1 {
                                            let parent_path = &node.path[..node.path.len() - 1];
                                            if let Some(p_idx) = visible_nodes
                                                .iter()
                                                .position(|n| n.path == parent_path)
                                            {
                                                selected_node_idx = p_idx;
                                            }
                                        }
                                    }
                                }
                                KeyCode::F(1) => {
                                    active_modal = ActiveModal::Help(HelpModal::new(
                                        "Visual Menu Editor Guide",
                                        ITEM_EDIT_HELP_TEXT,
                                    ));
                                }
                                KeyCode::F(2) => {
                                    toggle_all_nodes(&menu, &mut expanded_paths);
                                    toast_rack.add(ToastNotification {
                                        title: Some("Menu Editor".to_string()),
                                        message: "Toggled all branches".to_string(),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                }
                                KeyCode::F(3) => {
                                    active_modal = ActiveModal::AsciiPicker(AsciiGridModal::new(
                                        t!("common.dialogs.ascii_table_title"),
                                        196,
                                    ));
                                }
                                KeyCode::F(4) => {
                                    active_modal = ActiveModal::GlyphSearch(InputModal::new(
                                        t!("common.dialogs.search_glyphs_title"),
                                        t!("common.dialogs.search_glyphs_prompt"),
                                        "",
                                    ));
                                }
                                KeyCode::F(5) => {
                                    active_modal = ActiveModal::Help(HelpModal::new(
                                        "Placeholders & Macros",
                                        PLACEHOLDER_HELP_TEXT,
                                    ));
                                }
                                KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Insert => {
                                    active_modal = ActiveModal::TypePicker(
                                        ItemTypePickerModal::new("command"),
                                        None,
                                    );
                                }
                                KeyCode::Char('e') | KeyCode::Enter => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        active_modal = ActiveModal::ItemEdit(
                                            Box::new(ItemEditModal::new(&node.item)),
                                            node.path.clone(),
                                            None,
                                        );
                                    }
                                }
                                KeyCode::Char('t') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        active_modal = ActiveModal::TypePicker(
                                            ItemTypePickerModal::new("command"),
                                            Some(node.path.clone()),
                                        );
                                    }
                                }
                                KeyCode::Char('d') | KeyCode::Char('D') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if let Some(new_p) = menuedit_cli::duplicate_item(
                                            &mut menu,
                                            &node.path,
                                            &mut expanded_paths,
                                        ) {
                                            is_modified = true;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Duplicated item".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                            let new_vis = flatten_tree(&menu, &expanded_paths);
                                            if let Some(pos) =
                                                new_vis.iter().position(|n| n.path == new_p)
                                            {
                                                selected_node_idx = pos;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Delete => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        active_modal = ActiveModal::ConfirmDelete(
                                            ConfirmModal::new(
                                                t!("menuedit.delete_item_title"),
                                                t!(
                                                    "menuedit.delete_item_confirm",
                                                    label => &node.item.label
                                                ),
                                            ),
                                            node.path.clone(),
                                        );
                                    }
                                }
                                KeyCode::Char('m') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if let Some(new_p) =
                                            move_down_sibling(&mut menu, &node.path)
                                        {
                                            is_modified = true;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Moved item down".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                            let new_vis = flatten_tree(&menu, &expanded_paths);
                                            if let Some(pos) =
                                                new_vis.iter().position(|n| n.path == new_p)
                                            {
                                                selected_node_idx = pos;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('M') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if let Some(new_p) = move_up_sibling(&mut menu, &node.path)
                                        {
                                            is_modified = true;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Moved item up".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                            let new_vis = flatten_tree(&menu, &expanded_paths);
                                            if let Some(pos) =
                                                new_vis.iter().position(|n| n.path == new_p)
                                            {
                                                selected_node_idx = pos;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('>') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if let Some(new_p) =
                                            indent_item(&mut menu, &node.path, &mut expanded_paths)
                                        {
                                            is_modified = true;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Indented item into submenu".to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                            let new_vis = flatten_tree(&menu, &expanded_paths);
                                            if let Some(pos) =
                                                new_vis.iter().position(|n| n.path == new_p)
                                            {
                                                selected_node_idx = pos;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('<') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        if let Some(new_p) = outdent_item(&mut menu, &node.path) {
                                            is_modified = true;
                                            toast_rack.add(ToastNotification {
                                                title: Some("Menu Editor".to_string()),
                                                message: "Outdented item to parent level"
                                                    .to_string(),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                            let new_vis = flatten_tree(&menu, &expanded_paths);
                                            if let Some(pos) =
                                                new_vis.iter().position(|n| n.path == new_p)
                                            {
                                                selected_node_idx = pos;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('s') => {
                                    if let Err(e) = save_menu(&menu_path, &menu) {
                                        toast_rack.add(ToastNotification {
                                            title: Some("Error".to_string()),
                                            message: format!("Failed to save menu: {}", e),
                                            severity: ToastSeverity::Error,
                                            timeout_secs: 3.0,
                                        });
                                    } else {
                                        is_modified = false;
                                        toast_rack.add(ToastNotification {
                                            title: Some("Save Menu".to_string()),
                                            message: "Saved menu to disk".to_string(),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 2.0,
                                        });
                                    }
                                }
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    if is_modified {
                                        active_modal = ActiveModal::ConfirmExit(
                                            ConfirmModal::new(
                                                t!("common.dialogs.unsaved_changes_title"),
                                                t!("common.dialogs.unsaved_changes_msg"),
                                            )
                                            .with_default_cancel(),
                                        );
                                    } else {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else if matches!(active_modal, ActiveModal::None) {
            let _ = footer_hover.check_hover_timeout(has_hidden && area.height >= 12);
        }

        if last_marquee_tick.elapsed() >= marquee_interval {
            if matches!(active_modal, ActiveModal::None) {
                marquee_tracker.tick();
            }
            last_marquee_tick = std::time::Instant::now();
        }
    }

    Ok(())
}
