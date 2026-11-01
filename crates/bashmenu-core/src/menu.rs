//! Menu definitions, schema normalization, and serialization for `bashmenu.mnu`.

use crate::error::{BashMenuError, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Supported menu item types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum MenuItemType {
    /// Visual horizontal divider rule
    #[serde(rename = "{divider}")]
    Divider,
    /// Executes a shell command
    #[serde(rename = "command")]
    #[default]
    Command,
    /// Opens a nested submenu
    #[serde(rename = "submenu")]
    Submenu,
    /// Opens the built-in or external text editor
    #[serde(rename = "editor")]
    Editor,
    /// Executes a script
    #[serde(rename = "script")]
    Script,
    /// Modifies a bashmenu configuration setting
    #[serde(rename = "config")]
    Config,
    /// Toggles a boolean configuration value
    #[serde(rename = "toggle")]
    Toggle,
    /// Selects an option from a radio list
    #[serde(rename = "radio")]
    Radio,
    /// Launches Yes/No confirmation dialog box
    #[serde(rename = "confirm")]
    Confirm,
    /// Executes Python script
    #[serde(rename = "python")]
    Python,
    /// Opens the interactive theme selector modal
    #[serde(rename = "theme_selector")]
    ThemeSelector,
    /// Exits BashMenu
    #[serde(rename = "exit")]
    Exit,
    /// Opens a notebook
    #[serde(rename = "notebook")]
    Notebook,
    /// Notebook note item
    #[serde(rename = "notebook_note")]
    NotebookNote,
    /// Empty notebook notice
    #[serde(rename = "notebook_empty")]
    NotebookEmpty,
    /// Returns to parent menu
    #[serde(rename = "back")]
    Back,
    /// Custom user-defined type
    #[serde(untagged)]
    Other(String),
}

/// Action payload for confirm dialog choices (can be a plain command string or structured action object).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfirmAction {
    /// Plain command or script string (e.g. `reboot` or `{scripts_dir}/ping.sh`)
    Simple(String),
    /// Structured action with specific type, action, title, and message
    Structured(ConfirmActionDetails),
}

impl ConfirmAction {
    pub fn as_str(&self) -> &str {
        match self {
            ConfirmAction::Simple(s) => s.as_str(),
            ConfirmAction::Structured(d) => d
                .action
                .as_deref()
                .or(d.command.as_deref())
                .or(d.script.as_deref())
                .or(d.message.as_deref())
                .unwrap_or_default(),
        }
    }
}

impl From<&str> for ConfirmAction {
    fn from(s: &str) -> Self {
        ConfirmAction::Simple(s.to_string())
    }
}

