//! Terminal lifecycle management, raw mode setup, and subprocess suspension.

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::{Frame, Terminal};
use std::io::{self, BufWriter, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::framebuffer::FramebufferDevice;
use crate::modals::render_animated_modal_frame;
use crate::style::{ColorTier, ResolvedThemeStyles};
use crate::transition::{
    dim_color_for_tier, interpolate_zoom_rect, TransitionEffect, TransitionPacer, TransitionSpeed,
    BAYER_4X4,
};

/// Capacity (64 KiB) for buffered stdout writes so full TrueColor frames flush
/// inside DEC Mode 2026 synchronized output in minimal `write()` syscalls.
const STDOUT_BUF_CAPACITY: usize = 64 * 1024;

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
        let mut stdout = BufWriter::with_capacity(STDOUT_BUF_CAPACITY, io::stdout());
        if keep_alt_on_drop {
            execute!(stdout, EnableBracketedPaste, EnableMouseCapture, Hide)?;
        } else {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableBracketedPaste,
                EnableMouseCapture,
                Hide
            )?;
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
    ///
    /// Renders `render_callback` once into an offscreen snapshot buffer and
    /// paces intermediate frames against wall-clock deadlines with an adaptive
    /// circuit-breaker that snaps to the final frame under heavy system load.
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

        let snapshot = capture_snapshot(size.width, size.height, |f| render_callback(f));
        let tier = if self.fb_device.is_some() {
            ColorTier::TrueColor
        } else {
            ColorTier::detect()
        };
        let steps = if effect == TransitionEffect::Fade
            && tier != ColorTier::Color8
            && tier != ColorTier::Color16
        {
            5
        } else {
            6
        };

        let mut pacer = TransitionPacer::new(speed.total_duration_in(), steps);
        while let Some((fraction, frame_idx, is_final)) = pacer.next_frame(Instant::now()) {
            let draw_start = Instant::now();
            let _ = self.draw(|f| {
                apply_transition_in_frame(
                    effect,
                    tier,
                    &snapshot,
                    f.buffer_mut(),
                    fraction,
                    frame_idx,
                    is_final,
                );
            });
            if is_final {
                break;
            }
            let sleep_dur = pacer.sleep_after_frame(draw_start.elapsed(), Instant::now());
            if !sleep_dur.is_zero() {
                std::thread::sleep(sleep_dur);
            }
        }
        Ok(())
    }

    /// Draws a transition-out sequence (ramp-down/curtain close) before
    /// switching to an external application or process.
    ///
    /// Renders `render_callback` once into an offscreen snapshot buffer and
    /// paces intermediate frames against wall-clock deadlines with an adaptive
    /// circuit-breaker.
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

        let snapshot = capture_snapshot(size.width, size.height, |f| render_callback(f));
        let tier = if self.fb_device.is_some() {
            ColorTier::TrueColor
        } else {
            ColorTier::detect()
        };
        let steps = if effect == TransitionEffect::Fade
            && tier != ColorTier::Color8
            && tier != ColorTier::Color16
        {
            5
        } else {
            6
        };

        let mut pacer = TransitionPacer::new(speed.total_duration_out(), steps);
        while let Some((progress, frame_idx, is_final)) = pacer.next_frame(Instant::now()) {
            let visible_fraction = (1.0 - progress).clamp(0.0, 1.0);
            let draw_start = Instant::now();
            let _ = self.draw(|f| {
                apply_transition_out_frame(
                    effect,
                    tier,
                    &snapshot,
                    f.buffer_mut(),
                    visible_fraction,
                    frame_idx,
                    is_final,
                );
            });
            if is_final {
                break;
            }
            let sleep_dur = pacer.sleep_after_frame(draw_start.elapsed(), Instant::now());
            if !sleep_dur.is_zero() {
                std::thread::sleep(sleep_dur);
            }
        }
        Ok(())
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
        execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            DisableMouseCapture,
            Show
        )?;
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
            EnableBracketedPaste,
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
            DisableBracketedPaste,
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
            EnableBracketedPaste,
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
    ///
    /// Snapshots `render_background` once and paces frames via [`TransitionPacer`].
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
        let bg_snapshot = capture_snapshot(size.width, size.height, |f| render_background(f));
        let mut pacer = TransitionPacer::new(speed.total_duration_out(), 4);

        while let Some((factor, _idx, is_final)) = pacer.next_frame(Instant::now()) {
            if is_final {
                break;
            }
            let intermediate_rect = interpolate_zoom_rect(anchor, target_rect, factor, viewport);
            let draw_start = Instant::now();
            let _ = self.draw(|f| {
                copy_snapshot_to_buffer(&bg_snapshot, f.buffer_mut());
                render_animated_modal_frame(viewport, intermediate_rect, styles, f.buffer_mut());
            });
            let sleep_dur = pacer.sleep_after_frame(draw_start.elapsed(), Instant::now());
            if pacer.circuit_breaker_tripped() {
                break;
            }
            if !sleep_dur.is_zero() {
                std::thread::sleep(sleep_dur);
            }
        }

        Ok(())
    }

    /// Executes a snappy modal zoom shrink animation collapsing from
    /// `source_rect` back to `anchor` over the provided background.
    ///
    /// Snapshots `render_background` once and paces frames via [`TransitionPacer`].
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
        let bg_snapshot = capture_snapshot(size.width, size.height, |f| render_background(f));
        let mut pacer = TransitionPacer::new(speed.total_duration_out(), 4);

        while let Some((progress, _idx, is_final)) = pacer.next_frame(Instant::now()) {
            if is_final {
                break;
            }
            let factor = (1.0 - progress).clamp(0.0, 1.0);
            let intermediate_rect = interpolate_zoom_rect(anchor, source_rect, factor, viewport);
            let draw_start = Instant::now();
            let _ = self.draw(|f| {
                copy_snapshot_to_buffer(&bg_snapshot, f.buffer_mut());
                render_animated_modal_frame(viewport, intermediate_rect, styles, f.buffer_mut());
            });
            let sleep_dur = pacer.sleep_after_frame(draw_start.elapsed(), Instant::now());
            if pacer.circuit_breaker_tripped() {
                break;
            }
            if !sleep_dur.is_zero() {
                std::thread::sleep(sleep_dur);
            }
        }

        Ok(())
    }
}

