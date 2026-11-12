//! Terminal lifecycle management, raw mode setup, and subprocess suspension.

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::{Frame, Terminal};
use std::io::{self, BufWriter, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::framebuffer::FramebufferDevice;
use crate::modals::render_animated_modal_frame;
use crate::style::ResolvedThemeStyles;
use crate::transition::{
    dim_color, interpolate_zoom_rect, TransitionEffect, TransitionSpeed, BAYER_4X4,
};

/// Buffered terminal stdout wrapper that can suppress ANSI cell writes when
/// direct Linux framebuffer (`/dev/fb0`) rendering is active.
pub struct TerminalOutput {
    stdout: BufWriter<Stdout>,
    suppress_stdout: Arc<AtomicBool>,
}

impl TerminalOutput {
    /// Creates a new `TerminalOutput` wrapping standard output.
    pub fn new(stdout: BufWriter<Stdout>, suppress_stdout: Arc<AtomicBool>) -> Self {
        Self {
            stdout,
            suppress_stdout,
        }
    }
}

impl Write for TerminalOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.suppress_stdout.load(Ordering::Relaxed) {
            Ok(buf.len())
        } else {
            self.stdout.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.suppress_stdout.load(Ordering::Relaxed) {
            Ok(())
        } else {
            self.stdout.flush()
        }
    }
}

/// RAII guard managing terminal state (raw mode, alternate screen, mouse).
pub struct TerminalGuard {
    /// Underlying ratatui terminal instance wrapping buffered stdout
    terminal: Terminal<CrosstermBackend<TerminalOutput>>,
    /// Atomic flag suppressing stdout writes while framebuffer rendering is active
    suppress_stdout: Arc<AtomicBool>,
    /// Optional direct Linux framebuffer (`/dev/fb0`) renderer
    fb_device: Option<FramebufferDevice>,
    /// Default theme background color used by framebuffer compositor
    default_bg: Color,
    /// When true, drop keeps alternate screen buffer (used for child TUIs)
    keep_alt_on_drop: bool,
}

impl TerminalGuard {
    /// Initializes terminal in raw mode, on alternate screen with mouse,
    /// automatically attaching `/dev/fb0` when running on a Linux virtual console.
    pub fn init() -> io::Result<Self> {
        let fb = if std::env::var("BASHMENU_FB_DISABLE").as_deref() != Ok("1")
            && crate::framebuffer::is_linux_console()
            && crate::framebuffer::is_framebuffer_available()
        {
            let opened = FramebufferDevice::open();
            if opened.is_some() && std::env::var("BASHMENU_COLOR_TIER").is_err() {
                std::env::set_var("BASHMENU_COLOR_TIER", "truecolor");
            }
            opened
        } else {
            None
        };
        Self::init_with_framebuffer(fb)
    }

    /// Initializes terminal in raw mode with an explicitly provided optional
    /// [`FramebufferDevice`].
    pub fn init_with_framebuffer(fb_device: Option<FramebufferDevice>) -> io::Result<Self> {
        let keep_alt_on_drop = std::env::var("BASHMENU_CHILD_ALT").as_deref() == Ok("1");
        enable_raw_mode()?;
        let mut stdout = BufWriter::new(io::stdout());
        if keep_alt_on_drop {
            execute!(stdout, EnableMouseCapture, Hide)?;
        } else {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        }
        let _ = stdout.flush();
        let suppress_stdout = Arc::new(AtomicBool::new(false));
        let backend = CrosstermBackend::new(TerminalOutput::new(stdout, suppress_stdout.clone()));
        let terminal = Terminal::new(backend)?;
        let mut guard = Self {
            terminal,
            suppress_stdout,
            fb_device: None,
            default_bg: Color::Black,
            keep_alt_on_drop,
        };
        if keep_alt_on_drop {
            guard.request_clear();
        }
        if fb_device.is_some() {
            guard.set_framebuffer(fb_device);
        }
        Ok(guard)
    }

