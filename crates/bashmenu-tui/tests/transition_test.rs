use bashmenu_tui::{
    compute_modal_rect, dim_color, format_pixel_16, format_pixel_32, interpolate_zoom_rect,
    is_framebuffer_available, ModalAnimation, TransitionEffect, TransitionSpeed,
};
use image::RgbImage;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[test]
fn test_transition_effect_all_modes() {
    assert_eq!(
        TransitionEffect::parse(Some("instant")),
        TransitionEffect::Instant
    );
    assert_eq!(
        TransitionEffect::parse(Some("fade")),
        TransitionEffect::Fade
    );
    assert_eq!(
        TransitionEffect::parse(Some("curtain")),
        TransitionEffect::Curtain
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
        TransitionEffect::parse(Some("slide_left")),
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
    assert_eq!(
        TransitionEffect::parse(Some("none")),
        TransitionEffect::None
    );
    assert_eq!(
        TransitionEffect::parse(Some("invalid")),
        TransitionEffect::Fade
    );
    assert_eq!(TransitionEffect::parse(None), TransitionEffect::Fade);
}

#[test]
fn test_transition_speed_profiles() {
    assert_eq!(TransitionSpeed::parse(Some("fast")), TransitionSpeed::Fast);
    assert_eq!(
        TransitionSpeed::parse(Some("normal")),
        TransitionSpeed::Normal
    );
    assert_eq!(TransitionSpeed::parse(Some("slow")), TransitionSpeed::Slow);
    assert_eq!(
        TransitionSpeed::parse(Some("20")),
        TransitionSpeed::Custom(20)
    );
    assert_eq!(TransitionSpeed::parse(None), TransitionSpeed::Normal);

    assert!(TransitionSpeed::Fast.step_delay_in() < TransitionSpeed::Normal.step_delay_in());
    assert!(TransitionSpeed::Normal.step_delay_in() < TransitionSpeed::Slow.step_delay_in());
}

#[test]
fn test_transition_dim_color_levels() {
    std::env::set_var("BASHMENU_COLOR_TIER", "truecolor");
    let base = Color::Rgb(100, 200, 50);
    let dimmed_half = dim_color(base, 0.5);
    assert_eq!(dimmed_half, Color::Rgb(50, 100, 25));

    let dimmed_zero = dim_color(base, 0.0);
    assert_eq!(dimmed_zero, Color::Rgb(0, 0, 0));

    let dimmed_full = dim_color(base, 1.0);
    assert_eq!(dimmed_full, Color::Rgb(100, 200, 50));
    std::env::remove_var("BASHMENU_COLOR_TIER");
}

#[test]
fn test_framebuffer_pixel_formats() {
    // 32-bit ARGB/XRGB
    let p32 = format_pixel_32(200, 100, 50, 16, 8, 0);
    assert_eq!(p32, (200 << 16) | (100 << 8) | 50);

    // 16-bit RGB565
    let p16 = format_pixel_16(255, 255, 255);
    assert_eq!(p16, 0xFFFF);

    let p16_black = format_pixel_16(0, 0, 0);
    assert_eq!(p16_black, 0x0000);
}

#[test]
fn test_framebuffer_blit_rgb_buffer() {
    let mut img = RgbImage::new(4, 4);
    for x in 0..4 {
        for y in 0..4 {
            img.put_pixel(x, y, image::Rgb([10 * (x as u8), 20 * (y as u8), 50]));
        }
    }

    let mut buf32 = vec![0u8; 4 * 4 * 4];
    bashmenu_tui::framebuffer::blit_rgb_to_buffer(&img, &mut buf32, 4, 4, 16, 32, 16, 8, 0);
    assert_eq!(buf32.len(), 64);

    let mut buf16 = vec![0u8; 4 * 4 * 2];
    bashmenu_tui::framebuffer::blit_rgb_to_buffer(&img, &mut buf16, 4, 4, 8, 16, 11, 5, 0);
    assert_eq!(buf16.len(), 32);
}

#[test]
fn test_modal_animation_parsing_and_interpolation() {
    assert_eq!(ModalAnimation::parse(Some("zoom")), ModalAnimation::Zoom);
    assert_eq!(ModalAnimation::parse(Some("grow")), ModalAnimation::Zoom);
    assert_eq!(ModalAnimation::parse(Some("shrink")), ModalAnimation::Zoom);
    assert_eq!(ModalAnimation::parse(Some("none")), ModalAnimation::None);
    assert_eq!(ModalAnimation::parse(Some("off")), ModalAnimation::None);
    assert_eq!(
        ModalAnimation::parse(Some("disabled")),
        ModalAnimation::None
    );
    assert_eq!(ModalAnimation::parse(Some("0")), ModalAnimation::None);
    assert_eq!(ModalAnimation::parse(None), ModalAnimation::Zoom);

    let viewport = Rect::new(0, 0, 80, 24);
    let target = Rect::new(20, 6, 40, 12);
    let anchor = (40, 10);

    let r_start = interpolate_zoom_rect(anchor, target, 0.0, viewport);
    assert!(r_start.width <= 6);
    assert!(r_start.height <= 3);

    let r_mid = interpolate_zoom_rect(anchor, target, 0.5, viewport);
    assert!(r_mid.width > r_start.width);
    assert!(r_mid.height > r_start.height);
    assert!(r_mid.width < target.width);
    assert!(r_mid.height < target.height);

    let r_end = interpolate_zoom_rect(anchor, target, 1.0, viewport);
    assert_eq!(r_end, target);
}

#[test]
fn test_compute_modal_rect() {
    let area = Rect::new(0, 0, 80, 24);
    let rect = compute_modal_rect(area, 40, 10);
    assert_eq!(rect.width, 40);
    assert_eq!(rect.height, 10);
    // Centered horizontally: (80 - 42) / 2 = 19
    assert_eq!(rect.x, 19);
    // Centered vertically: (24 - 11) / 2 = 6
    assert_eq!(rect.y, 6);
}

#[test]
fn test_is_framebuffer_available_safe_call() {
    // Should execute safely without panicking in any environment
    let _ = is_framebuffer_available();
}
