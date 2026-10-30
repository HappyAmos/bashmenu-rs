//! Reusable TUI primitives, widgets, terminal guards, modals, and panic safety hooks for BashMenu.

pub mod bbcode;
pub mod borders;
pub mod menu_view;
pub mod modals;
pub mod style;
pub mod terminal;
pub mod wallpaper;

pub use bbcode::{
    is_bbcode_tag, parse_bbcode_spans, parse_color, split_label_brackets, strip_bbcode,
    truncate_to_width, visible_text_len,
};
pub use borders::ResolvedWindowBorders;
pub use menu_view::{MenuAction, MenuView};
pub use modals::{
    render_modal_frame, AsciiGridModal, ColorGridModal, ConfirmModal, FilePickerModal, HelpModal,
    InputModal, MessageModal, ThemePickerModal,
};
pub use style::ResolvedThemeStyles;
pub use terminal::TerminalGuard;
pub use wallpaper::{
    color_to_rgb, composite_buffer, resolve_wallpaper_path, WallpaperCell, WallpaperCompositor,
    WallpaperGrid,
};

use std::panic;

/// Registers a global panic hook ensuring terminal raw mode and alternate screen
/// are cleanly restored before printing panic diagnostics.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_hook_registration() {
        install_panic_hook();
    }
}
