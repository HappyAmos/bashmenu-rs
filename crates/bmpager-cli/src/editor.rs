//! External and built-in editor integration for bmpager.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Launches the configured or default text editor positioned at `line_no` on `file_path`.
pub fn open_editor(
    file_path: &Path,
    line_no: usize,
    editor_override: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let mode = resolve_editor_mode(editor_override);

    if mode == "builtin" || mode == "bm" || mode == "bashedit" {
        let exe = find_bashedit_exe();
        if let Some(exe_path) = exe {
            let status = Command::new(exe_path)
                .arg(format!("+{}", line_no))
                .arg(file_path)
                .status();
            if let Err(e) = status {
                return Ok(Some(format!("error launching bashedit: {}", e)));
            }
        } else {
            return Ok(Some("built-in editor not found: bashedit".to_string()));
        }
    } else {
        let editor = get_system_editor();
        if let Some(ed) = editor {
            let status = Command::new(&ed)
                .arg(format!("+{}", line_no))
                .arg(file_path)
                .status();
            if let Err(e) = status {
                return Ok(Some(format!("error launching {}: {}", ed, e)));
            }
        } else {
            return Ok(Some("no editor found (set $EDITOR or $VISUAL)".to_string()));
        }
    }

    Ok(None)
}

fn resolve_editor_mode(override_mode: Option<&str>) -> String {
    if let Some(m) = override_mode {
        return m.to_string();
    }
    if let Ok(v) = std::env::var("BMPAGER_EDITOR") {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    if let Ok(v) = std::env::var("PAGER_EDITOR") {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }

    // Try reading settings.pager_editor from bashmenu.yml
    let cfg_path = bashmenu_core::config::resolve_app_path("bashmenu.yml");
    if let Ok(content) = std::fs::read_to_string(cfg_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pager_editor:") {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    let val = parts[1].replace(['"', '\''], "").trim().to_string();
                    if !val.is_empty() {
                        return val;
                    }
                }
            }
        }
    }

    "default".to_string()
}

fn find_bashedit_exe() -> Option<PathBuf> {
    if let Ok(p) = which::which("bashedit") {
        return Some(p);
    }
    let local = dirs::home_dir()?.join(".local/bin/bashedit");
    if local.is_file() {
        return Some(local);
    }
    let target = PathBuf::from("target/release/bashedit");
    if target.is_file() {
        return Some(target);
    }
    None
}

fn get_system_editor() -> Option<String> {
    if let Ok(v) = std::env::var("VISUAL") {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    if let Ok(v) = std::env::var("EDITOR") {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    for fallback in &["nano", "vim", "vi"] {
        if which::which(fallback).is_ok() {
            return Some((*fallback).to_string());
        }
    }
    None
}

mod which {
    use std::path::PathBuf;

    pub fn which(name: &str) -> Result<PathBuf, ()> {
        let path_var = std::env::var("PATH").map_err(|_| ())?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        Err(())
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