impl From<String> for ConfirmAction {
    fn from(s: String) -> Self {
        ConfirmAction::Simple(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConfirmActionDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

/// A normalized menu item definition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MenuItem {
    /// Label or title displayed in the menu
    pub label: String,
    /// Item type directive
    pub r#type: MenuItemType,
    /// Nerd Font or emoji icon prefix
    pub icon: Option<String>,
    /// Shell command or action to execute
    pub command: Option<String>,
    /// Alias for command/action
    pub action: Option<String>,
    /// Script path
    pub script: Option<String>,
    /// File path for editors
    pub file: Option<String>,
    /// Input prompt
    pub prompt: Option<String>,
    /// Working directory for execution
    pub start_dir: Option<String>,
    /// Whether command requires interactive terminal session
    pub interactive: Option<bool>,
    /// Suppress stdout
    pub quiet: Option<bool>,
    /// Use alternate screen buffer
    pub alt_buffer: Option<bool>,
    /// Editor tabstop width
    pub tabstop: Option<usize>,
    /// Action to execute when Yes is selected (for confirm dialogs)
    pub on_yes: Option<ConfirmAction>,
    /// Action to execute when No is selected (for confirm dialogs)
    pub on_no: Option<ConfirmAction>,
    /// Nested submenu container
    pub submenu: Option<Submenu>,
    /// Additional custom keys
    pub extra: HashMap<String, serde_yml::Value>,
}

impl MenuItem {
    /// Creates a divider item
    pub fn divider() -> Self {
        Self {
            label: "{divider}".to_string(),
            r#type: MenuItemType::Divider,
            ..Default::default()
        }
    }

    /// Returns the on_yes action for confirm dialogs
    pub fn on_yes(&self) -> Option<&str> {
        self.on_yes
            .as_ref()
            .map(|a| a.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| self.extra.get("on_yes").and_then(|v| v.as_str()))
    }

    /// Returns the on_yes action object for confirm dialogs
    pub fn on_yes_action(&self) -> Option<&ConfirmAction> {
        self.on_yes.as_ref()
    }

    /// Returns the on_no action for confirm dialogs
    pub fn on_no(&self) -> Option<&str> {
        self.on_no
            .as_ref()
            .map(|a| a.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| self.extra.get("on_no").and_then(|v| v.as_str()))
    }

    /// Returns the on_no action object for confirm dialogs
    pub fn on_no_action(&self) -> Option<&ConfirmAction> {
        self.on_no.as_ref()
    }

    /// Whether this item represents a divider
    pub fn is_divider(&self) -> bool {
        self.r#type == MenuItemType::Divider || self.label == "{divider}"
    }

    /// Configuration key associated with this item (e.g. `settings.use_nerd_fonts`)
    pub fn key(&self) -> Option<&str> {
        self.extra.get("key").and_then(|v| v.as_str())
    }

    /// Display title for dialogs or inspector
    pub fn title(&self) -> Option<&str> {
        self.extra
            .get("title")
            .and_then(|v| v.as_str())
            .or(self.prompt.as_deref())
    }

    /// Prompt or descriptive message for dialogs
    pub fn message(&self) -> Option<&str> {
        self.extra
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| self.extra.get("text").and_then(|v| v.as_str()))
            .or(self.prompt.as_deref())
    }

    /// Picker mode if specified ("file" or "dir")
    pub fn picker(&self) -> Option<&str> {
        self.extra.get("picker").and_then(|v| v.as_str())
    }

    /// Whether input prompts for this item should mask characters with `*`
    pub fn masked(&self) -> bool {
        match self.extra.get("masked") {
            Some(serde_yml::Value::Bool(b)) => *b,
            Some(serde_yml::Value::String(s)) => {
                matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
            }
            Some(serde_yml::Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
            _ => false,
        }
    }

    /// Returns true if any executable or target payload field contains `{param}`
    pub fn has_param_placeholder(&self) -> bool {
        [
            self.command.as_deref(),
            self.action.as_deref(),
            self.script.as_deref(),
            self.file.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|s| s.contains("{param}"))
    }

    /// Substitutes `{param}` with `value` across all executable and target payload fields
    pub fn apply_param_placeholder(&mut self, value: &str) {
        if let Some(ref mut s) = self.command {
            if s.contains("{param}") {
                *s = s.replace("{param}", value);
            }
        }
        if let Some(ref mut s) = self.action {
            if s.contains("{param}") {
                *s = s.replace("{param}", value);
            }
        }
        if let Some(ref mut s) = self.script {
            if s.contains("{param}") {
                *s = s.replace("{param}", value);
            }
        }
        if let Some(ref mut s) = self.file {
            if s.contains("{param}") {
                *s = s.replace("{param}", value);
            }
        }
    }

    /// Modal header title for a `{param}` input dialog
    pub fn param_modal_title(&self) -> &str {
        if let Some(t) = self
            .extra
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
        {
            return t;
        }
        let trimmed_label = self.label.trim();
        if !trimmed_label.is_empty() {
            return trimmed_label;
        }
        "Parameter Input"
    }

    /// Modal prompt message for a `{param}` input dialog
    pub fn param_modal_prompt(&self) -> &str {
        self.prompt
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                self.extra
                    .get("message")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
            })
            .or_else(|| {
                self.extra
                    .get("text")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or("Enter parameter:")
    }

    /// Radio selection options as (value, label) pairs
    pub fn options(&self) -> Vec<(String, String)> {
        let mut list = Vec::new();
        if let Some(serde_yml::Value::Sequence(seq)) = self.extra.get("options") {
            for item in seq {
                match item {
                    serde_yml::Value::Mapping(map) => {
                        let val = map
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let lbl = map
                            .get("label")
                            .or_else(|| map.get("title"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&val)
                            .to_string();
                        list.push((val, lbl));
                    }
                    serde_yml::Value::Sequence(pair) if pair.len() >= 2 => {
                        let val = pair[0].as_str().unwrap_or_default().to_string();
                        let lbl = pair[1].as_str().unwrap_or(&val).to_string();
                        list.push((val, lbl));
                    }
                    serde_yml::Value::String(s) => {
                        list.push((s.clone(), s.clone()));
                    }
                    other => {
                        let s = format!("{:?}", other);
                        list.push((s.clone(), s));
                    }
                }
            }
        }
        list
    }

    /// Execution mode for this menu item
    pub fn execution_mode(&self) -> ExecutionMode {
        if let Some(s) = self.extra.get("exec_mode").and_then(|v| v.as_str()) {
            match s.to_lowercase().as_str() {
                "streaming" | "stream" => ExecutionMode::Streaming,
                "interactive" => ExecutionMode::Interactive,
                "console" | "standard" | "terminal" => ExecutionMode::Console,
                _ => ExecutionMode::Console,
            }
        } else if self.is_streaming() {
            ExecutionMode::Streaming
        } else if self
            .extra
            .get("floating")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            ExecutionMode::Interactive
        } else {
            ExecutionMode::Console
        }
    }

    /// Whether this item executes in streaming mode
    pub fn is_streaming(&self) -> bool {
        if let Some(b) = self.extra.get("stream").and_then(|v| v.as_bool()) {
            if b {
                return true;
            }
        }
        if self.interactive == Some(false) {
            return true;
        }
        false
    }

    /// Window percentage for floating dialogs (Streaming or Interactive mode)
    pub fn window_percent(&self) -> u16 {
        self.extra
            .get("window_percent")
            .and_then(|v| {
                if let Some(n) = v.as_u64() {
                    Some(n as u16)
                } else if let Some(s) = v.as_str() {
                    s.trim().trim_end_matches('%').parse::<u16>().ok()
                } else {
                    None
                }
            })
            .map(|pct| pct.clamp(20, 100))
            .unwrap_or(65)
    }
}

/// Execution mode for a menu command or action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    Streaming,
    Interactive,
    Console,
}

/// Submenu container holding child items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Submenu {
    /// Submenu display title
    pub title: String,
    /// Child menu items
    #[serde(default)]
    pub items: Vec<MenuItem>,
}

