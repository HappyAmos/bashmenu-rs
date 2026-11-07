//! Modal dialog framework and visual pickers.

pub mod background_config;
pub mod color_picker;
pub mod confirm;
pub mod file_picker;
pub mod frame;
pub mod grid_picker;
pub mod help;
pub mod input;
pub mod message;
pub mod note_conflict;
pub mod note_editor;
pub mod note_viewer;
pub mod radio;
pub mod stream_output;
pub mod theme_picker;
pub mod toggle;

pub use background_config::{
    BackgroundConfigAction, BackgroundConfigModal, BackgroundFocus, MODE_OPTIONS, SCALING_OPTIONS,
};
pub use color_picker::{ColorPickerModal, ColorPickerTab};
pub use confirm::{ConfirmChoice, ConfirmModal};
pub use file_picker::FilePickerModal;
pub use frame::{
    compute_modal_inner, compute_modal_rect, is_close_button_clicked, render_animated_modal_frame,
    render_modal_frame,
};
pub use grid_picker::{AsciiGridModal, ColorGridModal};
pub use help::HelpModal;
pub use input::{wrap_prompt_text, InputModal};
pub use message::MessageModal;
pub use note_conflict::{NoteConflictChoice, NoteConflictModal};
pub use note_editor::{NoteEditorAction, NoteEditorModal};
pub use note_viewer::{wrap_footer_tokens, NoteViewerAction, NoteViewerModal};
pub use radio::RadioModal;
pub use stream_output::{StreamOutputAction, StreamOutputModal, StreamOutputMsg};
pub use theme_picker::ThemePickerModal;
pub use toggle::{ToggleChoice, ToggleModal};
