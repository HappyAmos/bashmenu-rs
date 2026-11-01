//! Live preview pane rendering for ThemeEdit.

use crate::PreviewMode;
use bashmenu_tui::{
    composite_buffer, format_spread_gutter_line, parse_bbcode_spans, visible_text_len,
    ResolvedThemeStyles, WallpaperCompositor,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the preview pane for ThemeEdit across `right_area` into `buf`.
pub fn render_preview_pane(
    right_area: Rect,
    buf: &mut Buffer,
    mode: PreviewMode,
    preview_styles: &ResolvedThemeStyles,
    wp_compositor: &mut Option<WallpaperCompositor>,
) {
    if right_area.width < 10 || right_area.height < 6 {
        return;
    }

    let tab_menu = if mode == PreviewMode::Menu {
        "▶ 1: Menu ◀"
    } else {
        "1: Menu"
    };
    let tab_ed = if mode == PreviewMode::TextEditor {
        "▶ 2: Editor ◀"
    } else {
        "2: Editor"
    };
    let tab_mod = if mode == PreviewMode::Modals {
        "▶ 3: Modals ◀"
    } else {
        "3: Modals"
    };
    let tab_me = if mode == PreviewMode::MenuEditor {
        "▶ 4: MenuEdit ◀"
    } else {
        "4: MenuEdit"
    };
    let tab_toast = if mode == PreviewMode::Toast {
        "▶ 5: Toasts ◀"
    } else {
        "5: Toasts"
    };

    let preview_title = format!(
        "─[ Preview: {} | {} | {} | {} | {} ]",
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

    match mode {
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

            let help_items = ["[UP/DN]: Nav", "[ENTER]: Run", "[F1]: Help", "[F2]: Themes"];
            let help_avail_w = sep_x.saturating_sub(3);
            let help_spread = format_spread_gutter_line(&help_items, help_avail_w);
            let help_spans = parse_bbcode_spans(&help_spread, preview_styles.help_text);
            buf.set_line(
                right_area.x + 2,
                gy,
                &Line::from(help_spans),
                help_avail_w as u16,
            );

            let status_text = "16:56:00 | 49079 | Host: desktop";
            let status_vis = visible_text_len(status_text);
            let status_avail = right_seg_len.saturating_sub(2);
            let status_pad = status_avail.saturating_sub(status_vis);
            let mut s_spans = vec![Span::styled(" ".repeat(status_pad), preview_styles.gutter)];
            s_spans.append(&mut parse_bbcode_spans(status_text, preview_styles.gutter));
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
            let active_vis: usize = active_spans.iter().map(|s| s.content.chars().count()).sum();
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
            let normal_line2 = Line::from(vec![
                Span::raw("  "),
                Span::styled("  ⚙️  ", preview_styles.text),
                Span::styled("3. ", preview_styles.shortcut_key),
                Span::styled("System Configuration", preview_styles.text),
            ]);
            buf.set_line(p_inner_x, content_start_y + 2, &normal_line2, p_avail_w);

            // Divider row
            let div_char = &preview_styles.divider_char;
            if preview_styles.divider_is_screen_width {
                buf.set_string(right_area.x, content_start_y + 3, &b.left_tee, border_style);
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
                    buf.set_string(right_area.x, plugin_start_y, &b.left_tee, border_style);
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
            if let Some(comp) = wp_compositor {
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
            // Wallpaper background overlay
            if let Some(comp) = wp_compositor {
                if let Some(grid) = comp.get_grid(
                    right_area.width as usize,
                    right_area.height as usize,
                    preview_styles.background,
                ) {
                    composite_buffer(right_area, buf, grid, preview_styles.background);
                }
            }

            // Modal dialog box centered within preview area
            let mod_w = (p_avail_w.saturating_sub(4)).clamp(32, 50);
            let mod_h = 12_u16.min(p_avail_h);
            let mod_x = p_inner_x + (p_avail_w.saturating_sub(mod_w)) / 2;
            let mod_y = p_inner_y + (p_avail_h.saturating_sub(mod_h)) / 2;

            // Modal Drop Shadow over wallpaper
            let shadow_char = '░';
            let shadow_style = Style::default().fg(Color::DarkGray).bg(Color::Black);
            let max_bound_x = right_area.x + right_area.width.saturating_sub(1);
            let max_bound_y = right_area.y + right_area.height.saturating_sub(1);
            // Right shadow (2 columns wide)
            for y in (mod_y + 1)..(mod_y + mod_h + 1) {
                for dx in 0..2 {
                    let px = mod_x + mod_w + dx;
                    if px < max_bound_x && y < max_bound_y {
                        buf[(px, y)].set_char(shadow_char).set_style(shadow_style);
                    }
                }
            }
            // Bottom shadow (1 row high)
            let bottom_shadow_y = mod_y + mod_h;
            for px in (mod_x + 2)..(mod_x + mod_w + 2) {
                if px < max_bound_x && bottom_shadow_y < max_bound_y {
                    buf[(px, bottom_shadow_y)]
                        .set_char(shadow_char)
                        .set_style(shadow_style);
                }
            }

            // Clear modal dialog area so wallpaper doesn't bleed through dialog interior
            for cy in mod_y..(mod_y + mod_h) {
                for cx in mod_x..(mod_x + mod_w) {
                    buf[(cx, cy)]
                        .set_char(' ')
                        .set_style(Style::default().bg(preview_styles.background));
                }
            }

            let modal_block = Block::default()
                .borders(Borders::ALL)
                .title("─[ Confirmation Dialog ]")
                .title_style(preview_styles.title)
                .border_style(Style::default().fg(preview_styles.border))
                .style(Style::default().bg(preview_styles.background));
            modal_block.render(Rect::new(mod_x, mod_y, mod_w, mod_h), buf);

            // Modal Close Button [X]
            let mut close_btn_style = preview_styles.window_close_button;
            if close_btn_style.bg.is_none() || close_btn_style.bg == Some(Color::Reset) {
                close_btn_style = close_btn_style.bg(preview_styles.background);
            }
            buf.set_string(mod_x + mod_w - 4, mod_y, "[X]", close_btn_style);

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
            // Visual Menu Editor Split View with interior boxed panels
            let bot_y = right_area.y + right_area.height.saturating_sub(1);
            let footer_y = bot_y.saturating_sub(1);
            let panels_y = p_inner_y;
            let panels_h = footer_y.saturating_sub(panels_y);
            let left_w = p_avail_w / 2;
            let right_w = p_avail_w.saturating_sub(left_w);

            let tree_rect = Rect::new(p_inner_x, panels_y, left_w, panels_h);
            let insp_rect = Rect::new(p_inner_x + left_w, panels_y, right_w, panels_h);

            // Left Panel: Tree with interior borders in accent style
            let tree_block = Block::default()
                .borders(Borders::ALL)
                .title("─[ Menu Structure ]")
                .border_style(preview_styles.accent);
            tree_block.render(tree_rect, buf);

            // Right Panel: Property Inspector with interior borders in accent style
            let insp_block = Block::default()
                .borders(Borders::ALL)
                .title("───[ Property Inspector ]")
                .border_style(preview_styles.accent);
            insp_block.render(insp_rect, buf);

            // Left Pane: Tree Content
            if tree_rect.width > 2 && tree_rect.height > 2 {
                let t_x = tree_rect.x + 1;
                let mut t_y = tree_rect.y + 1;
                let max_y = tree_rect.y + tree_rect.height.saturating_sub(1);

                if t_y < max_y {
                    buf.set_string(t_x, t_y, "▼ [MNU] Main Menu", preview_styles.title);
                    t_y += 1;
                }
                if t_y < max_y {
                    buf.set_string(t_x, t_y, "  ├─▶ [SUB] Applications", preview_styles.text);
                    t_y += 1;
                }
                if t_y < max_y {
                    buf.set_string(t_x, t_y, "  │ └─• [CMD] Terminal", preview_styles.highlight);
                    t_y += 1;
                }
                if t_y < max_y {
                    buf.set_string(t_x, t_y, "  ├─• [CMD] File Manager", preview_styles.text);
                    t_y += 1;
                }
                if t_y < max_y {
                    buf.set_string(t_x, t_y, "  └─• [SUB] Settings", preview_styles.text);
                }
            }

            // Right Pane: Property Inspector Content
            if insp_rect.width > 4 && insp_rect.height > 2 {
                let r_x = insp_rect.x + 2;
                let mut r_y = insp_rect.y + 1;
                let max_y = insp_rect.y + insp_rect.height.saturating_sub(1);

                if r_y < max_y {
                    buf.set_string(r_x, r_y, "Title:  [ Terminal ]", preview_styles.accent);
                    r_y += 1;
                }
                if r_y < max_y {
                    buf.set_string(r_x, r_y, "Icon:   [ 💻 ]", preview_styles.text);
                    r_y += 1;
                }
                if r_y < max_y {
                    buf.set_string(
                        r_x,
                        r_y,
                        "Action: [ alacritty ]",
                        preview_styles.shortcut_key,
                    );
                    r_y += 1;
                }
                if r_y < max_y {
                    buf.set_string(r_x, r_y, "Type:   [ command ]", preview_styles.accent);
                    r_y += 2;
                }
                if r_y < max_y {
                    let btn_save = " [ Save ] ";
                    let btn_del = " [ Delete ] ";
                    buf.set_string(
                        r_x,
                        r_y,
                        btn_save,
                        preview_styles.accent.add_modifier(Modifier::BOLD),
                    );
                    buf.set_string(
                        r_x + btn_save.len() as u16 + 2,
                        r_y,
                        btn_del,
                        preview_styles.window_close_button,
                    );
                }
            }

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
            let p_spread = format_spread_gutter_line(&p_footer_items, p_avail_w as usize);
            buf.set_string(p_inner_x, footer_y, &p_spread, preview_styles.gutter);
        }
        PreviewMode::Toast => {
            // Wallpaper background overlay
            if let Some(comp) = wp_compositor {
                if let Some(grid) = comp.get_grid(
                    right_area.width as usize,
                    right_area.height as usize,
                    preview_styles.background,
                ) {
                    composite_buffer(right_area, buf, grid, preview_styles.background);
                }
            }

            let bg_color = match preview_styles.background {
                Color::Reset => Color::Rgb(52, 63, 73),
                c => c,
            };

            // Title banner
            let title_bg = if preview_styles.background == Color::Reset {
                bg_color
            } else {
                preview_styles.background
            };
            buf.set_string(
                p_inner_x,
                p_inner_y,
                "▶ Toast Notification Stacking & Accent Styles",
                preview_styles.title.bg(title_bg),
            );
            buf.set_string(
                p_inner_x,
                p_inner_y + 1,
                "Click any toast accent bar or text to customize color and glyphs",
                preview_styles.help_text.bg(title_bg),
            );

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
            for dx in 0..toast_w {
                buf[(toast_x + dx, t3_y)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
                buf[(toast_x + dx, t3_y + 1)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
            }
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
            for dx in 0..toast_w {
                buf[(toast_x + dx, t2_y)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
                buf[(toast_x + dx, t2_y + 1)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
            }
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
            for dx in 0..toast_w {
                buf[(toast_x + dx, t1_y)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
                buf[(toast_x + dx, t1_y + 1)]
                    .set_char(' ')
                    .set_style(Style::default().bg(bg_color));
            }
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
            let summary_style = preview_styles.gutter.bg(title_bg);
            buf.set_string(p_inner_x, summary_y, summary_txt, summary_style);
        }
    }
}
