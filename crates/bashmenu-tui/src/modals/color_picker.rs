//! TrueColor (24-bit RGB) and ANSI 256-color interactive picker modal.
//!
//! Provides a 3-mode tabbed interface:
//! 1. 256 Palette: Classic 16x16 ANSI color grid
//! 2. TrueColor (HSV): Continuous Saturation/Value canvas with Hue slider
//! 3. Hex Code: Direct 24-bit `#RRGGBB` hex code editor with live swatch

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crate::wallpaper::{color_to_rgb, rgb_to_xterm256};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Active mode tab within the color picker modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerTab {
    /// Classic 16x16 ANSI 256 color palette grid
    Palette256,
    /// 2D HSV canvas (Saturation x Value) with vertical Hue slider
    TrueColorHsv,
    /// Direct 24-bit `#RRGGBB` hexadecimal code text input
    HexInput,
}

/// Converts RGB (0..=255) to HSV (h: 0.0..360.0, s: 0.0..1.0, v: 0.0..1.0).
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let mut h = if delta <= 1e-5 {
        0.0
    } else if (max - rf).abs() <= 1e-5 {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (max - gf).abs() <= 1e-5 {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };

    if h < 0.0 {
        h += 360.0;
    }

    (h, s, v)
}

/// Converts HSV (h: 0.0..360.0, s: 0.0..1.0, v: 0.0..1.0) to RGB (0..=255).
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let h_norm = h.rem_euclid(360.0);
    let h_prime = h_norm / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if (0.0..1.0).contains(&h_prime) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h_prime) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h_prime) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h_prime) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h_prime) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Parses a 6-digit hexadecimal color string into RGB.
pub fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let clean = s.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    let val = u32::from_str_radix(clean, 16).ok()?;
    let r = ((val >> 16) & 0xFF) as u8;
    let g = ((val >> 8) & 0xFF) as u8;
    let b = (val & 0xFF) as u8;
    Some((r, g, b))
}

/// Interactive TrueColor and 256-color picker modal dialog.
#[derive(Debug, Clone)]
pub struct ColorPickerModal {
    /// Dialog title header
    pub title: String,
    /// Currently active mode tab
    pub tab: ColorPickerTab,
    /// Selected ANSI 256-color palette index (0..=255)
    pub selected_256: u8,
    /// Current Hue angle (0.0..=360.0 degrees)
    pub hue: f32,
    /// Current Saturation ratio (0.0..=1.0)
    pub sat: f32,
    /// Current Value / Brightness ratio (0.0..=1.0)
    pub val: f32,
    /// Hex text input buffer
    pub hex_input: String,
    /// Currently resolved color value
    pub current_color: Color,
}

impl ColorPickerModal {
    /// Constructs a color picker initialized with the given target Color.
    pub fn new(title: impl Into<String>, initial_color: Color) -> Self {
        let (r, g, b) = color_to_rgb(initial_color);
        let (h, s, v) = rgb_to_hsv(r, g, b);
        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);

        let (tab, sel_256) = match initial_color {
            Color::Indexed(code) => (ColorPickerTab::Palette256, code),
            Color::Rgb(_, _, _) => (ColorPickerTab::TrueColorHsv, rgb_to_xterm256(r, g, b)),
            _ => (ColorPickerTab::Palette256, rgb_to_xterm256(r, g, b)),
        };

