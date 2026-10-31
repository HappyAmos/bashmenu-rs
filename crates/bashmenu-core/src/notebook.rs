//! Notebook schema, note models, and YAML serialization for BashMenu notebooks.

use crate::error::{BashMenuError, Result};
use crate::macro_engine::{interpolate_placeholders, MacroContext};
use crate::menu::{MenuItem, MenuItemType, Submenu};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Root notebook document structure matching `notebooks/notes.yml`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookSchema {
    pub notebook: NotebookData,
}

/// Metadata and note collection within a notebook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookData {
    pub title: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub notes: Vec<serde_yml::Value>,
}

fn default_format() -> String {
    "markdown".to_string()
}

/// A parsed note entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NoteItem {
    pub title: String,
    #[serde(default = "default_note_type")]
    pub r#type: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub icon: Option<String>,
}

fn default_note_type() -> String {
    "note".to_string()
}

/// Resolves a notebook file path, checking `notebooks/` directory first.
pub fn resolve_notebook_path(raw_path: Option<&str>, ctx: &MacroContext) -> PathBuf {
    let notebooks_dir = ctx.bashmenu_dir.join("notebooks");
    let raw = match raw_path {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            let default_nb = notebooks_dir.join("notes.yml");
            if default_nb.exists() {
                return default_nb;
            }
            let root_nb = ctx.bashmenu_dir.join("notes.yml");
            if root_nb.exists() {
                return root_nb;
            }
            return default_nb;
        }
    };

    let interpolated = interpolate_placeholders(raw, ctx);
    let expanded = if let Some(stripped) = interpolated.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/{}", home, stripped)
        } else {
            interpolated
        }
    } else {
        interpolated
    };
    let p = PathBuf::from(&expanded);

    if p.is_absolute() {
        return p;
    }

    let cand1 = notebooks_dir.join(&p);
    if cand1.exists() {
        return cand1;
    }
    let cand2 = ctx.bashmenu_dir.join(&p);
    if cand2.exists() {
        return cand2;
    }

    cand1
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Loads a notebook YAML file. If it doesn't exist, initializes an empty notebook.
pub fn load_notebook(path: impl AsRef<Path>) -> Result<NotebookSchema> {
    let p = path.as_ref();
    if !p.exists() {
        let base_name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Notebook");
        let formatted_title = title_case(&base_name.replace('_', " "));
        let nb = NotebookSchema {
            notebook: NotebookData {
                title: formatted_title,
                format: "markdown".to_string(),
                notes: Vec::new(),
            },
        };
        save_notebook(p, &nb)?;
        return Ok(nb);
    }

    let raw = fs::read_to_string(p).map_err(|source| BashMenuError::Io {
        path: p.to_path_buf(),
        source,
    })?;
    let schema: NotebookSchema = serde_yml::from_str(&raw)?;
    Ok(schema)
}

/// Saves a notebook data structure to disk as clean YAML.
pub fn save_notebook(path: impl AsRef<Path>, nb: &NotebookSchema) -> Result<()> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let yaml_str = serde_yml::to_string(nb)?;
    fs::write(p, yaml_str).map_err(|source| BashMenuError::Io {
        path: p.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Helper to retrieve a string field from a serde_yml::Mapping
fn get_str_field<'a>(map: &'a serde_yml::Mapping, key: &str) -> Option<&'a str> {
    map.iter().find_map(|(k, v)| {
        if k.as_str() == Some(key) {
            v.as_str()
        } else {
            None
        }
    })
}

