//! Custom keyboard shortcuts (`shortcuts.yml`) schema, key normalization, and persistence.
//!
//! # Overview
//! This module provides domain models and parsing utilities for BashMenu's custom
//! keyboard shortcut subsystem. Shortcuts in `shortcuts.yml` allow users to bind
//! direct function keys (e.g. `F9`, `F10`), modifier combinations (e.g. `Ctrl+G`, `Alt+T`),
//! or two-stroke leader-key chords (e.g. `<leader> t`, `<leader> p`) to arbitrary menu actions.
//!
//! # Key Normalization & Validation
//! - [`normalize_key_spec`]: Normalizes inconsistent casing and syntax (such as `C-t`, `ctrl-t`,
//!   `<Ctrl+T>`, `leader+t`) into a canonical token string (e.g. `"<leader> t"` or `"ctrl+t"`).
//! - [`validate_key_spec`]: Guarantees that users cannot bind reserved navigation keys
//!   (`Up`, `Down`, `Enter`, `Esc`), critical function keys (`F1`..`F8`, `F11`), or unmodified single
//!   characters as direct shortcuts.
//! - Header Preservation: Serializing via [`save_shortcuts`] cleanly preserves the top-level
//!   [`SHORTCUTS_HEADER`] comment block.
//!
//! # Example API Usage
//! ```no_run
//! use bashmenu_core::{load_shortcuts, normalize_key_spec, validate_key_spec, ShortcutsConfig};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! // 1. Validate a user-typed shortcut string
//! let valid_key = validate_key_spec("<ctrl+space>", true).expect("Valid leader key");
//! assert_eq!(valid_key, "ctrl+space");
//!
//! // 2. Normalize chords
//! let chord = normalize_key_spec("leader+t");
//! assert_eq!(chord, "<leader> t");
//!
//! // 3. Load shortcuts configuration
//! let cfg = load_shortcuts("shortcuts.yml").unwrap_or_default();
//! println!("Leader key: {}", cfg.leader_key);
//! # Ok(())
//! # }
//! ```

use crate::config::get_user_config_dir;
use crate::error::{BashMenuError, Result};
use crate::macro_engine::resolve_dev_dir;
use crate::menu::{ExecutionMode, MenuItem, MenuItemType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Standard header preserved at the top of `shortcuts.yml` on every save.
pub const SHORTCUTS_HEADER: &str = r#"# ==============================================================================
# BashMenu Custom Keyboard Shortcuts (shortcuts.yml)
# ==============================================================================
# Configure custom direct key combos (e.g. "F9", "alt+t", "ctrl+g") and
# leader-key chords (e.g. "<leader> t", "<leader> g") to launch commands,
# scripts, editors, toast notifications, or interactive prompts.
#
# All target fields (action, command, script, file, message) support the full
# 6-tier BashMenu macro placeholder engine:
#   {bashmenu_dir}, {dev_dir}, {scripts_dir}, {home}, {env:VAR},
#   {param:prompt=...}, {file_picker:path=...}, {dir_picker}, {toast:3:"..."}
# ==============================================================================
"#;

fn default_leader_key() -> String {
    "ctrl+space".to_string()
}

fn default_leader_timeout_ms() -> u64 {
    1500
}

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// Top-level `shortcuts.yml` configuration model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    /// Global leader key for multi-key chord sequences (default: `"ctrl+space"`).
    #[serde(default = "default_leader_key")]
    pub leader_key: String,
    /// Timeout in milliseconds while waiting for the follow-up key after pressing `leader_key`.
    /// `0` waits indefinitely until a key or `Esc` is pressed.
    #[serde(default = "default_leader_timeout_ms")]
    pub leader_timeout_ms: u64,
    /// Whether to display the compact *which-key* popup overlay while in Leader Pending mode.
    #[serde(default = "default_true")]
    pub show_leader_hint: bool,
    /// Configured keyboard shortcuts.
    #[serde(default)]
    pub shortcuts: Vec<ShortcutEntry>,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            leader_key: default_leader_key(),
            leader_timeout_ms: default_leader_timeout_ms(),
            show_leader_hint: true,
            shortcuts: Vec::new(),
        }
    }
}

