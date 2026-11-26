//! Shared documentation, help texts, and reference tables for BashMenu and BashEdit.

use crate::macro_engine::{interpolate_placeholders, MacroContext};

/// Help text and reference guide for macro placeholders supported by BashMenu.
///
/// Uses double-brace escaping (`{{macro}}`) for literal placeholder names and single-brace
/// syntax (`{macro}`) in the `Value` column of 3-column tables so [`render_placeholder_help_text`]
/// can expand live values on the current system.
pub const PLACEHOLDER_HELP_TEXT: &str = "\
## Interactive Input Directives
| Macro / Placeholder | Description |
| :--- | :--- |
| {{param}} | Prompts for text input modal (masked=true supported) |
| {{file_picker}} | Launches visual file chooser dialog |
| {{file_picker:path:<dir>}} | File chooser opening in specified default directory |
| {{file_picker:stem}} | Path without file extension (/path/to/img) |
| {{file_picker:name}} | Filename with extension (img.png) |
| {{file_picker:base}} | Filename without extension (img) |
| {{file_picker:dir}} | Parent directory path (/path/to) |
| {{file_picker:ext}} | File extension with leading dot (.png) |
| {{file_picker_new}} | File chooser allowing creation of new files ('n') |
| {{dir_picker}} | Launches visual directory chooser dialog |
| {{dir_picker:path:<dir>}} | Directory chooser opening in specified default directory |
| {{dir_picker:name}} | Folder name without parent path |
| {{dir_picker:dir}} | Parent directory path of chosen folder |
| {{dir_picker_new}} | Directory chooser allowing creation of new folders ('n') |

## System & Path Placeholders
| Macro | Description | Value |
| :--- | :--- | :--- |
| {{user}} | Current logged-in username | {user} |
| {{username}} | Alias for {{user}} | {username} |
| {{host}} | System hostname | {host} |
| {{hostname}} | Alias for {{host}} | {hostname} |
| {{home}} | User home directory path | {home} |
| {{dev_dir}} | Repository root directory | {dev_dir} |
| {{dev}} | Alias for {{dev_dir}} | {dev} |
| {{defaults_dir}} | Repository defaults directory | {defaults_dir} |
| {{defaults}} | Alias for {{defaults_dir}} | {defaults} |
| {{bashmenu_dir}} | Application root directory | {bashmenu_dir} |
| {{config_dir}} | Alias for {{bashmenu_dir}} | {config_dir} |
| {{scripts_dir}} | Scripts directory path | {scripts_dir} |
| {{scripts}} | Alias for {{scripts_dir}} | {scripts} |
| {{templates_dir}} | Templates directory path | {templates_dir} |
| {{templates}} | Alias for {{templates_dir}} | {templates} |
| {{cache_dir}} | Cache directory path | {cache_dir} |
| {{cache}} | Alias for {{cache_dir}} | {cache} |
| {{notebooks_dir}} | Notebooks directory path | {notebooks_dir} |
| {{notebooks}} | Alias for {{notebooks_dir}} | {notebooks} |
| {{shortcuts_file}} | Custom shortcuts file path | {shortcuts_file} |
| {{shortcuts}} | Alias for {{shortcuts_file}} | {shortcuts} |
| {{bashmenu_yml}} | Currently loaded bashmenu.yml | {bashmenu_yml} |
| {{bashmenu_mnu}} | Currently loaded bashmenu.mnu | {bashmenu_mnu} |
| {{bashmenu_themes}} | Currently loaded bashmenu.themes | {bashmenu_themes} |
| {{shortcuts_yml}} | Currently loaded shortcuts.yml | {shortcuts_yml} |
| {{cheat_yml}} | Currently loaded cheat.yml | {cheat_yml} |
| {{bashrc}} | Path to ~/.bashrc profile file | {bashrc} |
| {{bash_aliases}} | Path to ~/.bash_aliases file | {bash_aliases} |
| {{zshrc}} | Path to ~/.zshrc config file | {zshrc} |
| {{profile}} | Path to ~/.profile | {profile} |
| {{bash_profile}} | Path to ~/.bash_profile | {bash_profile} |
| {{zprofile}} | Path to ~/.zprofile config file | {zprofile} |
| {{shell_profile}} | Active shell login profile | {shell_profile} |
| {{vimrc}} | Path to ~/.vimrc config file | {vimrc} |
| {{nanorc}} | Path to ~/.nanorc config file | {nanorc} |

