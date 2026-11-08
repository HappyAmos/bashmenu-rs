//! Visual Theme Editor for `bashmenu.themes`.
//!
//! Provides a side-by-side terminal interface for editing theme colors,
//! window borders, and indicators with live preview switching across
//! Main Menu, Text Editor, Popup Windows, and Visual Menu Editor.

use anyhow::Result;
use bashmenu_core::{ToastNotification, ToastSeverity};
use bashmenu_tui::{
    install_panic_hook, AsciiGridModal, BackgroundConfigAction, BackgroundConfigModal,
    BackgroundFocus, ColorPickerModal, ConfirmModal, FilePickerModal, InputModal,
    ResolvedThemeStyles, TerminalGuard, ToastRack, WallpaperCompositor,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::{Path, PathBuf};
use std::time::Duration;
use themeedit_cli::{
    default_wallpaper_dir, extract_preview_cell_color, extract_preview_color_at, format_color_code,
    get_token_preview_spans, relativize_wallpaper_path, render_preview_pane, PreviewMode,
    ThemeEditorState, TokenKind, TreeRow,
};

/// Command-line argument configuration for the Theme Editor (`themeedit`).
#[derive(Parser, Debug)]
#[command(
    name = "themeedit",
    author,
    version,
    about = "Interactive visual theme editor with side-by-side token editing and multi-view previews"
)]
struct Cli {
    /// Path to theme definition file
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Name of specific theme to edit
    #[arg(short, long)]
    theme: Option<String>,

    /// Load and save theme files in current working directory
    #[arg(long)]
    local: bool,
}

