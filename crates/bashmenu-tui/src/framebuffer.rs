//! Direct Linux Framebuffer (/dev/fb0) rendering engine.
//!
//! Provides native, self-contained framebuffer detection, ioctl querying
//! (FBIOGET_VSCREENINFO / FBIOGET_FSCREENINFO), memory mapping, and direct
//! 16-bit and 32-bit TrueColor image blitting and Ratatui buffer compositing
//! on minimal Linux consoles.

use crate::fb_font::{draw_bitmap_glyph, draw_procedural_glyph, CellPixelRect};
use crate::wallpaper::{color_to_rgb, resolve_wallpaper_path, scale_image};
use bashmenu_core::WallpaperConfig;
use image::{imageops, Rgb, RgbImage};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use std::path::Path;

/// Formats a 24-bit RGB pixel into a 32-bit pixel value matching framebuffer
/// bitfields.
pub fn format_pixel_32(
    r: u8,
    g: u8,
    b: u8,
    red_offset: u32,
    green_offset: u32,
    blue_offset: u32,
) -> u32 {
    ((r as u32) << red_offset) | ((g as u32) << green_offset) | ((b as u32) << blue_offset)
}

/// Formats a 24-bit RGB pixel into a standard 16-bit RGB565 pixel value.
pub fn format_pixel_16(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g6 = (g >> 2) as u16;
    let b5 = (b >> 3) as u16;
    (r5 << 11) | (g6 << 5) | b5
}

/// Writes a single `(r, g, b)` pixel at `(px, py)` into a framebuffer slice.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn write_pixel_to_slice(
    target: &mut [u8],
    px: u32,
    py: u32,
    width: u32,
    height: u32,
    line_length: u32,
    bpp: u32,
    red_offset: u32,
    green_offset: u32,
    blue_offset: u32,
    r: u8,
    g: u8,
    b: u8,
) {
    if px >= width || py >= height {
        return;
    }
    let bpp_bytes = (bpp / 8) as usize;
    if bpp_bytes == 0 {
        return;
    }
    let px_offset = (py as usize) * (line_length as usize) + (px as usize) * bpp_bytes;
    if px_offset + bpp_bytes > target.len() {
        return;
    }
    if bpp == 32 {
        let val = format_pixel_32(r, g, b, red_offset, green_offset, blue_offset);
        target[px_offset..px_offset + 4].copy_from_slice(&val.to_ne_bytes());
    } else if bpp == 16 {
        let val = format_pixel_16(r, g, b);
        target[px_offset..px_offset + 2].copy_from_slice(&val.to_ne_bytes());
    } else if bpp == 24 {
        if red_offset == 16 {
            target[px_offset] = b;
            target[px_offset + 1] = g;
            target[px_offset + 2] = r;
        } else {
            target[px_offset] = r;
            target[px_offset + 1] = g;
            target[px_offset + 2] = b;
        }
    }
}

/// Blits an RGB image into an in-memory buffer using 32-bit or 16-bit layouts.
#[allow(clippy::too_many_arguments)]
pub fn blit_rgb_to_buffer(
    img: &RgbImage,
    target: &mut [u8],
    width: u32,
    height: u32,
    line_length: u32,
    bpp: u32,
    red_offset: u32,
    green_offset: u32,
    blue_offset: u32,
) {
    let scaled = if img.width() != width || img.height() != height {
        imageops::resize(img, width, height, imageops::FilterType::Triangle)
    } else {
        img.clone()
    };

    for y in 0..height {
        for x in 0..width {
            let pixel = scaled.get_pixel(x, y);
            write_pixel_to_slice(
                target,
                x,
                y,
                width,
                height,
                line_length,
                bpp,
                red_offset,
                green_offset,
                blue_offset,
                pixel[0],
                pixel[1],
                pixel[2],
            );
        }
    }
}

