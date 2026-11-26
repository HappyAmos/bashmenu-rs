//! Modal window displaying real-time streaming output or an embedded VT100
//! interactive terminal for a running command.

use super::frame::{is_close_button_clicked, render_modal_frame};
use crate::ansi::ansi_to_line;
use crate::style::ResolvedThemeStyles;
use bashmenu_core::t;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
#[cfg(unix)]
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Default scrollback history depth (in lines) for the embedded VT100 screen.
const VT100_SCROLLBACK_LINES: usize = 2000;

/// Messages emitted by the background streaming process
pub enum StreamOutputMsg {
    /// Raw byte chunk received from a PTY child process (fed into `vt100::Parser`)
    RawBytes(Vec<u8>),
    /// A completed line of standard output or error received from a piped child process
    Line(String),
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

/// Modal window displaying live streaming command output or an interactive VT100 terminal.
pub struct StreamOutputModal {
    /// Dialog title header
    pub title: String,
    /// Command string being executed
    pub command: String,
    /// Viewport size percentage relative to terminal dimensions
    pub window_percent: u16,
    /// Whether stdin remains connected for interactive user inputs
    pub interactive: bool,
    /// Buffered output lines collected from process stdout/stderr (or synced from VT100 screen)
    pub lines: Vec<String>,
    /// Vertical scroll offset in lines (non-VT100 mode)
    pub scroll_offset: usize,
    /// Whether viewport automatically follows the latest output line
    pub auto_scroll: bool,
    /// Whether the background child process is actively executing
    pub is_running: bool,
    /// Process exit code upon termination
    pub exit_code: Option<i32>,
    /// Operating system process ID of child command
    pub child_pid: Option<u32>,
    /// Receiver channel for stdout/stderr output and termination messages from worker thread
    rx: Option<Receiver<StreamOutputMsg>>,
    /// Sender channel for delivering interactive keyboard input to child stdin
    stdin_tx: Option<Sender<Vec<u8>>>,
    /// Target PTY dimensions `(cols, rows)` used when allocating an interactive PTY
    pty_size: (u16, u16),
    /// Whether the running child process is attached to a PTY
    uses_pty: bool,
    /// Embedded VT100 terminal screen parser when running in PTY mode
    vt_parser: Option<Box<vt100::Parser>>,
    /// Carry-over bytes for terminal queries split across PTY read chunks
    vt_query_tail: Vec<u8>,
    /// Scrollback line offset into VT100 history (`0` = live bottom view)
    vt_scrollback: usize,
    /// Duplicated PTY master file handle used for `TIOCSWINSZ` resizes
    #[cfg(unix)]
    pty_resize_handle: Option<std::fs::File>,
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
            vt_parser: None,
            vt_query_tail: Vec::new(),
            vt_scrollback: 0,
            #[cfg(unix)]
            pty_resize_handle: None,
        }
    }

    /// Sets the target PTY dimensions `(cols, rows)` before [`Self::start`] is called.
    pub fn set_pty_size(&mut self, cols: u16, rows: u16) {
        self.pty_size = (cols.max(20), rows.max(4));
    }

    /// Resizes the embedded VT100 screen and PTY when the outer terminal resizes.
    pub fn resize_for_area(&mut self, area: Rect) {
        let (dw, dh) = self.calculate_dialog_dimensions(area);
        let cols = dw.saturating_sub(5).max(20);
        let rows = dh.saturating_sub(3).max(4);
        self.pty_size = (cols, rows);
        if let Some(ref mut parser) = self.vt_parser {
            parser.set_size(rows, cols);
        }
        #[cfg(unix)]
        if let Some(ref handle) = self.pty_resize_handle {
            use std::os::unix::io::AsRawFd;
            let ws = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let _ = unsafe { libc::ioctl(handle.as_raw_fd(), libc::TIOCSWINSZ as _, &ws) };
        }
    }

    /// Forwards pasted text directly to the running interactive child process.
    pub fn handle_paste(&mut self, text: &str) {
        if self.is_running && self.interactive && !text.is_empty() {
            self.reset_vt_scrollback();
            if let Some(ref tx) = self.stdin_tx {
                let _ = tx.send(text.as_bytes().to_vec());
            }
        }
    }

    /// Spawns the background command process and begins streaming stdout/stderr.
    ///
    /// When `self.interactive` is `true` on Unix, allocates a real POSIX PTY and
    /// an embedded `vt100::Parser` terminal screen so TTY-aware CLIs and
    /// interactive prompts work natively; falls back to standard pipes if PTY
    /// allocation fails or when `self.interactive` is `false`.
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
        self.vt_parser = None;
        self.vt_query_tail.clear();
        self.vt_scrollback = 0;

        let cmd_str = self.command.clone();

        #[cfg(unix)]
        if self.interactive {
            let (cols, rows) = self.pty_size;
            if let Ok((mut child, master_file)) = spawn_unix_pty(&cmd_str, cols, rows) {
                self.child_pid = Some(child.id());
                self.uses_pty = true;
                self.vt_parser = Some(Box::new(vt100::Parser::new(
                    rows,
                    cols,
                    VT100_SCROLLBACK_LINES,
                )));
                self.pty_resize_handle = master_file.try_clone().ok();

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
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let _ =
                                    msg_sender.send(StreamOutputMsg::RawBytes(buf[..n].to_vec()));
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(_) => break, // EIO is standard EOF when slave PTY closes on Linux
                        }
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

    /// Polls available incoming output from the child process without blocking.
    /// Returns true if any updates were processed.
    pub fn poll_output(&mut self) -> bool {
        let mut updated = false;
        let mut vt_dirty = false;
        let mut pending = Vec::new();
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                pending.push(msg);
            }
        }
        for msg in pending {
            updated = true;
            match msg {
                StreamOutputMsg::RawBytes(bytes) => {
                    if self.vt_parser.is_some() {
                        self.process_vt_bytes(&bytes);
                        vt_dirty = true;
                    } else {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            self.lines.push(line.replace('\t', "    "));
                        }
                    }
                }
                StreamOutputMsg::Line(line) => {
                    self.lines.push(line.replace('\t', "    "));
                }
                StreamOutputMsg::Finished(code) => {
                    self.is_running = false;
                    self.exit_code = code;
                    vt_dirty = self.vt_parser.is_some();
                }
            }
        }
        if vt_dirty {
            if let Some(ref mut parser) = self.vt_parser {
                if self.auto_scroll {
                    self.vt_scrollback = 0;
                }
                parser.set_scrollback(self.vt_scrollback);
                self.lines = parser
                    .screen()
                    .contents()
                    .lines()
                    .map(|l| l.to_string())
                    .collect();
            }
        }
        updated
    }

    /// Feeds PTY output into the embedded `vt100::Parser` while answering
    /// standard terminal queries (DSR/CPR, DA1/DA2, OSC 10/11) at the exact
    /// stream position so interactive CLIs (e.g. `gh`, `survey`, `lipgloss`)
    /// never block waiting for terminal replies.
    fn process_vt_bytes(&mut self, bytes: &[u8]) {
        let mut data = if self.vt_query_tail.is_empty() {
            bytes.to_vec()
        } else {
            let mut combined = std::mem::take(&mut self.vt_query_tail);
            combined.extend_from_slice(bytes);
            combined
        };
        // `vt100` 0.15 handles CSI `H` (CUP) but ignores its alias CSI `f` (HVP),
        // which CLIs like Go's `survey` (`gh issue create`) emit as `\x1b[999;999f`
        // right before `\x1b[6n` to probe terminal dimensions.
        normalize_hvp_to_cup(&mut data);

        let mut i = 0;
        let mut flushed = 0;
        while i < data.len() {
            if data[i] == 0x1b {
                match match_terminal_query(&data[i..]) {
                    QueryMatch::Complete { len, kind } => {
                        let end = i + len;
                        let reply = if let Some(ref mut parser) = self.vt_parser {
                            parser.process(&data[flushed..end]);
                            build_terminal_query_reply(kind, parser.screen().cursor_position())
                        } else {
                            None
                        };
                        if let (Some(reply_bytes), Some(ref tx)) = (reply, &self.stdin_tx) {
                            let _ = tx.send(reply_bytes);
                        }
                        i = end;
                        flushed = end;
                        continue;
                    }
                    QueryMatch::Incomplete => {
                        if flushed < i {
                            if let Some(ref mut parser) = self.vt_parser {
                                parser.process(&data[flushed..i]);
                            }
                        }
                        self.vt_query_tail = data[i..].to_vec();
                        return;
                    }
                    QueryMatch::NoMatch => {}
                }
            }
            i += 1;
        }

        if flushed < data.len() {
            if let Some(ref mut parser) = self.vt_parser {
                parser.process(&data[flushed..]);
            }
        }
    }

    /// Resets VT100 scrollback to live bottom view when the user sends input.
    fn reset_vt_scrollback(&mut self) {
        self.auto_scroll = true;
        if self.vt_scrollback != 0 {
            self.vt_scrollback = 0;
            if let Some(ref mut parser) = self.vt_parser {
                parser.set_scrollback(0);
            }
        }
    }

    /// Adjusts VT100 scrollback (or line-mode `scroll_offset`) up by `delta` lines.
    fn scroll_up(&mut self, delta: usize) {
        self.auto_scroll = false;
        if let Some(ref mut parser) = self.vt_parser {
            self.vt_scrollback = self.vt_scrollback.saturating_add(delta);
            parser.set_scrollback(self.vt_scrollback);
            self.vt_scrollback = parser.screen().scrollback();
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(delta);
        }
    }

    /// Adjusts VT100 scrollback (or line-mode `scroll_offset`) down by `delta` lines.
    fn scroll_down(&mut self, delta: usize) {
        if let Some(ref mut parser) = self.vt_parser {
            self.vt_scrollback = self.vt_scrollback.saturating_sub(delta);
            parser.set_scrollback(self.vt_scrollback);
            self.vt_scrollback = parser.screen().scrollback();
            if self.vt_scrollback == 0 {
                self.auto_scroll = true;
            }
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(delta);
        }
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
        #[cfg(unix)]
        {
            self.pty_resize_handle = None;
        }
    }

    /// Sends raw bytes to the running child's stdin channel.
    fn send_input_bytes(&mut self, bytes: Vec<u8>) {
        self.reset_vt_scrollback();
        if let Some(ref tx) = self.stdin_tx {
            let _ = tx.send(bytes);
        }
    }

    /// Handles keyboard events
    pub fn handle_key(&mut self, key: KeyEvent) -> StreamOutputAction {
        let is_live_interactive = self.is_running && self.interactive;
        let app_cursor = self
            .vt_parser
            .as_ref()
            .is_some_and(|p| p.screen().application_cursor());

        match key.code {
            KeyCode::Esc => {
                self.terminate();
                StreamOutputAction::Close
            }
            KeyCode::Enter => {
                if !self.is_running {
                    StreamOutputAction::Close
                } else {
                    let nl: &[u8] = if self.uses_pty { b"\r" } else { b"\n" };
                    self.send_input_bytes(nl.to_vec());
                    StreamOutputAction::None
                }
            }
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.scroll_up(10);
                StreamOutputAction::None
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.scroll_down(10);
                StreamOutputAction::None
            }
            _ if is_live_interactive => {
                if let Some(bytes) = encode_terminal_key(key, app_cursor) {
                    self.send_input_bytes(bytes);
                }
                StreamOutputAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                StreamOutputAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                StreamOutputAction::None
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                StreamOutputAction::None
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
                StreamOutputAction::None
            }
            KeyCode::Home => {
                self.scroll_up(VT100_SCROLLBACK_LINES);
                StreamOutputAction::None
            }
            KeyCode::End => {
                self.reset_vt_scrollback();
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
                self.scroll_up(3);
                StreamOutputAction::None
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
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

    /// Renders the streaming modal output or embedded VT100 screen into `buf`
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let (dialog_w, dialog_h) = self.calculate_dialog_dimensions(area);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        if inner.width < 4 || inner.height < 2 {
            return;
        }

        let content_h = inner.height.saturating_sub(1) as usize;
        let avail_w = inner.width.saturating_sub(3) as usize; // reserve margin and scrollbar
        let default_text_style = styles.text.bg(styles.background);

        if let Some(ref parser) = self.vt_parser {
            // 1a. Render embedded VT100 terminal screen with full cell styling & cursor
            let screen = parser.screen();
            let (screen_rows, screen_cols) = screen.size();
            let render_rows = content_h.min(screen_rows as usize);
            let render_cols = avail_w.min(screen_cols as usize);

            for r in 0..render_rows {
                let y = inner.y + r as u16;
                for c in 0..render_cols {
                    let Some(cell) = screen.cell(r as u16, c as u16) else {
                        continue;
                    };
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    let fg = vt100_color_to_ratatui(cell.fgcolor())
                        .or(default_text_style.fg)
                        .unwrap_or(Color::White);
                    let bg = vt100_color_to_ratatui(cell.bgcolor()).unwrap_or(styles.background);
                    let mut st = Style::default().fg(fg).bg(bg);
                    if cell.bold() {
                        st = st.add_modifier(Modifier::BOLD);
                    }
                    if cell.italic() {
                        st = st.add_modifier(Modifier::ITALIC);
                    }
                    if cell.underline() {
                        st = st.add_modifier(Modifier::UNDERLINED);
                    }
                    if cell.inverse() {
                        st = st.add_modifier(Modifier::REVERSED);
                    }
                    let sym = cell.contents();
                    let target = &mut buf[(inner.x + 1 + c as u16, y)];
                    if sym.is_empty() {
                        target.set_char(' ');
                    } else {
                        target.set_symbol(&sym);
                    }
                    target.set_style(st);
                }
            }

            // Render visible block cursor at the exact VT100 cursor position when live
            if self.is_running && self.vt_scrollback == 0 && !screen.hide_cursor() {
                let (cur_r, cur_c) = screen.cursor_position();
                if (cur_r as usize) < content_h && (cur_c as usize) < avail_w {
                    let cur_cell = &mut buf[(inner.x + 1 + cur_c, inner.y + cur_r)];
                    cur_cell.modifier.toggle(Modifier::REVERSED);
                }
            }
        } else {
            // 1b. Render line-buffered streaming output
            let total_lines = self.lines.len();
            let max_scroll = total_lines.saturating_sub(content_h);

            let eff_scroll = if self.auto_scroll {
                max_scroll
            } else {
                self.scroll_offset.min(max_scroll)
            };

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

/// Recognized VT100/xterm terminal queries emitted by interactive CLIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalQuery {
    /// CSI `6n` or CSI `?6n` — Cursor Position Report (DSR / CPR)
    CursorPosition { private: bool },
    /// CSI `5n` — Device Status Report (Operating Status)
    DeviceStatus,
    /// CSI `c` / `0c` — Primary Device Attributes (DA1)
    PrimaryDeviceAttrs,
    /// CSI `>c` / `>0c` — Secondary Device Attributes (DA2)
    SecondaryDeviceAttrs,
    /// OSC `10;?` — Default foreground color query
    OscForeground { bel: bool },
    /// OSC `11;?` — Default background color query
    OscBackground { bel: bool },
}

/// Result of matching a slice starting at `0x1b` against known terminal queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryMatch {
    Complete { len: usize, kind: TerminalQuery },
    Incomplete,
    NoMatch,
}

/// Static table of terminal query escape sequences and their semantic kinds.
const TERMINAL_QUERIES: &[(&[u8], TerminalQuery)] = &[
    (b"\x1b[6n", TerminalQuery::CursorPosition { private: false }),
    (b"\x1b[?6n", TerminalQuery::CursorPosition { private: true }),
    (b"\x1b[5n", TerminalQuery::DeviceStatus),
    (b"\x1b[c", TerminalQuery::PrimaryDeviceAttrs),
    (b"\x1b[0c", TerminalQuery::PrimaryDeviceAttrs),
    (b"\x1b[>c", TerminalQuery::SecondaryDeviceAttrs),
    (b"\x1b[>0c", TerminalQuery::SecondaryDeviceAttrs),
    (b"\x1b]10;?\x07", TerminalQuery::OscForeground { bel: true }),
    (
        b"\x1b]10;?\x1b\\",
        TerminalQuery::OscForeground { bel: false },
    ),
    (b"\x1b]11;?\x07", TerminalQuery::OscBackground { bel: true }),
    (
        b"\x1b]11;?\x1b\\",
        TerminalQuery::OscBackground { bel: false },
    ),
];

/// Matches the start of `slice` (which begins with `0x1b`) against known
/// terminal queries, distinguishing complete matches from truncated prefixes.
fn match_terminal_query(slice: &[u8]) -> QueryMatch {
    for &(seq, kind) in TERMINAL_QUERIES {
        if slice.starts_with(seq) {
            return QueryMatch::Complete {
                len: seq.len(),
                kind,
            };
        }
    }
    for &(seq, _) in TERMINAL_QUERIES {
        if seq.starts_with(slice) {
            return QueryMatch::Incomplete;
        }
    }
    QueryMatch::NoMatch
}

/// Builds the VT100/xterm reply bytes for a recognized terminal query.
fn build_terminal_query_reply(kind: TerminalQuery, cursor_pos: (u16, u16)) -> Option<Vec<u8>> {
    let (row, col) = cursor_pos;
    match kind {
        TerminalQuery::CursorPosition { private: false } => {
            Some(format!("\x1b[{};{}R", row + 1, col + 1).into_bytes())
        }
        TerminalQuery::CursorPosition { private: true } => {
            Some(format!("\x1b[?{};{}R", row + 1, col + 1).into_bytes())
        }
        TerminalQuery::DeviceStatus => Some(b"\x1b[0n".to_vec()),
        TerminalQuery::PrimaryDeviceAttrs => Some(b"\x1b[?62;22c".to_vec()),
        TerminalQuery::SecondaryDeviceAttrs => Some(b"\x1b[>0;276;0c".to_vec()),
        TerminalQuery::OscForeground { bel: true } => {
            Some(b"\x1b]10;rgb:ffff/ffff/ffff\x07".to_vec())
        }
        TerminalQuery::OscForeground { bel: false } => {
            Some(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\".to_vec())
        }
        TerminalQuery::OscBackground { bel: true } => {
            Some(b"\x1b]11;rgb:0000/0000/0000\x07".to_vec())
        }
        TerminalQuery::OscBackground { bel: false } => {
            Some(b"\x1b]11;rgb:0000/0000/0000\x1b\\".to_vec())
        }
    }
}

/// Rewrites ECMA-48 HVP (`CSI <digits/;> f`) in-place to its exact alias CUP
/// (`CSI <digits/;> H`) so `vt100::Parser` moves the cursor accurately.
fn normalize_hvp_to_cup(data: &mut [u8]) {
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0x1b && data[i + 1] == b'[' {
            let mut j = i + 2;
            while j < data.len() && (data[j].is_ascii_digit() || data[j] == b';') {
                j += 1;
            }
            if j < data.len() && data[j] == b'f' {
                data[j] = b'H';
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
}

/// Converts a `vt100::Color` to an optional Ratatui `Color` (`None` = inherit default).
fn vt100_color_to_ratatui(c: vt100::Color) -> Option<Color> {
    match c {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(Color::Indexed(i)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

/// Encodes a Crossterm `KeyEvent` into VT100/xterm bytes for forwarding to a PTY.
fn encode_terminal_key(key: KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Up => Some(if app_cursor { b"\x1bOA" } else { b"\x1b[A" }.to_vec()),
        KeyCode::Down => Some(if app_cursor { b"\x1bOB" } else { b"\x1b[B" }.to_vec()),
        KeyCode::Right => Some(if app_cursor { b"\x1bOC" } else { b"\x1b[C" }.to_vec()),
        KeyCode::Left => Some(if app_cursor { b"\x1bOD" } else { b"\x1b[D" }.to_vec()),
        KeyCode::Home => Some(if app_cursor { b"\x1bOH" } else { b"\x1b[H" }.to_vec()),
        KeyCode::End => Some(if app_cursor { b"\x1bOF" } else { b"\x1b[F" }.to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let lower = c.to_ascii_lowercase();
                if lower.is_ascii_lowercase() {
                    return Some(vec![(lower as u8) - b'a' + 1]);
                }
                match c {
                    ' ' | '@' => Some(vec![0x00]),
                    '[' => Some(vec![0x1b]),
                    '\\' => Some(vec![0x1c]),
                    ']' => Some(vec![0x1d]),
                    '^' => Some(vec![0x1e]),
                    '_' => Some(vec![0x1f]),
                    _ => None,
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                let mut out = vec![0x1b];
                let mut b = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                Some(out)
            } else {
                let mut b = [0u8; 4];
                Some(c.encode_utf8(&mut b).as_bytes().to_vec())
            }
        }
        _ => None,
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
