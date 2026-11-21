//! Semantic recursive menu merging engine for `bashmenu.mnu`.
//!
//! Provides two-way and three-way diffing, conflict detection, and non-destructive
//! synthesis between user customizations and upstream vanilla menu definitions.

use crate::menu::{MenuItem, MenuItemType, MenuSchema, Submenu};
use std::collections::HashMap;

/// Path identifier in the menu hierarchy (e.g. `["Options & Settings", "Bashmenu Settings", "Marquee Speed"]`).
pub type ItemPath = Vec<String>;

/// Represents a property-level difference between user and vanilla versions of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDiff {
    /// Name of the property that differs
    pub property_name: String,
    /// Value in user menu
    pub user_value: String,
    /// Value in vanilla menu
    pub vanilla_value: String,
}

/// A conflict detected between a user item and an updated vanilla item.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemConflict {
    /// Hierarchical title breadcrumbs to this item
    pub path: ItemPath,
    /// The user's version of the item
    pub user_item: MenuItem,
    /// Upstream vanilla's version of the item
    pub vanilla_item: MenuItem,
    /// Detailed list of differing properties
    pub property_diffs: Vec<PropertyDiff>,
}

/// A newly introduced item in the upstream vanilla menu not present in the user menu.
#[derive(Debug, Clone, PartialEq)]
pub struct NewUpstreamItem {
    /// Hierarchical title breadcrumbs where this item will be inserted
    pub path: ItemPath,
    /// The new vanilla item to be added
    pub item: MenuItem,
    /// Label of the item in vanilla menu that immediately precedes this item, if any
    pub predecessor_anchor: Option<String>,
}

/// Resolution choice for an item conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictChoice {
    /// Keep the user's customized item
    #[default]
    KeepUser,
    /// Take the upstream vanilla item
    TakeVanilla,
    /// Keep both items in the menu
    KeepBoth,
}

/// Collected decisions for resolving the merge.
#[derive(Debug, Clone, Default)]
pub struct MergeDecisions {
    /// Map of upstream item path string -> whether to include it (true = add, false = skip)
    pub additions: HashMap<String, bool>,
    /// Map of conflict item path string -> chosen resolution
    pub conflicts: HashMap<String, ConflictChoice>,
}

impl MergeDecisions {
    /// Formats a path slice into a canonical string key (e.g. "Options & Settings > Marquee Speed")
    pub fn path_key(path: &[String]) -> String {
        path.join(" > ")
    }

    /// Checks if a new addition is accepted (defaults to true if unspecified)
    pub fn is_addition_accepted(&self, path: &[String]) -> bool {
        let key = Self::path_key(path);
        self.additions.get(&key).copied().unwrap_or(true)
    }

    /// Returns the conflict resolution choice (defaults to KeepUser if unspecified)
    pub fn conflict_resolution(&self, path: &[String]) -> ConflictChoice {
        let key = Self::path_key(path);
        self.conflicts
            .get(&key)
            .copied()
            .unwrap_or(ConflictChoice::KeepUser)
    }
}

/// Summary of all differences detected between user and vanilla menus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MenuMergePlan {
    /// New items present in vanilla but absent in user menu
    pub new_items: Vec<NewUpstreamItem>,
    /// Conflicting items present in both with differing properties
    pub conflicts: Vec<ItemConflict>,
}

/// Normalizes label by trimming and stripping parameter brackets (e.g. `Notes [...]` -> `Notes`).
pub fn strip_param_brackets(label: &str) -> &str {
    if let Some(idx) = label.find('[') {
        label[..idx].trim()
    } else {
        label.trim()
    }
}

