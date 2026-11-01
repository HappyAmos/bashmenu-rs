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
                        let div_str = "─".repeat(p_avail_w as usize);
                        buf.set_line(
                            p_inner_x,
                            content_start_y + 3,
                            &Line::from(vec![Span::styled(div_str, preview_styles.divider)]),
                            p_avail_w,
                        );

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
                            buf.set_string(right_area.x, plugin_start_y, &b.left_tee, border_style);
                            buf.set_string(
                                right_area.x + right_area.width - 1,
                                plugin_start_y,
                                &b.right_tee,
                                border_style,
                            );
                            let p_div = "─".repeat(p_avail_w as usize);
                            buf.set_line(
                                p_inner_x,
                                plugin_start_y,
                                &Line::from(vec![Span::styled(p_div, preview_styles.divider)]),
                                p_avail_w,
                            );

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
                        buf.set_string(p_inner_x, status_y, &status_text, preview_styles.highlight);

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
                        buf.set_string(mod_x + mod_w - 1, mod_y + 3, "▲", preview_styles.accent);
                        buf.set_string(mod_x + mod_w - 1, mod_y + 4, "█", preview_styles.accent);
                        buf.set_string(mod_x + mod_w - 1, mod_y + 5, "▼", preview_styles.accent);
                    }
                    PreviewMode::MenuEditor => {
                        // Visual Menu Editor Split View
                        let sep_col = p_inner_x + p_avail_w / 2;

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

                        // Center divider with top_tee and bottom_tee
                        buf.set_string(sep_col, right_area.y, &b.top_tee, border_style);
                        for y in p_inner_y..(p_inner_y + p_avail_h.saturating_sub(1)) {
                            buf.set_string(sep_col, y, &b.border_vertical_left, border_style);
                        }
                        buf.set_string(
                            sep_col,
                            right_area.y + right_area.height.saturating_sub(2),
                            &b.bottom_tee,
                            border_style,
                        );

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

                        // Footer
                        let footer_y = right_area.y + right_area.height - 1;
                        let p_footer_items = ["[F1] Help", "[s] Save", "[ESC] Exit"];
                        let p_spread = bashmenu_tui::format_spread_gutter_line(
                            &p_footer_items,
                            p_avail_w as usize,
                        );
                        buf.set_string(p_inner_x, footer_y, p_spread, preview_styles.gutter);
                    }
                }

                // 3. Render Bottom Footer Status
                let footer_y = area.y + area.height - 1;
                let bottom_items = [
                    "[TAB/1-4] Switch Preview",
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

        let poll_dur = if click_flash.is_some() {
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

                            let left_w = (area.width * 45 / 100).clamp(30, 42);
                            let left_area =
                                Rect::new(area.x + 2, area.y + 2, left_w, area.height - 4);
                            let right_x = area.x + left_w + 1;
                            let right_w = (area.x + area.width).saturating_sub(right_x + 2);
                            let right_area =
                                Rect::new(right_x, area.y + 2, right_w, area.height - 4);

                            // 1. Click on left token list
                            if m.column >= left_area.x
                                && m.column < left_area.x + left_area.width
                                && m.row > left_area.y
                                && m.row < left_area.y + left_area.height - 1
                            {
                                let clicked_idx = (m.row.saturating_sub(left_area.y + 1)) as usize;
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
) -> Option<(&'static str, u16, u16, u16)> {
    let w = right_area.width as usize;
    let avail_both = (w.saturating_sub(6)).max(4);
    let left_gutter_w = avail_both / 2;
    let sep_x = (left_gutter_w + 2).min(w.saturating_sub(2));
    let gt_y = right_area.y + right_area.height.saturating_sub(3);
    let p_inner_x = right_area.x + 2;
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
                        return Some(("accent", row, col, col + 1));
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
            if col == sep_col {
                if row == right_area.y {
                    return Some(("top_tee", row, col, col + 1));
                }
                if row == right_area.y + right_area.height.saturating_sub(2) {
                    return Some(("bottom_tee", row, col, col + 1));
                }
                return Some(("border_vertical", row, col, col + 1));
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
    }
}
