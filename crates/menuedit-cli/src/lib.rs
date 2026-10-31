//! Library module for MenuEdit core tree navigation and menu manipulation.

pub mod item_edit;
pub mod tree;
pub mod type_picker;

use anyhow::Result;
use bashmenu_core::{
    interpolate_placeholders, load_menu, save_menu, BashMenuConfig, MacroContext, MenuItem,
    MenuItemType, MenuSchema, Submenu,
};
use bashmenu_tui::{split_label_brackets, strip_bbcode, visible_text_len};
use std::collections::HashSet;
use std::path::PathBuf;

pub use item_edit::{
    render_item_preview_line, BrowseTarget, FocusControl, ItemEditModal, ItemEditResult, TextInput,
};
pub use tree::{
    delete_item, find_item, find_item_mut, flatten_tree, indent_item, insert_item_after,
    move_down_sibling, move_up_sibling, mutate_item_type, outdent_item, toggle_all_nodes,
    VisibleNode,
};
pub use type_picker::{ItemTypePickerModal, ITEM_TYPES};

pub const ITEM_EDIT_HELP_TEXT: &str = "\
BashMenu Item Properties (Editor Help)

── 1. Text Fields ──

Title / Label (title | label)
  Visible text of the menu entry, aligned at column 11
  after the fixed 4-column icon slot.

Icon / Glyph (icon | glyph)
  Leading glyph drawn in a 4-column slot.
  Adaptive form {nf:<char>:<nerd_hex>:<emoji>}.
  Press F4 / CTRL+N to search Nerd Font & Emoji glyphs.

Action / Command / Script / File / Key
  Payload executed when the entry is selected.
  Type-specific key: command->command, script->script,
  editor->file, notebook->file, toggle/config->key, python->python.
  Can embed dynamic directives: {file_picker}, {param}, {dir_picker}.

Notebook File Path (file) [notebook]
  Path to YAML notebook notes file. Defaults to {notebooks_dir}/notes.yml.

Message / Prompt Text (message | prompt)
  Text shown to user, or prompt displayed for input/confirm modals.

Radio Options (options) [radio]
  Selectable choices presented in a radio dialog modal.

Template Path (template) [inject_block]
  Source file whose contents are injected.

Target Path (target) [inject_block]
  Destination file edited on install or removal.

── 2. Execution Flags ──

Alt Buffer: Run command inside alternate screen buffer (alt_buffer=true)
No Formatting: Disable BBCode parsing in title/message (no_formatting=true)
Quiet: Suppress stdout output (quiet=true)
Interactive: Attach full interactive terminal stdin/stdout (interactive=true)
Show Whitespace: Display visible tabs and spaces in editor (show_whitespace=true)
External: Execute outside terminal sandbox (external=true)
";

pub const PLACEHOLDER_HELP_TEXT: &str = "\
── Interactive Input Directives ──
{param}                      │ Prompts for text input modal (masked=true supported)
{file_picker}                │ Launches visual file chooser dialog
{file_picker:path:<dir>}     │ File chooser opening in specified default directory
{file_picker:stem}           │ Path without file extension (/path/to/img)
{file_picker:name}           │ Filename with extension (img.png)
{file_picker:base}           │ Filename without extension (img)
{file_picker:dir}            │ Parent directory path (/path/to)
{file_picker:ext}            │ File extension with leading dot (.png)
{file_picker_new}            │ File chooser allowing creation of new files ('n')
{dir_picker}                 │ Launches visual directory chooser dialog
{dir_picker:path:<dir>}      │ Directory chooser opening in specified default directory
{dir_picker:name}            │ Folder name without parent path
{dir_picker:dir}             │ Parent directory path of chosen folder
{dir_picker_new}             │ Directory chooser allowing creation of new folders ('n')

