//! Live preview pane rendering for ThemeEdit.

use crate::PreviewMode;
use bashmenu_tui::{
    composite_buffer, draw_box_drop_shadow, format_spread_gutter_line, parse_bbcode_spans,
    rgb_to_xterm256, visible_text_len, ResolvedThemeStyles, WallpaperCompositor,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};
use unicode_width::UnicodeWidthStr;

/// Computes `(toast_x, toast_w, [(y, h); 3])` for the 3 stacked sample toasts.
pub fn toast_preview_boxes(
    right_area: Rect,
    style: bashmenu_core::ToastStyle,
) -> (u16, u16, [(u16, u16); 3]) {
    let p_avail_w = right_area.width.saturating_sub(4);
    let toast_w = (p_avail_w.saturating_sub(6)).clamp(28, 48);
    let toast_x = right_area.x + right_area.width.saturating_sub(toast_w + 3);
    let bot_y = right_area.y + right_area.height.saturating_sub(3);

    match style {
        bashmenu_core::ToastStyle::Bar => {
            let t3_y = bot_y.saturating_sub(3);
            let t2_y = t3_y.saturating_sub(3);
            let t1_y = t2_y.saturating_sub(3);
            (toast_x, toast_w, [(t1_y, 2), (t2_y, 2), (t3_y, 2)])
        }
        bashmenu_core::ToastStyle::Framed => {
            let th = 4_u16;
            let step = 5_u16;
            let t3_y = bot_y.saturating_sub(th);
            let t2_y = t3_y.saturating_sub(step);
            let t1_y = t2_y.saturating_sub(step);
            (toast_x, toast_w, [(t1_y, th), (t2_y, th), (t3_y, th)])
        }
    }
}

/// Screen coordinates `(x_start, x_end)` of a rendered tab in the preview header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewTabSpan {
    /// Preview tab variant represented by this header segment.
    pub mode: PreviewMode,
    /// Inclusive starting terminal column (`x`).
    pub x_start: u16,
    /// Exclusive ending terminal column (`x`).
    pub x_end: u16,
    /// Formatted label rendered for this tab.
    pub label: String,
}

/// Computes all seven preview tab labels and their clickable header X ranges.
pub fn preview_tab_bar_layout(right_area: Rect, active: PreviewMode) -> Vec<PreviewTabSpan> {
    let use_short = right_area.width < 88;
    let mut cur_x = right_area.x.saturating_add(4); // After "┌─[ "
    let mut tabs = Vec::with_capacity(PreviewMode::ALL.len());
    for mode in PreviewMode::ALL {
        let base = if use_short {
            mode.short_label()
        } else {
            mode.tab_label()
        };
        let label = if mode == active {
            format!("▶ {} ◀", base)
        } else {
            base.to_string()
        };
        let width = label.width() as u16;
        tabs.push(PreviewTabSpan {
            mode,
            x_start: cur_x,
            x_end: cur_x.saturating_add(width),
            label,
        });
        cur_x = cur_x.saturating_add(width + 3); // " | " separator
    }
    tabs
}

/// Renders the preview pane for ThemeEdit across `right_area` into `buf`.
pub fn render_preview_pane(
    right_area: Rect,
    buf: &mut Buffer,
    mode: PreviewMode,
    preview_styles: &ResolvedThemeStyles,
    wp_compositor: &mut Option<WallpaperCompositor>,
) {
    render_preview_pane_with_base(
        right_area,
        buf,
        mode,
        preview_styles,
        preview_styles,
        wp_compositor,
    );
}

