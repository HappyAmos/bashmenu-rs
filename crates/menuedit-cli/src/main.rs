//! Terminal visual menu tree editor for `bashmenu.mnu`.

use anyhow::Result;
use bashmenu_core::{MenuItem, MenuItemType, Submenu};
use bashmenu_tui::{
    install_panic_hook, ConfirmModal, InputModal, ResolvedThemeStyles, TerminalGuard,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use menuedit_cli::MenuEditorState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "menuedit",
    author,
    version,
    about = "Visual menu tree editor for BashMenu"
)]
struct Cli {
    /// Path to menu definition file
    #[arg(short, long, default_value = "bashmenu.mnu")]
    file: PathBuf,
}

fn resolve_menu_path(target: &Path) -> PathBuf {
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
    AddLabel(InputModal),
    AddCommand(String, InputModal),
    EditLabel(InputModal),
    EditCommand(String, InputModal),
    ConfirmDelete(ConfirmModal),
    ConfirmExit(ConfirmModal),
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let menu_path = resolve_menu_path(&cli.file);
    let mut state = MenuEditorState::new(menu_path);

    let mut guard = TerminalGuard::init()?;
    let styles = ResolvedThemeStyles::default();
    let mut active_modal = ActiveModal::None;

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            let current = state.current_level();
            let title = format!(
                " [ MenuEdit: {} (Level {}) {} ] ",
                current.title,
                state.stack.len(),
                if state.is_modified { "(Modified)" } else { "" }
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
            let inner_y = area.y + 2;
            let avail_w = area.width.saturating_sub(4);
            let avail_h = area.height.saturating_sub(6);

            // Render Menu Items List
            for (idx, item) in current.items.iter().enumerate().take(avail_h as usize) {
                let row_y = inner_y + idx as u16;
                let is_sel = idx == current.selected_idx;

                let cursor = if is_sel { "▶ " } else { "  " };
                let row_style = if is_sel {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let line = if item.is_divider() {
                    Line::from(vec![
                        Span::styled(cursor, row_style),
                        Span::styled(
                            "──────────────── [Divider] ────────────────",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                } else {
                    let type_badge = match item.r#type {
                        MenuItemType::Submenu => " [Submenu »]",
                        MenuItemType::Command => " [Command]",
                        MenuItemType::Exit => " [Exit]",
                        _ => " [Item]",
                    };

                    let icon_str = item.icon.as_deref().unwrap_or("  ");
                    let cmd_info = if let Some(cmd) = &item.command {
                        format!(" (cmd: {})", cmd)
                    } else {
                        String::new()
                    };

                    Line::from(vec![
                        Span::styled(cursor, row_style),
                        Span::styled(format!("{:<3} ", icon_str), row_style),
                        Span::styled(&item.label, row_style),
                        Span::styled(
                            type_badge,
                            if is_sel {
                                row_style
                            } else {
                                Style::default().fg(Color::Yellow)
                            },
                        ),
                        Span::styled(cmd_info, Style::default().fg(Color::DarkGray)),
                    ])
                };

                buf.set_line(inner_x, row_y, &line, avail_w);
            }

            // Render Item Details Panel
            let details_y = area.y + area.height - 3;
            if let Some(sel_item) = current.items.get(current.selected_idx) {
                let details_text = format!(
                    " Selected: '{}' | Type: {:?} | Command: {}",
                    sel_item.label,
                    sel_item.r#type,
                    sel_item.command.as_deref().unwrap_or("<none>")
                );
                buf.set_string(
                    inner_x,
                    details_y,
                    details_text,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::DIM),
                );
            }

            // Render Footer Status Bar
            let footer_y = area.y + area.height - 1;
            let footer_help =
                " [a] Add | [e] Edit | [d] Del | [K/J] Move | [Enter] Open | [Esc] Back | [Ctrl+S] Save ";
            buf.set_string(
                inner_x,
                footer_y,
                footer_help,
                Style::default().fg(Color::DarkGray),
            );

            if !state.status_message.is_empty() {
                let msg_x = (area.x + area.width)
                    .saturating_sub(state.status_message.len() as u16 + 3);
                buf.set_string(
                    msg_x,
                    footer_y,
                    &state.status_message,
                    Style::default().fg(Color::Green),
                );
            }

            // Render Active Modal
            match &mut active_modal {
                ActiveModal::AddLabel(m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::AddCommand(_, m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::EditLabel(m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::EditCommand(_, m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::ConfirmDelete(m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::ConfirmExit(m) => {
                    m.render(area, &styles, buf);
                }
                ActiveModal::None => {}
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            match &mut active_modal {
                ActiveModal::AddLabel(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(lbl) = res {
                                let trimmed = lbl.trim().to_string();
                                if !trimmed.is_empty() {
                                    active_modal = ActiveModal::AddCommand(
                                        trimmed,
                                        InputModal::new(
                                            "Add Command",
                                            "Enter shell command (leave blank for submenu):",
                                            "",
                                        ),
                                    );
                                    continue;
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::AddCommand(label, modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(cmd) = res {
                                let trimmed_cmd = cmd.trim();
                                let (item_type, command, submenu) = if trimmed_cmd.is_empty() {
                                    (
                                        MenuItemType::Submenu,
                                        None,
                                        Some(Submenu {
                                            title: label.clone(),
                                            items: Vec::new(),
                                        }),
                                    )
                                } else {
                                    (MenuItemType::Command, Some(trimmed_cmd.to_string()), None)
                                };

                                let new_item = MenuItem {
                                    label: label.clone(),
                                    r#type: item_type,
                                    command,
                                    submenu,
                                    ..Default::default()
                                };
                                state.add_item(new_item);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::EditLabel(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(lbl) = res {
                                let trimmed = lbl.trim().to_string();
                                if !trimmed.is_empty() {
                                    let cur_cmd = state
                                        .current_items()
                                        .get(state.selected_index())
                                        .and_then(|i| i.command.clone())
                                        .unwrap_or_default();
                                    active_modal = ActiveModal::EditCommand(
                                        trimmed,
                                        InputModal::new(
                                            "Edit Command",
                                            "Enter shell command:",
                                            &cur_cmd,
                                        ),
                                    );
                                    continue;
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::EditCommand(new_label, modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(cmd) = res {
                                let trimmed_cmd = cmd.trim();
                                let cmd_opt = if trimmed_cmd.is_empty() {
                                    None
                                } else {
                                    Some(trimmed_cmd.to_string())
                                };
                                let cur_icon = state
                                    .current_items()
                                    .get(state.selected_index())
                                    .and_then(|i| i.icon.clone());
                                state.edit_selected(new_label.clone(), cmd_opt, cur_icon);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::ConfirmDelete(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                state.delete_selected();
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::ConfirmExit(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                }
                ActiveModal::None => {
                    if let Event::Key(k) = ev {
                        if k.modifiers.contains(KeyModifiers::CONTROL)
                            && k.code == KeyCode::Char('s')
                        {
                            let _ = state.save_to_disk();
                            continue;
                        }

                        match k.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.move_up();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.move_down();
                            }
                            // Shift+K or 'K' moves item up
                            KeyCode::Char('K') => {
                                state.swap_up();
                            }
                            // Shift+J or 'J' moves item down
                            KeyCode::Char('J') => {
                                state.swap_down();
                            }
                            // 'a' adds a new item
                            KeyCode::Char('a') => {
                                active_modal = ActiveModal::AddLabel(InputModal::new(
                                    "Add Menu Item",
                                    "Enter item label:",
                                    "",
                                ));
                            }
                            // 'e' edits currently selected item
                            KeyCode::Char('e') => {
                                if let Some(item) =
                                    state.current_items().get(state.selected_index())
                                {
                                    if !item.is_divider() {
                                        active_modal = ActiveModal::EditLabel(InputModal::new(
                                            "Edit Menu Item",
                                            "Enter new label:",
                                            &item.label,
                                        ));
                                    }
                                }
                            }
                            // 'd' deletes currently selected item
                            KeyCode::Char('d') => {
                                if let Some(item) =
                                    state.current_items().get(state.selected_index())
                                {
                                    active_modal = ActiveModal::ConfirmDelete(ConfirmModal::new(
                                        "Delete Item",
                                        format!("Delete item '{}'?", item.label),
                                    ));
                                }
                            }
                            // Enter dives into submenu
                            KeyCode::Enter => {
                                state.enter_submenu();
                            }
                            // Esc leaves submenu or prompts to exit
                            KeyCode::Esc => {
                                if !state.leave_submenu() {
                                    if state.is_modified {
                                        active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                            "Unsaved Changes",
                                            "Discard unsaved changes and exit?",
                                        ));
                                    } else {
                                        break;
                                    }
                                }
                            }
                            KeyCode::Char('q') => {
                                if state.is_modified {
                                    active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                        "Unsaved Changes",
                                        "Discard unsaved changes and exit?",
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
