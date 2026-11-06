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
    candidates.push(bashmenu_core::get_user_config_dir().join(&expanded_path));
    candidates.push(PathBuf::from("/opt/bashmenu").join(&expanded_path));
    candidates.push(PathBuf::from("/usr/local/share/bashmenu").join(&expanded_path));

    candidates.into_iter().find(|cand| cand.is_file())
}

/// Pre-calculated TrueColor values for a single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallpaperCell {
    /// Rendered glyph symbol (e.g. "▀" for halfblock, or quadrant characters
    /// like "▖", "▗", "▞")
    pub symbol: &'static str,
    /// Upper half-pixel or primary foreground color
    pub upper: Color,
    /// Lower half-pixel or secondary background color
    pub lower: Color,
    /// Blended average background color for overlaid characters
    pub avg_bg: Color,
}

impl WallpaperCell {
    /// Constructs a halfblock (`▀`) wallpaper cell with upper foreground
    /// and lower background.
    pub const fn new(upper: Color, lower: Color, avg_bg: Color) -> Self {
        Self {
            symbol: "▀",
            upper,
            lower,
            avg_bg,
        }
    }
}

/// 2D grid of pre-calculated wallpaper cell colors matching terminal dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperGrid {
    /// Visual column width of the grid
    pub width: usize,
    /// Terminal row height of the grid
    pub height: usize,
    /// Linear buffer of cells indexed as `y * width + x`
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

/// Composite cache lookup key matching texture dimensions, background color,
/// and scaling parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositorCacheKey {
    /// Image source path or identifier
    image: String,
    /// Terminal viewport column width
    width: usize,
    /// Terminal viewport row height
    height: usize,
    /// RGB color components of the container theme background
    theme_bg_rgb: (u8, u8, u8),
    /// Fixed-point opacity factor representation
    opacity: u32,
    /// Wallpaper scaling algorithm ("cover", "stretch", "fit", "center", "tile")
    scaling: String,
    /// Color quantization step
    quantization: usize,
    /// Color clustering distance threshold
    clustering: usize,
    /// Rendering mode ("halfblock" or "quarterblock")
    mode: String,
}

/// Manages loading, caching, aspect-ratio scaling, and color blending of wallpapers.
#[derive(Clone, Default)]
pub struct WallpaperCompositor {
    /// Active wallpaper configuration settings
    pub config: WallpaperConfig,
    /// Cached filesystem path of the loaded wallpaper image
    cached_path: Option<PathBuf>,
    /// Decoded source RGB image texture
    raw_image: Option<RgbImage>,
    /// Cached pre-rendered wallpaper cell grid
    cached_grid: Option<WallpaperGrid>,
    /// Cache validation key matching current dimensions, opacity, and theme background
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
        let clustering = self.config.clustering;
        let mode = self
            .config
            .mode
            .as_deref()
            .unwrap_or("halfblock")
            .to_lowercase();

        let key = CompositorCacheKey {
            image: self.config.image.clone().unwrap_or_default(),
            width,
            height,
            theme_bg_rgb,
            opacity: op_int,
            scaling: scaling.clone(),
            quantization: quant,
            clustering,
            mode: mode.clone(),
        };

        if self.cached_key.as_ref() == Some(&key) && self.cached_grid.is_some() {
            return self.cached_grid.as_ref();
        }

        let is_quarterblock = mode == "quarterblock";
        let pixel_w = if is_quarterblock {
            (width * 2) as u32
        } else {
            width as u32
        };
        let pixel_h = (height * 2) as u32;

        let scaled = scale_image(raw, pixel_w, pixel_h, &scaling, theme_bg_rgb);

        let inv_op = 1.0 - op;
        let (t_r, t_g, t_b) = (
            theme_bg_rgb.0 as f32,
            theme_bg_rgb.1 as f32,
            theme_bg_rgb.2 as f32,
        );