/// Renders the preview pane using `base_styles` as the outer canvas backdrop.
pub fn render_preview_pane_with_base(
    right_area: Rect,
    buf: &mut Buffer,
    mode: PreviewMode,
    preview_styles: &ResolvedThemeStyles,
    base_styles: &ResolvedThemeStyles,
    wp_compositor: &mut Option<WallpaperCompositor>,
) {
    if right_area.width < 10 || right_area.height < 6 {
        return;
    }

    let tabs = preview_tab_bar_layout(right_area, mode);
    let canvas_styles = if mode == PreviewMode::Toast {
        base_styles
    } else {
        preview_styles
    };

    let mut title_spans = Vec::with_capacity(tabs.len() * 2 + 3);
    title_spans.push(Span::styled("─", Style::default().fg(canvas_styles.border)));
    title_spans.push(Span::styled("[ ", canvas_styles.title));
    for (idx, tab) in tabs.iter().enumerate() {
        if idx > 0 {
            title_spans.push(Span::styled(" | ", canvas_styles.title));
        }
        let tab_style = if tab.mode == mode {
            canvas_styles.selected_tab.add_modifier(Modifier::BOLD)
        } else {
            canvas_styles.unselected_tab
        };
        title_spans.push(Span::styled(tab.label.clone(), tab_style));
    }
    title_spans.push(Span::styled(" ]", canvas_styles.title));

    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_spans))
        .border_style(Style::default().fg(canvas_styles.border))
        .style(Style::default().bg(canvas_styles.background));
    preview_block.render(right_area, buf);

    let p_inner_x = right_area.x + 2;
    let p_inner_y = right_area.y + 2;
    let p_avail_w = right_area.width.saturating_sub(4);
    let p_avail_h = right_area.height.saturating_sub(4);
    let b = &preview_styles.borders;
    let border_style = Style::default().fg(canvas_styles.border);

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

            let help_items = [
                "[ENTER]: Select",
                "[F1]: Help",
                "[F2]: Themes",
                "[F3]: Editor",
                "[F4]: Edit",
                "[F5]: Refresh",
                "[F6]: Keys",
                "[F7]: Gutter",
                "[F8]: ThemeEdit",
                "[F9]: ShortcutEdit",
            ];
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

            // Active item: shortcut text adopts highlight text color matching bashmenu
            let ind = &preview_styles.indicator_symbol;
            let active_sc_style = preview_styles.highlight.add_modifier(Modifier::BOLD);
            let mut active_spans = vec![
                Span::styled(ind, preview_styles.highlight),
                Span::styled("🚀  ", preview_styles.highlight),
                Span::styled("1. ", active_sc_style),
                Span::styled("Launch Application", preview_styles.highlight),
            ];
            let active_vis: usize = active_spans
                .iter()
                .map(|s| visible_text_len(&s.content))
                .sum();
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

            // Ensure unbroken margin-to-margin highlight bar across all cells
            if let Some(bg) = preview_styles.highlight.bg {
                for x in p_inner_x..(p_inner_x + p_avail_w) {
                    buf[(x, content_start_y)].bg = bg;
                }
            }

            // Inactive item 2
            let normal_line = Line::from(vec![
                Span::raw("  "),
                Span::styled("📁  ", preview_styles.text),
                Span::styled("2. ", preview_styles.shortcut_key),
                Span::styled("Browse Directory", preview_styles.text),
                Span::styled("  »", preview_styles.accent),
            ]);
            buf.set_line(p_inner_x, content_start_y + 1, &normal_line, p_avail_w);

            // Inactive item 3: uses theme default icon for items without an explicit icon
            let item3_icon =
                bashmenu_core::format_icon_slot(Some(&preview_styles.default_icon), true);
            let normal_line2 = Line::from(vec![
                Span::raw("  "),
                Span::styled(item3_icon, preview_styles.text),
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
                Span::styled("❌  ", preview_styles.text),
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
                    Span::styled(" ☁️  Weather: ", preview_styles.plugin),
                    Span::styled("+59°F Overcast", preview_styles.plugin),
                    Span::styled("  │  Alienware: ", preview_styles.plugin),
                    Span::styled("Online", preview_styles.plugin),
                ]);
                buf.set_line(p_inner_x, plugin_start_y + 1, &plugin_row1, p_avail_w);

                let plugin_row2 = Line::from(vec![
                    Span::styled(" 🖥️  CPU: ", preview_styles.plugin),
                    Span::styled("12%", preview_styles.plugin),
                    Span::styled("  │  Mem: ", preview_styles.plugin),
                    Span::styled("3.8G/16G", preview_styles.plugin),
                    Span::styled("  │  Pi-3B: ", preview_styles.plugin),
                    Span::styled("Online", preview_styles.plugin),
                ]);
                buf.set_line(p_inner_x, plugin_start_y + 2, &plugin_row2, p_avail_w);
            } else if content_avail_h >= 6 {
                let plugin_y = gt_y.saturating_sub(1);
                let plugin_row = Line::from(vec![
                    Span::styled(" ☁️  Weather: ", preview_styles.plugin),
                    Span::styled("+59°F Overcast", preview_styles.plugin),
                    Span::styled("  │  Alienware: ", preview_styles.plugin),
                    Span::styled("Online", preview_styles.plugin),
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
                Span::styled(
                    " [bashmenu.yml*] ",
                    preview_styles.selected_tab.add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [notes.yml] ", preview_styles.unselected_tab),
                Span::styled(" [+] ", preview_styles.accent),
            ]);
            buf.set_line(p_inner_x, p_inner_y, &tab_line, p_avail_w);

            let sep_str = "─".repeat(p_avail_w as usize);
            buf.set_string(p_inner_x, p_inner_y + 1, &sep_str, preview_styles.divider);

            // 2. Editor Text Content with Line Numbers, Whitespace, and Selection
            let rows_area_end = right_area.y + right_area.height.saturating_sub(3);
            let lines: Vec<Line> = vec![
                Line::from(vec![
                    Span::styled(" 1 │ ", preview_styles.line_numbers),
                    Span::styled("theme: \"dracula\"", preview_styles.text),
                ]),
                Line::from(vec![
                    Span::styled(" 2 │ ", preview_styles.line_numbers),
                    Span::styled("terminal:", preview_styles.text),
                ]),
                Line::from(vec![
                    Span::styled(" 3 │ ", preview_styles.line_numbers),
                    Span::styled("··", preview_styles.whitespace),
                    Span::styled("alt_buffer: true", preview_styles.highlight),
                ]),
                Line::from(vec![
                    Span::styled(" 4 │ ", preview_styles.line_numbers),
                    Span::styled("··", preview_styles.whitespace),
                    Span::styled("show_whitespace: ", preview_styles.text),
                    Span::styled("true", preview_styles.selection),
                ]),
                Line::from(vec![
                    Span::styled(" 5 │ ", preview_styles.line_numbers),
                    Span::styled("menu:", preview_styles.text),
                ]),
                Line::from(vec![
                    Span::styled(" 6 │ ", preview_styles.line_numbers),
                    Span::styled("··", preview_styles.whitespace),
                    Span::styled("preferred_width_pct: 90", preview_styles.text),
                ]),
                Line::from(vec![
                    Span::styled(" ~ │ ", preview_styles.line_numbers),
                    Span::styled("", preview_styles.text),
                ]),
            ];

            for (row, full_line) in lines.into_iter().enumerate() {
                let y = p_inner_y + 2 + row as u16;
                if y >= rows_area_end {
                    break;
                }
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
            let footer_line = Line::from(vec![
                Span::styled("^G ", preview_styles.shortcut_key),
                Span::styled("Help  ", preview_styles.help_text),
                Span::styled("^O ", preview_styles.shortcut_key),
                Span::styled("Save  ", preview_styles.help_text),
                Span::styled("^R ", preview_styles.shortcut_key),
                Span::styled("Open  ", preview_styles.help_text),
                Span::styled("^X ", preview_styles.shortcut_key),
                Span::styled("Exit", preview_styles.help_text),
            ]);
            buf.set_line(p_inner_x, footer_y, &footer_line, p_avail_w);
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

            // Modal Drop Shadow clipped to inner preview area
            let inner_bounds = Rect::new(
                right_area.x + 1,
                right_area.y + 1,
                right_area.width.saturating_sub(2),
                right_area.height.saturating_sub(2),
            );
            draw_box_drop_shadow(
                inner_bounds,
                Rect::new(mod_x, mod_y, mod_w, mod_h),
                preview_styles,
                buf,
            );

            // Clear modal dialog area so wallpaper doesn't bleed through dialog interior
            for cy in mod_y..(mod_y + mod_h) {
                for cx in mod_x..(mod_x + mod_w) {
                    buf[(cx, cy)]
                        .set_char(' ')
                        .set_style(Style::default().bg(preview_styles.background));
                }
            }

            let modal_border_style = Style::default().fg(preview_styles.border);
            let modal_block = Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("─", modal_border_style),
                    Span::styled("[ Confirmation Dialog ]", preview_styles.title),
                ]))
                .border_style(modal_border_style)
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

            // Action Buttons: [ Yes ]  [ No ]  [ Cancel ]
            let yes_btn = "[ Yes ]";
            let no_btn = "[ No ]";
            let cancel_btn = "[ Cancel ]";
            buf.set_string(
                mod_x + 3,
                mod_y + 7,
                yes_btn,
                preview_styles.modal_yes_button.add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                mod_x + 12,
                mod_y + 7,
                no_btn,
                preview_styles.modal_no_button,
            );
            buf.set_string(
                mod_x + 20,
                mod_y + 7,
                cancel_btn,
                preview_styles.modal_cancel_button,
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
                    let tree_text = "  │ └─• [CMD] Terminal";
                    let w = (tree_rect.width.saturating_sub(2)) as usize;
                    let padded = format!("{:<width$}", tree_text, width = w);
                    buf.set_string(t_x, t_y, &padded, preview_styles.highlight);
                    if let Some(bg) = preview_styles.highlight.bg {
                        for x in t_x..(t_x + w as u16) {
                            buf[(x, t_y)].bg = bg;
                        }
                    }
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
            let footer_line = Line::from(vec![
                Span::styled("[F1] ", preview_styles.shortcut_key),
                Span::styled("Help  ", preview_styles.help_text),
                Span::styled("[s] ", preview_styles.shortcut_key),
                Span::styled("Save  ", preview_styles.help_text),
                Span::styled("[ESC] ", preview_styles.shortcut_key),
                Span::styled("Exit", preview_styles.help_text),
            ]);
            buf.set_line(p_inner_x, footer_y, &footer_line, p_avail_w);
        }
        PreviewMode::Toast => {
            // Wallpaper background overlay
            if let Some(comp) = wp_compositor {
                if let Some(grid) = comp.get_grid(
                    right_area.width as usize,
                    right_area.height as usize,
                    canvas_styles.background,
                ) {
                    composite_buffer(right_area, buf, grid, canvas_styles.background);
                }
            }

            let bg_color = match preview_styles.background {
                Color::Reset => Color::Rgb(52, 63, 73),
                c => c,
            };

            // Title banner
            let title_bg = canvas_styles.background;
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
            let (toast_x, toast_w, boxes) =
                toast_preview_boxes(right_area, preview_styles.toast_style);
            let text_style = Style::default()
                .fg(preview_styles.text.fg.unwrap_or(Color::White))
                .bg(bg_color);
            let bg_only = Style::default().bg(bg_color);

            let samples = [
                (
                    boxes[0],
                    preview_styles.toast_info,
                    "Information",
                    "Saved configuration to disk",
                ),
                (
                    boxes[1],
                    preview_styles.toast_warning,
                    "Warning",
                    "High memory consumption (88%)",
                ),
                (
                    boxes[2],
                    preview_styles.toast_error,
                    "Error",
                    "Connection failed on port 8080",
                ),
            ];

            if preview_styles.toast_style == bashmenu_core::ToastStyle::Framed {
                let horiz = "─".repeat(toast_w.saturating_sub(2) as usize);
                let inner_blank = " ".repeat(toast_w.saturating_sub(2) as usize);
                let content_max = toast_w.saturating_sub(4) as usize;

                for ((ty, th), sev_color, title, msg) in samples {
                    let b_style = Style::default().fg(sev_color).bg(bg_color);
                    let t_style = b_style.add_modifier(Modifier::BOLD);
                    let right_x = toast_x + toast_w.saturating_sub(1);

                    // Top border
                    buf.set_string(toast_x, ty, format!("╓{}┐", horiz), b_style);
                    // Interior rows (blank padding + borders)
                    for row_y in (ty + 1)..(ty + th.saturating_sub(1)) {
                        buf.set_string(toast_x, row_y, "║", b_style);
                        buf.set_string(toast_x + 1, row_y, &inner_blank, bg_only);
                        buf.set_string(right_x, row_y, "│", b_style);
                    }
                    // Title & message rows directly inside top/bottom borders
                    let title_y = ty + 1;
                    let msg_y = ty + 2;
                    let clip_title: String = title.chars().take(content_max).collect();
                    let clip_msg: String = msg.chars().take(content_max).collect();
                    buf.set_string(toast_x + 2, title_y, clip_title, t_style);
                    buf.set_string(toast_x + 2, msg_y, clip_msg, text_style);
                    // Bottom border
                    buf.set_string(
                        toast_x,
                        ty + th.saturating_sub(1),
                        format!("╙{}┘", horiz),
                        b_style,
                    );
                }
            } else {
                for ((ty, _th), sev_color, title, msg) in samples {
                    let t_style = Style::default().fg(sev_color).bg(bg_color);
                    for dx in 0..toast_w {
                        buf[(toast_x + dx, ty)].set_char(' ').set_style(bg_only);
                        buf[(toast_x + dx, ty + 1)].set_char(' ').set_style(bg_only);
                    }
                    buf.set_string(toast_x, ty, bar_glyph, t_style);
                    buf.set_string(toast_x + 2, ty, title, t_style.add_modifier(Modifier::BOLD));
                    buf.set_string(toast_x, ty + 1, bar_glyph, t_style);
                    buf.set_string(toast_x + 2, ty + 1, msg, text_style);
                }
            }

            // Bottom configuration summary footer
            let summary_y = right_area.y + right_area.height - 2;
            let summary_txt = format!(
                "Style: '{}'  Bar: '{}'  [Enter: Toggle / Pick]",
                preview_styles.toast_style.display_label(),
                preview_styles.toast_bar_char
            );
            let summary_style = preview_styles.gutter.bg(title_bg);
            buf.set_string(p_inner_x, summary_y, summary_txt, summary_style);
        }
        PreviewMode::ThemeEditor => {
            render_theme_editor_preview(right_area, buf, preview_styles, wp_compositor);
        }
        PreviewMode::ShortcutEditor => {
            render_shortcut_editor_preview(right_area, buf, preview_styles);
        }
    }
}

