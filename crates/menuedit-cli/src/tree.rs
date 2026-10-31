//! Visual menu tree hierarchy, node traversal, and structural tree mutations.

use bashmenu_core::{MenuItem, MenuItemType, MenuSchema, Submenu};
use std::collections::HashSet;

/// A flattened visible row in the menu editor tree
#[derive(Debug, Clone)]
pub struct VisibleNode {
    /// Index path in the menu hierarchy (e.g. `[1, 0]` = second root item's first child)
    pub path: Vec<usize>,
    /// Indentation depth
    pub depth: usize,
    /// Whether this node represents a submenu container
    pub has_children: bool,
    /// Whether this submenu is currently expanded
    pub is_expanded: bool,
    /// Menu item snapshot
    pub item: MenuItem,
}

/// Recursively flattens the menu tree into visible rows based on expanded state
pub fn flatten_tree(menu: &MenuSchema, expanded: &HashSet<Vec<usize>>) -> Vec<VisibleNode> {
    let mut visible = Vec::new();
    flatten_items(&menu.items, &[], 0, expanded, &mut visible);
    visible
}

fn flatten_items(
    items: &[MenuItem],
    parent_path: &[usize],
    depth: usize,
    expanded: &HashSet<Vec<usize>>,
    out: &mut Vec<VisibleNode>,
) {
    for (idx, item) in items.iter().enumerate() {
        let mut path = parent_path.to_vec();
        path.push(idx);

        let has_children = item.r#type == MenuItemType::Submenu || item.submenu.is_some();
        let is_expanded = has_children && expanded.contains(&path);

        out.push(VisibleNode {
            path: path.clone(),
            depth,
            has_children,
            is_expanded,
            item: item.clone(),
        });

        if is_expanded {
            if let Some(sub) = &item.submenu {
                flatten_items(&sub.items, &path, depth + 1, expanded, out);
            }
        }
    }
}

/// Toggles expansion for all submenu nodes across the tree
pub fn toggle_all_nodes(menu: &MenuSchema, expanded: &mut HashSet<Vec<usize>>) {
    let all_paths = collect_all_submenu_paths(&menu.items, &[]);
    let all_expanded = all_paths.iter().all(|p| expanded.contains(p));
    if all_expanded {
        expanded.clear();
    } else {
        for p in all_paths {
            expanded.insert(p);
        }
    }
}

fn collect_all_submenu_paths(items: &[MenuItem], parent_path: &[usize]) -> Vec<Vec<usize>> {
    let mut paths = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let mut path = parent_path.to_vec();
        path.push(idx);
        if item.r#type == MenuItemType::Submenu || item.submenu.is_some() {
            paths.push(path.clone());
            if let Some(sub) = &item.submenu {
                paths.extend(collect_all_submenu_paths(&sub.items, &path));
            }
        }
    }
    paths
}

/// Retrieves a reference to an item by index path
pub fn find_item<'a>(menu: &'a MenuSchema, path: &[usize]) -> Option<&'a MenuItem> {
    if path.is_empty() {
        return None;
    }
    let mut current_items = &menu.items;
    for (i, &idx) in path.iter().enumerate() {
        let item = current_items.get(idx)?;
        if i == path.len() - 1 {
            return Some(item);
        }
        if let Some(sub) = &item.submenu {
            current_items = &sub.items;
        } else {
            return None;
        }
    }
    None
}

/// Retrieves a mutable reference to an item by index path
pub fn find_item_mut<'a>(menu: &'a mut MenuSchema, path: &[usize]) -> Option<&'a mut MenuItem> {
    if path.is_empty() {
        return None;
    }
    let mut current_items = &mut menu.items;
    for (i, &idx) in path.iter().enumerate() {
        if i == path.len() - 1 {
            return current_items.get_mut(idx);
        }
        let item = current_items.get_mut(idx)?;
        if let Some(sub) = &mut item.submenu {
            current_items = &mut sub.items;
        } else {
            return None;
        }
    }
    None
}

/// Swaps an item with its preceding sibling
pub fn move_up_sibling(menu: &mut MenuSchema, path: &[usize]) -> Option<Vec<usize>> {
    if path.is_empty() {
        return None;
    }
    let last_idx = *path.last()?;
    if last_idx == 0 {
        return None;
    }

    let parent_path = &path[..path.len() - 1];
    let items = get_container_items_mut(menu, parent_path)?;
    items.swap(last_idx, last_idx - 1);

    let mut new_path = parent_path.to_vec();
    new_path.push(last_idx - 1);
    Some(new_path)
}

/// Swaps an item with its subsequent sibling
pub fn move_down_sibling(menu: &mut MenuSchema, path: &[usize]) -> Option<Vec<usize>> {
    if path.is_empty() {
        return None;
    }
    let last_idx = *path.last()?;
    let parent_path = &path[..path.len() - 1];
    let items = get_container_items_mut(menu, parent_path)?;
    if last_idx + 1 >= items.len() {
        return None;
    }

    items.swap(last_idx, last_idx + 1);
    let mut new_path = parent_path.to_vec();
    new_path.push(last_idx + 1);
    Some(new_path)
}

