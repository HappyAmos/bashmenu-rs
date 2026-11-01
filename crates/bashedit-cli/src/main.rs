//! Full-screen terminal text editor with tab support, colocated swap files, and crash recovery.

use anyhow::Result;
use bashedit_cli::{
    get_swap_path, is_pid_running, read_swap_file, remove_swap_file, EditorBuffer, SwapHeader,
};
use bashmenu_core::{
    load_config, load_themes, ToastNotification, ToastSeverity, BASHEDIT_HELP_TEXT,
    MARKDOWN_REFERENCE_TEXT, PLACEHOLDER_HELP_TEXT,
};
use bashmenu_tui::{
    install_panic_hook, AsciiGridModal, ColorGridModal, ConfirmModal, FilePickerModal, HelpModal,
    InputModal, ResolvedThemeStyles, TerminalGuard, ToastRack,
};
use clap::Parser;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "bashedit",
    author,
    version,
    about = "Terminal text editor with crash recovery, tabs, and colocated swap files"
)]
struct Cli {
    /// File to edit (leave blank for empty unsaved buffer)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    /// Open in read-only mode
    #[arg(short, long)]
    read_only: bool,

    /// Tab stop width (spaces per tab)
    #[arg(long, default_value = "4")]
    tabstop: usize,

    /// Initial line number to jump to
    #[arg(short, long, default_value = "1")]
    line: usize,

    /// Theme override name
    #[arg(short, long)]
    theme: Option<String>,