/// Normalizes a string for heuristic comparison (case-insensitive, trimmed).
fn normalize_for_comparison(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Extracts the primary action, command, script, or file target from a MenuItem.
fn item_target(item: &MenuItem) -> Option<&str> {
    item.action
        .as_deref()
        .or(item.command.as_deref())
        .or(item.script.as_deref())
        .or(item.file.as_deref())
}

/// Extracts config target key if this is a config/toggle/radio item.
fn item_config_target(item: &MenuItem) -> Option<&str> {
    item.extra
        .get("target")
        .or_else(|| item.extra.get("key"))
        .or_else(|| item.extra.get("config_key"))
        .and_then(|v| v.as_str())
}

/// Determines whether two menu items represent the same logical menu entry.
pub fn items_match(a: &MenuItem, b: &MenuItem) -> bool {
    if a.is_divider() || b.is_divider() {
        return false;
    }

    // 1. Submenus match if their titles or labels match
    if a.r#type == MenuItemType::Submenu && b.r#type == MenuItemType::Submenu {
        let a_title = a
            .submenu
            .as_ref()
            .map(|s| s.title.as_str())
            .unwrap_or(&a.label);
        let b_title = b
            .submenu
            .as_ref()
            .map(|s| s.title.as_str())
            .unwrap_or(&b.label);
        return normalize_for_comparison(a_title) == normalize_for_comparison(b_title)
            || normalize_for_comparison(strip_param_brackets(&a.label))
                == normalize_for_comparison(strip_param_brackets(&b.label));
    }

    if a.r#type == MenuItemType::Submenu || b.r#type == MenuItemType::Submenu {
        return false;
    }

    // 2. Exact or stripped label match
    let a_clean_label = strip_param_brackets(&a.label);
    let b_clean_label = strip_param_brackets(&b.label);
    if !a_clean_label.is_empty()
        && normalize_for_comparison(a_clean_label) == normalize_for_comparison(b_clean_label)
    {
        return true;
    }

    // 3. Config target key match (e.g. settings.locale)
    if let (Some(at), Some(bt)) = (item_config_target(a), item_config_target(b)) {
        if !at.is_empty() && at == bt {
            return true;
        }
    }

    // 4. Command/action payload match
    if let (Some(at), Some(bt)) = (item_target(a), item_target(b)) {
        if !at.is_empty() && at == bt {
            return true;
        }
    }

    false
}

/// Compares properties of two matched items and returns any differences.
pub fn compute_property_diffs(user: &MenuItem, vanilla: &MenuItem) -> Vec<PropertyDiff> {
    let mut diffs = Vec::new();

    if user.label != vanilla.label {
        diffs.push(PropertyDiff {
            property_name: "label".to_string(),
            user_value: user.label.clone(),
            vanilla_value: vanilla.label.clone(),
        });
    }

    if user.icon != vanilla.icon {
        diffs.push(PropertyDiff {
            property_name: "icon".to_string(),
            user_value: user.icon.clone().unwrap_or_else(|| "<none>".to_string()),
            vanilla_value: vanilla.icon.clone().unwrap_or_else(|| "<none>".to_string()),
        });
    }

    if user.r#type != vanilla.r#type {
        diffs.push(PropertyDiff {
            property_name: "type".to_string(),
            user_value: format!("{:?}", user.r#type),
            vanilla_value: format!("{:?}", vanilla.r#type),
        });
    }

    let u_target = item_target(user);
    let v_target = item_target(vanilla);
    if u_target != v_target {
        diffs.push(PropertyDiff {
            property_name: "action/target".to_string(),
            user_value: u_target.unwrap_or("<none>").to_string(),
            vanilla_value: v_target.unwrap_or("<none>").to_string(),
        });
    }

    if user.interactive != vanilla.interactive {
        diffs.push(PropertyDiff {
            property_name: "interactive".to_string(),
            user_value: format!("{:?}", user.interactive),
            vanilla_value: format!("{:?}", vanilla.interactive),
        });
    }

    if user.quiet != vanilla.quiet {
        diffs.push(PropertyDiff {
            property_name: "quiet".to_string(),
            user_value: format!("{:?}", user.quiet),
            vanilla_value: format!("{:?}", vanilla.quiet),
        });
    }

    if user.prompt != vanilla.prompt {
        diffs.push(PropertyDiff {
            property_name: "prompt".to_string(),
            user_value: user.prompt.clone().unwrap_or_else(|| "<none>".to_string()),
            vanilla_value: vanilla
                .prompt
                .clone()
                .unwrap_or_else(|| "<none>".to_string()),
        });
    }

    diffs
}

/// Recursively computes differences between user items and vanilla items.
fn diff_items_recursive(
    user_items: &[MenuItem],
    vanilla_items: &[MenuItem],
    current_path: &[String],
    plan: &mut MenuMergePlan,
) {
    let mut prev_vanilla_label: Option<String> = None;

    for v_item in vanilla_items {
        if v_item.is_divider() {
            continue;
        }

        let mut item_path = current_path.to_vec();
        item_path.push(v_item.label.clone());

        if let Some(u_item) = user_items.iter().find(|u| items_match(u, v_item)) {
            // Matched existing item
            if v_item.r#type == MenuItemType::Submenu && u_item.r#type == MenuItemType::Submenu {
                if let (Some(u_sub), Some(v_sub)) = (&u_item.submenu, &v_item.submenu) {
                    diff_items_recursive(&u_sub.items, &v_sub.items, &item_path, plan);
                }
            } else {
                let diffs = compute_property_diffs(u_item, v_item);
                if !diffs.is_empty() {
                    plan.conflicts.push(ItemConflict {
                        path: item_path,
                        user_item: u_item.clone(),
                        vanilla_item: v_item.clone(),
                        property_diffs: diffs,
                    });
                }
            }
        } else {
            // New upstream item not present in user menu
            plan.new_items.push(NewUpstreamItem {
                path: item_path,
                item: v_item.clone(),
                predecessor_anchor: prev_vanilla_label.clone(),
            });
        }

        prev_vanilla_label = Some(v_item.label.clone());
    }
}

