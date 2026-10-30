//! Core domain models, configuration loaders, and serializers for BashMenu.

pub mod config;
pub mod error;
pub mod menu;
pub mod theme;

pub use config::{load_config, save_config, BashMenuConfig, Settings, CONFIG_HEADER};
pub use error::{BashMenuError, Result};
pub use menu::{load_menu, save_menu, MenuItem, MenuItemType, MenuSchema, Submenu};
pub use theme::{load_themes, save_themes, Theme, ThemeCollection};