/// Extracts a flat list of `NoteItem` structs from the notebook schema.
pub fn get_notes(nb: &NotebookSchema) -> Vec<NoteItem> {
    let mut results = Vec::new();

    for item in &nb.notebook.notes {
        if let serde_yml::Value::Mapping(map) = item {
            // Check for single-key mapping (Title: properties)
            if map.len() == 1 {
                if let Some((k, v)) = map.iter().next() {
                    let title = k.as_str().unwrap_or("Untitled").to_string();
                    let (note_type, format, content, icon) = match v {
                        serde_yml::Value::Mapping(props) => {
                            let note_type =
                                get_str_field(props, "type").unwrap_or("note").to_string();
                            let format = get_str_field(props, "format")
                                .unwrap_or("markdown")
                                .to_string();
                            let content = get_str_field(props, "content").unwrap_or("").to_string();
                            let icon = get_str_field(props, "icon").map(|s| s.to_string());
                            (note_type, format, content, icon)
                        }
                        serde_yml::Value::String(s) => {
                            ("note".to_string(), "markdown".to_string(), s.clone(), None)
                        }
                        _ => (
                            "note".to_string(),
                            "markdown".to_string(),
                            String::new(),
                            None,
                        ),
                    };

                    results.push(NoteItem {
                        title,
                        r#type: note_type,
                        format,
                        content,
                        icon,
                    });
                    continue;
                }
            }

            // Explicit title property
            let title = get_str_field(map, "title")
                .unwrap_or("Untitled")
                .to_string();
            let note_type = get_str_field(map, "type").unwrap_or("note").to_string();
            let format = get_str_field(map, "format")
                .unwrap_or("markdown")
                .to_string();
            let content = get_str_field(map, "content").unwrap_or("").to_string();
            let icon = get_str_field(map, "icon").map(|s| s.to_string());

            results.push(NoteItem {
                title,
                r#type: note_type,
                format,
                content,
                icon,
            });
        }
    }

    results
}

/// Finds a note by title.
pub fn get_note_by_title(nb: &NotebookSchema, title: &str) -> Option<NoteItem> {
    get_notes(nb).into_iter().find(|n| n.title == title)
}

