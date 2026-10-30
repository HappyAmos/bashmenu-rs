use bashmenu_core::{load_themes, MenuItem, MenuItemType, MenuSchema, WallpaperConfig};
use bashmenu_tui::{
    color_to_rgb, composite_buffer, resolve_wallpaper_path, MenuView, ResolvedThemeStyles,
    WallpaperCell, WallpaperCompositor, WallpaperGrid,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::path::PathBuf;

#[test]
fn test_color_to_rgb() {
    assert_eq!(color_to_rgb(Color::Rgb(10, 20, 30)), (10, 20, 30));
    assert_eq!(color_to_rgb(Color::Black), (0, 0, 0));
    // Index 235 is 24-step grayscale: 8 + (235 - 232) * 10 = 38
    assert_eq!(color_to_rgb(Color::Indexed(235)), (38, 38, 38));
}

#[test]
fn test_resolve_wallpaper_path() {
    let resolved = resolve_wallpaper_path("assets/wallpapers/alaska-mountain.png");
    assert!(resolved.is_some(), "alaska-mountain.png should be resolved");
    let p = resolved.unwrap();
    assert!(p.is_file(), "Resolved path must be an actual file: {:?}", p);

    let nonexistent = resolve_wallpaper_path("assets/wallpapers/nonexistent_image_123.png");
    assert!(
        nonexistent.is_none(),
        "Non-existent path should resolve to None"
    );
}

#[test]
fn test_wallpaper_compositor_get_grid_and_caching() {
    let config = WallpaperConfig {
        enabled: true,
        image: Some("assets/wallpapers/alaska-mountain.png".to_string()),
        opacity: 0.85,
        scaling: "stretch".to_string(),
        clustering: 0,
        quantization: 1,
        mode: Some("halfblock".to_string()),
    };

    let mut compositor = WallpaperCompositor::new(config);
    let theme_bg = Color::Indexed(235);

    let cell0 = {
        let grid = compositor.get_grid(40, 20, theme_bg);
        assert!(grid.is_some(), "Grid should be computed successfully");
        let grid_ref = grid.unwrap();
        assert_eq!(grid_ref.width, 40);
        assert_eq!(grid_ref.height, 20);
        assert_eq!(grid_ref.cells.len(), 40 * 20);
        grid_ref.cells[0]
    };

    // Verify subsequent call uses cache and produces identical cell
    let grid2 = compositor.get_grid(40, 20, theme_bg);
    assert!(grid2.is_some());
    assert_eq!(grid2.unwrap().cells[0], cell0);
}

#[test]
fn test_composite_buffer_padding_and_glyphs() {
    let area = Rect::new(0, 0, 10, 3);
    let mut buf = Buffer::empty(area);

    let theme_bg = Color::Rgb(20, 20, 20);
    let highlight_bg = Color::Rgb(250, 250, 250);

    // Row 0: all padding spaces
    for x in 0..10 {
        buf[(x, 0)].set_symbol(" ");
        buf[(x, 0)].set_bg(theme_bg);
    }

    // Row 1: "  Abc   " with padding on sides and text in center
    buf[(0, 1)].set_symbol(" ");
    buf[(1, 1)].set_symbol(" ");
    buf[(2, 1)].set_symbol("A").set_fg(Color::Yellow);
    buf[(3, 1)].set_symbol("b").set_fg(Color::Yellow);
    buf[(4, 1)].set_symbol("c").set_fg(Color::Yellow);
    buf[(5, 1)].set_symbol(" ");
    buf[(6, 1)].set_symbol(" ");
    buf[(7, 1)].set_symbol(" ");
    buf[(8, 1)].set_symbol(" ");
    buf[(9, 1)].set_symbol(" ");

    // Row 2: Selected row with explicit highlight background
    for x in 0..10 {
        buf[(x, 2)]
            .set_symbol("X")
            .set_bg(highlight_bg)
            .set_fg(Color::Black);
    }

    let default_cell = WallpaperCell {
        upper: Color::Rgb(100, 150, 200),
        lower: Color::Rgb(50, 75, 100),
        avg_bg: Color::Rgb(75, 112, 150),
    };

    let grid = WallpaperGrid {
        width: 10,
        height: 3,
        cells: vec![default_cell; 10 * 3],
    };

    composite_buffer(area, &mut buf, &grid, theme_bg);

    // Row 0: Padding spaces should have become '▀' with upper/lower colors
    for x in 0..10 {
        assert_eq!(buf[(x, 0)].symbol(), "▀");
        assert_eq!(buf[(x, 0)].fg, default_cell.upper);
        assert_eq!(buf[(x, 0)].bg, default_cell.lower);
    }

    // Row 1: Leading padding (x=0,1) became '▀'
    assert_eq!(buf[(0, 1)].symbol(), "▀");
    assert_eq!(buf[(1, 1)].symbol(), "▀");

    // Row 1: Text glyphs kept symbol and fg, while bg became avg_bg
    assert_eq!(buf[(2, 1)].symbol(), "A");
    assert_eq!(buf[(2, 1)].fg, Color::Yellow);
    assert_eq!(buf[(2, 1)].bg, default_cell.avg_bg);

    // Row 1: Trailing padding (x=5..9) became '▀'
    for x in 5..10 {
        assert_eq!(buf[(x, 1)].symbol(), "▀");
    }

    // Row 2: Selected row kept explicit highlight bg and symbol
    for x in 0..10 {
        assert_eq!(buf[(x, 2)].symbol(), "X");
        assert_eq!(buf[(x, 2)].bg, highlight_bg);
        assert_eq!(buf[(x, 2)].fg, Color::Black);
    }
}

#[test]
fn test_menu_view_renders_with_wallpaper() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    std::env::set_var("BASHMENU_DIR", &repo_root);

    let themes_path = repo_root.join("bashmenu.themes");
    let themes = load_themes(&themes_path).expect("load themes");
    let theme = themes
        .get("alaska-mountain")
        .expect("alaska-mountain theme");

    let wp_config = theme.wallpaper_config();
    assert!(
        wp_config.is_some(),
        "alaska-mountain must have wallpaper_config"
    );

    let styles = ResolvedThemeStyles::from_theme(theme, true);
    let menu = MenuSchema {
        version: "0.0.1".to_string(),
        title: "Test Wallpaper Menu".to_string(),
        items: vec![
            MenuItem {
                label: "Applications".to_string(),
                r#type: MenuItemType::Submenu,
                icon: Some("🚀".to_string()),
                ..Default::default()
            },
            MenuItem {
                label: "Exit".to_string(),
                r#type: MenuItemType::Command,
                icon: Some("❌".to_string()),
                ..Default::default()
            },
        ],
        extra: Default::default(),
    };

    let mut menu_view =
        MenuView::new(menu, styles, true, "Status | Gutter".to_string()).with_wallpaper(wp_config);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    menu_view.render(area, &mut buf);

    // Check that half-block compositing was applied in buffer
    let mut halfblock_count = 0;
    for y in 0..24 {
        for x in 0..80 {
            if buf[(x, y)].symbol() == "▀" {
                halfblock_count += 1;
            }
        }
    }

    assert!(
        halfblock_count > 100,
        "Buffer must contain halfblock TrueColor wallpaper cells (found {})",
        halfblock_count
    );
}

