//! Plugin execution engine, background caching, and table/list layout formatter.

use crate::config::BashMenuConfig;
use crate::macro_engine::{interpolate_placeholders, MacroContext};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

fn default_sleep() -> u64 {
    300
}

/// Definition of an individual plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDef {
    /// Command or script to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,

    /// Refresh interval in seconds (0 = run on every cycle)
    #[serde(default = "default_sleep")]
    pub sleep: u64,

    /// Text or `{divider}` prepended before plugin lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretext: Option<String>,

    /// Text or `{divider}` appended after plugin lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posttext: Option<String>,

    /// Whether this plugin spans the full row in a table layout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standalone: Option<bool>,

    /// Column span specification (e.g. "full", 1, 2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<serde_yml::Value>,
}

impl Default for PluginDef {
    fn default() -> Self {
        Self {
            script: None,
            sleep: default_sleep(),
            pretext: None,
            posttext: None,
            standalone: None,
            span: None,
        }
    }
}

/// Layout configuration for the plugins section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLayout {
    /// Layout type: "table" or "list"
    pub layout_type: String,
    /// Width specification (e.g. "100%")
    pub width: String,
    /// Maximum entries/columns per row in a table layout
    pub entries: usize,
    /// Optional table headers
    pub headers: Vec<String>,
    /// Divider configuration: "true", "screen", "window", etc.
    pub divider: Option<String>,
}

impl Default for PluginLayout {
    fn default() -> Self {
        Self {
            layout_type: "list".to_string(),
            width: "100%".to_string(),
            entries: 2,
            headers: Vec::new(),
            divider: None,
        }
    }
}

/// Normalized plugin configuration extracted from `settings.plugins`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PluginConfig {
    pub layout: PluginLayout,
    pub plugins: HashMap<String, PluginDef>,
    pub order: Vec<String>,
}

/// Cache entry for a plugin's stdout lines.
#[derive(Debug, Clone)]
pub struct PluginCacheEntry {
    pub fetched_at: Instant,
    pub lines: Vec<String>,
}

/// A formatted display row ready for TUI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDisplayRow {
    pub text: String,
    pub is_divider: bool,
}

/// Thread-safe plugin execution engine and background runner.
#[derive(Clone)]
pub struct PluginEngine {
    cache: Arc<RwLock<HashMap<String, PluginCacheEntry>>>,
    in_flight: Arc<Mutex<HashSet<String>>>,
    update_sender: Option<mpsc::Sender<String>>,
}

