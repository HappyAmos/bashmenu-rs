//! Visual split-screen menu tree editor for `bashmenu.mnu`.

use anyhow::Result;
use bashmenu_core::{
    load_config, load_menu, save_menu, MenuItem, MenuSchema, ToastNotification, ToastSeverity,
};
use bashmenu_tui::{
    compute_modal_inner, install_panic_hook, AsciiGridModal, ConfirmModal, FilePickerModal,
    HelpModal, InputModal, ResolvedThemeStyles, TerminalGuard, ToastRack,
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
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuEditFooterAction {
    Help,
    ToggleAll,
    Ascii,
    Glyphs,
    Placeholders,
    Add,
    Edit,
    Type,
    Toggle,
    Delete,
    MoveDown,
    MoveUp,
    Indent,
    Outdent,
    Save,
    Exit,
}

#[derive(Clone, Copy)]
struct FooterHitRegion {
    row: u16,
    start_x: u16,
    end_x: u16,
    action: MenuEditFooterAction,
}

#[derive(Parser, Debug)]
#[command(
    name = "menuedit",
    author,
    version,
    about = "Interactive visual menu editor with side-by-side tree and property inspector"
)]
struct Cli {
    /// Path to menu definition file
    #[arg(short, long, default_value = "bashmenu.mnu")]
    file: PathBuf,

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
}

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

fn resolve_script_path(script_name: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BASHMENU_DIR") {
        let p = PathBuf::from(dir).join("scripts").join(script_name);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home)
            .join(".config/bashmenu/scripts")
            .join(script_name);
        if p.exists() {
            return Some(p);
        }
    }
    let local = PathBuf::from("scripts").join(script_name);
    if local.exists() {
        return Some(local);
    }
    None
}

enum ChildModal {
    TypePicker(ItemTypePickerModal),
    FilePicker(FilePickerModal, BrowseTarget),
    Help(HelpModal),
    AsciiPicker(AsciiGridModal),
    GlyphSearch(InputModal),
}