        let blend_pixel = |p: &Rgb<u8>| -> (u8, u8, u8) {
            let r = ((p[0] as f32) * op + t_r * inv_op)
                .round()
                .clamp(0.0, 255.0) as u8;
            let g = ((p[1] as f32) * op + t_g * inv_op)
                .round()
                .clamp(0.0, 255.0) as u8;
            let b = ((p[2] as f32) * op + t_b * inv_op)
                .round()
                .clamp(0.0, 255.0) as u8;
            if quant > 1 {
                let q = quant as u8;
                ((r / q) * q, (g / q) * q, (b / q) * q)
            } else {
                (r, g, b)
            }
        };

        let mut cells = Vec::with_capacity(width * height);

        const QUADRANT_GLYPHS: [&str; 16] = [
            " ", "▘", "▝", "▀", "▖", "▌", "▞", "▛", "▗", "▚", "▐", "▜", "▄", "▙", "▟", "█",
        ];

        for row in 0..height {
            let y1 = (row * 2) as u32;
            let y2 = y1 + 1;

            let mut anchor_cell: Option<WallpaperCell> = None;
            let mut anchor_k: Option<(u8, u8, u8, u8, u8, u8)> = None;
            let mut cluster_run = 0;

            for col in 0..width {
                let (cell, k) = if is_quarterblock {
                    let x1 = (col * 2) as u32;
                    let x2 = x1 + 1;
                    let (r_tl, g_tl, b_tl) = blend_pixel(scaled.get_pixel(x1, y1));
                    let (r_tr, g_tr, b_tr) = blend_pixel(scaled.get_pixel(x2, y1));
                    let (r_bl, g_bl, b_bl) = blend_pixel(scaled.get_pixel(x1, y2));
                    let (r_br, g_br, b_br) = blend_pixel(scaled.get_pixel(x2, y2));

                    let avg_r = ((r_tl as u16 + r_tr as u16 + r_bl as u16 + r_br as u16) / 4) as u8;
                    let avg_g = ((g_tl as u16 + g_tr as u16 + g_bl as u16 + g_br as u16) / 4) as u8;
                    let avg_b = ((b_tl as u16 + b_tr as u16 + b_bl as u16 + b_br as u16) / 4) as u8;

                    let lum_tl = 0.299 * r_tl as f32 + 0.587 * g_tl as f32 + 0.114 * b_tl as f32;
                    let lum_tr = 0.299 * r_tr as f32 + 0.587 * g_tr as f32 + 0.114 * b_tr as f32;
                    let lum_bl = 0.299 * r_bl as f32 + 0.587 * g_bl as f32 + 0.114 * b_bl as f32;
                    let lum_br = 0.299 * r_br as f32 + 0.587 * g_br as f32 + 0.114 * b_br as f32;
                    let mean_lum = (lum_tl + lum_tr + lum_bl + lum_br) / 4.0;

                    let mut mask = 0usize;
                    let mut fg_count = 0u16;
                    let mut fg_sum = (0u16, 0u16, 0u16);
                    let mut bg_count = 0u16;
                    let mut bg_sum = (0u16, 0u16, 0u16);

                    let mut add_subpixel = |lum: f32, r: u8, g: u8, b: u8, bit: usize| {
                        if lum >= mean_lum {
                            mask |= bit;
                            fg_count += 1;
                            fg_sum.0 += r as u16;
                            fg_sum.1 += g as u16;
                            fg_sum.2 += b as u16;
                        } else {
                            bg_count += 1;
                            bg_sum.0 += r as u16;
                            bg_sum.1 += g as u16;
                            bg_sum.2 += b as u16;
                        }
                    };

                    add_subpixel(lum_tl, r_tl, g_tl, b_tl, 1);
                    add_subpixel(lum_tr, r_tr, g_tr, b_tr, 2);
                    add_subpixel(lum_bl, r_bl, g_bl, b_bl, 4);
                    add_subpixel(lum_br, r_br, g_br, b_br, 8);

                    let fg = match std::num::NonZeroU16::new(fg_count) {
                        Some(div) => Color::Rgb(
                            (fg_sum.0 / div.get()) as u8,
                            (fg_sum.1 / div.get()) as u8,
                            (fg_sum.2 / div.get()) as u8,
                        ),
                        None => Color::Rgb(avg_r, avg_g, avg_b),
                    };

                    let bg = match std::num::NonZeroU16::new(bg_count) {
                        Some(div) => Color::Rgb(
                            (bg_sum.0 / div.get()) as u8,
                            (bg_sum.1 / div.get()) as u8,
                            (bg_sum.2 / div.get()) as u8,
                        ),
                        None => Color::Rgb(avg_r, avg_g, avg_b),
                    };

                    let c = WallpaperCell {
                        symbol: QUADRANT_GLYPHS[mask],
                        upper: fg,
                        lower: bg,
                        avg_bg: Color::Rgb(avg_r, avg_g, avg_b),
                    };
                    let (fg_r, fg_g, fg_b) = color_to_rgb(fg);
                    let (bg_r, bg_g, bg_b) = color_to_rgb(bg);
                    (c, (fg_r, fg_g, fg_b, bg_r, bg_g, bg_b))
                } else {
                    let x = col as u32;
                    let (r1, g1, b1) = blend_pixel(scaled.get_pixel(x, y1));
                    let (r2, g2, b2) = blend_pixel(scaled.get_pixel(x, y2));

                    let avg_r = ((r1 as u16 + r2 as u16) / 2) as u8;
                    let avg_g = ((g1 as u16 + g2 as u16) / 2) as u8;
                    let avg_b = ((b1 as u16 + b2 as u16) / 2) as u8;

                    let c = WallpaperCell {
                        symbol: "▀",
                        upper: Color::Rgb(r1, g1, b1),
                        lower: Color::Rgb(r2, g2, b2),
                        avg_bg: Color::Rgb(avg_r, avg_g, avg_b),
                    };
                    (c, (r1, g1, b1, r2, g2, b2))
                };

                if clustering > 0 && cluster_run < 4 {
                    if let (Some(anch), Some(ak)) = (anchor_cell, anchor_k) {
                        let delta = (k.0 as i32 - ak.0 as i32).abs()
                            + (k.1 as i32 - ak.1 as i32).abs()
                            + (k.2 as i32 - ak.2 as i32).abs()
                            + (k.3 as i32 - ak.3 as i32).abs()
                            + (k.4 as i32 - ak.4 as i32).abs()
                            + (k.5 as i32 - ak.5 as i32).abs();
                        if delta as usize <= clustering {
                            cluster_run += 1;
                            cells.push(anch);
                            continue;
                        }
                    }
                }

                cluster_run = 0;
                anchor_cell = Some(cell);
                anchor_k = Some(k);
                cells.push(cell);
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
                // Padding space: render wallpaper glyph with upper/fg and lower/bg colors
                cell.set_symbol(wp.symbol);
                cell.fg = wp.upper;
                cell.bg = wp.lower;
            } else {
                // Text character, border glyph, or intra-word space: set background to blended wallpaper color
                cell.bg = wp.avg_bg;
            }
        }
    }
}

