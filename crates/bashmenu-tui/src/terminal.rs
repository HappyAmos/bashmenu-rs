//! Terminal lifecycle management, raw mode setup, and subprocess suspension.

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::{Frame, Terminal};
use std::io::{self, BufWriter, Stdout, Write};
use std::time::Duration;

/// RAII guard managing terminal state (raw mode, alternate screen, mouse events).
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<BufWriter<Stdout>>>,
    keep_alt_on_drop: bool,
}

impl TerminalGuard {
    /// Initializes terminal in raw mode, on alternate screen with mouse capture.
    pub fn init() -> io::Result<Self> {
        let keep_alt_on_drop = std::env::var("BASHMENU_CHILD_ALT").as_deref() == Ok("1");
        enable_raw_mode()?;
        let mut stdout = BufWriter::new(io::stdout());
        if keep_alt_on_drop {
            execute!(stdout, EnableMouseCapture, Hide)?;
        } else {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let mut guard = Self {
            terminal,
            keep_alt_on_drop,
        };
        if keep_alt_on_drop {
            guard.request_clear();
        }
        Ok(guard)
    }

    /// Access mutable reference to underlying Ratatui terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<BufWriter<Stdout>>> {
        &mut self.terminal
    }

    /// Invalidates the Ratatui previous frame buffer without sending `\x1b[2J` so the next
    /// `draw()` overwrites every cell atomically without a blank screen flash.
    pub fn request_clear(&mut self) {
        let _ = self.terminal.autoresize();
        for cell in self.terminal.current_buffer_mut().content.iter_mut() {
            cell.set_symbol("\u{ffff}");
        }
        self.terminal.swap_buffers();
    }

    /// Renders a frame wrapped in DEC Mode 2026 Synchronized Output (`\x1b[?2026h` ... `\x1b[?2026l`)
    /// to prevent terminal tearing or flicker.
    pub fn draw<F>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026h\x1b[0m");
        let res = self.terminal.draw(render_callback).map(|_| ());
        let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026l");
        let _ = self.terminal.backend_mut().flush();
        res
    }

    fn drain_stale_events() {
        while event::poll(Duration::ZERO).unwrap_or(false) {
            let _ = event::read();
        }
    }

    /// Suspends terminal raw mode and mouse capture while remaining on the alternate screen.
    /// Used when launching child TUIs (`bashedit`, `menuedit`, `themeedit`, `bmpager`) or
    /// alternate-buffer console commands so the terminal never flashes the normal shell screen.
    pub fn suspend_keep_alt<F, R>(&mut self, action: F) -> io::Result<R>
    where
        F: FnOnce() -> io::Result<R>,
    {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), DisableMouseCapture, Show)?;
        let _ = self.terminal.backend_mut().flush();

        let prev_env = std::env::var("BASHMENU_CHILD_ALT").ok();
        std::env::set_var("BASHMENU_CHILD_ALT", "1");

        let result = action();

        match prev_env {
            Some(v) => std::env::set_var("BASHMENU_CHILD_ALT", v),
            None => std::env::remove_var("BASHMENU_CHILD_ALT"),
        }

        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            Hide
        )?;
        Self::drain_stale_events();
        self.request_clear();

        result
    }

    /// Suspends terminal raw mode and alternate screen, executes an external closure
    /// (e.g. running an interactive shell command on the main screen), and restores terminal state.
    pub fn suspend<F, R>(&mut self, action: F) -> io::Result<R>
    where
        F: FnOnce() -> io::Result<R>,
    {
        // 1. Leave alternate screen and restore normal terminal state
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            Show
        )?;
        let _ = self.terminal.backend_mut().flush();

        // 2. Execute closure
        let result = action();

        // 3. Re-enter alternate screen and raw mode
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            Hide
        )?;
        Self::drain_stale_events();
        self.request_clear();

        result
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if self.keep_alt_on_drop {
            let _ = execute!(self.terminal.backend_mut(), DisableMouseCapture);
        } else {
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                Show
            );
        }
        let _ = self.terminal.backend_mut().flush();
    }
}
