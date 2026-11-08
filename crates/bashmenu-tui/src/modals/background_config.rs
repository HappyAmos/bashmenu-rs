//! Interactive background wallpaper configuration modal dialog.
//!
//! Provides configuration for wallpaper image paths, opacity blending, scaling
//! modes (cover, stretch, fit, center, tile), rendering modes (halfblock and
//! quarterblock), color clustering, and palette quantization.

use super::frame::render_modal_frame;
use crate::style::ResolvedThemeStyles;
use crate::text_input::TextInput;
use bashmenu_core::WallpaperConfig;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Focusable interactive control fields in the background configuration modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundFocus {
    /// Wallpaper file path text input
    Image,
    /// Launch file picker browser button
    Browse,
    /// Opacity factor text input (0.0 to 1.0)
    Opacity,
    /// Wallpaper scaling algorithm selector
    Scaling,
    /// Rendering mode selector (halfblock vs quarterblock)
    Mode,
    /// Color clustering distance text input (0 to 128)
    Clustering,
    /// Palette color quantization step text input (1 to 256)
    Quantization,
    /// Apply and save wallpaper settings button
    Apply,
    /// Remove background wallpaper button
    Remove,
    /// Cancel and discard changes button
    Cancel,
}

impl BackgroundFocus {
    /// Ordered sequence of all focusable controls for tab navigation.
    const ALL: [BackgroundFocus; 10] = [
        BackgroundFocus::Image,
        BackgroundFocus::Browse,
        BackgroundFocus::Opacity,
        BackgroundFocus::Scaling,
        BackgroundFocus::Mode,
        BackgroundFocus::Clustering,
        BackgroundFocus::Quantization,
        BackgroundFocus::Apply,
        BackgroundFocus::Remove,
        BackgroundFocus::Cancel,
    ];

    /// Advances to the next focusable control.
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&f| f == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// Moves to the previous focusable control.
    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&f| f == self).unwrap_or(0);
        if idx == 0 {
            Self::ALL[Self::ALL.len() - 1]
        } else {
            Self::ALL[idx - 1]
        }
    }
}

/// Resulting user action produced by the background configuration dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundConfigAction {
    /// Dialog remains open without external action
    None,
    /// Request to open the filesystem file picker browser
    Browse,
    /// Confirmed configuration changes to apply to the active theme
    Apply(WallpaperConfig),
    /// Request to remove the background wallpaper completely
    Remove,
    /// Cancel and close dialog without making changes
    Cancel,
}

/// Available wallpaper scaling algorithms matching Python parity.
pub const SCALING_OPTIONS: [(&str, &str); 5] = [
    ("cover", "cover (fill window)"),
    ("stretch", "stretch (exact fit)"),
    ("fit", "fit (proportional)"),
    ("center", "center (original size)"),
    ("tile", "tile (repeat pattern)"),
];

/// Available wallpaper rendering modes matching Python parity.
pub const MODE_OPTIONS: [(&str, &str); 2] = [
    ("halfblock", "halfblock (▀▄ dual-pixel)"),
    ("quarterblock", "quarterblock (▖▗ quad-pixel)"),
];

/// Interactive modal dialog for configuring wallpaper image and background
/// options.
#[derive(Debug, Clone)]
pub struct BackgroundConfigModal {
    /// Dialog title header
    pub title: String,
    /// Currently focused control
    pub focus: BackgroundFocus,
    /// Text input buffer for image file path
    pub image_input: TextInput,
    /// Text input buffer for opacity (0.0 to 1.0)
    pub opacity_input: TextInput,
    /// Selected scaling mode index into `SCALING_OPTIONS`
    pub scaling_idx: usize,
    /// Selected rendering mode index into `MODE_OPTIONS`
    pub mode_idx: usize,
    /// Text input buffer for color clustering threshold
    pub clustering_input: TextInput,
    /// Text input buffer for palette quantization step
    pub quantization_input: TextInput,
}

