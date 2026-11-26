//! Modal window displaying real-time streaming output of a running command.

use super::frame::{is_close_button_clicked, render_modal_frame};
use crate::ansi::ansi_to_line;
use crate::style::ResolvedThemeStyles;
use bashmenu_core::t;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Messages emitted by the background streaming process
pub enum StreamOutputMsg {
    /// A completed line of standard output or error received from child process
    Line(String),
    /// An in-progress line without a trailing newline (e.g., interactive prompt)
    PartialLine(String),
    /// Child process has exited with optional status code
    Finished(Option<i32>),
}

/// Action result from handling keyboard or mouse events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutputAction {
    /// No modal state transition
    None,
    /// Request to close and dismiss the streaming modal
    Close,
}

/// Modal window displaying live streaming command output
pub struct StreamOutputModal {
    /// Dialog title header
    pub title: String,
    /// Command string being executed
    pub command: String,
    /// Viewport size percentage relative to terminal dimensions
    pub window_percent: u16,
    /// Whether stdin remains connected for interactive user inputs
    pub interactive: bool,
    /// Buffered output lines collected from process stdout/stderr
    pub lines: Vec<String>,
    /// Vertical scroll offset in lines
    pub scroll_offset: usize,
    /// Whether viewport automatically follows the latest output line
    pub auto_scroll: bool,
    /// Whether the background child process is actively executing
    pub is_running: bool,
    /// Process exit code upon termination
    pub exit_code: Option<i32>,
    /// Operating system process ID of child command
    pub child_pid: Option<u32>,
    /// Receiver channel for stdout/stderr lines and termination messages from worker thread
    rx: Option<Receiver<StreamOutputMsg>>,
    /// Sender channel for delivering interactive keyboard input to child stdin
    stdin_tx: Option<Sender<Vec<u8>>>,
    /// Target PTY dimensions `(cols, rows)` used when allocating an interactive PTY
    pty_size: (u16, u16),
    /// Whether the running child process is attached to a PTY
    uses_pty: bool,
    /// Whether the last entry in `lines` is an in-progress partial line (no trailing newline yet)
    has_partial_line: bool,
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
            pty_size: (80, 24),
            uses_pty: false,
            has_partial_line: false,
        }
    }

    /// Sets the target PTY dimensions `(cols, rows)` before [`Self::start`] is called.
    pub fn set_pty_size(&mut self, cols: u16, rows: u16) {
        self.pty_size = (cols.max(20), rows.max(4));
    }

    /// Spawns the background command process and begins streaming stdout/stderr.
    ///
    /// When `self.interactive` is `true` on Unix, allocates a real POSIX PTY so
    /// TTY-aware CLIs (such as `gh`, `git`, `cargo`, and interactive prompts)
    /// behave naturally; falls back to standard pipes if PTY allocation fails
    /// or when `self.interactive` is `false`.
    pub fn start(&mut self) {
        if self.is_running {
            return;
        }

        let (msg_tx, msg_rx) = channel::<StreamOutputMsg>();
        let (in_tx, in_rx) = channel::<Vec<u8>>();
        self.rx = Some(msg_rx);
        self.stdin_tx = Some(in_tx);
        self.is_running = true;
        self.uses_pty = false;
        self.has_partial_line = false;

        let cmd_str = self.command.clone();

        #[cfg(unix)]
        if self.interactive {
            let (cols, rows) = self.pty_size;
            if let Ok((mut child, master_file)) = spawn_unix_pty(&cmd_str, cols, rows) {
                self.child_pid = Some(child.id());
                self.uses_pty = true;

                if let Ok(mut writer) = master_file.try_clone() {
                    thread::spawn(move || {
                        while let Ok(bytes) = in_rx.recv() {
                            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                                break;
                            }
                        }
                    });
                }

                let msg_sender = msg_tx.clone();
                thread::spawn(move || {
                    let mut reader = master_file;
                    let mut buf = [0u8; 4096];
                    let mut pending = String::new();
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let chunk = String::from_utf8_lossy(&buf[..n]);
                                emit_pty_chunk(&mut pending, &chunk, &msg_sender);
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(_) => break, // EIO is standard EOF when slave PTY closes on Linux
                        }
                    }
                    if !pending.is_empty() {
                        let _ = msg_sender.send(StreamOutputMsg::Line(pending));
                    }
                    let status = child.wait().ok();
                    let code = status.and_then(|s| s.code());
                    let _ = msg_sender.send(StreamOutputMsg::Finished(code));
                });
                return;
            }
        }

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
                let err_str = e.to_string();
                self.lines
                    .push(t!("bashmenu.stream_exec_failed", err => &err_str));
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
                        let clean = line.replace('\t', "    ");
                        if self.has_partial_line {
                            if let Some(last) = self.lines.last_mut() {
                                *last = clean;
                            } else {
                                self.lines.push(clean);
                            }
                            self.has_partial_line = false;
                        } else {
                            self.lines.push(clean);
                        }
                    }
                    StreamOutputMsg::PartialLine(line) => {
                        let clean = line.replace('\t', "    ");
                        if self.has_partial_line {
                            if let Some(last) = self.lines.last_mut() {
                                *last = clean;
                            } else {
                                self.lines.push(clean);
                            }
                        } else {
                            self.lines.push(clean);
                            self.has_partial_line = true;
                        }
                    }
                    StreamOutputMsg::Finished(code) => {
                        self.is_running = false;
                        self.exit_code = code;
                        self.has_partial_line = false;
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
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/T", "/F"])
                        .status();
                }
            }
            self.is_running = false;
        }
    }

    /// Handles keyboard events
    pub fn handle_key(&mut self, key: KeyEvent) -> StreamOutputAction {
        let is_live_interactive = self.is_running && self.interactive;

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
                        let nl: &[u8] = if self.uses_pty { b"\r" } else { b"\n" };
                        let _ = tx.send(nl.to_vec());
                    }
                    StreamOutputAction::None
                }
            }
            KeyCode::Backspace if is_live_interactive => {
                if let Some(ref tx) = self.stdin_tx {
                    let _ = tx.send(vec![0x7f]);
                }
                StreamOutputAction::None
            }
            KeyCode::Up => {
                self.auto_scroll = false;
                if self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                StreamOutputAction::None
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                StreamOutputAction::None
            }
            KeyCode::Char('k') if !is_live_interactive => {
                self.auto_scroll = false;
                if self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                StreamOutputAction::None
            }
            KeyCode::Char('j') if !is_live_interactive => {
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
            KeyCode::Char(c) if is_live_interactive => {
                if let Some(ref tx) = self.stdin_tx {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match c.to_ascii_lowercase() {
                            'c' => {
                                let _ = tx.send(vec![0x03]);
                            }
                            'd' => {
                                let _ = tx.send(vec![0x04]);
                            }
                            _ => {}
                        }
                    } else {
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
                let footer_y = inner.y + inner.height.saturating_sub(1);
                if is_close_button_clicked(inner, mouse.row, mouse.column)
                    || (mouse.row == footer_y
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width)
                {
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
    pub fn calculate_dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        let pct = self.window_percent as u32;
        let w = ((area.width as u32 * pct) / 100)
            .max(30)
            .min(area.width.saturating_sub(2) as u32) as u16;
        let h = ((area.height as u32 * pct) / 100)
            .max(10)
            .min(area.height.saturating_sub(2) as u32) as u16;
        (w, h)
    }

    /// Returns the target dialog dimensions for this modal.
    pub fn dialog_dimensions(&self, area: Rect) -> (u16, u16) {
        self.calculate_dialog_dimensions(area)
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
        if content_h >= 3 {
            super::frame::render_modal_scrollbar(
                inner.x + inner.width.saturating_sub(1),
                inner.y,
                content_h,
                total_lines,
                eff_scroll,
                styles,
                buf,
            );
        }

        // 3. Render Status Footer
        let footer_y = inner.y + inner.height - 1;
        let footer_style = Style::default()
            .fg(styles.accent.fg.unwrap_or(Color::Cyan))
            .bg(styles.background)
            .add_modifier(Modifier::BOLD);

        let footer_text = if self.is_running {
            t!("bashmenu.stream_running")
        } else if let Some(code) = self.exit_code {
            if code == 0 {
                t!("bashmenu.stream_completed")
            } else {
                t!("bashmenu.stream_failed", code => code)
            }
        } else {
            t!("bashmenu.stream_scroll")
        };

        let avail_footer_w = inner.width.saturating_sub(2) as usize;
        let clipped_footer: String = footer_text.chars().take(avail_footer_w).collect();
        buf.set_string(inner.x + 1, footer_y, &clipped_footer, footer_style);
    }
}

impl Drop for StreamOutputModal {
    /// Terminates the child streaming process and reader thread upon dropping the modal.
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Splits a raw PTY text chunk into completed lines and in-progress partial lines,
/// normalizing `\r\n` newlines and standalone `\r` carriage-return overwrites.
#[cfg(unix)]
fn emit_pty_chunk(pending: &mut String, chunk: &str, sender: &Sender<StreamOutputMsg>) {
    let mut chars = chunk.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                let line = std::mem::take(pending);
                let _ = sender.send(StreamOutputMsg::Line(line));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    let line = std::mem::take(pending);
                    let _ = sender.send(StreamOutputMsg::Line(line));
                } else {
                    pending.clear();
                }
            }
            '\x08' => {
                pending.pop();
            }
            other => {
                pending.push(other);
            }
        }
    }
    if !pending.is_empty() {
        let _ = sender.send(StreamOutputMsg::PartialLine(pending.clone()));
    }
}

