//! 6-tier macro placeholder interpolation engine for BashMenu.
//!
//! Evaluates placeholders in priority order:
//! 1. Environment variables: `{env:VAR_NAME}`
//! 2. Runtime & system builtins: `{home}`, `{bashmenu_dir}`, `{scripts_dir}`, `{user}`, `{theme}`, etc.
//! 3. Configuration properties: `{user.<key>}`, `{settings.<key>}`, `{app.<key>}`
//! 4. CP437 ASCII codes: `{ascii:NNN}`
//! 5. Adaptive glyphs: `{nf:[char]:[hex]:[emoji]}`
//! 6. Context variables: `{window_width}`, `{screen_width}`, etc.

use crate::config::BashMenuConfig;
use crate::glyph::{parse_ascii_val, resolve_glyph};
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Runtime context passed into the macro interpolation engine
#[derive(Debug, Clone)]
pub struct MacroContext<'a> {
    /// Active application configuration
    pub config: &'a BashMenuConfig,
    /// Root directory of BashMenu
    pub bashmenu_dir: PathBuf,
    /// User home directory
    pub home_dir: PathBuf,
    /// Development/repository root directory where bashmenu-rs resides
    pub dev_dir: PathBuf,
    /// Terminal viewport width
    pub window_width: usize,
    /// Terminal viewport height
    pub window_height: usize,
    /// Full screen width
    pub screen_width: usize,
    /// Extra runtime variables
    pub extra_vars: HashMap<String, String>,
}

impl<'a> MacroContext<'a> {
    /// Creates a default context with detected environment paths
    pub fn new(config: &'a BashMenuConfig) -> Self {
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"));
        let bashmenu_dir = std::env::var("BASHMENU_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let user_cfg = home_dir.join(".config/bashmenu");
                if user_cfg.exists() {
                    user_cfg
                } else {
                    let cur = std::env::current_dir().unwrap_or_else(|_| home_dir.clone());
                    if cur.join("bashmenu.mnu").exists() || cur.join("bashmenu.yml").exists() {
                        cur
                    } else {
                        user_cfg
                    }
                }
            });
        let dev_dir = resolve_dev_dir(&home_dir, &bashmenu_dir);

        Self {
            config,
            bashmenu_dir,
            home_dir,
            dev_dir,
            window_width: 80,
            window_height: 24,
            screen_width: 80,
            extra_vars: HashMap::new(),
        }
    }
}

/// Resolves the repository / project github directory where bashmenu-rs resides on the PC.
///
/// Detection strategy (non-hardcoded):
/// 1. Environment variable override: `BASHMENU_DEV_DIR` or `DEV_DIR`.
/// 2. Current working directory or ancestors containing `.git` and (`crates/bashmenu-core` or `Cargo.toml`).
/// 3. Canonical symlinks in `~/.config/bashmenu` (e.g. `assets`, `scripts`, `bashmenu.md`) pointing into the repository root.
/// 4. `CARGO_MANIFEST_DIR` if running under tests / cargo build.
/// 5. Walk up from current executable path `std::env::current_exe()`.
/// 6. Common user development directories under `home_dir`: `github/bashmenu-rs`, `projects/bashmenu-rs`, etc.
/// 7. Top-level git directory via `git rev-parse --show-toplevel`.
/// 8. Fallback to `bashmenu_dir`.
pub fn resolve_dev_dir(home_dir: &Path, bashmenu_dir: &Path) -> PathBuf {
    // 1. Environment variable override
    if let Ok(val) = std::env::var("BASHMENU_DEV_DIR").or_else(|_| std::env::var("DEV_DIR")) {
        let p = PathBuf::from(val.trim());
        if p.exists() {
            return p;
        }
    }

    // 2. Current working directory and ancestors
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            if (ancestor.join(".git").exists() || ancestor.join(".git").is_file())
                && (ancestor.join("crates/bashmenu-core").exists()
                    || (ancestor.join("Cargo.toml").exists()
                        && ancestor.join("bashmenu.mnu").exists()))
            {
                return ancestor.to_path_buf();
            }
        }
    }

    // 3. Check symlinks in bashmenu_dir (assets, scripts, bashmenu.md) pointing to the repo root
    for link_name in &["assets", "scripts", "bashmenu.md"] {
        let link_path = bashmenu_dir.join(link_name);
        if let Ok(target) = std::fs::read_link(&link_path) {
            let abs_target = if target.is_relative() {
                bashmenu_dir.join(&target)
            } else {
                target
            };
            if let Ok(canon) = abs_target.canonicalize() {
                if let Some(parent) = canon.parent() {
                    if parent.join(".git").exists() || parent.join("Cargo.toml").exists() {
                        return parent.to_path_buf();
                    }
                }
            }
        }
    }

    // 4. Check CARGO_MANIFEST_DIR (test harnesses and dev runs)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest_dir);
        for ancestor in p.ancestors() {
            if (ancestor.join(".git").exists() || ancestor.join(".git").is_file())
                && (ancestor.join("crates/bashmenu-core").exists()
                    || (ancestor.join("Cargo.toml").exists()
                        && ancestor.join("bashmenu.mnu").exists()))
            {
                return ancestor.to_path_buf();
            }
        }
    }

    // 5. Check executable path ancestors
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canon) = exe.canonicalize() {
            for ancestor in canon.ancestors() {
                if (ancestor.join(".git").exists() || ancestor.join(".git").is_file())
                    && (ancestor.join("crates/bashmenu-core").exists()
                        || (ancestor.join("Cargo.toml").exists()
                            && ancestor.join("bashmenu.mnu").exists()))
                {
                    return ancestor.to_path_buf();
                }
            }
        }
    }

    // 6. Common user development directories under home
    let common_subpaths = [
        "github/bashmenu-rs",
        "github/bashmenu",
        "src/bashmenu-rs",
        "projects/bashmenu-rs",
        "code/bashmenu-rs",
        "dev/bashmenu-rs",
        "git/bashmenu-rs",
        "Repositories/bashmenu-rs",
    ];
    for sub in &common_subpaths {
        let cand = home_dir.join(sub);
        if cand.is_dir() && (cand.join(".git").exists() || cand.join("Cargo.toml").exists()) {
            return cand;
        }
    }

    // 7. Fallback to git rev-parse if git CLI is present
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                let p = PathBuf::from(s);
                if p.is_dir() {
                    return p;
                }
            }
        }
    }

    // 8. Fallback to bashmenu_dir
    bashmenu_dir.to_path_buf()
}