impl BackgroundConfigModal {
    /// Creates a new background configuration modal pre-populated from
    /// `WallpaperConfig`.
    pub fn new(current_config: Option<&WallpaperConfig>) -> Self {
        let default_cfg = WallpaperConfig::default();
        let cfg = current_config.unwrap_or(&default_cfg);

        let initial_img = cfg.image.as_deref().unwrap_or("");
        let initial_op = format!("{:.2}", cfg.opacity.clamp(0.0, 1.0));
        let cur_scaling = cfg.scaling.to_lowercase();
        let scaling_idx = SCALING_OPTIONS
            .iter()
            .position(|(val, _)| *val == cur_scaling)
            .unwrap_or(0);

        let cur_mode = cfg.mode.as_deref().unwrap_or("halfblock").to_lowercase();
        let mode_idx = MODE_OPTIONS
            .iter()
            .position(|(val, _)| *val == cur_mode)
            .unwrap_or(0);

        let initial_clustering = format!("{}", cfg.clustering);
        let initial_quantization = format!("{}", cfg.quantization.max(1));

        Self {
            title: "Configure Background Wallpaper".to_string(),
            focus: BackgroundFocus::Image,
            image_input: TextInput::new(initial_img),
            opacity_input: TextInput::new(&initial_op),
            scaling_idx,
            mode_idx,
            clustering_input: TextInput::new(&initial_clustering),
            quantization_input: TextInput::new(&initial_quantization),
        }
    }

    /// Sets the initial focus control field for the modal dialog.
    pub fn with_focus(mut self, focus: BackgroundFocus) -> Self {
        self.focus = focus;
        self
    }

    /// Returns the currently focused interactive control field.
    pub fn focus(&self) -> BackgroundFocus {
        self.focus
    }

    /// Sets the wallpaper image file path input value.
    pub fn set_image_path(&mut self, path: &str) {
        self.image_input.set_text(path);
    }

    /// Cycle to the next scaling option.
    pub fn next_scaling(&mut self) {
        self.scaling_idx = (self.scaling_idx + 1) % SCALING_OPTIONS.len();
    }

    /// Cycle to the previous scaling option.
    pub fn prev_scaling(&mut self) {
        if self.scaling_idx == 0 {
            self.scaling_idx = SCALING_OPTIONS.len() - 1;
        } else {
            self.scaling_idx -= 1;
        }
    }

    /// Cycle to the next rendering mode option.
    pub fn next_mode(&mut self) {
        self.mode_idx = (self.mode_idx + 1) % MODE_OPTIONS.len();
    }

    /// Cycle to the previous rendering mode option.
    pub fn prev_mode(&mut self) {
        if self.mode_idx == 0 {
            self.mode_idx = MODE_OPTIONS.len() - 1;
        } else {
            self.mode_idx -= 1;
        }
    }

    /// Constructs the final `WallpaperConfig` based on the dialog's current
    /// inputs.
    pub fn build_config(&self) -> WallpaperConfig {
        let img = self.image_input.as_string().trim().to_string();
        let opacity = self
            .opacity_input
            .as_string()
            .trim()
            .parse::<f32>()
            .unwrap_or(0.85)
            .clamp(0.0, 1.0);
        let scaling = SCALING_OPTIONS[self.scaling_idx].0.to_string();
        let mode = Some(MODE_OPTIONS[self.mode_idx].0.to_string());
        let clustering = self
            .clustering_input
            .as_string()
            .trim()
            .parse::<usize>()
            .unwrap_or(0);
        let quantization = self
            .quantization_input
            .as_string()
            .trim()
            .parse::<usize>()
            .unwrap_or(1)
            .max(1);

        WallpaperConfig {
            enabled: !img.is_empty(),
            image: if img.is_empty() { None } else { Some(img) },
            opacity,
            scaling,
            clustering,
            quantization,
            mode,
        }
    }