/// A single keyboard shortcut definition in `shortcuts.yml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutEntry {
    /// Key combo (`"F9"`, `"ctrl+g"`, `"alt+t"`) or leader chord (`"<leader> t"`).
    pub key: String,
    /// Human-readable description displayed in the Shortcut Manager and leader popup.
    pub description: String,
    /// Item type (`command`, `script`, `editor`, `toast`, `confirm`, `notebook`, `theme_selector`).
    #[serde(default)]
    pub r#type: MenuItemType,
    /// Primary target command, script, or file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Optional explicit command alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Optional explicit script path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Optional file path (for `editor` or `notebook` types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Optional modal or toast title override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional message body (for `toast` or `confirm` types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Optional prompt text for `{param}` modals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Optional working directory for execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_dir: Option<String>,
    /// Execution mode (`console`, `streaming`, `interactive`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<ExecutionMode>,
    /// Suppress console header and `"Press [ENTER] to return to menu..."` pause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<bool>,
    /// Run console commands in alternate screen buffer (`true` by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_buffer: Option<bool>,
    /// Floating modal size percentage (`20`..=`100`) for `streaming` and `interactive` modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_percent: Option<u16>,
    /// Tabstop width when `type: editor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabstop: Option<usize>,
    /// Toast duration in seconds when `type: toast`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    /// Whether this shortcut is currently active.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Additional custom properties forwarded to `MenuItem`.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

impl Default for ShortcutEntry {
    fn default() -> Self {
        Self {
            key: "<leader> x".to_string(),
            description: "New Shortcut".to_string(),
            r#type: MenuItemType::Command,
            action: Some(String::new()),
            command: None,
            script: None,
            file: None,
            title: None,
            message: None,
            prompt: None,
            start_dir: None,
            execution_mode: Some(ExecutionMode::Console),
            quiet: Some(false),
            alt_buffer: Some(true),
            window_percent: None,
            tabstop: None,
            timeout: None,
            enabled: true,
            extra: HashMap::new(),
        }
    }
}

impl ShortcutEntry {
    /// Converts this `ShortcutEntry` into a [`MenuItem`] for execution via `MenuAction::Execute`.
    pub fn to_menu_item(&self) -> MenuItem {
        let mut extra = self.extra.clone();
        if let Some(mode) = self.execution_mode {
            let mode_str = match mode {
                ExecutionMode::Console => "console",
                ExecutionMode::Streaming => "streaming",
                ExecutionMode::Interactive => "interactive",
            };
            extra.insert(
                "exec_mode".to_string(),
                serde_yml::Value::String(mode_str.to_string()),
            );
        }
        if let Some(pct) = self.window_percent {
            extra.insert(
                "window_percent".to_string(),
                serde_yml::Value::Number(pct.into()),
            );
        }
        if let Some(ref t) = self.title {
            if !t.trim().is_empty() {
                extra.insert("title".to_string(), serde_yml::Value::String(t.clone()));
            }
        } else if !self.description.trim().is_empty() {
            extra.insert(
                "title".to_string(),
                serde_yml::Value::String(self.description.clone()),
            );
        }
        if let Some(ref m) = self.message {
            if !m.trim().is_empty() {
                extra.insert("message".to_string(), serde_yml::Value::String(m.clone()));
            }
        }
        if let Some(secs) = self.timeout {
            extra.insert(
                "timeout".to_string(),
                serde_yml::Value::String(secs.to_string()),
            );
        }

        let primary_action = self
            .action
            .clone()
            .or_else(|| self.command.clone())
            .or_else(|| self.script.clone());

        MenuItem {
            label: self.description.clone(),
            r#type: self.r#type.clone(),
            icon: None,
            command: self.command.clone().or_else(|| primary_action.clone()),
            action: primary_action,
            script: self.script.clone(),
            file: self.file.clone(),
            prompt: self.prompt.clone(),
            start_dir: self.start_dir.clone(),
            interactive: None,
            quiet: self.quiet,
            alt_buffer: self.alt_buffer,
            tabstop: self.tabstop,
            on_yes: None,
            on_no: None,
            submenu: None,
            extra,
        }
    }

    /// If this shortcut uses the `<leader>` prefix, returns the normalized follow-up key spec.
    pub fn leader_chord_key(&self) -> Option<String> {
        let norm = normalize_key_spec(&self.key);
        norm.strip_prefix("<leader> ").map(|s| s.trim().to_string())
    }