struct LocalTimeStrings {
    date_time_24: String,
    date_time_24_short: String,
    date_time_12: String,
    date_time_12_short: String,
    date: String,
    time_24: String,
    time_24_short: String,
    time_12: String,
    time_12_short: String,
}

fn get_local_time_strings() -> LocalTimeStrings {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    let tm_ptr = unsafe { libc::localtime_r(&now, tm.as_mut_ptr()) };
    if !tm_ptr.is_null() {
        let tm = unsafe { tm.assume_init() };
        let year = tm.tm_year + 1900;
        let month = tm.tm_mon + 1;
        let day = tm.tm_mday;
        let hour = tm.tm_hour;
        let min = tm.tm_min;
        let sec = tm.tm_sec;
        let hour12 = if hour == 0 {
            12
        } else if hour > 12 {
            hour - 12
        } else {
            hour
        };
        let ampm = if hour >= 12 { "PM" } else { "AM" };

        let date_time_24 = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month, day, hour, min, sec
        );
        let date_time_24_short =
            format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hour, min);
        let date_time_12 = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
            year, month, day, hour12, min, sec, ampm
        );
        let date_time_12_short = format!(
            "{:04}-{:02}-{:02} {:02}:{:02} {}",
            year, month, day, hour12, min, ampm
        );
        let date = format!("{:04}-{:02}-{:02}", year, month, day);
        let time_24 = format!("{:02}:{:02}:{:02}", hour, min, sec);
        let time_24_short = format!("{:02}:{:02}", hour, min);
        let time_12 = format!("{:02}:{:02}:{:02} {}", hour12, min, sec, ampm);
        let time_12_short = format!("{:02}:{:02} {}", hour12, min, ampm);

        LocalTimeStrings {
            date_time_24,
            date_time_24_short,
            date_time_12,
            date_time_12_short,
            date,
            time_24,
            time_24_short,
            time_12,
            time_12_short,
        }
    } else {
        LocalTimeStrings {
            date_time_24: String::new(),
            date_time_24_short: String::new(),
            date_time_12: String::new(),
            date_time_12_short: String::new(),
            date: String::new(),
            time_24: String::new(),
            time_24_short: String::new(),
            time_12: String::new(),
            time_12_short: String::new(),
        }
    }
}

fn resolve_profile_path(home_dir: &std::path::Path) -> PathBuf {
    let p = home_dir.join(".profile");
    if p.exists() {
        p
    } else {
        let bp = home_dir.join(".bash_profile");
        if bp.exists() {
            bp
        } else {
            p
        }
    }
}

fn resolve_bash_profile_path(home_dir: &std::path::Path) -> PathBuf {
    let bp = home_dir.join(".bash_profile");
    if bp.exists() {
        bp
    } else {
        let p = home_dir.join(".profile");
        if p.exists() {
            p
        } else {
            bp
        }
    }
}

fn resolve_shell_profile(home_dir: &std::path::Path) -> PathBuf {
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.ends_with("zsh") {
        let zprof = home_dir.join(".zprofile");
        if zprof.exists() {
            return zprof;
        }
        let zshrc = home_dir.join(".zshrc");
        if zshrc.exists() {
            return zshrc;
        }
        zprof
    } else {
        let bp = home_dir.join(".bash_profile");
        if bp.exists() {
            return bp;
        }
        let brc = home_dir.join(".bashrc");
        if brc.exists() {
            return brc;
        }
        let prof = home_dir.join(".profile");
        if prof.exists() {
            return prof;
        }
        brc
    }
}