/// Resolves the file path for `bashmenu.themes`.
/// If `local` is true, resolves relative to current working directory; otherwise uses user config path.
fn resolve_themes_path(target: &Path, local: bool) -> PathBuf {
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

/// Active popup modal dialog in the theme editor.
enum ActiveModal {
    /// No modal active; tree editor and preview pane interactions enabled.
    None,
    /// Color picker dialog (ANSI 256 + TrueColor HSV + Hex).
    ColorPicker(String, ColorPickerModal),
    /// ASCII symbol picker grid dialog.
    AsciiPicker(String, AsciiGridModal),
    /// "Save As" theme name input dialog.
    SaveAs(InputModal),
    /// "New Theme" name input dialog.
    NewTheme(InputModal),
    /// Delete theme confirmation dialog.
    ConfirmDelete(ConfirmModal),
    /// Unsaved changes exit confirmation dialog.
    ConfirmExit(ConfirmModal),
    /// Background wallpaper configuration modal dialog.
    BackgroundConfig(BackgroundConfigModal),
    /// Wallpaper file browser launched from background configuration modal.
    BackgroundBrowse(Box<BackgroundConfigModal>, FilePickerModal),
}

/// Visual flash highlight entry: (start time instant, list of
/// `(x, y, length)` spans).
type FlashHighlight = (std::time::Instant, Vec<(u16, u16, u16)>);

/// Entry point for the theme editor TUI application (`themeedit`).
fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let themes_path = if let Some(ref f) = cli.file {
        resolve_themes_path(f, cli.local)
    } else if cli.local {
        PathBuf::from("./bashmenu.themes")
    } else {
        let user_themes = bashmenu_core::get_user_config_dir().join("bashmenu.themes");
        if user_themes.exists() {
            user_themes
        } else {
            resolve_themes_path(Path::new("bashmenu.themes"), false)
        }
    };
    let mut state = ThemeEditorState::new(themes_path, cli.theme);

    let mut guard = TerminalGuard::init()?;
    let mut active_modal = ActiveModal::None;
    let mut last_grid_origin = (0, 0);
    let mut wp_compositor = state
        .current_theme
        .wallpaper_config()
        .map(WallpaperCompositor::new);
    let mut need_redraw = true;
    let mut click_flash: Option<FlashHighlight> = None;
    let mut toast_rack = ToastRack::new();
    let mut is_editing_name = false;
    let mut preview_buffer = Buffer::empty(Rect::default());
    let cfg_path = bashmenu_core::get_user_config_dir().join("bashmenu.yml");
    let delay_secs = bashmenu_core::load_config(&cfg_path)
        .ok()
        .map(|c| c.help_gutter_delay())
        .unwrap_or(4.0);
    let mut footer_hover = bashmenu_tui::HelpGutterHover::with_duration(
        std::time::Duration::from_secs_f64(delay_secs.max(0.0)),
    );

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

            guard.draw(|f| {
                let buf = f.buffer_mut();

                for y in area.y..area.y + area.height {
                    for x in area.x..area.x + area.width {
                        buf[(x, y)].reset();
                    }
                }

                // Background fill
                let root_block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "─[ ThemeEdit: {}{} ]",
                        state.current_name,
                        if state.is_modified { " (Modified)" } else { "" }
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
                // Reserve bottom rows for gutter divider and gutter help bar
                let bottom_items = [
                    "[F5] Derive",
                    "[F6] New",
                    "[F7] Rename",
                    "[F8] Del",
                    "[Ctrl+S] Save",
                    "[Ctrl+A] Save As",
                    "[TAB] View",
                    "[q] Exit",
                ];
                let bottom_avail_w = area.width.saturating_sub(4) as usize;
                let has_hidden =
                    bashmenu_tui::has_hidden_gutter_text(&bottom_items, bottom_avail_w, 1);
                let is_expanded = footer_hover.is_expanded() && has_hidden && area.height >= 12;
                let footer_rows: u16 = if is_expanded { 2 } else { 1 };

                let bot_y = area.y + area.height - 1;
                let footer_y = bot_y.saturating_sub(1);
                let gt_y = bot_y.saturating_sub(1 + footer_rows);
                let panel_y = area.y + 2;
                let panel_h = gt_y.saturating_sub(panel_y);

                let left_w = (area.width * 45 / 100).clamp(30, 42);
                let left_area = Rect::new(area.x + 2, panel_y, left_w - 3, panel_h);
                let right_x = area.x + left_w + 1;
                let right_w = (area.x + area.width).saturating_sub(right_x + 2);
                let right_area = Rect::new(right_x, panel_y, right_w, panel_h);

                // 1. Render Left Panel: Editable Theme Name & Token List
                let theme_label = "Theme: ";
                buf.set_string(
                    left_area.x,
                    left_area.y,
                    theme_label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
                let input_box_x = left_area.x + theme_label.len() as u16;
                let input_box_w = (left_area.width.saturating_sub(theme_label.len() as u16)).max(8);

                let name_str = state.name_input.as_string();
                let box_style = if is_editing_name {
                    Style::default().fg(Color::White).bg(Color::Indexed(236))
                } else {
                    Style::default().fg(Color::Cyan)
                };
                let inner_w = (input_box_w.saturating_sub(4)) as usize;
                let chars: Vec<char> = name_str.chars().collect();
                let scroll = state.name_input.scroll_for_width(inner_w);
                let vis_chars: String = chars.iter().skip(scroll).take(inner_w).collect();
                let padded = format!("[ {:<width$} ]", vis_chars, width = inner_w);
                buf.set_string(input_box_x, left_area.y, &padded, box_style);

                if is_editing_name {
                    let sel_range = state.name_input.selection_range();
                    let text_start_x = input_box_x + 2;
                    let rel_cursor = state.name_input.cursor.saturating_sub(scroll);
                    for (c_idx, ch) in vis_chars.chars().enumerate() {
                        let cx = text_start_x + c_idx as u16;
                        let global_idx = scroll + c_idx;
                        let mut char_style = box_style;
                        if let Some((s_start, s_end)) = sel_range {
                            if global_idx >= s_start && global_idx < s_end {
                                char_style = char_style.add_modifier(Modifier::REVERSED);
                            }
                        }
                        if global_idx == state.name_input.cursor {
                            char_style = char_style.add_modifier(Modifier::REVERSED);
                        }
                        buf.set_string(cx, left_area.y, ch.to_string(), char_style);
                    }
                    if state.name_input.cursor >= chars.len() && rel_cursor < inner_w {
                        let cursor_col = text_start_x + rel_cursor as u16;
                        if let Some(cell) = buf.cell_mut((cursor_col, left_area.y)) {
                            cell.set_char(' ');
                            cell.set_style(box_style.add_modifier(Modifier::REVERSED));
                        }
                    }
                }

                // Sub-divider under theme name
                let sep_style = Style::default().fg(Color::DarkGray);
                let div_w = left_area.width as usize;
                buf.set_string(left_area.x, left_area.y + 1, "─".repeat(div_w), sep_style);

                // Tokens header
                let visible_rows = (left_area.height.saturating_sub(3)) as usize;
                state.adjust_scroll(visible_rows);
                let tree_rows = state.visible_rows();

                let scroll_up_ind = if state.scroll_offset > 0 { " ▲" } else { "" };
                let scroll_dn_ind = if state.scroll_offset + visible_rows < tree_rows.len() {
                    " ▼"
                } else {
                    ""
                };
                let list_title = format!(" Theme Tokens{}{}", scroll_up_ind, scroll_dn_ind);
                buf.set_string(
                    left_area.x,
                    left_area.y + 2,
                    list_title,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );

                for (v_idx, tree_row) in tree_rows
                    .iter()
                    .enumerate()
                    .skip(state.scroll_offset)
                    .take(visible_rows)
                {
                    let row_y = left_area.y + 3 + (v_idx - state.scroll_offset) as u16;
                    let is_sel = !is_editing_name && v_idx == state.selected_row;

                    match *tree_row {
                        TreeRow::Category(cat) => {
                            let expanded = state.is_category_expanded(cat);
                            let icon = if expanded { "▾ " } else { "▸ " };
                            let row_style = if is_sel {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            };
                            let label_text = format!(" {}{:<25}", icon, cat.label());
                            buf.set_line(
                                left_area.x,
                                row_y,
                                &Line::from(vec![Span::styled(label_text, row_style)]),
                                left_area.width,
                            );
                        }
                        TreeRow::Token(t_idx) => {
                            let token = &state.tokens[t_idx];
                            let val_str = state.get_token_value_display(token);
                            let cursor = if is_sel { "  ▶ " } else { "    " };
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
                    }
                }

                // 2. Render Right Panel: Live Multi-View Preview Block
                render_preview_pane(
                    right_area,
                    buf,
                    state.preview_mode,
                    &preview_styles,
                    &mut wp_compositor,
                );

                preview_buffer = Buffer::empty(right_area);
                for y in right_area.y..right_area.y + right_area.height {
                    for x in right_area.x..right_area.x + right_area.width {
                        if let Some(cell) = buf.cell((x, y)) {
                            preview_buffer[(x, y)] = cell.clone();
                        }
                    }
                }

                // 3. Render Bottom Help Gutter with dedicated divider
                let b = &preview_styles.borders;
                let border_style = Style::default().fg(Color::Cyan);

                // Gutter top border divider
                buf.set_string(area.x, gt_y, &b.left_tee, border_style);
                buf.set_string(area.x + area.width - 1, gt_y, &b.right_tee, border_style);
                let div_w = area.width.saturating_sub(2) as usize;
                buf.set_string(
                    area.x + 1,
                    gt_y,
                    b.border_horizontal_bottom.repeat(div_w),
                    border_style,
                );

                if is_expanded {
                    let lines =
                        bashmenu_tui::wrap_spread_help_gutter(&bottom_items, bottom_avail_w, 2);
                    for (r_idx, line) in lines.iter().enumerate() {
                        let y = footer_y.saturating_sub((lines.len() - 1 - r_idx) as u16);
                        buf.set_string(area.x, y, &b.border_vertical_left, border_style);
                        buf.set_string(
                            area.x + area.width - 1,
                            y,
                            &b.border_vertical_right,
                            border_style,
                        );
                        buf.set_string(
                            area.x + 2,
                            y,
                            line,
                            Style::default()
                                .fg(preview_styles.help_text.fg.unwrap_or(Color::White)),
                        );
                    }
                } else {
                    buf.set_string(area.x, footer_y, &b.border_vertical_left, border_style);
                    buf.set_string(
                        area.x + area.width - 1,
                        footer_y,
                        &b.border_vertical_right,
                        border_style,
                    );
                    let bottom_spread =
                        bashmenu_tui::format_spread_gutter_line(&bottom_items, bottom_avail_w);
                    buf.set_string(
                        area.x + 2,
                        footer_y,
                        bottom_spread,
                        Style::default().fg(preview_styles.help_text.fg.unwrap_or(Color::White)),
                    );
                }

                // 4. Render Active Toasts
                toast_rack.render(area, &preview_styles, buf);

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
                    ActiveModal::NewTheme(modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::ConfirmDelete(modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::ConfirmExit(modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::BackgroundConfig(modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::BackgroundBrowse(_, modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::None => {}
                }

                // 5. Visual/Haptic feedback flash for preview panel clicks & inspector token selection (Modifier::REVERSED for 200ms)
                if let Some((flash_start, ref spans)) = click_flash {
                    if flash_start.elapsed() < Duration::from_millis(200) {
                        for &(fy, fx_start, fx_end) in spans {
                            for x in fx_start..fx_end {
                                if let Some(cell) = buf.cell_mut((x, fy)) {
                                    cell.modifier |= Modifier::REVERSED;
                                }
                            }
                        }
                    } else {
                        click_flash = None;
                    }
                }
            })?;
            need_redraw = false;
        }

        toast_rack.tick();
        if !toast_rack.is_empty() {
            need_redraw = true;
        }

        let poll_dur = if click_flash.is_some() || !toast_rack.is_empty() {
            Duration::from_millis(30)
        } else {
            Duration::from_millis(50)
        };

        let bottom_items = [
            "[F5] Derive",
            "[F6] New",
            "[F7] Rename",
            "[F8] Del",
            "[Ctrl+S] Save",
            "[Ctrl+A] Save As",
            "[TAB] View",
            "[R-Click] Dropper",
            "[q] Exit",
        ];
        let bottom_avail_w = area.width.saturating_sub(4) as usize;
        let has_hidden = bashmenu_tui::has_hidden_gutter_text(&bottom_items, bottom_avail_w, 1);

        if event::poll(poll_dur)? {
            let ev = event::read()?;

            if let Event::Mouse(m) = &ev {
                if m.kind == MouseEventKind::Moved {
                    let footer_rows: u16 = if footer_hover.is_expanded() { 2 } else { 1 };
                    let in_gutter = m.row >= (area.y + area.height).saturating_sub(1 + footer_rows)
                        && m.row < area.y + area.height - 1;
                    if matches!(active_modal, ActiveModal::None)
                        && footer_hover.handle_mouse_move(m.column, m.row, in_gutter && has_hidden)
                    {
                        need_redraw = true;
                    }
                    continue;
                }
                footer_hover.handle_interaction();
            }
            if let Event::Key(_) = &ev {
                footer_hover.handle_interaction();
            }

            need_redraw = true;

            // Modal interaction handles events first
            match &mut active_modal {
                ActiveModal::ColorPicker(key, modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(color) = res {
                                state.set_color_token_value(key, color);
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, area) {
                            if let Some(color) = res {
                                state.set_color_token_value(key, color);
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
                                if key == "divider_char" {
                                    state.set_divider_char(ch);
                                } else if key == "marquee_char" {
                                    state.set_marquee_char(ch);
                                } else if key == "toast_char" {
                                    state.set_toast_char(ch);
                                } else {
                                    state.set_border_token(key, ch);
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(res) = modal.handle_mouse(m, last_grid_origin) {
                            if let Some(ch) = res {
                                if key == "divider_char" {
                                    state.set_divider_char(ch);
                                } else if key == "marquee_char" {
                                    state.set_marquee_char(ch);
                                } else if key == "toast_char" {
                                    state.set_toast_char(ch);
                                } else {
                                    state.set_border_token(key, ch);
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::SaveAs(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key_with_modifiers(k.code, k.modifiers) {
                            if let Some(new_name) = res {
                                let trimmed = new_name.trim();
                                if !trimmed.is_empty() {
                                    match state.save_as_theme(trimmed) {
                                        Ok(_) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Save Theme As".to_string()),
                                                message: format!(
                                                    "Saved as new theme '{}' to disk",
                                                    state.current_name
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.0,
                                            });
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Error".to_string()),
                                                message: format!("Failed to save theme: {}", e),
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 3.0,
                                            });
                                        }
                                    }
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
                                    match state.save_as_theme(trimmed) {
                                        Ok(_) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Save Theme As".to_string()),
                                                message: format!(
                                                    "Saved as new theme '{}' to disk",
                                                    state.current_name
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.0,
                                            });
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Error".to_string()),
                                                message: format!("Failed to save theme: {}", e),
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 3.0,
                                            });
                                        }
                                    }
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::NewTheme(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(res) = modal.handle_key_with_modifiers(k.code, k.modifiers) {
                            if let Some(new_name) = res {
                                let trimmed = new_name.trim();
                                if !trimmed.is_empty() {
                                    match state.create_theme(trimmed) {
                                        Ok(_) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Theme Editor".to_string()),
                                                message: format!(
                                                    "Created new theme '{}' (monochrome base)",
                                                    state.current_name
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.0,
                                            });
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Error".to_string()),
                                                message: e,
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 3.0,
                                            });
                                        }
                                    }
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
                                    match state.create_theme(trimmed) {
                                        Ok(_) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Theme Editor".to_string()),
                                                message: format!(
                                                    "Created new theme '{}' (monochrome base)",
                                                    state.current_name
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.0,
                                            });
                                        }
                                        Err(e) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Error".to_string()),
                                                message: e,
                                                severity: ToastSeverity::Error,
                                                timeout_secs: 3.0,
                                            });
                                        }
                                    }
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    _ => {}
                },
                ActiveModal::ConfirmDelete(modal) => match ev {
                    Event::Key(k) => {
                        if let Some(choice) = modal.handle_key(k.code) {
                            if choice.is_yes() {
                                let to_del = state.current_name.clone();
                                match state.delete_theme(&to_del) {
                                    Ok(_) => {
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message: format!("Deleted theme '{}'", to_del),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 2.0,
                                        });
                                    }
                                    Err(e) => {
                                        toast_rack.add(ToastNotification {
                                            title: Some("Error".to_string()),
                                            message: e,
                                            severity: ToastSeverity::Error,
                                            timeout_secs: 3.0,
                                        });
                                    }
                                }
                            }
                            active_modal = ActiveModal::None;
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(choice) = modal.handle_mouse(m, area) {
                            if choice.is_yes() {
                                let to_del = state.current_name.clone();
                                match state.delete_theme(&to_del) {
                                    Ok(_) => {
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message: format!("Deleted theme '{}'", to_del),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 2.0,
                                        });
                                    }
                                    Err(e) => {
                                        toast_rack.add(ToastNotification {
                                            title: Some("Error".to_string()),
                                            message: e,
                                            severity: ToastSeverity::Error,
                                            timeout_secs: 3.0,
                                        });
                                    }
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
                            if choice.is_yes() {
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(choice) = modal.handle_mouse(m, area) {
                            if choice.is_yes() {
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                    _ => {}
                },
                ActiveModal::BackgroundConfig(ref mut modal) => {
                    let action = match ev {
                        Event::Key(k) => modal.handle_key_with_modifiers(k.code, k.modifiers),
                        Event::Mouse(m) => modal.handle_mouse(m, area),
                        _ => BackgroundConfigAction::None,
                    };
                    match action {
                        BackgroundConfigAction::None => {}
                        BackgroundConfigAction::Browse => {
                            let existing_img = modal.build_config().image.unwrap_or_default();
                            let start_dir = if !existing_img.trim().is_empty() {
                                if let Some(resolved) =
                                    bashmenu_tui::resolve_wallpaper_path(&existing_img)
                                {
                                    if let Some(parent) = resolved.parent() {
                                        if parent.is_dir() {
                                            parent.to_path_buf()
                                        } else {
                                            default_wallpaper_dir()
                                        }
                                    } else {
                                        default_wallpaper_dir()
                                    }
                                } else {
                                    default_wallpaper_dir()
                                }
                            } else {
                                default_wallpaper_dir()
                            };
                            let file_picker =
                                FilePickerModal::new("Choose Wallpaper Image", &start_dir, false);
                            active_modal =
                                ActiveModal::BackgroundBrowse(Box::new(modal.clone()), file_picker);
                        }
                        BackgroundConfigAction::Apply(cfg) => {
                            state.set_wallpaper_config(Some(cfg));
                            toast_rack.add(ToastNotification {
                                title: Some("Theme Editor".to_string()),
                                message: "Wallpaper configured".to_string(),
                                severity: ToastSeverity::Information,
                                timeout_secs: 2.0,
                            });
                            active_modal = ActiveModal::None;
                        }
                        BackgroundConfigAction::Remove => {
                            state.set_wallpaper_config(None);
                            toast_rack.add(ToastNotification {
                                title: Some("Theme Editor".to_string()),
                                message: "Wallpaper removed".to_string(),
                                severity: ToastSeverity::Information,
                                timeout_secs: 2.0,
                            });
                            active_modal = ActiveModal::None;
                        }
                        BackgroundConfigAction::Cancel => {
                            active_modal = ActiveModal::None;
                        }
                    }
                }
                ActiveModal::BackgroundBrowse(ref mut bg_modal, ref mut file_picker) => {
                    let picked = match ev {
                        Event::Key(k) => file_picker.handle_key_with_modifiers(k.code, k.modifiers),
                        Event::Mouse(m) => file_picker.handle_mouse(m, area),
                        _ => None,
                    };
                    if let Some(res) = picked {
                        if let Some(path) = res {
                            if path.is_file() {
                                let rel_path = relativize_wallpaper_path(&path);
                                bg_modal.set_image_path(&rel_path);
                            }
                        }
                        active_modal = ActiveModal::BackgroundConfig(*bg_modal.clone());
                    }
                }
                ActiveModal::None => {
                    let bot_y = area.y + area.height - 1;
                    let footer_y = bot_y.saturating_sub(1);
                    let gt_y = footer_y.saturating_sub(1);
                    let panel_y = area.y + 2;
                    let panel_h = gt_y.saturating_sub(panel_y);

                    let left_w = (area.width * 45 / 100).clamp(30, 42);
                    let left_area = Rect::new(area.x + 2, panel_y, left_w, panel_h);
                    let right_x = area.x + left_w + 1;
                    let right_w = (area.x + area.width).saturating_sub(right_x + 2);
                    let right_area = Rect::new(right_x, panel_y, right_w, panel_h);

                    match ev {
                        Event::Mouse(m) => {
                            if toast_rack.handle_mouse(m.column, m.row) {
                                continue;
                            }

                            if m.kind == MouseEventKind::ScrollUp {
                                if m.column >= left_area.x
                                    && m.column < left_area.x + left_area.width
                                    && state.selected_row > 0
                                {
                                    state.select_visible_row(state.selected_row - 1);
                                }
                                continue;
                            }
                            if m.kind == MouseEventKind::ScrollDown {
                                let total_rows = state.visible_rows().len();
                                if m.column >= left_area.x
                                    && m.column < left_area.x + left_area.width
                                    && state.selected_row + 1 < total_rows
                                {
                                    state.select_visible_row(state.selected_row + 1);
                                }
                                continue;
                            }

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

                                // Click on Theme Name field
                                if m.row == left_area.y
                                    && m.column >= left_area.x
                                    && m.column < left_area.x + left_area.width
                                {
                                    is_editing_name = true;
                                    let theme_label_len = 7_u16;
                                    let input_box_x = left_area.x + theme_label_len;
                                    let input_box_w =
                                        (left_area.width.saturating_sub(theme_label_len)).max(8);
                                    let inner_w = (input_box_w.saturating_sub(4)) as usize;
                                    let scroll = state.name_input.scroll_for_width(inner_w);
                                    let click_inner =
                                        m.column.saturating_sub(input_box_x + 2) as usize;
                                    state.name_input.cursor =
                                        (scroll + click_inner).min(state.name_input.len());
                                    state.name_input.clear_selection();
                                    state.name_input.ensure_cursor_visible(inner_w);
                                    continue;
                                }

                                // If clicked elsewhere while editing name, commit rename
                                if is_editing_name {
                                    let new_name = state.name_input.as_string();
                                    let _ = state.rename_theme(&new_name);
                                    is_editing_name = false;
                                }

                                // 1. Click on left token list
                                let visible_rows = (left_area.height.saturating_sub(3)) as usize;
                                if m.column >= left_area.x
                                    && m.column < left_area.x + left_area.width
                                    && m.row >= left_area.y + 3
                                    && m.row < left_area.y + 3 + visible_rows as u16
                                {
                                    let clicked_row_idx =
                                        state.scroll_offset + (m.row - (left_area.y + 3)) as usize;
                                    let rows = state.visible_rows();
                                    if let Some(&clicked_row) = rows.get(clicked_row_idx) {
                                        let was_selected = clicked_row_idx == state.selected_row;
                                        state.select_visible_row(clicked_row_idx);

                                        match clicked_row {
                                            TreeRow::Category(cat) => {
                                                state.toggle_category(cat);
                                                need_redraw = true;
                                            }
                                            TreeRow::Token(t_idx) => {
                                                let token = state.tokens[t_idx].clone();
                                                toast_rack.add(ToastNotification {
                                                    title: Some("Theme Editor".to_string()),
                                                    message: format!("Selected: {}", token.label),
                                                    severity: ToastSeverity::Information,
                                                    timeout_secs: 1.5,
                                                });
                                                if matches!(token.kind, TokenKind::ToastChar)
                                                    || matches!(
                                                        &token.kind,
                                                        TokenKind::Color(ref k) if k.starts_with("toast_")
                                                    )
                                                {
                                                    state.preview_mode = PreviewMode::Toast;
                                                }
                                                let spans = get_token_preview_spans(
                                                    &token,
                                                    right_area,
                                                    state.preview_mode,
                                                    &preview_styles,
                                                );
                                                click_flash =
                                                    Some((std::time::Instant::now(), spans));
                                                need_redraw = true;

                                                // If clicked again on the already selected item, or clicked on the value column, open picker
                                                if was_selected || m.column >= left_area.x + 24 {
                                                    open_selected_token_picker(
                                                        &mut state,
                                                        &mut active_modal,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    continue;
                                }

                                // 2. Click on preview mode tab header
                                let title_text = format!(
                                    " Preview: {} (Click to Switch) ",
                                    state.preview_mode.title()
                                );
                                let title_start = right_area.x + 3;
                                let title_end = title_start + title_text.len() as u16;
                                if m.row == right_area.y
                                    && m.column >= title_start
                                    && m.column < title_end
                                {
                                    state.preview_mode = state.preview_mode.next();
                                    continue;
                                }

                                // 3. Click on preview pane to focus clicked element and trigger haptic flash
                                if m.column >= right_area.x
                                    && m.column < right_area.x + right_area.width
                                    && m.row >= right_area.y
                                    && m.row < right_area.y + right_area.height
                                {
                                    if let Some((key, fy, fx_start, fx_end)) = hit_test_preview(
                                        m.column,
                                        m.row,
                                        right_area,
                                        state.preview_mode,
                                        preview_styles.divider_is_screen_width,
                                    ) {
                                        let color_str = extract_preview_color_at(
                                            &preview_buffer,
                                            m.column,
                                            m.row,
                                            &preview_styles,
                                        );
                                        let label = if state.select_token_by_key(key) {
                                            state.tokens[state.selected_idx].label.clone()
                                        } else {
                                            key.to_string()
                                        };
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message: format!(
                                                "Selected: {}\nColor Clicked: {}",
                                                label, color_str
                                            ),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 1.5,
                                        });
                                        click_flash = Some((
                                            std::time::Instant::now(),
                                            vec![(fy, fx_start, fx_end)],
                                        ));
                                        need_redraw = true;
                                    }
                                    continue;
                                }
                            }

                            if m.kind == MouseEventKind::Down(MouseButton::Right) {
                                // Color Dropper: right-click in preview pane samples pixel color and assigns it to selected token
                                if m.column >= right_area.x
                                    && m.column < right_area.x + right_area.width
                                    && m.row >= right_area.y
                                    && m.row < right_area.y + right_area.height
                                {
                                    let sampled_color = extract_preview_cell_color(
                                        &preview_buffer,
                                        m.column,
                                        m.row,
                                        &preview_styles,
                                    );
                                    let color_str = format_color_code(sampled_color);
                                    let clipboard_val = match sampled_color {
                                        Color::Indexed(code) => format!("{}", code),
                                        Color::Rgb(r, g, b) => {
                                            format!("#{r:02x}{g:02x}{b:02x}")
                                        }
                                        _ => {
                                            let (r, g, b) =
                                                bashmenu_tui::color_to_rgb(sampled_color);
                                            format!("#{r:02x}{g:02x}{b:02x}")
                                        }
                                    };
                                    bashmenu_tui::set_clipboard_text(&clipboard_val);

                                    let token = state.current_selected_token().cloned();
                                    if let Some(tok) = token {
                                        if let TokenKind::Color(ref key) = tok.kind {
                                            state.set_color_token_value(key, sampled_color);
                                            toast_rack.add(ToastNotification {
                                                title: Some("Color Dropper".to_string()),
                                                message: format!(
                                                    "Sampled: {}\nAssigned to: {}",
                                                    color_str, tok.label
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                        } else {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Color Dropper".to_string()),
                                                message: format!(
                                                    "Sampled: {}\n'{}' is not a color setting",
                                                    color_str, tok.label
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 1.5,
                                            });
                                        }
                                    }
                                    click_flash = Some((
                                        std::time::Instant::now(),
                                        vec![(m.row, m.column, m.column + 1)],
                                    ));
                                    need_redraw = true;
                                    continue;
                                }
                            }
                        }
                        Event::Key(k) => {
                            if is_editing_name {
                                match k.code {
                                    KeyCode::Enter => {
                                        let new_name = state.name_input.as_string();
                                        match state.rename_theme(&new_name) {
                                            Ok(_) => {
                                                is_editing_name = false;
                                                toast_rack.add(ToastNotification {
                                                    title: Some("Theme Editor".to_string()),
                                                    message: format!(
                                                        "Renamed theme to '{}'",
                                                        state.current_name
                                                    ),
                                                    severity: ToastSeverity::Information,
                                                    timeout_secs: 2.0,
                                                });
                                            }
                                            Err(err) => {
                                                toast_rack.add(ToastNotification {
                                                    title: Some("Rename Error".to_string()),
                                                    message: err,
                                                    severity: ToastSeverity::Error,
                                                    timeout_secs: 3.0,
                                                });
                                            }
                                        }
                                        continue;
                                    }
                                    KeyCode::Esc => {
                                        state.name_input.set_text(&state.current_name);
                                        is_editing_name = false;
                                        continue;
                                    }
                                    KeyCode::Tab | KeyCode::Down => {
                                        let new_name = state.name_input.as_string();
                                        if state.rename_theme(&new_name).is_ok() {
                                            is_editing_name = false;
                                        }
                                        continue;
                                    }
                                    _ => {
                                        state
                                            .name_input
                                            .handle_key_with_modifiers(k.code, k.modifiers);
                                        continue;
                                    }
                                }
                            }

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
                                            match state.save_to_disk() {
                                                Ok(_) => {
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Save Theme".to_string()),
                                                        message: format!(
                                                            "Saved theme '{}' to disk",
                                                            state.current_name
                                                        ),
                                                        severity: ToastSeverity::Information,
                                                        timeout_secs: 2.0,
                                                    });
                                                }
                                                Err(e) => {
                                                    toast_rack.add(ToastNotification {
                                                        title: Some("Error".to_string()),
                                                        message: format!(
                                                            "Failed to save theme: {}",
                                                            e
                                                        ),
                                                        severity: ToastSeverity::Error,
                                                        timeout_secs: 3.0,
                                                    });
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                    KeyCode::Char('a') => {
                                        // Ctrl+A opens Save As dialog
                                        active_modal = ActiveModal::SaveAs(InputModal::new(
                                            "Save Theme As",
                                            "Enter new theme name:",
                                            &state.current_name,
                                        ));
                                        continue;
                                    }
                                    KeyCode::Char('n') => {
                                        // Ctrl+N creates new theme with base monochrome theme
                                        active_modal = ActiveModal::NewTheme(InputModal::new(
                                            "Create New Theme",
                                            "Enter new theme name:",
                                            "",
                                        ));
                                        continue;
                                    }
                                    KeyCode::Char('r') => {
                                        // Ctrl+R renames theme
                                        is_editing_name = true;
                                        state.name_input.set_text(&state.current_name);
                                        state.name_input.select_all();
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message:
                                                "Editing theme name (Enter to apply, Esc to cancel)"
                                                    .to_string(),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 1.5,
                                        });
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            match k.code {
                                KeyCode::F(5) => {
                                    // F5 auto-derives theme colors from configured wallpaper image
                                    match state.derive_colors_from_wallpaper() {
                                        Ok(fname) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Derive Theme Colors".to_string()),
                                                message: format!(
                                                    "Derived theme colors from '{}'",
                                                    fname
                                                ),
                                                severity: ToastSeverity::Information,
                                                timeout_secs: 2.5,
                                            });
                                        }
                                        Err(err) => {
                                            toast_rack.add(ToastNotification {
                                                title: Some("Derive Theme Colors".to_string()),
                                                message: err,
                                                severity: ToastSeverity::Warning,
                                                timeout_secs: 3.0,
                                            });
                                        }
                                    }
                                }
                                KeyCode::F(6) => {
                                    // F6 creates new theme
                                    active_modal = ActiveModal::NewTheme(InputModal::new(
                                        "Create New Theme",
                                        "Enter new theme name:",
                                        "",
                                    ));
                                }
                                KeyCode::F(7) | KeyCode::F(2) => {
                                    // F7 / F2 renames theme
                                    is_editing_name = true;
                                    state.name_input.set_text(&state.current_name);
                                    state.name_input.select_all();
                                    toast_rack.add(ToastNotification {
                                        title: Some("Theme Editor".to_string()),
                                        message:
                                            "Editing theme name (Enter to apply, Esc to cancel)"
                                                .to_string(),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                }
                                KeyCode::F(8) => {
                                    // F8 deletes current theme with confirmation
                                    active_modal = ActiveModal::ConfirmDelete(ConfirmModal::new(
                                        "Delete Theme",
                                        format!(
                                            "Are you sure you want to delete theme '{}'?",
                                            state.current_name
                                        ),
                                    ));
                                }
                                KeyCode::Char('[') => {
                                    state.cycle_theme(false);
                                    toast_rack.add(ToastNotification {
                                        title: Some("Theme Switcher".to_string()),
                                        message: format!("Switched to '{}'", state.current_name),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                }
                                KeyCode::Char(']') => {
                                    state.cycle_theme(true);
                                    toast_rack.add(ToastNotification {
                                        title: Some("Theme Switcher".to_string()),
                                        message: format!("Switched to '{}'", state.current_name),
                                        severity: ToastSeverity::Information,
                                        timeout_secs: 1.5,
                                    });
                                }
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
                                KeyCode::Char('5') => {
                                    state.preview_mode = PreviewMode::Toast;
                                }
                                KeyCode::Left | KeyCode::Char('h') => {
                                    if state.handle_left_key() {
                                        need_redraw = true;
                                    }
                                }
                                KeyCode::Right | KeyCode::Char('l') => {
                                    if state.handle_right_key() {
                                        need_redraw = true;
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if state.selected_row == 0 {
                                        is_editing_name = true;
                                        state.name_input.select_all();
                                    } else {
                                        state.select_visible_row(state.selected_row - 1);
                                        if let Some(TreeRow::Token(t_idx)) =
                                            state.visible_rows().get(state.selected_row).copied()
                                        {
                                            let spans = get_token_preview_spans(
                                                &state.tokens[t_idx],
                                                right_area,
                                                state.preview_mode,
                                                &preview_styles,
                                            );
                                            click_flash = Some((std::time::Instant::now(), spans));
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let total_rows = state.visible_rows().len();
                                    if state.selected_row + 1 < total_rows {
                                        state.select_visible_row(state.selected_row + 1);
                                        if let Some(TreeRow::Token(t_idx)) =
                                            state.visible_rows().get(state.selected_row).copied()
                                        {
                                            let spans = get_token_preview_spans(
                                                &state.tokens[t_idx],
                                                right_area,
                                                state.preview_mode,
                                                &preview_styles,
                                            );
                                            click_flash = Some((std::time::Instant::now(), spans));
                                        }
                                    }
                                }
                                KeyCode::PageUp => {
                                    state.select_visible_row(state.selected_row.saturating_sub(5));
                                    if let Some(TreeRow::Token(t_idx)) =
                                        state.visible_rows().get(state.selected_row).copied()
                                    {
                                        let spans = get_token_preview_spans(
                                            &state.tokens[t_idx],
                                            right_area,
                                            state.preview_mode,
                                            &preview_styles,
                                        );
                                        click_flash = Some((std::time::Instant::now(), spans));
                                    }
                                }
                                KeyCode::PageDown => {
                                    let max_row = state.visible_rows().len().saturating_sub(1);
                                    state.select_visible_row((state.selected_row + 5).min(max_row));
                                    if let Some(TreeRow::Token(t_idx)) =
                                        state.visible_rows().get(state.selected_row).copied()
                                    {
                                        let spans = get_token_preview_spans(
                                            &state.tokens[t_idx],
                                            right_area,
                                            state.preview_mode,
                                            &preview_styles,
                                        );
                                        click_flash = Some((std::time::Instant::now(), spans));
                                    }
                                }
                                KeyCode::Home => {
                                    state.select_visible_row(0);
                                    if let Some(TreeRow::Token(t_idx)) =
                                        state.visible_rows().get(state.selected_row).copied()
                                    {
                                        let spans = get_token_preview_spans(
                                            &state.tokens[t_idx],
                                            right_area,
                                            state.preview_mode,
                                            &preview_styles,
                                        );
                                        click_flash = Some((std::time::Instant::now(), spans));
                                    }
                                }
                                KeyCode::End => {
                                    let max_row = state.visible_rows().len().saturating_sub(1);
                                    state.select_visible_row(max_row);
                                    if let Some(TreeRow::Token(t_idx)) =
                                        state.visible_rows().get(state.selected_row).copied()
                                    {
                                        let spans = get_token_preview_spans(
                                            &state.tokens[t_idx],
                                            right_area,
                                            state.preview_mode,
                                            &preview_styles,
                                        );
                                        click_flash = Some((std::time::Instant::now(), spans));
                                    }
                                }
                                KeyCode::F(4) | KeyCode::Char('b') => {
                                    let wp_cfg = state.current_theme.wallpaper_config();
                                    let focus = state
                                        .current_selected_token()
                                        .map(|t| match t.kind {
                                            TokenKind::WallpaperImage => BackgroundFocus::Image,
                                            TokenKind::WallpaperOpacity => BackgroundFocus::Opacity,
                                            TokenKind::WallpaperScaling => BackgroundFocus::Scaling,
                                            TokenKind::WallpaperMode => BackgroundFocus::Mode,
                                            TokenKind::WallpaperClustering => {
                                                BackgroundFocus::Clustering
                                            }
                                            TokenKind::WallpaperQuantization => {
                                                BackgroundFocus::Quantization
                                            }
                                            _ => BackgroundFocus::Image,
                                        })
                                        .unwrap_or(BackgroundFocus::Image);
                                    active_modal = ActiveModal::BackgroundConfig(
                                        BackgroundConfigModal::new(wp_cfg.as_ref())
                                            .with_focus(focus),
                                    );
                                }
                                KeyCode::Delete => {
                                    if matches!(
                                        state.tokens[state.selected_idx].kind,
                                        TokenKind::WallpaperImage
                                            | TokenKind::WallpaperOpacity
                                            | TokenKind::WallpaperScaling
                                            | TokenKind::WallpaperMode
                                            | TokenKind::WallpaperClustering
                                            | TokenKind::WallpaperQuantization
                                    ) {
                                        state.set_wallpaper_config(None);
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message: "Wallpaper removed".to_string(),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 2.0,
                                        });
                                    } else {
                                        active_modal =
                                            ActiveModal::ConfirmDelete(ConfirmModal::new(
                                                "Delete Theme",
                                                format!(
                                                    "Are you sure you want to delete theme '{}'?",
                                                    state.current_name
                                                ),
                                            ));
                                    }
                                }
                                KeyCode::Backspace => {
                                    if matches!(
                                        state.tokens[state.selected_idx].kind,
                                        TokenKind::WallpaperImage
                                            | TokenKind::WallpaperOpacity
                                            | TokenKind::WallpaperScaling
                                            | TokenKind::WallpaperMode
                                            | TokenKind::WallpaperClustering
                                            | TokenKind::WallpaperQuantization
                                    ) {
                                        state.set_wallpaper_config(None);
                                        toast_rack.add(ToastNotification {
                                            title: Some("Theme Editor".to_string()),
                                            message: "Wallpaper removed".to_string(),
                                            severity: ToastSeverity::Information,
                                            timeout_secs: 2.0,
                                        });
                                    }
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    let rows = state.visible_rows();
                                    match rows.get(state.selected_row).copied() {
                                        Some(TreeRow::Category(cat)) => {
                                            state.toggle_category(cat);
                                        }
                                        Some(TreeRow::Token(_)) => {
                                            open_selected_token_picker(
                                                &mut state,
                                                &mut active_modal,
                                            );
                                        }
                                        None => {}
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
                    }
                }
            }
        } else {
            if matches!(active_modal, ActiveModal::None)
                && footer_hover.check_hover_timeout(has_hidden && area.height >= 12)
            {
                need_redraw = true;
            }
            if let Some((flash_start, _)) = click_flash {
                if flash_start.elapsed() >= Duration::from_millis(200) {
                    click_flash = None;
                    need_redraw = true;
                }
            }
        }
    }

    Ok(())
}

/// Opens the appropriate interactive picker modal for the currently selected theme token
/// (e.g., ColorGridModal for colors, AsciiGridModal for glyphs, or cycles toggle values).
fn open_selected_token_picker(state: &mut ThemeEditorState, active_modal: &mut ActiveModal) {
    let token = state
        .current_selected_token()
        .cloned()
        .unwrap_or_else(|| state.tokens[state.selected_idx].clone());
    match token.kind {
        TokenKind::Color(key) => {
            let initial_color = state.get_color_token_color(&key);
            let title = if key == "background" {
                "Pick Background Color"
            } else if key == "highlight_text" {
                "Pick Highlight Text Color"
            } else if key == "highlight" {
                "Pick Highlight Bar Color"
            } else if key == "title" {
                "Pick Title Color"
            } else if key == "window_close_button" {
                "Pick Window Close Button Color"
            } else if key == "footer" {
                "Pick Footer Color"
            } else {
                "Pick Theme Color"
            };
            *active_modal =
                ActiveModal::ColorPicker(key, ColorPickerModal::new(title, initial_color));
        }
        TokenKind::WallpaperImage
        | TokenKind::WallpaperOpacity
        | TokenKind::WallpaperScaling
        | TokenKind::WallpaperMode
        | TokenKind::WallpaperClustering
        | TokenKind::WallpaperQuantization => {
            let wp_cfg = state.current_theme.wallpaper_config();
            let focus = match token.kind {
                TokenKind::WallpaperImage => BackgroundFocus::Image,
                TokenKind::WallpaperOpacity => BackgroundFocus::Opacity,
                TokenKind::WallpaperScaling => BackgroundFocus::Scaling,
                TokenKind::WallpaperMode => BackgroundFocus::Mode,
                TokenKind::WallpaperClustering => BackgroundFocus::Clustering,
                TokenKind::WallpaperQuantization => BackgroundFocus::Quantization,
                _ => BackgroundFocus::Image,
            };
            *active_modal = ActiveModal::BackgroundConfig(
                BackgroundConfigModal::new(wp_cfg.as_ref()).with_focus(focus),
            );
        }
        TokenKind::Border(key) => {
            *active_modal =
                ActiveModal::AsciiPicker(key, AsciiGridModal::new("Pick CP437 Border Char", 196));
        }
        TokenKind::Indicator => {
            state.current_theme.indicator = match state.current_theme.indicator.as_deref() {
                Some("> ") => Some("▶ ".to_string()),
                Some("▶ ") => Some("{ascii:240}".to_string()),
                Some("{ascii:240}") => Some("• ".to_string()),
                _ => Some("> ".to_string()),
            };
            state.is_modified = true;
        }
        TokenKind::DividerChar => {
            *active_modal = ActiveModal::AsciiPicker(
                "divider_char".to_string(),
                AsciiGridModal::new("Pick CP437 Divider Char", 196),
            );
        }
        TokenKind::DividerLength => {
            state.toggle_divider_length();
        }
        TokenKind::MarqueeChar => {
            *active_modal = ActiveModal::AsciiPicker(
                "marquee_char".to_string(),
                AsciiGridModal::new("Pick CP437 Marquee Char", 7),
            );
        }
        TokenKind::MarqueeGap => {
            state.cycle_marquee_gap();
        }
        TokenKind::ToastChar => {
            *active_modal = ActiveModal::AsciiPicker(
                "toast_char".to_string(),
                AsciiGridModal::new("Pick Toast Left Bar Glyph", 221),
            );
        }
    }
}

/// Maps mouse coordinates within the preview area to a theme token key and highlight span.
/// Returns `Some((token_key, x, y, width))` if a clickable theme element was hit.
fn hit_test_preview(
    col: u16,
    row: u16,
    right_area: Rect,
    mode: PreviewMode,
    _is_screen_width: bool,
) -> Option<(&'static str, u16, u16, u16)> {
    let w = right_area.width as usize;
    let avail_both = (w.saturating_sub(6)).max(4);
    let left_gutter_w = avail_both / 2;
    let sep_x = (left_gutter_w + 2).min(w.saturating_sub(2));
    let gt_y = right_area.y + right_area.height.saturating_sub(3);
    let p_inner_x = right_area.x + 2;
    let p_inner_y = right_area.y + 2;
    let p_avail_w = right_area.width.saturating_sub(4);
    let content_start_y = right_area.y + 2;
    let content_avail_h = gt_y.saturating_sub(content_start_y);
    let plugin_start_y = gt_y.saturating_sub(if content_avail_h >= 8 { 3 } else { 1 });

    match mode {
        PreviewMode::Menu => {
            // 1. Top border
            if row == right_area.y {
                let close_x = right_area.x + right_area.width.saturating_sub(4);
                if col >= close_x && col < close_x + 3 {
                    return Some(("window_close_button", row, close_x, close_x + 3));
                }
                if col == right_area.x {
                    return Some(("border_top_left", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("border_top_right", row, col, col + 1));
                }
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            // 2. Bottom border
            if row == right_area.y + right_area.height - 1 {
                if col == right_area.x {
                    return Some(("border_bottom_left", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("border_bottom_right", row, col, col + 1));
                }
                if col == right_area.x + sep_x as u16 {
                    return Some(("gutter_bottom_tee", row, col, col + 1));
                }
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            // 3. Left border
            if col == right_area.x {
                if row == content_start_y + 3 {
                    return Some(("left_tee", row, col, col + 1));
                }
                if content_avail_h >= 6 && row == plugin_start_y {
                    return Some(("left_tee", row, col, col + 1));
                }
                if row == gt_y {
                    return Some(("gutter_left_tee", row, col, col + 1));
                }
                return Some(("border_vertical", row, col, col + 1));
            }
            // 4. Right border
            if col == right_area.x + right_area.width - 1 {
                if row == content_start_y + 3 {
                    return Some(("right_tee", row, col, col + 1));
                }
                if content_avail_h >= 6 && row == plugin_start_y {
                    return Some(("right_tee", row, col, col + 1));
                }
                if row == gt_y {
                    return Some(("gutter_right_tee", row, col, col + 1));
                }
                return Some(("border_vertical", row, col, col + 1));
            }
            // 5. Gutter top border
            if row == gt_y {
                if col == right_area.x + sep_x as u16 {
                    return Some(("gutter_top_tee", row, col, col + 1));
                }
                if col == right_area.x {
                    return Some(("gutter_left_tee", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("gutter_right_tee", row, col, col + 1));
                }
                return Some((
                    "gutter_border",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            // 6. Gutter content row
            if row > gt_y && row < right_area.y + right_area.height - 1 {
                if col == right_area.x + sep_x as u16 {
                    return Some(("gutter_separator", row, col, col + 1));
                }
                if col < right_area.x + sep_x as u16 {
                    return Some((
                        "help_text",
                        row,
                        right_area.x + 2,
                        right_area.x + sep_x as u16,
                    ));
                }
                return Some((
                    "gutter",
                    row,
                    right_area.x + sep_x as u16 + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            // 7. Menu content
            if row == content_start_y {
                // Active menu row
                if col >= p_inner_x && col < p_inner_x + 2 {
                    return Some(("indicator", row, p_inner_x, p_inner_x + 2));
                }
                if col >= p_inner_x + 6 && col < p_inner_x + 9 {
                    return Some(("shortcut_key", row, p_inner_x + 6, p_inner_x + 9));
                }
                if col >= p_inner_x + 2 && col < p_inner_x + 28 {
                    return Some(("highlight_text", row, p_inner_x + 2, p_inner_x + 28));
                }
                return Some(("highlight", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 1 {
                // Item 2: Browse Directory »
                if col >= p_inner_x + 6 && col < p_inner_x + 9 {
                    return Some(("shortcut_key", row, p_inner_x + 6, p_inner_x + 9));
                }
                if (col >= p_inner_x + 25 && col < p_inner_x + 29)
                    || col >= right_area.x + right_area.width.saturating_sub(6)
                {
                    return Some(("accent", row, p_inner_x + 25, p_inner_x + 29));
                }
                return Some(("text", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 2 {
                // Item 3: System Configuration
                if col >= p_inner_x + 6 && col < p_inner_x + 9 {
                    return Some(("shortcut_key", row, p_inner_x + 6, p_inner_x + 9));
                }
                return Some(("text", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 3 {
                // Divider row
                return Some(("divider", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 4 {
                // Item 4: Exit
                if col >= p_inner_x + 6 && col < p_inner_x + 9 {
                    return Some(("shortcut_key", row, p_inner_x + 6, p_inner_x + 9));
                }
                return Some(("text", row, p_inner_x, p_inner_x + p_avail_w));
            }
            // Pseudo plugins
            if content_avail_h >= 6 && row >= plugin_start_y {
                if row == plugin_start_y {
                    return Some(("divider", row, p_inner_x, p_inner_x + p_avail_w));
                }
                return Some(("plugin", row, p_inner_x, p_inner_x + p_avail_w));
            }
            Some((
                "background",
                row,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ))
        }
        PreviewMode::Modals => {
            let p_avail_w = right_area.width.saturating_sub(4);
            let p_avail_h = right_area.height.saturating_sub(2);
            let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
            let mod_h = 12_u16.min(p_avail_h);
            let p_inner_y = right_area.y + 1;
            let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
            let mod_y = p_inner_y + (p_avail_h.saturating_sub(mod_h)) / 2;
            let m_div_y = mod_y + mod_h.saturating_sub(3);
            let m_gutter_y = mod_y + mod_h.saturating_sub(2);

            if col >= mod_x && col < mod_x + mod_w && row >= mod_y && row < mod_y + mod_h {
                if row == mod_y {
                    let close_x = mod_x + mod_w.saturating_sub(4);
                    if col >= close_x && col < close_x + 3 {
                        return Some(("window_close_button", row, close_x, close_x + 3));
                    }
                    if col >= mod_x + 2 && col < mod_x + 27 {
                        return Some(("title", row, mod_x + 2, mod_x + 27));
                    }
                    if col == mod_x {
                        return Some(("border_top_left", row, col, col + 1));
                    }
                    if col == mod_x + mod_w - 1 {
                        return Some(("border_top_right", row, col, col + 1));
                    }
                    return Some(("border_horizontal", row, mod_x + 1, mod_x + mod_w - 1));
                }
                if row == mod_y + mod_h - 1 {
                    if col == mod_x {
                        return Some(("border_bottom_left", row, col, col + 1));
                    }
                    if col == mod_x + mod_w - 1 {
                        return Some(("border_bottom_right", row, col, col + 1));
                    }
                    return Some(("border_horizontal", row, mod_x + 1, mod_x + mod_w - 1));
                }
                if col == mod_x {
                    if row == m_div_y {
                        return Some(("left_tee", row, col, col + 1));
                    }
                    return Some(("border_vertical", row, col, col + 1));
                }
                if col == mod_x + mod_w - 1 {
                    if row == m_div_y {
                        return Some(("right_tee", row, col, col + 1));
                    }
                    if row >= mod_y + 3 && row <= mod_y + 5 {
                        return Some(("scrollbar", row, col, col + 1));
                    }
                    return Some(("border_vertical", row, col, col + 1));
                }
                if row == m_div_y {
                    return Some(("divider", row, mod_x + 1, mod_x + mod_w - 1));
                }
                if row == m_gutter_y {
                    if col < mod_x + mod_w.saturating_sub(12) {
                        return Some((
                            "help_text",
                            row,
                            mod_x + 2,
                            mod_x + mod_w.saturating_sub(12),
                        ));
                    }
                    return Some((
                        "gutter",
                        row,
                        mod_x + mod_w.saturating_sub(12),
                        mod_x + mod_w - 1,
                    ));
                }
                if row == mod_y + 2 {
                    return Some(("text", row, mod_x + 2, mod_x + mod_w - 2));
                }
                if row == mod_y + 3 {
                    return Some(("help_text", row, mod_x + 2, mod_x + mod_w - 2));
                }
                if row == mod_y + 5 {
                    return Some(("text", row, mod_x + 2, mod_x + mod_w - 2));
                }
                if row == mod_y + 7 {
                    if col < mod_x + 18 {
                        return Some(("accent", row, mod_x + 2, mod_x + 18));
                    }
                    return Some(("text", row, mod_x + 18, mod_x + mod_w - 2));
                }
                return Some(("text", row, mod_x + 2, mod_x + mod_w - 2));
            }
            Some((
                "background",
                row,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ))
        }
        PreviewMode::TextEditor => {
            if row == right_area.y + 1 {
                return Some(("highlight", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == right_area.y + 2 {
                return Some(("border", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == right_area.y + right_area.height - 2 {
                return Some(("highlight", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == right_area.y + right_area.height - 1 {
                return Some(("gutter", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if col < p_inner_x + 6 {
                return Some(("gutter", row, p_inner_x, p_inner_x + 6));
            }
            if row == right_area.y + 5 {
                return Some(("highlight", row, p_inner_x, p_inner_x + p_avail_w));
            }
            Some(("background", row, p_inner_x, p_inner_x + p_avail_w))
        }
        PreviewMode::MenuEditor => {
            let bot_y = right_area.y + right_area.height.saturating_sub(1);
            let footer_y = bot_y.saturating_sub(1);
            let panels_y = p_inner_y;
            let panels_h = footer_y.saturating_sub(panels_y);
            let left_w = p_avail_w / 2;
            let right_w = p_avail_w.saturating_sub(left_w);
            let tree_rect = Rect::new(p_inner_x, panels_y, left_w, panels_h);
            let insp_rect = Rect::new(p_inner_x + left_w, panels_y, right_w, panels_h);

            // Bottom border and footer help gutter
            if row == footer_y {
                return Some(("footer", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == bot_y {
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x,
                    right_area.x + right_area.width,
                ));
            }

            // Interior panel borders -> Accent color
            if (row == panels_y || row == panels_y + panels_h.saturating_sub(1))
                && col >= p_inner_x
                && col < p_inner_x + p_avail_w
            {
                return Some(("accent", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if (col == tree_rect.x
                || col == tree_rect.x + tree_rect.width.saturating_sub(1)
                || col == insp_rect.x
                || col == insp_rect.x + insp_rect.width.saturating_sub(1))
                && row >= panels_y
                && row < panels_y + panels_h
            {
                return Some(("accent", row, col, col + 1));
            }

            // Inside Tree panel
            if col > tree_rect.x
                && col < tree_rect.x + tree_rect.width.saturating_sub(1)
                && row > tree_rect.y
                && row < tree_rect.y + tree_rect.height.saturating_sub(1)
            {
                if row == tree_rect.y + 1 {
                    return Some((
                        "title",
                        row,
                        tree_rect.x + 1,
                        tree_rect.x + tree_rect.width.saturating_sub(1),
                    ));
                }
                if row == tree_rect.y + 3 {
                    return Some((
                        "highlight",
                        row,
                        tree_rect.x + 1,
                        tree_rect.x + tree_rect.width.saturating_sub(1),
                    ));
                }
                return Some((
                    "text",
                    row,
                    tree_rect.x + 1,
                    tree_rect.x + tree_rect.width.saturating_sub(1),
                ));
            }

            // Inside Inspector panel
            if col > insp_rect.x
                && col < insp_rect.x + insp_rect.width.saturating_sub(1)
                && row > insp_rect.y
                && row < insp_rect.y + insp_rect.height.saturating_sub(1)
            {
                if row == insp_rect.y + 1 || row == insp_rect.y + 4 {
                    return Some((
                        "accent",
                        row,
                        insp_rect.x + 1,
                        insp_rect.x + insp_rect.width.saturating_sub(1),
                    ));
                }
                if row == insp_rect.y + 3 {
                    return Some((
                        "shortcut_key",
                        row,
                        insp_rect.x + 1,
                        insp_rect.x + insp_rect.width.saturating_sub(1),
                    ));
                }
                return Some((
                    "text",
                    row,
                    insp_rect.x + 1,
                    insp_rect.x + insp_rect.width.saturating_sub(1),
                ));
            }

            Some(("background", row, p_inner_x, p_inner_x + p_avail_w))
        }
        PreviewMode::Toast => {
            let toast_w = (p_avail_w.saturating_sub(6)).clamp(28, 48);
            let toast_x = right_area.x + right_area.width.saturating_sub(toast_w + 3);
            let bot_y = right_area.y + right_area.height.saturating_sub(3);
            let t3_y = bot_y.saturating_sub(3);
            let t2_y = t3_y.saturating_sub(3);
            let t1_y = t2_y.saturating_sub(3);
            let summary_y = right_area.y + right_area.height - 2;

            if row == summary_y {
                return Some(("toast_char", row, p_inner_x, p_inner_x + p_avail_w));
            }

            // Check Toast 1 (Info)
            if row == t1_y || row == t1_y + 1 {
                if col >= toast_x && col < toast_x + 2 {
                    return Some(("toast_char", row, toast_x, toast_x + 2));
                }
                if col >= toast_x + 2 && col < toast_x + toast_w {
                    if row == t1_y {
                        return Some(("toast_info", row, toast_x + 2, toast_x + toast_w));
                    } else {
                        return Some(("text", row, toast_x + 2, toast_x + toast_w));
                    }
                }
            }

            // Check Toast 2 (Warning)
            if row == t2_y || row == t2_y + 1 {
                if col >= toast_x && col < toast_x + 2 {
                    return Some(("toast_char", row, toast_x, toast_x + 2));
                }
                if col >= toast_x + 2 && col < toast_x + toast_w {
                    if row == t2_y {
                        return Some(("toast_warning", row, toast_x + 2, toast_x + toast_w));
                    } else {
                        return Some(("text", row, toast_x + 2, toast_x + toast_w));
                    }
                }
            }

            // Check Toast 3 (Error)
            if row == t3_y || row == t3_y + 1 {
                if col >= toast_x && col < toast_x + 2 {
                    return Some(("toast_char", row, toast_x, toast_x + 2));
                }
                if col >= toast_x + 2 && col < toast_x + toast_w {
                    if row == t3_y {
                        return Some(("toast_error", row, toast_x + 2, toast_x + toast_w));
                    } else {
                        return Some(("text", row, toast_x + 2, toast_x + toast_w));
                    }
                }
            }

            // Outer borders
            if row == right_area.y {
                if col == right_area.x {
                    return Some(("border_top_left", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("border_top_right", row, col, col + 1));
                }
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            if row == right_area.y + right_area.height - 1 {
                if col == right_area.x {
                    return Some(("border_bottom_left", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("border_bottom_right", row, col, col + 1));
                }
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            if col == right_area.x || col == right_area.x + right_area.width - 1 {
                return Some(("border_vertical", row, col, col + 1));
            }

            if row == p_inner_y {
                return Some(("accent", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == p_inner_y + 1 {
                return Some(("help_text", row, p_inner_x, p_inner_x + p_avail_w));
            }

            Some((
                "background",
                row,
                right_area.x + 1,
                right_area.x + right_area.width.saturating_sub(1),
            ))
        }
    }
}