    /// Attaches or detaches a direct Linux framebuffer (`/dev/fb0`) device.
    ///
    /// When attached, ANSI cell writes to `stdout` are suppressed so the
    /// kernel's 8-color `fbcon` driver does not overwrite TrueColor `/dev/fb0`
    /// output.
    pub fn set_framebuffer(&mut self, fb: Option<FramebufferDevice>) {
        let active = fb.is_some();
        self.suppress_stdout.store(false, Ordering::Relaxed);
        if active {
            let _ = self
                .terminal
                .backend_mut()
                .write_all(b"\x1b[?25l\x1b[?1c\x1b[0m\x1b[2J\x1b[H");
        } else if self.fb_device.is_some() {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?0c");
        }
        let _ = self.terminal.backend_mut().flush();
        self.fb_device = fb;
        self.suppress_stdout.store(active, Ordering::Relaxed);
    }

    /// Returns a mutable reference to the attached framebuffer device, if any.
    pub fn framebuffer_mut(&mut self) -> Option<&mut FramebufferDevice> {
        self.fb_device.as_mut()
    }

    /// Returns whether a direct Linux framebuffer device is currently active.
    pub fn has_framebuffer(&self) -> bool {
        self.fb_device.is_some()
    }

    /// Updates the default theme background color used by framebuffer rendering.
    pub fn set_default_bg(&mut self, bg: Color) {
        self.default_bg = bg;
    }

    /// Access mutable reference to underlying Ratatui terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<TerminalOutput>> {
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
    /// When a Linux framebuffer device is attached, composites the Ratatui
    /// buffer directly into `/dev/fb0` in 24-bit TrueColor.
    pub fn draw<F>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        let has_fb = self.fb_device.is_some();
        if !has_fb {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026h\x1b[0m");
        }
        let fb_opt = self.fb_device.as_mut();
        let default_bg = self.default_bg;
        let res = self
            .terminal
            .draw(|f| {
                render_callback(f);
                if let Some(fb) = fb_opt {
                    fb.render_buffer(f.buffer_mut(), default_bg);
                }
            })
            .map(|_| ());
        if !has_fb {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?2026l");
            let _ = self.terminal.backend_mut().flush();
        }
        res
    }