fn get_primary_ip() -> String {
    use std::net::UdpSocket;
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("1.1.1.1:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

fn get_battery_info() -> String {
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_bat = name.starts_with("BAT") || {
                let type_path = entry.path().join("type");
                std::fs::read_to_string(type_path)
                    .map(|t| t.to_lowercase().contains("battery"))
                    .unwrap_or(false)
            };
            if is_bat {
                let cap_path = entry.path().join("capacity");
                if let Ok(cap) = std::fs::read_to_string(cap_path) {
                    return format!("{}%", cap.trim());
                }
            }
        }
    }
    "N/A".to_string()
}

fn get_battery_status() -> String {
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_bat = name.starts_with("BAT") || {
                let type_path = entry.path().join("type");
                std::fs::read_to_string(type_path)
                    .map(|t| t.to_lowercase().contains("battery"))
                    .unwrap_or(false)
            };
            if is_bat {
                let stat_path = entry.path().join("status");
                if let Ok(stat) = std::fs::read_to_string(stat_path) {
                    return stat.trim().to_string();
                }
            }
        }
    }
    "N/A".to_string()
}

pub fn resolve_config_dot_path(config_val: &serde_json::Value, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }

    // Try resolving directly (e.g. settings.ping_target or user.postal_code)
    if let Some(val) = lookup_value_path(config_val, &parts) {
        return Some(val);
    }

    // If path starts with "app" or "bashmenu", try stripping prefix
    if parts.len() > 1 && (parts[0] == "app" || parts[0] == "bashmenu") {
        if let Some(val) = lookup_value_path(config_val, &parts[1..]) {
            return Some(val);
        }
    }

    None
}

fn lookup_value_path(mut curr: &serde_json::Value, parts: &[&str]) -> Option<String> {
    for &part in parts {
        match curr {
            serde_json::Value::Object(map) => {
                curr = map.get(part)?;
            }
            _ => return None,
        }
    }

    match curr {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => None,
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(curr).ok()
        }
    }
}

static NF_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{nf:[^}]*\}").unwrap());
static ENV_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{env:([a-zA-Z0-9_]+)\}").unwrap());
static DOT_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)+)\}").unwrap());
static COMMAND_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{command:((?:[^{}]|\{[^{}]*\})+)\}").unwrap());
static FILE_PICKER_MACRO_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\{(file_picker(?:_new)?)(?::((?:[^{}]|\{[^{}]*\})+))?\}").unwrap()
});
static DIR_PICKER_MACRO_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\{(dir_picker(?:_new)?)(?::((?:[^{}]|\{[^{}]*\})+))?\}").unwrap()
});
static PICKER_MACRO_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\{(file_picker(?:_new)?|dir_picker(?:_new)?)(?::((?:[^{}]|\{[^{}]*\})+))?\}")
        .unwrap()
});
static PRECEDING_QUOTED_DIR_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r#"["'`]([^"'`]+)/$"#).unwrap());
static PRECEDING_UNQUOTED_DIR_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?:^|[\s=])(\S+)/$").unwrap());

const CMD_CACHE_TTL_SECS: u64 = 30;
static CMD_CACHE: std::sync::LazyLock<
    std::sync::Mutex<HashMap<String, (std::time::Instant, String)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

fn eval_command_substitution(cmd_str: &str) -> String {
    let trimmed = cmd_str.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(cache) = CMD_CACHE.lock() {
        if let Some((ts, cached_out)) = cache.get(trimmed) {
            if ts.elapsed() < std::time::Duration::from_secs(CMD_CACHE_TTL_SECS) {
                return cached_out.clone();
            }
        }
    }
    let output = std::process::Command::new("/bin/bash")
        .arg("-c")
        .arg(trimmed)
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default();
    if let Ok(mut cache) = CMD_CACHE.lock() {
        cache.insert(
            trimmed.to_string(),
            (std::time::Instant::now(), output.clone()),
        );
    }
    output
}

/// Parsed specification for a `{file_picker...}` or `{dir_picker...}` interactive macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerSpec {
    /// True if this is a directory picker (`dir_picker` / `dir_picker_new`)
    pub is_dir: bool,
    /// True if creation of new files/folders (`'n'`) is allowed
    pub is_new: bool,
    /// Optional path modifier (`stem`, `name`, `base`, `dir`, `ext`, `parent`)
    pub modifier: Option<String>,
    /// Resolved starting directory for the picker modal
    pub start_dir: PathBuf,
}

fn expand_tilde_path(raw: &str, home_dir: &Path) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        home_dir.to_path_buf()
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        home_dir.join(rest)
    } else {
        PathBuf::from(trimmed)
    }
}

