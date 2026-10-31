//! Half-block TrueColor wallpaper compositing engine.
//!
//! Provides background image loading, aspect-ratio scaling (cover, fit, stretch,
//! center, tile), color blending with theme backgrounds, and half-block (`▀`)
//! TrueColor compositing across Ratatui terminal buffers.

use bashmenu_core::{interpolate_placeholders, MacroContext, WallpaperConfig};
use image::{imageops, Rgb, RgbImage};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::path::{Path, PathBuf};

/// Converts a Ratatui `Color` to an approximate RGB triplet `(r, g, b)`.
pub fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(idx) => indexed_to_rgb(idx),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray | Color::DarkGray => (128, 128, 128),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (229, 229, 229),
        Color::Reset => (15, 20, 32),
    }
}

/// Converts an 8-bit ANSI 256-color index to 24-bit TrueColor RGB.
fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        // Standard 16 colors
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        // 6x6x6 color cube (indices 16..231)
        16..=231 => {
            let offset = idx - 16;
            let r_idx = (offset / 36) % 6;
            let g_idx = (offset / 6) % 6;
            let b_idx = offset % 6;
            let steps = [0u8, 95, 175, 215, 255, 255];
            (
                steps[r_idx as usize],
                steps[g_idx as usize],
                steps[b_idx as usize],
            )
        }
        // 24 grayscale steps (indices 232..255)
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}

/// Resolves an image path by expanding placeholders, tildes, and relative directories.
pub fn resolve_wallpaper_path(path_str: &str) -> Option<PathBuf> {
    if path_str.trim().is_empty() {
        return None;
    }

    // Expand macros like {bashmenu_dir} if present
    let cfg = bashmenu_core::BashMenuConfig::default();
    let macro_ctx = MacroContext::new(&cfg);
    let expanded = interpolate_placeholders(path_str.trim(), &macro_ctx);

    // Expand ~ home directory
    let expanded_path = if let Some(rest) = expanded.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/{}", home, rest)
        } else {
            expanded
        }
    } else {
        expanded
    };

    let p = Path::new(&expanded_path);
    if p.is_file() {
        return Some(p.to_path_buf());
    }

    // Check candidate directories for relative paths
    let mut candidates = Vec::new();

    if let Ok(bdir) = std::env::var("BASHMENU_DIR") {
        candidates.push(PathBuf::from(bdir).join(&expanded_path));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(&expanded_path));
        if let Some(p1) = cwd.parent() {
            candidates.push(p1.join(&expanded_path));
            if let Some(p2) = p1.parent() {
                candidates.push(p2.join(&expanded_path));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".config/bashmenu")
                .join(&expanded_path),
        );
    }
    candidates.push(PathBuf::from("/opt/bashmenu").join(&expanded_path));
    candidates.push(PathBuf::from("/usr/local/share/bashmenu").join(&expanded_path));

    candidates.into_iter().find(|cand| cand.is_file())
}

/// Pre-calculated TrueColor values for a single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallpaperCell {
    /// Upper half-pixel foreground color
    pub upper: Color,
    /// Lower half-pixel background color
    pub lower: Color,
    /// Blended average background color for overlaid characters
    pub avg_bg: Color,
}

/// 2D grid of pre-calculated wallpaper cell colors matching terminal dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<WallpaperCell>,
}

impl WallpaperGrid {
    /// Get cell colors at terminal column `x` and row `y`.
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Option<&WallpaperCell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y * self.width + x])
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositorCacheKey {
    image: String,
    width: usize,
    height: usize,
    theme_bg_rgb: (u8, u8, u8),
    opacity: u32,
    scaling: String,
    quantization: usize,
}

/// Manages loading, caching, aspect-ratio scaling, and color blending of wallpapers.
#[derive(Clone, Default)]
pub struct WallpaperCompositor {
    pub config: WallpaperConfig,
    cached_path: Option<PathBuf>,
    raw_image: Option<RgbImage>,
    cached_grid: Option<WallpaperGrid>,
    cached_key: Option<CompositorCacheKey>,
}

impl WallpaperCompositor {
    /// Creates a new compositor with the specified configuration.
    pub fn new(config: WallpaperConfig) -> Self {
        Self {
            config,
            cached_path: None,
            raw_image: None,
            cached_grid: None,
            cached_key: None,
        }
    }

    /// Update active wallpaper configuration and invalidate cache if parameters changed.
    pub fn set_config(&mut self, config: WallpaperConfig) {
        if self.config != config {
            self.config = config;
            self.cached_key = None;
            self.cached_grid = None;
        }
    }