/// Root menu definition schema matching `bashmenu.mnu`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuSchema {
    /// Menu specification version
    #[serde(default = "default_version")]
    pub version: String,
    /// Top-level menu title
    #[serde(default = "default_title")]
    pub title: String,
    /// Menu items
    #[serde(default)]
    pub items: Vec<MenuItem>,
    /// Extra root fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

fn default_version() -> String {
    "0.0.1".to_string()
}

fn default_title() -> String {
    "Bash Menu".to_string()
}

impl Default for MenuSchema {
    fn default() -> Self {
        Self {
            version: default_version(),
            title: default_title(),
            items: Vec::new(),
            extra: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Custom Serialization and Deserialization for Label-as-Key YAML Schema
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct RawItemProperties {
    #[serde(default)]
    pub r#type: MenuItemType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt_buffer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabstop: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_yes: Option<ConfirmAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_no: Option<ConfirmAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submenu: Option<Submenu>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yml::Value>,
}

impl<'de> Deserialize<'de> for MenuItem {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = serde_yml::Value::deserialize(deserializer)?;

        // Case 1: Plain string (e.g. "{divider}" or a command string)
        if let serde_yml::Value::String(s) = &val {
            if s == "{divider}" {
                return Ok(MenuItem::divider());
            }
            return Ok(MenuItem {
                label: s.clone(),
                command: Some(s.clone()),
                action: Some(s.clone()),
                r#type: MenuItemType::Command,
                ..Default::default()
            });
        }

        // Case 2: Mapping
        if let serde_yml::Value::Mapping(map) = &val {
            if map.is_empty() {
                return Ok(MenuItem::default());
            }

            // Check if divider mapping: {'{divider}': null}
            for k in map.keys() {
                if let serde_yml::Value::String(ks) = k {
                    if ks == "{divider}" {
                        return Ok(MenuItem::divider());
                    }
                }
            }

            // Check if already normalized with "label" key
            let has_label = map.contains_key("label");
            if has_label {
                let label = match map.get("label") {
                    Some(serde_yml::Value::String(s)) => s.clone(),
                    _ => "Unnamed".to_string(),
                };
                let raw_props: RawItemProperties =
                    serde_yml::from_value(serde_yml::Value::Mapping(map.clone()))
                        .map_err(serde::de::Error::custom)?;
                let item_type =
                    if raw_props.submenu.is_some() && raw_props.r#type == MenuItemType::Command {
                        MenuItemType::Submenu
                    } else {
                        raw_props.r#type
                    };
                return Ok(MenuItem {
                    label,
                    r#type: item_type,
                    icon: raw_props.icon,
                    command: raw_props.command,
                    action: raw_props.action,
                    script: raw_props.script,
                    file: raw_props.file,
                    prompt: raw_props.prompt,
                    start_dir: raw_props.start_dir,
                    interactive: raw_props.interactive,
                    quiet: raw_props.quiet,
                    alt_buffer: raw_props.alt_buffer,
                    tabstop: raw_props.tabstop,
                    on_yes: raw_props.on_yes,
                    on_no: raw_props.on_no,
                    submenu: raw_props.submenu,
                    extra: raw_props.extra,
                });
            }

            // Standard label-as-key: { "Label": { props } } or { "Label": null }
            if map.len() == 1 {
                let (k, v) = map.iter().next().unwrap();
                let label = match k {
                    serde_yml::Value::String(s) => s.clone(),
                    other => format!("{:?}", other),
                };

                if v.is_null() {
                    return Ok(MenuItem {
                        label,
                        ..Default::default()
                    });
                }

                if let serde_yml::Value::Mapping(_) = v {
                    let raw_props: RawItemProperties =
                        serde_yml::from_value(v.clone()).map_err(serde::de::Error::custom)?;
                    let item_type = if raw_props.submenu.is_some()
                        && raw_props.r#type == MenuItemType::Command
                    {
                        MenuItemType::Submenu
                    } else {
                        raw_props.r#type
                    };
                    return Ok(MenuItem {
                        label,
                        r#type: item_type,
                        icon: raw_props.icon,
                        command: raw_props.command,
                        action: raw_props.action,
                        script: raw_props.script,
                        file: raw_props.file,
                        prompt: raw_props.prompt,
                        start_dir: raw_props.start_dir,
                        interactive: raw_props.interactive,
                        quiet: raw_props.quiet,
                        alt_buffer: raw_props.alt_buffer,
                        tabstop: raw_props.tabstop,
                        on_yes: raw_props.on_yes,
                        on_no: raw_props.on_no,
                        submenu: raw_props.submenu,
                        extra: raw_props.extra,
                    });
                }
            }
        }

        Ok(MenuItem::default())
    }
}

impl Serialize for MenuItem {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        if self.is_divider() {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("{divider}", &Option::<()>::None)?;
            return map.end();
        }