## Cross-Platform Environment
| Macro | Description | Value |
| :--- | :--- | :--- |
| {{prefix}} | Termux / system prefix path | {prefix} |
| {{termux_properties}} | Termux properties file path | {termux_properties} |
| {{termux_storage}} | Termux Android storage directory | {termux_storage} |
| {{powershell_profile}} | PowerShell profile script path | {powershell_profile} |
| {{appdata}} | Windows AppData roaming directory | {appdata} |
| {{userprofile}} | Windows user profile directory | {userprofile} |

## System Status & Network
| Macro | Description | Value |
| :--- | :--- | :--- |
| {{battery}} | Current battery capacity | {battery} |
| {{battery_stat}} | Current battery charging status | {battery_stat} |
| {{battery_status}} | Alias for {{battery_stat}} | {battery_status} |
| {{localip}} | Primary outbound IPv4 address | {localip} |
| {{user-mode}} | Privilege level (User or Root) | {user-mode} |
| {{version}} | Application release version | {version} |
| {{theme}} | Active color theme name | {theme} |
| {{locale}} | Active interface locale | {locale} |
| {{color_mode}} | Current terminal color mode | {color_mode} |
| {{tty_mode}} | Current TTY mode | {tty_mode} |

## Date & Time Placeholders
| Macro | Description | Value |
| :--- | :--- | :--- |
| {{date_time_12}} | Timestamp 12-hr format | {date_time_12} |
| {{date_time_12_short}} | Timestamp 12-hr short format | {date_time_12_short} |
| {{date_time_24}} | Timestamp 24-hr format | {date_time_24} |
| {{date_time_24_short}} | Timestamp 24-hr short format | {date_time_24_short} |
| {{date}} | Current date (YYYY-MM-DD) | {date} |
| {{time_12}} | Current time 12-hr format | {time_12} |
| {{time_12_short}} | Current time 12-hr short format | {time_12_short} |
| {{time_24}} | Current time 24-hr format | {time_24} |
| {{time_24_short}} | Current time 24-hr short format | {time_24_short} |
| {{utc_seconds}} | UTC Unix epoch timestamp | {utc_seconds} |

## Terminal Layout Directives
| Macro / Placeholder | Description |
| :--- | :--- |
| {{window_width}} | Inner terminal window column width |
| {{window_height}} | Inner terminal window line height |
| {{screen_width}} | Full terminal screen column width |
| {{divider}} | Themed horizontal divider bar ({{ascii:196}}) |

