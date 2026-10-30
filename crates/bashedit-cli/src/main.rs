//! Full-screen terminal text editor with colocated swap files and crash recovery.

use anyhow::Result;
use bashedit_cli::{
    get_swap_path, is_pid_running, read_swap_file, remove_swap_file, EditorBuffer, SwapHeader,
};
use bashmenu_tui::{install_panic_hook, ConfirmModal, ResolvedThemeStyles, TerminalGuard};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "bashedit",
    author,
    version,
    about = "Terminal text editor with crash recovery and colocated swap files"
)]
struct Cli {
    /// File to edit
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Open in read-only mode
    #[arg(short, long)]
    read_only: bool,
}

enum ActiveModal {
    None,
    SwapRecovery(SwapHeader, Vec<String>),
    ConfirmExit(ConfirmModal),
}

fn main() -> Result<()> {
    install_panic_hook();

    let cli = Cli::parse();
    let file_path = cli.file;

    let mut guard = TerminalGuard::init()?;
    let styles = ResolvedThemeStyles::default();

    // Check for existing swap file before opening buffer
    let swp_path = get_swap_path(&file_path);
    let mut initial_recovery = None;

    if swp_path.exists() {
        if let Some((header, lines)) = read_swap_file(&swp_path) {
            initial_recovery = Some((header, lines));
        }
    }

    let mut buffer = EditorBuffer::open(&file_path)?;
    if cli.read_only {
        buffer.is_read_only = true;
    }

    let mut active_modal = if let Some((header, lines)) = initial_recovery {
        ActiveModal::SwapRecovery(header, lines)
    } else {
        ActiveModal::None
    };

    loop {
        let size = guard.terminal_mut().size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        // Adjust scroll view to keep cursor visible
        let text_h = area.height.saturating_sub(4) as usize;
        let line_num_w = 6;
        let text_w = (area.width as usize).saturating_sub(line_num_w + 4);

        if buffer.cursor_row < buffer.scroll_row {
            buffer.scroll_row = buffer.cursor_row;
        } else if buffer.cursor_row >= buffer.scroll_row + text_h && text_h > 0 {
            buffer.scroll_row = buffer.cursor_row.saturating_sub(text_h - 1);
        }

        if buffer.cursor_col < buffer.scroll_col {
            buffer.scroll_col = buffer.cursor_col;
        } else if buffer.cursor_col >= buffer.scroll_col + text_w && text_w > 0 {
            buffer.scroll_col = buffer.cursor_col.saturating_sub(text_w - 1);
        }

        guard.terminal_mut().draw(|f| {
            let buf = f.buffer_mut();

            // Header status block
            let status_flags = if buffer.is_read_only {
                "[Read-Only]"
            } else if buffer.is_modified {
                "[Modified]"
            } else {
                "[Clean]"
            };

            let title = format!(
                " BashEdit: {}  Ln {}, Col {}  Total: {} lines  {} ",
                buffer.file_path.display(),
                buffer.cursor_row + 1,
                buffer.cursor_col + 1,
                buffer.lines.len(),
                status_flags
            );

            let root_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" [ {} ] ", title))
                .border_style(Style::default().fg(Color::Cyan));
            root_block.render(area, buf);

            if area.width < 20 || area.height < 5 {
                return;
            }

            let inner_x = area.x + 1;
            let inner_y = area.y + 1;

            // Render visible text lines with line numbers
            for view_row in 0..text_h {
                let doc_row = buffer.scroll_row + view_row;
                let screen_y = inner_y + view_row as u16;

                if doc_row >= buffer.lines.len() {
                    // Past end of file
                    buf.set_string(inner_x, screen_y, "~", Style::default().fg(Color::DarkGray));
                    continue;
                }

                // Line number
                let l_num_str = format!("{:>4} │ ", doc_row + 1);
                buf.set_string(
                    inner_x,
                    screen_y,
                    &l_num_str,
                    Style::default().fg(Color::DarkGray),
                );

                // Line text content
                let line_str = &buffer.lines[doc_row];
                let chars: Vec<char> = line_str.chars().collect();
                let text_start_x = inner_x + line_num_w as u16;

                for view_col in 0..text_w {
                    let doc_col = buffer.scroll_col + view_col;
                    let screen_x = text_start_x + view_col as u16;

                    let ch = chars.get(doc_col).copied().unwrap_or(' ');
                    let is_cursor = doc_row == buffer.cursor_row && doc_col == buffer.cursor_col;

                    let style = if is_cursor {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    buf.set_string(screen_x, screen_y, ch.to_string(), style);
                }

                // Cursor at end of line
                if doc_row == buffer.cursor_row && buffer.cursor_col >= chars.len() {
                    let cursor_view_col = buffer.cursor_col.saturating_sub(buffer.scroll_col);
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

            // Footer bar
            let footer_y = area.y + area.height - 1;
            let footer_text = " [Ctrl+S] Save | [Ctrl+Q] Exit | [F12] Help ";
            buf.set_string(
                inner_x,
                footer_y,
                footer_text,
                Style::default().fg(Color::DarkGray),
            );

            if !buffer.status_message.is_empty() {
                let msg_x =
                    (area.x + area.width).saturating_sub(buffer.status_message.len() as u16 + 3);
                buf.set_string(
                    msg_x,
                    footer_y,
                    &buffer.status_message,
                    Style::default().fg(Color::Green),
                );
            }

            // Render Modals
            match &mut active_modal {
                ActiveModal::SwapRecovery(header, _) => {
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
                        &styles,
                        buf,
                    );

                    let info_line1 = format!(
                        "A swap file exists: ./{}.swp",
                        buffer.file_path.file_name().unwrap().to_string_lossy()
                    );
                    let info_line2 = format!(
                        "Created by PID: {} (Process is {})",
                        header.pid,
                        if is_running { "RUNNING" } else { "TERMINATED" }
                    );
                    let options_line =
                        "Press: [R]ecover edits  [D]elete swap  [O]pen read-only  [Q]uit";

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
                }
                ActiveModal::ConfirmExit(modal) => {
                    modal.render(area, &styles, buf);
                }
                ActiveModal::None => {}
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            match &mut active_modal {
                ActiveModal::SwapRecovery(_header, recovered_lines) => {
                    if let Event::Key(k) = ev {
                        match k.code {
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                // Recover: load lines from swap
                                buffer.lines = recovered_lines.clone();
                                buffer.is_modified = true;
                                buffer.status_message =
                                    format!("Recovered {} lines from swap", buffer.lines.len());
                                active_modal = ActiveModal::None;
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                // Delete swap and load disk file
                                remove_swap_file(&buffer.swap_path);
                                buffer.status_message = "Deleted stale swap file".to_string();
                                active_modal = ActiveModal::None;
                            }
                            KeyCode::Char('o') | KeyCode::Char('O') => {
                                // Open Read-Only
                                buffer.is_read_only = true;
                                buffer.status_message = "Opened in Read-Only mode".to_string();
                                active_modal = ActiveModal::None;
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                // Abort and exit
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                ActiveModal::ConfirmExit(modal) => {
                    if let Event::Key(k) = ev {
                        if let Some(confirmed) = modal.handle_key(k.code) {
                            if confirmed {
                                buffer.cleanup_swap();
                                break;
                            } else {
                                active_modal = ActiveModal::None;
                            }
                        }
                    }
                }
                ActiveModal::None => {
                    if let Event::Key(k) = ev {
                        // Check for Ctrl shortcuts
                        if k.modifiers.contains(KeyModifiers::CONTROL) {
                            match k.code {
                                KeyCode::Char('s') => {
                                    let _ = buffer.save_to_disk();
                                    continue;
                                }
                                KeyCode::Char('q') => {
                                    if buffer.is_modified {
                                        active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                            "Unsaved Changes",
                                            "Discard unsaved changes and exit?",
                                        ));
                                    } else {
                                        buffer.cleanup_swap();
                                        break;
                                    }
                                    continue;
                                }
                                _ => {}
                            }
                        }

                        match k.code {
                            KeyCode::Esc => {
                                if buffer.is_modified {
                                    active_modal = ActiveModal::ConfirmExit(ConfirmModal::new(
                                        "Unsaved Changes",
                                        "Discard unsaved changes and exit?",
                                    ));
                                } else {
                                    buffer.cleanup_swap();
                                    break;
                                }
                            }
                            KeyCode::Left => buffer.move_left(),
                            KeyCode::Right => buffer.move_right(),
                            KeyCode::Up => buffer.move_up(),
                            KeyCode::Down => buffer.move_down(),
                            KeyCode::Home => buffer.move_home(),
                            KeyCode::End => buffer.move_end(),
                            KeyCode::PageUp => buffer.page_up(text_h),
                            KeyCode::PageDown => buffer.page_down(text_h),
                            KeyCode::Backspace => buffer.delete_backspace(),
                            KeyCode::Delete => buffer.delete_char(),
                            KeyCode::Enter => buffer.insert_newline(),
                            KeyCode::Tab => {
                                for _ in 0..4 {
                                    buffer.insert_char(' ');
                                }
                            }
                            KeyCode::Char(ch) => {
                                buffer.insert_char(ch);
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