        let props = RawItemProperties {
            r#type: self.r#type.clone(),
            icon: self.icon.clone(),
            command: self.command.clone(),
            action: self.action.clone(),
            script: self.script.clone(),
            file: self.file.clone(),
            prompt: self.prompt.clone(),
            start_dir: self.start_dir.clone(),
            interactive: self.interactive,
            quiet: self.quiet,
            alt_buffer: self.alt_buffer,
            tabstop: self.tabstop,
            on_yes: self.on_yes.clone(),
            on_no: self.on_no.clone(),
            submenu: self.submenu.clone(),
            extra: self.extra.clone(),
        };

        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.label, &props)?;
        map.end()
    }
}

/// Loads and parses `bashmenu.mnu` from the specified path.
pub fn load_menu<P: AsRef<Path>>(path: P) -> Result<MenuSchema> {
    let p = path.as_ref();
    let content = fs::read_to_string(p).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    let menu: MenuSchema = serde_yml::from_str(&content)?;
    Ok(menu)
}

/// Serializes and saves menu schema to disk using clean label-as-key formatting.
pub fn save_menu<P: AsRef<Path>>(path: P, menu: &MenuSchema) -> Result<()> {
    let p = path.as_ref();
    let yaml_body = serde_yml::to_string(menu)?;
    fs::write(p, yaml_body).map_err(|e| BashMenuError::Io {
        path: p.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_menu_fixture() {
        let fixture_path = Path::new("../../tests/fixtures/bashmenu.mnu");
        if fixture_path.exists() {
            let menu = load_menu(fixture_path).expect("Failed to load bashmenu.mnu");
            assert_eq!(menu.title, "Bash Menu");
            assert!(!menu.items.is_empty(), "Items should not be empty");

            // First item is a divider
            assert!(menu.items[0].is_divider());

            // Second item is "BM Developer Shortcuts" submenu
            let dev_shortcuts = &menu.items[1];
            assert_eq!(dev_shortcuts.label, "BM Developer Shortcuts");
            assert_eq!(dev_shortcuts.r#type, MenuItemType::Submenu);
            assert!(dev_shortcuts.submenu.is_some());

            let sub = dev_shortcuts.submenu.as_ref().unwrap();
            assert!(!sub.items.is_empty());
        }
    }
}
