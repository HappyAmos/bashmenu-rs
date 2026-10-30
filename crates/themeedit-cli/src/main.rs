//! Terminal visual theme editor for `bashmenu.themes`.

use anyhow::Result;
use bashmenu_tui::{
    install_panic_hook, AsciiGridModal, ColorGridModal, ConfirmModal, InputModal,
    ResolvedThemeStyles, TerminalGuard,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::{Path, PathBuf};
use std::time::Duration;
use themeedit_cli::{ThemeEditorState, TokenKind};

#[derive(Parser, Debug)]
#[command(
    name = "themeedit",
    author,
    version,
    about = "Visual theme and palette editor for BashMenu"
)]
struct Cli {
    /// Target theme name to edit
    #[arg(value_name = "THEME")]
    theme: Option<String>,

    /// Path to themes definition file
    #[arg(short, long, default_value = "bashmenu.themes")]
    file: PathBuf,
}

fn resolve_themes_path(target: &Path) -> PathBuf {
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
    let fixture = PathBuf::from("tests/fixtures").join(target);
    if fixture.exists() {
        return fixture;
    }
    target.to_path_buf()
}

enum ActiveModal {
    None,
    ColorPicker(String, ColorGridModal),
    AsciiPicker(String, AsciiGridModal),
    SaveAs(InputModal),
    ConfirmExit(ConfirmModal),
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let themes_path = resolve_themes_path(&cli.file);
    let mut state = ThemeEditorState::new(themes_path, cli.theme);