── System & Path Placeholders ──
{user}                       │ Current logged-in username
{username}                   │ Alias for {user}
{host}                       │ System hostname
{hostname}                   │ Alias for {host}
{home}                       │ User home directory path (~ / /home/username)
{bashmenu_dir}               │ Application root directory
{scripts_dir}                │ Scripts directory path
{scripts}                    │ Alias for {scripts_dir}
{templates_dir}              │ Templates directory path
{templates}                  │ Alias for {templates_dir}
{cache_dir}                  │ Cache directory path
{cache}                      │ Alias for {cache_dir}
{notebooks_dir}              │ Notebooks directory path
{notebooks}                  │ Alias for {notebooks_dir}
{bashrc}                     │ Path to ~/.bashrc profile file
{bash_aliases}               │ Path to ~/.bash_aliases file
{zshrc}                      │ Path to ~/.zshrc configuration file
{profile}                    │ Path to ~/.profile (or existing profile)
{bash_profile}               │ Path to ~/.bash_profile (or existing profile)
{zprofile}                   │ Path to ~/.zprofile configuration file
{shell_profile}              │ Path to active shell login profile
{vimrc}                      │ Path to ~/.vimrc configuration file
{nanorc}                     │ Path to ~/.nanorc configuration file

── Cross-Platform Environment ──
{prefix}                     │ Termux / system prefix path ($PREFIX / /usr)
{termux_properties}          │ Termux properties file path
{termux_storage}             │ Termux Android storage directory (~/storage)
{powershell_profile}         │ PowerShell profile script path
{appdata}                    │ Windows AppData roaming directory path
{userprofile}                │ Windows user profile directory path

── System Status & Network ──
{battery}                    │ Current battery capacity percentage
{battery_stat}               │ Current battery charging status (Charging, Discharging, Full)
{battery_status}             │ Alias for {battery_stat}
{localip}                    │ Primary outbound IPv4 address
{user-mode}                  │ Privilege level (User or Root)
{version}                    │ BashMenu application version
{theme}                      │ Active color theme name (or {app.theme})

── Date & Time Placeholders ──
{date_time_12}               │ Timestamp 12-hr format (YYYY-MM-DD HH:MM:SS AM/PM)
{date_time_12_short}         │ Timestamp 12-hr short format (YYYY-MM-DD HH:MM AM/PM)
{date_time_24}               │ Timestamp 24-hr format (YYYY-MM-DD HH:MM:SS)
{date_time_24_short}         │ Timestamp 24-hr short format (YYYY-MM-DD HH:MM)
{date}                       │ Current date (YYYY-MM-DD)
{time_12}                    │ Current time 12-hr format
{time_12_short}              │ Current time 12-hr short format
{time_24}                    │ Current time 24-hr format
{time_24_short}              │ Current time 24-hr short format
{utc_seconds}                │ UTC Unix epoch timestamp in seconds

── Terminal Layout Directives ──
{window_width}               │ Inner terminal window column width
{window_height}              │ Inner terminal window line height
{screen_width}               │ Full terminal screen column width
{divider}                    │ Themed horizontal divider bar ({ascii:196})