        Self {
            title: title.into(),
            tab,
            selected_256: sel_256,
            hue: h,
            sat: s,
            val: v,
            hex_input: hex,
            current_color: initial_color,
        }
    }

    /// Synchronizes current color from HSV components into RGB and hex representation.
    fn sync_from_hsv(&mut self) {
        let (r, g, b) = hsv_to_rgb(self.hue, self.sat, self.val);
        self.current_color = Color::Rgb(r, g, b);
        self.selected_256 = rgb_to_xterm256(r, g, b);
        self.hex_input = format!("#{:02x}{:02x}{:02x}", r, g, b);
    }

    /// Synchronizes current color from an ANSI 256 index into RGB and HSV.
    fn sync_from_256(&mut self, code: u8) {
        self.selected_256 = code;
        self.current_color = Color::Indexed(code);
        let (r, g, b) = color_to_rgb(Color::Indexed(code));
        let (h, s, v) = rgb_to_hsv(r, g, b);
        self.hue = h;
        self.sat = s;
        self.val = v;
        self.hex_input = format!("#{:02x}{:02x}{:02x}", r, g, b);
    }

    /// Synchronizes current color from hex input string if valid.
    fn sync_from_hex(&mut self) {
        if let Some((r, g, b)) = parse_hex_rgb(&self.hex_input) {
            self.current_color = Color::Rgb(r, g, b);
            let (h, s, v) = rgb_to_hsv(r, g, b);
            self.hue = h;
            self.sat = s;
            self.val = v;
            self.selected_256 = rgb_to_xterm256(r, g, b);
        }
    }

    /// Handles keyboard events across all color picker tabs.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<Option<Color>> {
        match code {
            KeyCode::Esc => Some(None),
            KeyCode::Tab => {
                self.tab = match self.tab {
                    ColorPickerTab::Palette256 => ColorPickerTab::TrueColorHsv,
                    ColorPickerTab::TrueColorHsv => ColorPickerTab::HexInput,
                    ColorPickerTab::HexInput => ColorPickerTab::Palette256,
                };
                None
            }
            KeyCode::BackTab => {
                self.tab = match self.tab {
                    ColorPickerTab::Palette256 => ColorPickerTab::HexInput,
                    ColorPickerTab::TrueColorHsv => ColorPickerTab::Palette256,
                    ColorPickerTab::HexInput => ColorPickerTab::TrueColorHsv,
                };
                None
            }
            KeyCode::Enter => Some(Some(self.current_color)),
            _ => match self.tab {
                ColorPickerTab::Palette256 => self.handle_key_palette(code),
                ColorPickerTab::TrueColorHsv => self.handle_key_hsv(code),
                ColorPickerTab::HexInput => self.handle_key_hex(code),
            },
        }
    }

    fn handle_key_palette(&mut self, code: KeyCode) -> Option<Option<Color>> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                let new_code = self.selected_256.saturating_sub(1);
                self.sync_from_256(new_code);
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let new_code = self.selected_256.saturating_add(1);
                self.sync_from_256(new_code);
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let new_code = self.selected_256.saturating_sub(16);
                self.sync_from_256(new_code);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_256 <= 239 {
                    let new_code = self.selected_256 + 16;
                    self.sync_from_256(new_code);
                }
                None
            }
            KeyCode::PageUp => {
                let new_code = self.selected_256.saturating_sub(64);
                self.sync_from_256(new_code);
                None
            }
            KeyCode::PageDown => {
                if self.selected_256 <= 191 {
                    let new_code = self.selected_256 + 64;
                    self.sync_from_256(new_code);
                }
                None
            }
            KeyCode::Home => {
                self.sync_from_256(0);
                None
            }
            KeyCode::End => {
                self.sync_from_256(255);
                None
            }
            KeyCode::Char('2') => {
                self.tab = ColorPickerTab::TrueColorHsv;
                None
            }
            KeyCode::Char('3') => {
                self.tab = ColorPickerTab::HexInput;
                None
            }
            _ => None,
        }
    }

    fn handle_key_hsv(&mut self, code: KeyCode) -> Option<Option<Color>> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.sat = (self.sat - 0.032).max(0.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.sat = (self.sat + 0.032).min(1.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.val = (self.val + 0.045).min(1.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.val = (self.val - 0.045).max(0.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Char('[') | KeyCode::Char('<') | KeyCode::PageDown => {
                self.hue = (self.hue - 6.0).rem_euclid(360.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Char(']') | KeyCode::Char('>') | KeyCode::PageUp => {
                self.hue = (self.hue + 6.0).rem_euclid(360.0);
                self.sync_from_hsv();
                None
            }
            KeyCode::Char('1') => {
                self.tab = ColorPickerTab::Palette256;
                None
            }
            KeyCode::Char('3') => {
                self.tab = ColorPickerTab::HexInput;
                None
            }
            _ => None,
        }
    }

    fn handle_key_hex(&mut self, code: KeyCode) -> Option<Option<Color>> {
        match code {
            KeyCode::Char(c) if c.is_ascii_hexdigit() => {
                let mut current = self.hex_input.trim_start_matches('#').to_string();
                if current.len() < 6 {
                    current.push(c.to_ascii_uppercase());
                    self.hex_input = format!("#{}", current);
                    self.sync_from_hex();
                }
                None
            }
            KeyCode::Backspace => {
                let mut current = self.hex_input.trim_start_matches('#').to_string();
                if !current.is_empty() {
                    current.pop();
                    self.hex_input = format!("#{}", current);
                    self.sync_from_hex();
                }
                None
            }
            _ => None,
        }
    }

    /// Handles mouse clicks across tabs, grids, canvases, and sliders.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, origin: (u16, u16)) -> Option<Option<Color>> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return None;
        }

        let (ox, oy) = origin;
        let col = mouse.column;
        let row = mouse.row;

        // 1. Tab Bar interaction on row 0 of inner
        if row == oy {
            let t1_start = ox + 1;
            let t1_end = t1_start + 18;
            let t2_start = t1_end + 1;
            let t2_end = t2_start + 21;
            let t3_start = t2_end + 1;
            let t3_end = t3_start + 16;

            if col >= t1_start && col < t1_end {
                self.tab = ColorPickerTab::Palette256;
                return None;
            }
            if col >= t2_start && col < t2_end {
                self.tab = ColorPickerTab::TrueColorHsv;
                return None;
            }
            if col >= t3_start && col < t3_end {
                self.tab = ColorPickerTab::HexInput;
                return None;
            }
        }

        // 2. Mode-specific mouse handlers
        match self.tab {
            ColorPickerTab::Palette256 => {
                let gx = ox + 2;
                let gy = oy + 2;
                if col >= gx && col < gx + (16 * 3) && row >= gy && row < gy + 16 {
                    let cell_x = ((col - gx) / 3) as u8;
                    let cell_y = (row - gy) as u8;
                    let code = cell_y * 16 + cell_x;
                    self.sync_from_256(code);
                    return Some(Some(self.current_color));
                }
            }
            ColorPickerTab::TrueColorHsv => {
                let canvas_x = ox + 2;
                let canvas_y = oy + 2;
                let canvas_w = 32u16;
                let canvas_h = 14u16;

                // Click on Sat/Val canvas
                if col >= canvas_x
                    && col < canvas_x + canvas_w
                    && row >= canvas_y
                    && row < canvas_y + canvas_h
                {
                    let dx = (col - canvas_x) as f32;
                    let dy = (row - canvas_y) as f32;
                    self.sat = (dx / (canvas_w.saturating_sub(1) as f32)).clamp(0.0, 1.0);
                    self.val = (1.0 - (dy / (canvas_h.saturating_sub(1) as f32))).clamp(0.0, 1.0);
                    self.sync_from_hsv();
                    return None;
                }

                // Click on vertical Hue slider
                let hue_x = canvas_x + canvas_w + 3;
                let hue_y = canvas_y;
                let hue_w = 3u16;
                let hue_h = 14u16;

                if col >= hue_x && col < hue_x + hue_w && row >= hue_y && row < hue_y + hue_h {
                    let dy = (row - hue_y) as f32;
                    self.hue = ((dy / (hue_h.saturating_sub(1) as f32)) * 360.0).clamp(0.0, 360.0);
                    self.sync_from_hsv();
                    return None;
                }
            }
            ColorPickerTab::HexInput => {
                // Clicking swatch area commits color
                let swatch_y = oy + 7;
                if row >= swatch_y && row <= swatch_y + 3 && col >= ox + 15 && col <= ox + 45 {
                    return Some(Some(self.current_color));
                }
            }
        }

        None
    }

    /// Renders the color picker modal into `buf`.
    pub fn render(
        &self,
        area: Rect,
        styles: &ResolvedThemeStyles,
        buf: &mut Buffer,
    ) -> (Rect, (u16, u16)) {
        let dialog_w = 64;
        let dialog_h = 23;

        let inner = render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        // 1. Render Tab Header Bar
        let t1_active = self.tab == ColorPickerTab::Palette256;
        let t2_active = self.tab == ColorPickerTab::TrueColorHsv;
        let t3_active = self.tab == ColorPickerTab::HexInput;

        let active_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let inactive_style = Style::default().fg(Color::White);

        let t1_s = if t1_active {
            active_style
        } else {
            inactive_style
        };
        let t2_s = if t2_active {
            active_style
        } else {
            inactive_style
        };
        let t3_s = if t3_active {
            active_style
        } else {
            inactive_style
        };

        let t1_text = " [1: 256 Palette] ";
        let t2_text = " [2: TrueColor (HSV)] ";
        let t3_text = " [3: Hex Code] ";

        let t1_x = inner.x + 1;
        let t2_x = t1_x + t1_text.len() as u16;
        let t3_x = t2_x + t2_text.len() as u16;

        buf.set_string(t1_x, inner.y, t1_text, t1_s);
        buf.set_string(t2_x, inner.y, t2_text, t2_s);
        buf.set_string(t3_x, inner.y, t3_text, t3_s);

        // Divider under tab bar
        let div_w = inner.width as usize;
        buf.set_string(
            inner.x,
            inner.y + 1,
            "─".repeat(div_w),
            Style::default().fg(Color::DarkGray),
        );

        // 2. Render Active Tab View
        match self.tab {
            ColorPickerTab::Palette256 => self.render_palette_view(inner, styles, buf),
            ColorPickerTab::TrueColorHsv => self.render_hsv_view(inner, styles, buf),
            ColorPickerTab::HexInput => self.render_hex_view(inner, styles, buf),
        }

        (inner, (inner.x, inner.y))
    }

    fn render_palette_view(&self, inner: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let grid_x = inner.x + 2;
        let grid_y = inner.y + 2;

        for row in 0..16u8 {
            for col in 0..16u8 {
                let code = row * 16 + col;
                let px = grid_x + (col as u16 * 3);
                let py = grid_y + row as u16;

                let is_selected = code == self.selected_256;
                let (cell_str, cell_style) = if is_selected {
                    let (r, g, b) = color_to_rgb(Color::Indexed(code));
                    let lum = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
                    let fg_color = if lum > 128 {
                        Color::Black
                    } else {
                        Color::White
                    };
                    (
                        " X",
                        Style::default()
                            .fg(fg_color)
                            .bg(Color::Indexed(code))
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (
                        "  ",
                        Style::default().bg(Color::Indexed(code)).fg(Color::Reset),
                    )
                };

                buf.set_string(px, py, cell_str, cell_style);
            }
        }

        // Side preview & status
        let info_x = grid_x + 50;
        let info_y = grid_y + 2;
        let (r, g, b) = color_to_rgb(Color::Indexed(self.selected_256));

        buf.set_string(info_x, info_y, "Swatch:", styles.gutter);
        buf.set_string(
            info_x,
            info_y + 1,
            "████████",
            Style::default().fg(Color::Indexed(self.selected_256)),
        );
        buf.set_string(
            info_x,
            info_y + 3,
            format!("Index: {:3}", self.selected_256),
            styles.accent.add_modifier(Modifier::BOLD),
        );
        buf.set_string(
            info_x,
            info_y + 4,
            format!("Hex: #{r:02x}{g:02x}{b:02x}"),
            Style::default().fg(Color::White),
        );
        buf.set_string(
            info_x,
            info_y + 5,
            format!("R:{:<3} G:{:<3}", r, g),
            styles.gutter,
        );
        buf.set_string(info_x, info_y + 6, format!("B:{:<3}", b), styles.gutter);

        // Bottom status hint
        let badge_y = inner.y + inner.height.saturating_sub(1);
        let badge_line = Line::from(vec![
            Span::styled(" [", styles.border),
            Span::styled("██", Style::default().fg(Color::Indexed(self.selected_256))),
            Span::styled("] ", styles.border),
            Span::styled(
                format!("Color: {:03}", self.selected_256),
                styles.accent.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  (Enter: Select | Tab: Modes | Esc: Cancel) ",
                styles.gutter,
            ),
        ]);
        buf.set_line(
            inner.x + 2,
            badge_y,
            &badge_line,
            inner.width.saturating_sub(4),
        );
    }

    fn render_hsv_view(&self, inner: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let canvas_x = inner.x + 2;
        let canvas_y = inner.y + 2;
        let canvas_w = 32u16;
        let canvas_h = 14u16;

        // 1. Render Saturation x Value canvas with half-blocks (28 vertical resolution steps)
        let cur_cx =
            ((self.sat * (canvas_w.saturating_sub(1) as f32)).round() as u16).min(canvas_w - 1);
        let cur_step = (((1.0 - self.val) * 27.0).round() as u16).min(27);
        let cur_cy = cur_step / 2;

        for cy in 0..canvas_h {
            for cx in 0..canvas_w {
                let px = canvas_x + cx;
                let py = canvas_y + cy;

                let s = cx as f32 / (canvas_w.saturating_sub(1) as f32);
                let v_top = 1.0 - ((cy * 2) as f32 / 27.0);
                let v_bot = 1.0 - ((cy * 2 + 1) as f32 / 27.0);

                let (rt, gt, bt) = hsv_to_rgb(self.hue, s, v_top.clamp(0.0, 1.0));
                let (rb, gb, bb) = hsv_to_rgb(self.hue, s, v_bot.clamp(0.0, 1.0));

                let fg_c = Color::Rgb(rt, gt, bt);
                let bg_c = Color::Rgb(rb, gb, bb);

                if cx == cur_cx && cy == cur_cy {
                    // Contrasting crosshair cursor
                    let (r_cur, g_cur, b_cur) = color_to_rgb(self.current_color);
                    let lum = (r_cur as u32 * 299 + g_cur as u32 * 587 + b_cur as u32 * 114) / 1000;
                    let cursor_fg = if lum > 128 {
                        Color::Black
                    } else {
                        Color::White
                    };
                    buf.set_string(
                        px,
                        py,
                        "+",
                        Style::default()
                            .fg(cursor_fg)
                            .bg(bg_c)
                            .add_modifier(Modifier::BOLD),
                    );
                } else {
                    buf.set_string(px, py, "▀", Style::default().fg(fg_c).bg(bg_c));
                }
            }
        }

        // 2. Render Vertical Hue Slider
        let hue_x = canvas_x + canvas_w + 3;
        let hue_y = canvas_y;
        let hue_w = 3u16;
        let hue_h = 14u16;

        let cur_hue_step = ((self.hue / 360.0) * 27.0).round() as u16;
        let cur_hue_row = (cur_hue_step / 2).min(hue_h - 1);

        for hy in 0..hue_h {
            let h_top = ((hy * 2) as f32 / 27.0) * 360.0;
            let h_bot = ((hy * 2 + 1) as f32 / 27.0) * 360.0;

            let (rt, gt, bt) = hsv_to_rgb(h_top, 1.0, 1.0);
            let (rb, gb, bb) = hsv_to_rgb(h_bot, 1.0, 1.0);

            let style = Style::default()
                .fg(Color::Rgb(rt, gt, bt))
                .bg(Color::Rgb(rb, gb, bb));

            for hx in 0..hue_w {
                buf.set_string(hue_x + hx, hue_y + hy, "▀", style);
            }

            if hy == cur_hue_row {
                buf.set_string(
                    hue_x + hue_w,
                    hue_y + hy,
                    "◀",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // 3. Render Color Info Panel
        let (r, g, b) = color_to_rgb(self.current_color);
        let info_x = hue_x + hue_w + 3;
        let info_y = canvas_y;

        buf.set_string(info_x, info_y, "Selected Swatch:", styles.gutter);
        buf.set_string(
            info_x,
            info_y + 1,
            "████████████",
            Style::default().fg(self.current_color),
        );
        buf.set_string(
            info_x,
            info_y + 2,
            "████████████",
            Style::default().fg(self.current_color),
        );

        buf.set_string(
            info_x,
            info_y + 4,
            format!("Hex:  #{r:02x}{g:02x}{b:02x}"),
            styles.accent.add_modifier(Modifier::BOLD),
        );
        buf.set_string(
            info_x,
            info_y + 5,
            format!("R:{:<3} G:{:<3} B:{:<3}", r, g, b),
            Style::default().fg(Color::White),
        );
        buf.set_string(
            info_x,
            info_y + 7,
            format!("Hue: {:>3.0}°", self.hue),
            styles.gutter,
        );
        buf.set_string(
            info_x,
            info_y + 8,
            format!("Sat: {:>3.0}%", self.sat * 100.0),
            styles.gutter,
        );
        buf.set_string(
            info_x,
            info_y + 9,
            format!("Val: {:>3.0}%", self.val * 100.0),
            styles.gutter,
        );
        buf.set_string(
            info_x,
            info_y + 11,
            format!("ANSI: ~{:3}", self.selected_256),
            styles.gutter,
        );

        // Bottom status line
        let badge_y = inner.y + inner.height.saturating_sub(1);
        let hint_line = Line::from(vec![
            Span::styled(" [", styles.border),
            Span::styled("██", Style::default().fg(self.current_color)),
            Span::styled("] ", styles.border),
            Span::styled(
                format!("#{r:02x}{g:02x}{b:02x}"),
                styles.accent.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  (Arrows: S/V | [/]: Hue | Enter: Select | Tab: Modes) ",
                styles.gutter,
            ),
        ]);
        buf.set_line(
            inner.x + 2,
            badge_y,
            &hint_line,
            inner.width.saturating_sub(4),
        );
    }

    fn render_hex_view(&self, inner: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let center_x = inner.x + (inner.width.saturating_sub(44)) / 2;
        let start_y = inner.y + 3;

        buf.set_string(
            center_x,
            start_y,
            "Enter 24-bit TrueColor Hex Code:",
            styles.accent.add_modifier(Modifier::BOLD),
        );

        let input_box = format!("[ {:<8} ]", self.hex_input);
        buf.set_string(
            center_x,
            start_y + 2,
            &input_box,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let (r, g, b) = color_to_rgb(self.current_color);
        buf.set_string(
            center_x,
            start_y + 5,
            "Color Preview Swatch:",
            styles.gutter,
        );
        buf.set_string(
            center_x,
            start_y + 6,
            "██████████████████████████",
            Style::default().fg(self.current_color),
        );
        buf.set_string(
            center_x,
            start_y + 7,
            "██████████████████████████",
            Style::default().fg(self.current_color),
        );

        buf.set_string(
            center_x,
            start_y + 9,
            format!("RGB Channels:  Red: {:3}  Green: {:3}  Blue: {:3}", r, g, b),
            Style::default().fg(Color::White),
        );
        buf.set_string(
            center_x,
            start_y + 11,
            format!("Nearest ANSI 256 Index:  {}", self.selected_256),
            styles.gutter,
        );

        // Bottom status line
        let badge_y = inner.y + inner.height.saturating_sub(1);
        let hint_line = Line::from(vec![
            Span::styled(" [", styles.border),
            Span::styled("██", Style::default().fg(self.current_color)),
            Span::styled("] ", styles.border),
            Span::styled(
                format!("#{r:02x}{g:02x}{b:02x}"),
                styles.accent.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  (Type 0-9/A-F | Backspace | Enter: Select | Tab: Modes) ",
                styles.gutter,
            ),
        ]);
        buf.set_line(
            inner.x + 2,
            badge_y,
            &hint_line,
            inner.width.saturating_sub(4),
        );
    }
}
