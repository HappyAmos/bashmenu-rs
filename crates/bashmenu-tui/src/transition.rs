//! Screen transition animations when returning to menu from external commands.

use ratatui::style::Color;

/// Supported screen transition animation styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionEffect {
    /// Instant atomic redraw via DEC Mode 2026 synchronized output.
    #[default]
    Instant,
    /// Progressive brightness ramp from dark to full color.
    Fade,
    /// Center-to-outer vertical curtain wipe reveal.
    Curtain,
    /// Standard immediate draw without animation.
    None,
}

impl TransitionEffect {
    /// Parses a transition name from configuration string.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("fade") => Self::Fade,
            Some("curtain") => Self::Curtain,
            Some("none") | Some("disabled") | Some("off") => Self::None,
            _ => Self::Instant,
        }
    }
}

/// Dims a color by a scaling factor between 0.0 and 1.0.
pub fn dim_color(color: Color, factor: f32) -> Color {
    let f = factor.clamp(0.0, 1.0);
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * f).round() as u8,
            ((g as f32) * f).round() as u8,
            ((b as f32) * f).round() as u8,
        ),
        Color::Indexed(idx) => {
            if f < 0.5 {
                Color::DarkGray
            } else {
                Color::Indexed(idx)
            }
        }
        Color::White
        | Color::LightCyan
        | Color::LightBlue
        | Color::LightGreen
        | Color::LightYellow
        | Color::LightMagenta
        | Color::LightRed => {
            if f < 0.4 {
                Color::DarkGray
            } else if f < 0.75 {
                match color {
                    Color::White => Color::Gray,
                    Color::LightCyan => Color::Cyan,
                    Color::LightBlue => Color::Blue,
                    Color::LightGreen => Color::Green,
                    Color::LightYellow => Color::Yellow,
                    Color::LightMagenta => Color::Magenta,
                    Color::LightRed => Color::Red,
                    other => other,
                }
            } else {
                color
            }
        }
        Color::Cyan
        | Color::Blue
        | Color::Green
        | Color::Yellow
        | Color::Magenta
        | Color::Red
        | Color::Gray => {
            if f < 0.5 {
                Color::DarkGray
            } else {
                color
            }
        }
        _ => color,
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
        assert_eq!(TransitionEffect::parse(None), TransitionEffect::Instant);
    }

    #[test]
    fn test_dim_color() {
        let white_dim = dim_color(Color::White, 0.2);
        assert_eq!(white_dim, Color::DarkGray);

        let rgb = dim_color(Color::Rgb(100, 200, 50), 0.5);
        assert_eq!(rgb, Color::Rgb(50, 100, 25));

        let reset = dim_color(Color::Reset, 0.5);
        assert_eq!(reset, Color::Reset);
    }
}