/// Analyzes differences between a user's menu and a new upstream vanilla menu.
pub fn compute_menu_diff(user: &MenuSchema, vanilla: &MenuSchema) -> MenuMergePlan {
    let mut plan = MenuMergePlan::default();
    diff_items_recursive(&user.items, &vanilla.items, &[], &mut plan);
    plan
}

/// Recursively synthesizes merged menu items applying all user decisions.
fn apply_merge_items_recursive(
    user_items: &[MenuItem],
    vanilla_items: &[MenuItem],
    current_path: &[String],
    decisions: &MergeDecisions,
) -> Vec<MenuItem> {
    let mut result: Vec<MenuItem> = Vec::new();

    // Identify which vanilla items are new additions to insert
    let mut pending_additions: Vec<NewUpstreamItem> = Vec::new();
    let mut prev_v_label: Option<String> = None;

    for v_item in vanilla_items {
        if v_item.is_divider() {
            continue;
        }
        let mut p = current_path.to_vec();
        p.push(v_item.label.clone());

        if !user_items.iter().any(|u| items_match(u, v_item)) && decisions.is_addition_accepted(&p)
        {
            pending_additions.push(NewUpstreamItem {
                path: p,
                item: v_item.clone(),
                predecessor_anchor: prev_v_label.clone(),
            });
        }
        prev_v_label = Some(v_item.label.clone());
    }

    // Insert any additions anchored to the top (predecessor_anchor is None)
    for add in pending_additions
        .iter()
        .filter(|a| a.predecessor_anchor.is_none())
    {
        result.push(add.item.clone());
    }
    pending_additions.retain(|a| a.predecessor_anchor.is_some());

    // Iterate user items, preserving their original order and custom items
    for u_item in user_items {
        if u_item.is_divider() {
            // Push divider if previous item is not already a divider
            if !result.last().map(|it| it.is_divider()).unwrap_or(false) {
                result.push(u_item.clone());
            }
            continue;
        }

        let mut item_path = current_path.to_vec();
        item_path.push(u_item.label.clone());

        let matched_vanilla = vanilla_items.iter().find(|v| items_match(u_item, v));

        if let Some(v_item) = matched_vanilla {
            if u_item.r#type == MenuItemType::Submenu && v_item.r#type == MenuItemType::Submenu {
                let u_sub = u_item.submenu.clone().unwrap_or_default();
                let v_sub = v_item.submenu.clone().unwrap_or_default();
                let merged_sub_items =
                    apply_merge_items_recursive(&u_sub.items, &v_sub.items, &item_path, decisions);

                let mut merged_sub_item = u_item.clone();
                merged_sub_item.submenu = Some(Submenu {
                    title: u_sub.title,
                    items: merged_sub_items,
                });
                result.push(merged_sub_item);
            } else {
                let diffs = compute_property_diffs(u_item, v_item);
                if diffs.is_empty() {
                    result.push(u_item.clone());
                } else {
                    match decisions.conflict_resolution(&item_path) {
                        ConflictChoice::KeepUser => {
                            result.push(u_item.clone());
                        }
                        ConflictChoice::TakeVanilla => {
                            result.push(v_item.clone());
                        }
                        ConflictChoice::KeepBoth => {
                            result.push(u_item.clone());
                            let mut dup_vanilla = v_item.clone();
                            if dup_vanilla.label == u_item.label {
                                dup_vanilla.label = format!("{} (Vanilla)", v_item.label);
                            }
                            result.push(dup_vanilla);
                        }
                    }
                }
            }

            // Check if any pending additions were anchored right after this vanilla item
            let v_anchor = Some(v_item.label.clone());
            let mut matching_adds = Vec::new();
            pending_additions.retain(|add| {
                if add.predecessor_anchor == v_anchor {
                    matching_adds.push(add.item.clone());
                    false
                } else {
                    true
                }
            });
            for add_item in matching_adds {
                result.push(add_item);
            }
        } else {
            // User custom item (never existed in vanilla) -> keep untouched
            result.push(u_item.clone());
        }
    }

    // Append any remaining additions that couldn't match an anchor
    for add in pending_additions {
        result.push(add.item);
    }

    // Clean up trailing and double dividers
    clean_dividers(&mut result);

    result
}

