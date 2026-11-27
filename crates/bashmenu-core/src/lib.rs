//! Core domain models, configuration loaders, serializers, and macro engine.

pub mod config;
pub mod constants;
pub mod error;
pub mod glyph;
pub mod i18n;
pub mod interactive_md;
pub mod macro_engine;
pub mod menu;
pub mod menu_merge;
pub mod notebook;
pub mod plugin;
pub mod shortcuts;
pub mod theme;

pub use config::{
    get_user_config_dir, load_config, locate_companion_tool, resolve_app_path,
    resolve_path_with_local, save_config, BashMenuConfig, Settings, CONFIG_HEADER,
};
pub use constants::*;
pub use error::{BashMenuError, Result};
pub use glyph::{
    cp437_to_char, format_icon_slot, normalize_variation_selectors, parse_ascii_val, resolve_glyph,
    resolve_script_path, run_glyphs_search, CP437_TABLE,
};
pub use i18n::{
    current_locale, discover_locales, extract_locale_from_args, init_locale_from_env,
    interpolate_string, parse_and_flatten_yaml, preinit_locale_from_args, resolve_locale_path,
    set_locale, translate, translate_with, LocaleCatalog, LocaleInfo, DEFAULT_EN_US_YAML,
};
pub use interactive_md::{
    find_button_macro_span, find_checkbox_macro_span, find_input_macro_span, find_radio_macro_span,
    has_interactive_markdown_form, has_markdown_form_inputs, is_valid_bash_identifier,
    parse_interactive_markdown, parse_interactive_markdown_doc, parse_markdown_form_blocks,
    substitute_id_placeholders, transpile_bashmenu_bash, ButtonSpec, ButtonType, CheckboxSpec,
    InputFieldSpec, InteractiveMarkdownDoc, MarkdownFormBlock, MarkdownSegment, RadioLayout,
    RadioSpec, ScriptBlock, UiRpcRequest, UiRpcResponse, UI_RPC_SOCKET_ENV,
};
pub use macro_engine::{
    apply_dir_picker_macros, apply_dir_picker_macros_raw, apply_file_picker_macros,
    apply_file_picker_macros_raw, clear_command_cache, detect_color_mode, detect_tty_mode,
    evaluate_color_mode, evaluate_tty_mode, extract_first_message_macro, extract_info_macro_path,
    extract_picker_spec, extract_toasts, find_info_macro_span, find_message_macro_span,
    full_file_location, interpolate_placeholders, parse_markdown_frontmatter,
    parse_markdown_frontmatter_full, parse_picker_macro, posix_shell_quote,
    resolve_config_dot_path, resolve_dev_dir, resolve_workspace_version, shell_quote_for_context,
    MacroContext, MarkdownFrontmatter, MessageMacroData, PickerSpec, ToastNotification,
    ToastSeverity,
};
pub use menu::{
    load_menu, save_menu, ConfirmAction, ConfirmActionDetails, ExecutionMode, MenuItem,
    MenuItemType, MenuSchema, Submenu,
};
pub use menu_merge::{
    apply_merge, compute_menu_diff, compute_property_diffs, items_match, ConflictChoice,
    ItemConflict, ItemPath, MenuMergePlan, MergeDecisions, NewUpstreamItem, PropertyDiff,
};
pub use notebook::{
    add_or_update_note, build_notebook_submenu, delete_note, get_note_by_title, get_notes,
    load_notebook, merge_note_content, rename_note, resolve_notebook_path, save_notebook, NoteItem,
    NoteMergeResult, NoteSwapFile, NotebookData, NotebookSchema,
};
pub use plugin::{
    PluginCacheEntry, PluginConfig, PluginDef, PluginDisplayRow, PluginEngine, PluginLayout,
};
pub use shortcuts::{
    builtin_action_locale_key, load_shortcuts, normalize_key_spec, normalize_single_key_token,
    resolve_builtin_description, resolve_shortcuts_path, save_shortcuts, validate_key_spec,
    ShortcutEntry, ShortcutsConfig, BUILTIN_ACTIONS, SHORTCUTS_HEADER,
};
pub use theme::{
    activity_override_tokens, default_activity_overrides_map, is_random_theme_spec,
    is_scalar_color_token, load_themes, pick_random_theme, save_themes, DividerConfig,
    MarqueeConfig, Theme, ThemeCollection, ToastConfig, ToastStyle, WallpaperConfig,
    WallpaperSetting, WindowBorders, ACTIVITY_OVERRIDE_KEYS, DEFAULT_ITEM_ICON, RANDOM_THEME_LABEL,
};