── Special Encodings & Dynamic Macros ──
{ascii:<code_num>}           │ CP437 ASCII character byte (e.g. {ascii:196} -> ─)
{command:<cmd>}              │ Executes shell command and inserts output
{toast:SECONDS:\"MESSAGE\"}    │ Displays non-blocking popup toast message
{nf:<char>:<hex>:<emoji>}    │ Adaptive Nerd Font / Unicode / Emoji glyph
{app.<key.path>}             │ Configuration key in bashmenu.yml (e.g. {app.theme})
{<key.path>}                 │ Refers to nested key in bashmenu.yml (e.g. {user.postal_code})
";

/// Returns the 5-character badge identifying an item's type
pub fn get_type_badge(item: &MenuItem) -> &'static str {
    if item.is_divider() {
        return "[DIV]";
    }
    match &item.r#type {
        MenuItemType::Submenu => "[DIR]",
        MenuItemType::Command => "[CMD]",
        MenuItemType::Script => "[SCR]",
        MenuItemType::Config => "[CFG]",
        MenuItemType::Toggle => "[TGL]",
        MenuItemType::Radio => "[RAD]",
        MenuItemType::Editor => "[EDT]",
        MenuItemType::Notebook => "[NBK]",
        MenuItemType::NotebookNote => "[NOT]",
        MenuItemType::NotebookEmpty => "[EMP]",
        MenuItemType::Exit => "[EXT]",
        MenuItemType::Back => "[BCK]",
        MenuItemType::ThemeSelector => "[THM]",
        MenuItemType::Python => "[PY ]",
        MenuItemType::Other(s) => match s.as_str() {
            "confirm" => "[CNF]",
            "message" => "[MSG]",
            "inject_block" => "[INJ]",
            "theme_selector" => "[THM]",
            "notebook" => "[NBK]",
            "back" => "[BCK]",
            "exit" => "[EXT]",
            _ => "[???]",
        },
        _ => "[???]",
    }
}