/// Converts an RGB triplet to the nearest xterm-256 palette index (16..=255 preferred for accuracy).
pub fn rgb_to_xterm256(r: u8, g: u8, b: u8) -> u8 {
    let mut best_idx = 16u8;
    let mut best_dist = u32::MAX;
    for idx in 16..=255u8 {
        let (pr, pg, pb) = indexed_to_rgb(idx);
        let dr = r as i32 - pr as i32;
        let dg = g as i32 - pg as i32;
        let db = b as i32 - pb as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
            if dist == 0 {
                break;
            }
        }
    }
    best_idx
}

/// Converts standard sRGB component values (0-255) to HSV color space:
/// hue in `[0.0, 1.0)`, saturation in `[0.0, 1.0]`, and value/brightness in `[0.0, 1.0]`.
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let h = if delta < 1e-6 {
        0.0
    } else if (max - rf).abs() < 1e-6 {
        ((gf - bf) / delta).rem_euclid(6.0) / 6.0
    } else if (max - gf).abs() < 1e-6 {
        (((bf - rf) / delta) + 2.0) / 6.0
    } else {
        (((rf - gf) / delta) + 4.0) / 6.0
    };
    let s = if max < 1e-6 { 0.0 } else { delta / max };
    (h, s, max)
}

/// Computes the relative perceptual luminance of an sRGB color using standard Rec. 709 coefficients.
fn color_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * (r as f64) + 0.7152 * (g as f64) + 0.0722 * (b as f64)
}