    /// Executes a screen transition effect when restoring the menu after an
    /// external command.
    pub fn draw_with_transition<F>(
        &mut self,
        effect: TransitionEffect,
        speed: TransitionSpeed,
        mut render_callback: F,
    ) -> io::Result<()>
    where
        F: FnMut(&mut Frame<'_>),
    {
        if effect == TransitionEffect::None || effect == TransitionEffect::Instant {
            return self.draw(|f| render_callback(f));
        }
        let size = self.terminal.size()?;
        if size.width == 0 || size.height == 0 {
            return self.draw(|f| render_callback(f));
        }

        match effect {
            TransitionEffect::None | TransitionEffect::Instant => self.draw(|f| render_callback(f)),
            TransitionEffect::Fade => {
                let tier = if self.fb_device.is_some() {
                    crate::style::ColorTier::TrueColor
                } else {
                    crate::style::ColorTier::detect()
                };
                if tier == crate::style::ColorTier::Color8
                    || tier == crate::style::ColorTier::Color16
                {
                    let size = self.terminal.size()?;
                    let delay = speed.step_delay_in();
                    let steps = 6;
                    for step in 1..=steps {
                        let fraction = step as f32 / steps as f32;
                        let threshold = (fraction * 16.0).round() as u8;
                        let _ = self.draw(|f| {
                            render_callback(f);
                            if step < steps {
                                let buf = f.buffer_mut();
                                for y in 0..size.height {
                                    for x in 0..size.width {
                                        let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                                        if dither >= threshold {
                                            let cell = &mut buf[(x, y)];
                                            cell.set_char(' ');
                                            cell.set_fg(Color::Black);
                                            cell.set_bg(Color::Black);
                                            cell.modifier = Modifier::empty();
                                        }
                                    }
                                }
                            }
                        });
                        if step < steps {
                            std::thread::sleep(delay);
                        }
                    }
                    return Ok(());
                }

                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 5;
                for step in 1..=steps {
                    let factor = (step as f32) / (steps as f32);
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.fg = dim_color(cell.fg, factor);
                                    if cell.bg != Color::Reset {
                                        cell.bg = dim_color(cell.bg, factor);
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::Curtain => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let fraction = (step as f32) / (steps as f32);
                    let visible_width = ((size.width as f32) * fraction).round() as u16;
                    let left_bound = (size.width.saturating_sub(visible_width)) / 2;
                    let right_bound = left_bound + visible_width;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    if x < left_bound || x >= right_bound {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::Dissolve => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let threshold = (fraction * 16.0).round() as u8;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                                    if dither >= threshold {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::Matrix => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            let fraction = step as f32 / steps as f32;
                            for x in 0..size.width {
                                let col_offset =
                                    ((x as u32).wrapping_mul(37).wrapping_add(11) % 5) as i32;
                                let lead_y =
                                    ((size.height as f32 + 4.0) * fraction) as i32 - col_offset;
                                for y in 0..size.height {
                                    let yi = y as i32;
                                    if yi > lead_y {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    } else if yi == lead_y || yi == lead_y - 1 {
                                        let cell = &mut buf[(x, y)];
                                        let seed = (x as u32)
                                            .wrapping_mul(1103515245)
                                            .wrapping_add((y as u32).wrapping_mul(12345))
                                            .wrapping_add(step as u32);
                                        let glyph =
                                            b"0123456789ABCDEF@#$%&*"[(seed % 22) as usize] as char;
                                        cell.set_char(glyph);
                                        if yi == lead_y {
                                            cell.set_fg(Color::LightGreen);
                                        } else {
                                            cell.set_fg(Color::Green);
                                        }
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::BOLD;
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::SlideLeft => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let offset = ((1.0 - fraction) * size.width as f32).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps && offset > 0 {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in (0..size.width).rev() {
                                    if x >= offset {
                                        let src = buf[(x - offset, y)].clone();
                                        buf[(x, y)] = src;
                                    } else {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::SlideRight => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let offset = ((1.0 - fraction) * size.width as f32).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps && offset > 0 {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    if x + offset < size.width {
                                        let src = buf[(x + offset, y)].clone();
                                        buf[(x, y)] = src;
                                    } else {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::Blinds => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                let slat_size = 4u16;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let visible_per_slat = (slat_size as f32 * fraction).ceil() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                if (y % slat_size) >= visible_per_slat {
                                    for x in 0..size.width {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::Iris => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                let cx = size.width / 2;
                let cy = size.height / 2;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let rx = ((size.width as f32 / 2.0) * fraction).round() as u16;
                    let ry = ((size.height as f32 / 2.0) * fraction).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                let dy = y.abs_diff(cy);
                                for x in 0..size.width {
                                    let dx = x.abs_diff(cx);
                                    if dx > rx || dy > ry {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
            TransitionEffect::WipeDown => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_in();
                let steps = 6;
                for step in 1..=steps {
                    let fraction = step as f32 / steps as f32;
                    let visible_height = ((size.height as f32) * fraction).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        if step < steps {
                            let buf = f.buffer_mut();
                            for y in visible_height..size.height {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    if step < steps {
                        std::thread::sleep(delay);
                    }
                }
                Ok(())
            }
        }
    }

    /// Draws a transition-out sequence (ramp-down/curtain close) before
    /// switching to an external application or process.
    pub fn draw_with_transition_out<F>(
        &mut self,
        effect: TransitionEffect,
        speed: TransitionSpeed,
        mut render_callback: F,
    ) -> io::Result<()>
    where
        F: FnMut(&mut Frame<'_>),
    {
        if effect == TransitionEffect::None || effect == TransitionEffect::Instant {
            return Ok(());
        }
        let size = self.terminal.size()?;
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        match effect {
            TransitionEffect::None | TransitionEffect::Instant => Ok(()),
            TransitionEffect::Fade => {
                let tier = if self.fb_device.is_some() {
                    crate::style::ColorTier::TrueColor
                } else {
                    crate::style::ColorTier::detect()
                };
                if tier == crate::style::ColorTier::Color8
                    || tier == crate::style::ColorTier::Color16
                {
                    let size = self.terminal.size()?;
                    let delay = speed.step_delay_out();
                    let steps = 6;
                    for step in (0..steps).rev() {
                        let fraction = step as f32 / steps as f32;
                        let threshold = (fraction * 16.0).round() as u8;
                        let _ = self.draw(|f| {
                            render_callback(f);
                            let buf = f.buffer_mut();
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                                    if step == 0 || dither >= threshold {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    }
                                }
                            }
                        });
                        std::thread::sleep(delay);
                    }
                    return Ok(());
                }

                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 5;
                for step in (0..steps).rev() {
                    let factor = (step as f32) / (steps as f32);
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            for x in 0..size.width {
                                let cell = &mut buf[(x, y)];
                                if step == 0 {
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                } else {
                                    cell.fg = dim_color(cell.fg, factor);
                                    if cell.bg != Color::Reset {
                                        cell.bg = dim_color(cell.bg, factor);
                                    }
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::Curtain => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let fraction = (step as f32) / (steps as f32);
                    let visible_width = ((size.width as f32) * fraction).round() as u16;
                    let left_bound = (size.width.saturating_sub(visible_width)) / 2;
                    let right_bound = left_bound + visible_width;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            for x in 0..size.width {
                                if step == 0 || x < left_bound || x >= right_bound {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::Dissolve => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let fraction = step as f32 / steps as f32;
                    let threshold = (fraction * 16.0).round() as u8;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            for x in 0..size.width {
                                let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                                if step == 0 || dither >= threshold {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::Matrix => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        if step == 0 {
                            for y in 0..size.height {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        } else {
                            let fraction = (steps - step) as f32 / steps as f32;
                            for x in 0..size.width {
                                let col_offset =
                                    ((x as u32).wrapping_mul(37).wrapping_add(11) % 5) as i32;
                                let lead_y =
                                    ((size.height as f32 + 4.0) * fraction) as i32 - col_offset;
                                for y in 0..size.height {
                                    let yi = y as i32;
                                    if yi < lead_y - 1 {
                                        let cell = &mut buf[(x, y)];
                                        cell.set_char(' ');
                                        cell.set_fg(Color::Black);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::empty();
                                    } else if yi == lead_y || yi == lead_y - 1 {
                                        let cell = &mut buf[(x, y)];
                                        let seed = (x as u32)
                                            .wrapping_mul(1103515245)
                                            .wrapping_add((y as u32).wrapping_mul(12345))
                                            .wrapping_add(step as u32);
                                        let glyph =
                                            b"0123456789ABCDEF@#$%&*"[(seed % 22) as usize] as char;
                                        cell.set_char(glyph);
                                        cell.set_fg(Color::LightGreen);
                                        cell.set_bg(Color::Black);
                                        cell.modifier = Modifier::BOLD;
                                    }
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::SlideLeft => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let shift =
                        (((steps - step) as f32 / steps as f32) * size.width as f32).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            for x in 0..size.width {
                                if step == 0 || x + shift >= size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                } else {
                                    let src = buf[(x + shift, y)].clone();
                                    buf[(x, y)] = src;
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::SlideRight => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let shift =
                        (((steps - step) as f32 / steps as f32) * size.width as f32).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            for x in (0..size.width).rev() {
                                if step == 0 || x < shift {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                } else {
                                    let src = buf[(x - shift, y)].clone();
                                    buf[(x, y)] = src;
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::Blinds => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                let slat_size = 4u16;
                for step in (0..steps).rev() {
                    let fraction = step as f32 / steps as f32;
                    let visible_per_slat = (slat_size as f32 * fraction).floor() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            if step == 0 || (y % slat_size) >= visible_per_slat {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::Iris => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                let cx = size.width / 2;
                let cy = size.height / 2;
                for step in (0..steps).rev() {
                    let fraction = step as f32 / steps as f32;
                    let rx = ((size.width as f32 / 2.0) * fraction).round() as u16;
                    let ry = ((size.height as f32 / 2.0) * fraction).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            let dy = y.abs_diff(cy);
                            for x in 0..size.width {
                                let dx = x.abs_diff(cx);
                                if step == 0 || dx > rx || dy > ry {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
                }
                Ok(())
            }
            TransitionEffect::WipeDown => {
                let size = self.terminal.size()?;
                let delay = speed.step_delay_out();
                let steps = 6;
                for step in (0..steps).rev() {
                    let fraction = step as f32 / steps as f32;
                    let visible_height = ((size.height as f32) * fraction).round() as u16;
                    let _ = self.draw(|f| {
                        render_callback(f);
                        let buf = f.buffer_mut();
                        for y in 0..size.height {
                            if step == 0 || y >= visible_height {
                                for x in 0..size.width {
                                    let cell = &mut buf[(x, y)];
                                    cell.set_char(' ');
                                    cell.set_fg(Color::Black);
                                    cell.set_bg(Color::Black);
                                    cell.modifier = Modifier::empty();
                                }
                            }
                        }
                    });
                    std::thread::sleep(delay);
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
        let had_fb = self.fb_device.is_some();
        if let Some(ref mut fb) = self.fb_device {
            fb.clear(0, 0, 0);
        }
        self.suppress_stdout.store(false, Ordering::Relaxed);
        if had_fb {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?0c");
        }

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

        ensure_terminal_sane();
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            Hide
        )?;
        if had_fb {
            let _ = self
                .terminal
                .backend_mut()
                .write_all(b"\x1b[?25l\x1b[?1c\x1b[0m\x1b[2J\x1b[H");
        }
        let _ = self.terminal.backend_mut().flush();
        self.suppress_stdout.store(had_fb, Ordering::Relaxed);
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
        let had_fb = self.fb_device.is_some();
        if let Some(ref mut fb) = self.fb_device {
            fb.clear(0, 0, 0);
        }
        self.suppress_stdout.store(false, Ordering::Relaxed);
        if had_fb {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?0c");
        }

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
        ensure_terminal_sane();
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            Hide
        )?;
        if had_fb {
            let _ = self
                .terminal
                .backend_mut()
                .write_all(b"\x1b[?25l\x1b[?1c\x1b[0m\x1b[2J\x1b[H");
        }
        let _ = self.terminal.backend_mut().flush();
        self.suppress_stdout.store(had_fb, Ordering::Relaxed);
        Self::drain_stale_events();
        self.request_clear();

        result
    }

    /// Executes a snappy modal zoom grow animation expanding from `anchor`
    /// to `target_rect` over the provided background.
    pub fn animate_modal_grow<F>(
        &mut self,
        anchor: (u16, u16),
        target_rect: Rect,
        styles: &ResolvedThemeStyles,
        speed: TransitionSpeed,
        mut render_background: F,
    ) -> io::Result<()>
    where
        F: FnMut(&mut Frame<'_>),
    {
        let size = self.terminal.size()?;
        if size.width == 0 || size.height == 0 || target_rect.width == 0 || target_rect.height == 0
        {
            return Ok(());
        }

        let viewport = Rect::new(0, 0, size.width, size.height);
        let delay = speed.step_delay_out();
        let factors = [0.25_f32, 0.50_f32, 0.75_f32];

        for factor in factors {
            let intermediate_rect = interpolate_zoom_rect(anchor, target_rect, factor, viewport);
            let _ = self.draw(|f| {
                render_background(f);
                render_animated_modal_frame(viewport, intermediate_rect, styles, f.buffer_mut());
            });
            std::thread::sleep(delay);
        }

        Ok(())
    }

    /// Executes a snappy modal zoom shrink animation collapsing from
    /// `source_rect` back to `anchor` over the provided background.
    pub fn animate_modal_shrink<F>(
        &mut self,
        anchor: (u16, u16),
        source_rect: Rect,
        styles: &ResolvedThemeStyles,
        speed: TransitionSpeed,
        mut render_background: F,
    ) -> io::Result<()>
    where
        F: FnMut(&mut Frame<'_>),
    {
        let size = self.terminal.size()?;
        if size.width == 0 || size.height == 0 || source_rect.width == 0 || source_rect.height == 0
        {
            return Ok(());
        }

        let viewport = Rect::new(0, 0, size.width, size.height);
        let delay = speed.step_delay_out();
        let factors = [0.70_f32, 0.40_f32, 0.15_f32];

        for factor in factors {
            let intermediate_rect = interpolate_zoom_rect(anchor, source_rect, factor, viewport);
            let _ = self.draw(|f| {
                render_background(f);
                render_animated_modal_frame(viewport, intermediate_rect, styles, f.buffer_mut());
            });
            std::thread::sleep(delay);
        }

        Ok(())
    }
}

impl Drop for TerminalGuard {
    /// Restores terminal raw mode, cursor visibility, and alternate screen.
    fn drop(&mut self) {
        let had_fb = self.fb_device.is_some();
        if let Some(ref mut fb) = self.fb_device {
            fb.clear(0, 0, 0);
        }
        self.suppress_stdout.store(false, Ordering::Relaxed);
        if had_fb {
            let _ = self.terminal.backend_mut().write_all(b"\x1b[?0c");
        }

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
        let _ = disable_raw_mode();

        if !self.keep_alt_on_drop {
            ensure_terminal_sane();
        }
    }
}

/// Restores canonical cooked mode, ensuring character echo, line-buffering,
/// signal processing, and newline translations are strictly enabled.
#[cfg(unix)]
pub fn ensure_terminal_sane() {
    let fds = [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO];
    for &fd in &fds {
        if unsafe { libc::isatty(fd) } == 1 {
            unsafe {
                let mut termios = std::mem::zeroed::<libc::termios>();
                if libc::tcgetattr(fd, &mut termios) == 0 {
                    termios.c_lflag |=
                        libc::ECHO | libc::ICANON | libc::ISIG | libc::ECHOE | libc::ECHOK;
                    termios.c_iflag |= libc::ICRNL;
                    termios.c_iflag &= !(libc::IGNCR | libc::INLCR);
                    termios.c_oflag |= libc::OPOST | libc::ONLCR;
                    let _ = libc::tcsetattr(fd, libc::TCSADRAIN, &termios);
                }
            }
        }
    }
    if let Ok(dev_tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    {
        use std::os::unix::io::AsRawFd;
        let fd = dev_tty.as_raw_fd();
        unsafe {
            let mut termios = std::mem::zeroed::<libc::termios>();
            if libc::tcgetattr(fd, &mut termios) == 0 {
                termios.c_lflag |=
                    libc::ECHO | libc::ICANON | libc::ISIG | libc::ECHOE | libc::ECHOK;
                termios.c_iflag |= libc::ICRNL;
                termios.c_iflag &= !(libc::IGNCR | libc::INLCR);
                termios.c_oflag |= libc::OPOST | libc::ONLCR;
                let _ = libc::tcsetattr(fd, libc::TCSADRAIN, &termios);
            }
        }
    }
}

/// No-op fallback for non-unix targets.
#[cfg(not(unix))]
pub fn ensure_terminal_sane() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_terminal_sane_callable() {
        ensure_terminal_sane();
    }
}
