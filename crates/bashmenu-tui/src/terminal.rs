//! Terminal lifecycle management, raw mode setup, and subprocess suspension.

use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, BufWriter, Stdout};

/// RAII guard managing terminal state (raw mode, alternate screen, mouse events).
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<BufWriter<Stdout>>>,
}

impl TerminalGuard {
    /// Initializes terminal in raw mode, on alternate screen with mouse capture.
    pub fn init() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = BufWriter::new(io::stdout());
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Access mutable reference to underlying Ratatui terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<BufWriter<Stdout>>> {
        &mut self.terminal
    }

    /// Suspends terminal raw mode and alternate screen, executes an external closure
    /// (e.g. running an interactive shell command or editor), and restores terminal state.
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
        self.terminal.clear()?;

        result
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            Show
        );
    }
}
