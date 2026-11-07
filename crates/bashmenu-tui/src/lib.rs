//! Reusable TUI primitives, widgets, terminal guards, modals, and panic hooks.

pub mod ansi;
pub mod bbcode;
pub mod borders;
pub mod clipboard;
pub mod framebuffer;
pub mod gutter;
pub mod marquee;
pub mod menu_view;
pub mod modals;
pub mod shortcuts_overlay;
pub mod style;
pub mod terminal;
pub mod text_input;
pub mod toast;
pub mod transition;
pub mod wallpaper;

pub use ansi::ansi_to_line;
pub use bbcode::{
    is_bbcode_tag, parse_bbcode_spans, parse_color, split_label_brackets, strip_bbcode,
    truncate_to_width, visible_text_len,
};
pub use borders::ResolvedWindowBorders;
pub use clipboard::{base64_encode, get_clipboard_text, set_clipboard_text};
pub use framebuffer::{
    format_pixel_16, format_pixel_32, is_framebuffer_available, FramebufferDevice,
};
pub use gutter::{
    format_spread_gutter_line, parse_help_tokens, spread_gutter_line_layout,
    wrap_spread_help_gutter, wrap_spread_help_gutter_layout,
};
pub use marquee::{format_marquee_spacer, get_marquee_slice, MarqueeTracker};
pub use menu_view::{MenuAction, MenuView};
pub use modals::{
    compute_modal_inner, compute_modal_rect, is_close_button_clicked, render_animated_modal_frame,
    render_modal_frame, wrap_prompt_text, AsciiGridModal, BackgroundConfigAction,
    BackgroundConfigModal, BackgroundFocus, ColorGridModal, ColorPickerModal, ColorPickerTab,
    ConfirmChoice, ConfirmModal, FilePickerModal, HelpModal, InputModal, MessageModal,
    NoteConflictChoice, NoteConflictModal, NoteEditorAction, NoteEditorModal, NoteViewerAction,
    NoteViewerModal, RadioModal, StreamOutputAction, StreamOutputModal, StreamOutputMsg,
    ThemePickerModal, ToggleChoice, ToggleModal, MODE_OPTIONS, SCALING_OPTIONS,
};
pub use shortcuts_overlay::{key_event_to_token, ShortcutDispatchResult, ShortcutDispatcher};
pub use style::{
    ansi_index_to_color, ansi_name_to_color, downsample_color_to_16, downsample_color_to_8,
    downsample_color_to_tier, parse_color_value, parse_color_value_with_tier, parse_style_pair,
    parse_style_pair_with_tier, ColorTier, ResolvedThemeStyles,
};
pub use terminal::TerminalGuard;
pub use text_input::TextInput;
pub use toast::{ActiveToast, ToastRack};
pub use transition::{
    dim_color, interpolate_zoom_rect, ModalAnimation, TransitionEffect, TransitionSpeed,
};
pub use wallpaper::{
    color_to_rgb, composite_buffer, derive_palette_from_image, downsample_color_value_to_16,
    downsample_color_value_to_8, downsample_palette_to_16, downsample_palette_to_8,
    resolve_wallpaper_path, rgb_to_ansi16_name, rgb_to_ansi8_name, rgb_to_xterm256,
    supports_quadrant_blocks, WallpaperCell, WallpaperCompositor, WallpaperGrid, ANSI16_NAMES,
    ANSI8_NAMES,
};

use std::panic;

/// Registers global panic hook restoring raw mode and alternate screen
/// before printing panic diagnostics.
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