/// Adds a new note or updates an existing note by title.
pub fn add_or_update_note(nb: &mut NotebookSchema, note: NoteItem) {
    let mut updated = false;

    for item in &mut nb.notebook.notes {
        if let serde_yml::Value::Mapping(map) = item {
            // Check for label-as-key
            if map.len() == 1 {
                if let Some(first_key) = map.keys().next().cloned() {
                    if first_key.as_str() == Some(&note.title) {
                        let mut new_props = serde_yml::Mapping::new();
                        new_props.insert(
                            serde_yml::Value::String("type".to_string()),
                            serde_yml::Value::String(note.r#type.clone()),
                        );
                        new_props.insert(
                            serde_yml::Value::String("format".to_string()),
                            serde_yml::Value::String(note.format.clone()),
                        );
                        new_props.insert(
                            serde_yml::Value::String("content".to_string()),
                            serde_yml::Value::String(note.content.clone()),
                        );
                        if let Some(ref ic) = note.icon {
                            new_props.insert(
                                serde_yml::Value::String("icon".to_string()),
                                serde_yml::Value::String(ic.clone()),
                            );
                        }
                        *item = serde_yml::Value::Mapping({
                            let mut m = serde_yml::Mapping::new();
                            m.insert(first_key, serde_yml::Value::Mapping(new_props));
                            m
                        });
                        updated = true;
                        break;
                    }
                }
            } else if get_str_field(map, "title") == Some(&note.title) {
                map.insert(
                    serde_yml::Value::String("content".to_string()),
                    serde_yml::Value::String(note.content.clone()),
                );
                map.insert(
                    serde_yml::Value::String("format".to_string()),
                    serde_yml::Value::String(note.format.clone()),
                );
                map.insert(
                    serde_yml::Value::String("type".to_string()),
                    serde_yml::Value::String(note.r#type.clone()),
                );
                if let Some(ref ic) = note.icon {
                    map.insert(
                        serde_yml::Value::String("icon".to_string()),
                        serde_yml::Value::String(ic.clone()),
                    );
                }
                updated = true;
                break;
            }
        }
    }

    if !updated {
        let mut props = serde_yml::Mapping::new();
        props.insert(
            serde_yml::Value::String("type".to_string()),
            serde_yml::Value::String(note.r#type),
        );
        props.insert(
            serde_yml::Value::String("format".to_string()),
            serde_yml::Value::String(note.format),
        );
        props.insert(
            serde_yml::Value::String("content".to_string()),
            serde_yml::Value::String(note.content),
        );
        if let Some(ic) = note.icon {
            props.insert(
                serde_yml::Value::String("icon".to_string()),
                serde_yml::Value::String(ic),
            );
        }

        let mut entry = serde_yml::Mapping::new();
        entry.insert(
            serde_yml::Value::String(note.title),
            serde_yml::Value::Mapping(props),
        );
        nb.notebook.notes.push(serde_yml::Value::Mapping(entry));
    }
}

/// Renames a note in the notebook data structure.
pub fn rename_note(nb: &mut NotebookSchema, old_title: &str, new_title: &str) -> bool {
    for item in &mut nb.notebook.notes {
        if let serde_yml::Value::Mapping(map) = item {
            if map.len() == 1 {
                if let Some(first_key) = map.keys().next().cloned() {
                    if first_key.as_str() == Some(old_title) {
                        let val = map.values().next().cloned().unwrap();
                        let mut new_map = serde_yml::Mapping::new();
                        new_map.insert(serde_yml::Value::String(new_title.to_string()), val);
                        *item = serde_yml::Value::Mapping(new_map);
                        return true;
                    }
                }
            } else if get_str_field(map, "title") == Some(old_title) {
                map.insert(
                    serde_yml::Value::String("title".to_string()),
                    serde_yml::Value::String(new_title.to_string()),
                );
                return true;
            }
        }
    }
    false
}

/// Deletes a note by title from the notebook.
pub fn delete_note(nb: &mut NotebookSchema, title: &str) -> bool {
    let before_len = nb.notebook.notes.len();
    nb.notebook.notes.retain(|item| {
        if let serde_yml::Value::Mapping(map) = item {
            if map.len() == 1 {
                if let Some(k) = map.keys().next() {
                    return k.as_str() != Some(title);
                }
            } else if get_str_field(map, "title") == Some(title) {
                return false;
            }
        }
        true
    });
    nb.notebook.notes.len() < before_len
}

/// Constructs a dynamic submenu from a loaded notebook schema.
pub fn build_notebook_submenu(filepath: &str, nb: &NotebookSchema) -> Submenu {
    let mut items = vec![MenuItem::divider()];

    let notes = get_notes(nb);
    if notes.is_empty() {
        items.push(MenuItem {
            label: "(No notes yet — press [N] to create)".to_string(),
            r#type: MenuItemType::NotebookEmpty,
            file: Some(filepath.to_string()),
            icon: Some("    ".to_string()),
            ..Default::default()
        });
    } else {
        for note in notes {
            let note_icon = note.icon.unwrap_or_else(|| "{nf:󰎞:#f039e}".to_string());
            items.push(MenuItem {
                label: note.title.clone(),
                icon: Some(note_icon),
                r#type: MenuItemType::NotebookNote,
                file: Some(filepath.to_string()),
                action: Some(note.title),
                ..Default::default()
            });
        }
    }

    items.push(MenuItem::divider());
    items.push(MenuItem {
        label: "Back".to_string(),
        icon: Some("{nf:󰁮:#f048}".to_string()),
        r#type: MenuItemType::Back,
        ..Default::default()
    });

    Submenu {
        title: nb.notebook.title.clone(),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notebook_crud_roundtrip() {
        let tmp_dir = std::env::temp_dir().join(format!("nb_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let nb_path = tmp_dir.join("test_notes.yml");

        let mut nb = load_notebook(&nb_path).unwrap();
        assert_eq!(nb.notebook.title, "Test Notes");
        assert!(get_notes(&nb).is_empty());

        // Add a note
        add_or_update_note(
            &mut nb,
            NoteItem {
                title: "Tasks".to_string(),
                r#type: "note".to_string(),
                format: "markdown".to_string(),
                content: "- [ ] Step 1\n- [x] Step 2".to_string(),
                icon: None,
            },
        );

        save_notebook(&nb_path, &nb).unwrap();

        // Reload and verify
        let reloaded = load_notebook(&nb_path).unwrap();
        let notes = get_notes(&reloaded);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Tasks");
        assert_eq!(notes[0].content, "- [ ] Step 1\n- [x] Step 2");

        // Rename note
        let mut nb_mut = reloaded;
        assert!(rename_note(&mut nb_mut, "Tasks", "Project Tasks"));
        let notes_renamed = get_notes(&nb_mut);
        assert_eq!(notes_renamed[0].title, "Project Tasks");

        // Submenu generation
        let sub = build_notebook_submenu(nb_path.to_str().unwrap(), &nb_mut);
        assert_eq!(sub.title, "Test Notes");
        assert_eq!(sub.items.len(), 4); // Divider, Note, Divider, Back
        assert_eq!(sub.items[1].label, "Project Tasks");
        assert_eq!(sub.items[1].r#type, MenuItemType::NotebookNote);

        // Delete note
        assert!(delete_note(&mut nb_mut, "Project Tasks"));
        let sub_empty = build_notebook_submenu(nb_path.to_str().unwrap(), &nb_mut);
        assert_eq!(sub_empty.items[1].r#type, MenuItemType::NotebookEmpty);

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
