//! Screen transition animations when returning to menu from external commands.

use ratatui::style::Color;
use std::time::Duration;

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
}

/// Dims a color by a scaling factor between 0.0 and 1.0.
pub fn dim_color(color: Color, factor: f32) -> Color {
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

    match crate::style::ColorTier::detect() {
        crate::style::ColorTier::TrueColor | crate::style::ColorTier::Color256 => {
            Color::Rgb(dr, dg, db)
        }
        crate::style::ColorTier::Color16 => {
            crate::style::downsample_color_to_16(Color::Rgb(dr, dg, db))
        }
        crate::style::ColorTier::Color8 => {
            crate::style::downsample_color_to_8(Color::Rgb(dr, dg, db))
        }
    }
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
        let white_dim = dim_color(Color::White, 0.0);
        assert_eq!(white_dim, Color::Rgb(0, 0, 0));

        let rgb = dim_color(Color::Rgb(100, 200, 50), 0.5);
        assert_eq!(rgb, Color::Rgb(50, 100, 25));

        let full = dim_color(Color::Rgb(100, 200, 50), 1.0);
        assert_eq!(full, Color::Rgb(100, 200, 50));
    }
}