    /// Returns the canonical normalized key string (e.g. `"<leader> t"`, `"F9"`, `"ctrl+g"`).
    pub fn normalized_key(&self) -> String {
        normalize_key_spec(&self.key)
    }

    /// Returns a concise target summary string for UI lists.
    pub fn target_summary(&self) -> &str {
        self.action
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| self.command.as_deref().filter(|s| !s.is_empty()))
            .or_else(|| self.script.as_deref().filter(|s| !s.is_empty()))
            .or_else(|| self.file.as_deref().filter(|s| !s.is_empty()))
            .or_else(|| self.message.as_deref().filter(|s| !s.is_empty()))
            .unwrap_or("(unconfigured)")
    }

    /// Returns a 5-char badge representing the item type / execution mode.
    pub fn mode_badge(&self) -> &'static str {
        match &self.r#type {
            MenuItemType::Editor => "[EDT]",
            MenuItemType::Confirm => "[CFM]",
            MenuItemType::Notebook => "[NBK]",
            MenuItemType::ThemeSelector => "[THM]",
            MenuItemType::Other(s) if s == "toast" => "[TST]",
            _ => match self.execution_mode.unwrap_or(ExecutionMode::Console) {
                ExecutionMode::Streaming => "[STR]",
                ExecutionMode::Interactive => "[INT]",
                ExecutionMode::Console => "[CON]",
            },
        }
    }

    /// Returns the effective execution mode for this entry.
    pub fn effective_execution_mode(&self) -> ExecutionMode {
        self.execution_mode.unwrap_or(ExecutionMode::Console)
    }
}

/// Normalizes a single key token (without `<leader>` prefix).
pub fn normalize_single_key_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Handle single character directly (preserving case for plain letters like 't' vs 'T')
    if trimmed.chars().count() == 1 {
        let ch = trimmed.chars().next().unwrap();
        if ch == ' ' {
            return "space".to_string();
        }
        return ch.to_string();
    }

    let lower = trimmed
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_ascii_lowercase();

    // Check function keys f1..f12 -> F1..F12
    if let Some(num_str) = lower.strip_prefix('f') {
        if let Ok(n) = num_str.parse::<u8>() {
            if (1..=24).contains(&n) {
                return format!("F{}", n);
            }
        }
    }

    // Normalize modifier combinations (ctrl+..., alt+..., shift+...)
    if lower.contains('+')
        || lower.contains('-')
            && (lower.starts_with("c-") || lower.starts_with("a-") || lower.starts_with("m-"))
    {
        let normalized_sep = if let Some(r) = lower.strip_prefix("c-") {
            format!("ctrl+{}", r)
        } else if let Some(r) = lower
            .strip_prefix("a-")
            .or_else(|| lower.strip_prefix("m-"))
        {
            format!("alt+{}", r)
        } else {
            lower.clone()
        };

        let mut has_ctrl = false;
        let mut has_alt = false;
        let mut has_shift = false;
        let mut key_part = String::new();

        for part in normalized_sep.split('+') {
            let p = part.trim().trim_start_matches('<').trim_end_matches('>');
            match p {
                "ctrl" | "control" | "ctl" | "c" => has_ctrl = true,
                "alt" | "meta" | "opt" | "option" | "a" | "m" => has_alt = true,
                "shift" | "shft" | "s" => has_shift = true,
                " " | "spc" | "space" => key_part = "space".to_string(),
                other => {
                    if let Some(num_str) = other.strip_prefix('f') {
                        if let Ok(n) = num_str.parse::<u8>() {
                            if (1..=24).contains(&n) {
                                key_part = format!("F{}", n);
                                continue;
                            }
                        }
                    }
                    key_part = other.to_string();
                }
            }
        }

        let mut prefix = Vec::new();
        if has_ctrl {
            prefix.push("ctrl");
        }
        if has_alt {
            prefix.push("alt");
        }
        if has_shift {
            prefix.push("shift");
        }
        if !key_part.is_empty() {
            prefix.push(&key_part);
        }
        return prefix.join("+");
    }

    match lower.as_str() {
        "spc" | "space" | "spacebar" => "space".to_string(),
        "esc" | "escape" => "esc".to_string(),
        "ret" | "return" | "enter" => "enter".to_string(),
        "tab" => "tab".to_string(),
        "bs" | "backspace" => "backspace".to_string(),
        _ => trimmed.to_string(),
    }
}

