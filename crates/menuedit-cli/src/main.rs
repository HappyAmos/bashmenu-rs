//! Visual split-screen menu tree editor for `bashmenu.mnu`.

use anyhow::Result;
use bashmenu_core::{load_config, load_menu, save_menu, MenuItem, MenuSchema};
use bashmenu_tui::{
    install_panic_hook, AsciiGridModal, ConfirmModal, HelpModal, ResolvedThemeStyles, TerminalGuard,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use menuedit_cli::{
    delete_item, find_item_mut, flatten_tree, format_inspector_text, get_type_badge, indent_item,
    insert_item_after, move_down_sibling, move_up_sibling, mutate_item_type, outdent_item,
    toggle_all_nodes, ItemEditModal, ItemTypePickerModal, ITEM_EDIT_HELP_TEXT,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
}

fn resolve_menu_path(target: &Path) -> PathBuf {
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
    let fixture = PathBuf::from("tests/fixtures").join(target);
    if fixture.exists() {
        return fixture;
    }
    target.to_path_buf()
}

enum ActiveModal {
    None,
    TypePicker(ItemTypePickerModal, Option<Vec<usize>>), // (modal, target_path)
    ItemEdit(Box<ItemEditModal>, Vec<usize>),            // (modal, target_path)
    ConfirmDelete(ConfirmModal, Vec<usize>),             // (modal, target_path)
    ConfirmExit(ConfirmModal),
    Help(HelpModal),
    AsciiPicker(AsciiGridModal),
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let menu_path = resolve_menu_path(&cli.file);
    let mut menu = if menu_path.exists() {
        load_menu(&menu_path).unwrap_or_default()
    } else {
        MenuSchema::default()
    };

    let cfg = load_config("bashmenu.yml").ok();
    let mut guard = TerminalGuard::init()?;
    let styles = ResolvedThemeStyles::default();

    let mut expanded_paths = HashSet::new();
    let mut selected_node_idx = 0;
    let mut tree_scroll = 0;
    let mut inspector_scroll = 0;
    let mut is_modified = false;
    let mut status_message = "Ready".to_string();
    let mut active_modal = ActiveModal::None;

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        let visible_nodes = flatten_tree(&menu, &expanded_paths);
        if !visible_nodes.is_empty() && selected_node_idx >= visible_nodes.len() {
            selected_node_idx = visible_nodes.len() - 1;
        }

        // Keep tree scroll visible
        let max_tree_rows = area.height.saturating_sub(6) as usize;
        if selected_node_idx < tree_scroll {
            tree_scroll = selected_node_idx;
        } else if selected_node_idx >= tree_scroll + max_tree_rows && max_tree_rows > 0 {
            tree_scroll = selected_node_idx.saturating_sub(max_tree_rows - 1);
        }

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            // Root window title
            let file_name = menu_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "bashmenu.mnu".to_string());
            let title = format!(
                " [ Visual Menu Editor - {} {} ] ",
                file_name,
                if is_modified { "(Modified)" } else { "" }
            );

            let root_block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Cyan));
            root_block.render(area, buf);

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
                .title(" [ Menu Structure ] ")
                .border_style(Style::default().fg(Color::Blue));
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
                .title(" ── Property Inspector ── ")
                .border_style(Style::default().fg(Color::Blue));
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
                    } else if line.starts_with("Title/Label:") {
                        Style::default().fg(Color::White)
                    } else if line.starts_with("Icon/Glyph:") {
                        Style::default().fg(Color::Cyan)
                    } else if line.starts_with("Action/Target:") {
                        Style::default().fg(Color::Yellow)
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

            // Footer (2 Lines)
            let footer_y1 = area.y + area.height - 2;
            let footer_y2 = area.y + area.height - 1;
            let row1_text = " [F1] Help | [F2] Toggle All | [F3] ASCII | [F5] Placeholders | [a] Add | [e/ENTER] Edit | [t] Type ";
            let row2_text = " [SPACE] Toggle | [d] Delete | [m] Move Dn | [M] Move Up | [>] Indent | [<] Outdent | [s] Save | [ESC/q] Exit ";

            buf.set_string(
                inner_x,
                footer_y1,
                row1_text,
                Style::default().fg(Color::DarkGray),
            );
            buf.set_string(
                inner_x,
                footer_y2,
                row2_text,
                Style::default().fg(Color::DarkGray),
            );

            if !status_message.is_empty() {
                let msg_x = (area.x + area.width).saturating_sub(status_message.len() as u16 + 3);
                buf.set_string(
                    msg_x,
                    footer_y2,
                    &status_message,
                    Style::default().fg(Color::Green),
                );
            }

            // Render Active Modals
            match &mut active_modal {
                ActiveModal::None => {}
                ActiveModal::TypePicker(modal, _) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::ItemEdit(modal, _) => {
                    modal.render(area, &styles, buf);
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
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            match &mut active_modal {
                ActiveModal::TypePicker(modal, target_path_opt) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(chosen_type) = res {
                                if let Some(path) = target_path_opt.take() {
                                    // Mutate existing item type
                                    if let Some(item) = find_item_mut(&mut menu, &path) {
                                        mutate_item_type(item, &chosen_type);
                                        is_modified = true;
                                        status_message = format!("Mutated type to {}", chosen_type);
                                    }
                                } else {
                                    // Add new item
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
                                    status_message = format!("Added new {}", chosen_type);
                                    active_modal = ActiveModal::ItemEdit(
                                        Box::new(ItemEditModal::new(&new_item)),
                                        new_path,
                                    );
                                    continue;
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::ItemEdit(modal, path) => {
                    if let Event::Key(k) = ev {
                        let (res, request_change_type) = modal.handle_key(k.code, k.modifiers);
                        if request_change_type {
                            let cur_type = format!("{:?}", modal.item.r#type);
                            active_modal = ActiveModal::TypePicker(
                                ItemTypePickerModal::new(&cur_type),
                                Some(path.clone()),
                            );
                            continue;
                        }
                        if let Some(opt_item) = res {
                            if let Some(updated) = opt_item {
                                if let Some(item_slot) = find_item_mut(&mut menu, path) {
                                    *item_slot = updated;
                                    is_modified = true;
                                    status_message = "Updated item properties".to_string();
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::ConfirmDelete(modal, path) => {
                    if let Event::Key(k) = ev {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                if let Some(deleted) = delete_item(&mut menu, path) {
                                    is_modified = true;
                                    status_message = format!("Deleted '{}'", deleted.label);
                                    selected_node_idx = selected_node_idx.saturating_sub(1);
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::ConfirmExit(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                let _ = save_menu(&menu_path, &menu);
                                break;
                            } else {
                                break; // Discard changes and exit
                            }
                        }
                    }
                }
                ActiveModal::Help(modal) => {
                    if let Event::Key(k) = ev {
                        if modal.handle_key(k.code) {
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::AsciiPicker(modal) => {
                    if let Event::Key(k) = ev {
                        if modal.handle_key(k.code).is_some() {
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::None => {
                    if let Event::Key(k) = ev {
                        // Check for Ctrl combinations
                        if k.modifiers.contains(KeyModifiers::CONTROL) {
                            match k.code {
                                KeyCode::Char('s') => {
                                    if let Err(e) = save_menu(&menu_path, &menu) {
                                        status_message = format!("Save error: {}", e);
                                    } else {
                                        is_modified = false;
                                        status_message = "Saved menu to disk".to_string();
                                    }
                                    continue;
                                }
                                KeyCode::Char('a') => {
                                    active_modal = ActiveModal::AsciiPicker(AsciiGridModal::new(
                                        "ASCII Character Table",
                                        196,
                                    ));
                                    continue;
                                }
                                KeyCode::Char('p') => {
                                    active_modal = ActiveModal::Help(HelpModal::new(
                                        "Placeholders & Macros",
                                        ITEM_EDIT_HELP_TEXT,
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
                                    if node.has_children && expanded_paths.contains(&node.path) {
                                        expanded_paths.remove(&node.path);
                                    } else if node.path.len() > 1 {
                                        // Jump to parent node
                                        let parent_path = &node.path[..node.path.len() - 1];
                                        if let Some(p_idx) =
                                            visible_nodes.iter().position(|n| n.path == parent_path)
                                        {
                                            selected_node_idx = p_idx;
                                        }
                                    }
                                }
                            }
                            KeyCode::F(2) => {
                                toggle_all_nodes(&menu, &mut expanded_paths);
                                status_message = "Toggled all branches".to_string();
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
                                    if let Some(new_p) = move_down_sibling(&mut menu, &node.path) {
                                        is_modified = true;
                                        status_message = "Moved item down".to_string();
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
                                    if let Some(new_p) = move_up_sibling(&mut menu, &node.path) {
                                        is_modified = true;
                                        status_message = "Moved item up".to_string();
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
                                        status_message = "Indented item into submenu".to_string();
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
                                        status_message =
                                            "Outdented item to parent level".to_string();
                                        let new_vis = flatten_tree(&menu, &expanded_paths);
                                        if let Some(pos) =
                                            new_vis.iter().position(|n| n.path == new_p)
                                        {
                                            selected_node_idx = pos;
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
                            KeyCode::F(3) => {
                                active_modal = ActiveModal::AsciiPicker(AsciiGridModal::new(
                                    "ASCII Character Table",
                                    196,
                                ));
                            }
                            KeyCode::F(5) => {
                                active_modal = ActiveModal::Help(HelpModal::new(
                                    "Placeholders & Macros",
                                    ITEM_EDIT_HELP_TEXT,
                                ));
                            }
                            KeyCode::Char('s') => {
                                if let Err(e) = save_menu(&menu_path, &menu) {
                                    status_message = format!("Save error: {}", e);
                                } else {
                                    is_modified = false;
                                    status_message = "Saved menu to disk".to_string();
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
                }
            }
        }
    }

    Ok(())
}