/// Captures a single offscreen frame into a [`Buffer`] via [`TestBackend`].
pub(crate) fn capture_snapshot<F>(width: u16, height: u16, render_callback: F) -> Buffer
where
    F: FnOnce(&mut Frame<'_>),
{
    let backend = TestBackend::new(width, height);
    let Ok(mut scratch) = Terminal::new(backend);
    let Ok(_) = scratch.draw(render_callback);
    scratch.backend().buffer().clone()
}

/// Copies a snapshot buffer into `dst`, handling rare mid-transition resizes
/// safely without panicking.
fn copy_snapshot_to_buffer(snapshot: &Buffer, dst: &mut Buffer) {
    if dst.area == snapshot.area {
        dst.content.clone_from_slice(&snapshot.content);
    } else {
        let max_w = dst.area.width.min(snapshot.area.width);
        let max_h = dst.area.height.min(snapshot.area.height);
        for y in 0..max_h {
            for x in 0..max_w {
                dst[(x, y)] = snapshot[(x, y)].clone();
            }
        }
    }
}

/// Blanks a single terminal cell to a black space with empty modifiers.
#[inline]
fn clear_cell(cell: &mut Cell) {
    cell.set_char(' ');
    cell.set_fg(Color::Black);
    cell.set_bg(Color::Black);
    cell.modifier = Modifier::empty();
}

/// Applies one transition-in frame from `snapshot` into `dst`.
pub(crate) fn apply_transition_in_frame(
    effect: TransitionEffect,
    tier: ColorTier,
    snapshot: &Buffer,
    dst: &mut Buffer,
    fraction: f32,
    frame_idx: usize,
    is_final: bool,
) {
    copy_snapshot_to_buffer(snapshot, dst);
    if is_final || effect == TransitionEffect::None || effect == TransitionEffect::Instant {
        return;
    }

    let w = dst.area.width;
    let h = dst.area.height;
    match effect {
        TransitionEffect::None | TransitionEffect::Instant => {}
        TransitionEffect::Fade => {
            if tier == ColorTier::Color8 || tier == ColorTier::Color16 {
                let threshold = (fraction * 16.0).round() as u8;
                for y in 0..h {
                    for x in 0..w {
                        let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                        if dither >= threshold {
                            clear_cell(&mut dst[(x, y)]);
                        }
                    }
                }
            } else {
                for cell in dst.content.iter_mut() {
                    cell.fg = dim_color_for_tier(cell.fg, fraction, tier);
                    if cell.bg != Color::Reset {
                        cell.bg = dim_color_for_tier(cell.bg, fraction, tier);
                    }
                }
            }
        }
        TransitionEffect::Curtain => {
            let visible_width = ((w as f32) * fraction).round() as u16;
            let left_bound = (w.saturating_sub(visible_width)) / 2;
            let right_bound = left_bound + visible_width;
            for y in 0..h {
                for x in 0..w {
                    if x < left_bound || x >= right_bound {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Dissolve => {
            let threshold = (fraction * 16.0).round() as u8;
            for y in 0..h {
                for x in 0..w {
                    let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                    if dither >= threshold {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Matrix => {
            let step_seed = (frame_idx + 1) as u32;
            for x in 0..w {
                let col_offset = ((x as u32).wrapping_mul(37).wrapping_add(11) % 5) as i32;
                let lead_y = ((h as f32 + 4.0) * fraction) as i32 - col_offset;
                for y in 0..h {
                    let yi = y as i32;
                    if yi > lead_y {
                        clear_cell(&mut dst[(x, y)]);
                    } else if yi == lead_y || yi == lead_y - 1 {
                        let cell = &mut dst[(x, y)];
                        let seed = (x as u32)
                            .wrapping_mul(1103515245)
                            .wrapping_add((y as u32).wrapping_mul(12345))
                            .wrapping_add(step_seed);
                        let glyph = b"0123456789ABCDEF@#$%&*"[(seed % 22) as usize] as char;
                        cell.set_char(glyph);
                        cell.set_fg(if yi == lead_y {
                            Color::LightGreen
                        } else {
                            Color::Green
                        });
                        cell.set_bg(Color::Black);
                        cell.modifier = Modifier::BOLD;
                    }
                }
            }
        }
        TransitionEffect::SlideLeft => {
            let offset = ((1.0 - fraction) * w as f32).round() as u16;
            if offset > 0 {
                let snap_w = snapshot.area.width;
                let snap_h = snapshot.area.height;
                for y in 0..h {
                    for x in 0..w {
                        if x >= offset && (x - offset) < snap_w && y < snap_h {
                            dst[(x, y)] = snapshot[(x - offset, y)].clone();
                        } else {
                            clear_cell(&mut dst[(x, y)]);
                        }
                    }
                }
            }
        }
        TransitionEffect::SlideRight => {
            let offset = ((1.0 - fraction) * w as f32).round() as u16;
            if offset > 0 {
                let snap_w = snapshot.area.width;
                let snap_h = snapshot.area.height;
                for y in 0..h {
                    for x in 0..w {
                        if x + offset < snap_w && y < snap_h {
                            dst[(x, y)] = snapshot[(x + offset, y)].clone();
                        } else {
                            clear_cell(&mut dst[(x, y)]);
                        }
                    }
                }
            }
        }
        TransitionEffect::Blinds => {
            let slat_size = 4u16;
            let visible_per_slat = (slat_size as f32 * fraction).ceil() as u16;
            for y in 0..h {
                if (y % slat_size) >= visible_per_slat {
                    for x in 0..w {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Iris => {
            let cx = w / 2;
            let cy = h / 2;
            let rx = ((w as f32 / 2.0) * fraction).round() as u16;
            let ry = ((h as f32 / 2.0) * fraction).round() as u16;
            for y in 0..h {
                let dy = y.abs_diff(cy);
                for x in 0..w {
                    let dx = x.abs_diff(cx);
                    if dx > rx || dy > ry {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::WipeDown => {
            let visible_height = ((h as f32) * fraction).round() as u16;
            for y in visible_height..h {
                for x in 0..w {
                    clear_cell(&mut dst[(x, y)]);
                }
            }
        }
    }
}

/// Applies one transition-out frame from `snapshot` into `dst`.
pub(crate) fn apply_transition_out_frame(
    effect: TransitionEffect,
    tier: ColorTier,
    snapshot: &Buffer,
    dst: &mut Buffer,
    visible_fraction: f32,
    frame_idx: usize,
    is_final: bool,
) {
    if is_final {
        for cell in dst.content.iter_mut() {
            clear_cell(cell);
        }
        return;
    }

    copy_snapshot_to_buffer(snapshot, dst);
    let w = dst.area.width;
    let h = dst.area.height;
    match effect {
        TransitionEffect::None | TransitionEffect::Instant => {}
        TransitionEffect::Fade => {
            if tier == ColorTier::Color8 || tier == ColorTier::Color16 {
                let threshold = (visible_fraction * 16.0).round() as u8;
                for y in 0..h {
                    for x in 0..w {
                        let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                        if dither >= threshold {
                            clear_cell(&mut dst[(x, y)]);
                        }
                    }
                }
            } else {
                for cell in dst.content.iter_mut() {
                    cell.fg = dim_color_for_tier(cell.fg, visible_fraction, tier);
                    if cell.bg != Color::Reset {
                        cell.bg = dim_color_for_tier(cell.bg, visible_fraction, tier);
                    }
                }
            }
        }
        TransitionEffect::Curtain => {
            let visible_width = ((w as f32) * visible_fraction).round() as u16;
            let left_bound = (w.saturating_sub(visible_width)) / 2;
            let right_bound = left_bound + visible_width;
            for y in 0..h {
                for x in 0..w {
                    if x < left_bound || x >= right_bound {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Dissolve => {
            let threshold = (visible_fraction * 16.0).round() as u8;
            for y in 0..h {
                for x in 0..w {
                    let dither = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
                    if dither >= threshold {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Matrix => {
            let progress = 1.0 - visible_fraction;
            let step_seed = (frame_idx + 1) as u32;
            for x in 0..w {
                let col_offset = ((x as u32).wrapping_mul(37).wrapping_add(11) % 5) as i32;
                let lead_y = ((h as f32 + 4.0) * progress) as i32 - col_offset;
                for y in 0..h {
                    let yi = y as i32;
                    if yi < lead_y - 1 {
                        clear_cell(&mut dst[(x, y)]);
                    } else if yi == lead_y || yi == lead_y - 1 {
                        let cell = &mut dst[(x, y)];
                        let seed = (x as u32)
                            .wrapping_mul(1103515245)
                            .wrapping_add((y as u32).wrapping_mul(12345))
                            .wrapping_add(step_seed);
                        let glyph = b"0123456789ABCDEF@#$%&*"[(seed % 22) as usize] as char;
                        cell.set_char(glyph);
                        cell.set_fg(Color::LightGreen);
                        cell.set_bg(Color::Black);
                        cell.modifier = Modifier::BOLD;
                    }
                }
            }
        }
        TransitionEffect::SlideLeft => {
            let shift = ((1.0 - visible_fraction) * w as f32).round() as u16;
            let snap_w = snapshot.area.width;
            let snap_h = snapshot.area.height;
            for y in 0..h {
                for x in 0..w {
                    if x + shift < snap_w && y < snap_h {
                        dst[(x, y)] = snapshot[(x + shift, y)].clone();
                    } else {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::SlideRight => {
            let shift = ((1.0 - visible_fraction) * w as f32).round() as u16;
            let snap_w = snapshot.area.width;
            let snap_h = snapshot.area.height;
            for y in 0..h {
                for x in 0..w {
                    if x >= shift && (x - shift) < snap_w && y < snap_h {
                        dst[(x, y)] = snapshot[(x - shift, y)].clone();
                    } else {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Blinds => {
            let slat_size = 4u16;
            let visible_per_slat = (slat_size as f32 * visible_fraction).floor() as u16;
            for y in 0..h {
                if (y % slat_size) >= visible_per_slat {
                    for x in 0..w {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::Iris => {
            let cx = w / 2;
            let cy = h / 2;
            let rx = ((w as f32 / 2.0) * visible_fraction).round() as u16;
            let ry = ((h as f32 / 2.0) * visible_fraction).round() as u16;
            for y in 0..h {
                let dy = y.abs_diff(cy);
                for x in 0..w {
                    let dx = x.abs_diff(cx);
                    if dx > rx || dy > ry {
                        clear_cell(&mut dst[(x, y)]);
                    }
                }
            }
        }
        TransitionEffect::WipeDown => {
            let visible_height = ((h as f32) * visible_fraction).round() as u16;
            for y in visible_height..h {
                for x in 0..w {
                    clear_cell(&mut dst[(x, y)]);
                }
            }
        }
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
            let _ = execute!(
                self.terminal.backend_mut(),
                DisableBracketedPaste,
                DisableMouseCapture
            );
        } else {
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableBracketedPaste,
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
    let _ = io::stdout().write_all(b"\x1b[?2004l");
    let _ = io::stdout().flush();
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

    #[test]
    fn test_capture_snapshot_invokes_callback_once_and_preserves_cells() {
        let mut calls = 0usize;
        let snapshot = capture_snapshot(20, 6, |f| {
            calls += 1;
            let buf = f.buffer_mut();
            buf[(2, 1)].set_char('X').set_fg(Color::Rgb(120, 180, 240));
        });

        assert_eq!(calls, 1);
        assert_eq!(snapshot.area, Rect::new(0, 0, 20, 6));
        assert_eq!(snapshot[(2, 1)].symbol(), "X");
        assert_eq!(snapshot[(2, 1)].fg, Color::Rgb(120, 180, 240));
    }

    #[test]
    fn test_apply_transition_in_and_out_frames_all_effects() {
        let snapshot = capture_snapshot(16, 8, |f| {
            let buf = f.buffer_mut();
            for y in 0..8 {
                for x in 0..16 {
                    buf[(x, y)]
                        .set_char('#')
                        .set_fg(Color::Rgb(200, 100, 50))
                        .set_bg(Color::Rgb(20, 30, 40));
                }
            }
        });

        let effects = [
            TransitionEffect::Fade,
            TransitionEffect::Curtain,
            TransitionEffect::Dissolve,
            TransitionEffect::Matrix,
            TransitionEffect::SlideLeft,
            TransitionEffect::SlideRight,
            TransitionEffect::Blinds,
            TransitionEffect::Iris,
            TransitionEffect::WipeDown,
        ];

        for effect in effects {
            let mut dst = Buffer::empty(snapshot.area);
            // Mid-frame transition-in
            apply_transition_in_frame(
                effect,
                ColorTier::TrueColor,
                &snapshot,
                &mut dst,
                0.5,
                2,
                false,
            );
            // Final transition-in must match snapshot identically
            apply_transition_in_frame(
                effect,
                ColorTier::TrueColor,
                &snapshot,
                &mut dst,
                1.0,
                5,
                true,
            );
            assert_eq!(
                dst.content, snapshot.content,
                "final in-frame mismatch for {effect:?}"
            );

            // Mid-frame transition-out
            apply_transition_out_frame(
                effect,
                ColorTier::TrueColor,
                &snapshot,
                &mut dst,
                0.5,
                2,
                false,
            );
            // Final transition-out must blank all cells to black space
            apply_transition_out_frame(
                effect,
                ColorTier::TrueColor,
                &snapshot,
                &mut dst,
                0.0,
                5,
                true,
            );
            assert!(
                dst.content
                    .iter()
                    .all(|c| c.symbol() == " " && c.fg == Color::Black && c.bg == Color::Black),
                "final out-frame must be black for {effect:?}"
            );
        }

        // Mid-transition resize safety (different destination dimensions must not panic)
        let mut resized = Buffer::empty(Rect::new(0, 0, 10, 4));
        apply_transition_in_frame(
            TransitionEffect::SlideLeft,
            ColorTier::TrueColor,
            &snapshot,
            &mut resized,
            0.5,
            1,
            false,
        );
        assert_eq!(resized.area, Rect::new(0, 0, 10, 4));
    }
}