/// Normalizes a full shortcut key specification (supporting `<leader> <key>` or direct combos).
pub fn normalize_key_spec(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("<leader>") {
        let orig_rest = &trimmed["<leader>".len()..].trim();
        let norm_rest = normalize_single_key_token(if orig_rest.is_empty() {
            rest.trim()
        } else {
            orig_rest
        });
        if norm_rest.is_empty() {
            return "<leader>".to_string();
        }
        return format!("<leader> {}", norm_rest);
    }
    if let Some(rest) = lower.strip_prefix("leader+") {
        let orig_rest = &trimmed["leader+".len()..].trim();
        let norm_rest = normalize_single_key_token(if orig_rest.is_empty() {
            rest.trim()
        } else {
            orig_rest
        });
        if norm_rest.is_empty() {
            return "<leader>".to_string();
        }
        return format!("<leader> {}", norm_rest);
    }
    if let Some(rest) = lower.strip_prefix("leader ") {
        let orig_rest = &trimmed["leader ".len()..].trim();
        let norm_rest = normalize_single_key_token(if orig_rest.is_empty() {
            rest.trim()
        } else {
            orig_rest
        });
        if norm_rest.is_empty() {
            return "<leader>".to_string();
        }
        return format!("<leader> {}", norm_rest);
    }

    normalize_single_key_token(trimmed)
}

/// Reserved navigation and system keys that cannot be bound as direct single-stroke shortcuts.
const RESERVED_DIRECT_KEYS: &[&str] = &[
    "up",
    "down",
    "left",
    "right",
    "enter",
    "esc",
    "backspace",
    "tab",
    "home",
    "end",
    "pageup",
    "pagedown",
    "space",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F11",
];

/// Validates a shortcut key specification or leader key definition.
/// Returns `Ok(normalized_key)` if valid, or `Err(reason)` if invalid or reserved.
pub fn validate_key_spec(raw: &str, is_leader_def: bool) -> std::result::Result<String, String> {
    let norm = normalize_key_spec(raw);
    if norm.is_empty() {
        return Err("Key binding cannot be empty.".to_string());
    }

    if is_leader_def {
        if norm.starts_with("<leader>") {
            return Err("Leader key definition cannot reference <leader> itself.".to_string());
        }
        if norm.chars().count() == 1 {
            return Err(
                "Leader key must use a modifier (e.g. 'ctrl+space', 'ctrl+b', 'alt+space') so it does not block menu item keys."
                    .to_string(),
            );
        }
        if RESERVED_DIRECT_KEYS
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&norm))
        {
            return Err(format!("'{}' is a reserved navigation/function key.", norm));
        }
        return Ok(norm);
    }

    // Check leader chord: `<leader> <key>`
    if let Some(follow) = norm.strip_prefix("<leader>") {
        let follow = follow.trim();
        if follow.is_empty() {
            return Err(
                "Leader chord must specify a follow-up key (e.g. '<leader> t').".to_string(),
            );
        }
        if follow.eq_ignore_ascii_case("esc") {
            return Err(
                "'<leader> esc' is reserved for cancelling Leader Pending mode.".to_string(),
            );
        }
        return Ok(format!("<leader> {}", follow));
    }

    // Direct single-stroke shortcut validation
    if norm.chars().count() == 1 {
        return Err(format!(
            "Unmodified single key '{}' conflicts with main menu item shortcuts. Use '<leader> {}', 'alt+{}', or 'ctrl+{}'.",
            norm, norm, norm, norm
        ));
    }

    if RESERVED_DIRECT_KEYS
        .iter()
        .any(|k| k.eq_ignore_ascii_case(&norm))
    {
        return Err(format!(
            "'{}' is reserved for core BashMenu navigation or built-in functions (use F9, F10, F12, ctrl+<key>, alt+<key>, or '<leader> <key>').",
            norm
        ));
    }

    // Must be a function key (F9, F10, F12) or modified key (ctrl+..., alt+...)
    let is_fn = norm.starts_with('F') && norm[1..].parse::<u8>().is_ok();
    let is_mod = norm.starts_with("ctrl+") || norm.starts_with("alt+");
    if !is_fn && !is_mod {
        return Err(format!(
            "Direct shortcut '{}' must use 'ctrl+<key>', 'alt+<key>', 'F9'/'F10'/'F12', or '<leader> <key>'.",
            norm
        ));
    }

    Ok(norm)
}

