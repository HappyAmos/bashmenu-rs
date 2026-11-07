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
    /// Inner notebook metadata and notes collection
    pub notebook: NotebookData,
}

/// Metadata and note collection within a notebook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookData {
    /// Title header of the notebook
    pub title: String,
    /// Default text format (typically "markdown")
    #[serde(default = "default_format")]
    pub format: String,
    /// Sequence of raw YAML note mappings
    #[serde(default)]
    pub notes: Vec<serde_yml::Value>,
}

/// Default note format string (`"markdown"`).
fn default_format() -> String {
    "markdown".to_string()
}

/// A parsed note entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NoteItem {
    /// Display title / note identifier
    pub title: String,
    /// Note type descriptor (default "note")
    #[serde(default = "default_note_type")]
    pub r#type: String,
    /// Document format (e.g. "markdown")
    #[serde(default = "default_format")]
    pub format: String,
    /// Multi-line body content of the note
    #[serde(default)]
    pub content: String,
    /// Optional leading visual icon or glyph
    #[serde(default)]
    pub icon: Option<String>,
}

/// Default note type string (`"note"`).
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

    if p.is_absolute() && p.exists() {
        return p;
    }

    // 1st Priority: ~/.config/bashmenu/notebooks/ or ~/.config/bashmenu/
    if let Some(file_name) = p.file_name() {
        let user_cfg_nb = ctx
            .home_dir
            .join(".config/bashmenu/notebooks")
            .join(file_name);
        if user_cfg_nb.exists() {
            return user_cfg_nb;
        }
        let user_cfg_root = ctx.home_dir.join(".config/bashmenu").join(file_name);
        if user_cfg_root.exists() {
            return user_cfg_root;
        }
    }

    // 2nd Priority: {bashmenu_dir}/notebooks/ or {bashmenu_dir}/
    let cand1 = notebooks_dir.join(&p);
    if cand1.exists() {
        return cand1;
    }
    let cand2 = ctx.bashmenu_dir.join(&p);
    if cand2.exists() {
        return cand2;
    }

    if p.is_absolute() {
        return p;
    }

    cand1
}

/// Converts whitespace-separated string into Title Case.
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

/// Loads a notebook YAML file or initializes an empty notebook if missing.
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

/// Swap file metadata for concurrent editing protection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteSwapFile {
    /// Process ID holding the edit session
    pub pid: u32,
    /// Unix timestamp when session began
    pub timestamp: u64,
    /// Note title being edited
    pub note_title: String,
    /// Original base content of the note when editing began
    pub base_content: String,
}

impl NoteSwapFile {
    /// Computes canonical path to swap file for a notebook file and note
    /// title.
    pub fn swap_path(notebook_path: &Path, note_title: &str) -> PathBuf {
        let parent = notebook_path.parent().unwrap_or_else(|| Path::new("."));
        let file_stem = notebook_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("notes.yml");
        let slug: String = note_title
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        parent.join(format!(".{file_stem}.note.{slug}.swp"))
    }

    /// Creates and writes the swap file to disk.
    pub fn create(notebook_path: &Path, note_title: &str, base_content: &str) -> Option<PathBuf> {
        let path = Self::swap_path(notebook_path, note_title);
        let swap = Self {
            pid: std::process::id(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            note_title: note_title.to_string(),
            base_content: base_content.to_string(),
        };
        let yaml = serde_yml::to_string(&swap).ok()?;
        fs::write(&path, yaml).ok()?;
        Some(path)
    }

    /// Reads existing swap file if present.
    pub fn load(notebook_path: &Path, note_title: &str) -> Option<Self> {
        let path = Self::swap_path(notebook_path, note_title);
        if path.exists() {
            let data = fs::read_to_string(&path).ok()?;
            serde_yml::from_str(&data).ok()
        } else {
            None
        }
    }

    /// Removes swap file from disk.
    pub fn remove(notebook_path: &Path, note_title: &str) {
        let path = Self::swap_path(notebook_path, note_title);
        let _ = fs::remove_file(path);
    }
}

/// Result of a 3-way merge attempt on note content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMergeResult {
    /// Merged content string
    pub content: String,
    /// Whether any line conflicts were detected and marked with conflict
    /// markers
    pub has_conflicts: bool,
}