/// Formats the property inspector panel content for a given menu item
pub fn format_inspector_text(
    item: &MenuItem,
    cfg: Option<&BashMenuConfig>,
    avail_w: usize,
) -> Vec<String> {
    let default_cfg = BashMenuConfig::default();
    let effective_cfg = cfg.unwrap_or(&default_cfg);
    let mut ctx = MacroContext::new(effective_cfg);
    ctx.window_width = avail_w;

    if item.is_divider() {
        let char_val = item
            .extra
            .get("char")
            .and_then(|v| v.as_str())
            .unwrap_or("{ascii:196}");
        let length_val = item
            .extra
            .get("length")
            .and_then(|v| v.as_str())
            .unwrap_or("{window_width}");

        let exp_char = interpolate_placeholders(char_val, &ctx);
        let div_glyph = if exp_char.is_empty() {
            "─".to_string()
        } else {
            exp_char
        };

        let target_len = length_val
            .parse::<usize>()
            .unwrap_or_else(|_| avail_w.max(10));
        let disp_len = target_len.min(avail_w.max(10));
        let rep = div_glyph.repeat(disp_len);
        let preview_line = if rep.chars().count() > disp_len {
            rep.chars().take(disp_len).collect::<String>()
        } else {
            rep
        };

        return vec![
            "[DIV] DIVIDER".to_string(),
            String::new(),
            format!("Repeating Char (Theme): {}", char_val),
            format!("Length Directive (Theme): {}", length_val),
            String::new(),
            "Preview:".to_string(),
            preview_line,
        ];
    }

    let badge = get_type_badge(item);
    let type_name = match &item.r#type {
        MenuItemType::Other(s) => s.to_uppercase(),
        t => format!("{:?}", t).to_uppercase(),
    };

    let exp_title = interpolate_placeholders(&item.label, &ctx);
    let mut lines = vec![
        format!("{} {}", badge, type_name),
        format!("Title/Label: {}", item.label),
    ];
    if exp_title != item.label {
        lines.push(format!("Expanded Title: {}", exp_title));
    }

    if let Some(ref icon) = item.icon {
        lines.push(format!("Icon/Glyph: {}", icon));
        let exp_icon = interpolate_placeholders(icon, &ctx);
        if exp_icon != *icon {
            lines.push(format!("Expanded Icon: {}", exp_icon));
        }
    }

    let action = item
        .action
        .as_ref()
        .or(item.command.as_ref())
        .or(item.script.as_ref())
        .or(item.file.as_ref());
    if let Some(act) = action {
        lines.push(format!("Action/Target: {}", act));
        let exp_act = interpolate_placeholders(act, &ctx);
        if exp_act != *act {
            lines.push(format!("Expanded Action: {}", exp_act));
        }
    }

    if let Some(val) = item.extra.get("key").and_then(|v| v.as_str()) {
        lines.push(format!("Config Key: {}", val));
    }

    if let Some(ref prompt) = item.prompt {
        lines.push(format!("Message/Prompt: {}", prompt));
        let exp_prompt = interpolate_placeholders(prompt, &ctx);
        if exp_prompt != *prompt {
            lines.push(format!("Expanded Message: {}", exp_prompt));
        }
    } else if let Some(msg) = item.extra.get("message").and_then(|v| v.as_str()) {
        lines.push(format!("Message/Prompt: {}", msg));
        let exp_msg = interpolate_placeholders(msg, &ctx);
        if exp_msg != msg {
            lines.push(format!("Expanded Message: {}", exp_msg));
        }
    }

    if let Some(tmpl) = item.extra.get("template").and_then(|v| v.as_str()) {
        lines.push(format!("Template: {}", tmpl));
        let exp_tmpl = interpolate_placeholders(tmpl, &ctx);
        if exp_tmpl != tmpl {
            lines.push(format!("Expanded Template: {}", exp_tmpl));
        }
    }

    if let Some(tgt) = item.extra.get("target").and_then(|v| v.as_str()) {
        lines.push(format!("Target File: {}", tgt));
        let exp_tgt = interpolate_placeholders(tgt, &ctx);
        if exp_tgt != tgt {
            lines.push(format!("Expanded Target: {}", exp_tgt));
        }
    }

    if let Some(bid) = item.extra.get("block_id").and_then(|v| v.as_str()) {
        lines.push(format!("Block ID: {}", bid));
    }

    if let Some(ref sdir) = item.start_dir {
        lines.push(format!("Start Directory: {}", sdir));
        let exp_sdir = interpolate_placeholders(sdir, &ctx);
        if exp_sdir != *sdir {
            lines.push(format!("Expanded Directory: {}", exp_sdir));
        }
    }

    if let Some(pkr) = item.extra.get("picker").and_then(|v| v.as_str()) {
        lines.push(format!("Picker Type: {}", pkr));
    }

    if let Some(oy) = item.extra.get("on_yes").and_then(|v| v.as_str()) {
        lines.push(format!("On Yes Action: {}", oy));
    }

    if let Some(on) = item.extra.get("on_no").and_then(|v| v.as_str()) {
        lines.push(format!("On No Action: {}", on));
    }

    if let Some(alt) = item.alt_buffer {
        lines.push(format!("Alt Buffer: {}", alt));
    }

    if let Some(nf) = item.extra.get("no_formatting").and_then(|v| v.as_bool()) {
        lines.push(format!("No Formatting: {}", nf));
    }

    let mut flags = Vec::new();
    if let Some(b) = item.extra.get("stream").and_then(|v| v.as_bool()) {
        flags.push(format!("stream={}", b));
    }
    if let Some(b) = item.quiet {
        flags.push(format!("quiet={}", b));
    }
    if let Some(b) = item.interactive {
        flags.push(format!("interactive={}", b));
    }
    if let Some(b) = item.extra.get("show_whitespace").and_then(|v| v.as_bool()) {
        flags.push(format!("show_whitespace={}", b));
    }
    if let Some(b) = item.extra.get("masked").and_then(|v| v.as_bool()) {
        flags.push(format!("masked={}", b));
    }
    if let Some(b) = item.extra.get("refresh").and_then(|v| v.as_bool()) {
        flags.push(format!("refresh={}", b));
    }
    if let Some(b) = item.extra.get("external").and_then(|v| v.as_bool()) {
        flags.push(format!("external={}", b));
    }
    if let Some(ts) = item.tabstop {
        flags.push(format!("tabstop={}", ts));
    }

    if !flags.is_empty() {
        lines.push(format!("Flags: {}", flags.join(", ")));
    }

    if let Some(ref sub) = item.submenu {
        lines.push(format!("Submenu Items: {} items", sub.items.len()));
    }

    // Extra properties
    let known_keys = [
        "type",
        "title",
        "label",
        "icon",
        "glyph",
        "action",
        "command",
        "script",
        "file",
        "python",
        "key",
        "message",
        "prompt",
        "text",
        "template",
        "target",
        "block_id",
        "start_dir",
        "picker",
        "on_yes",
        "on_no",
        "alt_buffer",
        "no_formatting",
        "stream",
        "quiet",
        "interactive",
        "show_whitespace",
        "masked",
        "refresh",
        "external",
        "display_theme_colors",
        "tabstop",
        "submenu",
        "options",
    ];
    let mut extras = Vec::new();
    for (k, v) in &item.extra {
        if !known_keys.contains(&k.as_str()) {
            let val_str = match v {
                serde_yml::Value::String(s) => s.clone(),
                serde_yml::Value::Bool(b) => b.to_string(),
                serde_yml::Value::Number(n) => n.to_string(),
                other => format!("{:?}", other),
            };
            extras.push(format!("{}={}", k, val_str));
        }
    }
    if !extras.is_empty() {
        lines.push(format!("Extra Properties: {}", extras.join(", ")));
    }

    // Live Item Preview
    lines.push(String::new());
    lines.push("Preview:".to_string());

    let (left_raw, right_raw) = split_label_brackets(&exp_title);
    let left_clean = strip_bbcode(left_raw);
    let right_clean = strip_bbcode(right_raw);

    let exp_icon = item
        .icon
        .as_deref()
        .map(|ic| interpolate_placeholders(ic, &ctx))
        .unwrap_or_default();

    let icon_str = if !exp_icon.is_empty() {
        let vis_w = visible_text_len(&exp_icon);
        let pad_w = 4_usize.saturating_sub(vis_w).max(1);
        format!("{}{}", exp_icon, " ".repeat(pad_w))
    } else {
        "    ".to_string()
    };

    let prefix = format!("  1.  {}", icon_str);
    let prefix_len = visible_text_len(&prefix);
    let left_len = visible_text_len(&left_clean);
    let right_len = visible_text_len(&right_clean);

    let preview_str = if !right_clean.is_empty() {
        let total_needed = prefix_len + left_len + right_len;
        if total_needed < avail_w {
            let spacer_len = avail_w - total_needed;
            format!(
                "{}{}{}{}",
                prefix,
                left_clean,
                " ".repeat(spacer_len),
                right_clean
            )
        } else {
            format!("{} {} {}", prefix, left_clean, right_clean)
        }
    } else {
        format!("{}{}", prefix, left_clean)
    };

    lines.push(preview_str);

    lines
}

