//! Terminal lifecycle management, raw mode setup, and subprocess suspension.

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Modifier};
use ratatui::{Frame, Terminal};
use std::io::{self, BufWriter, Stdout, Write};
use std::time::Duration;

use crate::transition::{dim_color, TransitionEffect};

/// RAII guard managing terminal state (raw mode, alternate screen, mouse).
pub struct TerminalGuard {
    /// Underlying ratatui terminal instance wrapping buffered stdout
    terminal: Terminal<CrosstermBackend<BufWriter<Stdout>>>,
    /// When true, drop keeps alternate screen buffer (used for child TUIs)
    keep_alt_on_drop: bool,
}

impl TerminalGuard {
    /// Initializes terminal in raw mode, on alternate screen with mouse.
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

    /// Invalidates previous frame buffer without `\x1b[2J` so the next
    /// `draw()` overwrites every cell atomically without a blank flash.
    pub fn request_clear(&mut self) {
        let _ = self.terminal.autoresize();
        for cell in self.terminal.current_buffer_mut().content.iter_mut() {
            cell.set_symbol("\u{ffff}");
        }
        self.terminal.swap_buffers();
    }

    /// Renders a frame wrapped in DEC Mode 2026 Synchronized Output
    /// (`\x1b[?2026h` ... `\x1b[?2026l`) to prevent tearing or flicker.
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

    /// Executes a screen transition effect when restoring the menu after an
    /// external command.
    pub fn draw_with_transition<F>(
        &mut self,
        effect: TransitionEffect,
        mut render_callback: F,
    ) -> io::Result<()>
    where
        F: FnMut(&mut Frame<'_>),
    {
        match effect {
            TransitionEffect::None | TransitionEffect::Instant => self.draw(|f| render_callback(f)),
            TransitionEffect::Fade => {
                let size = self.terminal.size()?;
                for step in 1..=3 {
                    let factor = (step as f32) / 3.0;
                    let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026h\x1b[0m");
                    let _ = self.terminal.draw(|f| {
                        render_callback(f);
                        if step < 3 {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.fg = dim_color(cell.fg, factor);
                                    cell.bg = dim_color(cell.bg, factor);
                                }
                            }
                        }
                    });
                    let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026l");
                    let _ = self.terminal.backend_mut().flush();
                    if step < 3 {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
                Ok(())
            }
            TransitionEffect::Curtain => {
                let size = self.terminal.size()?;
                for step in 1..=3 {
                    let fraction = (step as f32) / 3.0;
                    let visible_width = ((size.width as f32) * fraction).round() as u16;
                    let left_bound = (size.width.saturating_sub(visible_width)) / 2;
                    let right_bound = left_bound + visible_width;
                    let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026h\x1b[0m");
                    let _ = self.terminal.draw(|f| {
                        render_callback(f);
                        if step < 3 {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    if x < left_bound || x >= right_bound {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Reset);
                                        cell.set_bg(Color::Reset);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026l");
                    let _ = self.terminal.backend_mut().flush();
                    if step < 3 {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                Ok(())
            }
        }
    }

    /// Discards any buffered or stale terminal input events from queue.
    ///
    /// This prevents pending keystrokes or mouse events accumulated during
    /// long operations from unexpectedly triggering actions upon resumption.
    fn drain_stale_events() {
        while event::poll(Duration::ZERO).unwrap_or(false) {
            let _ = event::read();
        }
    }

    /// Suspends terminal raw mode while remaining on alternate screen.
    /// Used when launching child TUIs (`bashedit`, `menuedit`, `themeedit`)
    /// so the terminal never flashes the normal shell screen.
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

    /// Suspends terminal raw mode and alternate screen, executes closure
    /// (e.g. running interactive shell command), and restores terminal.
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
    /// Restores terminal raw mode, cursor visibility, and alternate screen.
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