/// Renders a miniature Theme Editor split-pane mockup showcasing ThemeEdit tokens.
fn render_theme_editor_preview(
    right_area: Rect,
    buf: &mut Buffer,
    preview_styles: &ResolvedThemeStyles,
    wp_compositor: &mut Option<WallpaperCompositor>,
) {
    let p_inner_x = right_area.x + 2;
    let p_inner_y = right_area.y + 2;
    let p_avail_w = right_area.width.saturating_sub(4);
    let b = &preview_styles.borders;
    let border_style = Style::default().fg(preview_styles.border);

    let bot_y = right_area.y + right_area.height.saturating_sub(1);
    let footer_y = bot_y.saturating_sub(1);
    let div_y = footer_y.saturating_sub(1);

    let body_h = div_y.saturating_sub(p_inner_y);
    let left_w = ((p_avail_w as usize * 48) / 100).max(16) as u16;
    let right_w = p_avail_w.saturating_sub(left_w + 1);

    // Top miniature close button
    let close_x = right_area.x + right_area.width.saturating_sub(4);
    let mut close_st = preview_styles.window_close_button;
    if close_st.bg.is_none() || close_st.bg == Some(Color::Reset) {
        close_st = close_st.bg(preview_styles.background);
    }
    buf.set_string(close_x, right_area.y, "[X]", close_st);

    if body_h >= 4 && right_w >= 10 {
        let tree_rect = Rect::new(p_inner_x, p_inner_y, left_w, body_h);
        let sample_rect = Rect::new(p_inner_x + left_w + 1, p_inner_y, right_w, body_h);

        Block::default()
            .borders(Borders::ALL)
            .title(Line::from(vec![
                Span::styled("─", border_style),
                Span::styled("[ Tokens ]", preview_styles.title),
            ]))
            .border_style(border_style)
            .style(Style::default().bg(preview_styles.background))
            .render(tree_rect, buf);

        let sample_border_style =
            Style::default().fg(preview_styles.accent.fg.unwrap_or(preview_styles.border));
        Block::default()
            .borders(Borders::ALL)
            .title(Line::from(vec![
                Span::styled("─", sample_border_style),
                Span::styled("[ Main Menu ]", preview_styles.title),
            ]))
            .border_style(sample_border_style)
            .style(Style::default().bg(preview_styles.background))
            .render(sample_rect, buf);

        // Tree rows matching real ThemeEdit collapsible category dropdowns
        let cat_style = preview_styles.accent.add_modifier(Modifier::BOLD);
        let rows: [(&str, Style); 7] = [
            ("▾ Theme:", cat_style),
            ("▾ Colors:", cat_style),
            ("  Background    Color 234", preview_styles.text),
            ("▶ Text Color    Color 255", preview_styles.highlight),
            ("  Help Text     Inherit", preview_styles.text),
            ("  Divider       Color 81", preview_styles.text),
            ("▸ Borders:", cat_style),
        ];
        for (idx, (txt, st)) in rows.iter().enumerate() {
            let y = tree_rect.y + 1 + idx as u16;
            if y >= tree_rect.bottom().saturating_sub(1) {
                break;
            }
            let inner_w = tree_rect.width.saturating_sub(2) as usize;
            let clipped = format!(
                "{:<w$}",
                txt.chars().take(inner_w).collect::<String>(),
                w = inner_w
            );
            buf.set_string(tree_rect.x + 1, y, &clipped, *st);
        }

        // Scrollbar thumb inside tree right border
        if tree_rect.height > 4 {
            buf[(tree_rect.right() - 1, tree_rect.y + 2)]
                .set_char('█')
                .set_style(preview_styles.scrollbar);
        }

        // Right sample preview content: miniature Main Menu
        let sx = sample_rect.x + 1;
        let sw = sample_rect.width.saturating_sub(2) as usize;
        let sy = sample_rect.y + 1;
        let smax = sample_rect.bottom().saturating_sub(1);
        let has_bottom_gutter = smax >= sy + 3 && sw >= 6;
        let s_gutter_y = smax.saturating_sub(1);
        let s_div_y = s_gutter_y.saturating_sub(1);
        let menu_max_y = if has_bottom_gutter { s_div_y } else { smax };

        if sy < menu_max_y && sw > 0 {
            let active_row = format!(
                "{:<w$}",
                "► 1. Terminal".chars().take(sw).collect::<String>(),
                w = sw
            );
            buf.set_string(sx, sy, &active_row, preview_styles.highlight);
        }
        if sy + 1 < menu_max_y && sw > 0 {
            let line = Line::from(vec![
                Span::styled("  ", preview_styles.text),
                Span::styled("2.", preview_styles.shortcut_key),
                Span::styled(" Files ", preview_styles.text),
                Span::styled("»", preview_styles.accent),
            ]);
            buf.set_line(sx, sy + 1, &line, sw as u16);
        }
        if sy + 2 < menu_max_y && sw > 0 {
            buf.set_string(sx, sy + 2, "─".repeat(sw), preview_styles.divider);
        }
        if sy + 3 < menu_max_y && sw > 0 {
            let line = Line::from(vec![
                Span::styled("  ", preview_styles.text),
                Span::styled("3.", preview_styles.shortcut_key),
                Span::styled(" Settings", preview_styles.text),
            ]);
            buf.set_line(sx, sy + 3, &line, sw as u16);
        }
        if has_bottom_gutter {
            let sep_offset = sw / 2;
            let sep_col = sx + sep_offset as u16;
            buf.set_string(
                sample_rect.x,
                s_div_y,
                &b.gutter_left_tee,
                sample_border_style,
            );
            buf.set_string(
                sample_rect.right() - 1,
                s_div_y,
                &b.gutter_right_tee,
                sample_border_style,
            );
            buf.set_string(sx, s_div_y, "─".repeat(sw), sample_border_style);
            buf.set_string(sep_col, s_div_y, &b.gutter_top_tee, sample_border_style);
            buf.set_string(
                sep_col,
                s_gutter_y,
                &b.gutter_separator,
                sample_border_style,
            );
            buf.set_string(
                sep_col,
                sample_rect.bottom() - 1,
                &b.gutter_bottom_tee,
                sample_border_style,
            );

            // Bottom-left help gutter
            let left_clip: String = "F1 Help".chars().take(sep_offset).collect();
            buf.set_string(sx, s_gutter_y, &left_clip, preview_styles.help_text);

            // Bottom-right status gutter (right-aligned)
            let right_w_avail = sw.saturating_sub(sep_offset + 1);
            if right_w_avail > 0 {
                let status_str = "12:00";
                let right_clip: String = status_str.chars().take(right_w_avail).collect();
                let right_x = sep_col + 1 + right_w_avail.saturating_sub(right_clip.len()) as u16;
                buf.set_string(right_x, s_gutter_y, &right_clip, preview_styles.gutter);
            }
        }

        // Composite background wallpaper scaled to fit inside the miniature Main Menu pane
        if let Some(comp) = wp_compositor {
            let inner_sample = Rect::new(
                sample_rect.x + 1,
                sample_rect.y + 1,
                sample_rect.width.saturating_sub(2),
                sample_rect.height.saturating_sub(2),
            );
            if inner_sample.width > 0 && inner_sample.height > 0 {
                if let Some(grid) = comp.get_grid(
                    inner_sample.width as usize,
                    inner_sample.height as usize,
                    preview_styles.background,
                ) {
                    composite_buffer(inner_sample, buf, grid, preview_styles.background);
                }
            }
        }
    }

    // Divider & Footer bar
    buf.set_string(right_area.x, div_y, &b.border_vertical_left, border_style);
    buf.set_string(
        right_area.x + right_area.width - 1,
        div_y,
        &b.border_vertical_right,
        border_style,
    );
    buf.set_string(
        p_inner_x,
        div_y,
        "─".repeat(p_avail_w as usize),
        preview_styles.divider,
    );

    let footer_line = Line::from(vec![
        Span::styled("[1-7] ", preview_styles.shortcut_key),
        Span::styled("View  ", preview_styles.help_text),
        Span::styled("[Del] ", preview_styles.shortcut_key),
        Span::styled("Reset -1  ", preview_styles.help_text),
        Span::styled("[Ctrl+S] ", preview_styles.shortcut_key),
        Span::styled("Save", preview_styles.help_text),
    ]);
    buf.set_line(p_inner_x, footer_y, &footer_line, p_avail_w);
}

