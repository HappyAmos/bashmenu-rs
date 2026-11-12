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

    // 256-color tier must return Color::Indexed, avoiding raw RGB escapes
    std::env::set_var("BASHMENU_COLOR_TIER", "256");
    let dimmed_256 = dim_color(base, 0.5);
    assert!(matches!(dimmed_256, Color::Indexed(_)));

    // 16-color tier must return standard 16-color ANSI
    std::env::set_var("BASHMENU_COLOR_TIER", "16");
    let dimmed_16 = dim_color(base, 0.5);
    assert!(!matches!(dimmed_16, Color::Rgb(_, _, _)));

    // 8-color tier must return standard 8-color ANSI
    std::env::set_var("BASHMENU_COLOR_TIER", "8");
    let dimmed_8 = dim_color(base, 0.5);
    assert!(!matches!(dimmed_8, Color::Rgb(_, _, _)));

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

#[test]
fn test_framebuffer_default_character_and_image_mode_rendering() {
    use bashmenu_core::{load_menu, WallpaperConfig};
    use bashmenu_tui::framebuffer::render_ratatui_buffer_to_slice;
    use bashmenu_tui::{MenuView, ResolvedThemeStyles};
    use ratatui::buffer::Buffer;

    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    std::env::set_var("BASHMENU_DIR", &repo_root);
    let menu_path = repo_root.join("bashmenu.mnu");
    let schema = load_menu(&menu_path).expect("bashmenu.mnu must load");

    let styles = ResolvedThemeStyles {
        background: Color::Rgb(15, 20, 30),
        ..Default::default()
    };

    let wp_cfg = WallpaperConfig {
        enabled: true,
        image: Some("assets/wallpapers/alaska-mountain.png".to_string()),
        opacity: 0.85,
        scaling: "cover".to_string(),
        clustering: 0,
        quantization: 1,
        mode: Some("halfblock".to_string()),
    };

    let area = Rect::new(0, 0, 80, 24);

    // 1. Default Mode: character-block wallpaper translation
    let mut view_char = MenuView::new(schema.clone(), styles.clone(), false, String::new());
    view_char.set_wallpaper(Some(wp_cfg.clone()));
    assert!(!view_char.is_framebuffer_image_mode());

    let mut buf_char = Buffer::empty(area);
    view_char.render(area, &mut buf_char);
    // Padding cells at top-left (0, 0) should be converted to halfblock or colored cell
    assert_ne!(buf_char[(0, 0)].bg, Color::Reset);

    // 2. Full-Resolution Image Mode (-i / --image): background cells marked Color::Reset
    let mut view_img = MenuView::new(schema, styles.clone(), false, String::new());
    view_img.set_wallpaper(Some(wp_cfg));
    view_img.set_framebuffer_image_mode(true);
    assert!(view_img.is_framebuffer_image_mode());

    let mut buf_img = Buffer::empty(area);
    view_img.render(area, &mut buf_img);
    assert_eq!(buf_img[(0, 0)].bg, Color::Reset);

    // Render into a 160x48 32-bit framebuffer slice over a green full-res wallpaper
    let fb_w = 160u32;
    let fb_h = 48u32;
    let stride = fb_w * 4;
    let mut wp_img = RgbImage::new(fb_w, fb_h);
    for y in 0..fb_h {
        for x in 0..fb_w {
            wp_img.put_pixel(x, y, image::Rgb([12, 210, 45]));
        }
    }
    let mut wp_bytes = vec![0u8; (fb_h * stride) as usize];
    bashmenu_tui::framebuffer::blit_rgb_to_buffer(
        &wp_img,
        &mut wp_bytes,
        fb_w,
        fb_h,
        stride,
        32,
        16,
        8,
        0,
    );

    let mut target_fb = vec![0u8; (fb_h * stride) as usize];
    render_ratatui_buffer_to_slice(
        &buf_img,
        styles.background,
        Some(&wp_bytes),
        &mut target_fb,
        fb_w,
        fb_h,
        stride,
        32,
        16,
        8,
        0,
    );

    // Pixel (0, 0) corresponds to transparent margin cell (0, 0) -> preserves full-res wallpaper
    let px0 = u32::from_ne_bytes(target_fb[0..4].try_into().unwrap());
    assert_eq!(px0, format_pixel_32(12, 210, 45, 16, 8, 0));
}