impl Default for PluginEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginEngine {
    /// Create a new plugin engine.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            update_sender: None,
        }
    }

    /// Attach an MPSC notification sender that receives plugin names when updated.
    pub fn with_sender(mut self, sender: mpsc::Sender<String>) -> Self {
        self.update_sender = Some(sender);
        self
    }

    /// Clear all cached outputs.
    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.write() {
            c.clear();
        }
    }

    /// Retrieve cached output lines for a plugin if available.
    pub fn get_cached(&self, name: &str) -> Option<Vec<String>> {
        self.cache.read().ok()?.get(name).map(|e| e.lines.clone())
    }

    /// Manually store cache lines for testing or initial state.
    pub fn set_cached(&self, name: &str, lines: Vec<String>) {
        if let Ok(mut c) = self.cache.write() {
            c.insert(
                name.to_string(),
                PluginCacheEntry {
                    fetched_at: Instant::now(),
                    lines,
                },
            );
        }
    }

    /// Check if a plugin is currently executing in a background thread.
    pub fn is_in_flight(&self, name: &str) -> bool {
        self.in_flight
            .lock()
            .map(|set| set.contains(name))
            .unwrap_or(false)
    }

    /// Parse `settings.plugins` from `BashMenuConfig` into structured `PluginConfig`.
    pub fn parse_plugins(config: &BashMenuConfig) -> PluginConfig {
        let mut result = PluginConfig::default();
        let plugins_val = match &config.settings.plugins {
            Some(val) => val,
            None => return result,
        };

        let map = match plugins_val.as_mapping() {
            Some(m) => m,
            None => return result,
        };

        // Extract layout fields if present
        if let Some(t) = map.get(serde_yml::Value::String("type".to_string())) {
            if let Some(s) = t.as_str() {
                result.layout.layout_type = s.to_string();
            }
        }
        if let Some(w) = map.get(serde_yml::Value::String("width".to_string())) {
            if let Some(s) = w.as_str() {
                result.layout.width = s.to_string();
            }
        }
        if let Some(e) = map.get(serde_yml::Value::String("entries".to_string())) {
            if let Some(n) = e.as_u64() {
                result.layout.entries = n.max(1) as usize;
            }
        } else if let Some(e) = map.get(serde_yml::Value::String("columns".to_string())) {
            if let Some(n) = e.as_u64() {
                result.layout.entries = n.max(1) as usize;
            }
        }
        if let Some(d) = map.get(serde_yml::Value::String("divider".to_string())) {
            if let Some(b) = d.as_bool() {
                if b {
                    result.layout.divider = Some("true".to_string());
                }
            } else if let Some(s) = d.as_str() {
                result.layout.divider = Some(s.to_string());
            }
        }
        if let Some(h) = map.get(serde_yml::Value::String("headers".to_string())) {
            if let Some(seq) = h.as_sequence() {
                result.layout.headers = seq
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }

        // Check if layout was defined as nested dictionary `layout: { ... }`
        if let Some(nested_layout) = map.get(serde_yml::Value::String("layout".to_string())) {
            if let Some(nl_map) = nested_layout.as_mapping() {
                if let Some(t) = nl_map.get(serde_yml::Value::String("type".to_string())) {
                    if let Some(s) = t.as_str() {
                        result.layout.layout_type = s.to_string();
                    }
                }
                if let Some(w) = nl_map.get(serde_yml::Value::String("width".to_string())) {
                    if let Some(s) = w.as_str() {
                        result.layout.width = s.to_string();
                    }
                }
                if let Some(e) = nl_map.get(serde_yml::Value::String("entries".to_string())) {
                    if let Some(n) = e.as_u64() {
                        result.layout.entries = n.max(1) as usize;
                    }
                }
                if let Some(d) = nl_map.get(serde_yml::Value::String("divider".to_string())) {
                    if let Some(b) = d.as_bool() {
                        if b {
                            result.layout.divider = Some("true".to_string());
                        }
                    } else if let Some(s) = d.as_str() {
                        result.layout.divider = Some(s.to_string());
                    }
                }
                if let Some(h) = nl_map.get(serde_yml::Value::String("headers".to_string())) {
                    if let Some(seq) = h.as_sequence() {
                        result.layout.headers = seq
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                }
            }
        }

        // Iterate through entries and extract plugins
        let reserved_keys = [
            "type",
            "width",
            "entries",
            "columns",
            "headers",
            "divider",
            "top_divider",
            "pretext",
            "posttext",
            "layout",
            "_layout",
            "rows",
        ];

        for (k, v) in map {
            if let Some(key_str) = k.as_str() {
                if reserved_keys.contains(&key_str) {
                    continue;
                }
                if let Ok(plugin_def) = serde_yml::from_value::<PluginDef>(v.clone()) {
                    result.order.push(key_str.to_string());
                    result.plugins.insert(key_str.to_string(), plugin_def);
                }
            }
        }

        result
    }

    /// Sanitize raw subprocess output lines:
    /// - Strips null bytes `\x00`
    /// - Expands tabs `\t` to 4 spaces
    /// - Removes variation selectors `\u{fe0f}` and `\u{fe0e}`
    /// - Strips trailing carriage returns
    pub fn sanitize_lines(raw: &str) -> Vec<String> {
        raw.lines()
            .map(|l| {
                l.replace('\0', "")
                    .replace('\t', "    ")
                    .replace(['\u{fe0f}', '\u{fe0e}'], "")
                    .trim_end_matches('\r')
                    .to_string()
            })
            .filter(|l| !l.trim().is_empty())
            .collect()
    }

    /// Synchronously execute a script with a 5-second timeout and return sanitized lines.
    pub fn fetch_plugin_sync(
        script_cmd: &str,
        scripts_dir: Option<&Path>,
        avail_cols: usize,
    ) -> Vec<String> {
        let trimmed = script_cmd.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        // Check if script is an existing file or relative to scripts_dir
        let script_path = PathBuf::from(trimmed);
        let exec_path = if script_path.is_file() {
            Some(script_path)
        } else if let Some(dir) = scripts_dir {
            let candidate = dir.join(trimmed);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };

        let mut cmd = if let Some(file) = exec_path {
            let mut c = Command::new(file);
            c.env("COLUMNS", avail_cols.to_string());
            c
        } else {
            let mut c = Command::new("bash");
            c.arg("-c").arg(trimmed);
            c.env("COLUMNS", avail_cols.to_string());
            c
        };

        cmd.stdout(Stdio::piped()).stderr(Stdio::null());

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        // Run process wait in a background thread with 5-second timeout
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let res = child.wait_with_output();
            let _ = tx.send(res);
        });

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(output)) if output.status.success() => {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                Self::sanitize_lines(&stdout_str)
            }
            _ => Vec::new(),
        }
    }

    /// Trigger background update for plugins configured in `BashMenuConfig`.
    ///
    /// If `force` is true or if elapsed time > `plugin.sleep`, a background
    /// thread is spawned to run the command and update cache.
    pub fn trigger_refresh(&self, config: &BashMenuConfig, force: bool) {
        let parsed = Self::parse_plugins(config);
        let now = Instant::now();
        let ctx = MacroContext::new(config);
        let scripts_dir_str = config
            .settings
            .scripts_dir
            .as_deref()
            .unwrap_or("{bashmenu_dir}/scripts");
        let expanded_scripts_dir = interpolate_placeholders(scripts_dir_str, &ctx);
        let scripts_dir = PathBuf::from(expanded_scripts_dir);

        for (name, plugin) in parsed.plugins {
            let script_raw = match &plugin.script {
                Some(s) if !s.trim().is_empty() => s,
                _ => continue,
            };

            let script_cmd = interpolate_placeholders(script_raw, &ctx);

            // Check cache validity
            let needs_fetch = if force {
                true
            } else if let Ok(c) = self.cache.read() {
                if let Some(entry) = c.get(&name) {
                    if plugin.sleep == 0 {
                        true
                    } else {
                        now.duration_since(entry.fetched_at).as_secs() >= plugin.sleep
                    }
                } else {
                    true
                }
            } else {
                true
            };

            if !needs_fetch {
                continue;
            }

            // Check if already in flight
            {
                let mut flight = match self.in_flight.lock() {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if flight.contains(&name) {
                    continue;
                }
                flight.insert(name.clone());
            }

            // Spawn background fetch thread
            let engine_cache = Arc::clone(&self.cache);
            let engine_flight = Arc::clone(&self.in_flight);
            let name_clone = name.clone();
            let scripts_dir_clone = scripts_dir.clone();
            let sender_clone = self.update_sender.clone();

            std::thread::spawn(move || {
                let lines = Self::fetch_plugin_sync(&script_cmd, Some(&scripts_dir_clone), 74);

                if let Ok(mut c) = engine_cache.write() {
                    c.insert(
                        name_clone.clone(),
                        PluginCacheEntry {
                            fetched_at: Instant::now(),
                            lines,
                        },
                    );
                }

                if let Ok(mut flight) = engine_flight.lock() {
                    flight.remove(&name_clone);
                }

                if let Some(tx) = sender_clone {
                    let _ = tx.send(name_clone);
                }
            });
        }
    }

    /// Format display rows for active plugins adhering to layout constraints,
    /// standalone flags, table columns, and dividers.
    ///
    /// Clamps output to at most 10 lines.
    pub fn format_display_rows(
        &self,
        config: &BashMenuConfig,
        avail_w: usize,
        divider_char: char,
    ) -> Vec<PluginDisplayRow> {
        let parsed = Self::parse_plugins(config);
        let mut rows: Vec<PluginDisplayRow> = Vec::new();

        if parsed.plugins.is_empty() {
            return rows;
        }

        let ctx = MacroContext::new(config);

        // Top divider if configured
        if parsed.layout.divider.is_some() {
            rows.push(PluginDisplayRow {
                text: divider_char.to_string().repeat(avail_w),
                is_divider: true,
            });
        }

        let is_table = parsed.layout.layout_type == "table";

        if is_table {
            // Group plugins into rows according to `entries` and `standalone` flags
            let max_cols = parsed.layout.entries.max(1);
            let mut grouped_rows: Vec<Vec<String>> = Vec::new();
            let mut current_row: Vec<String> = Vec::new();

            for name in &parsed.order {
                let plugin = match parsed.plugins.get(name) {
                    Some(p) => p,
                    None => continue,
                };

                let is_standalone = plugin.standalone.unwrap_or(false)
                    || match &plugin.span {
                        Some(serde_yml::Value::String(s)) => s == "full",
                        Some(serde_yml::Value::Number(n)) => n.as_u64() == Some(1),
                        _ => false,
                    };

                if is_standalone {
                    if !current_row.is_empty() {
                        grouped_rows.push(current_row);
                        current_row = Vec::new();
                    }
                    grouped_rows.push(vec![name.clone()]);
                } else {
                    current_row.push(name.clone());
                    if current_row.len() >= max_cols {
                        grouped_rows.push(current_row);
                        current_row = Vec::new();
                    }
                }
            }
            if !current_row.is_empty() {
                grouped_rows.push(current_row);
            }

            // Render each grouped row
            for group in grouped_rows {
                if rows.len() >= 10 {
                    break;
                }

                if group.len() == 1 {
                    let name = &group[0];
                    let plugin = match parsed.plugins.get(name) {
                        Some(p) => p,
                        None => continue,
                    };
                    let is_standalone = plugin.standalone.unwrap_or(false);

                    if is_standalone {
                        let cached_lines = self.get_cached(name).unwrap_or_default();
                        let mut full_lines = Vec::new();
                        if let Some(pre) = &plugin.pretext {
                            full_lines.push(pre.clone());
                        }
                        full_lines.extend(cached_lines);
                        if let Some(post) = &plugin.posttext {
                            full_lines.push(post.clone());
                        }

                        for line in full_lines {
                            if rows.len() >= 10 {
                                break;
                            }
                            let is_div = line.contains("{divider}");
                            let text = if is_div {
                                divider_char.to_string().repeat(avail_w)
                            } else {
                                interpolate_placeholders(&line, &ctx)
                            };
                            rows.push(PluginDisplayRow {
                                text,
                                is_divider: is_div,
                            });
                        }
                        continue;
                    }
                }

                // Multi-column row: get lines for each cell in group
                let num_cols = group.len();
                let spacing = 2; // two spaces between columns
                let total_spacing = spacing * (num_cols - 1);
                let col_w = if avail_w > total_spacing {
                    (avail_w - total_spacing) / num_cols
                } else {
                    avail_w / num_cols
                };

                let mut col_lines: Vec<Vec<String>> = Vec::new();
                let mut max_cell_lines = 0;

                for name in &group {
                    let plugin = parsed.plugins.get(name);
                    let cached = self.get_cached(name).unwrap_or_default();
                    let mut cell = Vec::new();
                    if let Some(p) = plugin {
                        if let Some(pre) = &p.pretext {
                            cell.push(pre.clone());
                        }
                        cell.extend(cached);
                        if let Some(post) = &p.posttext {
                            cell.push(post.clone());
                        }
                    } else {
                        cell.extend(cached);
                    }
                    if cell.len() > max_cell_lines {
                        max_cell_lines = cell.len();
                    }
                    col_lines.push(cell);
                }

                if max_cell_lines == 0 {
                    continue;
                }

                // Render each line of the multi-column row
                for line_idx in 0..max_cell_lines {
                    if rows.len() >= 10 {
                        break;
                    }

                    let mut row_str = String::new();
                    for (c_idx, cell) in col_lines.iter().enumerate() {
                        if c_idx > 0 {
                            row_str.push_str("  ");
                        }
                        let raw = cell.get(line_idx).map(|s| s.as_str()).unwrap_or("");
                        let interpolated = interpolate_placeholders(raw, &ctx);
                        let w = UnicodeWidthStr::width(interpolated.as_str());

                        if w > col_w {
                            // Truncate to col_w
                            let mut truncated = String::new();
                            let mut cur_w = 0;
                            for ch in interpolated.chars() {
                                let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                                if cur_w + cw > col_w {
                                    break;
                                }
                                truncated.push(ch);
                                cur_w += cw;
                            }
                            row_str.push_str(&truncated);
                            if cur_w < col_w {
                                row_str.push_str(&" ".repeat(col_w - cur_w));
                            }
                        } else {
                            row_str.push_str(&interpolated);
                            row_str.push_str(&" ".repeat(col_w - w));
                        }
                    }

                    rows.push(PluginDisplayRow {
                        text: row_str,
                        is_divider: false,
                    });
                }
            }
        } else {
            // Sequential list layout
            for name in &parsed.order {
                if rows.len() >= 10 {
                    break;
                }
                let plugin = match parsed.plugins.get(name) {
                    Some(p) => p,
                    None => continue,
                };
                let cached = self.get_cached(name).unwrap_or_default();
                let mut lines = Vec::new();
                if let Some(pre) = &plugin.pretext {
                    lines.push(pre.clone());
                }
                lines.extend(cached);
                if let Some(post) = &plugin.posttext {
                    lines.push(post.clone());
                }

                for line in lines {
                    if rows.len() >= 10 {
                        break;
                    }
                    let is_div = line.contains("{divider}");
                    let text = if is_div {
                        divider_char.to_string().repeat(avail_w)
                    } else {
                        interpolate_placeholders(&line, &ctx)
                    };
                    rows.push(PluginDisplayRow {
                        text,
                        is_divider: is_div,
                    });
                }
            }
        }

        if rows.len() > 10 {
            rows.truncate(10);
        }

        rows
    }
}