/// Renders a miniature Shortcuts Editor mockup showcasing `shortcutedit` tokens.
fn render_shortcut_editor_preview(
    right_area: Rect,
    buf: &mut Buffer,
    preview_styles: &ResolvedThemeStyles,
) {
    let p_inner_x = right_area.x + 2;
    let p_inner_y = right_area.y + 2;
    let p_avail_w = right_area.width.saturating_sub(4);
    let b = &preview_styles.borders;
    let border_style = Style::default().fg(preview_styles.border);

    let bot_y = right_area.y + right_area.height.saturating_sub(1);
    let footer_y = bot_y.saturating_sub(1);
    let div_y = footer_y.saturating_sub(1);

    let list_h = div_y.saturating_sub(p_inner_y).max(3);
    let left_w = p_avail_w / 2;
    let right_w = p_avail_w.saturating_sub(left_w);
    let list_rect = Rect::new(p_inner_x, p_inner_y, left_w, list_h);
    let insp_rect = Rect::new(p_inner_x + left_w, p_inner_y, right_w, list_h);

    Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::styled("─", border_style),
            Span::styled("[ Keyboard Shortcuts ]", preview_styles.title),
        ]))
        .border_style(border_style)
        .style(Style::default().bg(preview_styles.background))
        .render(list_rect, buf);

    let items: [(&str, &str, bool); 4] = [
        ("[1]     ", "Launch Terminal (alacritty)", true),
        ("[2]     ", "Open File Manager (yazi)", false),
        ("[Ctrl+E]", "Edit Configuration File", false),
        ("[F2]    ", "Quick System Diagnostics", false),
    ];

    for (idx, (key, desc, is_sel)) in items.iter().enumerate() {
        let y = list_rect.y + 1 + idx as u16;
        if y >= list_rect.bottom().saturating_sub(1) {
            break;
        }
        let inner_w = list_rect.width.saturating_sub(2) as usize;
        if *is_sel {
            let text = format!(
                "{:<w$}",
                format!("{} {}", key, desc)
                    .chars()
                    .take(inner_w)
                    .collect::<String>(),
                w = inner_w
            );
            buf.set_string(list_rect.x + 1, y, &text, preview_styles.highlight);
        } else {
            let line = Line::from(vec![
                Span::styled(*key, preview_styles.key_color),
                Span::styled(" ", preview_styles.text),
                Span::styled(*desc, preview_styles.text),
            ]);
            buf.set_line(list_rect.x + 1, y, &line, inner_w as u16);
        }
    }

    if list_rect.height > 4 {
        buf[(list_rect.right() - 1, list_rect.y + 1)]
            .set_char('█')
            .set_style(preview_styles.scrollbar);
    }

    // Right Pane: Shortcut Inspector matching real `shortcutedit` two-pane layout
    if insp_rect.width >= 6 {
        Block::default()
            .borders(Borders::ALL)
            .title(Line::from(vec![
                Span::styled("─", border_style),
                Span::styled("[ Shortcut Inspector ]", preview_styles.title),
            ]))
            .border_style(border_style)
            .style(Style::default().bg(preview_styles.background))
            .render(insp_rect, buf);

        if insp_rect.width > 4 && insp_rect.height > 2 {
            let r_x = insp_rect.x + 2;
            let r_w = insp_rect.width.saturating_sub(4);
            let max_y = insp_rect.bottom().saturating_sub(1);
            let insp_rows: [(&str, Style); 5] = [
                (
                    "Shortcut Properties",
                    preview_styles.accent.add_modifier(Modifier::BOLD),
                ),
                ("Description: Launch Terminal", preview_styles.text),
                ("Key Chord:   [1]", preview_styles.accent),
                ("Action:      alacritty", preview_styles.text),
                ("Mode:        Console", preview_styles.help_text),
            ];
            for (idx, (txt, st)) in insp_rows.iter().enumerate() {
                let r_y = insp_rect.y + 1 + idx as u16;
                if r_y >= max_y {
                    break;
                }
                buf.set_line(r_x, r_y, &Line::from(Span::styled(*txt, *st)), r_w);
            }
        }
    }

    // Divider & Footer
    buf.set_string(right_area.x, div_y, &b.border_vertical_left, border_style);
    buf.set_string(
        right_area.x + right_area.width - 1,
        div_y,
        &b.border_vertical_right,
        border_style,
    );
    buf.set_string(
        p_inner_x,
        div_y,
        "─".repeat(p_avail_w as usize),
        preview_styles.divider,
    );

    let footer_line = Line::from(vec![
        Span::styled("[Ins] ", preview_styles.shortcut_key),
        Span::styled("Add  ", preview_styles.help_text),
        Span::styled("[Enter] ", preview_styles.shortcut_key),
        Span::styled("Edit  ", preview_styles.help_text),
        Span::styled("[Del] ", preview_styles.shortcut_key),
        Span::styled("Remove  ", preview_styles.help_text),
        Span::styled("[ESC] ", preview_styles.shortcut_key),
        Span::styled("Back", preview_styles.help_text),
    ]);
    buf.set_line(p_inner_x, footer_y, &footer_line, p_avail_w);
}

