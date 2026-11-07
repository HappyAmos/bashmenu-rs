//! Direct Linux Framebuffer (/dev/fb0) rendering engine.
//!
//! Provides native, self-contained framebuffer detection, ioctl querying
//! (FBIOGET_VSCREENINFO / FBIOGET_FSCREENINFO), memory mapping, and direct
//! 16-bit and 32-bit TrueColor image blitting on minimal Linux consoles.

use image::{imageops, RgbImage};
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

    let bpp_bytes = (bpp / 8) as usize;
    for y in 0..height {
        let row_offset = (y as usize) * (line_length as usize);
        for x in 0..width {
            let px_offset = row_offset + (x as usize) * bpp_bytes;
            if px_offset + bpp_bytes > target.len() {
                continue;
            }
            let pixel = scaled.get_pixel(x, y);
            if bpp == 32 {
                let val = format_pixel_32(
                    pixel[0],
                    pixel[1],
                    pixel[2],
                    red_offset,
                    green_offset,
                    blue_offset,
                );
                target[px_offset..px_offset + 4].copy_from_slice(&val.to_ne_bytes());
            } else if bpp == 16 {
                let val = format_pixel_16(pixel[0], pixel[1], pixel[2]);
                target[px_offset..px_offset + 2].copy_from_slice(&val.to_ne_bytes());
            }
        }
    }
}

/// Checks whether a Linux framebuffer device exists and is accessible.
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

/// Linux Framebuffer device wrapper managing memory-mapped video RAM.
pub struct FramebufferDevice {
    #[cfg(target_os = "linux")]
    _file: std::fs::File,
    #[cfg(target_os = "linux")]
    mmap_ptr: *mut u8,
    #[cfg(target_os = "linux")]
    smem_len: usize,
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

    /// Blits an image directly onto the mapped framebuffer video memory.
    pub fn blit(&mut self, img: &RgbImage) {
        #[cfg(target_os = "linux")]
        {
            if self.mmap_ptr.is_null() || self.smem_len == 0 {
                return;
            }
            let target_slice =
                unsafe { std::slice::from_raw_parts_mut(self.mmap_ptr, self.smem_len) };
            blit_rgb_to_buffer(
                img,
                target_slice,
                self.width,
                self.height,
                self.line_length,
                self.bpp,
                self.red_offset,
                self.green_offset,
                self.blue_offset,
            );
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
            let target = unsafe { std::slice::from_raw_parts_mut(self.mmap_ptr, self.smem_len) };
            let bpp_bytes = (self.bpp / 8) as usize;
            for y in 0..self.height {
                let row_offset = (y as usize) * (self.line_length as usize);
                for x in 0..self.width {
                    let px_offset = row_offset + (x as usize) * bpp_bytes;
                    if px_offset + bpp_bytes <= target.len() {
                        if self.bpp == 32 {
                            let val = format_pixel_32(
                                r,
                                g,
                                b,
                                self.red_offset,
                                self.green_offset,
                                self.blue_offset,
                            );
                            target[px_offset..px_offset + 4].copy_from_slice(&val.to_ne_bytes());
                        } else if self.bpp == 16 {
                            let val = format_pixel_16(r, g, b);
                            target[px_offset..px_offset + 2].copy_from_slice(&val.to_ne_bytes());
                        }
                    }
                }
            }
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
}
