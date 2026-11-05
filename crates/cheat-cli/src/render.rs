//! Cheatsheet rendering via `rich.sh`, `bmpager`, or terminal fallback.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Checks whether a given path points to an executable file.
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        if let Ok(meta) = path.metadata() {
            return meta.permissions().mode() & 0o111 != 0;
        }
        false
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Locates `rich.sh` in the binary directory, user config, repo, or PATH.
pub fn find_rich_script() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("rich.sh");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(&home).join(".config/bashmenu/scripts/rich.sh");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let rel_candidates = [
        PathBuf::from("scripts/rich.sh"),
        PathBuf::from("../scripts/rich.sh"),
    ];
    for cand in &rel_candidates {
        if cand.is_file() {
            return Some(cand.clone());
        }
    }

    find_in_path("rich.sh")
}

/// Locates `bmpager` binary in executable directory, user bin, or PATH.
pub fn find_bmpager() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("bmpager");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(&home).join(".local/bin/bmpager");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    find_in_path("bmpager")
}

/// Searches directories in `$PATH` for a given binary name.
fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let full_path = dir.join(binary_name);
            if full_path.is_file() {
                return Some(full_path);
            }
        }
    }
    None
}

/// Displays a Markdown file using `rich.sh`, `bmpager`, or direct stdout fallback.
pub fn display_markdown(file_path: &Path) {
    // 1. Try to render via rich.sh (1:1 parity with cheat.py)
    if let Some(rich) = find_rich_script() {
        let mut cmd = if is_executable(&rich) {
            let mut c = Command::new(&rich);
            c.arg(file_path);
            c
        } else {
            let mut c = Command::new("bash");
            c.arg(&rich);
            c.arg(file_path);
            c
        };

        if let Ok(status) = cmd.status() {
            if !status.success() {
                let code = status.code().unwrap_or(130);
                exit(code);
            }
            return;
        }
    }

    // 2. Try to render via native bmpager
    if let Some(bmpager) = find_bmpager() {
        let mut cmd = Command::new(bmpager);
        cmd.arg("--markdown");
        cmd.arg(file_path);

        if let Ok(status) = cmd.status() {
            if !status.success() {
                let code = status.code().unwrap_or(130);
                exit(code);
            }
            return;
        }
    }

    // 3. Fallback: print raw file contents to stdout
    match fs::read_to_string(file_path) {
        Ok(content) => {
            print!("{content}");
        }
        Err(err) => {
            eprintln!("Error reading {}: {}", file_path.display(), err);
            exit(1);
        }
    }
}