/// Renders a Ratatui [`Buffer`] into a raw framebuffer byte slice in TrueColor.
///
/// When `wallpaper_slice` is `Some`, it is copied as the full-resolution base
/// backdrop and preserved wherever `cell.bg == Color::Reset` (and not
/// `Modifier::REVERSED`).
#[allow(clippy::too_many_arguments)]
pub fn render_ratatui_buffer_to_slice(
    buf: &Buffer,
    default_bg: Color,
    wallpaper_slice: Option<&[u8]>,
    target: &mut [u8],
    width: u32,
    height: u32,
    line_length: u32,
    bpp: u32,
    red_offset: u32,
    green_offset: u32,
    blue_offset: u32,
) {
    let cols = buf.area.width as u32;
    let rows = buf.area.height as u32;
    if cols == 0 || rows == 0 || width == 0 || height == 0 {
        return;
    }

    let has_full_res_wallpaper = if let Some(wp) = wallpaper_slice {
        let copy_len = target.len().min(wp.len());
        target[..copy_len].copy_from_slice(&wp[..copy_len]);
        true
    } else {
        false
    };

    let default_bg_rgb = if default_bg == Color::Reset {
        (0, 0, 0)
    } else {
        color_to_rgb(default_bg)
    };

    for row in 0..rows {
        let y0 = (row * height) / rows;
        let y1 = ((row + 1) * height) / rows;
        if y0 >= y1 {
            continue;
        }

        let mut skip_next_wide = false;
        for col in 0..cols {
            let x0 = (col * width) / cols;
            let x1 = ((col + 1) * width) / cols;
            if x0 >= x1 {
                continue;
            }

            let cell = &buf[(buf.area.x + col as u16, buf.area.y + row as u16)];
            let sym = cell.symbol();
            let ch = sym.chars().next().unwrap_or(' ');

            let mut fg_rgb = if cell.fg == Color::Reset {
                (255, 255, 255)
            } else {
                color_to_rgb(cell.fg)
            };

            let transparent_bg = has_full_res_wallpaper
                && !cell.modifier.contains(Modifier::REVERSED)
                && cell.bg == Color::Reset;

            let mut bg_rgb = if cell.bg == Color::Reset {
                default_bg_rgb
            } else {
                color_to_rgb(cell.bg)
            };

            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg_rgb, &mut bg_rgb);
            }

            if cell.modifier.contains(Modifier::DIM) {
                fg_rgb = (
                    ((fg_rgb.0 as u16 * 6) / 10) as u8,
                    ((fg_rgb.1 as u16 * 6) / 10) as u8,
                    ((fg_rgb.2 as u16 * 6) / 10) as u8,
                );
            }

            // 1. Paint cell background unless transparent over full-res wallpaper
            if !transparent_bg {
                for py in y0..y1 {
                    for px in x0..x1 {
                        write_pixel_to_slice(
                            target,
                            px,
                            py,
                            width,
                            height,
                            line_length,
                            bpp,
                            red_offset,
                            green_offset,
                            blue_offset,
                            bg_rgb.0,
                            bg_rgb.1,
                            bg_rgb.2,
                        );
                    }
                }
            }

            if skip_next_wide {
                skip_next_wide = false;
                continue;
            }

            if unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) > 1 && col + 1 < cols {
                skip_next_wide = true;
            }

            // 2. Paint cell foreground glyph
            if ch != ' ' && ch != '\0' {
                let rect = CellPixelRect { x0, y0, x1, y1 };
                let bold = cell.modifier.contains(Modifier::BOLD);
                let underlined = cell.modifier.contains(Modifier::UNDERLINED);

                let handled = draw_procedural_glyph(ch, rect, bold, |px, py| {
                    write_pixel_to_slice(
                        target,
                        px,
                        py,
                        width,
                        height,
                        line_length,
                        bpp,
                        red_offset,
                        green_offset,
                        blue_offset,
                        fg_rgb.0,
                        fg_rgb.1,
                        fg_rgb.2,
                    );
                });

                if !handled {
                    draw_bitmap_glyph(ch, rect, bold, underlined, |px, py| {
                        write_pixel_to_slice(
                            target,
                            px,
                            py,
                            width,
                            height,
                            line_length,
                            bpp,
                            red_offset,
                            green_offset,
                            blue_offset,
                            fg_rgb.0,
                            fg_rgb.1,
                            fg_rgb.2,
                        );
                    });
                }
            } else if cell.modifier.contains(Modifier::UNDERLINED) {
                let rect = CellPixelRect { x0, y0, x1, y1 };
                draw_bitmap_glyph(' ', rect, false, true, |px, py| {
                    write_pixel_to_slice(
                        target,
                        px,
                        py,
                        width,
                        height,
                        line_length,
                        bpp,
                        red_offset,
                        green_offset,
                        blue_offset,
                        fg_rgb.0,
                        fg_rgb.1,
                        fg_rgb.2,
                    );
                });
            }
        }
    }
}