/// Computes the circular angular difference between two normalized hue values in `[0.0, 1.0)`.
fn hue_distance(h1: f64, h2: f64) -> f64 {
    let diff = (h1 - h2).abs();
    diff.min(1.0 - diff)
}

/// Represents an extracted color candidate with ANSI index and HSV properties.
#[derive(Debug, Clone, Copy)]
struct ExtractedColor {
    /// ANSI 256-color palette index.
    idx: u8,
    /// Hue value in `[0.0, 1.0)`
    h: f64,
    /// Saturation value in `[0.0, 1.0]`
    s: f64,
    /// Relative perceptual luminance
    lum: f64,
    /// Pixel frequency count in the source image
    count: u32,
}

/// Auto-derives a harmonious 256-color theme palette (`colors_256` map) from an image file.
pub fn derive_palette_from_image(
    image_path: &Path,
) -> Result<std::collections::HashMap<String, serde_yml::Value>, String> {
    let dyn_img = image::open(image_path)
        .map_err(|e| format!("Failed to open image '{}': {}", image_path.display(), e))?;
    let rgb_img = dyn_img.to_rgb8();
    let thumb = if rgb_img.width() > 160 || rgb_img.height() > 160 {
        imageops::thumbnail(&rgb_img, 160, 160)
    } else {
        rgb_img
    };

    let mut counts = [0u32; 256];
    let mut total_pixels = 0u32;
    for pixel in thumb.pixels() {
        let idx = rgb_to_xterm256(pixel[0], pixel[1], pixel[2]);
        counts[idx as usize] += 1;
        total_pixels += 1;
    }

    if total_pixels == 0 {
        return Err("Image contains no pixels".to_string());
    }

    let mut ranked: Vec<ExtractedColor> = (16..=255u8)
        .filter(|&i| counts[i as usize] > 0)
        .map(|idx| {
            let (r, g, b) = indexed_to_rgb(idx);
            let (h, s, _) = rgb_to_hsv(r, g, b);
            let lum = color_luminance(r, g, b);
            ExtractedColor {
                idx,
                h,
                s,
                lum,
                count: counts[idx as usize],
            }
        })
        .collect();

    ranked.sort_by_key(|a| std::cmp::Reverse(a.count));
    ranked.truncate(24);

    if ranked.is_empty() {
        return Err("Could not extract any colors from image".to_string());
    }

    // 1. Background color (darkest dominant shade)
    let dark_cands: Vec<ExtractedColor> = ranked.iter().copied().filter(|c| c.lum < 55.0).collect();
    let (bg_color, bg_lum) = if let Some(darkest) = dark_cands.iter().min_by(|a, b| {
        a.lum
            .partial_cmp(&b.lum)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        (darkest.idx, darkest.lum)
    } else {
        (16u8, 0.0)
    };

    // 2. Score candidates for accents (balancing saturation, contrast with background, and prevalence)
    let mut scored: Vec<(ExtractedColor, f64)> = ranked
        .iter()
        .copied()
        .filter(|c| c.idx != bg_color)
        .map(|c| {
            let contrast = (c.lum - bg_lum).abs() / 100.0;
            let freq_bonus = ((c.count as f64) / (total_pixels as f64)).sqrt() * 0.4;
            let score = (c.s * 1.6) + (contrast * 1.2) + freq_bonus;
            (c, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let primary = scored
        .iter()
        .find(|(c, _)| c.lum >= 45.0)
        .or_else(|| scored.first())
        .map(|(c, _)| *c)
        .unwrap_or(ranked[0]);
    let primary_accent = primary.idx;

    let secondary_accent = scored
        .iter()
        .map(|(c, _)| *c)
        .find(|c| {
            c.idx != primary_accent
                && (hue_distance(c.h, primary.h) > 0.08 || (c.lum - primary.lum).abs() > 25.0)
        })
        .or_else(|| {
            scored
                .iter()
                .map(|(c, _)| *c)
                .find(|c| c.idx != primary_accent)
        })
        .map(|c| c.idx)
        .unwrap_or(primary_accent);

    // 3. Borders and structural lines (mid-tones)
    let mut border_cands: Vec<ExtractedColor> = ranked
        .iter()
        .copied()
        .filter(|c| (40.0..=150.0).contains(&c.lum) && c.idx != bg_color)
        .collect();
    border_cands.sort_by(|a, b| {
        (a.lum - 85.0)
            .abs()
            .partial_cmp(&(b.lum - 85.0).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let border_color = border_cands
        .first()
        .map(|c| c.idx)
        .unwrap_or(primary_accent);

    // 4. Dimmer gutter/whitespace tone
    let mut gutter_cands: Vec<ExtractedColor> = ranked
        .iter()
        .copied()
        .filter(|c| (25.0..=100.0).contains(&c.lum) && c.idx != bg_color && c.idx != border_color)
        .collect();
    gutter_cands.sort_by(|a, b| {
        a.lum
            .partial_cmp(&b.lum)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let gutter_color = gutter_cands.first().map(|c| c.idx).unwrap_or(238);

    // 5. Contrast foreground for selection/highlight
    let selection_fg: u8 = if primary.lum > 135.0 { 16 } else { 255 };
    let text_fg: u8 = 255;

    // 6. Semantic alert/button accents from image if available
    let button_error = ranked
        .iter()
        .find(|c| (c.h >= 0.94 || c.h <= 0.06) && c.s >= 0.35)
        .map(|c| c.idx)
        .unwrap_or(196);
    let toast_warn = ranked
        .iter()
        .find(|c| (0.06..=0.20).contains(&c.h) && c.s >= 0.30)
        .map(|c| c.idx)
        .unwrap_or(214);
    let button_success = ranked
        .iter()
        .find(|c| (0.22..=0.44).contains(&c.h) && c.s >= 0.30)
        .map(|c| c.idx)
        .unwrap_or(40);

    let make_pair = |fg: i64, bg: i64| -> serde_yml::Value {
        serde_yml::Value::Sequence(vec![
            serde_yml::Value::Number(fg.into()),
            serde_yml::Value::Number(bg.into()),
        ])
    };

    let mut colors = std::collections::HashMap::new();
    colors.insert(
        "background".to_string(),
        serde_yml::Value::Number((bg_color as i64).into()),
    );
    colors.insert("title".to_string(), make_pair(primary_accent as i64, -1));
    colors.insert("border".to_string(), make_pair(border_color as i64, -1));
    colors.insert("text".to_string(), make_pair(text_fg as i64, -1));
    colors.insert(
        "highlight".to_string(),
        make_pair(selection_fg as i64, primary_accent as i64),
    );
    colors.insert("accent".to_string(), make_pair(secondary_accent as i64, -1));
    colors.insert("footer".to_string(), make_pair(border_color as i64, -1));
    colors.insert("gutter".to_string(), make_pair(gutter_color as i64, -1));
    colors.insert(
        "selection".to_string(),
        make_pair(selection_fg as i64, primary_accent as i64),
    );
    colors.insert(
        "shortcut_key".to_string(),
        make_pair(secondary_accent as i64, -1),
    );
    colors.insert("divider".to_string(), make_pair(border_color as i64, -1));
    colors.insert(
        "button_primary".to_string(),
        make_pair(selection_fg as i64, primary_accent as i64),
    );
    colors.insert(
        "button_error".to_string(),
        make_pair(255, button_error as i64),
    );
    colors.insert(
        "button_cancel".to_string(),
        make_pair(text_fg as i64, gutter_color as i64),
    );
    colors.insert(
        "button_success".to_string(),
        make_pair(255, button_success as i64),
    );
    colors.insert(
        "help_text".to_string(),
        make_pair(secondary_accent as i64, -1),
    );
    colors.insert("plugin".to_string(), make_pair(secondary_accent as i64, -1));
    colors.insert("window_close_button".to_string(), make_pair(203, -1));
    colors.insert(
        "scrollbar".to_string(),
        make_pair(primary_accent as i64, 236),
    );
    colors.insert("shadow".to_string(), make_pair(236, bg_color as i64));
    colors.insert("whitespace".to_string(), make_pair(gutter_color as i64, -1));
    colors.insert(
        "toast_info".to_string(),
        make_pair(primary_accent as i64, -1),
    );
    colors.insert(
        "toast_warning".to_string(),
        make_pair(toast_warn as i64, -1),
    );
    colors.insert(
        "toast_error".to_string(),
        make_pair(button_error as i64, -1),
    );

    Ok(colors)
}

/// Canonical ANSI 16-color palette names.
pub const ANSI16_NAMES: [&str; 16] = [
    "COLOR_BLACK",
    "COLOR_RED",
    "COLOR_GREEN",
    "COLOR_YELLOW",
    "COLOR_BLUE",
    "COLOR_MAGENTA",
    "COLOR_CYAN",
    "COLOR_WHITE",
    "COLOR_GREY",
    "COLOR_BRIGHT_RED",
    "COLOR_BRIGHT_GREEN",
    "COLOR_BRIGHT_YELLOW",
    "COLOR_BRIGHT_BLUE",
    "COLOR_BRIGHT_MAGENTA",
    "COLOR_BRIGHT_CYAN",
    "COLOR_BRIGHT_WHITE",
];

/// Canonical ANSI 8-color palette names.
pub const ANSI8_NAMES: [&str; 8] = [
    "COLOR_BLACK",
    "COLOR_RED",
    "COLOR_GREEN",
    "COLOR_YELLOW",
    "COLOR_BLUE",
    "COLOR_MAGENTA",
    "COLOR_CYAN",
    "COLOR_WHITE",
];

/// Maps an RGB triplet to the closest canonical ANSI 16-color name.
pub fn rgb_to_ansi16_name(r: u8, g: u8, b: u8) -> &'static str {
    let (h, s, v) = rgb_to_hsv(r, g, b);
    let lum = color_luminance(r, g, b);

    if lum < 28.0 {
        return "COLOR_BLACK";
    }
    if s < 0.18 {
        if lum < 70.0 {
            "COLOR_BLACK"
        } else if lum < 150.0 {
            "COLOR_GREY"
        } else if lum < 225.0 {
            "COLOR_WHITE"
        } else {
            "COLOR_BRIGHT_WHITE"
        }
    } else {
        let is_bright = lum >= 115.0 || v >= 0.75;
        if !(0.05..0.95).contains(&h) {
            if is_bright {
                "COLOR_BRIGHT_RED"
            } else {
                "COLOR_RED"
            }
        } else if h < 0.18 {
            if is_bright {
                "COLOR_BRIGHT_YELLOW"
            } else {
                "COLOR_YELLOW"
            }
        } else if h < 0.42 {
            if is_bright {
                "COLOR_BRIGHT_GREEN"
            } else {
                "COLOR_GREEN"
            }
        } else if h < 0.58 {
            if is_bright {
                "COLOR_BRIGHT_CYAN"
            } else {
                "COLOR_CYAN"
            }
        } else if h < 0.75 {
            if is_bright {
                "COLOR_BRIGHT_BLUE"
            } else {
                "COLOR_BLUE"
            }
        } else if is_bright {
            "COLOR_BRIGHT_MAGENTA"
        } else {
            "COLOR_MAGENTA"
        }
    }
}

/// Maps an RGB triplet to the closest canonical ANSI 8-color name.
pub fn rgb_to_ansi8_name(r: u8, g: u8, b: u8) -> &'static str {
    let (h, s, _v) = rgb_to_hsv(r, g, b);
    let lum = color_luminance(r, g, b);

    if lum < 32.0 {
        return "COLOR_BLACK";
    }
    if s < 0.18 {
        if lum >= 110.0 {
            "COLOR_WHITE"
        } else {
            "COLOR_BLACK"
        }
    } else if !(0.05..0.95).contains(&h) {
        "COLOR_RED"
    } else if h < 0.18 {
        "COLOR_YELLOW"
    } else if h < 0.42 {
        "COLOR_GREEN"
    } else if h < 0.58 {
        "COLOR_CYAN"
    } else if h < 0.75 {
        "COLOR_BLUE"
    } else {
        "COLOR_MAGENTA"
    }
}

/// Downsamples an arbitrary color value (sequence, number, hex string,
/// or named token) into a valid ANSI 16-color representation.
pub fn downsample_color_value_to_16(val: &serde_yml::Value) -> serde_yml::Value {
    match val {
        serde_yml::Value::Sequence(seq) => {
            serde_yml::Value::Sequence(seq.iter().map(downsample_color_value_to_16).collect())
        }
        serde_yml::Value::Mapping(map) => {
            let mut out = serde_yml::Mapping::new();
            for (k, v) in map {
                out.insert(k.clone(), downsample_color_value_to_16(v));
            }
            serde_yml::Value::Mapping(out)
        }
        serde_yml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i < 0 {
                    serde_yml::Value::Number((-1).into())
                } else if i < 16 {
                    serde_yml::Value::String(ANSI16_NAMES[i as usize].to_string())
                } else if i <= 255 {
                    let (r, g, b) = indexed_to_rgb(i as u8);
                    serde_yml::Value::String(rgb_to_ansi16_name(r, g, b).to_string())
                } else {
                    val.clone()
                }
            } else {
                val.clone()
            }
        }
        serde_yml::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim == "-1" {
                return serde_yml::Value::Number((-1).into());
            }
            if let Ok(i) = s_trim.parse::<i64>() {
                return downsample_color_value_to_16(&serde_yml::Value::Number(i.into()));
            }
            if s_trim.starts_with('#') {
                let hex = s_trim.trim_start_matches('#');
                if hex.len() == 6 {
                    if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;
                        return serde_yml::Value::String(rgb_to_ansi16_name(r, g, b).to_string());
                    }
                }
            }
            let upper = s_trim.to_uppercase();
            if ANSI16_NAMES.contains(&upper.as_str()) {
                return serde_yml::Value::String(upper);
            }
            if let Some(c) = crate::style::parse_color_value(val) {
                if c == ratatui::style::Color::Reset {
                    serde_yml::Value::Number((-1).into())
                } else {
                    let (r, g, b) = color_to_rgb(c);
                    serde_yml::Value::String(rgb_to_ansi16_name(r, g, b).to_string())
                }
            } else {
                val.clone()
            }
        }
        _ => val.clone(),
    }
}

/// Downsamples an arbitrary color value (sequence, number, hex string,
/// or named token) into a valid ANSI 8-color representation.
pub fn downsample_color_value_to_8(val: &serde_yml::Value) -> serde_yml::Value {
    match val {
        serde_yml::Value::Sequence(seq) => {
            serde_yml::Value::Sequence(seq.iter().map(downsample_color_value_to_8).collect())
        }
        serde_yml::Value::Mapping(map) => {
            let mut out = serde_yml::Mapping::new();
            for (k, v) in map {
                out.insert(k.clone(), downsample_color_value_to_8(v));
            }
            serde_yml::Value::Mapping(out)
        }
        serde_yml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i < 0 {
                    serde_yml::Value::Number((-1).into())
                } else if i < 8 {
                    serde_yml::Value::String(ANSI8_NAMES[i as usize].to_string())
                } else if i < 16 {
                    let name = if i == 8 {
                        "COLOR_BLACK"
                    } else {
                        ANSI8_NAMES[(i % 8) as usize]
                    };
                    serde_yml::Value::String(name.to_string())
                } else if i <= 255 {
                    let (r, g, b) = indexed_to_rgb(i as u8);
                    serde_yml::Value::String(rgb_to_ansi8_name(r, g, b).to_string())
                } else {
                    val.clone()
                }
            } else {
                val.clone()
            }
        }
        serde_yml::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim == "-1" {
                return serde_yml::Value::Number((-1).into());
            }
            if let Ok(i) = s_trim.parse::<i64>() {
                return downsample_color_value_to_8(&serde_yml::Value::Number(i.into()));
            }
            if s_trim.starts_with('#') {
                let hex = s_trim.trim_start_matches('#');
                if hex.len() == 6 {
                    if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                        let r = ((rgb >> 16) & 0xFF) as u8;
                        let g = ((rgb >> 8) & 0xFF) as u8;
                        let b = (rgb & 0xFF) as u8;
                        return serde_yml::Value::String(rgb_to_ansi8_name(r, g, b).to_string());
                    }
                }
            }
            let upper = s_trim.to_uppercase();
            if ANSI8_NAMES.contains(&upper.as_str()) {
                return serde_yml::Value::String(upper);
            }
            match upper.as_str() {
                "COLOR_BRIGHT_RED" => return serde_yml::Value::String("COLOR_RED".to_string()),
                "COLOR_BRIGHT_GREEN" => return serde_yml::Value::String("COLOR_GREEN".to_string()),
                "COLOR_BRIGHT_YELLOW" => {
                    return serde_yml::Value::String("COLOR_YELLOW".to_string())
                }
                "COLOR_BRIGHT_BLUE" => return serde_yml::Value::String("COLOR_BLUE".to_string()),
                "COLOR_BRIGHT_MAGENTA" => {
                    return serde_yml::Value::String("COLOR_MAGENTA".to_string())
                }
                "COLOR_BRIGHT_CYAN" => return serde_yml::Value::String("COLOR_CYAN".to_string()),
                "COLOR_BRIGHT_WHITE" => return serde_yml::Value::String("COLOR_WHITE".to_string()),
                "COLOR_BRIGHT_BLACK" | "COLOR_GREY" | "COLOR_GRAY" => {
                    return serde_yml::Value::String("COLOR_BLACK".to_string())
                }
                _ => {}
            }
            if let Some(c) = crate::style::parse_color_value(val) {
                if c == ratatui::style::Color::Reset {
                    serde_yml::Value::Number((-1).into())
                } else {
                    let (r, g, b) = color_to_rgb(c);
                    serde_yml::Value::String(rgb_to_ansi8_name(r, g, b).to_string())
                }
            } else {
                val.clone()
            }
        }
        _ => val.clone(),
    }
}

/// Downsamples a 256-color palette map into an ANSI 16-color fallback palette map.
pub fn downsample_palette_to_16(
    palette_256: &std::collections::HashMap<String, serde_yml::Value>,
) -> std::collections::HashMap<String, serde_yml::Value> {
    let mut out = std::collections::HashMap::new();
    for (k, v) in palette_256 {
        out.insert(k.clone(), downsample_color_value_to_16(v));
    }
    out
}

/// Downsamples a 256-color palette map into an ANSI 8-color fallback palette map.
pub fn downsample_palette_to_8(
    palette_256: &std::collections::HashMap<String, serde_yml::Value>,
) -> std::collections::HashMap<String, serde_yml::Value> {
    let mut out = std::collections::HashMap::new();
    for (k, v) in palette_256 {
        out.insert(k.clone(), downsample_color_value_to_8(v));
    }
    out
}