/// Indents an item into the preceding sibling submenu
pub fn indent_item(
    menu: &mut MenuSchema,
    path: &[usize],
    expanded: &mut HashSet<Vec<usize>>,
) -> Option<Vec<usize>> {
    if path.is_empty() {
        return None;
    }
    let last_idx = *path.last()?;
    if last_idx == 0 {
        return None;
    }

    let parent_path = &path[..path.len() - 1];
    let items = get_container_items_mut(menu, parent_path)?;

    // Check if preceding sibling is a submenu
    let prev_idx = last_idx - 1;
    if items[prev_idx].r#type != MenuItemType::Submenu && items[prev_idx].submenu.is_none() {
        return None;
    }

    let item_to_move = items.remove(last_idx);
    let prev_item = &mut items[prev_idx];
    if prev_item.submenu.is_none() {
        prev_item.submenu = Some(Submenu {
            title: prev_item.label.clone(),
            items: Vec::new(),
        });
    }

    let sub = prev_item.submenu.as_mut().unwrap();
    sub.items.push(item_to_move);
    let new_child_idx = sub.items.len() - 1;

    let mut target_parent = parent_path.to_vec();
    target_parent.push(prev_idx);
    expanded.insert(target_parent.clone());

    let mut new_path = target_parent;
    new_path.push(new_child_idx);
    Some(new_path)
}

/// Outdents an item from a child submenu back into its parent container
pub fn outdent_item(menu: &mut MenuSchema, path: &[usize]) -> Option<Vec<usize>> {
    if path.len() < 2 {
        return None; // Cannot outdent root-level items
    }

    let item_idx = *path.last()?;
    let sub_path = &path[..path.len() - 1];
    let parent_idx = *sub_path.last()?;
    let grandparent_path = &sub_path[..sub_path.len() - 1];

    // Remove from child submenu
    let item_to_move = {
        let sub_items = get_container_items_mut(menu, sub_path)?;
        if item_idx >= sub_items.len() {
            return None;
        }
        sub_items.remove(item_idx)
    };

    // Insert into grandparent container directly after the parent submenu
    let grand_items = get_container_items_mut(menu, grandparent_path)?;
    let insert_pos = (parent_idx + 1).min(grand_items.len());
    grand_items.insert(insert_pos, item_to_move);

    let mut new_path = grandparent_path.to_vec();
    new_path.push(insert_pos);
    Some(new_path)
}

/// Deletes an item by path
pub fn delete_item(menu: &mut MenuSchema, path: &[usize]) -> Option<MenuItem> {
    if path.is_empty() {
        return None;
    }
    let last_idx = *path.last()?;
    let parent_path = &path[..path.len() - 1];
    let items = get_container_items_mut(menu, parent_path)?;
    if last_idx < items.len() {
        Some(items.remove(last_idx))
    } else {
        None
    }
}

/// Inserts a new item directly after the current node
pub fn insert_item_after(menu: &mut MenuSchema, path: &[usize], new_item: MenuItem) -> Vec<usize> {
    if path.is_empty() {
        menu.items.push(new_item);
        return vec![menu.items.len() - 1];
    }

    let last_idx = *path.last().unwrap();
    let parent_path = &path[..path.len() - 1];
    if let Some(items) = get_container_items_mut(menu, parent_path) {
        let insert_idx = (last_idx + 1).min(items.len());
        items.insert(insert_idx, new_item);
        let mut new_path = parent_path.to_vec();
        new_path.push(insert_idx);
        new_path
    } else {
        menu.items.push(new_item);
        vec![menu.items.len() - 1]
    }
}

/// Mutates an item's type, migrating payloads and initializing/removing containers
pub fn mutate_item_type(item: &mut MenuItem, new_type_str: &str) {
    let old_type = item.r#type.clone();

    if new_type_str == "{divider}" || new_type_str == "divider" {
        item.r#type = MenuItemType::Divider;
        item.label = "{divider}".to_string();
        item.command = None;
        item.action = None;
        item.script = None;
        item.file = None;
        item.submenu = None;
        return;
    }

    let target_type = match new_type_str {
        "submenu" => MenuItemType::Submenu,
        "command" => MenuItemType::Command,
        "script" => MenuItemType::Script,
        "config" => MenuItemType::Config,
        "toggle" => MenuItemType::Toggle,
        "radio" => MenuItemType::Radio,
        "editor" => MenuItemType::Editor,
        "notebook" => MenuItemType::Notebook,
        "exit" => MenuItemType::Exit,
        "back" => MenuItemType::Back,
        "theme_selector" => MenuItemType::ThemeSelector,
        "python" => MenuItemType::Python,
        other => MenuItemType::Other(other.to_string()),
    };

    // Migrate payload
    match &target_type {
        MenuItemType::Submenu => {
            if item.submenu.is_none() {
                item.submenu = Some(Submenu {
                    title: item.label.clone(),
                    items: Vec::new(),
                });
            }
        }
        MenuItemType::Command => {
            if item.command.is_none() {
                item.command = item
                    .action
                    .take()
                    .or(item.script.take())
                    .or(item.file.take());
            }
            if old_type == MenuItemType::Submenu {
                item.submenu = None;
            }
        }
        MenuItemType::Script => {
            if item.script.is_none() {
                item.script = item
                    .action
                    .take()
                    .or(item.command.take())
                    .or(item.file.take());
            }
            if old_type == MenuItemType::Submenu {
                item.submenu = None;
            }
        }
        MenuItemType::Editor | MenuItemType::Notebook => {
            if item.file.is_none() {
                item.file = item
                    .action
                    .take()
                    .or(item.command.take())
                    .or(item.script.take());
            }
            if old_type == MenuItemType::Submenu {
                item.submenu = None;
            }
        }
        _ => {
            if old_type == MenuItemType::Submenu {
                item.submenu = None;
            }
        }
    }

    item.r#type = target_type;
}

fn get_container_items_mut<'a>(
    menu: &'a mut MenuSchema,
    parent_path: &[usize],
) -> Option<&'a mut Vec<MenuItem>> {
    if parent_path.is_empty() {
        return Some(&mut menu.items);
    }
    let mut current_items = &mut menu.items;
    for &idx in parent_path {
        let item = current_items.get_mut(idx)?;
        let sub = item.submenu.as_mut()?;
        current_items = &mut sub.items;
    }
    Some(current_items)
}