## Special Encodings & Dynamic Macros
| Macro / Placeholder | Description |
| :--- | :--- |
| {{ascii:<code_num>}} | CP437 ASCII character byte (e.g. {{ascii:196}} -> ─) |
| {{command:<cmd>}} / {{cmd:<cmd>}} | Executes shell command (30s cache) and inserts output |
| {{toast:SECONDS:\"MESSAGE\"}} | Displays non-blocking popup toast message |
| {{nf:<char>:<hex>:<emoji>}} | Adaptive Nerd Font / Unicode / Emoji glyph |
| {{app.<key.path>}} | Configuration key in bashmenu.yml (e.g. {{app.theme}}) |
| {{env:<var_name>}} | Environment variable value (e.g. {{env:USER}}) |
| {{env}} / {{environment}} | Environment inspection modal / overview table |
| {{info:<path>}} | Renders Markdown file with YAML frontmatter (title, width, height) |
| {{help}} | Built-in action: opens BashMenu documentation manual |
| {{theme_picker}} | Built-in action: opens interactive theme selector modal |
| {{text_editor}} | Built-in action: launches bashedit text editor |
| {{menu_editor}} | Built-in action: launches menuedit focused on active menu item |
| {{refresh}} | Built-in action: reloads config, menu, themes, plugins, and gutters |
| {{keys_toggle}} | Built-in action: toggles menu item shortcut keys visibility |
| {{user_gutter}} | Built-in action: opens 1-line status gutter editor modal |
| {{theme_editor}} | Built-in action: launches themeedit on active theme |
| {{shortcut_editor}} | Built-in action: launches shortcutedit on active shortcuts.yml |
| {{back}} | Built-in action: returns to parent menu or exits |
| {{select}} | Built-in action: activates the currently selected menu item |
| {{toggle_gutter}} | Built-in action: toggles help gutter expansion |
| {{<key.path>}} | Refers to nested key in bashmenu.yml (e.g. {{user.postal_code}}) |

## Escaping Literal Braces
| Syntax | Output / Behavior |
| :--- | :--- |
| {{{{user}}}} | Renders literal {{user}} without expanding its value |
| {{{{{{user}}}}}} | Renders {{{user}}} (outer literal braces around expanded value) |

## Markdown Modal Frontmatter ({{info:<path>}})
| Frontmatter Key | Description |
| :--- | :--- |
| --- ... --- | Optional YAML frontmatter block at the top of a Markdown file |
| title: <text> | Popup window title (supports placeholders; defaults to filename) |
| width: <1..100>[%] | Popup width as % of terminal width (accepts 80, \"80\", or \"80%\") |
| height: <1..100>[%] | Popup height as % of terminal height (accepts 70, \"70\", or \"70%\") |
";

/// Returns `true` when running inside a Termux Android environment.
fn is_termux_env() -> bool {
    std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX")
            .map(|p| p.contains("com.termux"))
            .unwrap_or(false)
}

/// Returns `true` if a placeholder token does not apply to the current OS/environment.
fn is_placeholder_na(token: &str) -> bool {
    match token {
        "{termux_properties}" | "{termux_storage}" => !is_termux_env(),
        "{powershell_profile}" => !cfg!(windows) && std::env::var("PROFILE").is_err(),
        "{appdata}" => !cfg!(windows) && std::env::var("APPDATA").is_err(),
        "{userprofile}" => !cfg!(windows) && std::env::var("USERPROFILE").is_err(),
        "{prefix}" => cfg!(windows) && std::env::var("PREFIX").is_err(),
        "{bashrc}" | "{bash_aliases}" | "{zshrc}" | "{profile}" | "{bash_profile}"
        | "{zprofile}" | "{shell_profile}" | "{vimrc}" | "{nanorc}" => cfg!(windows),
        _ => false,
    }
}

/// Renders [`PLACEHOLDER_HELP_TEXT`] with live expanded values in 3-column tables.
///
/// Platform-unavailable placeholders display `na`; empty/unset values display `null`.
pub fn render_placeholder_help_text(ctx: &MacroContext) -> String {
    let mut out = Vec::new();
    for line in PLACEHOLDER_HELP_TEXT.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let parts: Vec<&str> = trimmed.split('|').collect();
            if parts.len() == 5 {
                let c0 = parts[1].trim();
                let c1 = parts[2].trim();
                let c2 = parts[3].trim();
                let is_delim = parts[1..4]
                    .iter()
                    .all(|p| p.trim().chars().all(|ch| ch == '-' || ch == ':'));
                let is_header = c0 == "Macro" && c2 == "Value";
                if is_delim || is_header {
                    out.push(line.to_string());
                    continue;
                }

                let col0 = interpolate_placeholders(c0, ctx);
                let col1 = interpolate_placeholders(c1, ctx);
                let col2 = if is_placeholder_na(c2) {
                    "na".to_string()
                } else {
                    let expanded = interpolate_placeholders(c2, ctx);
                    let clean = expanded.replace(['\r', '\n'], " ").replace('|', "/");
                    if matches!(c2, "{battery}" | "{battery_stat}" | "{battery_status}")
                        && clean.trim().eq_ignore_ascii_case("n/a")
                    {
                        "na".to_string()
                    } else if clean.trim().is_empty() {
                        "null".to_string()
                    } else {
                        clean
                    }
                };
                out.push(format!("| {} | {} | {} |", col0, col1, col2));
                continue;
            }
        }
        out.push(interpolate_placeholders(line, ctx));
    }
    out.join("\n")
}

/// Markdown formatting reference table displayed in help modals.
pub const MARKDOWN_REFERENCE_TEXT: &str = "\
# Markdown Syntax Reference

| Syntax | Output / Action |
| :--- | :--- |
| `# Heading 1` | Top-level section heading |
| `## Heading 2` | Secondary section heading |
| `### Heading 3` | Subsection heading |
| `**Bold**` or `__Bold__` | Bold weighted text |
| `*Italic*` or `_Italic_` | Italic emphasized text |
| `~~Strikethrough~~` | Strikethrough text |
| `` `code` `` | Inline monospace code snippet |
| `> Blockquote` | Quoted callout block |
| `- Item` or `* Item` | Unordered bulleted list item |
| `1. Item` | Ordered numbered list item |
| `- [ ] Todo` / `- [x] Done` | Task list checkboxes |
| `[Title](url)` | Hyperlink anchor reference |
| `![Alt](path/to/img)` | Image embed reference |
| `---` or `***` | Horizontal divider rule |
| ```` ```bash ```` | Fenced code block with syntax highlight |
| `{{placeholder}}` | Escaped literal `{placeholder}` (no macro expansion) |

## YAML Frontmatter for Custom Modals (`{info:<path>}`)

| Frontmatter Key | Description |
| :--- | :--- |
| `--- ... ---` | Optional YAML frontmatter block at the top of a `.md` file |
| `title: <text>` | Popup window title (supports placeholders; defaults to filename) |
| `width: <1..100>[%]` | Popup width as % of terminal width (e.g. `80`, `\"80\"`, `\"80%\"`) |
| `height: <1..100>[%]` | Popup height as % of terminal height (e.g. `70`, `\"70\"`, `\"70%\"`) |
";

/// BashEdit keybindings, keyboard shortcuts, and control commands reference table.
pub const BASHEDIT_HELP_TEXT: &str = "\
# BashEdit Keybindings & Controls

| Keybinding / Shortcut | Description / Action |
| :--- | :--- |
| `F1` | Help Manual |
| `F2` | Toggle Whitespace Display (spaces & tabs) |
| `F3` | View ASCII Character Table |
| `F4` | View Terminal Colors |
| `F5` | Show Available Placeholders & Macros |
| `F6` | Markdown Preview (via bmpager --markdown) |
| `F7` / `Alt+N` | Toggle Line Numbers |
| `F8` | Show Markdown Syntax Quick Reference |
| `F9` / `Alt+W` | Toggle Soft-Wrap |
| `^O` / `^R` | Open File Picker |
| `^S` | Save File |
| `^A` | Save File As (Save Under New Name) |
| `^N` / `^E` / `[ + ]` | New Tab / New File |
| `Alt+Left/Right` / `Alt+[/]` / `Ctrl+Tab` / `Ctrl+Shift+Tab` | Switch Tabs (Prev / Next) |
| `Ctrl+Left` / `Ctrl+Right` | Move Word Left / Right |
| `Ctrl+Up` / `Ctrl+Down` | Move Paragraph Up / Down |
| `^W` | Where Is (Search text) |
| `^X` | Cut Selection or Current Line |
| `^C` | Copy Selection or Current Line |
| `^V` | Paste Buffer / Clipboard |
| `^Z` / `^Y` | Undo / Redo |
| `^Q` / `ESC` | Exit Editor |
";
