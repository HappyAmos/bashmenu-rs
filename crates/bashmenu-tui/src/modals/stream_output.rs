//! Modal window displaying real-time streaming output of a running command.

use super::frame::{is_close_button_clicked, render_modal_frame};
use crate::ansi::ansi_to_line;
use crate::style::ResolvedThemeStyles;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Messages emitted by the background streaming process
pub enum StreamOutputMsg {
    Line(String),
    Finished(Option<i32>),
}

/// Action result from handling keyboard or mouse events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutputAction {
    None,
    Close,
}

/// Modal window displaying live streaming command output
pub struct StreamOutputModal {
    pub title: String,
    pub command: String,
    pub window_percent: u16,
    pub interactive: bool,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub is_running: bool,
    pub exit_code: Option<i32>,
    pub child_pid: Option<u32>,
    rx: Option<Receiver<StreamOutputMsg>>,
    stdin_tx: Option<Sender<Vec<u8>>>,
}

impl StreamOutputModal {
    /// Creates a new `StreamOutputModal`
    pub fn new(title: String, command: String, window_percent: u16, interactive: bool) -> Self {
        Self {
            title,
            command,
            window_percent: window_percent.clamp(20, 100),
            interactive,
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            is_running: false,
            exit_code: None,
            child_pid: None,
            rx: None,
            stdin_tx: None,
        }
    }