/// Resolves the active `shortcuts.yml` path following the Single Source of Truth hierarchy.
pub fn resolve_shortcuts_path(local_override: bool) -> PathBuf {
    let filename = Path::new("shortcuts.yml");
    if local_override && filename.exists() {
        return filename.to_path_buf();
    }
    if let Ok(bm_dir) = std::env::var("BASHMENU_DIR") {
        let p = PathBuf::from(bm_dir).join(filename);
        if p.exists() {
            return p;
        }
    }
    let user_cfg = get_user_config_dir();
    let user_path = user_cfg.join(filename);
    if user_path.exists() {
        return user_path;
    }
    if filename.exists() {
        return filename.to_path_buf();
    }
    let home_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));
    let dev_dir = resolve_dev_dir(&home_dir, &user_cfg);
    let dev_path = dev_dir.join(filename);
    if dev_path.exists() {
        return dev_path;
    }
    user_path
}

/// Loads and parses `shortcuts.yml` from the specified path.
pub fn load_shortcuts<P: AsRef<Path>>(path: P) -> Result<ShortcutsConfig> {
    let p = path.as_ref();
    let content = fs::read_to_string(p).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    if content.trim().is_empty() {
        return Ok(ShortcutsConfig::default());
    }
    let mut cfg: ShortcutsConfig = serde_yml::from_str(&content)?;
    cfg.leader_key = normalize_key_spec(&cfg.leader_key);
    for entry in &mut cfg.shortcuts {
        entry.key = normalize_key_spec(&entry.key);
    }
    Ok(cfg)
}

/// Serializes and saves `shortcuts.yml` while preserving the top-level `SHORTCUTS_HEADER` documentation.
pub fn save_shortcuts<P: AsRef<Path>>(path: P, config: &ShortcutsConfig) -> Result<()> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let yaml_body = serde_yml::to_string(config)?;
    let full_content = format!("{}\n{}", SHORTCUTS_HEADER.trim_end(), yaml_body);
    fs::write(p, full_content).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_and_validate_key_specs() {
        assert_eq!(normalize_key_spec("Ctrl+Space"), "ctrl+space");
        assert_eq!(normalize_key_spec("<Ctrl+Space>"), "ctrl+space");
        assert_eq!(normalize_key_spec("<leader> t"), "<leader> t");
        assert_eq!(normalize_key_spec("leader+T"), "<leader> T");
        assert_eq!(normalize_key_spec("f9"), "F9");
        assert_eq!(normalize_key_spec("Alt+G"), "alt+g");

        assert!(validate_key_spec("ctrl+space", true).is_ok());
        assert!(validate_key_spec("t", true).is_err());

        assert!(validate_key_spec("<leader> t", false).is_ok());
        assert!(validate_key_spec("<leader> T", false).is_ok());
        assert!(validate_key_spec("<leader> 1", false).is_ok());
        assert!(validate_key_spec("F9", false).is_ok());
        assert!(validate_key_spec("ctrl+g", false).is_ok());
        assert!(validate_key_spec("alt+1", false).is_ok());

        // Single unmodified char or F1..F8 / F11 must be rejected as direct shortcuts
        assert!(validate_key_spec("t", false).is_err());
        assert!(validate_key_spec("1", false).is_err());
        assert!(validate_key_spec("F1", false).is_err());
        assert!(validate_key_spec("F11", false).is_err());
    }

    #[test]
    fn test_shortcut_entry_to_menu_item_conversion() {
        let entry = ShortcutEntry {
            key: "<leader> g".to_string(),
            description: "Git Status Summary".to_string(),
            r#type: MenuItemType::Command,
            action: Some("git -C {dev_dir} status -sb".to_string()),
            execution_mode: Some(ExecutionMode::Streaming),
            window_percent: Some(70),
            quiet: Some(true),
            alt_buffer: Some(true),
            ..Default::default()
        };
        let item = entry.to_menu_item();
        assert_eq!(item.label, "Git Status Summary");
        assert_eq!(item.execution_mode(), ExecutionMode::Streaming);
        assert_eq!(item.window_percent(), 70);
        assert_eq!(item.quiet, Some(true));
        assert_eq!(item.title(), Some("Git Status Summary"));
    }
}
