//! Screen transition animations when returning to menu from external commands.

use ratatui::style::Color;
use std::time::{Duration, Instant};

/// Single-frame draw time threshold above which transitions skip remaining
/// intermediate frames and snap straight to the final frame.
pub const ADAPTIVE_FRAME_BUDGET: Duration = Duration::from_millis(45);

/// Supported screen transition animation styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionEffect {
    /// Progressive brightness ramp from dark to full color.
    #[default]
    Fade,
    /// Center-to-outer vertical curtain wipe reveal.
    Curtain,
    /// Retro game Bayer matrix pixel dither dissolve.
    Dissolve,
    /// Cascading green digital rain resolving into view.
    Matrix,
    /// Horizontal slide from right to left.
    SlideLeft,
    /// Horizontal slide from left to right.
    SlideRight,
    /// Horizontal venetian blinds slats opening simultaneously.
    Blinds,
    /// Rectangular box aperture zoom expanding from center.
    Iris,
    /// Vertical top-to-bottom roll wipe down.
    WipeDown,
    /// Instant atomic redraw via DEC Mode 2026 synchronized output.
    Instant,
    /// Standard immediate draw without animation.
    None,
}

impl TransitionEffect {
    /// Parses a transition name from configuration string. Defaults to `Fade`.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("curtain") => Self::Curtain,
            Some("dissolve") | Some("dither") | Some("pixel") => Self::Dissolve,
            Some("matrix") | Some("rain") | Some("cyber") => Self::Matrix,
            Some("slide_left") | Some("slideleft") | Some("slide") => Self::SlideLeft,
            Some("slide_right") | Some("slideright") => Self::SlideRight,
            Some("blinds") | Some("venetian") | Some("slats") => Self::Blinds,
            Some("iris") | Some("zoom") | Some("box") => Self::Iris,
            Some("wipedown") | Some("wipe_down") | Some("blind") | Some("roll") => Self::WipeDown,
            Some("instant") => Self::Instant,
            Some("none") | Some("disabled") | Some("off") | Some("0") | Some("false") => Self::None,
            Some("fade") => Self::Fade,
            _ => Self::Fade,
        }
    }
}

/// 4x4 Bayer dither matrix for dissolve transitions.
pub const BAYER_4X4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Supported transition animation speed profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionSpeed {
    /// Snappy fast transition (~60-75ms total duration).
    Fast,
    /// Balanced standard transition (~100-125ms total duration).
    #[default]
    Normal,
    /// Deliberate relaxed transition (~200-250ms total duration).
    Slow,
    /// Custom millisecond step interval.
    Custom(u64),
}

/// Modal popup animation effect styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalAnimation {
    /// Fast zoom grow on open, and shrink back to launch point on dismiss.
    #[default]
    Zoom,
    /// Standard immediate popup without animation.
    None,
}

impl ModalAnimation {
    /// Parses modal animation effect from configuration string.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("none") | Some("disabled") | Some("off") | Some("false") | Some("0")
            | Some("instant") => Self::None,
            Some("zoom") | Some("shrink") | Some("grow") | Some("scale") => Self::Zoom,
            _ => Self::Zoom,
        }
    }
}

/// Interpolates an intermediate rectangle between an anchor and target.
pub fn interpolate_zoom_rect(
    anchor: (u16, u16),
    target: ratatui::layout::Rect,
    factor: f32,
    viewport: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let f = factor.clamp(0.0, 1.0);
    let ax = anchor.0 as f32;
    let ay = anchor.1 as f32;
    let tx = target.x as f32 + target.width as f32 / 2.0;
    let ty = target.y as f32 + target.height as f32 / 2.0;

    let cx = ax + (tx - ax) * f;
    let cy = ay + (ty - ay) * f;

    let min_w = 6.0_f32.min(target.width as f32);
    let min_h = 3.0_f32.min(target.height as f32);

    let w = (min_w + (target.width as f32 - min_w) * f).round() as u16;
    let h = (min_h + (target.height as f32 - min_h) * f).round() as u16;

    let x = (cx - (w as f32) / 2.0).round().max(viewport.x as f32) as u16;
    let y = (cy - (h as f32) / 2.0).round().max(viewport.y as f32) as u16;

    let max_w = viewport.width.saturating_sub(x.saturating_sub(viewport.x));
    let max_h = viewport.height.saturating_sub(y.saturating_sub(viewport.y));

    ratatui::layout::Rect::new(x, y, w.min(max_w), h.min(max_h))
}