    /// Spawns the background command process and begins streaming stdout/stderr
    pub fn start(&mut self) {
        if self.is_running {
            return;
        }

        let (msg_tx, msg_rx) = channel::<StreamOutputMsg>();
        let (in_tx, in_rx) = channel::<Vec<u8>>();
        self.rx = Some(msg_rx);
        self.stdin_tx = Some(in_tx);
        self.is_running = true;

        let cmd_str = self.command.clone();

        let spawn_res = Command::new("/bin/bash")
            .arg("-c")
            .arg(format!("{} 2>&1", cmd_str))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match spawn_res {
            Ok(mut child) => {
                let pid = child.id();
                self.child_pid = Some(pid);

                // Handle child stdin in worker thread
                if let Some(mut stdin) = child.stdin.take() {
                    thread::spawn(move || {
                        while let Ok(bytes) = in_rx.recv() {
                            if stdin.write_all(&bytes).is_err() || stdin.flush().is_err() {
                                break;
                            }
                        }
                    });
                }

                // Handle child stdout in worker thread
                if let Some(stdout) = child.stdout.take() {
                    let msg_sender = msg_tx.clone();
                    thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = msg_sender.send(StreamOutputMsg::Line(line));
                        }
                        let status = child.wait().ok();
                        let code = status.and_then(|s| s.code());
                        let _ = msg_sender.send(StreamOutputMsg::Finished(code));
                    });
                }
            }
            Err(e) => {
                self.is_running = false;
                self.lines.push(format!("Failed to execute command: {}", e));
                self.exit_code = Some(1);
            }
        }
    }

    /// Polls available incoming lines from the child process without blocking.
    /// Returns true if any updates were processed.
    pub fn poll_output(&mut self) -> bool {
        let mut updated = false;
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                updated = true;
                match msg {
                    StreamOutputMsg::Line(line) => {
                        self.lines.push(line);
                    }
                    StreamOutputMsg::Finished(code) => {
                        self.is_running = false;
                        self.exit_code = code;
                    }
                }
            }
        }
        updated
    }

    /// Terminates the child process if it is currently running
    pub fn terminate(&mut self) {
        if self.is_running {
            if let Some(pid) = self.child_pid {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
            self.is_running = false;
        }
    }

    /// Handles keyboard events
    pub fn handle_key(&mut self, key: KeyEvent) -> StreamOutputAction {
        match key.code {
            KeyCode::Esc => {
                self.terminate();
                StreamOutputAction::Close
            }
            KeyCode::Enter => {
                if !self.is_running {
                    StreamOutputAction::Close
                } else {
                    if let Some(ref tx) = self.stdin_tx {
                        let _ = tx.send(b"\n".to_vec());
                    }
                    StreamOutputAction::None
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.auto_scroll = false;
                if self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                StreamOutputAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                StreamOutputAction::None
            }
            KeyCode::PageUp => {
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                StreamOutputAction::None
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
                StreamOutputAction::None
            }
            KeyCode::Home => {
                self.auto_scroll = false;
                self.scroll_offset = 0;
                StreamOutputAction::None
            }
            KeyCode::End => {
                self.auto_scroll = true;
                StreamOutputAction::None
            }
            KeyCode::Char(c) => {
                if self.is_running && self.interactive {
                    if let Some(ref tx) = self.stdin_tx {
                        let mut b = [0u8; 4];
                        let s = c.encode_utf8(&mut b);
                        let _ = tx.send(s.as_bytes().to_vec());
                    }
                }
                StreamOutputAction::None
            }
            _ => StreamOutputAction::None,
        }
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> StreamOutputAction {
        let (dialog_w, dialog_h) = self.calculate_dialog_dimensions(area);
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
                StreamOutputAction::None
            }
            MouseEventKind::ScrollDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(3);
                StreamOutputAction::None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_close_button_clicked(inner, mouse.row, mouse.column) {
                    self.terminate();
                    StreamOutputAction::Close
                } else {
                    StreamOutputAction::None
                }
            }
            _ => StreamOutputAction::None,
        }
    }

    /// Calculates dialog dimensions based on screen area and configured window percentage
    fn calculate_dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let pct = self.window_percent as u32;
        let w = ((area.width as u32 * pct) / 100)
            .max(30)
            .min(area.width.saturating_sub(2) as u32) as u16;
        let h = ((area.height as u32 * pct) / 100)
            .max(10)
            .min(area.height.saturating_sub(2) as u32) as u16;
        (w, h)
    }

    /// Renders the streaming modal output dialog into `buf`
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let (dialog_w, dialog_h) = self.calculate_dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        if inner.width < 4 || inner.height < 2 {
            return;
        }

        let content_h = inner.height.saturating_sub(1) as usize;
        let avail_w = inner.width.saturating_sub(3) as usize; // reserve margin and scrollbar

        let total_lines = self.lines.len();
        let max_scroll = total_lines.saturating_sub(content_h);

        let eff_scroll = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        let default_text_style = styles.text.bg(styles.background);

        // 1. Render Output Lines
        for row_idx in 0..content_h {
            let line_idx = eff_scroll + row_idx;
            let y = inner.y + row_idx as u16;

            if line_idx < total_lines {
                let raw_line = &self.lines[line_idx];
                let styled_line = ansi_to_line(raw_line, default_text_style);
                buf.set_line(inner.x + 1, y, &styled_line, avail_w as u16);
            }
        }

        // 2. Render Scrollbar if total lines exceed window height
        if total_lines > content_h && content_h > 0 {
            let sb_x = inner.x + inner.width.saturating_sub(1);
            let content_start_y = inner.y;
            let sb_fg = styles
                .scrollbar
                .fg
                .unwrap_or(styles.accent.fg.unwrap_or(Color::Cyan));
            let sb_bg = styles.background;
            let arrow_style = Style::default()
                .fg(sb_fg)
                .bg(sb_bg)
                .add_modifier(Modifier::BOLD);
            let thumb_style = Style::default().fg(sb_fg).bg(sb_bg);
            let track_style = Style::default().fg(Color::DarkGray).bg(sb_bg);

            if content_h >= 3 {
                buf.set_string(sb_x, content_start_y, "▲", arrow_style);
                buf.set_string(
                    sb_x,
                    content_start_y + content_h as u16 - 1,
                    "▼",
                    arrow_style,
                );

                let track_len = content_h - 2;
                let thumb_size = ((track_len as f64 * content_h as f64) / total_lines as f64)
                    .round()
                    .clamp(1.0, track_len as f64) as usize;
                let available_track = track_len.saturating_sub(thumb_size);
                let scroll_ratio = if max_scroll > 0 {
                    eff_scroll as f64 / max_scroll as f64
                } else {
                    0.0
                };
                let thumb_start = (scroll_ratio * available_track as f64).round() as usize;

                for t_idx in 0..track_len {
                    let y = content_start_y + 1 + t_idx as u16;
                    if t_idx >= thumb_start && t_idx < thumb_start + thumb_size {
                        buf.set_string(sb_x, y, "█", thumb_style);
                    } else {
                        buf.set_string(sb_x, y, "░", track_style);
                    }
                }
            }
        }

        // 3. Render Status Footer
        let footer_y = inner.y + inner.height - 1;
        let footer_style = Style::default()
            .fg(styles.accent.fg.unwrap_or(Color::Cyan))
            .bg(styles.background)
            .add_modifier(Modifier::BOLD);

        let footer_text = if self.is_running {
            " ● Running...  [ESC]: Terminate / Close ".to_string()
        } else if let Some(code) = self.exit_code {
            if code == 0 {
                " ✔ Completed (Exit: 0)  [ESC/ENTER]: Close | [▲/▼/PgUp/PgDn]: Scroll ".to_string()
            } else {
                format!(
                    " ✘ Finished (Exit: {})  [ESC/ENTER]: Close | [▲/▼/PgUp/PgDn]: Scroll ",
                    code
                )
            }
        } else {
            " [ESC/ENTER]: Close | [▲/▼/PgUp/PgDn]: Scroll ".to_string()
        };

        buf.set_string(inner.x + 1, footer_y, footer_text, footer_style);
    }
}

impl Drop for StreamOutputModal {
    fn drop(&mut self) {
        self.terminate();
    }
}