/// Formats a Ratatui Color into a string color code (hex, index, or name) with 256 decimal code.
pub fn format_color_code(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => {
            let idx = rgb_to_xterm256(r, g, b);
            format!("#{r:02x}{g:02x}{b:02x} (256: {idx})")
        }
        Color::Indexed(code) => format!("{code} (256: {code})"),
        Color::Reset => "default".to_string(),
        Color::Black => "black (256: 0)".to_string(),
        Color::Red => "red (256: 1)".to_string(),
        Color::Green => "green (256: 2)".to_string(),
        Color::Yellow => "yellow (256: 3)".to_string(),
        Color::Blue => "blue (256: 4)".to_string(),
        Color::Magenta => "magenta (256: 5)".to_string(),
        Color::Cyan => "cyan (256: 6)".to_string(),
        Color::Gray => "gray (256: 7)".to_string(),
        Color::DarkGray => "darkgray (256: 8)".to_string(),
        Color::LightRed => "lightred (256: 9)".to_string(),
        Color::LightGreen => "lightgreen (256: 10)".to_string(),
        Color::LightYellow => "lightyellow (256: 11)".to_string(),
        Color::LightBlue => "lightblue (256: 12)".to_string(),
        Color::LightMagenta => "lightmagenta (256: 13)".to_string(),
        Color::LightCyan => "lightcyan (256: 14)".to_string(),
        Color::White => "white (256: 15)".to_string(),
    }
}