/// Performs a 3-way line merge between base, disk (external), and local
/// changes.
pub fn merge_note_content(base: &str, disk: &str, local: &str) -> NoteMergeResult {
    // If local equals disk, no conflict at all
    if local == disk {
        return NoteMergeResult {
            content: local.to_string(),
            has_conflicts: false,
        };
    }
    // If disk hasn't changed from base, local wins
    if disk == base {
        return NoteMergeResult {
            content: local.to_string(),
            has_conflicts: false,
        };
    }
    // If local hasn't changed from base, disk wins
    if local == base {
        return NoteMergeResult {
            content: disk.to_string(),
            has_conflicts: false,
        };
    }

    let base_lines: Vec<&str> = base.lines().collect();
    let disk_lines: Vec<&str> = disk.lines().collect();
    let local_lines: Vec<&str> = local.lines().collect();

    let mut merged = Vec::new();
    let mut has_conflicts = false;

    let mut b_idx = 0;
    let mut d_idx = 0;
    let mut l_idx = 0;

    while d_idx < disk_lines.len() || l_idx < local_lines.len() {
        let b_line = base_lines.get(b_idx).copied();
        let d_line = disk_lines.get(d_idx).copied();
        let l_line = local_lines.get(l_idx).copied();

        if d_line == l_line {
            if let Some(line) = d_line {
                merged.push(line.to_string());
            }
            b_idx += 1;
            d_idx += 1;
            l_idx += 1;
        } else if d_line == b_line {
            if let Some(line) = l_line {
                merged.push(line.to_string());
            }
            b_idx += 1;
            d_idx += 1;
            l_idx += 1;
        } else if l_line == b_line {
            if let Some(line) = d_line {
                merged.push(line.to_string());
            }
            b_idx += 1;
            d_idx += 1;
            l_idx += 1;
        } else if b_line.is_none() {
            if let Some(ll) = l_line {
                merged.push(ll.to_string());
                l_idx += 1;
            }
            if let Some(dl) = d_line {
                if d_line != l_line {
                    merged.push(dl.to_string());
                }
                d_idx += 1;
            }
        } else {
            has_conflicts = true;
            merged.push("<<<<<<< Local Edits".to_string());
            if let Some(ll) = l_line {
                merged.push(ll.to_string());
            }
            merged.push("=======".to_string());
            if let Some(dl) = d_line {
                merged.push(dl.to_string());
            }
            merged.push(">>>>>>> Disk Version".to_string());
            b_idx += 1;
            d_idx += 1;
            l_idx += 1;
        }
    }

    NoteMergeResult {
        content: merged.join("\n"),
        has_conflicts,
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

    #[test]
    fn test_note_swap_file_and_3way_merge() {
        let tmp_dir = std::env::temp_dir().join(format!("swp_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let nb_path = tmp_dir.join("test_nb.yml");

        let base_content = "Line 1\nLine 2\nLine 3";
        let swp_path = NoteSwapFile::create(&nb_path, "My Note", base_content);
        assert!(swp_path.is_some());
        let swp = NoteSwapFile::load(&nb_path, "My Note");
        assert!(swp.is_some());
        let loaded = swp.unwrap();
        assert_eq!(loaded.note_title, "My Note");
        assert_eq!(loaded.base_content, base_content);

        // Non-conflicting append merge: disk added Line 4, local added Line 0
        let disk_content = "Line 1\nLine 2\nLine 3\nDisk Appended";
        let local_content = "Line 1\nLine 2\nLine 3\nLocal Appended";
        let merge_res = merge_note_content(base_content, disk_content, local_content);
        assert!(!merge_res.has_conflicts);
        assert!(merge_res.content.contains("Local Appended"));
        assert!(merge_res.content.contains("Disk Appended"));

        // Conflicting change on Line 2
        let disk_conflict = "Line 1\nLine 2 Changed By Disk\nLine 3";
        let local_conflict = "Line 1\nLine 2 Changed By Local\nLine 3";
        let conflict_res = merge_note_content(base_content, disk_conflict, local_conflict);
        assert!(conflict_res.has_conflicts);
        assert!(conflict_res.content.contains("<<<<<<< Local Edits"));
        assert!(conflict_res.content.contains("======="));
        assert!(conflict_res.content.contains(">>>>>>> Disk Version"));

        NoteSwapFile::remove(&nb_path, "My Note");
        assert!(NoteSwapFile::load(&nb_path, "My Note").is_none());
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