    /// Load and save configuration files in current working directory
    #[arg(long)]
    local: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterAction {
    HelpManual,
    ToggleWhitespace,
    AsciiTable,
    ViewColors,
    PlaceholdersHelp,
    MarkdownPreview,
    ToggleLineno,
    MarkdownReference,
    NewTab,
    OpenFile,
    SaveFile,
    SaveFileAs,
    Search,
    Undo,
    NextTab,
    Exit,
}

struct FooterHitRegion {
    row: u16,
    start_x: u16,
    end_x: u16,
    action: FooterAction,
}

enum ActiveModal {
    None,
    SwapRecovery {
        header: SwapHeader,
        recovered_lines: Vec<String>,
        tab_idx: usize,
    },
    ConfirmCloseTab {
        tab_idx: usize,
        modal: ConfirmModal,
    },
    ConfirmExit(ConfirmModal),
    SaveAs(FilePickerModal),
    Search(InputModal),
    OpenFile(FilePickerModal),
    HelpManual(HelpModal),
    PlaceholdersHelp(HelpModal),
    MarkdownReference(HelpModal),
    MarkdownPreview(HelpModal),
    AsciiTable(AsciiGridModal),
    ViewColors(ColorGridModal),
}

fn resolve_path_tilde(path: &Path) -> PathBuf {
    if let Some(stripped) = path.to_str().and_then(|s| s.strip_prefix("~/")) {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(stripped)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    }
}

fn make_save_as_modal(tab: &EditorBuffer) -> FilePickerModal {
    let home_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let start_dir = tab
        .file_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(home_dir);
    let mut picker = FilePickerModal::new("Save File As", &start_dir, false).with_allow_new(true);
    if let Some(ref path) = tab.file_path {
        picker = picker.with_initial_selection(path);
    }
    picker
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let mut guard = TerminalGuard::init()?;

    // Load active theme configuration (single source of truth: ~/.config/bashmenu/)
    let config_path = if cli.local {
        PathBuf::from("bashmenu.yml")
    } else {
        bashmenu_core::resolve_app_path(Path::new("bashmenu.yml"))
    };
    let themes_path = if cli.local {
        PathBuf::from("bashmenu.themes")
    } else {
        bashmenu_core::resolve_app_path(Path::new("bashmenu.themes"))
    };
    let config = load_config(&config_path).unwrap_or_default();
    let themes = load_themes(&themes_path).unwrap_or_default();
    let use_nerd = config.settings.use_nerd_fonts.unwrap_or(false);

    let active_theme_name = cli.theme.as_deref().unwrap_or(&config.theme);

    let styles = if let Some(t) = themes.get(active_theme_name) {
        ResolvedThemeStyles::from_theme(t, use_nerd)
    } else {
        ResolvedThemeStyles::default()
    };

    // Initialize document tabs
    let mut tabs: Vec<EditorBuffer> = Vec::new();
    let mut pending_recovery = None;

    if let Some(ref path) = cli.file {
        let actual_path = resolve_path_tilde(path);
        let swp_path = get_swap_path(&actual_path);
        let recovery = if swp_path.exists() {
            read_swap_file(&swp_path)
        } else {
            None
        };
        let mut buf = EditorBuffer::open(&actual_path)?;
        if cli.read_only {
            buf.is_read_only = true;
        }
        if cli.line > 1 && cli.line <= buf.lines.len() {
            buf.cursor_row = cli.line - 1;
        }
        tabs.push(buf);
        if let Some((header, lines)) = recovery {
            pending_recovery = Some((header, lines, 0));
        }
    } else {
        tabs.push(EditorBuffer::new_empty());
    }

    let mut active_tab_idx = 0;
    let mut show_line_numbers = true;
    let mut show_whitespace = false;
    let mut shared_cutbuffer: Vec<String> = Vec::new();

    let mut active_modal = if let Some((header, lines, tab_idx)) = pending_recovery {
        ActiveModal::SwapRecovery {
            header,
            recovered_lines: lines,
            tab_idx,
        }
    } else {
        ActiveModal::None
    };

    let mut last_grid_origin = (0, 0);
    let mut toast_rack = ToastRack::new();

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        // UI geometry definitions
        let header_y = area.y;
        let tab_bar_y = area.y + 1;
        let divider_y = (area.y + area.height).saturating_sub(4);
        let legend_row1_y = (area.y + area.height).saturating_sub(3);
        let legend_row2_y = (area.y + area.height).saturating_sub(2);
        let status_y = (area.y + area.height).saturating_sub(1);

        let editor_canvas_y = area.y + 2;
        let editor_canvas_h = if area.height >= 7 {
            (area.height - 6) as usize
        } else {
            0
        };

        // Ensure active tab bounds
        if active_tab_idx >= tabs.len() {
            active_tab_idx = tabs.len().saturating_sub(1);
        }

        let line_num_w: usize = if show_line_numbers { 7 } else { 0 };
        let text_w = (area.width as usize).saturating_sub(line_num_w + 2);

        // Adjust scrolling for active tab
        {
            let active_tab = &mut tabs[active_tab_idx];
            if active_tab.cursor_row < active_tab.scroll_row {
                active_tab.scroll_row = active_tab.cursor_row;
            } else if active_tab.cursor_row >= active_tab.scroll_row + editor_canvas_h
                && editor_canvas_h > 0
            {
                active_tab.scroll_row = active_tab.cursor_row.saturating_sub(editor_canvas_h - 1);
            }

            if active_tab.cursor_col < active_tab.scroll_col {
                active_tab.scroll_col = active_tab.cursor_col;
            } else if active_tab.cursor_col >= active_tab.scroll_col + text_w && text_w > 0 {
                active_tab.scroll_col = active_tab.cursor_col.saturating_sub(text_w - 1);
            }
        }

        let mut tab_click_regions: Vec<(usize, u16, u16)> = Vec::new();
        let mut tab_close_regions: Vec<(usize, u16, u16)> = Vec::new();
        let mut add_tab_region: Option<(u16, u16)> = None;
        let mut footer_hit_regions: Vec<FooterHitRegion> = Vec::new();

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            // Clear screen area with background style
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    buf[(x, y)].reset();
                    buf[(x, y)].set_bg(styles.background);
                }
            }

            if area.width < 10 || area.height < 6 {
                return;
            }

            // 1. Header Bar (Row 0): Centered title + right close [X]
            let active_name = tabs[active_tab_idx]
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string());
            let header_title = format!("  BashEdit - {}  ", active_name);

            // Fill header bar background
            let header_bg_style = Style::default()
                .bg(styles.border)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);

            for x in area.x..area.x + area.width {
                buf[(x, header_y)].set_style(header_bg_style);
            }

            let title_x = area.x + (area.width.saturating_sub(header_title.len() as u16)) / 2;
            buf.set_string(title_x, header_y, &header_title, header_bg_style);

            let close_btn_label = " [X] ";
            let close_btn_x = area.x + area.width.saturating_sub(close_btn_label.len() as u16);
            buf.set_string(
                close_btn_x,
                header_y,
                close_btn_label,
                header_bg_style.add_modifier(Modifier::BOLD),
            );

            // 2. Tab Bar (Row 1): Multiple document tabs with active highlight, [x] close, and [+]
            let tab_bar_style = Style::default().bg(styles.background).fg(Color::DarkGray);

            for x in area.x..area.x + area.width {
                buf[(x, tab_bar_y)].set_style(tab_bar_style);
            }

            let mut tab_x = area.x + 1;
            for (idx, tab) in tabs.iter().enumerate() {
                let name = tab
                    .file_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                let mod_str = if tab.is_modified { " *" } else { "" };
                let tab_label = format!(" {}: {}{} ", idx + 1, name, mod_str);
                let label_len = tab_label.len() as u16;

                let is_active = idx == active_tab_idx;
                let tab_style = if is_active {
                    styles.selection.add_modifier(Modifier::BOLD)
                } else {
                    styles.text.add_modifier(Modifier::DIM)
                };

                buf.set_string(tab_x, tab_bar_y, &tab_label, tab_style);
                let start_x = tab_x;
                tab_x += label_len;
                tab_click_regions.push((idx, start_x, tab_x));

                if tabs.len() > 1 {
                    let close_x = tab_x;
                    buf.set_string(close_x, tab_bar_y, " ✕ ", Style::default().fg(Color::Red));
                    tab_x += 3;
                    tab_close_regions.push((idx, close_x, tab_x));
                }
            }

            // Tab add button
            let add_x = tab_x;
            buf.set_string(add_x, tab_bar_y, " [ + ] ", styles.accent);
            tab_x += 7;
            add_tab_region = Some((add_x, tab_x));

            // 3. Editor Canvas (Row 2 .. H - 4)
            let inner_x = area.x + 1;
            let active_tab = &tabs[active_tab_idx];
            let selection_range = active_tab.get_selection_range();

            for view_row in 0..editor_canvas_h {
                let doc_row = active_tab.scroll_row + view_row;
                let screen_y = editor_canvas_y + view_row as u16;

                if doc_row >= active_tab.lines.len() {
                    buf.set_string(inner_x, screen_y, "~", Style::default().fg(Color::DarkGray));
                    continue;
                }

                if show_line_numbers {
                    let l_num_str = format!("{:>4} │ ", doc_row + 1);
                    buf.set_string(
                        inner_x,
                        screen_y,
                        &l_num_str,
                        Style::default().fg(Color::DarkGray),
                    );
                }

                let line_str = &active_tab.lines[doc_row];
                let chars: Vec<char> = line_str.chars().collect();
                let text_start_x = inner_x + line_num_w as u16;

                for view_col in 0..text_w {
                    let doc_col = active_tab.scroll_col + view_col;
                    let screen_x = text_start_x + view_col as u16;

                    let raw_ch = chars.get(doc_col).copied().unwrap_or(' ');
                    let is_cursor =
                        doc_row == active_tab.cursor_row && doc_col == active_tab.cursor_col;

                    let is_selected = if let Some(((sy, sx), (ey, ex))) = selection_range {
                        if sy == ey {
                            doc_row == sy && doc_col >= sx && doc_col < ex
                        } else if doc_row == sy {
                            doc_col >= sx
                        } else if doc_row == ey {
                            doc_col < ex
                        } else {
                            doc_row > sy && doc_row < ey
                        }
                    } else {
                        false
                    };

                    let display_ch = if show_whitespace {
                        if raw_ch == ' ' && doc_col < chars.len() {
                            '·'
                        } else if raw_ch == '\t' {
                            '→'
                        } else {
                            raw_ch
                        }
                    } else {
                        raw_ch
                    };

                    let style = if is_cursor {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else if is_selected {
                        styles.highlight.add_modifier(Modifier::REVERSED)
                    } else {
                        styles.text
                    };

                    buf.set_string(screen_x, screen_y, display_ch.to_string(), style);
                }

                // Cursor rendered at trailing end of line
                if doc_row == active_tab.cursor_row && active_tab.cursor_col >= chars.len() {
                    let cursor_view_col =
                        active_tab.cursor_col.saturating_sub(active_tab.scroll_col);
                    if cursor_view_col < text_w {
                        let screen_x = text_start_x + cursor_view_col as u16;
                        buf.set_string(
                            screen_x,
                            screen_y,
                            " ",
                            Style::default().fg(Color::Black).bg(Color::White),
                        );
                    }
                }
            }

            // 4. Divider Line (Row H - 4)
            buf.set_string(
                area.x,
                divider_y,
                "─".repeat(area.width as usize),
                styles.divider,
            );

            // 5. Help Gutter / Legend (Row H - 3 & Row H - 2)
            let items_r1 = [
                ("F1 Help", FooterAction::HelpManual),
                ("F2 Whitespace", FooterAction::ToggleWhitespace),
                ("F3 ASCII", FooterAction::AsciiTable),
                ("F4 Colors", FooterAction::ViewColors),
                ("F5 Macros", FooterAction::PlaceholdersHelp),
                ("F6 MD", FooterAction::MarkdownPreview),
                ("F7 Lineno", FooterAction::ToggleLineno),
                ("F8 MD Ref", FooterAction::MarkdownReference),
            ];

            let legend_avail_w = (area.width as usize).saturating_sub(2);
            let labels_r1: Vec<&str> = items_r1.iter().map(|(lbl, _)| *lbl).collect();
            let (line_r1, ranges_r1) =
                bashmenu_tui::spread_gutter_line_layout(&labels_r1, legend_avail_w);

            buf.set_string(
                inner_x,
                legend_row1_y,
                &line_r1,
                Style::default().fg(Color::DarkGray),
            );

            for (idx, (start, end)) in ranges_r1.into_iter().enumerate() {
                if idx < items_r1.len() {
                    let (label, action) = items_r1[idx];
                    let screen_start = inner_x + start as u16;
                    let screen_end = inner_x + end as u16;
                    buf.set_string(screen_start, legend_row1_y, label, styles.accent);
                    footer_hit_regions.push(FooterHitRegion {
                        row: legend_row1_y,
                        start_x: screen_start,
                        end_x: screen_end,
                        action,
                    });
                }
            }

            let items_r2 = [
                ("^N Tab", FooterAction::NewTab),
                ("^O Open", FooterAction::OpenFile),
                ("^S Save", FooterAction::SaveFile),
                ("^A SaveAs", FooterAction::SaveFileAs),
                ("^W Search", FooterAction::Search),
                ("^Z Undo", FooterAction::Undo),
                ("Alt+←/→ Tab", FooterAction::NextTab),
                ("^Q/ESC Exit", FooterAction::Exit),
            ];

            let labels_r2: Vec<&str> = items_r2.iter().map(|(lbl, _)| *lbl).collect();
            let (line_r2, ranges_r2) =
                bashmenu_tui::spread_gutter_line_layout(&labels_r2, legend_avail_w);

            buf.set_string(
                inner_x,
                legend_row2_y,
                &line_r2,
                Style::default().fg(Color::DarkGray),
            );

            for (idx, (start, end)) in ranges_r2.into_iter().enumerate() {
                if idx < items_r2.len() {
                    let (label, action) = items_r2[idx];
                    let screen_start = inner_x + start as u16;
                    let screen_end = inner_x + end as u16;
                    buf.set_string(screen_start, legend_row2_y, label, styles.accent);
                    footer_hit_regions.push(FooterHitRegion {
                        row: legend_row2_y,
                        start_x: screen_start,
                        end_x: screen_end,
                        action,
                    });
                }
            }

            // 6. Status Gutter (Row H - 1)
            let status_flags = if active_tab.is_read_only {
                "[Read-Only]"
            } else if active_tab.is_modified {
                "[Modified]"
            } else {
                "[Clean]"
            };

            let pos_str = format!(
                "  Line {}/{}, Col {}  ",
                active_tab.cursor_row + 1,
                active_tab.lines.len(),
                active_tab.cursor_col + 1
            );
            buf.set_string(
                inner_x,
                status_y,
                &pos_str,
                Style::default().fg(Color::White),
            );

            let status_info = if !active_tab.status_message.is_empty() {
                format!("{}  {}", active_tab.status_message, status_flags)
            } else {
                status_flags.to_string()
            };

            let status_info_x = (area.x + area.width).saturating_sub(status_info.len() as u16 + 2);
            buf.set_string(
                status_info_x,
                status_y,
                &status_info,
                Style::default().fg(if active_tab.is_modified {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            );

            // 7. Active Toasts
            toast_rack.render(area, &styles, buf);

            // 8. Modals
            if let Some(orig) = render_modal(&mut active_modal, area, &styles, &tabs, buf) {
                last_grid_origin = orig;
            }
        })?;

        toast_rack.tick();

        let poll_dur = if !toast_rack.is_empty() {
            Duration::from_millis(30)
        } else {
            Duration::from_millis(50)
        };

        // 9. Event Handling
        if event::poll(poll_dur)? {
            let ev = event::read()?;

            // Ignore mouse movement and key release events to eliminate latency and tearing
            if let Event::Mouse(m) = &ev {
                if m.kind == MouseEventKind::Moved {
                    continue;
                }
                if toast_rack.handle_mouse(m.column, m.row) {
                    continue;
                }
            }
            if let Event::Key(k) = &ev {
                if k.kind == KeyEventKind::Release {
                    continue;
                }
            }

            // Handle active modal
            let handled_modal = handle_active_modal(
                &mut active_modal,
                &mut tabs,
                &mut active_tab_idx,
                &ev,
                area,
                cli.read_only,
                last_grid_origin,
                &mut toast_rack,
            );

            if handled_modal {
                if let ActiveModal::None = active_modal {
                    // Check if exit was requested from confirm modal
                }
                continue;
            }

            // If exit was chosen from exit modal:
            if let Event::Key(k) = &ev {
                if matches!(active_modal, ActiveModal::None) {
                    // Handle main editor key events
                    match k.code {
                        KeyCode::F(1) => {
                            active_modal = ActiveModal::HelpManual(HelpModal::new(
                                "BashEdit Manual",
                                BASHEDIT_HELP_TEXT,
                            ));
                            continue;
                        }
                        KeyCode::F(2) => {
                            show_whitespace = !show_whitespace;
                            tabs[active_tab_idx].status_message = format!(
                                "Whitespace display {}",
                                if show_whitespace {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            );
                            continue;
                        }
                        KeyCode::F(3) => {
                            active_modal = ActiveModal::AsciiTable(AsciiGridModal::new(
                                "ASCII Character Table",
                                65,
                            ));
                            continue;
                        }
                        KeyCode::F(4) => {
                            active_modal =
                                ActiveModal::ViewColors(ColorGridModal::new("Terminal Colors", 7));
                            continue;
                        }
                        KeyCode::F(5) => {
                            active_modal = ActiveModal::PlaceholdersHelp(HelpModal::new(
                                "Available Placeholders & Macros",
                                PLACEHOLDER_HELP_TEXT,
                            ));
                            continue;
                        }
                        KeyCode::F(6) => {
                            let md_text = tabs[active_tab_idx].lines.join("\n");
                            active_modal = ActiveModal::MarkdownPreview(HelpModal::new(
                                "Markdown Preview",
                                &md_text,
                            ));
                            continue;
                        }
                        KeyCode::F(7) => {
                            show_line_numbers = !show_line_numbers;
                            tabs[active_tab_idx].status_message = format!(
                                "Line numbers {}",
                                if show_line_numbers {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            );
                            continue;
                        }
                        KeyCode::F(8) => {
                            active_modal = ActiveModal::MarkdownReference(HelpModal::new(
                                "Markdown Syntax Reference",
                                MARKDOWN_REFERENCE_TEXT,
                            ));
                            continue;
                        }
                        _ => {}
                    }

                    // Ctrl key combinations
                    if k.modifiers.contains(KeyModifiers::CONTROL) {
                        match k.code {
                            KeyCode::Char('s') => {
                                let tab = &mut tabs[active_tab_idx];
                                if tab.file_path.is_none() {
                                    active_modal = ActiveModal::SaveAs(make_save_as_modal(tab));
                                } else {
                                    let res = tab.save_to_disk();
                                    show_save_toast(&mut toast_rack, res, tab);
                                }
                                continue;
                            }
                            KeyCode::Char('a') => {
                                active_modal =
                                    ActiveModal::SaveAs(make_save_as_modal(&tabs[active_tab_idx]));
                                continue;
                            }
                            KeyCode::Char('n') | KeyCode::Char('e') => {
                                tabs.push(EditorBuffer::new_empty());
                                active_tab_idx = tabs.len() - 1;
                                continue;
                            }
                            KeyCode::Char('o') | KeyCode::Char('r') => {
                                let current_dir = tabs[active_tab_idx]
                                    .file_path
                                    .as_ref()
                                    .and_then(|p| p.parent())
                                    .map(|p| p.to_path_buf())
                                    .unwrap_or_else(|| {
                                        std::env::current_dir()
                                            .unwrap_or_else(|_| PathBuf::from("."))
                                    });
                                active_modal = ActiveModal::OpenFile(FilePickerModal::new(
                                    "Open File",
                                    &current_dir,
                                    false,
                                ));
                                continue;
                            }
                            KeyCode::Char('w') => {
                                active_modal = ActiveModal::Search(InputModal::new(
                                    "Where Is (Search)",
                                    "Search for text:",
                                    "",
                                ));
                                continue;
                            }
                            KeyCode::Char('x') => {
                                tabs[active_tab_idx].cut(&mut shared_cutbuffer);
                                continue;
                            }
                            KeyCode::Char('c') => {
                                tabs[active_tab_idx].copy_selection(&mut shared_cutbuffer);
                                tabs[active_tab_idx].status_message =
                                    "Selection Copied".to_string();
                                continue;
                            }
                            KeyCode::Char('v') => {
                                tabs[active_tab_idx].paste(&shared_cutbuffer);
                                continue;
                            }
                            KeyCode::Char('z') => {
                                tabs[active_tab_idx].undo();
                                continue;
                            }
                            KeyCode::Char('y') => {
                                tabs[active_tab_idx].redo();
                                continue;
                            }
                            KeyCode::Char('q') => {
                                let has_modified = tabs.iter().any(|t| t.is_modified);
                                if has_modified {
                                    active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                        "Unsaved Changes",
                                        "One or more tabs have unsaved changes. Exit anyway?",
                                    ));
                                } else {
                                    for t in &tabs {
                                        t.cleanup_swap();
                                    }
                                    break;
                                }
                                continue;
                            }
                            KeyCode::Tab => {
                                if tabs.len() > 1 {
                                    active_tab_idx = (active_tab_idx + 1) % tabs.len();
                                }
                                continue;
                            }
                            KeyCode::BackTab => {
                                if tabs.len() > 1 {
                                    active_tab_idx = (active_tab_idx + tabs.len() - 1) % tabs.len();
                                }
                                continue;
                            }
                            KeyCode::Left => {
                                tabs[active_tab_idx].move_word_left();
                                continue;
                            }
                            KeyCode::Right => {
                                tabs[active_tab_idx].move_word_right();
                                continue;
                            }
                            KeyCode::Up => {
                                tabs[active_tab_idx].move_paragraph_up();
                                continue;
                            }
                            KeyCode::Down => {
                                tabs[active_tab_idx].move_paragraph_down();
                                continue;
                            }
                            _ => {}
                        }
                    }

                    // Alt key combinations
                    if k.modifiers.contains(KeyModifiers::ALT) {
                        match k.code {
                            KeyCode::Char(']') | KeyCode::Right => {
                                if tabs.len() > 1 {
                                    active_tab_idx = (active_tab_idx + 1) % tabs.len();
                                }
                                continue;
                            }
                            KeyCode::Char('[') | KeyCode::Left => {
                                if tabs.len() > 1 {
                                    active_tab_idx = (active_tab_idx + tabs.len() - 1) % tabs.len();
                                }
                                continue;
                            }
                            KeyCode::Char('n') => {
                                show_line_numbers = !show_line_numbers;
                                tabs[active_tab_idx].status_message = format!(
                                    "Line numbers {}",
                                    if show_line_numbers {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                );
                                continue;
                            }
                            _ => {}
                        }
                    }

                    // Standard editing keys
                    let active_tab = &mut tabs[active_tab_idx];
                    match k.code {
                        KeyCode::Esc => {
                            let has_modified = tabs.iter().any(|t| t.is_modified);
                            if has_modified {
                                active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                    "Unsaved Changes",
                                    "One or more tabs have unsaved changes. Exit anyway?",
                                ));
                            } else {
                                for t in &tabs {
                                    t.cleanup_swap();
                                }
                                break;
                            }
                        }
                        KeyCode::Left => active_tab.move_left(),
                        KeyCode::Right => active_tab.move_right(),
                        KeyCode::Up => active_tab.move_up(),
                        KeyCode::Down => active_tab.move_down(),
                        KeyCode::Home => active_tab.move_home(),
                        KeyCode::End => active_tab.move_end(),
                        KeyCode::PageUp => active_tab.page_up(editor_canvas_h),
                        KeyCode::PageDown => active_tab.page_down(editor_canvas_h),
                        KeyCode::Backspace => active_tab.delete_backspace(),
                        KeyCode::Delete => active_tab.delete_char(),
                        KeyCode::Enter => active_tab.insert_newline(),
                        KeyCode::Tab => {
                            for _ in 0..cli.tabstop {
                                active_tab.insert_char(' ');
                            }
                        }
                        KeyCode::Char(ch) => {
                            active_tab.insert_char(ch);
                        }
                        _ => {}
                    }
                }
            } else if let Event::Mouse(m) = &ev {
                if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                    // Immediate mouse interaction handling
                    if m.row == header_y {
                        let close_btn_label = " [X] ";
                        let close_btn_x =
                            area.x + area.width.saturating_sub(close_btn_label.len() as u16);
                        if m.column >= close_btn_x {
                            let has_modified = tabs.iter().any(|t| t.is_modified);
                            if has_modified {
                                active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                    "Unsaved Changes",
                                    "One or more tabs have unsaved changes. Exit anyway?",
                                ));
                            } else {
                                for t in &tabs {
                                    t.cleanup_swap();
                                }
                                break;
                            }
                        }
                    } else if m.row == tab_bar_y {
                        if let Some((add_s, add_e)) = add_tab_region {
                            if m.column >= add_s && m.column < add_e {
                                tabs.push(EditorBuffer::new_empty());
                                active_tab_idx = tabs.len() - 1;
                                continue;
                            }
                        }
                        let mut tab_closed = false;
                        for (idx, cs, ce) in &tab_close_regions {
                            if m.column >= *cs && m.column < *ce {
                                let target_idx = *idx;
                                if tabs[target_idx].is_modified {
                                    let tab_name = tabs[target_idx]
                                        .file_path
                                        .as_ref()
                                        .and_then(|p| p.file_name())
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Untitled".to_string());
                                    active_modal = ActiveModal::ConfirmCloseTab {
                                        tab_idx: target_idx,
                                        modal: ConfirmModal::new(
                                            "Save Modified Tab?",
                                            format!(
                                                "Tab '{}' has unsaved changes. Save before closing?",
                                                tab_name
                                            ),
                                        ),
                                    };
                                } else {
                                    tabs[target_idx].cleanup_swap();
                                    if tabs.len() == 1 {
                                        tabs[0] = EditorBuffer::new_empty();
                                        active_tab_idx = 0;
                                    } else {
                                        tabs.remove(target_idx);
                                        if active_tab_idx >= tabs.len() {
                                            active_tab_idx = tabs.len() - 1;
                                        }
                                    }
                                }
                                tab_closed = true;
                                break;
                            }
                        }
                        if !tab_closed {
                            for (idx, ts, te) in &tab_click_regions {
                                if m.column >= *ts && m.column < *te {
                                    active_tab_idx = *idx;
                                    break;
                                }
                            }
                        }
                    } else if m.row >= editor_canvas_y && m.row < divider_y {
                        let click_v_row = (m.row - editor_canvas_y) as usize;
                        let active_tab = &mut tabs[active_tab_idx];
                        let target_row = active_tab.scroll_row + click_v_row;
                        if target_row < active_tab.lines.len() {
                            active_tab.cursor_row = target_row;
                            let line_offset = if show_line_numbers { 7 } else { 0 };
                            let inner_x = area.x + 1;
                            if m.column >= inner_x + line_offset {
                                let v_col = (m.column - inner_x - line_offset) as usize;
                                active_tab.cursor_col = active_tab.scroll_col + v_col;
                                active_tab.clamp_cursor();
                            }
                        }
                    } else {
                        // Check footer region clicks
                        for r in &footer_hit_regions {
                            if m.row == r.row && m.column >= r.start_x && m.column < r.end_x {
                                match r.action {
                                    FooterAction::HelpManual => {
                                        active_modal = ActiveModal::HelpManual(HelpModal::new(
                                            "BashEdit Manual",
                                            BASHEDIT_HELP_TEXT,
                                        ));
                                    }
                                    FooterAction::ToggleWhitespace => {
                                        show_whitespace = !show_whitespace;
                                        tabs[active_tab_idx].status_message = format!(
                                            "Whitespace display {}",
                                            if show_whitespace {
                                                "enabled"
                                            } else {
                                                "disabled"
                                            }
                                        );
                                    }
                                    FooterAction::AsciiTable => {
                                        active_modal = ActiveModal::AsciiTable(
                                            AsciiGridModal::new("ASCII Character Table", 65),
                                        );
                                    }
                                    FooterAction::ViewColors => {
                                        active_modal = ActiveModal::ViewColors(
                                            ColorGridModal::new("Terminal Colors", 7),
                                        );
                                    }
                                    FooterAction::PlaceholdersHelp => {
                                        active_modal =
                                            ActiveModal::PlaceholdersHelp(HelpModal::new(
                                                "Available Placeholders & Macros",
                                                PLACEHOLDER_HELP_TEXT,
                                            ));
                                    }
                                    FooterAction::MarkdownPreview => {
                                        let md_text = tabs[active_tab_idx].lines.join("\n");
                                        active_modal = ActiveModal::MarkdownPreview(
                                            HelpModal::new("Markdown Preview", &md_text),
                                        );
                                    }
                                    FooterAction::ToggleLineno => {
                                        show_line_numbers = !show_line_numbers;
                                        tabs[active_tab_idx].status_message = format!(
                                            "Line numbers {}",
                                            if show_line_numbers {
                                                "enabled"
                                            } else {
                                                "disabled"
                                            }
                                        );
                                    }
                                    FooterAction::MarkdownReference => {
                                        active_modal =
                                            ActiveModal::MarkdownReference(HelpModal::new(
                                                "Markdown Syntax Reference",
                                                MARKDOWN_REFERENCE_TEXT,
                                            ));
                                    }
                                    FooterAction::NewTab => {
                                        tabs.push(EditorBuffer::new_empty());
                                        active_tab_idx = tabs.len() - 1;
                                    }
                                    FooterAction::OpenFile => {
                                        let current_dir = tabs[active_tab_idx]
                                            .file_path
                                            .as_ref()
                                            .and_then(|p| p.parent())
                                            .map(|p| p.to_path_buf())
                                            .unwrap_or_else(|| {
                                                std::env::current_dir()
                                                    .unwrap_or_else(|_| PathBuf::from("."))
                                            });
                                        active_modal = ActiveModal::OpenFile(FilePickerModal::new(
                                            "Open File",
                                            &current_dir,
                                            false,
                                        ));
                                    }
                                    FooterAction::SaveFile => {
                                        let tab = &mut tabs[active_tab_idx];
                                        if tab.file_path.is_none() {
                                            active_modal =
                                                ActiveModal::SaveAs(make_save_as_modal(tab));
                                        } else {
                                            let res = tab.save_to_disk();
                                            show_save_toast(&mut toast_rack, res, tab);
                                        }
                                    }
                                    FooterAction::SaveFileAs => {
                                        active_modal = ActiveModal::SaveAs(make_save_as_modal(
                                            &tabs[active_tab_idx],
                                        ));
                                    }
                                    FooterAction::Search => {
                                        active_modal = ActiveModal::Search(InputModal::new(
                                            "Where Is (Search)",
                                            "Search for text:",
                                            "",
                                        ));
                                    }
                                    FooterAction::Undo => {
                                        tabs[active_tab_idx].undo();
                                    }
                                    FooterAction::NextTab => {
                                        if tabs.len() > 1 {
                                            active_tab_idx = (active_tab_idx + 1) % tabs.len();
                                        }
                                    }
                                    FooterAction::Exit => {
                                        let has_modified = tabs.iter().any(|t| t.is_modified);
                                        if has_modified {
                                            active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                                "Unsaved Changes",
                                                "One or more tabs have unsaved changes. Exit anyway?",
                                            ));
                                        } else {
                                            for t in &tabs {
                                                t.cleanup_swap();
                                            }
                                            return Ok(());
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                } else if let MouseEventKind::ScrollDown = m.kind {
                    let active_tab = &mut tabs[active_tab_idx];
                    active_tab.cursor_row =
                        (active_tab.cursor_row + 3).min(active_tab.lines.len().saturating_sub(1));
                    active_tab.clamp_cursor();
                } else if let MouseEventKind::ScrollUp = m.kind {
                    let active_tab = &mut tabs[active_tab_idx];
                    active_tab.cursor_row = active_tab.cursor_row.saturating_sub(3);
                    active_tab.clamp_cursor();
                }
            }
        }
    }

    Ok(())
}