/// Parses a picker macro type and optional colon-delimited specification into
/// `(is_new, modifier, start_dir_string)`.
pub fn parse_picker_macro(
    macro_type: &str,
    spec: Option<&str>,
    ctx: Option<&MacroContext>,
) -> (bool, Option<String>, Option<String>) {
    let mut is_new = macro_type.ends_with("_new");
    let mut modifier = None;
    let mut start_dir = None;

    if let Some(raw_spec) = spec {
        let resolved_spec = if let Some(c) = ctx {
            if raw_spec.contains('{') {
                interpolate_placeholders(raw_spec, c)
            } else {
                raw_spec.to_string()
            }
        } else {
            raw_spec.to_string()
        };

        let mut tokens: Vec<&str> = resolved_spec.split(':').collect();
        if let Some(pos) = tokens.iter().position(|&t| t == "new") {
            is_new = true;
            tokens.remove(pos);
        }

        if let Some(&last) = tokens.last() {
            if matches!(last, "stem" | "name" | "base" | "dir" | "ext" | "parent") {
                modifier = Some(last.to_string());
                tokens.pop();
            }
        }

        if !tokens.is_empty() {
            if tokens[0] == "path" {
                tokens.remove(0);
            }
            if !tokens.is_empty() {
                start_dir = Some(tokens.join(":"));
            }
        }
    }

    (is_new, modifier, start_dir)
}

/// Extracts the first `{file_picker...}` or `{dir_picker...}` specification from an action string.
pub fn extract_picker_spec(
    action_str: &str,
    ctx: &MacroContext,
    default_start_dir: Option<&str>,
) -> Option<PickerSpec> {
    let m = PICKER_MACRO_RE.captures(action_str)?;
    let full_match = m.get(0)?;
    let macro_type = m.get(1)?.as_str();
    let spec_opt = m.get(2).map(|s| s.as_str());
    let is_dir = macro_type.starts_with("dir_picker");
    let (is_new, modifier, parsed_start) = parse_picker_macro(macro_type, spec_opt, Some(ctx));

    let start_dir_str = if let Some(sd) = parsed_start {
        if sd.contains('{') {
            interpolate_placeholders(&sd, ctx)
        } else {
            sd
        }
    } else {
        let preceding = &action_str[..full_match.start()];
        let resolved_prec = if preceding.contains('{') {
            interpolate_placeholders(preceding, ctx)
        } else {
            preceding.to_string()
        };
        let cand_opt = PRECEDING_QUOTED_DIR_RE
            .captures(&resolved_prec)
            .or_else(|| PRECEDING_UNQUOTED_DIR_RE.captures(&resolved_prec))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        if let Some(cand) = cand_opt {
            let expanded = expand_tilde_path(&cand, &ctx.home_dir);
            if expanded.is_dir() || default_start_dir.is_none() {
                cand
            } else {
                let def = default_start_dir.unwrap_or("~");
                interpolate_placeholders(def, ctx)
            }
        } else {
            let def = default_start_dir.unwrap_or("~");
            interpolate_placeholders(def, ctx)
        }
    };

    Some(PickerSpec {
        is_dir,
        is_new,
        modifier,
        start_dir: expand_tilde_path(&start_dir_str, &ctx.home_dir),
    })
}

/// Substitutes `{file_picker...}` macros and their modifiers (`:stem`, `:name`, `:base`, `:dir`, `:ext`, `:path:`)
/// with the chosen file path.
pub fn apply_file_picker_macros(
    template_str: &str,
    chosen_path: &Path,
    ctx: Option<&MacroContext>,
) -> String {
    if !template_str.contains("{file_picker") {
        return template_str.to_string();
    }
    let fpath = chosen_path.to_string_lossy().to_string();
    let p_name = chosen_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let p_base = chosen_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let p_dir = chosen_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let p_stem = if p_dir.is_empty() || p_dir == "." {
        p_base.clone()
    } else {
        chosen_path
            .parent()
            .unwrap()
            .join(&p_base)
            .to_string_lossy()
            .to_string()
    };
    let p_ext = chosen_path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    let mut res = template_str.to_string();

    // Avoid duplicating a preceding directory prefix when the macro has no modifier
    if let Some(caps) = FILE_PICKER_MACRO_RE.captures(&res) {
        if let (Some(full), Some(m_type)) = (caps.get(0), caps.get(1)) {
            let (_, modifier, _) =
                parse_picker_macro(m_type.as_str(), caps.get(2).map(|m| m.as_str()), ctx);
            if modifier.is_none() {
                let preceding = &res[..full.start()];
                let dir_cap = PRECEDING_QUOTED_DIR_RE
                    .captures(preceding)
                    .or_else(|| PRECEDING_UNQUOTED_DIR_RE.captures(preceding));
                if let Some(dc) = dir_cap {
                    if let Some(dir_match) = dc.get(1) {
                        let cand = dir_match.as_str();
                        let cand_resolved = match ctx {
                            Some(c) if cand.contains('{') => interpolate_placeholders(cand, c),
                            _ => cand.to_string(),
                        };
                        let home_fallback = PathBuf::from("/");
                        let home_ref = ctx.map(|c| c.home_dir.as_path()).unwrap_or(&home_fallback);
                        let cand_path = expand_tilde_path(&cand_resolved, home_ref);
                        let cand_clean = cand_path
                            .to_string_lossy()
                            .trim_end_matches('/')
                            .to_string();
                        let parent_clean = p_dir.trim_end_matches('/').to_string();
                        if (!cand_clean.is_empty() && cand_clean == parent_clean)
                            || cand_path.is_dir()
                        {
                            res = format!(
                                "{}{}{}",
                                &res[..dir_match.start()],
                                fpath,
                                &res[full.end()..]
                            );
                        }
                    }
                }
            }
        }
    }

    FILE_PICKER_MACRO_RE
        .replace_all(&res, |caps: &Captures| {
            let m_type = &caps[1];
            let spec_opt = caps.get(2).map(|m| m.as_str());
            let (_, modifier, _) = parse_picker_macro(m_type, spec_opt, ctx);
            match modifier.as_deref() {
                Some("stem") => p_stem.clone(),
                Some("name") => p_name.clone(),
                Some("base") => p_base.clone(),
                Some("dir") => p_dir.clone(),
                Some("ext") => p_ext.clone(),
                _ => fpath.clone(),
            }
        })
        .into_owned()
}

