//! Library module for MenuEdit core tree navigation and menu manipulation.

use anyhow::Result;
use bashmenu_core::{load_menu, save_menu, MenuItem, MenuItemType, MenuSchema, Submenu};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EditorLevel {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub selected_idx: usize,
}

pub struct MenuEditorState {
    pub menu_path: PathBuf,
    pub root_menu: MenuSchema,
    pub stack: Vec<EditorLevel>,
    pub is_modified: bool,
    pub status_message: String,
}

impl MenuEditorState {
    pub fn new(menu_path: PathBuf) -> Self {
        let root_menu = if menu_path.exists() {
            load_menu(&menu_path).unwrap_or_default()
        } else {
            MenuSchema::default()
        };

        let initial_level = EditorLevel {
            title: root_menu.title.clone(),
            items: root_menu.items.clone(),
            selected_idx: 0,
        };

        Self {
            menu_path,
            root_menu,
            stack: vec![initial_level],
            is_modified: false,
            status_message: "Ready".to_string(),
        }
    }

    pub fn current_level(&self) -> &EditorLevel {
        self.stack.last().expect("Stack must never be empty")
    }

    pub fn current_level_mut(&mut self) -> &mut EditorLevel {
        self.stack.last_mut().expect("Stack must never be empty")
    }

    pub fn current_items(&self) -> &[MenuItem] {
        &self.current_level().items
    }

    pub fn selected_index(&self) -> usize {
        self.current_level().selected_idx
    }

    pub fn move_up(&mut self) {
        let level = self.current_level_mut();
        if level.selected_idx > 0 {
            level.selected_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let level = self.current_level_mut();
        if !level.items.is_empty() && level.selected_idx + 1 < level.items.len() {
            level.selected_idx += 1;
        }
    }

    pub fn swap_up(&mut self) {
        let level = self.current_level_mut();
        let idx = level.selected_idx;
        if idx > 0 && idx < level.items.len() {
            level.items.swap(idx, idx - 1);
            level.selected_idx = idx - 1;
            self.is_modified = true;
            self.status_message = "Moved item up".to_string();
        }
    }

    pub fn swap_down(&mut self) {
        let level = self.current_level_mut();
        let idx = level.selected_idx;
        if !level.items.is_empty() && idx + 1 < level.items.len() {
            level.items.swap(idx, idx + 1);
            level.selected_idx = idx + 1;
            self.is_modified = true;
            self.status_message = "Moved item down".to_string();
        }
    }

    pub fn add_item(&mut self, item: MenuItem) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            level.items.push(item);
            level.selected_idx = 0;
        } else {
            let insert_at = (level.selected_idx + 1).min(level.items.len());
            level.items.insert(insert_at, item);
            level.selected_idx = insert_at;
        }
        self.is_modified = true;
        self.status_message = "Added new item".to_string();
    }

    pub fn edit_selected(&mut self, label: String, command: Option<String>, icon: Option<String>) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return;
        }
        let idx = level.selected_idx;
        level.items[idx].label = label;
        level.items[idx].command = command;
        level.items[idx].icon = icon;
        self.is_modified = true;
        self.status_message = "Updated item properties".to_string();
    }

    pub fn delete_selected(&mut self) -> Option<MenuItem> {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return None;
        }
        let idx = level.selected_idx;
        let removed = level.items.remove(idx);
        if level.selected_idx >= level.items.len() && !level.items.is_empty() {
            level.selected_idx = level.items.len() - 1;
        }
        self.is_modified = true;
        self.status_message = format!("Deleted '{}'", removed.label);
        Some(removed)
    }

    pub fn enter_submenu(&mut self) -> bool {
        let sub_opt = {
            let level = self.current_level();
            if level.items.is_empty() {
                None
            } else {
                let item = &level.items[level.selected_idx];
                if item.r#type == MenuItemType::Submenu {
                    item.submenu.clone()
                } else {
                    None
                }
            }
        };

        if let Some(sub) = sub_opt {
            let new_level = EditorLevel {
                title: sub.title.clone(),
                items: sub.items,
                selected_idx: 0,
            };
            self.stack.push(new_level);
            self.status_message = format!("Entered submenu '{}'", sub.title);
            return true;
        }
        false
    }

    pub fn leave_submenu(&mut self) -> bool {
        if self.stack.len() <= 1 {
            return false;
        }
        let child_level = self.stack.pop().unwrap();
        // Sync child items back to the parent item's submenu
        let parent_level = self.stack.last_mut().unwrap();
        let idx = parent_level.selected_idx;
        if idx < parent_level.items.len() {
            parent_level.items[idx].submenu = Some(Submenu {
                title: child_level.title,
                items: child_level.items,
            });
        }
        self.status_message = "Returned to parent menu".to_string();
        true
    }

    /// Sync active editor tree to root_menu and persist to `bashmenu.mnu`
    pub fn save_to_disk(&mut self) -> Result<()> {
        // Unwind child levels to sync all nested items
        while self.stack.len() > 1 {
            self.leave_submenu();
        }
        self.root_menu.items = self.stack[0].items.clone();
        save_menu(&self.menu_path, &self.root_menu)?;
        self.is_modified = false;
        self.status_message = "Saved menu to disk".to_string();
        Ok(())
    }
}