    /// Handles keyboard input with explicit modifier keys.
    pub fn handle_key_with_modifiers(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> BackgroundConfigAction {
        let is_shift = modifiers.contains(KeyModifiers::SHIFT);
        let is_ctrl = modifiers.contains(KeyModifiers::CONTROL);

        if is_ctrl {
            match code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    return BackgroundConfigAction::Apply(self.build_config());
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Esc => BackgroundConfigAction::Cancel,
            KeyCode::Tab => {
                self.focus = if is_shift {
                    self.focus.prev()
                } else {
                    self.focus.next()
                };
                BackgroundConfigAction::None
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                BackgroundConfigAction::None
            }
            KeyCode::Down => {
                self.focus = self.focus.next();
                BackgroundConfigAction::None
            }
            KeyCode::Up => {
                self.focus = self.focus.prev();
                BackgroundConfigAction::None
            }
            KeyCode::Left => match self.focus {
                BackgroundFocus::Scaling => {
                    self.prev_scaling();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Mode => {
                    self.prev_mode();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Image => {
                    self.image_input.handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Opacity => {
                    self.opacity_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Clustering => {
                    self.clustering_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Quantization => {
                    self.quantization_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                _ => BackgroundConfigAction::None,
            },
            KeyCode::Right => match self.focus {
                BackgroundFocus::Scaling => {
                    self.next_scaling();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Mode => {
                    self.next_mode();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Image => {
                    self.image_input.handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Opacity => {
                    self.opacity_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Clustering => {
                    self.clustering_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Quantization => {
                    self.quantization_input
                        .handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                _ => BackgroundConfigAction::None,
            },
            KeyCode::Enter => match self.focus {
                BackgroundFocus::Browse => BackgroundConfigAction::Browse,
                BackgroundFocus::Apply => BackgroundConfigAction::Apply(self.build_config()),
                BackgroundFocus::Remove => BackgroundConfigAction::Remove,
                BackgroundFocus::Cancel => BackgroundConfigAction::Cancel,
                BackgroundFocus::Scaling => {
                    self.next_scaling();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Mode => {
                    self.next_mode();
                    BackgroundConfigAction::None
                }
                _ => BackgroundConfigAction::Apply(self.build_config()),
            },
            KeyCode::Char(' ') => match self.focus {
                BackgroundFocus::Browse => BackgroundConfigAction::Browse,
                BackgroundFocus::Apply => BackgroundConfigAction::Apply(self.build_config()),
                BackgroundFocus::Remove => BackgroundConfigAction::Remove,
                BackgroundFocus::Cancel => BackgroundConfigAction::Cancel,
                BackgroundFocus::Scaling => {
                    self.next_scaling();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Mode => {
                    self.next_mode();
                    BackgroundConfigAction::None
                }
                BackgroundFocus::Image => {
                    self.image_input.handle_key_with_modifiers(code, modifiers);
                    BackgroundConfigAction::None
                }
                _ => BackgroundConfigAction::None,
            },
            _ => {
                match self.focus {
                    BackgroundFocus::Image => {
                        self.image_input.handle_key_with_modifiers(code, modifiers);
                    }
                    BackgroundFocus::Opacity => {
                        self.opacity_input
                            .handle_key_with_modifiers(code, modifiers);
                    }
                    BackgroundFocus::Clustering => {
                        self.clustering_input
                            .handle_key_with_modifiers(code, modifiers);
                    }
                    BackgroundFocus::Quantization => {
                        self.quantization_input
                            .handle_key_with_modifiers(code, modifiers);
                    }
                    _ => {}
                }
                BackgroundConfigAction::None
            }
        }
    }

    /// Handles keyboard input without explicit modifiers.
    pub fn handle_key(&mut self, code: KeyCode) -> BackgroundConfigAction {
        self.handle_key_with_modifiers(code, KeyModifiers::NONE)
    }

    /// Handles mouse clicks on dialog controls.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> BackgroundConfigAction {
        let (inner_area, _, _) = self.compute_layout(area);

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let row = mouse.row;
            let col = mouse.column;

            if super::frame::is_close_button_clicked(inner_area, row, col) {
                return BackgroundConfigAction::Cancel;
            }

            let y_start = inner_area.y + 1;

            // Row 0: Wallpaper Image File label
            // Row 1: Image Input + Browse button
            if row == y_start + 1 {
                let browse_x = inner_area.x + inner_area.width.saturating_sub(15);
                if col >= browse_x && col < browse_x + 13 {
                    self.focus = BackgroundFocus::Browse;
                    return BackgroundConfigAction::Browse;
                } else if col >= inner_area.x + 2 && col < browse_x {
                    self.focus = BackgroundFocus::Image;
                    return BackgroundConfigAction::None;
                }
            }

            // Row 2: Opacity label + Scaling label
            // Row 3: Opacity Input + Scaling Selector
            if row == y_start + 3 {
                let half_w = inner_area.width / 2;
                if col < inner_area.x + half_w {
                    self.focus = BackgroundFocus::Opacity;
                } else {
                    self.focus = BackgroundFocus::Scaling;
                    self.next_scaling();
                }
                return BackgroundConfigAction::None;
            }

            // Row 4: Mode label + Clustering label
            // Row 5: Mode Selector + Clustering Input
            if row == y_start + 5 {
                let half_w = inner_area.width / 2;
                if col < inner_area.x + half_w {
                    self.focus = BackgroundFocus::Mode;
                    self.next_mode();
                } else {
                    self.focus = BackgroundFocus::Clustering;
                }
                return BackgroundConfigAction::None;
            }

            // Row 6: Quantization label
            // Row 7: Quantization Input
            if row == y_start + 7 {
                self.focus = BackgroundFocus::Quantization;
                return BackgroundConfigAction::None;
            }

            // Row 9: Buttons row: [ Apply ]  [ Remove Wallpaper ]  [ Cancel ]
            let btn_y = inner_area.y + inner_area.height.saturating_sub(2);
            if row == btn_y {
                let cx = col;
                // Roughly 3 buttons spread across width
                let w = inner_area.width;
                let b1_x = inner_area.x + 4;
                let b1_end = b1_x + 11; // [ Apply ]
                let b2_x = inner_area.x + (w.saturating_sub(24)) / 2;
                let b2_end = b2_x + 22; // [ Remove Wallpaper ]
                let b3_x = inner_area.x + w.saturating_sub(16);
                let b3_end = b3_x + 12; // [ Cancel ]

                if cx >= b1_x && cx <= b1_end {
                    self.focus = BackgroundFocus::Apply;
                    return BackgroundConfigAction::Apply(self.build_config());
                } else if cx >= b2_x && cx <= b2_end {
                    self.focus = BackgroundFocus::Remove;
                    return BackgroundConfigAction::Remove;
                } else if cx >= b3_x && cx <= b3_end {
                    self.focus = BackgroundFocus::Cancel;
                    return BackgroundConfigAction::Cancel;
                }
            }
        }

        BackgroundConfigAction::None
    }

    /// Computes the modal inner rect and dimensions.
    fn compute_layout(&self, area: Rect) -> (Rect, u16, u16) {
        let dialog_w = 68.min(area.width.saturating_sub(4));
        let dialog_h = 16.min(area.height.saturating_sub(2));
        let inner = super::frame::compute_modal_inner(area, dialog_w, dialog_h);
        (inner, dialog_w, dialog_h)
    }

    /// Renders the background configuration dialog into `buf`.
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) {
        let (inner_area, dialog_w, dialog_h) = self.compute_layout(area);
        render_modal_frame(area, dialog_w, dialog_h, &self.title, styles, buf);

        let label_style = Style::default()
            .fg(styles.accent.fg.unwrap_or(Color::Cyan))
            .add_modifier(Modifier::BOLD);
        let normal_input_style = Style::default()
            .fg(styles.text.fg.unwrap_or(Color::White))
            .bg(Color::Rgb(35, 38, 48));
        let focused_input_style = Style::default()
            .fg(Color::Black)
            .bg(styles.highlight.bg.unwrap_or(Color::Yellow))
            .add_modifier(Modifier::BOLD);
        let btn_normal = Style::default()
            .fg(styles.text.fg.unwrap_or(Color::White))
            .bg(Color::Rgb(45, 48, 58));
        let btn_focused = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let btn_danger = Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(160, 40, 40));
        let btn_danger_focused = Style::default()
            .fg(Color::Black)
            .bg(Color::LightRed)
            .add_modifier(Modifier::BOLD);

        let y_start = inner_area.y + 1;
        let left_x = inner_area.x + 2;
        let avail_w = inner_area.width.saturating_sub(4) as usize;

        // 1. Wallpaper Image Path + Browse Button
        buf.set_string(left_x, y_start, "Wallpaper Image File:", label_style);
        let browse_w = 13_u16;
        let browse_x = inner_area.x + inner_area.width.saturating_sub(browse_w + 2);
        let inp_w = browse_x.saturating_sub(left_x + 1) as usize;

        render_text_input_field(
            buf,
            left_x,
            y_start + 1,
            inp_w,
            &self.image_input,
            self.focus == BackgroundFocus::Image,
            normal_input_style,
            focused_input_style,
            Some("(None / disabled)"),
        );

        let browse_st = if self.focus == BackgroundFocus::Browse {
            btn_focused
        } else {
            btn_normal
        };
        buf.set_string(browse_x, y_start + 1, "[ Browse... ]", browse_st);

        // 2. Opacity and Scaling Mode (Split columns)
        let col_w = (avail_w.saturating_sub(2)) / 2;
        let col2_x = left_x + col_w as u16 + 2;

        buf.set_string(left_x, y_start + 2, "Opacity (0.0 to 1.0):", label_style);
        buf.set_string(col2_x, y_start + 2, "Scaling Mode:", label_style);

        render_text_input_field(
            buf,
            left_x,
            y_start + 3,
            col_w,
            &self.opacity_input,
            self.focus == BackgroundFocus::Opacity,
            normal_input_style,
            focused_input_style,
            None,
        );

        let scaling_label = SCALING_OPTIONS[self.scaling_idx].1;
        let scaling_disp = format!(
            "◀ {:^width$} ▶",
            scaling_label,
            width = col_w.saturating_sub(4)
        );
        let sc_st = if self.focus == BackgroundFocus::Scaling {
            btn_focused
        } else {
            btn_normal
        };
        buf.set_string(
            col2_x,
            y_start + 3,
            &scaling_disp[..scaling_disp.len().min(col_w)],
            sc_st,
        );

        // 3. Rendering Mode and Clustering (Split columns)
        buf.set_string(left_x, y_start + 4, "Rendering Mode:", label_style);
        buf.set_string(
            col2_x,
            y_start + 4,
            "Clustering (0=none, 16-128):",
            label_style,
        );

        let mode_label = MODE_OPTIONS[self.mode_idx].1;
        let mode_disp = format!(
            "◀ {:^width$} ▶",
            mode_label,
            width = col_w.saturating_sub(4)
        );
        let md_st = if self.focus == BackgroundFocus::Mode {
            btn_focused
        } else {
            btn_normal
        };
        buf.set_string(
            left_x,
            y_start + 5,
            &mode_disp[..mode_disp.len().min(col_w)],
            md_st,
        );

        render_text_input_field(
            buf,
            col2_x,
            y_start + 5,
            col_w,
            &self.clustering_input,
            self.focus == BackgroundFocus::Clustering,
            normal_input_style,
            focused_input_style,
            None,
        );

        // 4. Palette Quantization
        buf.set_string(
            left_x,
            y_start + 6,
            "Palette Quantization (1=off, 16-256):",
            label_style,
        );
        render_text_input_field(
            buf,
            left_x,
            y_start + 7,
            col_w,
            &self.quantization_input,
            self.focus == BackgroundFocus::Quantization,
            normal_input_style,
            focused_input_style,
            None,
        );

        // 5. Action Buttons Row: [ Apply ]   [ Remove Wallpaper ]   [ Cancel ]
        let btn_y = inner_area.y + inner_area.height.saturating_sub(2);
        let w = inner_area.width;
        let b1_x = inner_area.x + 4;
        let b2_x = inner_area.x + (w.saturating_sub(24)) / 2;
        let b3_x = inner_area.x + w.saturating_sub(16);

        let b_apply_st = if self.focus == BackgroundFocus::Apply {
            btn_focused
        } else {
            btn_normal
        };
        let b_remove_st = if self.focus == BackgroundFocus::Remove {
            btn_danger_focused
        } else {
            btn_danger
        };
        let b_cancel_st = if self.focus == BackgroundFocus::Cancel {
            btn_focused
        } else {
            btn_normal
        };

        buf.set_string(b1_x, btn_y, "[ Apply ]", b_apply_st);
        buf.set_string(b2_x, btn_y, "[ Remove Wallpaper ]", b_remove_st);
        buf.set_string(b3_x, btn_y, "[ Cancel ]", b_cancel_st);
    }
}

/// Helper to render an interactive TextInput field with an inverted cursor
/// cell when focused.
#[allow(clippy::too_many_arguments)]
fn render_text_input_field(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: usize,
    input: &TextInput,
    is_focused: bool,
    normal_style: Style,
    focused_style: Style,
    placeholder: Option<&str>,
) {
    if width == 0 {
        return;
    }
    let raw_text = input.as_string();
    if raw_text.is_empty() && !is_focused {
        let ph = placeholder.unwrap_or("");
        let padded = format!("{:<w$}", ph, w = width);
        buf.set_string(x, y, &padded[..padded.len().min(width)], normal_style);
        return;
    }

    let chars: Vec<char> = raw_text.chars().collect();
    let cursor = input.cursor.min(chars.len());
    let scroll = input.scroll_for_width(width);

    let vis_chars: Vec<char> = chars.iter().skip(scroll).take(width).copied().collect();
    let relative_cursor = cursor.saturating_sub(scroll);

    for i in 0..width {
        let ch = if i < vis_chars.len() {
            vis_chars[i]
        } else {
            ' '
        };
        let cell_style = if is_focused {
            if i == relative_cursor {
                focused_style.add_modifier(Modifier::REVERSED)
            } else {
                focused_style
            }
        } else {
            normal_style
        };

        let cell = &mut buf[(x + i as u16, y)];
        cell.set_char(ch);
        cell.set_style(cell_style);
    }
}
