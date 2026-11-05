//! Configuration loading and cheatsheet root directory resolution.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Minimal configuration schema corresponding to `.cheat.yml`.
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

/// Locates or creates the `.cheat.yml` configuration file.
///
/// Checks candidate locations in order:
/// 1. `CHEAT_CONFIG` environment variable
/// 2. Next to the running executable (`exe_dir/.cheat.yml`)
/// 3. Standard user script config (`~/.config/bashmenu/scripts/.cheat.yml`)
/// 4. Standard user config (`~/.config/bashmenu/.cheat.yml`)
/// 5. Repository or working directory relative paths (`scripts/.cheat.yml`, `.cheat.yml`)
/// 6. User home directory (`~/.cheat.yml`)
///
/// If no configuration file exists, creates a default one pointing to `~/cheat`.
pub fn get_config_path() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("CHEAT_CONFIG") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join(".cheat.yml");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(&home).join(".config/bashmenu/scripts/.cheat.yml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        let candidate = PathBuf::from(&home).join(".config/bashmenu/.cheat.yml");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    let rel_candidates = [
        PathBuf::from("scripts/.cheat.yml"),
        PathBuf::from(".cheat.yml"),
    ];
    for cand in &rel_candidates {
        if cand.is_file() {
            return Ok(cand.clone());
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(&home).join(".cheat.yml");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    // Determine target location to create default configuration file
    let target = if let Ok(home) = std::env::var("HOME") {
        let scripts_dir = PathBuf::from(&home).join(".config/bashmenu/scripts");
        if scripts_dir.is_dir() {
            scripts_dir.join(".cheat.yml")
        } else {
            let cfg_dir = PathBuf::from(&home).join(".config/bashmenu");
            if cfg_dir.is_dir() {
                cfg_dir.join(".cheat.yml")
            } else {
                PathBuf::from(&home).join(".cheat.yml")
            }
        }
    } else {
        PathBuf::from(".cheat.yml")
    };

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
                "Error: No .cheat.yml found, and failed to create one at {}: {}",
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

        let config_file = dir.path().join(".cheat.yml");
        fs::write(&config_file, format!("dir: {}\n", cheats_dir.display())).expect("write config");

        let root = get_root_directory(&config_file).expect("get root");
        let expected = fs::canonicalize(&cheats_dir).expect("canonicalize");
        assert_eq!(root, expected);
    }

    #[test]
    fn test_get_root_directory_missing_dir_key() {
        let dir = tempdir().expect("tempdir");
        let config_file = dir.path().join(".cheat.yml");
        fs::write(&config_file, "other_key: 123\n").expect("write config");

        let res = get_root_directory(&config_file);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Missing 'dir' key"));
    }

    #[test]
    fn test_get_root_directory_invalid_dir() {
        let dir = tempdir().expect("tempdir");
        let config_file = dir.path().join(".cheat.yml");
        fs::write(&config_file, "dir: /nonexistent/path/for/cheat\n").expect("write config");

        let res = get_root_directory(&config_file);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Invalid directory"));
    }
}