#[derive(Debug, Clone)]
pub struct EditorLevel {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub selected_idx: usize,
}

pub struct MenuEditorState {
    pub menu_path: PathBuf,
    pub root_menu: MenuSchema,
    pub stack: Vec<EditorLevel>,
    pub expanded_paths: HashSet<Vec<usize>>,
    pub is_modified: bool,
    pub status_message: String,
}

impl MenuEditorState {
    pub fn new(menu_path: PathBuf) -> Self {
        let root_menu = if menu_path.exists() {
            load_menu(&menu_path).unwrap_or_default()
        } else {
            MenuSchema::default()
        };

        let initial_level = EditorLevel {
            title: root_menu.title.clone(),
            items: root_menu.items.clone(),
            selected_idx: 0,
        };

        Self {
            menu_path,
            root_menu,
            stack: vec![initial_level],
            expanded_paths: HashSet::new(),
            is_modified: false,
            status_message: "Ready".to_string(),
        }
    }

    pub fn current_level(&self) -> &EditorLevel {
        self.stack.last().expect("Stack must never be empty")
    }

    pub fn current_level_mut(&mut self) -> &mut EditorLevel {
        self.stack.last_mut().expect("Stack must never be empty")
    }

    pub fn current_items(&self) -> &[MenuItem] {
        &self.current_level().items
    }

