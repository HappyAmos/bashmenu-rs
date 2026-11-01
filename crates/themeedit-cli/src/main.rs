//! Visual Theme Editor for `bashmenu.themes`.
//!
//! Provides a side-by-side terminal interface for editing theme colors,
//! window borders, and indicators with live preview switching across
//! Main Menu, Text Editor, Popup Windows, and Visual Menu Editor.

use anyhow::Result;
use bashmenu_core::{ToastNotification, ToastSeverity};
use bashmenu_tui::{
    composite_buffer, install_panic_hook, AsciiGridModal, ColorGridModal, ConfirmModal, InputModal,
    ResolvedThemeStyles, TerminalGuard, ToastRack, WallpaperCompositor,
};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::{Path, PathBuf};
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

    /// Load and save theme files in current working directory
    #[arg(long)]
    local: bool,
}

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
    let themes_path = resolve_themes_path(&cli.file, cli.local);
    let mut state = ThemeEditorState::new(themes_path, cli.theme);

    let mut guard = TerminalGuard::init()?;
    let mut active_modal = ActiveModal::None;
    let mut last_grid_origin = (0, 0);
    let mut wp_compositor = state
        .current_theme
        .wallpaper_config()
        .map(WallpaperCompositor::new);
    let mut need_redraw = true;
    let mut click_flash: Option<(std::time::Instant, u16, u16, u16)> = None;
    let mut toast_rack = ToastRack::new();

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
                // Reserve bottom 2 rows for gutter divider and gutter help bar
                let bot_y = area.y + area.height - 1;
                let footer_y = bot_y.saturating_sub(1);
                let gt_y = footer_y.saturating_sub(1);
                let panel_y = area.y + 2;
                let panel_h = gt_y.saturating_sub(panel_y);

                let left_w = (area.width * 45 / 100).clamp(30, 42);
                let left_area = Rect::new(area.x + 2, panel_y, left_w - 3, panel_h);
                let right_x = area.x + left_w + 1;
                let right_w = (area.x + area.width).saturating_sub(right_x + 2);
                let right_area = Rect::new(right_x, panel_y, right_w, panel_h);

                // 1. Render Left Panel: Token List
                let visible_rows = (left_area.height.saturating_sub(2)) as usize;
                state.adjust_scroll(visible_rows);

                let scroll_up_ind = if state.scroll_offset > 0 { " ▲" } else { "" };
                let scroll_dn_ind = if state.scroll_offset + visible_rows < state.tokens.len() {
                    " ▼"
                } else {
                    ""
                };
                let list_title = format!(" Theme Tokens{}{}", scroll_up_ind, scroll_dn_ind);
                buf.set_string(
                    left_area.x,
                    left_area.y,
                    list_title,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );

                for (v_idx, token) in state
                    .tokens
                    .iter()
                    .enumerate()
                    .skip(state.scroll_offset)
                    .take(visible_rows)
                {
                    let row_y = left_area.y + 2 + (v_idx - state.scroll_offset) as u16;

                    let is_sel = v_idx == state.selected_idx;
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
                let tab_toast = if state.preview_mode == PreviewMode::Toast {
                    "▶ 5: Toasts ◀"
                } else {
                    "5: Toasts"
                };

                let preview_title = format!(
                    " [ Preview: {} | {} | {} | {} | {} ] ",
                    tab_menu, tab_ed, tab_mod, tab_me, tab_toast
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
                let b = &preview_styles.borders;
                let border_style = Style::default().fg(preview_styles.border);

                match state.preview_mode {
                    PreviewMode::Menu => {
                        let w = right_area.width as usize;

                        // 1. Gutter layout calculation
                        let avail_both = (w.saturating_sub(6)).max(4);
                        let left_gutter_w = avail_both / 2;
                        let sep_x = (left_gutter_w + 2).min(w.saturating_sub(2));
                        let left_seg_len = sep_x.saturating_sub(1);
                        let right_seg_len = w.saturating_sub(2 + sep_x);

                        let bot_y = right_area.y + right_area.height - 1;
                        let gy = bot_y.saturating_sub(1);
                        let gt_y = gy.saturating_sub(1);

                        // 2. Window Bottom Border with gutter bottom tee
                        let bot_line = format!(
                            "{}{}{}{}{}",
                            b.border_bottom_left,
                            b.border_horizontal_bottom.repeat(left_seg_len),
                            b.gutter_bottom_tee,
                            b.border_horizontal_bottom.repeat(right_seg_len),
                            b.border_bottom_right,
                        );
                        buf.set_string(right_area.x, bot_y, &bot_line, border_style);

                        // 3. Gutter Content Row (Pseudo Help Gutter on left, Pseudo Status Gutter on right)
                        buf.set_string(right_area.x, gy, &b.border_vertical_left, border_style);
                        buf.set_string(
                            right_area.x + sep_x as u16,
                            gy,
                            &b.gutter_separator,
                            border_style,
                        );
                        buf.set_string(
                            right_area.x + right_area.width - 1,
                            gy,
                            &b.border_vertical_right,
                            border_style,
                        );

                        let help_items =
                            ["[UP/DN]: Nav", "[ENTER]: Run", "[F1]: Help", "[F2]: Themes"];
                        let help_avail_w = sep_x.saturating_sub(3);
                        let help_spread =
                            bashmenu_tui::format_spread_gutter_line(&help_items, help_avail_w);
                        let help_spans = bashmenu_tui::parse_bbcode_spans(
                            &help_spread,
                            preview_styles.help_text,
                        );
                        buf.set_line(
                            right_area.x + 2,
                            gy,
                            &Line::from(help_spans),
                            help_avail_w as u16,
                        );

                        let status_text = "16:56:00 | 49079 | Host: desktop";
                        let status_vis = bashmenu_tui::visible_text_len(status_text);
                        let status_avail = right_seg_len.saturating_sub(2);
                        let status_pad = status_avail.saturating_sub(status_vis);
                        let mut s_spans =
                            vec![Span::styled(" ".repeat(status_pad), preview_styles.gutter)];
                        s_spans.append(&mut bashmenu_tui::parse_bbcode_spans(
                            status_text,
                            preview_styles.gutter,
                        ));
                        buf.set_line(
                            right_area.x + sep_x as u16 + 2,
                            gy,
                            &Line::from(s_spans),
                            status_avail as u16,
                        );

                        // 4. Gutter Top Border
                        let gt_line = format!(
                            "{}{}{}{}{}",
                            b.gutter_left_tee,
                            b.gutter_border.repeat(left_seg_len),
                            b.gutter_top_tee,
                            b.gutter_border.repeat(right_seg_len),
                            b.gutter_right_tee,
                        );
                        buf.set_string(right_area.x, gt_y, &gt_line, border_style);

                        // 5. Menu Items
                        let content_start_y = p_inner_y;
                        let content_avail_h = gt_y.saturating_sub(content_start_y) as usize;

                        // Active item
                        let ind = &preview_styles.indicator_symbol;
                        let sc_active_style = Style::default()
                            .fg(preview_styles.shortcut_key.fg.unwrap_or(Color::Magenta))
                            .bg(preview_styles.highlight.bg.unwrap_or(Color::Cyan))
                            .add_modifier(Modifier::BOLD);
                        let mut active_spans = vec![
                            Span::styled(ind, preview_styles.highlight),
                            Span::styled("  🚀 ", preview_styles.highlight),
                            Span::styled("1. ", sc_active_style),
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
                        buf.set_line(
                            p_inner_x,
                            content_start_y,
                            &Line::from(active_spans),
                            p_avail_w,
                        );

                        // Inactive item 2
                        let normal_line = Line::from(vec![
                            Span::raw("  "),
                            Span::styled("  📁 ", preview_styles.text),
                            Span::styled("2. ", preview_styles.shortcut_key),
                            Span::styled("Browse Directory", preview_styles.text),
                            Span::styled("  »", preview_styles.accent),
                        ]);
                        buf.set_line(p_inner_x, content_start_y + 1, &normal_line, p_avail_w);

                        // Inactive item 3
                        let note_line = Line::from(vec![
                            Span::raw("  "),
                            Span::styled("  📝 ", preview_styles.text),
                            Span::styled("3. ", preview_styles.shortcut_key),
                            Span::styled("Notes & Tasks", preview_styles.text),
                            Span::styled("  [notes.yml]", preview_styles.gutter),
                        ]);
                        buf.set_line(p_inner_x, content_start_y + 2, &note_line, p_avail_w);

                        // Divider rule
                        let div_char = &preview_styles.divider_char;
                        if preview_styles.divider_is_screen_width {
                            buf.set_string(
                                right_area.x,
                                content_start_y + 3,
                                &b.left_tee,
                                border_style,
                            );
                            buf.set_string(
                                right_area.x + right_area.width - 1,
                                content_start_y + 3,
                                &b.right_tee,
                                border_style,
                            );
                            let div_w = right_area.width.saturating_sub(2) as usize;
                            let div_str = div_char.repeat(div_w);
                            buf.set_line(
                                right_area.x + 1,
                                content_start_y + 3,
                                &Line::from(vec![Span::styled(div_str, preview_styles.divider)]),
                                div_w as u16,
                            );
                        } else {
                            let div_str = div_char.repeat(p_avail_w as usize);
                            buf.set_line(
                                p_inner_x,
                                content_start_y + 3,
                                &Line::from(vec![Span::styled(div_str, preview_styles.divider)]),
                                p_avail_w,
                            );
                        }

                        // Exit item
                        let exit_line = Line::from(vec![
                            Span::raw("  "),
                            Span::styled("  ❌ ", preview_styles.text),
                            Span::styled("4. ", preview_styles.shortcut_key),
                            Span::styled("Exit to Terminal", preview_styles.text),
                        ]);
                        buf.set_line(p_inner_x, content_start_y + 4, &exit_line, p_avail_w);

                        // 6. Pseudo Plugins pinned above gutter
                        if content_avail_h >= 8 {
                            let plugin_start_y = gt_y.saturating_sub(3);
                            if preview_styles.divider_is_screen_width {
                                buf.set_string(
                                    right_area.x,
                                    plugin_start_y,
                                    &b.left_tee,
                                    border_style,
                                );
                                buf.set_string(
                                    right_area.x + right_area.width - 1,
                                    plugin_start_y,
                                    &b.right_tee,
                                    border_style,
                                );
                                let div_w = right_area.width.saturating_sub(2) as usize;
                                let p_div = div_char.repeat(div_w);
                                buf.set_line(
                                    right_area.x + 1,
                                    plugin_start_y,
                                    &Line::from(vec![Span::styled(p_div, preview_styles.divider)]),
                                    div_w as u16,
                                );
                            } else {
                                let p_div = div_char.repeat(p_avail_w as usize);
                                buf.set_line(
                                    p_inner_x,
                                    plugin_start_y,
                                    &Line::from(vec![Span::styled(p_div, preview_styles.divider)]),
                                    p_avail_w,
                                );
                            }

                            let plugin_row1 = Line::from(vec![
                                Span::styled(" ☁️  Weather: ", preview_styles.accent),
                                Span::styled("+59°F Overcast", preview_styles.text),
                                Span::styled("  │  Alienware: ", preview_styles.divider),
                                Span::styled("Online", preview_styles.accent),
                            ]);
                            buf.set_line(p_inner_x, plugin_start_y + 1, &plugin_row1, p_avail_w);

                            let plugin_row2 = Line::from(vec![
                                Span::styled(" 🖥️  CPU: ", preview_styles.accent),
                                Span::styled("12%", preview_styles.text),
                                Span::styled("  │  Mem: ", preview_styles.divider),
                                Span::styled("3.8G/16G", preview_styles.text),
                                Span::styled("  │  Pi-3B: ", preview_styles.divider),
                                Span::styled("Online", preview_styles.accent),
                            ]);
                            buf.set_line(p_inner_x, plugin_start_y + 2, &plugin_row2, p_avail_w);
                        } else if content_avail_h >= 6 {
                            let plugin_y = gt_y.saturating_sub(1);
                            let plugin_row = Line::from(vec![
                                Span::styled(" ☁️  Weather: ", preview_styles.accent),
                                Span::styled("+59°F Overcast", preview_styles.text),
                                Span::styled("  │  Alienware: ", preview_styles.divider),
                                Span::styled("Online", preview_styles.accent),
                            ]);
                            buf.set_line(p_inner_x, plugin_y, &plugin_row, p_avail_w);
                        }

                        // 7. Wallpaper overlay
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
                            if y >= right_area.y + right_area.height.saturating_sub(3) {
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
                        let bot_y = right_area.y + right_area.height.saturating_sub(1);
                        let footer_y = bot_y.saturating_sub(1);
                        let status_y = footer_y.saturating_sub(1);

                        buf.set_string(
                            right_area.x,
                            status_y,
                            &b.border_vertical_left,
                            border_style,
                        );
                        buf.set_string(
                            right_area.x + right_area.width - 1,
                            status_y,
                            &b.border_vertical_right,
                            border_style,
                        );
                        let status_text = format!(
                            " NORMAL │ bashmenu.yml │ utf-8 │ 6L, 128B │ 3:12 {:<width$}",
                            "",
                            width = p_avail_w.saturating_sub(48) as usize
                        );
                        buf.set_string(p_inner_x, status_y, &status_text, preview_styles.highlight);

                        // 4. Editor Command Footer
                        buf.set_string(
                            right_area.x,
                            footer_y,
                            &b.border_vertical_left,
                            border_style,
                        );
                        buf.set_string(
                            right_area.x + right_area.width - 1,
                            footer_y,
                            &b.border_vertical_right,
                            border_style,
                        );
                        buf.set_string(
                            p_inner_x,
                            footer_y,
                            "^G Help  ^O WriteOut  ^R ReadFile  ^X Exit",
                            preview_styles.gutter,
                        );
                    }
                    PreviewMode::Modals => {
                        // Modal dialog box centered within preview area
                        let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
                        let mod_h = 12_u16.min(p_avail_h);
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
                        let inp_str = format!(
                            " [ {:<width$} ]",
                            "cluster_production_01",
                            width = inp_w.saturating_sub(4)
                        );
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

                        // Modal Gutter Divider
                        let m_div_y = mod_y + mod_h.saturating_sub(3);
                        buf.set_string(mod_x, m_div_y, &b.left_tee, border_style);
                        buf.set_string(mod_x + mod_w - 1, m_div_y, &b.right_tee, border_style);
                        let m_div_str = "─".repeat(mod_w.saturating_sub(2) as usize);
                        buf.set_string(mod_x + 1, m_div_y, &m_div_str, border_style);

                        // Modal Help Gutter Text
                        let m_gutter_y = mod_y + mod_h.saturating_sub(2);
                        let m_help = "[ENTER]: Confirm | [ESC]: Cancel";
                        buf.set_string(mod_x + 2, m_gutter_y, m_help, preview_styles.help_text);
                        let m_stat = "[F1]: Help";
                        if mod_w >= 36 {
                            buf.set_string(
                                mod_x + mod_w - 2 - m_stat.len() as u16,
                                m_gutter_y,
                                m_stat,
                                preview_styles.gutter,
                            );
                        }

                        // Scrollbar indicator on right border
                        let sb_st = preview_styles.scrollbar;
                        buf.set_string(mod_x + mod_w - 1, mod_y + 3, "▲", sb_st);
                        buf.set_string(mod_x + mod_w - 1, mod_y + 4, "█", sb_st);
                        buf.set_string(mod_x + mod_w - 1, mod_y + 5, "▼", sb_st);
                    }
                    PreviewMode::MenuEditor => {
                        // Visual Menu Editor Split View
                        let sep_col = p_inner_x + p_avail_w / 2;
                        let bot_y = right_area.y + right_area.height.saturating_sub(1);
                        let footer_y = bot_y.saturating_sub(1);
                        let gt_y = footer_y.saturating_sub(1);

                        // Left Pane: Tree
                        buf.set_string(p_inner_x, p_inner_y, "▼ Main Menu", preview_styles.title);
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

                        // Center vertical divider
                        buf.set_string(sep_col, right_area.y, &b.top_tee, border_style);
                        for y in p_inner_y..gt_y {
                            buf.set_string(sep_col, y, &b.border_vertical_left, border_style);
                        }

                        // Right Pane: Property Inspector
                        let r_x = sep_col + 2;
                        buf.set_string(
                            r_x,
                            p_inner_y,
                            "Title:  [ Terminal ]",
                            preview_styles.accent,
                        );
                        buf.set_string(r_x, p_inner_y + 1, "Icon:   [ 💻 ]", preview_styles.text);
                        buf.set_string(
                            r_x,
                            p_inner_y + 2,
                            "Action: [ alacritty ]",
                            preview_styles.shortcut_key,
                        );
                        buf.set_string(
                            r_x,
                            p_inner_y + 3,
                            "Type:   [ command ]",
                            preview_styles.accent,
                        );

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

                        // Gutter top border divider (connecting center divider to outer borders)
                        buf.set_string(right_area.x, gt_y, &b.left_tee, border_style);
                        buf.set_string(
                            right_area.x + right_area.width - 1,
                            gt_y,
                            &b.right_tee,
                            border_style,
                        );
                        let div_left_len = sep_col.saturating_sub(right_area.x + 1) as usize;
                        let div_left = b.border_horizontal_bottom.repeat(div_left_len);
                        buf.set_string(right_area.x + 1, gt_y, &div_left, border_style);
                        buf.set_string(sep_col, gt_y, &b.bottom_tee, border_style);

                        let div_right_start = sep_col + 1;
                        let div_right_end = right_area.x + right_area.width - 1;
                        let div_right_len = div_right_end.saturating_sub(div_right_start) as usize;
                        let div_right = b.border_horizontal_bottom.repeat(div_right_len);
                        buf.set_string(div_right_start, gt_y, &div_right, border_style);

                        // Footer help gutter
                        buf.set_string(
                            right_area.x,
                            footer_y,
                            &b.border_vertical_left,
                            border_style,
                        );
                        buf.set_string(
                            right_area.x + right_area.width - 1,
                            footer_y,
                            &b.border_vertical_right,
                            border_style,
                        );
                        let p_footer_items = ["[F1] Help", "[s] Save", "[ESC] Exit"];
                        let p_spread = bashmenu_tui::format_spread_gutter_line(
                            &p_footer_items,
                            p_avail_w as usize,
                        );
                        buf.set_string(p_inner_x, footer_y, &p_spread, preview_styles.gutter);
                    }
                    PreviewMode::Toast => {
                        // Title banner
                        buf.set_string(
                            p_inner_x,
                            p_inner_y,
                            "▶ Toast Notification Stacking & Accent Styles",
                            preview_styles.title,
                        );
                        buf.set_string(
                            p_inner_x,
                            p_inner_y + 1,
                            "Click any toast accent bar or text to customize color and glyphs",
                            preview_styles.help_text,
                        );

                        let bg_color = match preview_styles.background {
                            Color::Reset => Color::Rgb(52, 63, 73),
                            c => c,
                        };
                        let bar_glyph = &preview_styles.toast_bar_char;

                        // 3 Stacked Sample Toasts docked bottom-right
                        let toast_w = (p_avail_w.saturating_sub(6)).clamp(28, 48);
                        let toast_x = right_area.x + right_area.width.saturating_sub(toast_w + 3);

                        let bot_y = right_area.y + right_area.height.saturating_sub(3);

                        // Toast 3 (Error) at bottom
                        let t3_y = bot_y.saturating_sub(3);
                        let t3_style = Style::default().fg(preview_styles.toast_error).bg(bg_color);
                        let t3_text_style = Style::default()
                            .fg(preview_styles.text.fg.unwrap_or(Color::White))
                            .bg(bg_color);
                        buf.set_string(toast_x, t3_y, bar_glyph, t3_style);
                        buf.set_string(
                            toast_x + 2,
                            t3_y,
                            "Error",
                            t3_style.add_modifier(Modifier::BOLD),
                        );
                        buf.set_string(toast_x, t3_y + 1, bar_glyph, t3_style);
                        buf.set_string(
                            toast_x + 2,
                            t3_y + 1,
                            "Connection failed on port 8080",
                            t3_text_style,
                        );

                        // Toast 2 (Warning) above Toast 3
                        let t2_y = t3_y.saturating_sub(3);
                        let t2_style = Style::default()
                            .fg(preview_styles.toast_warning)
                            .bg(bg_color);
                        let t2_text_style = Style::default()
                            .fg(preview_styles.text.fg.unwrap_or(Color::White))
                            .bg(bg_color);
                        buf.set_string(toast_x, t2_y, bar_glyph, t2_style);
                        buf.set_string(
                            toast_x + 2,
                            t2_y,
                            "Warning",
                            t2_style.add_modifier(Modifier::BOLD),
                        );
                        buf.set_string(toast_x, t2_y + 1, bar_glyph, t2_style);
                        buf.set_string(
                            toast_x + 2,
                            t2_y + 1,
                            "High memory consumption (88%)",
                            t2_text_style,
                        );

                        // Toast 1 (Information) above Toast 2
                        let t1_y = t2_y.saturating_sub(3);
                        let t1_style = Style::default().fg(preview_styles.toast_info).bg(bg_color);
                        let t1_text_style = Style::default()
                            .fg(preview_styles.text.fg.unwrap_or(Color::White))
                            .bg(bg_color);
                        buf.set_string(toast_x, t1_y, bar_glyph, t1_style);
                        buf.set_string(
                            toast_x + 2,
                            t1_y,
                            "Information",
                            t1_style.add_modifier(Modifier::BOLD),
                        );
                        buf.set_string(toast_x, t1_y + 1, bar_glyph, t1_style);
                        buf.set_string(
                            toast_x + 2,
                            t1_y + 1,
                            "Saved configuration to disk",
                            t1_text_style,
                        );

                        // Bottom configuration summary footer
                        let summary_y = right_area.y + right_area.height - 2;
                        let summary_txt = format!(
                            "Bar: '{}'  Timeout: {:.1}s  [Enter: Cycle / Pick]",
                            preview_styles.toast_bar_char, preview_styles.toast_timeout
                        );
                        buf.set_string(p_inner_x, summary_y, summary_txt, preview_styles.gutter);
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

                // Gutter row side borders
                buf.set_string(area.x, footer_y, &b.border_vertical_left, border_style);
                buf.set_string(
                    area.x + area.width - 1,
                    footer_y,
                    &b.border_vertical_right,
                    border_style,
                );

                let bottom_items = [
                    "[TAB/1-5] Switch Preview",
                    "[Enter] Pick",
                    "[Ctrl+S] Save",
                    "[Ctrl+A] Save As",
                    "[q] Exit",
                ];
                let bottom_avail_w = area.width.saturating_sub(4) as usize;
                let bottom_spread =
                    bashmenu_tui::format_spread_gutter_line(&bottom_items, bottom_avail_w);
                buf.set_string(
                    area.x + 2,
                    footer_y,
                    bottom_spread,
                    Style::default().fg(preview_styles.help_text.fg.unwrap_or(Color::White)),
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
                    ActiveModal::ConfirmExit(modal) => {
                        modal.render(area, &preview_styles, buf);
                    }
                    ActiveModal::None => {}
                }

                // 5. Visual/Haptic feedback flash for preview panel clicks (Modifier::REVERSED for 150ms)
                if let Some((flash_start, fy, fx_start, fx_end)) = click_flash {
                    if flash_start.elapsed() < Duration::from_millis(150) {
                        for x in fx_start..fx_end {
                            if let Some(cell) = buf.cell_mut((x, fy)) {
                                cell.modifier |= Modifier::REVERSED;
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

        if event::poll(poll_dur)? {
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
                        if let Some(res) = modal.handle_key(k.code) {
                            if let Some(new_name) = res {
                                let trimmed = new_name.trim();
                                if !trimmed.is_empty() {
                                    state.current_name = trimmed.to_string();
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
                                    state.current_name = trimmed.to_string();
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
                        if toast_rack.handle_mouse(m.column, m.row) {
                            continue;
                        }

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

                        if m.kind == MouseEventKind::ScrollUp {
                            if m.column >= left_area.x
                                && m.column < left_area.x + left_area.width
                                && state.selected_idx > 0
                            {
                                state.selected_idx -= 1;
                            }
                            continue;
                        }
                        if m.kind == MouseEventKind::ScrollDown {
                            if m.column >= left_area.x
                                && m.column < left_area.x + left_area.width
                                && state.selected_idx + 1 < state.tokens.len()
                            {
                                state.selected_idx += 1;
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

                            // 1. Click on left token list
                            let visible_rows = (left_area.height.saturating_sub(2)) as usize;
                            if m.column >= left_area.x
                                && m.column < left_area.x + left_area.width
                                && m.row >= left_area.y + 2
                                && m.row < left_area.y + 2 + visible_rows as u16
                            {
                                let clicked_idx =
                                    state.scroll_offset + (m.row - (left_area.y + 2)) as usize;
                                if clicked_idx < state.tokens.len() {
                                    state.selected_idx = clicked_idx;
                                    state.status_message =
                                        format!("Selected: {}", state.tokens[clicked_idx].label);
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
                                    state.select_token_by_key(key);
                                    click_flash =
                                        Some((std::time::Instant::now(), fy, fx_start, fx_end));
                                    need_redraw = true;
                                }
                                continue;
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
                                                    message: format!("Failed to save theme: {}", e),
                                                    severity: ToastSeverity::Error,
                                                    timeout_secs: 3.0,
                                                });
                                            }
                                        }
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
                            KeyCode::Char('5') => {
                                state.preview_mode = PreviewMode::Toast;
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
                            KeyCode::PageUp => {
                                state.selected_idx = state.selected_idx.saturating_sub(5);
                            }
                            KeyCode::PageDown => {
                                state.selected_idx = (state.selected_idx + 5)
                                    .min(state.tokens.len().saturating_sub(1));
                            }
                            KeyCode::Home => {
                                state.selected_idx = 0;
                            }
                            KeyCode::End => {
                                state.selected_idx = state.tokens.len().saturating_sub(1);
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
                                        // Cycle indicator
                                        state.current_theme.indicator =
                                            match state.current_theme.indicator.as_deref() {
                                                Some("> ") => Some("▶ ".to_string()),
                                                Some("▶ ") => Some("{ascii:240}".to_string()),
                                                Some("{ascii:240}") => Some("• ".to_string()),
                                                _ => Some("> ".to_string()),
                                            };
                                        state.is_modified = true;
                                    }
                                    TokenKind::DividerChar => {
                                        active_modal = ActiveModal::AsciiPicker(
                                            "divider_char".to_string(),
                                            AsciiGridModal::new("Pick CP437 Divider Char", 196),
                                        );
                                    }
                                    TokenKind::DividerLength => {
                                        state.toggle_divider_length();
                                    }
                                    TokenKind::MarqueeChar => {
                                        active_modal = ActiveModal::AsciiPicker(
                                            "marquee_char".to_string(),
                                            AsciiGridModal::new("Pick CP437 Marquee Char", 7),
                                        );
                                    }
                                    TokenKind::MarqueeGap => {
                                        state.cycle_marquee_gap();
                                    }
                                    TokenKind::ToastChar => {
                                        active_modal = ActiveModal::AsciiPicker(
                                            "toast_char".to_string(),
                                            AsciiGridModal::new("Pick Toast Left Bar Glyph", 221),
                                        );
                                    }
                                    TokenKind::ToastTimeout => {
                                        state.cycle_toast_timeout();
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
        } else if let Some((flash_start, _, _, _)) = click_flash {
            if flash_start.elapsed() >= Duration::from_millis(150) {
                click_flash = None;
                need_redraw = true;
            }
        }
    }

    Ok(())
}

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
                if col >= p_inner_x + 7 && col < p_inner_x + 10 {
                    return Some(("shortcut_key", row, p_inner_x + 7, p_inner_x + 10));
                }
                return Some(("highlight", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 1 {
                // Item 2: Browse Directory
                if col >= p_inner_x + 7 && col < p_inner_x + 10 {
                    return Some(("shortcut_key", row, p_inner_x + 7, p_inner_x + 10));
                }
                if col >= right_area.x + right_area.width.saturating_sub(6) {
                    return Some((
                        "accent",
                        row,
                        right_area.x + right_area.width.saturating_sub(6),
                        right_area.x + right_area.width - 1,
                    ));
                }
                return Some((
                    "text",
                    row,
                    p_inner_x + 10,
                    right_area.x + right_area.width - 6,
                ));
            }
            if row == content_start_y + 2 {
                // Item 3: Notes & Tasks [notes.yml]
                if col >= p_inner_x + 7 && col < p_inner_x + 10 {
                    return Some(("shortcut_key", row, p_inner_x + 7, p_inner_x + 10));
                }
                if col >= right_area.x + right_area.width.saturating_sub(18) {
                    return Some((
                        "gutter",
                        row,
                        right_area.x + right_area.width.saturating_sub(18),
                        right_area.x + right_area.width - 1,
                    ));
                }
                return Some((
                    "text",
                    row,
                    p_inner_x + 10,
                    right_area.x + right_area.width - 18,
                ));
            }
            if row == content_start_y + 3 {
                // Divider row
                return Some(("divider", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == content_start_y + 4 {
                // Item 4: Exit
                if col >= p_inner_x + 7 && col < p_inner_x + 10 {
                    return Some(("shortcut_key", row, p_inner_x + 7, p_inner_x + 10));
                }
                return Some((
                    "text",
                    row,
                    p_inner_x + 10,
                    right_area.x + right_area.width - 2,
                ));
            }
            // Pseudo plugins
            if content_avail_h >= 6 && row >= plugin_start_y {
                if row == plugin_start_y {
                    return Some(("divider", row, p_inner_x, p_inner_x + p_avail_w));
                }
                return Some(("accent", row, p_inner_x, p_inner_x + p_avail_w));
            }
            Some(("text", row, p_inner_x, p_inner_x + p_avail_w))
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
            Some(("border", row, right_area.x, right_area.x + right_area.width))
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
            Some(("text", row, p_inner_x, p_inner_x + p_avail_w))
        }
        PreviewMode::MenuEditor => {
            let sep_col = p_inner_x + p_avail_w / 2;
            let bot_y = right_area.y + right_area.height.saturating_sub(1);
            let footer_y = bot_y.saturating_sub(1);
            let gt_y = footer_y.saturating_sub(1);

            if col == sep_col {
                if row == right_area.y {
                    return Some(("top_tee", row, col, col + 1));
                }
                if row == gt_y {
                    return Some(("bottom_tee", row, col, col + 1));
                }
                return Some(("border_vertical", row, col, col + 1));
            }
            if row == gt_y {
                if col == right_area.x {
                    return Some(("left_tee", row, col, col + 1));
                }
                if col == right_area.x + right_area.width - 1 {
                    return Some(("right_tee", row, col, col + 1));
                }
                return Some((
                    "border_horizontal",
                    row,
                    right_area.x + 1,
                    right_area.x + right_area.width - 1,
                ));
            }
            if row == footer_y {
                return Some(("gutter", row, p_inner_x, p_inner_x + p_avail_w));
            }
            if row == bot_y {
                return Some((
                    "border_bottom",
                    row,
                    right_area.x,
                    right_area.x + right_area.width,
                ));
            }
            if col < sep_col {
                if row == right_area.y + 1 {
                    return Some(("accent", row, p_inner_x, sep_col));
                }
                if row == right_area.y + 3 {
                    return Some(("highlight", row, p_inner_x, sep_col));
                }
                Some(("text", row, p_inner_x, sep_col))
            } else {
                if row == right_area.y + 1 {
                    return Some(("accent", row, sep_col + 1, p_inner_x + p_avail_w));
                }
                Some(("text", row, sep_col + 1, p_inner_x + p_avail_w))
            }
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
                return Some(("toast_timeout", row, p_inner_x, p_inner_x + p_avail_w));
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

            Some(("text", row, p_inner_x, p_inner_x + p_avail_w))
        }
    }
}
