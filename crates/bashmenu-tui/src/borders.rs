//! Window borders character definitions and fallbacks.

use bashmenu_core::{parse_ascii_val, WindowBorders};

/// Resolved character glyphs for window borders, gutters, tees, and caps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWindowBorders {
    /// Horizontal top frame border character
    pub border_horizontal_top: String,
    /// Horizontal bottom frame border character
    pub border_horizontal_bottom: String,
    /// Vertical left frame border character
    pub border_vertical_left: String,
    /// Vertical right frame border character
    pub border_vertical_right: String,
    /// Top-left frame corner character
    pub border_top_left: String,
    /// Top-right frame corner character
    pub border_top_right: String,
    /// Bottom-left frame corner character
    pub border_bottom_left: String,
    /// Bottom-right frame corner character
    pub border_bottom_right: String,
    /// Left T-junction connector character
    pub left_tee: String,
    /// Right T-junction connector character
    pub right_tee: String,
    /// Top T-junction connector character
    pub top_tee: String,
    /// Bottom T-junction connector character
    pub bottom_tee: String,
    /// Left cap bracket enclosing panel titles
    pub title_left_cap: String,
    /// Right cap bracket enclosing panel titles
    pub title_right_cap: String,
    /// Horizontal border character separating status/help gutter
    pub gutter_border: String,
    /// Vertical separator character dividing gutter columns
    pub gutter_separator: String,
    /// Left T-junction connector on the gutter border
    pub gutter_left_tee: String,
    /// Right T-junction connector on the gutter border
    pub gutter_right_tee: String,
    /// Top T-junction connector on the gutter border
    pub gutter_top_tee: String,
    /// Bottom T-junction connector on the gutter border
    pub gutter_bottom_tee: String,
    /// Character used to render window drop shadows
    pub shadow_char: String,
}

impl Default for ResolvedWindowBorders {
    /// Returns default window borders using standard single-line box drawing characters.
    fn default() -> Self {
        Self::from_theme_borders(None)
    }
}

impl ResolvedWindowBorders {
    /// Resolves border glyphs from a theme's `window:` configuration with standard box-drawing fallbacks
    pub fn from_theme_borders(b: Option<&WindowBorders>) -> Self {
        let parse = |opt: Option<&String>| opt.map(|s| parse_ascii_val(s));

        let b_h = b
            .and_then(|x| parse(x.border_horizontal.as_ref()))
            .unwrap_or_else(|| "─".to_string());
        let b_v = b
            .and_then(|x| parse(x.border_vertical.as_ref()))
            .unwrap_or_else(|| "│".to_string());
        let b_h_top = b
            .and_then(|x| parse(x.border_horizontal_top.as_ref()))
            .unwrap_or_else(|| b_h.clone());
        let b_h_bot = b
            .and_then(|x| parse(x.border_horizontal_bottom.as_ref()))
            .unwrap_or_else(|| b_h.clone());
        let b_v_left = b
            .and_then(|x| parse(x.border_vertical_left.as_ref()))
            .unwrap_or_else(|| b_v.clone());
        let b_v_right = b
            .and_then(|x| parse(x.border_vertical_right.as_ref()))
            .unwrap_or_else(|| b_v.clone());

        let left_tee = b
            .and_then(|x| parse(x.left_tee.as_ref()))
            .unwrap_or_else(|| "├".to_string());
        let right_tee = b
            .and_then(|x| parse(x.right_tee.as_ref()))
            .unwrap_or_else(|| "┤".to_string());
        let top_tee = b
            .and_then(|x| parse(x.top_tee.as_ref()))
            .unwrap_or_else(|| "┬".to_string());
        let bottom_tee = b
            .and_then(|x| parse(x.bottom_tee.as_ref()))
            .unwrap_or_else(|| "┴".to_string());

        let gutter_border = b
            .and_then(|x| parse(x.gutter_border.as_ref()))
            .unwrap_or_else(|| b_h_bot.clone());
        let gutter_separator = b
            .and_then(|x| parse(x.gutter_separator.as_ref()))
            .unwrap_or_else(|| b_v.clone());
        let gutter_left_tee = b
            .and_then(|x| parse(x.gutter_left_tee.as_ref()))
            .unwrap_or_else(|| left_tee.clone());
        let gutter_right_tee = b
            .and_then(|x| parse(x.gutter_right_tee.as_ref()))
            .unwrap_or_else(|| right_tee.clone());
        let gutter_top_tee = b
            .and_then(|x| parse(x.gutter_top_tee.as_ref()))
            .unwrap_or_else(|| top_tee.clone());
        let gutter_bottom_tee = b
            .and_then(|x| parse(x.gutter_bottom_tee.as_ref()))
            .unwrap_or_else(|| bottom_tee.clone());

        Self {
            border_horizontal_top: b_h_top,
            border_horizontal_bottom: b_h_bot,
            border_vertical_left: b_v_left,
            border_vertical_right: b_v_right,
            border_top_left: b
                .and_then(|x| parse(x.border_top_left.as_ref()))
                .unwrap_or_else(|| "┌".to_string()),
            border_top_right: b
                .and_then(|x| parse(x.border_top_right.as_ref()))
                .unwrap_or_else(|| "┐".to_string()),
            border_bottom_left: b
                .and_then(|x| parse(x.border_bottom_left.as_ref()))
                .unwrap_or_else(|| "└".to_string()),
            border_bottom_right: b
                .and_then(|x| parse(x.border_bottom_right.as_ref()))
                .unwrap_or_else(|| "┘".to_string()),
            left_tee,
            right_tee,
            top_tee,
            bottom_tee,
            title_left_cap: b
                .and_then(|x| parse(x.title_left_cap.as_ref()))
                .unwrap_or_else(|| "[".to_string()),
            title_right_cap: b
                .and_then(|x| parse(x.title_right_cap.as_ref()))
                .unwrap_or_else(|| "]".to_string()),
            gutter_border,
            gutter_separator,
            gutter_left_tee,
            gutter_right_tee,
            gutter_top_tee,
            gutter_bottom_tee,
            shadow_char: b
                .and_then(|x| parse(x.shadow_char.as_ref()))
                .unwrap_or_else(|| "░".to_string()),
        }
    }
}
