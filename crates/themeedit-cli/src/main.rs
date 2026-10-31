//! Visual Theme Editor for `bashmenu.themes`.
//!
//! Provides a side-by-side terminal interface for editing theme colors,
//! window borders, and indicators with live preview switching across
//! Main Menu, Text Editor, Popup Windows, and Visual Menu Editor.

use anyhow::Result;
use bashmenu_tui::{
    composite_buffer, install_panic_hook, AsciiGridModal, ColorGridModal, ConfirmModal, InputModal,
    ResolvedThemeStyles, TerminalGuard, WallpaperCompositor,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::PathBuf;
use std::time::Duration;
use themeedit_cli::{PreviewMode, ThemeEditorState, TokenKind};

#[derive(Parser, Debug)]
#[command(
    name = "themeedit",
    author,
    version,
    about = "Interactive visual theme editor with side-by-side token editing and multi-view previews"
)]
struct Cli {
    /// Path to theme definition file
    #[arg(short, long, default_value = "bashmenu.themes")]
    file: PathBuf,

    /// Name of specific theme to edit
    #[arg(short, long)]
    theme: Option<String>,
}

fn resolve_themes_path(target: &PathBuf) -> PathBuf {
    if target.is_absolute() {
        return target.clone();
    }
    if target.components().count() > 1 && target.exists() {
        return target.clone();
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
        return target.clone();
    }
    let fixture = PathBuf::from("tests/fixtures").join(target);
    if fixture.exists() {
        return fixture;
    }
    target.clone()
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
    let mut last_grid_origin = (0, 0);
    let mut wp_compositor = state
        .current_theme
        .wallpaper_config()
        .map(WallpaperCompositor::new);
    let mut need_redraw = true;

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        // Derive resolved styles from current edited theme for live preview
        let preview_styles = ResolvedThemeStyles::from_theme(&state.current_theme, true);

        if need_redraw {
            let wp_cfg = state.current_theme.wallpaper_config();
            match (&mut wp_compositor, wp_cfg) {
                (Some(c), Some(cfg)) => c.set_config(cfg),
                (None, Some(cfg)) => wp_compositor = Some(WallpaperCompositor::new(cfg)),
                (Some(_), None) => wp_compositor = None,
                (None, None) => {}
            }

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

            // Universal close button [X] on top right just left of border
            if area.width > 6 {
                let close_x = area.x + area.width - 4;
                buf.set_string(close_x, area.y, "[X]", preview_styles.window_close_button);
            }

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

            // 2. Render Right Panel: Live Multi-View Preview Block
            let tab_menu = if state.preview_mode == PreviewMode::Menu {
                "▶ 1: Menu ◀"
            } else {
                "1: Menu"
            };
            let tab_ed = if state.preview_mode == PreviewMode::TextEditor {
                "▶ 2: Editor ◀"
            } else {
                "2: Editor"
            };
            let tab_mod = if state.preview_mode == PreviewMode::Modals {
                "▶ 3: Modals ◀"
            } else {
                "3: Modals"
            };
            let tab_me = if state.preview_mode == PreviewMode::MenuEditor {
                "▶ 4: MenuEdit ◀"
            } else {
                "4: MenuEdit"
            };

            let preview_title = format!(
                " [ Preview: {} | {} | {} | {} ] ",
                tab_menu, tab_ed, tab_mod, tab_me
            );

            let preview_block = Block::default()
                .borders(Borders::ALL)
                .title(preview_title)
                .border_style(Style::default().fg(preview_styles.border))
                .style(Style::default().bg(preview_styles.background));
            preview_block.render(right_area, buf);

            let p_inner_x = right_area.x + 2;
            let p_inner_y = right_area.y + 2;
            let p_avail_w = right_area.width.saturating_sub(4);
            let p_avail_h = right_area.height.saturating_sub(4);

            match state.preview_mode {
                PreviewMode::Menu => {
                    // Active item
                    let ind = &preview_styles.indicator_symbol;
                    let mut active_spans = vec![
                        Span::styled(ind, preview_styles.highlight),
                        Span::styled("  🚀 ", preview_styles.highlight),
                        Span::styled("1. ", preview_styles.highlight),
                        Span::styled("Launch Application", preview_styles.highlight),
                    ];
                    let active_vis: usize =
                        active_spans.iter().map(|s| s.content.chars().count()).sum();
                    let pad_active = (p_avail_w as usize).saturating_sub(active_vis);
                    if pad_active > 0 {
                        active_spans.push(Span::styled(
                            " ".repeat(pad_active),
                            preview_styles.highlight,
                        ));
                    }
                    buf.set_line(p_inner_x, p_inner_y, &Line::from(active_spans), p_avail_w);

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

                    // Wallpaper overlay
                    if let Some(comp) = &mut wp_compositor {
                        if let Some(grid) = comp.get_grid(
                            right_area.width as usize,
                            right_area.height as usize,
                            preview_styles.background,
                        ) {
                            composite_buffer(right_area, buf, grid, preview_styles.background);
                        }
                    }
                }
                PreviewMode::TextEditor => {
                    // 1. Tab Bar
                    let tab_line = Line::from(vec![
                        Span::styled(" [bashmenu.yml*] ", preview_styles.highlight),
                        Span::styled(" [notes.yml] ", preview_styles.text),
                        Span::styled(" [+] ", preview_styles.accent),
                    ]);
                    buf.set_line(p_inner_x, p_inner_y, &tab_line, p_avail_w);

                    let sep_str = "─".repeat(p_avail_w as usize);
                    buf.set_string(p_inner_x, p_inner_y + 1, &sep_str, preview_styles.border);

                    // 2. Editor Text Content with Line Numbers
                    let code_lines = [
                        (" 1 │ ", "theme: \"dracula\""),
                        (" 2 │ ", "terminal:"),
                        (" 3 │ ", "  alt_buffer: true"),
                        (" 4 │ ", "  show_whitespace: false"),
                        (" 5 │ ", "menu:"),
                        (" 6 │ ", "  preferred_width_pct: 90"),
                        (" ~ │ ", ""),
                    ];

                    for (row, (lineno, text)) in code_lines.iter().enumerate() {
                        let y = p_inner_y + 2 + row as u16;
                        if y >= right_area.y + right_area.height - 2 {
                            break;
                        }
                        let is_cursor_line = row == 2;
                        let line_style = if is_cursor_line {
                            preview_styles.highlight
                        } else {
                            preview_styles.text
                        };

                        let full_line = Line::from(vec![
                            Span::styled(*lineno, preview_styles.gutter),
                            Span::styled(*text, line_style),
                        ]);
                        buf.set_line(p_inner_x, y, &full_line, p_avail_w);
                    }

                    // 3. Status Bar
                    let status_y = right_area.y + right_area.height - 2;
                    let status_text = format!(
                        " NORMAL │ bashmenu.yml │ utf-8 │ 6L, 128B │ 3:12 {:<width$}",
                        "",
                        width = p_avail_w.saturating_sub(48) as usize
                    );
                    buf.set_string(
                        p_inner_x,
                        status_y,
                        &status_text,
                        preview_styles.highlight,
                    );

                    // 4. Editor Command Footer
                    let footer_y = right_area.y + right_area.height - 1;
                    buf.set_string(
                        p_inner_x,
                        footer_y,
                        "^G Help  ^O WriteOut  ^R ReadFile  ^X Exit",
                        preview_styles.gutter,
                    );
                }
                PreviewMode::Modals => {
                    // Modal dialog box centered within preview area
                    let mod_w = (p_avail_w.saturating_sub(4)).clamp(28, 44);
                    let mod_h = 10_u16.min(p_avail_h);
                    let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
                    let mod_y = p_inner_y + (p_avail_h.saturating_sub(mod_h)) / 2;

                    let modal_block = Block::default()
                        .borders(Borders::ALL)
                        .title(" [ Confirmation Dialog ] ")
                        .border_style(Style::default().fg(preview_styles.border))
                        .style(Style::default().bg(preview_styles.background));
                    modal_block.render(Rect::new(mod_x, mod_y, mod_w, mod_h), buf);

                    // Modal Close Button [X]
                    buf.set_string(
                        mod_x + mod_w - 4,
                        mod_y,
                        "[X]",
                        preview_styles.window_close_button,
                    );

                    // Modal Text
                    buf.set_string(
                        mod_x + 2,
                        mod_y + 2,
                        "Are you sure you want to proceed?",
                        preview_styles.text,
                    );
                    buf.set_string(
                        mod_x + 2,
                        mod_y + 3,
                        "All pending changes will be saved.",
                        preview_styles.help_text,
                    );

                    // Sample input box
                    let inp_w = mod_w.saturating_sub(6) as usize;
                    let inp_str = format!(" [ {:<width$} ]", "cluster_production_01", width = inp_w.saturating_sub(4));
                    buf.set_string(mod_x + 2, mod_y + 5, &inp_str, preview_styles.text);

                    // Action Buttons
                    let ok_btn = " [ OK / Confirm ] ";
                    let cancel_btn = " [ Cancel ] ";
                    buf.set_string(
                        mod_x + 3,
                        mod_y + 7,
                        ok_btn,
                        preview_styles.accent.add_modifier(Modifier::BOLD),
                    );
                    buf.set_string(
                        mod_x + 3 + ok_btn.len() as u16 + 2,
                        mod_y + 7,
                        cancel_btn,
                        preview_styles.text,
                    );

                    // Scrollbar indicator on right border
                    buf.set_string(mod_x + mod_w - 1, mod_y + 3, "▲", preview_styles.accent);
                    buf.set_string(mod_x + mod_w - 1, mod_y + 4, "█", preview_styles.accent);
                    buf.set_string(mod_x + mod_w - 1, mod_y + 5, "▼", preview_styles.accent);
                }
                PreviewMode::MenuEditor => {
                    // Visual Menu Editor Split View
                    let sep_col = p_inner_x + p_avail_w / 2;

                    // Left Pane: Tree
                    buf.set_string(
                        p_inner_x,
                        p_inner_y,
                        "▼ Main Menu",
                        preview_styles.title,
                    );
                    buf.set_string(
                        p_inner_x,
                        p_inner_y + 1,
                        "  ├─▶ Applications",
                        preview_styles.text,
                    );
                    buf.set_string(
                        p_inner_x,
                        p_inner_y + 2,
                        "  │ └─• Terminal",
                        preview_styles.highlight,
                    );
                    buf.set_string(
                        p_inner_x,
                        p_inner_y + 3,
                        "  ├─• File Manager",
                        preview_styles.text,
                    );
                    buf.set_string(
                        p_inner_x,
                        p_inner_y + 4,
                        "  └─• Settings",
                        preview_styles.text,
                    );

                    // Center divider
                    for y in p_inner_y..(p_inner_y + p_avail_h.saturating_sub(1)) {
                        buf.set_string(sep_col, y, "│", preview_styles.border);
                    }

                    // Right Pane: Property Inspector
                    let r_x = sep_col + 2;
                    buf.set_string(r_x, p_inner_y, "Title:  [ Terminal ]", preview_styles.accent);
                    buf.set_string(r_x, p_inner_y + 1, "Icon:   [ 💻 ]", preview_styles.text);
                    buf.set_string(r_x, p_inner_y + 2, "Action: [ alacritty ]", preview_styles.shortcut_key);
                    buf.set_string(r_x, p_inner_y + 3, "Type:   [ command ]", preview_styles.accent);

                    let btn_save = " [ Save ] ";
                    let btn_del = " [ Delete ] ";
                    buf.set_string(
                        r_x,
                        p_inner_y + 5,
                        btn_save,
                        preview_styles.accent.add_modifier(Modifier::BOLD),
                    );
                    buf.set_string(
                        r_x + btn_save.len() as u16 + 2,
                        p_inner_y + 5,
                        btn_del,
                        preview_styles.window_close_button,
                    );

                    // Footer
                    let footer_y = right_area.y + right_area.height - 1;
                    buf.set_string(
                        p_inner_x,
                        footer_y,
                        "[F1] Help | [s] Save | [ESC] Exit",
                        preview_styles.gutter,
                    );
                }
            }

            // 3. Render Bottom Footer Status
            let footer_y = area.y + area.height - 1;
            let footer_text =
                " [TAB/1-4] Switch Preview | [Enter] Pick | [Ctrl+S] Save | [Ctrl+A] Save As | [q] Exit ";
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
            need_redraw = false;
        }

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            // Ignore pure mouse movement events to avoid event queue flooding and latency
            if let Event::Mouse(m) = ev {
                if m.kind == MouseEventKind::Moved {
                    continue;
                }
            }

            need_redraw = true;

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
                ActiveModal::SaveAs(modal) => match ev {
                    Event::Key(k) => {
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
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, area) {
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
                    _ => {}
                },
                ActiveModal::ConfirmExit(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(confirmed) = modal.handle_mouse(m, area) {
                            if confirmed {
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    _ => {}
                },
                ActiveModal::None => match ev {
                    Event::Mouse(m) => {
                        if m.kind == MouseEventKind::Down(MouseButton::Left) {
                            // Universal close button [X]
                            if m.row == area.y && m.column >= area.x + area.width - 5 {
                                if state.is_modified {
                                    active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                        "Unsaved Changes",
                                        "Discard unsaved changes and exit?",
                                    ));
                                } else {
                                    break;
                                }
                                continue;
                            }

                            // Click on preview mode tab header
                            let left_w = (area.width * 45 / 100).clamp(30, 42);
                            let right_x = area.x + left_w + 1;
                            let right_w = (area.x + area.width).saturating_sub(right_x + 2);
                            let right_area =
                                Rect::new(right_x, area.y + 2, right_w, area.height - 4);
                            if m.row == right_area.y
                                && m.column >= right_area.x
                                && m.column < right_area.x + right_area.width
                            {
                                state.preview_mode = state.preview_mode.next();
                            }
                        }
                    }
                    Event::Key(k) => {
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
                            KeyCode::Tab | KeyCode::Char('p') | KeyCode::Char('P') => {
                                state.preview_mode = state.preview_mode.next();
                            }
                            KeyCode::Char('1') => {
                                state.preview_mode = PreviewMode::Menu;
                            }
                            KeyCode::Char('2') => {
                                state.preview_mode = PreviewMode::TextEditor;
                            }
                            KeyCode::Char('3') => {
                                state.preview_mode = PreviewMode::Modals;
                            }
                            KeyCode::Char('4') => {
                                state.preview_mode = PreviewMode::MenuEditor;
                            }
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
                    _ => {}
                },
            }
        }
    }

    Ok(())
}