#[test]
fn test_wallpaper_scaling_modes() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    std::env::set_var("BASHMENU_DIR", &repo_root);

    let modes = ["cover", "fit", "stretch", "center", "tile"];
    for mode in modes {
        let config = WallpaperConfig {
            enabled: true,
            image: Some("assets/wallpapers/alaska-mountain.png".to_string()),
            opacity: 0.5,
            scaling: mode.to_string(),
            clustering: 0,
            quantization: 1,
            mode: Some("halfblock".to_string()),
        };

        let mut comp = WallpaperCompositor::new(config);
        let grid = comp.get_grid(30, 15, Color::Black);
        assert!(
            grid.is_some(),
            "Scaling mode {} must produce a valid WallpaperGrid",
            mode
        );
        let g = grid.unwrap();
        assert_eq!(g.width, 30);
        assert_eq!(g.height, 15);
    }
}

#[test]
fn test_resolve_wallpaper_user_overrides() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    std::env::set_var("BASHMENU_DIR", &repo_root);

    let themes = load_themes(repo_root.join("bashmenu.themes")).expect("load themes");
    let theme = themes
        .get("alaska-mountain")
        .expect("alaska-mountain theme");

    let mut config = bashmenu_core::BashMenuConfig::default();

    // 1. By default, theme wallpaper resolves
    let resolved = config.resolve_wallpaper(Some(theme));
    assert!(resolved.is_some());
    assert_eq!(
        resolved.as_ref().unwrap().image.as_deref(),
        Some("assets/wallpapers/alaska-mountain.png")
    );

    // 2. User explicitly disables wallpaper in settings
    config.settings.background = Some(bashmenu_core::WallpaperSetting::Config(WallpaperConfig {
        enabled: false,
        ..Default::default()
    }));
    assert!(config.resolve_wallpaper(Some(theme)).is_none());

    // 3. User overrides image and opacity
    config.settings.background = Some(bashmenu_core::WallpaperSetting::Config(WallpaperConfig {
        enabled: true,
        image: Some("assets/wallpapers/twilight.png".to_string()),
        opacity: 0.9,
        ..Default::default()
    }));
    let overridden = config.resolve_wallpaper(Some(theme)).unwrap();
    assert_eq!(
        overridden.image.as_deref(),
        Some("assets/wallpapers/twilight.png")
    );
    assert!((overridden.opacity - 0.9).abs() < f32::EPSILON);
}
