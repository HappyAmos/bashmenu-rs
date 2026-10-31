//! Core domain models, configuration loaders, serializers, and macro engines for BashMenu.

pub mod config;
pub mod constants;
pub mod error;
pub mod glyph;
pub mod macro_engine;
pub mod menu;
pub mod notebook;
pub mod plugin;
pub mod theme;

pub use config::{load_config, save_config, BashMenuConfig, Settings, CONFIG_HEADER};
pub use constants::*;
pub use error::{BashMenuError, Result};
pub use glyph::{
    cp437_to_char, format_icon_slot, normalize_variation_selectors, parse_ascii_val, resolve_glyph,
    CP437_TABLE,
};
pub use macro_engine::{interpolate_placeholders, resolve_config_dot_path, MacroContext};
pub use menu::{load_menu, save_menu, MenuItem, MenuItemType, MenuSchema, Submenu};
pub use notebook::{
    add_or_update_note, build_notebook_submenu, delete_note, get_note_by_title, get_notes,
    load_notebook, rename_note, resolve_notebook_path, save_notebook, NoteItem, NotebookData,
    NotebookSchema,
};
pub use plugin::{
    PluginCacheEntry, PluginConfig, PluginDef, PluginDisplayRow, PluginEngine, PluginLayout,
};
pub use theme::{
    load_themes, save_themes, Theme, ThemeCollection, WallpaperConfig, WallpaperSetting,
    WindowBorders,
};