    pub fn selected_index(&self) -> usize {
        self.current_level().selected_idx
    }

    pub fn move_up(&mut self) {
        let level = self.current_level_mut();
        if level.selected_idx > 0 {
            level.selected_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let level = self.current_level_mut();
        if !level.items.is_empty() && level.selected_idx + 1 < level.items.len() {
            level.selected_idx += 1;
        }
    }

    pub fn swap_up(&mut self) {
        let level = self.current_level_mut();
        let idx = level.selected_idx;
        if idx > 0 && idx < level.items.len() {
            level.items.swap(idx, idx - 1);
            level.selected_idx = idx - 1;
            self.is_modified = true;
            self.status_message = "Moved item up".to_string();
        }
    }

    pub fn swap_down(&mut self) {
        let level = self.current_level_mut();
        let idx = level.selected_idx;
        if !level.items.is_empty() && idx + 1 < level.items.len() {
            level.items.swap(idx, idx + 1);
            level.selected_idx = idx + 1;
            self.is_modified = true;
            self.status_message = "Moved item down".to_string();
        }
    }

    pub fn add_item(&mut self, item: MenuItem) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            level.items.push(item);
            level.selected_idx = 0;
        } else {
            let insert_at = (level.selected_idx + 1).min(level.items.len());
            level.items.insert(insert_at, item);
            level.selected_idx = insert_at;
        }
        self.is_modified = true;
        self.status_message = "Added new item".to_string();
    }

    pub fn edit_selected(&mut self, label: String, command: Option<String>, icon: Option<String>) {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return;
        }
        let idx = level.selected_idx;
        level.items[idx].label = label;
        level.items[idx].command = command;
        level.items[idx].icon = icon;
        self.is_modified = true;
        self.status_message = "Updated item properties".to_string();
    }

    pub fn delete_selected(&mut self) -> Option<MenuItem> {
        let level = self.current_level_mut();
        if level.items.is_empty() {
            return None;
        }
        let idx = level.selected_idx;
        let removed = level.items.remove(idx);
        if level.selected_idx >= level.items.len() && !level.items.is_empty() {
            level.selected_idx = level.items.len() - 1;
        }
        self.is_modified = true;
        self.status_message = format!("Deleted '{}'", removed.label);
        Some(removed)
    }

    pub fn enter_submenu(&mut self) -> bool {
        let sub_opt = {
            let level = self.current_level();
            if level.items.is_empty() {
                None
            } else {
                let item = &level.items[level.selected_idx];
                if item.r#type == MenuItemType::Submenu {
                    item.submenu.clone()
                } else {
                    None
                }
            }
        };

        if let Some(sub) = sub_opt {
            let new_level = EditorLevel {
                title: sub.title.clone(),
                items: sub.items,
                selected_idx: 0,
            };
            self.stack.push(new_level);
            self.status_message = format!("Entered submenu '{}'", sub.title);
            return true;
        }
        false
    }

    pub fn leave_submenu(&mut self) -> bool {
        if self.stack.len() <= 1 {
            return false;
        }
        let child_level = self.stack.pop().unwrap();
        let parent_level = self.stack.last_mut().unwrap();
        let idx = parent_level.selected_idx;
        if idx < parent_level.items.len() {
            parent_level.items[idx].submenu = Some(Submenu {
                title: child_level.title,
                items: child_level.items,
            });
        }
        self.status_message = "Returned to parent menu".to_string();
        true
    }

    /// Persists changes to disk using `bashmenu_core::save_menu`
    pub fn save_to_disk(&mut self) -> Result<()> {
        while self.stack.len() > 1 {
            self.leave_submenu();
        }
        self.root_menu.items = self.stack[0].items.clone();
        save_menu(&self.menu_path, &self.root_menu)?;
        self.is_modified = false;
        self.status_message = "Saved menu to disk".to_string();
        Ok(())
    }
}