/// Substitutes `{dir_picker...}` macros and their modifiers (`:name`, `:dir`, `:parent`, `:path:`)
/// with the chosen directory path.
pub fn apply_dir_picker_macros(
    template_str: &str,
    chosen_path: &Path,
    ctx: Option<&MacroContext>,
) -> String {
    if !template_str.contains("{dir_picker") {
        return template_str.to_string();
    }
    let dpath = chosen_path.to_string_lossy().to_string();
    let p_name = chosen_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let p_dir = chosen_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut res = template_str.to_string();

    if let Some(caps) = DIR_PICKER_MACRO_RE.captures(&res) {
        if let (Some(full), Some(m_type)) = (caps.get(0), caps.get(1)) {
            let (_, modifier, _) =
                parse_picker_macro(m_type.as_str(), caps.get(2).map(|m| m.as_str()), ctx);
            if modifier.is_none() {
                let preceding = &res[..full.start()];
                let dir_cap = PRECEDING_QUOTED_DIR_RE
                    .captures(preceding)
                    .or_else(|| PRECEDING_UNQUOTED_DIR_RE.captures(preceding));
                if let Some(dc) = dir_cap {
                    if let Some(dir_match) = dc.get(1) {
                        let cand = dir_match.as_str();
                        let cand_resolved = match ctx {
                            Some(c) if cand.contains('{') => interpolate_placeholders(cand, c),
                            _ => cand.to_string(),
                        };
                        let home_fallback = PathBuf::from("/");
                        let home_ref = ctx.map(|c| c.home_dir.as_path()).unwrap_or(&home_fallback);
                        let cand_path = expand_tilde_path(&cand_resolved, home_ref);
                        let cand_clean = cand_path
                            .to_string_lossy()
                            .trim_end_matches('/')
                            .to_string();
                        let parent_clean = p_dir.trim_end_matches('/').to_string();
                        if (!cand_clean.is_empty() && cand_clean == parent_clean)
                            || cand_path.is_dir()
                        {
                            res = format!(
                                "{}{}{}",
                                &res[..dir_match.start()],
                                dpath,
                                &res[full.end()..]
                            );
                        }
                    }
                }
            }
        }
    }

    DIR_PICKER_MACRO_RE
        .replace_all(&res, |caps: &Captures| {
            let m_type = &caps[1];
            let spec_opt = caps.get(2).map(|m| m.as_str());
            let (_, modifier, _) = parse_picker_macro(m_type, spec_opt, ctx);
            match modifier.as_deref() {
                Some("name") => p_name.clone(),
                Some("dir") | Some("parent") => p_dir.clone(),
                _ => dpath.clone(),
            }
        })
        .into_owned()
}
static TOAST_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"\{toast:\s*(?:(\d+(?:\.\d+)?)\s*:\s*)?(?:"([^"]*)"|'([^']*)'|([^}]+))\s*\}"#)
        .unwrap()
});

/// Toast notification severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ToastSeverity {
    #[default]
    Information,
    Warning,
    Error,
}

/// A parsed toast notification from macro evaluation
#[derive(Debug, Clone, PartialEq)]
pub struct ToastNotification {
    pub message: String,
    pub title: Option<String>,
    pub severity: ToastSeverity,
    pub timeout_secs: f64,
}

impl ToastNotification {
    pub fn new(message: impl Into<String>, title: Option<String>, timeout_secs: f64) -> Self {
        Self {
            message: message.into(),
            title,
            severity: ToastSeverity::Information,
            timeout_secs,
        }
    }
}