enum ActiveModal {
    None,
    TypePicker(ItemTypePickerModal, Option<Vec<usize>>), // (modal, target_path)
    ItemEdit(Box<ItemEditModal>, Vec<usize>, Option<Box<ChildModal>>),
    ConfirmDelete(ConfirmModal, Vec<usize>), // (modal, target_path)
    ConfirmExit(ConfirmModal),
    Help(HelpModal),
    AsciiPicker(AsciiGridModal),
    GlyphSearch(InputModal),
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let menu_path = resolve_menu_path(&cli.file, cli.local);
    let mut menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        MenuSchema::default()
    };

    let cfg_path = resolve_menu_path(Path::new("bashmenu.yml"), cli.local);
    let cfg = load_config(&cfg_path).ok();
    let themes_path = resolve_menu_path(Path::new("bashmenu.themes"), cli.local);
    let themes = bashmenu_core::load_themes(&themes_path).ok();
    let use_nerd = cfg
        .as_ref()
        .and_then(|c| c.settings.use_nerd_fonts)
        .unwrap_or(true);
    let styles = if let (Some(c), Some(th)) = (&cfg, &themes) {
        if let Some(t) = th.get(&c.theme) {
            ResolvedThemeStyles::from_theme(t, use_nerd)
        } else {
            ResolvedThemeStyles::default()
        }
    } else {
        ResolvedThemeStyles::default()
    };
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

    loop {
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

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            // Root window title
            let file_name = menu_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "bashmenu.mnu".to_string());
            let mod_str = if is_modified { " (Modified)" } else { "" };
            let title = format!("─[ Visual Menu Editor - {}{} ]", file_name, mod_str);

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
            let avail_h = area.height.saturating_sub(4);

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
                let label_text = if node.item.is_divider() {
                    "──────────────── [Divider]".to_string()
                } else {
                    node.item.label.clone()
                };

                let row_style = if is_sel {
                    styles.highlight.add_modifier(Modifier::BOLD)
                } else {
                    styles.text
                };

                let line = Line::from(vec![
                    Span::styled(indent, row_style),
                    Span::styled(
                        expand_icon,
                        if is_sel {
                            row_style
                        } else {
                            Style::default().fg(Color::Cyan)
                        },
                    ),
                    Span::styled(
                        format!("{:<5} ", badge),
                        if is_sel {
                            row_style
                        } else {
                            Style::default().fg(Color::Yellow)
                        },
                    ),
                    Span::styled(label_text, row_style),
                ]);

                buf.set_line(tree_rect.x + 1, row_y, &line, tree_content_w);
            }

            // Right Panel: Property Inspector
            let insp_x = inner_x + left_w;
            let insp_rect = Rect::new(insp_x, inner_y, right_w, avail_h.saturating_sub(1));
            let insp_block = Block::default()
                .borders(Borders::ALL)
                .title("───[ Property Inspector ]")
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
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    } else if line.starts_with("Title/Label:")
                        || line.starts_with("Expanded Title:")
                    {
                        Style::default().fg(Color::White)
                    } else if line.starts_with("Icon/Glyph:") || line.starts_with("Expanded Icon:")
                    {
                        Style::default().fg(Color::Cyan)
                    } else if line.starts_with("Action/Target:")
                        || line.starts_with("Expanded Action:")
                    {
                        Style::default().fg(Color::Yellow)
                    } else if line.starts_with("Config Key:") {
                        Style::default().fg(Color::Green)
                    } else if line.starts_with("Message/Prompt:") {
                        Style::default().fg(Color::Blue)
                    } else if line.starts_with("Preview:") {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        styles.text
                    };

                    let truncated: String = line.chars().take(insp_content_w as usize).collect();
                    buf.set_string(insp_rect.x + 1, y, &truncated, line_style);
                }
            } else {
                buf.set_string(
                    insp_rect.x + 2,
                    insp_rect.y + 2,
                    "No item selected",
                    styles.text,
                );
            }

            // Footer (2 Lines matching Python Menuedit, raised above bottom border)
            let footer_y1 = (area.y + area.height).saturating_sub(3);
            let footer_y2 = (area.y + area.height).saturating_sub(2);

            let items_r1 = [
                ("[F1] Help", MenuEditFooterAction::Help),
                ("[F2] Toggle All", MenuEditFooterAction::ToggleAll),
                ("[F3] ASCII", MenuEditFooterAction::Ascii),
                ("[F4] Glyphs", MenuEditFooterAction::Glyphs),
                ("[F5] Placeholders", MenuEditFooterAction::Placeholders),
                ("[a] Add", MenuEditFooterAction::Add),
                ("[e/ENTER] Edit", MenuEditFooterAction::Edit),
                ("[t] Type", MenuEditFooterAction::Type),
            ];
            let items_r2 = [
                ("[SPACE] Toggle", MenuEditFooterAction::Toggle),
                ("[d] Delete", MenuEditFooterAction::Delete),
                ("[m] Move Dn", MenuEditFooterAction::MoveDown),
                ("[M] Move Up", MenuEditFooterAction::MoveUp),
                ("[>] Indent", MenuEditFooterAction::Indent),
                ("[<] Outdent", MenuEditFooterAction::Outdent),
                ("[s] Save", MenuEditFooterAction::Save),
                ("[ESC/q] Exit", MenuEditFooterAction::Exit),
            ];

            let labels_r1: Vec<&str> = items_r1.iter().map(|(lbl, _)| *lbl).collect();
            let labels_r2: Vec<&str> = items_r2.iter().map(|(lbl, _)| *lbl).collect();

            let (line_r1, ranges_r1) =
                bashmenu_tui::spread_gutter_line_layout(&labels_r1, avail_w as usize);
            let (line_r2, ranges_r2) =
                bashmenu_tui::spread_gutter_line_layout(&labels_r2, avail_w as usize);

            buf.set_string(
                inner_x,
                footer_y1,
                &line_r1,
                Style::default().fg(Color::DarkGray),
            );
            for (idx, (start, end)) in ranges_r1.into_iter().enumerate() {
                if idx < items_r1.len() {
                    let (label, action) = items_r1[idx];
                    let screen_start = inner_x + start as u16;
                    let screen_end = inner_x + end as u16;
                    buf.set_string(screen_start, footer_y1, label, styles.accent);
                    footer_hit_regions.push(FooterHitRegion {
                        row: footer_y1,
                        start_x: screen_start,
                        end_x: screen_end,
                        action,
                    });
                }
            }

            buf.set_string(
                inner_x,
                footer_y2,
                &line_r2,
                Style::default().fg(Color::DarkGray),
            );
            for (idx, (start, end)) in ranges_r2.into_iter().enumerate() {
                if idx < items_r2.len() {
                    let (label, action) = items_r2[idx];
                    let screen_start = inner_x + start as u16;
                    let screen_end = inner_x + end as u16;
                    buf.set_string(screen_start, footer_y2, label, styles.accent);
                    footer_hit_regions.push(FooterHitRegion {
                        row: footer_y2,
                        start_x: screen_start,
                        end_x: screen_end,
                        action,
                    });
                }
            }

            // Render Active Modals
            match &mut active_modal {
                ActiveModal::None => {}
                ActiveModal::TypePicker(modal, _) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::ItemEdit(modal, _, child_opt) => {
                    modal.render(area, &styles, buf);
                    if let Some(child) = child_opt {
                        match child.as_mut() {
                            ChildModal::TypePicker(m) => {
                                m.render(area, &styles, buf);
                            }
                            ChildModal::FilePicker(m, _) => {
                                m.render(area, &styles, buf);
                            }
                            ChildModal::Help(m) => {
                                m.render(area, &styles, buf);
                            }
                            ChildModal::AsciiPicker(m) => {
                                m.render(area, &styles, buf);
                            }
                            ChildModal::GlyphSearch(m) => {
                                m.render(area, &styles, buf);
                            }
                        }
                    }
                }
                ActiveModal::ConfirmDelete(modal, _) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::ConfirmExit(modal) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::Help(modal) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::AsciiPicker(modal) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::GlyphSearch(modal) => {
                    modal.render(area, &styles, buf);
                }
            }

            // Render floating toast notifications atop everything
            toast_rack.render(area, &styles, buf);
        })?;

        toast_rack.tick();

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

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
                                    let inner = compute_modal_inner(area, 60, 24);
                                    let grid_start = (inner.x + 2, inner.y + 2);
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
                                *child_opt = Some(Box::new(ChildModal::AsciiPicker(
                                    AsciiGridModal::new("ASCII Character Table", 196),
                                )));
                            }
                            ItemEditResult::ShowGlyphs => {
                                *child_opt =
                                    Some(Box::new(ChildModal::GlyphSearch(InputModal::new(
                                        "Search Nerd Font & Emoji",
                                        "Enter glyph search term (e.g. arrow, folder, python):",
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
                        let inner = compute_modal_inner(area, 60, 24);
                        let grid_start = (inner.x + 2, inner.y + 2);
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
                                        active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                            "Unsaved Changes",
                                            "You have unsaved changes. Save before exiting?",
                                        ));
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
                                            active_modal = ActiveModal::AsciiPicker(
                                                AsciiGridModal::new("ASCII Character Table", 196),
                                            );
                                        }
                                        MenuEditFooterAction::Glyphs => {
                                            active_modal = ActiveModal::GlyphSearch(InputModal::new(
                                                "Search Nerd Font & Emoji",
                                                "Enter glyph search term (e.g. arrow, folder, python):",
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
                                        MenuEditFooterAction::Delete => {
                                            if let Some(node) = visible_nodes.get(selected_node_idx)
                                            {
                                                active_modal = ActiveModal::ConfirmDelete(
                                                    ConfirmModal::new(
                                                        "Delete Item",
                                                        format!(
                                                            "Are you sure you want to delete '{}'?",
                                                            node.item.label
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
                                                active_modal =
                                                    ActiveModal::ConfirmExit(ConfirmModal::new(
                                                        "Unsaved Changes",
                                                        "You have unsaved changes. Save before exiting?",
                                                    ));
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
                                        active_modal = ActiveModal::AsciiPicker(
                                            AsciiGridModal::new("ASCII Character Table", 196),
                                        );
                                        continue;
                                    }
                                    KeyCode::Char('n') => {
                                        active_modal = ActiveModal::GlyphSearch(InputModal::new(
                                            "Search Nerd Font & Emoji",
                                            "Enter glyph search term (e.g. arrow, folder, python):",
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
                                        "ASCII Character Table",
                                        196,
                                    ));
                                }
                                KeyCode::F(4) => {
                                    active_modal = ActiveModal::GlyphSearch(InputModal::new(
                                        "Search Nerd Font & Emoji",
                                        "Enter glyph search term (e.g. arrow, folder, python):",
                                        "",
                                    ));
                                }
                                KeyCode::F(5) => {
                                    active_modal = ActiveModal::Help(HelpModal::new(
                                        "Placeholders & Macros",
                                        PLACEHOLDER_HELP_TEXT,
                                    ));
                                }
                                KeyCode::Char('a') => {
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
                                KeyCode::Char('d') => {
                                    if let Some(node) = visible_nodes.get(selected_node_idx) {
                                        active_modal = ActiveModal::ConfirmDelete(
                                            ConfirmModal::new(
                                                "Delete Item",
                                                format!(
                                                    "Are you sure you want to delete '{}'?",
                                                    node.item.label
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
                                        active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                            "Unsaved Changes",
                                            "You have unsaved changes. Save before exiting?",
                                        ));
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
        }
    }

    Ok(())
}

fn run_glyphs_search(term: &str) -> String {
    if let Some(script_path) = resolve_script_path("glyphs.sh") {
        if let Ok(output) = Command::new("bash").arg(script_path).arg(term).output() {
            let res = String::from_utf8_lossy(&output.stdout).to_string();
            if !res.trim().is_empty() {
                return res;
            }
        }
    }

    format!(
        "Glyph Search: '{}'\n\nUse adaptive nerd font directive in icons:\n{{nf:<char>:<nerd_hex>:<emoji>}}\n\nExamples:\n- {{nf:*:f015:🏠}} (Home)\n- {{nf:*:f07b:📁}} (Folder)\n- {{nf:*:e73c:🐍}} (Python)\n- {{nf:*:f120:💻}} (Terminal)\n- {{nf:*:f085:⚙️}} (Gear/Settings)",
        term
    )
}