    /// Ensures the source image is loaded from disk into memory.
    fn ensure_image_loaded(&mut self) -> bool {
        let Some(img_str) = &self.config.image else {
            self.raw_image = None;
            self.cached_path = None;
            return false;
        };

        let Some(resolved) = resolve_wallpaper_path(img_str) else {
            self.raw_image = None;
            self.cached_path = None;
            return false;
        };

        if self.raw_image.is_some() && self.cached_path.as_deref() == Some(&resolved) {
            return true;
        }

        match image::open(&resolved) {
            Ok(dyn_img) => {
                let rgb = dyn_img.to_rgb8();
                // Downsample excessively large 4K/8K images for performance
                let max_w = 1920;
                let max_h = 1080;
                let final_img = if rgb.width() > max_w || rgb.height() > max_h {
                    imageops::thumbnail(&rgb, max_w, max_h)
                } else {
                    rgb
                };
                self.cached_path = Some(resolved);
                self.raw_image = Some(final_img);
                true
            }
            Err(_) => {
                self.raw_image = None;
                self.cached_path = None;
                false
            }
        }
    }

    /// Retrieve or compute 2D cell grid for the given terminal dimensions and theme background.
    pub fn get_grid(
        &mut self,
        width: usize,
        height: usize,
        theme_bg: Color,
    ) -> Option<&WallpaperGrid> {
        if !self.config.enabled || width == 0 || height == 0 {
            return None;
        }

        if !self.ensure_image_loaded() {
            return None;
        }

        let raw = self.raw_image.as_ref()?;
        let theme_bg_rgb = color_to_rgb(theme_bg);
        let op = self.config.opacity.clamp(0.0, 1.0);
        let op_int = (op * 1000.0) as u32;
        let scaling = self.config.scaling.to_lowercase();
        let quant = self.config.quantization.max(1);

        let key = CompositorCacheKey {
            image: self.config.image.clone().unwrap_or_default(),
            width,
            height,
            theme_bg_rgb,
            opacity: op_int,
            scaling: scaling.clone(),
            quantization: quant,
        };

        if self.cached_key.as_ref() == Some(&key) && self.cached_grid.is_some() {
            return self.cached_grid.as_ref();
        }

        let pixel_w = width as u32;
        let pixel_h = (height * 2) as u32;

        let scaled = scale_image(raw, pixel_w, pixel_h, &scaling, theme_bg_rgb);

        let inv_op = 1.0 - op;
        let (t_r, t_g, t_b) = (
            theme_bg_rgb.0 as f32,
            theme_bg_rgb.1 as f32,
            theme_bg_rgb.2 as f32,
        );

        let mut cells = Vec::with_capacity(width * height);

        for row in 0..height {
            let y1 = (row * 2) as u32;
            let y2 = y1 + 1;

            for col in 0..width {
                let x = col as u32;
                let p1 = scaled.get_pixel(x, y1);
                let p2 = scaled.get_pixel(x, y2);

                let r1_b = ((p1[0] as f32) * op + t_r * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let g1_b = ((p1[1] as f32) * op + t_g * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let b1_b = ((p1[2] as f32) * op + t_b * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;

                let r2_b = ((p2[0] as f32) * op + t_r * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let g2_b = ((p2[1] as f32) * op + t_g * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let b2_b = ((p2[2] as f32) * op + t_b * inv_op)
                    .round()
                    .clamp(0.0, 255.0) as u8;

                let (r1, g1, b1, r2, g2, b2) = if quant > 1 {
                    let q = quant as u8;
                    (
                        (r1_b / q) * q,
                        (g1_b / q) * q,
                        (b1_b / q) * q,
                        (r2_b / q) * q,
                        (g2_b / q) * q,
                        (b2_b / q) * q,
                    )
                } else {
                    (r1_b, g1_b, b1_b, r2_b, g2_b, b2_b)
                };

                let avg_r = ((r1 as u16 + r2 as u16) / 2) as u8;
                let avg_g = ((g1 as u16 + g2 as u16) / 2) as u8;
                let avg_b = ((b1 as u16 + b2 as u16) / 2) as u8;

                cells.push(WallpaperCell {
                    upper: Color::Rgb(r1, g1, b1),
                    lower: Color::Rgb(r2, g2, b2),
                    avg_bg: Color::Rgb(avg_r, avg_g, avg_b),
                });
            }
        }

        self.cached_grid = Some(WallpaperGrid {
            width,
            height,
            cells,
        });
        self.cached_key = Some(key);

        self.cached_grid.as_ref()
    }
}

/// Scales an RGB image to target dimensions according to the specified scaling mode.
fn scale_image(
    img: &RgbImage,
    target_w: u32,
    target_h: u32,
    scaling: &str,
    bg_rgb: (u8, u8, u8),
) -> RgbImage {
    let img_w = img.width().max(1);
    let img_h = img.height().max(1);

    match scaling {
        "cover" => {
            let img_ratio = img_w as f64 / img_h as f64;
            let target_ratio = target_w as f64 / target_h.max(1) as f64;

            let (crop_x, crop_y, crop_w, crop_h) = if img_ratio > target_ratio {
                let new_w = (img_h as f64 * target_ratio).round() as u32;
                let left = (img_w.saturating_sub(new_w)) / 2;
                (left, 0, new_w.min(img_w), img_h)
            } else {
                let new_h = (img_w as f64 / target_ratio).round() as u32;
                let top = (img_h.saturating_sub(new_h)) / 2;
                (0, top, img_w, new_h.min(img_h))
            };

            let cropped =
                imageops::crop_imm(img, crop_x, crop_y, crop_w.max(1), crop_h.max(1)).to_image();
            imageops::resize(&cropped, target_w, target_h, imageops::FilterType::Triangle)
        }
        "fit" => {
            let img_ratio = img_w as f64 / img_h as f64;
            let target_ratio = target_w as f64 / target_h.max(1) as f64;

            let (fit_w, fit_h) = if img_ratio > target_ratio {
                let w = target_w;
                let h = ((target_w as f64 / img_ratio).round() as u32).clamp(1, target_h);
                (w, h)
            } else {
                let h = target_h;
                let w = ((target_h as f64 * img_ratio).round() as u32).clamp(1, target_w);
                (w, h)
            };

            let resized = imageops::resize(img, fit_w, fit_h, imageops::FilterType::Triangle);
            let mut canvas =
                RgbImage::from_pixel(target_w, target_h, Rgb([bg_rgb.0, bg_rgb.1, bg_rgb.2]));
            let paste_x = (target_w.saturating_sub(fit_w)) / 2;
            let paste_y = (target_h.saturating_sub(fit_h)) / 2;
            imageops::overlay(&mut canvas, &resized, paste_x as i64, paste_y as i64);
            canvas
        }
        "center" => {
            let mut canvas =
                RgbImage::from_pixel(target_w, target_h, Rgb([bg_rgb.0, bg_rgb.1, bg_rgb.2]));
            let crop_l = (img_w.saturating_sub(target_w)) / 2;
            let crop_t = (img_h.saturating_sub(target_h)) / 2;
            let crop_w = img_w.min(target_w);
            let crop_h = img_h.min(target_h);
            let cropped = imageops::crop_imm(img, crop_l, crop_t, crop_w, crop_h).to_image();

            let paste_x = (target_w.saturating_sub(crop_w)) / 2;
            let paste_y = (target_h.saturating_sub(crop_h)) / 2;
            imageops::overlay(&mut canvas, &cropped, paste_x as i64, paste_y as i64);
            canvas
        }
        "tile" => {
            let mut canvas =
                RgbImage::from_pixel(target_w, target_h, Rgb([bg_rgb.0, bg_rgb.1, bg_rgb.2]));
            let mut y = 0;
            while y < target_h {
                let mut x = 0;
                while x < target_w {
                    imageops::overlay(&mut canvas, img, x as i64, y as i64);
                    x += img_w;
                }
                y += img_h;
            }
            canvas
        }
        _ => {
            // "stretch" (default fallback)
            imageops::resize(img, target_w, target_h, imageops::FilterType::Triangle)
        }
    }
}

/// Composites the half-block wallpaper onto a Ratatui `Buffer` across the specified `area`.
///
/// Converts padding spaces into `'▀'` half-blocks with dual TrueColor foreground and
/// background, while setting the background of text characters and window borders
/// to the blended underlying cell color.
pub fn composite_buffer(area: Rect, buf: &mut Buffer, grid: &WallpaperGrid, theme_bg: Color) {
    let w = area.width as usize;
    let h = area.height as usize;

    for r in 0..h {
        let gy = r;
        let y = area.y + r as u16;

        // 1. Identify padding cells: runs of 2+ spaces, leading spaces, trailing spaces, or blank rows
        let mut is_padding = vec![false; w];
        let mut start_idx = None;

        for c in 0..w {
            let x = area.x + c as u16;
            let sym = buf[(x, y)].symbol();
            let space = sym == " " || sym.is_empty();

            if space {
                if start_idx.is_none() {
                    start_idx = Some(c);
                }
            } else if let Some(s) = start_idx {
                let len = c - s;
                if len >= 2 || s == 0 {
                    is_padding[s..c].fill(true);
                }
                start_idx = None;
            }
        }
        if let Some(s) = start_idx {
            // Trailing spaces to end of row
            is_padding[s..w].fill(true);
        }

        // 2. Composite cells
        for (c, &padding) in is_padding.iter().enumerate().take(w) {
            let gx = c;
            let x = area.x + c as u16;

            let Some(wp) = grid.get(gx, gy) else {
                continue;
            };

            let cell = &mut buf[(x, y)];

            // Check if cell has an explicit highlight background (e.g. selected menu row or button)
            let is_highlight = cell.bg != theme_bg && cell.bg != Color::Reset;

            if is_highlight {
                // Keep explicit highlight background and foreground intact
                continue;
            }

            if padding {
                // Padding space: render half-block with wallpaper top and bottom pixels
                cell.set_symbol("▀");
                cell.fg = wp.upper;
                cell.bg = wp.lower;
            } else {
                // Text character, border glyph, or intra-word space: set background to blended wallpaper color
                cell.bg = wp.avg_bg;
            }
        }
    }
}