    let mut guard = TerminalGuard::init()?;
    let mut active_modal = ActiveModal::None;
    let mut last_grid_origin = (0u16, 0u16);

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        // Derive resolved styles from current edited theme for live preview
        let preview_styles = ResolvedThemeStyles::from_theme(&state.current_theme, true);

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            // Background fill
            let root_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " [ ThemeEdit: {} {} ] ",
                    state.current_name,
                    if state.is_modified { "(Modified)" } else { "" }
                ))
                .border_style(Style::default().fg(Color::Cyan));
            root_block.render(area, buf);

            if area.width < 50 || area.height < 15 {
                return;
            }

            // Split into Left Panel (Tokens) and Right Panel (Live Preview)
            let left_w = (area.width * 45 / 100).clamp(30, 42);
            let left_area = Rect::new(area.x + 2, area.y + 2, left_w - 3, area.height - 4);
            let right_x = area.x + left_w + 1;
            let right_w = (area.x + area.width).saturating_sub(right_x + 2);
            let right_area = Rect::new(right_x, area.y + 2, right_w, area.height - 4);

            // 1. Render Left Panel: Token List
            let list_title = " Theme Tokens ";
            buf.set_string(
                left_area.x,
                left_area.y,
                list_title,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

            for (idx, token) in state.tokens.iter().enumerate() {
                let row_y = left_area.y + 2 + idx as u16;
                if row_y >= left_area.y + left_area.height {
                    break;
                }

                let is_sel = idx == state.selected_idx;
                let val_str = state.get_token_value_display(token);

                let cursor = if is_sel { "▶ " } else { "  " };
                let row_style = if is_sel {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let label_text = format!("{}{:<20} {}", cursor, token.label, val_str);
                buf.set_line(
                    left_area.x,
                    row_y,
                    &Line::from(vec![Span::styled(label_text, row_style)]),
                    left_area.width,
                );
            }

            // 2. Render Right Panel: Live Preview Menu Block
            let preview_block = Block::default()
                .borders(Borders::ALL)
                .title(" [ Live Menu Preview ] ")
                .border_style(Style::default().fg(preview_styles.border))
                .style(Style::default().bg(preview_styles.background));
            preview_block.render(right_area, buf);

            let p_inner_x = right_area.x + 2;
            let p_inner_y = right_area.y + 2;
            let p_avail_w = right_area.width.saturating_sub(4);

            // Active item
            let ind = &preview_styles.indicator_symbol;
            let active_line = Line::from(vec![
                Span::styled(ind, preview_styles.highlight),
                Span::styled("  🚀 ", preview_styles.highlight),
                Span::styled("1. ", preview_styles.shortcut_key),
                Span::styled("Launch Application", preview_styles.highlight),
            ]);
            buf.set_line(p_inner_x, p_inner_y, &active_line, p_avail_w);

            // Inactive item
            let normal_line = Line::from(vec![
                Span::raw("  "),
                Span::styled("  📁 ", preview_styles.text),
                Span::styled("2. ", preview_styles.shortcut_key),
                Span::styled("Browse Directory", preview_styles.text),
                Span::styled("  »", preview_styles.accent),
            ]);
            buf.set_line(p_inner_x, p_inner_y + 1, &normal_line, p_avail_w);

            // Divider rule
            let div_str = "─".repeat(p_avail_w as usize);
            buf.set_line(
                p_inner_x,
                p_inner_y + 2,
                &Line::from(vec![Span::styled(div_str, preview_styles.divider)]),
                p_avail_w,
            );

            // Exit item
            let exit_line = Line::from(vec![
                Span::raw("  "),
                Span::styled("  ❌ ", preview_styles.text),
                Span::styled("3. ", preview_styles.shortcut_key),
                Span::styled("Exit to Terminal", preview_styles.text),
            ]);
            buf.set_line(p_inner_x, p_inner_y + 3, &exit_line, p_avail_w);

            // Gutter sample
            let p_gutter_y = right_area.y + right_area.height - 1;
            buf.set_string(
                p_inner_x,
                p_gutter_y,
                "[Esc] Back | [Ctrl+S] Save",
                preview_styles.gutter,
            );

            // 3. Render Bottom Footer Status
            let footer_y = area.y + area.height - 1;
            let footer_text =
                " [Ctrl+N] New | [Ctrl+S] Save | [Ctrl+A] Save As | [Enter] Pick | [q] Exit ";
            buf.set_string(
                area.x + 2,
                footer_y,
                footer_text,
                Style::default().fg(Color::DarkGray),
            );

            if !state.status_message.is_empty() {
                let msg_x =
                    (area.x + area.width).saturating_sub(state.status_message.len() as u16 + 3);
                buf.set_string(
                    msg_x,
                    footer_y,
                    &state.status_message,
                    Style::default().fg(Color::Green),
                );
            }

            // 4. Render Active Modal if present
            match &mut active_modal {
                ActiveModal::ColorPicker(_, modal) => {
                    let (_, origin) = modal.render(area, &preview_styles, buf);
                    last_grid_origin = origin;
                }
                ActiveModal::AsciiPicker(_, modal) => {
                    let (_, origin) = modal.render(area, &preview_styles, buf);
                    last_grid_origin = origin;
                }
                ActiveModal::SaveAs(modal) => {
                    modal.render(area, &preview_styles, buf);
                }
                ActiveModal::ConfirmExit(modal) => {
                    modal.render(area, &preview_styles, buf);
                }
                ActiveModal::None => {}
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            // Modal interaction handles events first
            match &mut active_modal {
                ActiveModal::ColorPicker(key, modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(code) = res {
                                state.set_color_token(key, code);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, last_grid_origin) {
                            if let Some(code) = res {
                                state.set_color_token(key, code);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::AsciiPicker(key, modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(ch) = res {
                                state.set_border_token(key, ch);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, last_grid_origin) {
                            if let Some(ch) = res {
                                state.set_border_token(key, ch);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::SaveAs(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(new_name) = res {
                                let trimmed = new_name.trim();
                                if !trimmed.is_empty() {
                                    state.current_name = trimmed.to_string();
                                    let _ = state.save_to_disk();
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
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                }
                ActiveModal::None => {
                    // Main editor controls
                    if let Event::Key(k) = ev {
                        // Invariant: Ctrl+S saves directly whenever theme_name is set; only untitled diverts to Save As
                        if k.modifiers.contains(KeyModifiers::CONTROL) {
                            match k.code {
                                KeyCode::Char('s') => {
                                    if state.current_name.is_empty()
                                        || state.current_name == "untitled"
                                    {
                                        active_modal = ActiveModal::SaveAs(InputModal::new(
                                            "Save Theme As",
                                            "Enter theme name:",
                                            &state.current_name,
                                        ));
                                    } else {
                                        let _ = state.save_to_disk();
                                    }
                                    continue;
                                }
                                KeyCode::Char('a') => {
                                    // Ctrl+A opens Save As
                                    active_modal = ActiveModal::SaveAs(InputModal::new(
                                        "Save Theme As",
                                        "Enter new theme name:",
                                        &state.current_name,
                                    ));
                                    continue;
                                }
                                KeyCode::Char('n') => {
                                    // Ctrl+N creates new theme
                                    state.current_name = "untitled".to_string();
                                    state.current_theme = ThemeEditorState::create_default_theme();
                                    state.is_modified = true;
                                    state.status_message = "Created new untitled theme".to_string();
                                    continue;
                                }
                                _ => {}
                            }
                        }

                        match k.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if state.selected_idx > 0 {
                                    state.selected_idx -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if state.selected_idx + 1 < state.tokens.len() {
                                    state.selected_idx += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let token = state.tokens[state.selected_idx].clone();
                                match token.kind {
                                    TokenKind::Color(key) => {
                                        active_modal = ActiveModal::ColorPicker(
                                            key,
                                            ColorGridModal::new("Pick 256 Color", 255),
                                        );
                                    }
                                    TokenKind::Border(key) => {
                                        active_modal = ActiveModal::AsciiPicker(
                                            key,
                                            AsciiGridModal::new("Pick CP437 Border Char", 196),
                                        );
                                    }
                                    TokenKind::Indicator => {
                                        // Toggle indicator
                                        state.current_theme.indicator =
                                            if state.current_theme.indicator.as_deref()
                                                == Some("▶ ")
                                            {
                                                Some("> ".to_string())
                                            } else {
                                                Some("▶ ".to_string())
                                            };
                                        state.is_modified = true;
                                    }
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
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
