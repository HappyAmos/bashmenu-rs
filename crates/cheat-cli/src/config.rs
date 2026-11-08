//! Configuration loading and cheatsheet root directory resolution.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Minimal configuration schema corresponding to `cheat.yml`.
#[derive(Debug, Clone, Deserialize)]
pub struct CheatConfig {
    /// Path to root cheatsheets directory.
    pub dir: Option<String>,
}

/// Expands leading tilde (`~`) in paths with the user's home directory.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let p = path.as_ref();
    if let Ok(stripped) = p.strip_prefix("~") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    p.to_path_buf()
}

/// Locates or creates the `cheat.yml` configuration file in `~/.config/bashmenu`.
///
/// Checks candidate locations in order:
/// 1. `CHEAT_CONFIG` environment variable
/// 2. User config directory (`~/.config/bashmenu/cheat.yml`)
/// 3. Repository root fallback (`cheat.yml`)
///
/// If no configuration file exists, creates a default one pointing to `~/cheat`.
pub fn get_config_path() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("CHEAT_CONFIG") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
    }

    let user_cfg = bashmenu_core::get_user_config_dir().join("cheat.yml");
    if user_cfg.is_file() {
        return Ok(user_cfg);
    }

    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));
    let dev_dir = bashmenu_core::resolve_dev_dir(&home, &user_cfg);
    let dev_cheat = dev_dir.join("cheat.yml");
    if dev_cheat.is_file() {
        return Ok(dev_cheat);
    }

    let root_paths = [PathBuf::from("cheat.yml"), PathBuf::from("../../cheat.yml")];
    for p in &root_paths {
        if p.is_file() {
            return Ok(p.clone());
        }
    }

    // Target location to create default configuration file: ~/.config/bashmenu/cheat.yml
    let target = user_cfg;
    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match fs::write(&target, "dir: ~/cheat\n") {
        Ok(_) => {
            println!(
                "Created default configuration file at: {}",
                target.display()
            );
            Ok(target)
        }
        Err(err) => {
            bail!(
                "Error: No cheat.yml found, and failed to create one at {}: {}",
                target.display(),
                err
            );
        }
    }
}

/// Extracts and validates the target cheatsheets root directory from configuration.
pub fn get_root_directory(config_path: &Path) -> Result<PathBuf> {
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Error reading config file {}", config_path.display()))?;

    let config: CheatConfig = serde_yml::from_str(&content)
        .with_context(|| format!("Error reading config file {}", config_path.display()))?;

    let dir_str = match config.dir {
        Some(d) if !d.trim().is_empty() => d,
        _ => bail!("Error: Missing 'dir' key in {}", config_path.display()),
    };

    let expanded = expand_tilde(dir_str.trim());
    let root = match fs::canonicalize(&expanded) {
        Ok(canon) => canon,
        Err(_) => expanded,
    };

    if !root.is_dir() {
        bail!(
            "Error: Invalid directory in {}: {}",
            config_path.display(),
            root.display()
        );
    }

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_expand_tilde() {
        let path = PathBuf::from("~/test/file.md");
        let expanded = expand_tilde(&path);
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(expanded, PathBuf::from(home).join("test/file.md"));
        }
    }

    #[test]
    fn test_get_root_directory_valid() {
        let dir = tempdir().expect("tempdir");
        let cheats_dir = dir.path().join("cheats");
        fs::create_dir(&cheats_dir).expect("create dir");

        let config_file = dir.path().join("cheat.yml");
        fs::write(&config_file, format!("dir: {}\n", cheats_dir.display())).expect("write config");

        let root = get_root_directory(&config_file).expect("get root");
        let expected = fs::canonicalize(&cheats_dir).expect("canonicalize");
        assert_eq!(root, expected);
    }

    #[test]
    fn test_get_root_directory_missing_dir_key() {
        let dir = tempdir().expect("tempdir");
        let config_file = dir.path().join("cheat.yml");
        fs::write(&config_file, "other_key: 123\n").expect("write config");

        let res = get_root_directory(&config_file);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Missing 'dir' key"));
    }

    #[test]
    fn test_get_root_directory_invalid_dir() {
        let dir = tempdir().expect("tempdir");
        let config_file = dir.path().join("cheat.yml");
        fs::write(&config_file, "dir: /nonexistent/path/for/cheat\n").expect("write config");

        let res = get_root_directory(&config_file);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Invalid directory"));
    }
}