/// Collapses consecutive duplicate dividers.
fn clean_dividers(items: &mut Vec<MenuItem>) {
    let mut cleaned: Vec<MenuItem> = Vec::new();
    for it in items.drain(..) {
        if it.is_divider()
            && cleaned
                .last()
                .map(|prev| prev.is_divider())
                .unwrap_or(false)
        {
            continue; // Collapse consecutive duplicate dividers
        }
        cleaned.push(it);
    }
    *items = cleaned;
}

/// Merges a user's menu with a new upstream vanilla menu applying user choices.
pub fn apply_merge(
    user: &MenuSchema,
    vanilla: &MenuSchema,
    decisions: &MergeDecisions,
) -> MenuSchema {
    let merged_items = apply_merge_items_recursive(&user.items, &vanilla.items, &[], decisions);

    MenuSchema {
        version: vanilla.version.clone(),
        title: user.title.clone(),
        items: merged_items,
        extra: user.extra.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(label: &str, action: &str) -> MenuItem {
        MenuItem {
            label: label.to_string(),
            r#type: MenuItemType::Command,
            action: Some(action.to_string()),
            ..Default::default()
        }
    }

    fn make_submenu(title: &str, items: Vec<MenuItem>) -> MenuItem {
        MenuItem {
            label: title.to_string(),
            r#type: MenuItemType::Submenu,
            submenu: Some(Submenu {
                title: title.to_string(),
                items,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_detect_new_upstream_items_and_anchors() {
        let user = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![
                MenuItem::divider(),
                make_item("Status Gutter", "status.sh"),
                MenuItem::divider(),
            ],
            extra: HashMap::new(),
        };

        let vanilla = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![
                MenuItem::divider(),
                make_item("Status Gutter", "status.sh"),
                make_item("Marquee Speed", "speed.sh"),
                MenuItem::divider(),
            ],
            extra: HashMap::new(),
        };

        let diff = compute_menu_diff(&user, &vanilla);
        assert_eq!(diff.new_items.len(), 1);
        assert_eq!(diff.new_items[0].item.label, "Marquee Speed");
        assert_eq!(
            diff.new_items[0].predecessor_anchor.as_deref(),
            Some("Status Gutter")
        );
        assert!(diff.conflicts.is_empty());

        // Apply merge accepting the addition
        let mut decisions = MergeDecisions::default();
        decisions
            .additions
            .insert("Marquee Speed".to_string(), true);

        let merged = apply_merge(&user, &vanilla, &decisions);
        assert_eq!(merged.items.len(), 4); // divider, Status Gutter, Marquee Speed, divider
        assert_eq!(merged.items[1].label, "Status Gutter");
        assert_eq!(merged.items[2].label, "Marquee Speed");
    }

    #[test]
    fn test_conflict_resolution_choices() {
        let mut user_item = make_item("Notes", "notes.sh");
        user_item.icon = Some("{nf::#user}".to_string());

        let mut vanilla_item = make_item("Notes", "notes.sh");
        vanilla_item.icon = Some("{nf::#vanilla}".to_string());

        let user = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![user_item.clone()],
            extra: HashMap::new(),
        };

        let vanilla = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![vanilla_item.clone()],
            extra: HashMap::new(),
        };

        let diff = compute_menu_diff(&user, &vanilla);
        assert_eq!(diff.conflicts.len(), 1);
        assert_eq!(diff.conflicts[0].property_diffs.len(), 1);
        assert_eq!(diff.conflicts[0].property_diffs[0].property_name, "icon");

        // 1. Keep User
        let mut d_user = MergeDecisions::default();
        d_user
            .conflicts
            .insert("Notes".to_string(), ConflictChoice::KeepUser);
        let m_user = apply_merge(&user, &vanilla, &d_user);
        assert_eq!(m_user.items[0].icon.as_deref(), Some("{nf::#user}"));

        // 2. Take Vanilla
        let mut d_vanilla = MergeDecisions::default();
        d_vanilla
            .conflicts
            .insert("Notes".to_string(), ConflictChoice::TakeVanilla);
        let m_vanilla = apply_merge(&user, &vanilla, &d_vanilla);
        assert_eq!(m_vanilla.items[0].icon.as_deref(), Some("{nf::#vanilla}"));

        // 3. Keep Both
        let mut d_both = MergeDecisions::default();
        d_both
            .conflicts
            .insert("Notes".to_string(), ConflictChoice::KeepBoth);
        let m_both = apply_merge(&user, &vanilla, &d_both);
        assert_eq!(m_both.items.len(), 2);
        assert_eq!(m_both.items[0].label, "Notes");
        assert_eq!(m_both.items[1].label, "Notes (Vanilla)");
    }

    #[test]
    fn test_recursive_submenu_merge_and_preservation() {
        let user = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![make_submenu(
                "Options & Settings",
                vec![
                    make_item("Inactivity Timeout", "timeout.sh"),
                    make_item("My Custom Setting", "custom.sh"),
                ],
            )],
            extra: HashMap::new(),
        };

        let vanilla = MenuSchema {
            version: "0.0.1".to_string(),
            title: "Menu".to_string(),
            items: vec![make_submenu(
                "Options & Settings",
                vec![
                    make_item("Inactivity Timeout", "timeout.sh"),
                    make_item("Language / Locale", "locale.sh"),
                ],
            )],
            extra: HashMap::new(),
        };

        let diff = compute_menu_diff(&user, &vanilla);
        assert_eq!(diff.new_items.len(), 1);
        assert_eq!(diff.new_items[0].item.label, "Language / Locale");
        assert_eq!(
            diff.new_items[0].path,
            vec!["Options & Settings", "Language / Locale"]
        );

        let decisions = MergeDecisions::default(); // default accepts addition
        let merged = apply_merge(&user, &vanilla, &decisions);
        let sub = merged.items[0].submenu.as_ref().unwrap();

        assert_eq!(sub.items.len(), 3);
        assert_eq!(sub.items[0].label, "Inactivity Timeout");
        assert_eq!(sub.items[1].label, "Language / Locale"); // Inserted after anchor
        assert_eq!(sub.items[2].label, "My Custom Setting"); // User custom preserved!
    }
}