/// Extracts the effective Ratatui Color of the rendered cell at (col, row).
pub fn extract_preview_cell_color(
    buf: &Buffer,
    col: u16,
    row: u16,
    fallback_styles: &ResolvedThemeStyles,
) -> Color {
    if let Some(cell) = buf.cell((col, row)) {
        let sym = cell.symbol();
        if sym == " " || sym.is_empty() {
            // Padding space: visible pixel color is the background
            if cell.bg != Color::Reset {
                cell.bg
            } else {
                fallback_styles.background
            }
        } else if sym == "▀" {
            // Wallpaper half-block: upper half is fg, lower half is bg
            if cell.fg != Color::Reset {
                cell.fg
            } else if cell.bg != Color::Reset {
                cell.bg
            } else {
                fallback_styles.background
            }
        } else {
            // Character glyph: visible pixel color is the foreground
            if cell.fg != Color::Reset {
                cell.fg
            } else {
                fallback_styles.text.fg.unwrap_or(Color::White)
            }
        }
    } else {
        fallback_styles.background
    }
}

/// Extracts the color code string of the rendered cell at (col, row).
pub fn extract_preview_color_at(
    buf: &Buffer,
    col: u16,
    row: u16,
    fallback_styles: &ResolvedThemeStyles,
) -> String {
    let color = extract_preview_cell_color(buf, col, row, fallback_styles);
    format_color_code(color)
}
