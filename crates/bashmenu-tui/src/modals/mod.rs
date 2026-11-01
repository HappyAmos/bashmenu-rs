//! Modal dialog framework and visual pickers.

pub mod confirm;
pub mod file_picker;
pub mod frame;
pub mod grid_picker;
pub mod help;
pub mod input;
pub mod message;
pub mod note_editor;
pub mod note_viewer;
pub mod radio;
pub mod stream_output;
pub mod theme_picker;
pub mod toggle;

pub use confirm::{ConfirmChoice, ConfirmModal};
pub use file_picker::FilePickerModal;
pub use frame::{compute_modal_inner, is_close_button_clicked, render_modal_frame};
pub use grid_picker::{AsciiGridModal, ColorGridModal};
pub use help::HelpModal;
pub use input::InputModal;
pub use message::MessageModal;
pub use note_editor::{NoteEditorAction, NoteEditorModal};
pub use note_viewer::{wrap_footer_tokens, NoteViewerAction, NoteViewerModal};
pub use radio::RadioModal;
pub use stream_output::{StreamOutputAction, StreamOutputModal, StreamOutputMsg};
pub use theme_picker::ThemePickerModal;
pub use toggle::{ToggleChoice, ToggleModal};