/// Checks whether a Linux framebuffer device exists on the system.
pub fn is_framebuffer_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        let fb_path = std::env::var("FRAMEBUFFER").unwrap_or_else(|_| "/dev/fb0".to_string());
        Path::new(&fb_path).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Checks whether the Linux framebuffer device exists but fails to open due to
/// insufficient user permissions (`EACCES` / `PermissionDenied`, e.g. user not
/// in the `video` group).
pub fn is_framebuffer_permission_denied() -> bool {
    #[cfg(target_os = "linux")]
    {
        let fb_path = std::env::var("FRAMEBUFFER").unwrap_or_else(|_| "/dev/fb0".to_string());
        if !Path::new(&fb_path).exists() {
            return false;
        }
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&fb_path)
        {
            Ok(_) => false,
            Err(e) => e.kind() == std::io::ErrorKind::PermissionDenied,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Interactively prompts the user (when attached to a TTY) to grant `video`
/// group membership and immediate `/dev/fb0` ACL access via `sudo` or `doas`.
/// Returns `true` if `/dev/fb0` becomes accessible.
pub fn ensure_framebuffer_access_interactive() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::io::{self, Write};

        if !is_framebuffer_permission_denied() {
            return is_framebuffer_available();
        }
        if std::env::var("BASHMENU_NONINTERACTIVE").as_deref() == Ok("1") {
            return false;
        }
        let is_tty = unsafe {
            libc::isatty(libc::STDIN_FILENO) == 1 && libc::isatty(libc::STDOUT_FILENO) == 1
        };
        if !is_tty {
            return false;
        }

        let fb_path = std::env::var("FRAMEBUFFER").unwrap_or_else(|_| "/dev/fb0".to_string());
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default();
        if user.is_empty() {
            return false;
        }

        let priv_cmd = if Path::new("/usr/bin/sudo").exists() || Path::new("/bin/sudo").exists() {
            "sudo"
        } else if Path::new("/usr/bin/doas").exists() || Path::new("/bin/doas").exists() {
            "doas"
        } else {
            return false;
        };

        let _ = writeln!(
            io::stderr(),
            "\n  [!] Linux framebuffer ({}) permission denied for user '{}'.",
            fb_path,
            user
        );
        let _ = writeln!(
            io::stderr(),
            "      TrueColor & photo-grade console graphics require 'video' group / ACL access."
        );
        let _ = write!(
            io::stderr(),
            "  [?] Grant '{}' access now via {}? [Y/n]: ",
            fb_path,
            priv_cmd
        );
        let _ = io::stderr().flush();

        let mut reply = String::new();
        if io::stdin().read_line(&mut reply).is_err() {
            return false;
        }
        let trimmed = reply.trim().to_ascii_lowercase();
        if !trimmed.is_empty() && trimmed != "y" && trimmed != "yes" {
            return false;
        }

        let script = r#"
usermod -aG video "$1" 2>/dev/null || true
if [ -d /etc/udev/rules.d ]; then
    printf 'SUBSYSTEM=="graphics", KERNEL=="fb[0-9]*", TAG+="uaccess", GROUP="video", MODE="0660"\n' > /etc/udev/rules.d/70-bashmenu-fb.rules 2>/dev/null || true
    udevadm control --reload-rules 2>/dev/null || true
    udevadm trigger --subsystem-match=graphics 2>/dev/null || true
fi
if command -v setfacl >/dev/null 2>&1; then
    setfacl -m "u:$1:rw" "$2" 2>/dev/null || chmod 0666 "$2" 2>/dev/null || true
else
    chmod 0666 "$2" 2>/dev/null || true
fi
"#;
        let _ = std::process::Command::new(priv_cmd)
            .args(["sh", "-c", script, "--", &user, &fb_path])
            .status();

        !is_framebuffer_permission_denied()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Pure helper evaluating `(is_desktop_terminal, is_linux_console)` from
/// environment variables without mutating global state.
pub fn evaluate_console_env(
    term: Option<&str>,
    xdg_session_type: Option<&str>,
    display: Option<&str>,
    wayland_display: Option<&str>,
    force_fb: bool,
    fb_disable: bool,
) -> (bool, bool) {
    if force_fb {
        return (false, true);
    }
    let is_desktop = if let Some(session_type) = xdg_session_type {
        let st = session_type.trim().to_ascii_lowercase();
        if st == "x11" || st == "wayland" {
            true
        } else if st == "tty" {
            false
        } else {
            display.map(|v| !v.trim().is_empty()).unwrap_or(false)
                || wayland_display
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
        }
    } else if display.map(|v| !v.trim().is_empty()).unwrap_or(false)
        || wayland_display
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    {
        true
    } else if let Some(t_raw) = term {
        let t = t_raw.trim().to_ascii_lowercase();
        !t.is_empty() && t != "linux" && t != "console" && !t.starts_with("linux-")
    } else {
        false
    };

    if fb_disable {
        return (is_desktop, false);
    }

    let term_is_linux = if let Some(t_raw) = term {
        let t = t_raw.trim().to_ascii_lowercase();
        t == "linux" || t == "console" || t.starts_with("linux-")
    } else {
        false
    };

    (is_desktop, term_is_linux && !is_desktop)
}

/// Checks whether the current process is running inside a desktop GUI terminal
/// emulator window (X11/Wayland) rather than a raw Linux virtual console.
pub fn is_desktop_terminal() -> bool {
    let term = std::env::var("TERM").ok();
    let xdg = std::env::var("XDG_SESSION_TYPE").ok();
    let disp = std::env::var("DISPLAY").ok();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let force_fb = std::env::var("BASHMENU_FORCE_FB").as_deref() == Ok("1");
    let fb_disable = std::env::var("BASHMENU_FB_DISABLE").as_deref() == Ok("1");
    evaluate_console_env(
        term.as_deref(),
        xdg.as_deref(),
        disp.as_deref(),
        wayland.as_deref(),
        force_fb,
        fb_disable,
    )
    .0
}

/// Checks whether the current `$TERM` and session represent a bare Linux
/// virtual console (and not a desktop GUI terminal emulator).
pub fn is_linux_console() -> bool {
    let term = std::env::var("TERM").ok();
    let xdg = std::env::var("XDG_SESSION_TYPE").ok();
    let disp = std::env::var("DISPLAY").ok();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let force_fb = std::env::var("BASHMENU_FORCE_FB").as_deref() == Ok("1");
    let fb_disable = std::env::var("BASHMENU_FB_DISABLE").as_deref() == Ok("1");
    evaluate_console_env(
        term.as_deref(),
        xdg.as_deref(),
        disp.as_deref(),
        wayland.as_deref(),
        force_fb,
        fb_disable,
    )
    .1
}

#[cfg(target_os = "linux")]
const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
#[cfg(target_os = "linux")]
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: usize,
    smem_len: u32,
    type_: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: usize,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

/// Cache key identifying a pre-scaled full-resolution framebuffer wallpaper.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FbWallpaperCacheKey {
    image_path: String,
    width: u32,
    height: u32,
    theme_bg_rgb: (u8, u8, u8),
    opacity_milli: u32,
    scaling: String,
}

/// Linux Framebuffer device wrapper managing memory-mapped video RAM.
pub struct FramebufferDevice {
    #[cfg(target_os = "linux")]
    _file: std::fs::File,
    #[cfg(target_os = "linux")]
    mmap_ptr: *mut u8,
    #[cfg(target_os = "linux")]
    smem_len: usize,
    /// Off-screen double buffer in system RAM for tear-free frame compositing
    backbuffer: Vec<u8>,
    /// Pre-scaled, opacity-blended full-resolution wallpaper in device format
    wallpaper_fb: Option<Vec<u8>>,
    /// Cache key for `wallpaper_fb`
    wallpaper_key: Option<FbWallpaperCacheKey>,
    /// Whether full-resolution pixel wallpaper mode (`-i` / `--image`) is active
    image_mode: bool,
    /// Horizontal visible resolution in pixels
    pub width: u32,
    /// Vertical visible resolution in pixels
    pub height: u32,
    /// Pixel bit depth (e.g. 16 or 32)
    pub bpp: u32,
    /// Stride in bytes per scanline
    pub line_length: u32,
    /// Bitfield offset for red channel
    pub red_offset: u32,
    /// Bitfield offset for green channel
    pub green_offset: u32,
    /// Bitfield offset for blue channel
    pub blue_offset: u32,
}

// Safety: FramebufferDevice owns the memory-mapped pointer and file descriptor.
unsafe impl Send for FramebufferDevice {}
unsafe impl Sync for FramebufferDevice {}

impl FramebufferDevice {
    /// Attempts to open and memory-map `/dev/fb0` (or `$FRAMEBUFFER`).
    pub fn open() -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::fs::OpenOptions;
            use std::os::unix::io::AsRawFd;

            let fb_path = std::env::var("FRAMEBUFFER").unwrap_or_else(|_| "/dev/fb0".to_string());
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fb_path)
                .ok()?;
            let fd = file.as_raw_fd();

            let mut vinfo = FbVarScreeninfo::default();
            let mut finfo: FbFixScreeninfo = unsafe { std::mem::zeroed() };

            unsafe {
                if libc::ioctl(fd, FBIOGET_VSCREENINFO, &mut vinfo) < 0 {
                    return None;
                }
                if libc::ioctl(fd, FBIOGET_FSCREENINFO, &mut finfo) < 0 {
                    return None;
                }
            }

            let smem_len = finfo.smem_len as usize;
            if smem_len == 0 {
                return None;
            }

            let mmap_ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    smem_len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
            };

            if mmap_ptr == libc::MAP_FAILED || mmap_ptr.is_null() {
                return None;
            }

            Some(Self {
                _file: file,
                mmap_ptr: mmap_ptr as *mut u8,
                smem_len,
                backbuffer: vec![0u8; smem_len],
                wallpaper_fb: None,
                wallpaper_key: None,
                image_mode: false,
                width: vinfo.xres,
                height: vinfo.yres,
                bpp: vinfo.bits_per_pixel,
                line_length: finfo.line_length,
                red_offset: vinfo.red.offset,
                green_offset: vinfo.green.offset,
                blue_offset: vinfo.blue.offset,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// Enables or disables full-resolution pixel wallpaper mode (`-i` / `--image`).
    pub fn set_image_mode(&mut self, enabled: bool) {
        self.image_mode = enabled;
        if !enabled {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
        }
    }

    /// Returns whether full-resolution pixel wallpaper mode (`-i` / `--image`) is active.
    pub fn is_image_mode(&self) -> bool {
        self.image_mode
    }

    /// Updates and caches the full-resolution wallpaper image for `--image` mode.
    pub fn set_wallpaper(&mut self, config: Option<&WallpaperConfig>, theme_bg: Color) {
        if !self.image_mode || self.width == 0 || self.height == 0 {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        }

        let Some(cfg) = config else {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        };

        if !cfg.enabled {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        }

        let Some(ref img_str) = cfg.image else {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        };

        let Some(resolved_path) = resolve_wallpaper_path(img_str) else {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        };

        let path_str = resolved_path.to_string_lossy().to_string();
        let theme_bg_rgb = if theme_bg == Color::Reset {
            (0, 0, 0)
        } else {
            color_to_rgb(theme_bg)
        };
        let op = cfg.opacity.clamp(0.0, 1.0);
        let op_milli = (op * 1000.0).round() as u32;
        let scaling = cfg.scaling.to_ascii_lowercase();

        let key = FbWallpaperCacheKey {
            image_path: path_str,
            width: self.width,
            height: self.height,
            theme_bg_rgb,
            opacity_milli: op_milli,
            scaling: scaling.clone(),
        };

        if self.wallpaper_key.as_ref() == Some(&key) && self.wallpaper_fb.is_some() {
            return;
        }

        let Ok(dyn_img) = image::open(&resolved_path) else {
            self.wallpaper_fb = None;
            self.wallpaper_key = None;
            return;
        };

        let raw_rgb = dyn_img.to_rgb8();
        let mut scaled = scale_image(&raw_rgb, self.width, self.height, &scaling, theme_bg_rgb);

        if op_milli < 1000 {
            let inv_op = 1.0 - op;
            let (bg_r, bg_g, bg_b) = (
                theme_bg_rgb.0 as f32,
                theme_bg_rgb.1 as f32,
                theme_bg_rgb.2 as f32,
            );
            for y in 0..scaled.height() {
                for x in 0..scaled.width() {
                    let p = scaled.get_pixel(x, y);
                    let r = ((p[0] as f32) * op + bg_r * inv_op)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    let g = ((p[1] as f32) * op + bg_g * inv_op)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    let b = ((p[2] as f32) * op + bg_b * inv_op)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    scaled.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }

        let buf_len = self.backbuffer.len();
        let mut wp_bytes = vec![0u8; buf_len];
        blit_rgb_to_buffer(
            &scaled,
            &mut wp_bytes,
            self.width,
            self.height,
            self.line_length,
            self.bpp,
            self.red_offset,
            self.green_offset,
            self.blue_offset,
        );

        self.wallpaper_fb = Some(wp_bytes);
        self.wallpaper_key = Some(key);
    }

    /// Flushes `self.backbuffer` into `/dev/fb0` via both memory-mapped video
    /// RAM and `pwrite()` so DRM shadow-buffered drivers (`i915drmfb`,
    /// `simpledrm`, `amdgpudrmfb`, `virtio_gpu`) trigger dirty-damage updates.
    #[cfg(target_os = "linux")]
    fn flush_backbuffer(&mut self) {
        use std::os::unix::io::AsRawFd;

        if self.mmap_ptr.is_null() || self.smem_len == 0 {
            return;
        }
        let copy_len = self.smem_len.min(self.backbuffer.len());
        if copy_len == 0 {
            return;
        }
        let target_slice = unsafe { std::slice::from_raw_parts_mut(self.mmap_ptr, self.smem_len) };
        target_slice[..copy_len].copy_from_slice(&self.backbuffer[..copy_len]);
        let _ = unsafe {
            libc::pwrite(
                self._file.as_raw_fd(),
                self.backbuffer.as_ptr() as *const libc::c_void,
                copy_len,
                0,
            )
        };
    }

    /// Renders a Ratatui [`Buffer`] into the off-screen backbuffer and flushes
    /// it atomically to `/dev/fb0` video memory.
    pub fn render_buffer(&mut self, buf: &Buffer, default_bg: Color) {
        #[cfg(target_os = "linux")]
        {
            if self.mmap_ptr.is_null() || self.smem_len == 0 {
                return;
            }
            let wp_slice = if self.image_mode {
                self.wallpaper_fb.as_deref()
            } else {
                None
            };
            render_ratatui_buffer_to_slice(
                buf,
                default_bg,
                wp_slice,
                &mut self.backbuffer,
                self.width,
                self.height,
                self.line_length,
                self.bpp,
                self.red_offset,
                self.green_offset,
                self.blue_offset,
            );
            self.flush_backbuffer();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (buf, default_bg);
        }
    }

    /// Blits an image directly onto the mapped framebuffer video memory.
    pub fn blit(&mut self, img: &RgbImage) {
        #[cfg(target_os = "linux")]
        {
            if self.mmap_ptr.is_null() || self.smem_len == 0 {
                return;
            }
            blit_rgb_to_buffer(
                img,
                &mut self.backbuffer,
                self.width,
                self.height,
                self.line_length,
                self.bpp,
                self.red_offset,
                self.green_offset,
                self.blue_offset,
            );
            self.flush_backbuffer();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = img;
        }
    }

    /// Loads an image file from disk and blits it onto the framebuffer.
    pub fn blit_file<P: AsRef<Path>>(&mut self, path: P) -> bool {
        if let Ok(img) = image::open(path) {
            self.blit(&img.to_rgb8());
            true
        } else {
            false
        }
    }

    /// Clears the framebuffer video memory with the specified RGB color.
    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        #[cfg(target_os = "linux")]
        {
            if self.mmap_ptr.is_null() || self.smem_len == 0 {
                return;
            }
            for y in 0..self.height {
                for x in 0..self.width {
                    write_pixel_to_slice(
                        &mut self.backbuffer,
                        x,
                        y,
                        self.width,
                        self.height,
                        self.line_length,
                        self.bpp,
                        self.red_offset,
                        self.green_offset,
                        self.blue_offset,
                        r,
                        g,
                        b,
                    );
                }
            }
            self.flush_backbuffer();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (r, g, b);
        }
    }
}

impl Drop for FramebufferDevice {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if !self.mmap_ptr.is_null() && self.smem_len > 0 {
                unsafe {
                    libc::munmap(self.mmap_ptr as *mut libc::c_void, self.smem_len);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    #[test]
    fn test_format_pixel_32() {
        let val = format_pixel_32(255, 128, 64, 16, 8, 0);
        assert_eq!(val, (255 << 16) | (128 << 8) | 64);
    }

    #[test]
    fn test_format_pixel_16() {
        let val = format_pixel_16(255, 255, 255);
        assert_eq!(val, 0xFFFF);

        let black = format_pixel_16(0, 0, 0);
        assert_eq!(black, 0x0000);
    }

    #[test]
    fn test_blit_rgb_to_buffer_in_memory() {
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        img.put_pixel(1, 0, image::Rgb([0, 255, 0]));
        img.put_pixel(0, 1, image::Rgb([0, 0, 255]));
        img.put_pixel(1, 1, image::Rgb([255, 255, 255]));

        let mut buf = vec![0u8; 2 * 2 * 4];
        blit_rgb_to_buffer(&img, &mut buf, 2, 2, 8, 32, 16, 8, 0);

        let p00 = u32::from_ne_bytes(buf[0..4].try_into().unwrap());
        assert_eq!(p00, 255 << 16);

        let p10 = u32::from_ne_bytes(buf[4..8].try_into().unwrap());
        assert_eq!(p10, 255 << 8);

        let p01 = u32::from_ne_bytes(buf[8..12].try_into().unwrap());
        assert_eq!(p01, 255);
    }

    #[test]
    fn test_render_ratatui_buffer_text_and_full_res_wallpaper() {
        // 2x1 character grid rendered onto a 16x16 32-bit framebuffer
        let mut ratatui_buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        // Cell (0, 0): half-block '▀' with Red upper, Blue lower
        ratatui_buf[(0, 0)].set_symbol("▀").set_style(
            Style::default()
                .fg(Color::Rgb(255, 0, 0))
                .bg(Color::Rgb(0, 0, 255)),
        );
        // Cell (1, 0): transparent background (Color::Reset) over full-res wallpaper
        ratatui_buf[(1, 0)]
            .set_symbol(" ")
            .set_style(Style::default().fg(Color::White).bg(Color::Reset));

        let mut wp_img = RgbImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                wp_img.put_pixel(x, y, image::Rgb([10, 200, 30]));
            }
        }
        let mut wp_slice = vec![0u8; 16 * 16 * 4];
        blit_rgb_to_buffer(&wp_img, &mut wp_slice, 16, 16, 64, 32, 16, 8, 0);

        let mut fb_out = vec![0u8; 16 * 16 * 4];
        render_ratatui_buffer_to_slice(
            &ratatui_buf,
            Color::Black,
            Some(&wp_slice),
            &mut fb_out,
            16,
            16,
            64,
            32,
            16,
            8,
            0,
        );

        // Cell (0, 0) top half (x=2, y=2) should be Red (255, 0, 0)
        let top_idx = (2 * 16 + 2) * 4;
        let top_px = u32::from_ne_bytes(fb_out[top_idx..top_idx + 4].try_into().unwrap());
        assert_eq!(top_px, format_pixel_32(255, 0, 0, 16, 8, 0));

        // Cell (0, 0) bottom half (x=2, y=12) should be Blue (0, 0, 255)
        let bot_idx = (12 * 16 + 2) * 4;
        let bot_px = u32::from_ne_bytes(fb_out[bot_idx..bot_idx + 4].try_into().unwrap());
        assert_eq!(bot_px, format_pixel_32(0, 0, 255, 16, 8, 0));

        // Cell (1, 0) (x=12, y=8) has Color::Reset bg so full-res wallpaper (10, 200, 30) shines through
        let wp_idx = (8 * 16 + 12) * 4;
        let wp_px = u32::from_ne_bytes(fb_out[wp_idx..wp_idx + 4].try_into().unwrap());
        assert_eq!(wp_px, format_pixel_32(10, 200, 30, 16, 8, 0));
    }

    #[test]
    fn test_evaluate_console_env_desktop_and_linux_vt_guards() {
        // 1. GNOME Terminal on X11 desktop -> desktop=true, linux_console=false
        assert_eq!(
            evaluate_console_env(
                Some("xterm-256color"),
                Some("x11"),
                Some(":0"),
                None,
                false,
                false
            ),
            (true, false)
        );

        // 2. Spoofed TERM=linux inside X11 desktop -> desktop=true, linux_console=false
        assert_eq!(
            evaluate_console_env(Some("linux"), Some("x11"), Some(":0"), None, false, false),
            (true, false)
        );

        // 3. Real Linux VT (Ctrl+Alt+F2, XDG_SESSION_TYPE=tty, TERM=linux) -> desktop=false, linux_console=true
        assert_eq!(
            evaluate_console_env(Some("linux"), Some("tty"), None, None, false, false),
            (false, true)
        );

        // 4. Minimal Debian console without systemd/XDG vars -> desktop=false, linux_console=true
        assert_eq!(
            evaluate_console_env(Some("linux"), None, None, None, false, false),
            (false, true)
        );

        // 5. Explicit BASHMENU_FB_DISABLE=1 disables linux_console
        assert_eq!(
            evaluate_console_env(Some("linux"), Some("tty"), None, None, false, true),
            (false, false)
        );
    }
}