/// Allocates a POSIX pseudo-terminal and spawns `/bin/bash -c <cmd_str>` attached to it.
#[cfg(unix)]
fn spawn_unix_pty(
    cmd_str: &str,
    cols: u16,
    rows: u16,
) -> std::io::Result<(std::process::Child, std::fs::File)> {
    use std::ffi::{CStr, CString};
    use std::os::unix::io::FromRawFd;
    use std::os::unix::process::CommandExt;

    let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    if master_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Wrap immediately so any early error closes master_fd cleanly
    let master_file = unsafe { std::fs::File::from_raw_fd(master_fd) };

    if unsafe { libc::grantpt(master_fd) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::unlockpt(master_fd) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let slave_path_ptr = unsafe { libc::ptsname(master_fd) };
    if slave_path_ptr.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let slave_path = unsafe { CStr::from_ptr(slave_path_ptr) }.to_owned();
    let slave_cstr = CString::new(slave_path.as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let ws = libc::winsize {
        ws_row: rows.max(4),
        ws_col: cols.max(20),
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let _ = unsafe { libc::ioctl(master_fd, libc::TIOCSWINSZ as _, &ws) };

    let slave_fd = unsafe { libc::open(slave_cstr.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if slave_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let slave_stdin = unsafe { std::fs::File::from_raw_fd(slave_fd) };
    let slave_stdout = slave_stdin.try_clone()?;
    let slave_stderr = slave_stdin.try_clone()?;

    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-c")
        .arg(cmd_str)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::from(slave_stdin))
        .stdout(Stdio::from(slave_stdout))
        .stderr(Stdio::from(slave_stderr));

    // Safety: setsid + TIOCSCTTY are async-signal-safe syscalls executed in the child before exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            let _ = libc::ioctl(0, libc::TIOCSCTTY as _, 0);
            Ok(())
        });
    }

    let child = cmd.spawn()?;
    Ok((child, master_file))
}