/// Extracts `{toast:SECONDS:"MESSAGE"}` tokens, interpolates nested placeholders inside
/// the message text, and returns the cleaned string along with the extracted toast notifications.
pub fn extract_toasts(text: &str, ctx: &MacroContext) -> (String, Vec<ToastNotification>) {
    if !text.contains("{toast:") {
        return (text.to_string(), Vec::new());
    }

    let mut notifications = Vec::new();
    let cleaned = TOAST_RE.replace_all(text, |caps: &Captures| {
        let timeout = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or(2.0);

        let raw_msg = caps
            .get(2)
            .or_else(|| caps.get(3))
            .or_else(|| caps.get(4))
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim();

        let interp_msg = if raw_msg.contains('{') {
            interpolate_placeholders(raw_msg, ctx)
        } else {
            raw_msg.to_string()
        };

        notifications.push(ToastNotification {
            message: interp_msg,
            title: None,
            severity: ToastSeverity::Information,
            timeout_secs: timeout,
        });

        ""
    });

    (cleaned.into_owned(), notifications)
}

/// Recursively interpolates macro placeholders within a text string.
pub fn interpolate_placeholders(text: &str, ctx: &MacroContext) -> String {
    interpolate_internal(text, ctx, 0)
}

fn interpolate_internal(text: &str, ctx: &MacroContext, depth: usize) -> String {
    if text.is_empty() || !text.contains('{') {
        return text.to_string();
    }

    // 0. Toast tokens: strip so they do not leak into visible labels/titles
    let mut res = if text.contains("{toast:") {
        TOAST_RE.replace_all(text, "").to_string()
    } else {
        text.to_string()
    };

    // 1. CP437 ASCII: {ascii:NNN}
    res = parse_ascii_val(&res);

    // 2. Adaptive glyph: {nf:...}
    if res.contains("{nf:") {
        let use_nerd = ctx.config.settings.use_nerd_fonts.unwrap_or(false);
        res = NF_RE
            .replace_all(&res, |caps: &Captures| resolve_glyph(&caps[0], use_nerd))
            .to_string();
    }

    // 3. Environment variables: {env:VAR_NAME}
    if res.contains("{env:") {
        res = ENV_RE
            .replace_all(&res, |caps: &Captures| {
                std::env::var(&caps[1]).unwrap_or_default()
            })
            .to_string();
    }

    // 4. Runtime & System Builtins (computed lazily only if referenced)
    let mut direct_map: HashMap<&str, String> = HashMap::new();

    if res.contains("{home}") {
        direct_map.insert("{home}", ctx.home_dir.to_string_lossy().to_string());
    }
    if res.contains("{dev_dir}") || res.contains("{dev}") {
        let dev_str = ctx.dev_dir.to_string_lossy().to_string();
        direct_map.insert("{dev_dir}", dev_str.clone());
        direct_map.insert("{dev}", dev_str);
    }
    if res.contains("{bashmenu_dir}") {
        direct_map.insert(
            "{bashmenu_dir}",
            ctx.bashmenu_dir.to_string_lossy().to_string(),
        );
    }
    if res.contains("{scripts_dir}") || res.contains("{scripts}") {
        let scripts_dir = ctx
            .config
            .settings
            .scripts_dir
            .clone()
            .unwrap_or_else(|| format!("{}/scripts", ctx.bashmenu_dir.display()));
        direct_map.insert("{scripts_dir}", scripts_dir.clone());
        direct_map.insert("{scripts}", scripts_dir);
    }
    if res.contains("{templates_dir}") || res.contains("{templates}") {
        let templates_dir = ctx
            .config
            .settings
            .templates_dir
            .clone()
            .unwrap_or_else(|| format!("{}/templates", ctx.bashmenu_dir.display()));
        direct_map.insert("{templates_dir}", templates_dir.clone());
        direct_map.insert("{templates}", templates_dir);
    }
    if res.contains("{cache_dir}") || res.contains("{cache}") {
        let cache_dir = ctx
            .config
            .settings
            .cache_dir
            .clone()
            .unwrap_or_else(|| format!("{}/.cache/bashmenu", ctx.home_dir.display()));
        direct_map.insert("{cache_dir}", cache_dir.clone());
        direct_map.insert("{cache}", cache_dir);
    }
    if res.contains("{notebooks_dir}") || res.contains("{notebooks}") {
        let notebooks_dir = format!("{}/notebooks", ctx.bashmenu_dir.display());
        direct_map.insert("{notebooks_dir}", notebooks_dir.clone());
        direct_map.insert("{notebooks}", notebooks_dir);
    }

    // Shell profiles and editor configs (guarded to avoid disk stats per frame)
    if res.contains("{bashrc}") {
        direct_map.insert(
            "{bashrc}",
            ctx.home_dir.join(".bashrc").to_string_lossy().to_string(),
        );
    }
    if res.contains("{bash_aliases}") {
        direct_map.insert(
            "{bash_aliases}",
            ctx.home_dir
                .join(".bash_aliases")
                .to_string_lossy()
                .to_string(),
        );
    }
    if res.contains("{zshrc}") {
        direct_map.insert(
            "{zshrc}",
            ctx.home_dir.join(".zshrc").to_string_lossy().to_string(),
        );
    }
    if res.contains("{profile}") {
        direct_map.insert(
            "{profile}",
            resolve_profile_path(&ctx.home_dir)
                .to_string_lossy()
                .to_string(),
        );
    }
    if res.contains("{bash_profile}") {
        direct_map.insert(
            "{bash_profile}",
            resolve_bash_profile_path(&ctx.home_dir)
                .to_string_lossy()
                .to_string(),
        );
    }
    if res.contains("{zprofile}") {
        direct_map.insert(
            "{zprofile}",
            ctx.home_dir.join(".zprofile").to_string_lossy().to_string(),
        );
    }
    if res.contains("{shell_profile}") {
        direct_map.insert(
            "{shell_profile}",
            resolve_shell_profile(&ctx.home_dir)
                .to_string_lossy()
                .to_string(),
        );
    }
    if res.contains("{vimrc}") {
        direct_map.insert(
            "{vimrc}",
            ctx.home_dir.join(".vimrc").to_string_lossy().to_string(),
        );
    }
    if res.contains("{nanorc}") {
        direct_map.insert(
            "{nanorc}",
            ctx.home_dir.join(".nanorc").to_string_lossy().to_string(),
        );
    }

    // Cross-Platform Environment
    if res.contains("{prefix}") {
        let prefix = std::env::var("PREFIX").unwrap_or_else(|_| "/usr".to_string());
        direct_map.insert("{prefix}", prefix);
    }
    if res.contains("{termux_properties}") {
        direct_map.insert(
            "{termux_properties}",
            ctx.home_dir
                .join(".termux/termux.properties")
                .to_string_lossy()
                .to_string(),
        );
    }
    if res.contains("{termux_storage}") {
        direct_map.insert(
            "{termux_storage}",
            ctx.home_dir.join("storage").to_string_lossy().to_string(),
        );
    }
    if res.contains("{powershell_profile}") {
        let powershell_profile = std::env::var("PROFILE").unwrap_or_else(|_| {
            ctx.home_dir
                .join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
                .to_string_lossy()
                .to_string()
        });
        direct_map.insert("{powershell_profile}", powershell_profile);
    }
    if res.contains("{appdata}") {
        direct_map.insert("{appdata}", std::env::var("APPDATA").unwrap_or_default());
    }
    if res.contains("{userprofile}") {
        let userprofile = std::env::var("USERPROFILE")
            .unwrap_or_else(|_| ctx.home_dir.to_string_lossy().to_string());
        direct_map.insert("{userprofile}", userprofile);
    }

    // Divider
    if res.contains("{divider}") {
        let div_len = ctx.window_width.max(10);
        direct_map.insert("{divider}", "─".repeat(div_len));
    }

    if res.contains("{user}") || res.contains("{username}") {
        let user_name = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        direct_map.insert("{user}", user_name.clone());
        direct_map.insert("{username}", user_name);
    }
    if res.contains("{host}") || res.contains("{hostname}") {
        let host_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        direct_map.insert("{host}", host_name.clone());
        direct_map.insert("{hostname}", host_name);
    }
    if res.contains("{user-mode}") {
        let is_root = unsafe { libc::geteuid() } == 0;
        let user_mode = if is_root { "Root" } else { "User" };
        direct_map.insert("{user-mode}", user_mode.to_string());
    }
    if res.contains("{version}") {
        direct_map.insert("{version}", ctx.config.version.clone());
    }
    if res.contains("{theme}") || res.contains("{app.theme}") {
        direct_map.insert("{theme}", ctx.config.theme.clone());
        direct_map.insert("{app.theme}", ctx.config.theme.clone());
    }

    if res.contains("{window_width}") {
        direct_map.insert("{window_width}", ctx.window_width.to_string());
    }
    if res.contains("{window_height}") {
        direct_map.insert("{window_height}", ctx.window_height.to_string());
    }
    if res.contains("{screen_width}") {
        direct_map.insert("{screen_width}", ctx.screen_width.to_string());
    }

    if res.contains("{date") || res.contains("{time") {
        let times = get_local_time_strings();
        direct_map.insert("{date_time_24}", times.date_time_24);
        direct_map.insert("{date_time_24_short}", times.date_time_24_short);
        direct_map.insert("{date_time_12}", times.date_time_12);
        direct_map.insert("{date_time_12_short}", times.date_time_12_short);
        direct_map.insert("{date}", times.date);
        direct_map.insert("{time_24}", times.time_24);
        direct_map.insert("{time_24_short}", times.time_24_short);
        direct_map.insert("{time_12}", times.time_12);
        direct_map.insert("{time_12_short}", times.time_12_short);
    }
    if res.contains("{utc_seconds}") {
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        direct_map.insert("{utc_seconds}", now.to_string());
    }

    if res.contains("{localip}") {
        direct_map.insert("{localip}", get_primary_ip());
    }
    if res.contains("{battery}") {
        direct_map.insert("{battery}", get_battery_info());
    }
    if res.contains("{battery_stat}") || res.contains("{battery_status}") {
        let stat = get_battery_status();
        direct_map.insert("{battery_stat}", stat.clone());
        direct_map.insert("{battery_status}", stat);
    }

    for (k, v) in direct_map {
        if res.contains(k) {
            res = res.replace(k, &v);
        }
    }

    // 5. Extra variables from context
    for (k, v) in &ctx.extra_vars {
        let placeholder = format!("{{{}}}", k);
        if res.contains(&placeholder) {
            res = res.replace(&placeholder, v);
        }
    }

    // 6. Dot notation properties: {user.<key>}, {settings.<key>}, {app.<key>}, etc.
    if res.contains('.') && res.contains('{') {
        let config_val = serde_json::to_value(ctx.config).unwrap_or(serde_json::Value::Null);

        res = DOT_RE
            .replace_all(&res, |caps: &Captures| {
                let path = &caps[1];
                if let Some(val) = resolve_config_dot_path(&config_val, path) {
                    val
                } else {
                    caps[0].to_string()
                }
            })
            .to_string();
    }

    // 7. Command substitution: {command:<cmd>} with 30s TTL caching
    if res.contains("{command:") {
        res = COMMAND_RE
            .replace_all(&res, |caps: &Captures| {
                let raw_cmd = &caps[1];
                let resolved_cmd = if raw_cmd.contains('{') && depth < 3 {
                    interpolate_internal(raw_cmd, ctx, depth + 1)
                } else {
                    raw_cmd.to_string()
                };
                eval_command_substitution(&resolved_cmd)
            })
            .to_string();
    }

    // Recursive resolution (up to depth 3) if placeholders remain
    if res.contains('{') && depth < 3 {
        res = interpolate_internal(&res, ctx, depth + 1);
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_hierarchy() {
        let mut config = BashMenuConfig {
            theme: "cyberpunk".to_string(),
            ..Default::default()
        };
        config.user.insert(
            "postal_code".to_string(),
            serde_yml::Value::String("49079".to_string()),
        );

        let mut ctx = MacroContext::new(&config);
        ctx.bashmenu_dir = PathBuf::from("/opt/bashmenu");
        ctx.home_dir = PathBuf::from("/home/user");

        // Environment variable
        std::env::set_var("BM_TEST_ENV", "Antigravity");
        assert_eq!(
            interpolate_placeholders("Hello {env:BM_TEST_ENV}", &ctx),
            "Hello Antigravity"
        );

        // Runtime builtins
        assert_eq!(
            interpolate_placeholders("Path: {bashmenu_dir}/bin", &ctx),
            "Path: /opt/bashmenu/bin"
        );
        assert_eq!(
            interpolate_placeholders("Active: {theme}", &ctx),
            "Active: cyberpunk"
        );

        // Dot variables: user.<key>
        assert_eq!(
            interpolate_placeholders("Zip: {user.postal_code}", &ctx),
            "Zip: 49079"
        );

        // CP437 ASCII macro
        assert_eq!(
            interpolate_placeholders("Border: {ascii:196}", &ctx),
            "Border: ─"
        );

        // Nested macro resolution: {scripts_dir} -> {bashmenu_dir}/scripts -> /opt/bashmenu/scripts
        assert_eq!(
            interpolate_placeholders("Run: {scripts_dir}/tool.sh", &ctx),
            "Run: /opt/bashmenu/scripts/tool.sh"
        );
    }

    #[test]
    fn test_toast_extraction_and_interpolation() {
        let config = BashMenuConfig {
            theme: "dracula".to_string(),
            ..Default::default()
        };
        let ctx = MacroContext::new(&config);

        let input = r#"{toast:2:"Hello, world. Using theme: {theme}"}"#;
        let (cleaned, toasts) = extract_toasts(input, &ctx);
        assert_eq!(cleaned, "");
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].timeout_secs, 2.0);
        assert_eq!(toasts[0].message, "Hello, world. Using theme: dracula");

        // Toast with surrounding command
        let input2 = r#"echo 'before' {toast:1.5:'Saved!'} && echo 'after'"#;
        let (cleaned2, toasts2) = extract_toasts(input2, &ctx);
        assert_eq!(cleaned2, "echo 'before'  && echo 'after'");
        assert_eq!(toasts2.len(), 1);
        assert_eq!(toasts2[0].timeout_secs, 1.5);
        assert_eq!(toasts2[0].message, "Saved!");

        // General interpolation strips {toast:...} so it does not leak into visible text
        assert_eq!(
            interpolate_placeholders(r#"Status: {toast:2:"popup"}Ready"#, &ctx),
            "Status: Ready"
        );
    }
}
