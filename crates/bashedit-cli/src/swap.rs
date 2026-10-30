//! Colocated swap file management and crash recovery for BashEdit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SWAP_MAGIC: &str = "BASHEDIT-SWAP-v1";
pub const SWAP_DELIMITER: &str = "\n---BASHEDIT-SWAP-PAYLOAD---\n";

/// Metadata header stored at the beginning of a `.swp` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapHeader {
    pub magic: String,
    pub pid: u32,
    pub user: String,
    pub host: String,
    pub file_path: String,
    pub updated_at: f64,
    pub base_mtime: f64,
    pub base_sha256: String,
    pub line_count: usize,
}

/// Compute the canonical colocated swap file path: `.<filename>.swp`
/// in the same directory as the target file.
pub fn get_swap_path(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    let swp_name = format!(".{}.swp", file_name);
    parent.join(swp_name)
}

/// Computes SHA256 hex string of a file, or returns empty string if missing/unreadable.
pub fn compute_file_sha256(path: &Path) -> String {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    hasher.update(&data);
    format!("{:x}", hasher.finalize())
}

/// Checks whether a process with the given PID is actively running on the system.
pub fn is_pid_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Writes buffer contents to the swap file with metadata header.
pub fn write_swap_file(
    swp_path: &Path,
    file_path: &Path,
    lines: &[String],
    base_mtime: f64,
    base_sha256: &str,
) -> bool {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "localhost".to_string());

    let canonical_path = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    let header = SwapHeader {
        magic: SWAP_MAGIC.to_string(),
        pid: std::process::id(),
        user,
        host,
        file_path: canonical_path,
        updated_at: now,
        base_mtime,
        base_sha256: base_sha256.to_string(),
        line_count: lines.len(),
    };

    let header_json = match serde_json::to_string(&header) {
        Ok(j) => j,
        Err(_) => return false,
    };

    let payload = lines.join("\n");
    let content = format!("{}{}{}", header_json, SWAP_DELIMITER, payload);

    fs::write(swp_path, content).is_ok()
}

/// Reads swap file, verifies magic, and extracts header and payload lines.
pub fn read_swap_file(swp_path: &Path) -> Option<(SwapHeader, Vec<String>)> {
    let content = fs::read_to_string(swp_path).ok()?;
    let parts: Vec<&str> = content.splitn(2, SWAP_DELIMITER).collect();
    if parts.len() != 2 {
        return None;
    }

    let header: SwapHeader = serde_json::from_str(parts[0]).ok()?;
    if header.magic != SWAP_MAGIC {
        return None;
    }

    let payload = parts[1];
    let lines = payload.lines().map(|s| s.to_string()).collect();

    Some((header, lines))
}

/// Removes the swap file if it exists. Returns true if removed, false if not found or failed.
pub fn remove_swap_file(swp_path: &Path) -> bool {
    if swp_path.exists() {
        fs::remove_file(swp_path).is_ok()
    } else {
        false
    }
}