impl TransitionSpeed {
    /// Parses transition speed ("fast", "normal", "slow", or milliseconds).
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("fast") | Some("quick") => Self::Fast,
            Some("slow") => Self::Slow,
            Some("normal") | Some("medium") | Some("default") => Self::Normal,
            Some(custom) => {
                if let Ok(ms) = custom.parse::<u64>() {
                    Self::Custom(ms.clamp(5, 500))
                } else {
                    Self::Normal
                }
            }
            None => Self::Normal,
        }
    }

    /// Returns the step sleep duration for transition-in animations.
    pub fn step_delay_in(&self) -> Duration {
        match self {
            Self::Fast => Duration::from_millis(15),
            Self::Normal => Duration::from_millis(25),
            Self::Slow => Duration::from_millis(50),
            Self::Custom(ms) => Duration::from_millis(*ms),
        }
    }

    /// Returns the step sleep duration for transition-out animations.
    pub fn step_delay_out(&self) -> Duration {
        match self {
            Self::Fast => Duration::from_millis(12),
            Self::Normal => Duration::from_millis(20),
            Self::Slow => Duration::from_millis(40),
            Self::Custom(ms) => Duration::from_millis((*ms * 4) / 5),
        }
    }

    /// Returns the total target wall-clock duration for transition-in animations.
    pub fn total_duration_in(&self) -> Duration {
        match self {
            Self::Fast => Duration::from_millis(80),
            Self::Normal => Duration::from_millis(140),
            Self::Slow => Duration::from_millis(260),
            Self::Custom(ms) => Duration::from_millis(ms.saturating_mul(6)),
        }
    }

    /// Returns the total target wall-clock duration for transition-out animations.
    pub fn total_duration_out(&self) -> Duration {
        match self {
            Self::Fast => Duration::from_millis(65),
            Self::Normal => Duration::from_millis(115),
            Self::Slow => Duration::from_millis(210),
            Self::Custom(ms) => Duration::from_millis(((*ms * 4) / 5).saturating_mul(6)),
        }
    }
}

/// Wall-clock deadline pacer with an adaptive circuit-breaker for transitions.
///
/// Tracks real elapsed time against a fixed total animation duration so slow
/// frames never stack fixed sleeps on top of draw latency. If any single frame
/// exceeds `frame_budget`, the circuit-breaker trips and forces the next
/// iteration to snap straight to the final frame.
#[derive(Debug, Clone)]
pub struct TransitionPacer {
    start: Instant,
    total_duration: Duration,
    steps: usize,
    step_interval: Duration,
    frame_idx: usize,
    frame_budget: Duration,
    force_final: bool,
    finished: bool,
}

impl TransitionPacer {
    /// Creates a new pacer starting now with [`ADAPTIVE_FRAME_BUDGET`].
    pub fn new(total_duration: Duration, steps: usize) -> Self {
        Self::with_start_and_budget(Instant::now(), total_duration, steps, ADAPTIVE_FRAME_BUDGET)
    }

    /// Creates a new pacer with an explicit start time and frame budget.
    pub fn with_start_and_budget(
        start: Instant,
        total_duration: Duration,
        steps: usize,
        frame_budget: Duration,
    ) -> Self {
        let clamped_steps = steps.max(1);
        let step_interval = total_duration
            .checked_div(clamped_steps as u32)
            .unwrap_or(Duration::ZERO);
        Self {
            start,
            total_duration,
            steps: clamped_steps,
            step_interval,
            frame_idx: 0,
            frame_budget,
            force_final: false,
            finished: false,
        }
    }

    /// Returns the next `(progress, frame_idx, is_final)` tuple for `now`.
    ///
    /// `progress` is in `(0.0, 1.0]` and always terminates with `is_final = true`
    /// and `progress = 1.0` before returning `None`.
    pub fn next_frame(&mut self, now: Instant) -> Option<(f32, usize, bool)> {
        if self.finished {
            return None;
        }
        let idx = self.frame_idx;
        let elapsed = now.saturating_duration_since(self.start);
        if self.force_final
            || idx + 1 >= self.steps
            || self.total_duration.is_zero()
            || elapsed >= self.total_duration
        {
            self.finished = true;
            return Some((1.0, idx, true));
        }

        let min_frac = (idx + 1) as f32 / self.steps as f32;
        let wall_frac = (elapsed.as_secs_f32() / self.total_duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = wall_frac.max(min_frac).min(0.999);
        self.frame_idx += 1;
        Some((progress, idx, false))
    }

    /// Computes how long to sleep after a frame that took `draw_elapsed`.
    ///
    /// Trips the adaptive circuit-breaker (returning `Duration::ZERO`) when
    /// `draw_elapsed` exceeds the configured frame budget.
    pub fn sleep_after_frame(&mut self, draw_elapsed: Duration, now: Instant) -> Duration {
        if draw_elapsed > self.frame_budget {
            self.force_final = true;
            return Duration::ZERO;
        }
        let target = self
            .start
            .checked_add(self.step_interval.saturating_mul(self.frame_idx as u32))
            .unwrap_or(now);
        target.saturating_duration_since(now)
    }

    /// Returns whether the adaptive circuit-breaker has tripped.
    pub fn circuit_breaker_tripped(&self) -> bool {
        self.force_final
    }
}

/// Dims a color by a scaling factor between 0.0 and 1.0 using an explicit
/// [`crate::style::ColorTier`] without per-cell environment lookups.
pub fn dim_color_for_tier(color: Color, factor: f32, tier: crate::style::ColorTier) -> Color {
    let f = factor.clamp(0.0, 1.0);
    if (f - 1.0).abs() <= 0.001 {
        return color;
    }

    let (r, g, b) = match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => (200, 200, 200),
        other => crate::wallpaper::color_to_rgb(other),
    };

