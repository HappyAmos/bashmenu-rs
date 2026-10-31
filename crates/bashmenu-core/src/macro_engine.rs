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
use std::path::PathBuf;

/// Runtime context passed into the macro interpolation engine
#[derive(Debug, Clone)]
pub struct MacroContext<'a> {
    /// Active application configuration
    pub config: &'a BashMenuConfig,
    /// Root directory of BashMenu
    pub bashmenu_dir: PathBuf,
    /// User home directory
    pub home_dir: PathBuf,
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

        Self {
            config,
            bashmenu_dir,
            home_dir,
            window_width: 80,
            window_height: 24,
            screen_width: 80,
            extra_vars: HashMap::new(),
        }
    }
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

/// Recursively interpolates macro placeholders within a text string.
pub fn interpolate_placeholders(text: &str, ctx: &MacroContext) -> String {
    interpolate_internal(text, ctx, 0)
}

fn interpolate_internal(text: &str, ctx: &MacroContext, depth: usize) -> String {
    if text.is_empty() || !text.contains('{') {
        return text.to_string();
    }

    // 1. CP437 ASCII: {ascii:NNN}
    let mut res = parse_ascii_val(text);

    // 2. Adaptive glyph: {nf:...}
    if res.contains("{nf:") {
        let nf_re = Regex::new(r"\{nf:[^}]*\}").unwrap();
        let use_nerd = ctx.config.settings.use_nerd_fonts.unwrap_or(false);
        res = nf_re
            .replace_all(&res, |caps: &Captures| resolve_glyph(&caps[0], use_nerd))
            .to_string();
    }

    // 3. Environment variables: {env:VAR_NAME}
    if res.contains("{env:") {
        let env_re = Regex::new(r"\{env:([a-zA-Z0-9_]+)\}").unwrap();
        res = env_re
            .replace_all(&res, |caps: &Captures| {
                std::env::var(&caps[1]).unwrap_or_default()
            })
            .to_string();
    }

    // 4. Runtime & System Builtins
    let scripts_dir = ctx
        .config
        .settings
        .scripts_dir
        .clone()
        .unwrap_or_else(|| "{bashmenu_dir}/scripts".to_string());
    let templates_dir = ctx
        .config
        .settings
        .templates_dir
        .clone()
        .unwrap_or_else(|| "{bashmenu_dir}/templates".to_string());
    let cache_dir = ctx
        .config
        .settings
        .cache_dir
        .clone()
        .unwrap_or_else(|| "{home}/.cache/bashmenu".to_string());
    let notebooks_dir = format!("{}/notebooks", ctx.bashmenu_dir.display());

    let user_name = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let host_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let is_root = unsafe { libc::geteuid() } == 0;
    let user_mode = if is_root { "Root" } else { "User" };

    let times = get_local_time_strings();

    let mut direct_map: HashMap<&str, String> = HashMap::new();
    direct_map.insert("{home}", ctx.home_dir.to_string_lossy().to_string());
    direct_map.insert(
        "{bashmenu_dir}",
        ctx.bashmenu_dir.to_string_lossy().to_string(),
    );
    direct_map.insert("{scripts_dir}", scripts_dir.clone());
    direct_map.insert("{scripts}", scripts_dir);
    direct_map.insert("{templates_dir}", templates_dir.clone());
    direct_map.insert("{templates}", templates_dir);
    direct_map.insert("{cache_dir}", cache_dir.clone());
    direct_map.insert("{cache}", cache_dir);
    direct_map.insert("{notebooks_dir}", notebooks_dir.clone());
    direct_map.insert("{notebooks}", notebooks_dir);

    // Shell profiles and editor configs
    let bashrc = ctx.home_dir.join(".bashrc").to_string_lossy().to_string();
    let bash_aliases = ctx
        .home_dir
        .join(".bash_aliases")
        .to_string_lossy()
        .to_string();
    let zshrc = ctx.home_dir.join(".zshrc").to_string_lossy().to_string();
    let profile = resolve_profile_path(&ctx.home_dir)
        .to_string_lossy()
        .to_string();
    let bash_profile = resolve_bash_profile_path(&ctx.home_dir)
        .to_string_lossy()
        .to_string();
    let zprofile = ctx.home_dir.join(".zprofile").to_string_lossy().to_string();
    let shell_profile = resolve_shell_profile(&ctx.home_dir)
        .to_string_lossy()
        .to_string();
    let vimrc = ctx.home_dir.join(".vimrc").to_string_lossy().to_string();
    let nanorc = ctx.home_dir.join(".nanorc").to_string_lossy().to_string();

    direct_map.insert("{bashrc}", bashrc);
    direct_map.insert("{bash_aliases}", bash_aliases);
    direct_map.insert("{zshrc}", zshrc);
    direct_map.insert("{profile}", profile);
    direct_map.insert("{bash_profile}", bash_profile);
    direct_map.insert("{zprofile}", zprofile);
    direct_map.insert("{shell_profile}", shell_profile);
    direct_map.insert("{vimrc}", vimrc);
    direct_map.insert("{nanorc}", nanorc);

    // Cross-Platform Environment
    let prefix = std::env::var("PREFIX").unwrap_or_else(|_| "/usr".to_string());
    let termux_properties = ctx
        .home_dir
        .join(".termux/termux.properties")
        .to_string_lossy()
        .to_string();
    let termux_storage = ctx.home_dir.join("storage").to_string_lossy().to_string();
    let powershell_profile = std::env::var("PROFILE").unwrap_or_else(|_| {
        ctx.home_dir
            .join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
            .to_string_lossy()
            .to_string()
    });
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let userprofile =
        std::env::var("USERPROFILE").unwrap_or_else(|_| ctx.home_dir.to_string_lossy().to_string());

    direct_map.insert("{prefix}", prefix);
    direct_map.insert("{termux_properties}", termux_properties);
    direct_map.insert("{termux_storage}", termux_storage);
    direct_map.insert("{powershell_profile}", powershell_profile);
    direct_map.insert("{appdata}", appdata);
    direct_map.insert("{userprofile}", userprofile);

    // Divider
    let div_len = ctx.window_width.max(10);
    direct_map.insert("{divider}", "─".repeat(div_len));

    direct_map.insert("{user}", user_name.clone());
    direct_map.insert("{username}", user_name);
    direct_map.insert("{host}", host_name.clone());
    direct_map.insert("{hostname}", host_name);
    direct_map.insert("{user-mode}", user_mode.to_string());
    direct_map.insert("{version}", ctx.config.version.clone());
    direct_map.insert("{theme}", ctx.config.theme.clone());
    direct_map.insert("{app.theme}", ctx.config.theme.clone());

    direct_map.insert("{window_width}", ctx.window_width.to_string());
    direct_map.insert("{window_height}", ctx.window_height.to_string());
    direct_map.insert("{screen_width}", ctx.screen_width.to_string());

    direct_map.insert("{date_time_24}", times.date_time_24);
    direct_map.insert("{date_time_24_short}", times.date_time_24_short);
    direct_map.insert("{date_time_12}", times.date_time_12);
    direct_map.insert("{date_time_12_short}", times.date_time_12_short);
    direct_map.insert("{date}", times.date);
    direct_map.insert("{time_24}", times.time_24);
    direct_map.insert("{time_24_short}", times.time_24_short);
    direct_map.insert("{time_12}", times.time_12);
    direct_map.insert("{time_12_short}", times.time_12_short);

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
    if res.contains('{') {
        let dot_re = Regex::new(r"\{([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)+)\}").unwrap();
        let config_val = serde_json::to_value(ctx.config).unwrap_or(serde_json::Value::Null);

        res = dot_re
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
}
