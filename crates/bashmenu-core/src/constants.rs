//! Shared documentation, help texts, and reference tables for BashMenu and BashEdit.

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
";

pub const BASHEDIT_HELP_TEXT: &str = "\
# BashEdit Keybindings & Controls

| Keybinding / Shortcut | Description / Action |
| :--- | :--- |
| `F1` | Help Manual |
| `F2` | Toggle Whitespace Display (spaces & tabs) |
| `F3` | View ASCII Character Table |
| `F4` | View Terminal Colors |
| `F5` | Show Available Placeholders & Macros |
| `F6` | Toggle Markdown Preview |
| `F7` / `Alt+N` | Toggle Line Numbers |
| `F8` | Show Markdown Syntax Quick Reference |
| `^O` / `^R` | Open File Picker |
| `^S` | Save File |
| `^A` / `^Shift+S` | Save File As (Save Under New Name) |
| `^N` / `^E` / `[ + ]` | New Tab / New File |
| `Alt+Left/Right` / `Alt+[/]` / `Ctrl+Tab` | Switch Tabs (Prev / Next) |
| `Ctrl+Left` / `Ctrl+Right` | Move Word Left / Right |
| `Ctrl+Up` / `Ctrl+Down` | Move Paragraph Up / Down |
| `^W` | Where Is (Search text) |
| `^X` | Cut Selection or Current Line |
| `^C` | Copy Selection or Current Line |
| `^V` | Paste Buffer / Clipboard |
| `^Z` / `^Y` | Undo / Redo |
| `^Q` / `ESC` | Exit Editor |
";