    let dr = ((r as f32) * f).round() as u8;
    let dg = ((g as f32) * f).round() as u8;
    let db = ((b as f32) * f).round() as u8;

    match tier {
        crate::style::ColorTier::TrueColor => Color::Rgb(dr, dg, db),
        crate::style::ColorTier::Color256 => {
            Color::Indexed(crate::wallpaper::rgb_to_xterm256(dr, dg, db))
        }
        crate::style::ColorTier::Color16 => {
            crate::style::downsample_color_to_16(Color::Rgb(dr, dg, db))
        }
        crate::style::ColorTier::Color8 => {
            crate::style::downsample_color_to_8(Color::Rgb(dr, dg, db))
        }
    }
}

/// Dims a color by a scaling factor between 0.0 and 1.0.
pub fn dim_color(color: Color, factor: f32) -> Color {
    dim_color_for_tier(color, factor, crate::style::ColorTier::detect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_effect_parse() {
        assert_eq!(
            TransitionEffect::parse(Some("instant")),
            TransitionEffect::Instant
        );
        assert_eq!(
            TransitionEffect::parse(Some("fade")),
            TransitionEffect::Fade
        );
        assert_eq!(
            TransitionEffect::parse(Some("Fade")),
            TransitionEffect::Fade
        );
        assert_eq!(
            TransitionEffect::parse(Some("curtain")),
            TransitionEffect::Curtain
        );
        assert_eq!(
            TransitionEffect::parse(Some("none")),
            TransitionEffect::None
        );
        assert_eq!(
            TransitionEffect::parse(Some("disabled")),
            TransitionEffect::None
        );
        assert_eq!(
            TransitionEffect::parse(Some("dissolve")),
            TransitionEffect::Dissolve
        );
        assert_eq!(
            TransitionEffect::parse(Some("matrix")),
            TransitionEffect::Matrix
        );
        assert_eq!(
            TransitionEffect::parse(Some("slide")),
            TransitionEffect::SlideLeft
        );
        assert_eq!(
            TransitionEffect::parse(Some("slide_right")),
            TransitionEffect::SlideRight
        );
        assert_eq!(
            TransitionEffect::parse(Some("blinds")),
            TransitionEffect::Blinds
        );
        assert_eq!(
            TransitionEffect::parse(Some("iris")),
            TransitionEffect::Iris
        );
        assert_eq!(
            TransitionEffect::parse(Some("wipedown")),
            TransitionEffect::WipeDown
        );
        assert_eq!(TransitionEffect::parse(None), TransitionEffect::Fade);
    }

    #[test]
    fn test_transition_speed_parse() {
        assert_eq!(TransitionSpeed::parse(Some("fast")), TransitionSpeed::Fast);
        assert_eq!(TransitionSpeed::parse(Some("quick")), TransitionSpeed::Fast);
        assert_eq!(
            TransitionSpeed::parse(Some("normal")),
            TransitionSpeed::Normal
        );
        assert_eq!(TransitionSpeed::parse(Some("slow")), TransitionSpeed::Slow);
        assert_eq!(
            TransitionSpeed::parse(Some("30")),
            TransitionSpeed::Custom(30)
        );
        assert_eq!(TransitionSpeed::parse(None), TransitionSpeed::Normal);
        assert_eq!(
            TransitionSpeed::parse(Some("invalid")),
            TransitionSpeed::Normal
        );
    }

    #[test]
    fn test_dim_color() {
        std::env::set_var("BASHMENU_COLOR_TIER", "truecolor");
        let white_dim = dim_color(Color::White, 0.0);
        assert_eq!(white_dim, Color::Rgb(0, 0, 0));

        let rgb = dim_color(Color::Rgb(100, 200, 50), 0.5);
        assert_eq!(rgb, Color::Rgb(50, 100, 25));

        let full = dim_color(Color::Rgb(100, 200, 50), 1.0);
        assert_eq!(full, Color::Rgb(100, 200, 50));

        // 256-color tier should downsample to Color::Indexed
        std::env::set_var("BASHMENU_COLOR_TIER", "256");
        let rgb_256 = dim_color(Color::Rgb(100, 200, 50), 0.5);
        let expected_idx = crate::wallpaper::rgb_to_xterm256(50, 100, 25);
        assert_eq!(rgb_256, Color::Indexed(expected_idx));

        std::env::remove_var("BASHMENU_COLOR_TIER");
    }

    #[test]
    fn test_total_durations_ordered() {
        assert!(
            TransitionSpeed::Fast.total_duration_in() < TransitionSpeed::Normal.total_duration_in()
        );
        assert!(
            TransitionSpeed::Normal.total_duration_in() < TransitionSpeed::Slow.total_duration_in()
        );
        assert!(
            TransitionSpeed::Fast.total_duration_out()
                < TransitionSpeed::Normal.total_duration_out()
        );
        assert!(
            TransitionSpeed::Normal.total_duration_out()
                < TransitionSpeed::Slow.total_duration_out()
        );
        assert_eq!(
            TransitionSpeed::Custom(20).total_duration_in(),
            Duration::from_millis(120)
        );
    }

    #[test]
    fn test_transition_pacer_normal_and_catchup() {
        let start = Instant::now();
        let mut pacer = TransitionPacer::with_start_and_budget(
            start,
            Duration::from_millis(120),
            6,
            Duration::from_millis(45),
        );

        // Frame 0 at t=0ms: progress = 1/6, draw takes 5ms -> sleep 15ms to reach 20ms deadline
        let (p0, idx0, final0) = pacer.next_frame(start).unwrap();
        assert!(!final0);
        assert_eq!(idx0, 0);
        assert!((p0 - (1.0 / 6.0)).abs() < 0.01);
        let sleep0 =
            pacer.sleep_after_frame(Duration::from_millis(5), start + Duration::from_millis(5));
        assert_eq!(sleep0, Duration::from_millis(15));
        assert!(!pacer.circuit_breaker_tripped());

        // Frame 1 at t=70ms (behind schedule): progress catches up to 70/120, sleep is 0
        let t1 = start + Duration::from_millis(70);
        let (p1, idx1, final1) = pacer.next_frame(t1).unwrap();
        assert!(!final1);
        assert_eq!(idx1, 1);
        assert!(p1 >= 0.58);
        let sleep1 =
            pacer.sleep_after_frame(Duration::from_millis(10), t1 + Duration::from_millis(10));
        assert_eq!(sleep1, Duration::ZERO);

        // Elapsed >= total_duration -> immediately snaps to final frame
        let (pf, _idxf, is_final) = pacer
            .next_frame(start + Duration::from_millis(125))
            .unwrap();
        assert!(is_final);
        assert_eq!(pf, 1.0);
        assert!(pacer
            .next_frame(start + Duration::from_millis(130))
            .is_none());
    }

    #[test]
    fn test_transition_pacer_adaptive_circuit_breaker() {
        let start = Instant::now();
        let mut pacer = TransitionPacer::with_start_and_budget(
            start,
            Duration::from_millis(140),
            6,
            Duration::from_millis(45),
        );

        // Frame 0 takes 60ms (> 45ms budget) -> trips circuit-breaker with zero sleep
        let (_p0, _idx0, final0) = pacer.next_frame(start).unwrap();
        assert!(!final0);
        let sleep0 =
            pacer.sleep_after_frame(Duration::from_millis(60), start + Duration::from_millis(60));
        assert_eq!(sleep0, Duration::ZERO);
        assert!(pacer.circuit_breaker_tripped());

        // Next frame snaps directly to final (progress=1.0, is_final=true)
        let (pf, _idxf, is_final) = pacer.next_frame(start + Duration::from_millis(61)).unwrap();
        assert!(is_final);
        assert_eq!(pf, 1.0);
        assert!(pacer
            .next_frame(start + Duration::from_millis(62))
            .is_none());
    }

    #[test]
    fn test_dim_color_for_tier_explicit() {
        let base = Color::Rgb(100, 200, 50);
        assert_eq!(
            dim_color_for_tier(base, 0.5, crate::style::ColorTier::TrueColor),
            Color::Rgb(50, 100, 25)
        );
        assert!(matches!(
            dim_color_for_tier(base, 0.5, crate::style::ColorTier::Color256),
            Color::Indexed(_)
        ));
    }
}