fn render_modal(
    active_modal: &mut ActiveModal,
    area: Rect,
    styles: &ResolvedThemeStyles,
    tabs: &[EditorBuffer],
    buf: &mut Buffer,
) -> Option<(u16, u16)> {
    match active_modal {
        ActiveModal::SwapRecovery {
            header, tab_idx, ..
        } => {
            let is_running = is_pid_running(header.pid);
            let prompt_title = if is_running {
                " Swap File Collision Detected! "
            } else {
                " Crash Recovery: Swap File Found! "
            };

            let dialog_w = 64;
            let dialog_h = 10;
            let inner = bashmenu_tui::render_modal_frame(
                area,
                dialog_w,
                dialog_h,
                prompt_title,
                styles,
                buf,
            );

            let file_display = tabs[*tab_idx]
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "buffer".to_string());
            let info_line1 = format!("A swap file exists: ./{}.swp", file_display);
            let info_line2 = format!(
                "Created by PID: {} (Process is {})",
                header.pid,
                if is_running { "RUNNING" } else { "TERMINATED" }
            );
            let options_line = "Press: [R]ecover edits  [D]elete swap  [O]pen read-only  [Q]uit";

            buf.set_string(inner.x + 2, inner.y + 1, info_line1, styles.text);
            buf.set_string(
                inner.x + 2,
                inner.y + 2,
                info_line2,
                if is_running {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            );
            buf.set_string(
                inner.x + 2,
                inner.y + 4,
                options_line,
                styles.highlight.add_modifier(Modifier::BOLD),
            );
            None
        }
        ActiveModal::ConfirmCloseTab { modal, .. } => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::ConfirmExit(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::SaveAs(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::Search(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::OpenFile(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::HelpManual(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::PlaceholdersHelp(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::MarkdownReference(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::MarkdownPreview(modal) => {
            modal.render(area, styles, buf);
            None
        }
        ActiveModal::AsciiTable(modal) => {
            let (_, origin) = modal.render(area, styles, buf);
            Some(origin)
        }
        ActiveModal::ViewColors(modal) => {
            let (_, origin) = modal.render(area, styles, buf);
            Some(origin)
        }
        ActiveModal::None => None,
    }
}

fn show_save_toast(toast_rack: &mut ToastRack, res: Result<()>, tab: &EditorBuffer) {
    let name = tab
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    match res {
        Ok(_) => {
            toast_rack.add(ToastNotification {
                title: Some("Save File".to_string()),
                message: format!("Saved '{}' to disk", name),
                severity: ToastSeverity::Information,
                timeout_secs: 2.0,
            });
        }
        Err(e) => {
            toast_rack.add(ToastNotification {
                title: Some("Error".to_string()),
                message: format!("Failed to save: {}", e),
                severity: ToastSeverity::Error,
                timeout_secs: 3.0,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_active_modal(
    active_modal: &mut ActiveModal,
    tabs: &mut Vec<EditorBuffer>,
    active_tab_idx: &mut usize,
    ev: &Event,
    area: Rect,
    is_cli_read_only: bool,
    last_grid_origin: (u16, u16),
    toast_rack: &mut ToastRack,
) -> bool {
    match active_modal {
        ActiveModal::SwapRecovery {
            recovered_lines,
            tab_idx,
            ..
        } => {
            if let Event::Key(k) = ev {
                match k.code {
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        let tab = &mut tabs[*tab_idx];
                        tab.lines = recovered_lines.clone();
                        tab.is_modified = true;
                        tab.status_message =
                            format!("Recovered {} lines from swap", tab.lines.len());
                        *active_modal = ActiveModal::None;
                        return true;
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        let tab = &mut tabs[*tab_idx];
                        remove_swap_file(&tab.swap_path);
                        tab.status_message = "Deleted stale swap file".to_string();
                        *active_modal = ActiveModal::None;
                        return true;
                    }
                    KeyCode::Char('o') | KeyCode::Char('O') => {
                        let tab = &mut tabs[*tab_idx];
                        tab.is_read_only = true;
                        tab.status_message = "Opened in Read-Only mode".to_string();
                        *active_modal = ActiveModal::None;
                        return true;
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        tabs[*tab_idx].cleanup_swap();
                        if tabs.len() == 1 {
                            std::process::exit(0);
                        } else {
                            tabs.remove(*tab_idx);
                            if *active_tab_idx >= tabs.len() {
                                *active_tab_idx = tabs.len() - 1;
                            }
                            *active_modal = ActiveModal::None;
                            return true;
                        }
                    }
                    _ => return true,
                }
            }
            true
        }
        ActiveModal::ConfirmCloseTab { tab_idx, modal } => {
            let choice = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => None,
            };
            if let Some(confirmed) = choice {
                let target_idx = *tab_idx;
                if confirmed {
                    let tab = &mut tabs[target_idx];
                    if tab.file_path.is_none() {
                        *active_modal = ActiveModal::SaveAs(make_save_as_modal(tab));
                        return true;
                    }
                    let res = tab.save_to_disk();
                    show_save_toast(toast_rack, res, tab);
                }
                tabs[target_idx].cleanup_swap();
                if tabs.len() == 1 {
                    tabs[0] = EditorBuffer::new_empty();
                    *active_tab_idx = 0;
                } else {
                    tabs.remove(target_idx);
                    if *active_tab_idx >= tabs.len() {
                        *active_tab_idx = tabs.len() - 1;
                    }
                }
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::ConfirmExit(modal) => {
            let choice = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => None,
            };
            if let Some(confirmed) = choice {
                if confirmed {
                    for t in tabs.iter() {
                        t.cleanup_swap();
                    }
                    std::process::exit(0);
                } else {
                    *active_modal = ActiveModal::None;
                }
            }
            true
        }
        ActiveModal::SaveAs(modal) => {
            let submitted = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => None,
            };
            if let Some(opt_path) = submitted {
                if let Some(path) = opt_path {
                    let tab = &mut tabs[*active_tab_idx];
                    tab.set_file_path(path);
                    let res = tab.save_to_disk();
                    show_save_toast(toast_rack, res, tab);
                }
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::Search(modal) => {
            let submitted = match ev {
                Event::Key(k) => modal.handle_key_with_modifiers(k.code, k.modifiers),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => None,
            };
            if let Some(opt_query) = submitted {
                if let Some(query) = opt_query {
                    let trimmed = query.trim();
                    if !trimmed.is_empty() {
                        tabs[*active_tab_idx].search(trimmed);
                    }
                }
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::OpenFile(modal) => {
            let chosen = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => None,
            };
            if let Some(opt_path) = chosen {
                if let Some(path) = opt_path {
                    let swp_path = get_swap_path(&path);
                    let recovery = if swp_path.exists() {
                        read_swap_file(&swp_path)
                    } else {
                        None
                    };
                    if let Ok(mut new_buf) = EditorBuffer::open(&path) {
                        if is_cli_read_only {
                            new_buf.is_read_only = true;
                        }
                        if tabs.len() == 1
                            && tabs[0].file_path.is_none()
                            && !tabs[0].is_modified
                            && tabs[0].lines == vec![""]
                        {
                            tabs[0] = new_buf;
                            *active_tab_idx = 0;
                        } else {
                            tabs.push(new_buf);
                            *active_tab_idx = tabs.len() - 1;
                        }
                        if let Some((header, lines)) = recovery {
                            *active_modal = ActiveModal::SwapRecovery {
                                header,
                                recovered_lines: lines,
                                tab_idx: *active_tab_idx,
                            };
                            return true;
                        }
                    }
                }
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::HelpManual(modal) => {
            let dismiss = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => false,
            };
            if dismiss {
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::PlaceholdersHelp(modal) => {
            let dismiss = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => false,
            };
            if dismiss {
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::MarkdownReference(modal) => {
            let dismiss = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => false,
            };
            if dismiss {
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::MarkdownPreview(modal) => {
            let dismiss = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, area),
                _ => false,
            };
            if dismiss {
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::AsciiTable(modal) => {
            let selected = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, last_grid_origin),
                _ => None,
            };
            if let Some(opt_ch) = selected {
                if let Some(ch) = opt_ch {
                    tabs[*active_tab_idx].insert_char(ch);
                }
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::ViewColors(modal) => {
            let selected = match ev {
                Event::Key(k) => modal.handle_key(k.code),
                Event::Mouse(m) => modal.handle_mouse(*m, last_grid_origin),
                _ => None,
            };
            if selected.is_some() {
                *active_modal = ActiveModal::None;
            }
            true
        }
        ActiveModal::None => false,
    }
}
