# HA Bash Menu Specification & User Manual (v0.1.2)

Welcome to the specification and user manual for HA Bash Menu, a lightweight,
data-driven Ratatui Text User Interface (TUI) menu engine and configuration
editor for Linux systems.

<details>
<summary><b>Table of Contents</b> (click to expand)</summary>

- [1. Component Overview & Files](#1-component-overview--files)
  - [1.1 Local Manual Pages (Man Pages)](#11-local-manual-pages-man-pages)
- [2. Dynamic Scripts & Templates References](#2-dynamic-scripts--templates-references)
  - [2.1 Provided Utility Scripts (`scripts/`)](#21-provided-utility-scripts-scripts)
  - [2.2 Provided Templates (`templates/`)](#22-provided-templates-templates)
- [3. Keyboard Shortcuts & Navigation Reference](#3-keyboard-shortcuts--navigation-reference)
  - [3.1 Main Menu Engine (`bashmenu`)](#31-main-menu-engine-bashmenu)
  - [3.2 Visual Menu Editor (`menuedit`)](#32-visual-menu-editor-menuedit)
  - [3.3 Built-in Text Editor (`bashedit`)](#33-built-in-text-editor-bashedit)
  - [3.4 Visual Theme Editor (`themeedit`)](#34-visual-theme-editor-themeedit)
  - [3.5 Interactive Terminal Pager (`bmpager`)](#35-interactive-terminal-pager-bmpager)
  - [3.6 Cheatsheet Manager (`cheat`)](#36-cheatsheet-manager-cheat)
  - [3.7 Custom Keyboard Shortcuts Editor (`shortcutedit`)](#37-custom-keyboard-shortcuts-editor-shortcutedit--menuedit---shortcuts)
- [4. Configuration & Schema Specifications](#4-configuration--schema-specifications)
  - [4.1 Settings Store (`bashmenu.yml`)](#41-settings-store-bashmenuyml)
  - [4.2 Menu Structure (`bashmenu.mnu`)](#42-menu-structure-bashmenumnu)
  - [4.3 Color Themes Specification (`bashmenu.themes`)](#43-color-themes-specification-bashmenuthemes)
  - [4.4 Wallpaper Backgrounds & Image Preparation](#44-wallpaper-backgrounds--image-preparation)
- [5. Complete Menu Option Types Reference](#5-complete-menu-option-types-reference)
  - [5.1 Notebook Menu Option (`type: notebook`)](#51-notebook-menu-option-type-notebook)
- [6. Dynamic Directives & Variables](#6-dynamic-directives--variables)
  - [6.1 Dynamic Input Directives](#61-dynamic-input-directives)
  - [6.2 System & Path Placeholders](#62-system--path-placeholders)
  - [6.3 Nerd Fonts & Emoji Adaptive Resolution](#63-nerd-fonts--emoji-adaptive-resolution)
  - [6.4 Console Tag Rich Formatting](#64-console-tag-rich-formatting)
- [7. Advanced Integration Use Cases](#7-advanced-integration-use-cases)
  - [7.1 Modular Block Injection (`type: inject_block`)](#71-modular-block-injection-type-inject_block)
  - [7.2 Subprocess Execution Modes (`execution_mode`, `stream`, `interactive`)](#72-subprocess-execution-modes-execution_mode-stream-interactive)
  - [7.3 Multi-Platform Support & Environment Detection](#73-multi-platform-support--environment-detection)
  - [7.4 Background Footer Plugins Architecture & Creation Guide](#74-background-footer-plugins-architecture--creation-guide)

</details>

---

## 1. Component Overview & Files

HA Bash Menu is comprised of modular scripts and configuration files:

| Component / File | Description |
| :--- | :--- |
| `install.sh` | Universal POSIX /bin/sh installer for workspace binaries and user config. |
| `uninstall.sh` | Pure POSIX /bin/sh uninstaller removing files, symlinks, and cache. |
| `bashmenu.sh` | Launcher shell script with auto-setup and self-healing. |
| `bashmenu` | Core high-performance Ratatui TUI application engine and main rendering window. |
| `bashedit` | Built-in text editor engine with swap file recovery. |
| `menuedit` | Interactive TUI visual editor for `bashmenu.mnu` (run via `F4`). |
| `themeedit` | Interactive TUI visual theme creator and palette editor (run via `F8`). |
| `shortcutedit` | Interactive TUI keyboard shortcuts editor for `shortcuts.yml` (run via `F9`). |
| `bmpager` | Interactive full-screen terminal text & Markdown pager (`bmpager --markdown`). |
| `cheat` | Fast CLI Markdown cheatsheet search engine and viewer. |
| `bashmenu.yml` | YAML user settings configuration store. |
| `bashmenu.mnu` | YAML menu layout structure and action definitions. |
| `bashmenu.themes` | YAML color themes for 256, 16, and 8 color modes. |
| `ymlcheck` | Command line validator for theme and configuration YAML files. |

### 1.1 Local Manual Pages (Man Pages)

| File | Description |
| :--- | :--- |
| `bashmenu.1` | Terminal manual page for HA Bash Menu. |

### 1.2 Command-Line Options (`bashmenu`)

| Option | Short | Description |
| :--- | :--- | :--- |
| `--config <FILE>` | `-c` | Explicit path to configuration file (default: `~/.config/bashmenu/bashmenu.yml`). |
| `--menu <FILE>` | `-m` | Explicit path to menu definition file (default: `~/.config/bashmenu/bashmenu.mnu`). |
| `--theme <NAME>` | `-t` | Override active theme name. |
| `--shortcuts <FILE>` | `-s` | Explicit path to keyboard shortcuts file (default: `~/.config/bashmenu/shortcuts.yml`). |
| `--repo` | `-r` | Load configuration files (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, `shortcuts.yml`) from repository root directory. |
| `--image` | `-i` | Render full-resolution TrueColor background images directly via the Linux framebuffer (`/dev/fb0`) instead of character-block translation. |
| `--local` | | Load and save configuration files in current working directory. |

---

## 2. Dynamic Scripts & Templates References

### 2.1 Provided Utility Scripts (`scripts/`)
These scripts are stored in the `/scripts` directory and can be used directly or as action references:

| Script | Description |
| :--- | :--- |
| `scripts/asteroids.sh` | Asteroids game runner that downloads, compiles, and launches the terminal game. |
| `scripts/bsdgames.sh` | Interactive runner menu for classic BSD games (adventure, atc, battlestar, robots, snake, tetris, etc.). |
| `scripts/cheat.sh` | Cheatsheet lookup utility launcher trampoline for the native cheat binary. |
| `scripts/dnsconfig.sh` | Interactive shell script that configures IPv4 and IPv6 DNS server addresses for active network connections using nmcli and systemd-resolved. |
| `scripts/games.sh` | Interactive runner menu for terminal games (ninvaders, pacman4console, nsnake, greed, moon-buggy, nethack, asteroids). |
| `scripts/get_api_key.sh` | Script to securely retrieve and format API keys or credentials. |
| `scripts/getdns.py` | CLI python utility to query and discover currently active upstream system DNS server settings. |
| `scripts/glyphs.sh` | Curses search and preview dialog box utility for selecting Unicode emojis and Nerd Font icons. |
| `scripts/hostname.sh` | Hostname and system identity discovery utility reporting hostname, FQDN, kernel, architecture, and OS distribution. |
| `scripts/install_basics.sh` | Cross-platform basic tools installer wrapper. |
| `scripts/install_cheat.sh` | Setup script that installs interactive cheat sheet terminal tools into the local environment. |
| `scripts/install_games.sh` | Cross-platform installer for terminal games (ninvaders, nsnake, greed, moon-buggy, nethack, etc.). |
| `scripts/install_node.sh` | Installer script for Node.js via nvm or system package manager. |
| `scripts/ip_info.sh` | Displays current network interface IP addresses and connection status. |
| `scripts/otd.sh` | Shell script fetching a random "On This Day" historical quote/event and link from today.zenquotes.io API using curl and jq. |
| `scripts/ping.sh` | Cross-platform ping wrapper with unprivileged TCP handshake latency fallback for restricted environments. |
| `scripts/serverup.sh` | Cross-platform server ping and uptime availability check script. |
| `scripts/sso-ssh.sh` | Automates Ed25519 SSH key generation, authorization, and configuration for passwordless remote server connections. |
| `scripts/system_update.sh` | Cross-platform system update wrapper supporting apt, dnf, pacman, pkg, and brew. |
| `scripts/temp.sh` | Native server and CPU temperature reporting script supporting Celsius and Fahrenheit (`--fahrenheit` / `-f`). |
| `scripts/unicode.sh` | Interactive Unicode Block Explorer script. |
| `scripts/webopen.sh` | Cross-platform web browser launcher wrapper script that opens a specified URL, file, or default browser (supporting Linux, macOS, WSL, and Termux). Supports `--gui` / `-g` for graphical browsers and `--tty` / `tty` for terminal browsers (lynx, links, brow6el). |

### 2.2 Provided Templates (`templates/`)
Used as dynamic templates for code generation, settings, or block injections:

| Template | Description |
| :--- | :--- |
| `templates/autoexec.sh.tmpl` | Configurable template for system autoexec shell scripts (used by configure_autoexec plugin). |
| `templates/bash_aliases.tmpl` | Sourced shell alias definitions injected dynamically into user profiles. |

---

## 3. Keyboard Shortcuts & Navigation Reference

### 3.1 Main Menu Engine (`bashmenu`)
| Keybinding | Action |
| :--- | :--- |
| `UP` / `DOWN`, `k` / `j` | Navigate highlighted selection row. |
| `PageUp` / `PageDown` | Scroll selection up / down by a page increment (10 items). |
| `0` - `9`, `a` - `z` | Direct option shortcut hotkey jump. |
| `Ctrl+Space` | Leader key chord prefix (opens which-key shortcut palette overlay). |
| `ENTER` | Execute selected item action. |
| `F1` | View Markdown user manual (`bashmenu.md`) via `bmpager --markdown`. |
| `F2` | Open Theme selector dialog. |
| `F3` | Launch built-in text editor (`bashedit`). |
| `F4` | Launch visual menu editor (`menuedit`). |
| `F5` | Full system refresh: clear command cache, reload config (`bashmenu.yml`), menus (`bashmenu.mnu`), themes (`bashmenu.themes`), wallpapers, status/help gutters, and plugins. |
| `F6` | Toggle option shortcut key badges display. |
| `F7` | Configure status gutter format modal. |
| `F8` | Launch interactive Visual Theme Editor (`themeedit`). |
| `F9` | Launch interactive Visual Keyboard Shortcuts Editor (`shortcutedit`). |
| `ESC` / `q` | Return to parent submenu or exit application. |

*Note on Intelligent State Retention:* When returning from editors, or scripts with `refresh: true`, the engine traces and preserves your submenu coordinate depth to return you to your exact location, skipping the root menu.

### 3.2 Visual Menu Editor (`menuedit`)

| Keybinding | Action |
| :--- | :--- |
| `a` / `Insert` | Add new menu option node. |
| `e` / `ENTER` | Edit selected node properties (opens `ItemEditModal`). |
| `t` | Change selected item type. |
| `SPACE` / `LEFT` / `RIGHT` | Expand or collapse submenu tree node. |
| `d` / `Delete` | Delete selected option node (with confirm box). |
| `m` | Move selected node down. |
| `M` | Move selected node up. |
| `>` | Indent item into preceding Submenu folder. |
| `<` | Outdent item out to Parent menu list. |
| `F1` / `?` | Display HA Bash Menu manual (`bashmenu.md`). |
| `F2` | Toggle (collapse/expand) all children of top root menu. |
| `F3` / `CTRL+A` | View ASCII character table. |
| `F4` / `CTRL+N` | Search Nerd Font & Emoji glyphs. |
| `F5` / `CTRL+P` | View available placeholders and macros modal. |
| `s` / `CTRL+S` | Save changes to `bashmenu.mnu`. |
| `ESC` / `q` | Exit editor (prompts if unsaved changes exist). |

*ItemEditModal Dialog Keybindings:*
| Keybinding | Action |
| :--- | :--- |
| `CTRL+S` / `F2` / `s` | Save changes to item. |
| `CTRL+T` / `t` | Change item type. |
| `CTRL+A` / `F3` | View ASCII character table. |
| `CTRL+N` / `F4` | Search Nerd Font & Emoji glyphs. |
| `CTRL+P` / `F5` | View available placeholders and macros modal. |
| `F1` / `?` | Display item property editor help modal. |
| `ESC` / `c` | Cancel changes and close dialog. |

*Note on Menu Focus Synchronization & Property Inspector Previews:* When launching `menuedit` from `bashmenu` (via `F4` or menu item action), the editor tree automatically synchronizes active keyboard focus and selection directly on the item that was selected in `bashmenu`, so pressing `e` or `ENTER` immediately opens the `ItemEditModal` dialog for that exact item. The Property Inspector side panel features a real-time visual `Preview:` section for all item types (commands, submenus, scripts, dividers, etc.) showing exactly how the item will appear in the main menu (including resolved Nerd Font/emoji icons, placeholders, and right-aligned shortcut brackets). Additionally, the `ItemEditModal` dialog dynamically updates its preview box in real-time as title or icon fields are edited.

### 3.3 Built-in Text Editor (`bashedit`)

| Keybinding | Action |
| :--- | :--- |
| `F1` | View BashEdit keybindings manual modal. |
| `F2` | Toggle whitespace characters visibility. |
| `F3` | Open CP437 / ASCII character table modal. |
| `F4` | Open 256-color palette viewer modal. |
| `F5` | View available placeholders & macros modal. |
| `F6` | View Markdown preview via `bmpager --markdown --no-file`. |
| `F7` | Toggle line numbers gutter display. |
| `F8` | View Markdown syntax quick reference modal. |
| `F9` / `Alt+W` | Toggle soft-wrap text display. |
| `Ctrl+Q` / `ESC` | Exit editor (prompts save if modified). |
| `Ctrl+O` / `Ctrl+R` | Open file via file picker modal. |
| `Ctrl+S` | Save active file. |
| `Ctrl+A` / `Ctrl+Shift+S` | Save As (prompt destination path). |
| `Ctrl+N` / `[ + ]` | Create new empty tab document. |
| `Alt+Left` / `Alt+Right` | Switch to previous / next tab. |
| `Alt+]` / `Ctrl+Tab` | Switch to next tab. |
| `Alt+[` / `Shift+Tab` | Switch to previous tab. |
| `Ctrl+W` | Where Is (search document text). |
| `Shift+Arrows` / `Shift+Home/End` | Select text range (desktop selection). |
| `Ctrl+Left` / `Ctrl+Right` | Move word left / right. |
| `Ctrl+Up` / `Ctrl+Down` | Move paragraph up / down. |
| `Ctrl+X` | Cut line or active selection block. |
| `Ctrl+C` | Copy line or active selection block. |
| `Ctrl+V` | Paste cutbuffer / clipboard text. |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo edit operations. |

### 3.4 Visual Theme Editor (`themeedit`)

The visual theme editor provides a split-screen interface featuring a collapsible token tree (`Theme:`, `Colors:`, `Borders:`) on the left and a live multi-view preview pane on the right (`Main Menu`, `Text Editor`, `Popup Windows / Modals`, `Visual Menu Editor`, and `Toast Notifications`). All category nodes start collapsed by default and open via `Left`/`Right` arrow keys, `Enter`/`Space`, mouse clicks on the category header, or clicking directly on an element in the preview pane (which auto-expands the category and focuses the matching token in the tree).

| Keybinding | Action |
| :--- | :--- |
| `UP` / `DOWN`, `k` / `j` | Navigate collapsible category nodes and tokens. |
| `LEFT` / `RIGHT`, `h` / `l` | Collapse or expand category nodes (`Theme:`, `Colors:`, `Borders:`). |
| `ENTER` / `SPACE` | Toggle selected category node, or open color/character/file picker for the selected token. |
| `[` / `]` | Cycle to previous / next theme (alphabetically sorted). |
| `TAB` / `p` / `1`-`5` | Cycle or jump between live preview views (`1` Menu, `2` Text Editor, `3` Modals, `4` Menu Editor, `5` Toasts). |
| `F4` / `b` | Choose background wallpaper image via file picker modal. |
| `F5` | Auto-derive harmonious 256-color palette (including `plugin` text color) from the configured wallpaper image. |
| `F6` / `Ctrl+N` | Create a new theme using the monochrome base palette. |
| `F7` / `F2` / `Ctrl+R` | Rename current theme inline. |
| `F8` / `Delete` | Delete current theme from `bashmenu.themes` (with confirmation modal), or clear wallpaper image when `Wallpaper Image` token is focused. |
| `Ctrl+S` | Save theme directly to `bashmenu.themes` (prompts Save As only if `untitled`). |
| `Ctrl+A` | Save As under a new theme name. |
| `Right Click` (Preview) | **Color Dropper**: Samples the visible pixel color from the clicked location in the preview pane, copies the color code to the system clipboard, and assigns it directly to the currently selected Theme Token (if it is a color setting). |
| `ESC` / `q` | Exit theme editor (prompts if unsaved changes exist). |

### 3.5 Interactive Terminal Pager (`bmpager`)

The `bmpager` standalone pager provides full-screen interactive viewing of files and piped streams with ANSI color handling, vim navigation, CommonMark markdown formatting (`--markdown`), and optional soft word wrapping (`--wrap` / `--wrap-text`).

| Option | Description |
| :--- | :--- |
| `-m`, `--markdown` | Render input formatted as CommonMark Markdown (wrapped to window width). |
| `-w`, `--wrap`, `--wrap-text` | Soft-wrap non-markdown plain and ANSI text lines to terminal width. |
| `-c`, `--no-color` | Discard ANSI escape sequences instead of rendering colors. |
| `--no-file` | Omit filename badge and disable external/built-in editor launcher. |
| `--editor=MODE` | Configure editor mode (`default` or `builtin`). |
| `+N` | Start viewing with line `N` at the top of the window. |
| `+/PATTERN` | Jump to the first line matching search pattern. |

| Keybinding | Action |
| :--- | :--- |
| `DOWN` / `j` / `ENTER` | Scroll down one line. |
| `UP` / `k` | Scroll up one line. |
| `SPACE` / `f` / `PgDn` | Scroll down one full page. |
| `b` / `PgUp` | Scroll up one full page. |
| `Ctrl+D` / `Ctrl+U` | Scroll down / up half a page. |
| `g` / `Home`, `G` / `End` | Jump to top or bottom of document. |
| `/` | Search forward for pattern. |
| `n` / `N` | Jump to next / previous search match. |
| `l` / `o` | Open interactive hyperlink and section anchor selector. |
| `e` / `E` | Open active file in configured editor (`bashedit` or `$EDITOR`). |
| `h` / `?` / `F1` | Display scrollable pager help modal. |
| `q` / `Q` / `ESC` | Exit pager. |

### 3.6 Cheatsheet Manager (`cheat`)

The `cheat` standalone CLI tool searches, lists, and displays Markdown cheatsheets based on exact path matches, frontmatter tags, or full-text content search.

| Option | Description |
| :--- | :--- |
| `[query]` | Search query to display matching markdown cheatsheet file. |
| `-l`, `--list [query]` | List all cheatsheet files with tags, or list matching files for an optional query. |
| `-s`, `--search [query]` | Search cheatsheets and list matching files with tags. |
| `-d`, `--directory` | Print the configured cheatsheets directory path. |
| `-h`, `--help` | Print help information. |

Configuration is stored in `~/.config/bashmenu/cheat.yml` (pointing to `dir: ~/cheat`). Cheatsheets are rendered via `bmpager --markdown` with raw text terminal fallback.

### 3.7 Custom Keyboard Shortcuts Editor (`shortcutedit` / `menuedit --shortcuts`)
`shortcutedit` (also accessible via `menuedit --shortcuts` or `F9` / `<leader> s` from the main menu) is a split-screen visual editor for `shortcuts.yml` (`{bashmenu_dir}/shortcuts.yml`).

- **Leader Key Chords (`<Ctrl+Space>`):** Pressing the configured `leader_key` (default `ctrl+space`) in the main menu opens a compact *which-key* popup overlay showing all active `<leader> <key>` shortcuts and waits up to `leader_timeout_ms` (`1500` ms default) for the follow-up key.
- **Direct Shortcuts:** Function keys (`F10`, `F12`) and modified combos (`ctrl+<key>`, `alt+<key>`) execute immediately with a single keystroke.
- **Full Placeholder & Execution Mode Parity:** Every shortcut supports all 6-tier macro placeholders (`{dev_dir}`, `{bashmenu_dir}`, `{scripts_dir}`, `{param:prompt=...}`, `{file_picker}`, `{dir_picker}`, `{toast:...}`) and execution modes (`console`, `streaming`, `interactive`, `quiet`, `alt_buffer`, `window_percent`).

| Keybinding | Action |
| :--- | :--- |
| `a` | Add a new keyboard shortcut. |
| `e` / `ENTER` | Edit the selected shortcut's properties. |
| `c` / `F2` | Interactively capture a direct key combo or `<leader> <key>` chord. |
| `l` | Configure global `leader_key`, `leader_timeout_ms`, and popup hint. |
| `SPACE` | Toggle the selected shortcut enabled/disabled. |
| `d` / `Delete` | Delete the selected shortcut. |
| `m` / `M` | Move the selected shortcut down / up. |
| `CTRL+P` | View Placeholders & Macros reference modal. |
| `s` / `CTRL+S` | Save `shortcuts.yml` (preserving `SHORTCUTS_HEADER`). |
| `ESC` / `q` | Exit editor. |

*Theme Renaming & Deletion:*
- **Renaming:** Themes can be renamed directly by editing the **Theme Name:** text input field in the inspector panel under **General & Identity**, or by clicking the adjacent **[Rename]** button (or pressing `Ctrl+R`), which opens the interactive `RenameThemeModal` with automatic slug sanitization.
- **Deleting:** Themes can be deleted from `bashmenu.themes` either by clicking the **[Delete]** button in the inspector panel under **General & Identity** (or pressing `Ctrl+D`), or directly from the Theme Picker modal (`F2`) using `[d]` / `Delete`. Both options present a confirmation dialog. If the theme is currently active in `bashmenu.yml`, the user is notified and the active theme is automatically switched to a fallback theme (e.g. `dracula`).

*Launching Theme Editor:*
- Directly via command line: `themeedit [theme_name]` (or `themeedit -f <themes_file> -t <theme_name>`).
- Directly inside HA Bash Menu via `F8` shortcut.
- Inside HA Bash Menu via `F2` (Theme Picker) by pressing `e` on any highlighted theme.
- Via the main menu under `Bashmenu Settings -> Theme Editor`.
- Auto-derive theme colors from wallpaper: press `<F5>` inside `themeedit` to automatically derive a harmonious 256-color palette (with 16/8-color perceptual downsampling) from the configured background wallpaper.

---

## 4. Configuration & Schema Specifications

### 4.1 Settings Store (`bashmenu.yml`)
The settings file stores nested key-value pairs used throughout the menu.
All entries can be referenced dynamically using the explicit `{app.<key>}`
placeholder namespace (e.g. `{app.theme}`, `{app.settings.tabstop}`,
`{app.user.postal_code}`) or shorthand shortcuts (`{theme}`,
`{settings.<key>}`, `{user.<key>}`).
Example:
```yaml
theme: dracula
user:
  divider:
    char: '{ascii:196}'
    length: '{window_width}'
settings:
  use_nerd_fonts: true
  tabstop: 4
  status_gutter: "{user} | {battery} | {date_time_24}"
  plugins:
    otd:
      script: otd.sh
      sleep: 300
      pretext: '{user.divider}'
      posttext: '{user.divider}'
  dns:
    ipv4:
      primary: 1.1.1.1
```
Any dot-notation key (e.g., `settings.dns.ipv4.primary`, `settings.plugins.otd.sleep`, `user.divider`, or `settings.status_gutter`) can be interpolated
inside menu titles, actions, labels, or template files using brackets.

The `settings.plugins` setting specifies plugin scripts located in the configured `scripts_dir`. Each plugin can be defined as a simple script path or a dictionary with `script`, `sleep`, `pretext`, and `posttext` options:
| Setting | Description |
| :--- | :--- |
| `script` | Script filename or command to execute. |
| `sleep` | Plugin-specific output cache duration in seconds (setting `sleep: 0` will update live data on every frame tick). |
| `pretext` | Text or placeholder rendered immediately before the plugin script output. |
| `posttext` | Text or placeholder rendered immediately after the plugin script output. |

#### Multi-Column Plugin Dashboard Layout:
Plugins can be arranged horizontally into columns spanning the full width of the interface using a declarative configuration under `settings.plugins` (configured either inside a `layout` block or directly at the top level of `plugins`):
```yaml
settings:
  plugins:
    divider: true            # Render native divider above plugin section (options: true, "screen", "window", false)
    type: table
    width: "100%"            # Automatically divides available width equally across columns (e.g. 50/50, 33/33/33)
    entries: 3               # Maximum entries per table row (default: 2)
    headers: ''              # Optional column headers (list or empty string)
    weather:
      script: "curl -s 'wttr.in?format=3'"
      sleep: 300
    uptime:
      script: "uptime.sh"
      sleep: 30
    disk:
      script: "disk.sh"
      sleep: 60
    otd:
      script: "otd.sh"
      sleep: 300
      standalone: true         # Renders full-width across the screen outside table borders
      pretext: '{divider}'
      posttext: '{divider}'
```
- **Section Divider (`divider: true` / `top_divider: true`)**: Draws a native divider line directly above the plugin section. Accepts `true` (inherits active theme width), `"screen"` (full-width with border tees `├───┤`), or `"window"` (inset with side margins).
- **Flexible Configuration Placement**: Layout options (`type: table`, `width`, `entries`, `headers`, `rows`, `divider`) can be placed directly under `settings.plugins` or nested in `settings.plugins.layout`.
- **Automatic Multi-Table Chunking (`entries: N`)**: When `entries: N` is set and `rows` is omitted, plugins are automatically grouped into N-column tables. If additional plugins are encountered, subsequent tables are constructed automatically.
- **Standalone Plugins (`standalone: true` or `span: full`)**: Full-width plugins (such as quote-of-the-day or system banners) stretch across the entire screen on their own without table borders, sitting above or below multi-column tables.
- **Asynchronous Execution**: Each plugin continues running asynchronously in its own background worker thread honoring its own `sleep` interval without blocking the user interface. Content within each cell automatically wraps cleanly when lines exceed the column width.

#### Inactivity Timeout & Screensaver (`settings.inactivity_timeout`):
The `settings.inactivity_timeout` configuration enables automated screensaver
execution or command execution when no keyboard or mouse activity is detected
for a configured duration:

```yaml
settings:
  inactivity_timeout:
    milliseconds: 300000     # Duration in milliseconds (e.g. 5 minutes)
    # command: 'cmatrix'     # Optional: command to execute (unconfigured by default)
```

| Setting | Type | Description |
| :--- | :--- | :--- |
| `milliseconds` | Integer / Float | Total inactivity duration before triggering. |
| `command` | String (Optional) | Command to execute in alternate buffer. Unconfigured by default; if omitted, timeout does nothing. |

- **Unconfigured Default**: The timeout directive does not include a command
  argument by default. If `command` is not configured, the timeout function
  does nothing.
- **Countdown Reset**: Any keyboard keystroke, mouse movement, click, or scroll
  action resets the countdown timer back to the beginning.
- **Alternate Buffer Isolation**: The command executes inside an alternate
  screen buffer (`\x1b[?1049h`), ensuring neither the underlying terminal
  command line nor the menu layout is disturbed.
- **Seamless Return**: When the screensaver command exits (e.g. via `q` or
  `Ctrl+C`), the menu interface repaints cleanly and the inactivity countdown is
  re-armed automatically.

Dividers can be defined under `user.divider` (or `settings.divider`) with full specifications including `char` (e.g., `{ascii:196}` or `-`) and `length` (e.g., `{window_width}` or `40`). Using `{user.divider}` or `{divider}` in `pretext` or `posttext` expands to a styled divider line matching the active theme's configured divider color.

The `settings.status_gutter` setting allows customization of the system badges displayed in the bottom right corner (the status gutter). This setting is a string containing text and placeholders separated by pipe symbols (`|`). Neither the status gutter nor the left-aligned help gutter ever exceeds 50% of the screen width. Badges and help action items are delimited by pipe symbols (`|`) and treated as atomic units. When text exceeds 50% of the screen width, overlapping items drop to the next line. Neither gutter ever exceeds two lines in normal mode.

#### Help Gutter Hover Expansion & Delay (`settings.help_gutter_delay`):
The bottom left help gutter displays contextual key navigation bindings and configured custom shortcuts. In normal display mode, the gutter uses at most two lines. If there are more shortcuts than fit in the default two lines, stationary mouse hover over the help gutter expands the gutter upward to display all remaining shortcuts without truncation:
- **Customizable Delay (`settings.help_gutter_delay`)**: Configures the duration in seconds of stationary mouse hover required before the help gutter expands. Setting `help_gutter_delay: 0` causes the help gutter to expand immediately whenever hovered. Configurable directly in `bashmenu.yml` or through `Options & Settings -> Bashmenu Settings -> Help Gutter Hover Delay`. Defaults to `4.0` seconds.
- **Fixed Status Gutter Border Decoupling**: When the help gutter expands upward, it operates as an independent left-side panel overlay. The status gutter top border and status badges on the right side remain strictly anchored in place and are completely unaffected by the help expansion.
- **Dynamic Unbounded Height**: The expansion routine adds as many rows as needed to display all available help tokens and shortcuts. It dynamically expands upward, covering background plugins or empty space if necessary, while stopping cleanly below the active menu items without interfering with menu item navigation or click targets.
- **Immediate Collapse on Interaction**: Any key press, mouse click, scroll, or cursor departure outside the help gutter instantly collapses the expanded gutter back to its normal height.

Available status gutter placeholders (badges) include:

| Placeholder | Description |
| :--- | :--- |
| `{user}` / `{username}` | Current system username. |
| `{host}` | System hostname. |
| `{battery}` | Current battery percentage (with a 5-second performance cache). |
| `{date_time_12}` / `{date_time_24}` | Full date and time (includes seconds). |
| `{date_time_12_short}` / `{date_time_24_short}` | Short date and time (excludes seconds; optimal for snappier performance). |
| `{time_12}` / `{time_24}` | Current time (includes seconds). |
| `{time_12_short}` / `{time_24_short}` | Short time (excludes seconds; optimal for snappier performance). |
| `{date}` | Current date (YYYY-MM-DD). |
| `{utc_seconds}` | Current epoch seconds. |

#### Status Refresh Rate & Granularity:
The main menu dynamically calculates its status refresh interval to balance real-time feedback with CPU and battery efficiency:
- **Automatic Second Precision (1.0s):** If `settings.status_gutter` contains second tokens (`{date_time_24}`, `{date_time_12}`, `{time_24}`, `{time_12}`, `{seconds}`, or `%S`), the status line refreshes every **1.0 second**.
- **Automatic Minute Precision (5.0s):** If only minute/date tokens (`{date_time_24_short}`, `{time_24_short}`, `{date}`) or static badges are used, the engine automatically throttles to **5.0 seconds**.
- **Explicit Override (`settings.refresh_interval`):** You can explicitly configure the refresh interval in `bashmenu.yml` under `settings.refresh_interval` (or `settings.status_refresh_rate`). The setting accepts seconds (e.g. `1.0`, `2.5`) or milliseconds (e.g. `1000`, `2000`):
  ```yaml
  settings:
    refresh_interval: 1.0   # Explicitly refresh status line every 1 second
  ```
  *(Note: Background plugins configured under `settings.plugins.<name>` control their own individual polling frequencies via their `sleep:` setting in milliseconds, e.g. `sleep: 10000`.)*

#### Visual Screen Transitions & Speed (`settings.transition` & `settings.transition_speed`):
HA Bash Menu features hardware-synchronized visual transitions rendered
between screens, submenus, paging jumps, and external application switches.

```yaml
settings:
  transition: fade            # Options: fade, curtain, dissolve, matrix, slide_left, slide_right, blinds, iris, wipedown, instant, none (default: fade)
  transition_speed: normal    # Options: fast, normal, slow, or milliseconds (default: normal)
```

| Transition Effect | Description |
| :--- | :--- |
| `fade` | Smooth multi-step luminance ramp-down and ramp-up with TrueColor scaling and DEC Mode 2026 synchronized output. |
| `curtain` | High-contrast horizontal curtain wipe expanding symmetrically outward from the screen center. |
| `dissolve` | Retro game style 4x4 Bayer matrix dither dissolve. |
| `matrix` | Cascading green digital rain resolving smoothly into the target view. |
| `slide_left` | Horizontal slide wipe across columns moving from right to left. |
| `slide_right` | Horizontal slide wipe across columns moving from left to right. |
| `blinds` | Venetian blinds horizontal multi-row slats opening simultaneously. |
| `iris` | Rectangular box aperture zoom expanding outward from the center. |
| `wipedown` | Vertical top-to-bottom roll wipe revealing rows from upper to lower. |
| `instant` / `none` | Immediate frame swaps without intermediate animation frames. |

##### Transition Speed Profiles (`settings.transition_speed`):
You can configure animation playback speed using preset profiles or explicit millisecond step delays:

| Speed Profile | Animation Duration | Step Interval | Recommended Use Case |
| :--- | :--- | :--- | :--- |
| `fast` | ~60–75ms | 15ms in / 12ms out | High-speed navigation and power users desiring rapid response with subtle visual motion. |
| **`normal`** (Default) | ~100–125ms | 25ms in / 20ms out | Balanced, buttery-smooth cinematic pacing across modern terminals and netbook displays. |
| `slow` | ~200–250ms | 50ms in / 40ms out | Deliberate, relaxed pacing ideal for presentations, high-contrast themes, or low-power screens. |
| `<integer>` (e.g. `30`) | Custom | User-specified ms | Explicit custom step interval in milliseconds (clamped between 5ms and 500ms). |

Visual transitions are automatically triggered:
- On initial startup launch.
- Between menu levels / submenus (entering submenus or returning via Back/Esc).
- On page jumps (`PageUp` / `PageDown`).
- Between application switches (`bashmenu` ↔ `themeedit`, `shortcutedit`, `menuedit`, `bashedit`, `bmpager`, console commands, and shell scripts).

#### Modal Popup Zoom Animations (`settings.modal_animation` & `settings.modal_animation_speed`):
Popup modal dialogs (help, theme picker, note viewer, note editor, prompts, confirmations) utilize dedicated lightweight zoom animations that rapidly grow from the initiating menu item row (or screen center for function key shortcuts) when opened, and shrink back to that launch anchor when dismissed, eliminating full-screen transition disruptions.

```yaml
settings:
  modal_animation: zoom          # Options: zoom, none (default: zoom)
  modal_animation_speed: fast    # Options: fast, normal, slow, or milliseconds (default: fast)
```

| Modal Animation Style | Description |
| :--- | :--- |
| `zoom` (Default) | Snappy animated growth from launch anchor into dialog frame, and shrink back on dismissal. |
| `none` | Immediate instant popup frame draw and dismissal without animation. |

If `modal_animation_speed` is omitted from `bashmenu.yml`, it falls back to `settings.transition_speed` (or `fast`).

*Disabling Transitions for Specific Commands:* Any command or script menu item
can disable transitions by specifying `transition: false` in `bashmenu.mnu`:
```yaml
- Fast Diagnostic:
    type: command
    action: "fast_check.sh"
    transition: false
```

#### Linux Framebuffer Graphics Device (`settings.framebuffer`):
For bare Linux Virtual Consoles (`/dev/tty1`-`/dev/tty6`, `TERM=linux`, or
`console`), HA Bash Menu includes a direct Linux framebuffer driver that renders
TrueColor 32-bit (ARGB/XRGB/BGRA) and 16-bit (RGB565) graphics directly to
`/dev/fb0` via kernel `ioctl` (`FBIOGET_VSCREENINFO`, `FBIOGET_FSCREENINFO`)
and video RAM memory mapping (`mmap`), without requiring X11, Wayland, or
external emulators.

```yaml
settings:
  framebuffer: auto   # Options: auto, image, true, false
```

| Value | Behavior |
| :--- | :--- |
| `auto` | Automatically enables TrueColor framebuffer rendering when running on Linux VT (`TERM=linux` / `console`) with accessible `/dev/fb0`, using text-based character-block wallpaper translation by default. |
| `image` | Enables full-resolution native pixel wallpaper rendering on `/dev/fb0` (equivalent to passing `--image` / `-i`). |
| `true` | Enforces direct framebuffer character and TUI rendering. |
| `false` | Disables framebuffer rendering and uses standard terminal character cells only. |

### 4.2 Menu Structure (`bashmenu.mnu`)
The menu structure is defined as a hierarchical list of dictionaries under
a main `options` list. Each option dictionary supports several attributes:

| Attribute | Description |
| :--- | :--- |
| `type` | The action option type (see Section 5 for complete list). |
| `label` | The row label string. Can use bracketed placeholders. |
| `icon` | Icon tag using `{nf:[char]:[nerd-font hex]:[emoji]}` syntax. |
| `action` | Shell command, script, or editor file path. Can use dynamic directives like `{param}`, `{file_picker}`, or `{dir_picker}`. |
| `template` | Source template path for dynamic block injection. |
| `target` | Destination path for dynamic block injection. |
| `block_id` | Unique tag for dynamic block boundary markers. |
| `stream` | `true`\|`false` (Execution Mode: real-time streamed command output window inside TUI). |
| `interactive` | `true`\|`false` (Execution Mode: suspends TUI for full-screen TTY tools without header/footer prompts). |
| `quiet` | `true`\|`false` (Execution Mode: suppresses command headers and press-ENTER prompts). |
| `no_formatting` | `true`\|`false` (disables BBCode rich formatting parser for raw plain text display). |
| `external` | `true`\|`false` (runs scripts in separate shell or process). |
| `user_mode` | `"root"`\|`"user"` (defines execution privilege levels). |
| `key` | YAML dot-notation settings key path in `bashmenu.yml`. |
| `picker` | `"file"`\|`"dir"`\|`"none"` (launches visual configuration chooser). |
| `start_dir` | Default folder path for file/directory picker modals. |
| `masked` | `true`\|`false` (masks password input fields with asterisks). |
| `show_whitespace` | `true`\|`false` (enables spaces/tabs visual indicators). |
| `tab_to_spaces` | `true`\|`false` (converts typed tab keystrokes to spaces). |
| `tabstop` | Tab indents column width as integer. |
| `on_yes` | Nested action executed when "Yes" is selected. |
| `on_no` | Nested action executed when "No" is selected. |
| `message` | Body text prompt string for toggle or confirmation popups. |
| `transition` | `true`\|`false` (enables or disables visual transition when executing item; defaults to `true`). |

### 4.3 Color Themes Specification (`bashmenu.themes`)
Color themes configure the visual palette for the main menu, visual menu editor, text editor, and modal screens. Themes are defined under top-level theme names (`dracula`, `nord`, `cyberpunk`, `gruvbox`, `qbasic`, `pacman`, `industry`, `matrix`, `monochrome`, `synthwave`, `amber_crt`, `hotdog_stand`) for `256`, `16`, and `8` color modes using single-line compact lists `[fg, bg]`.

| Supported Theme Element | Description |
| :--- | :--- |
| `background` | Window background color (`COLOR_BLACK`, int index `0-255`, or `-1`). |
| `title` | Top window title header text color. |
| `border` | Outer frame and panel border line color. |
| `text` | Regular menu item text color. |
| `highlight` | Active selection bar text and background colors. |
| `accent` | Accent highlights, path labels, and prompt titles. |
| `footer` | Help footer line color. |
| `help_text` | Dedicated help text color in the bottom gutter and modal footers. |
| `plugin` | Dedicated text color for background plugin output feeds. |
| `gutter` | Status gutter text color. |
| `selection` | Text selection highlight color. |
| `shortcut_key` | Direct selection shortcut hotkey character badge color. |
| `divider` | Horizontal divider line color. |
| `button_primary` | Modal dialog primary action button color. |
| `button_error` | Modal dialog error/destructive action button color. |
| `button_cancel` | Modal dialog cancel action button color. |
| `button_success` | Modal dialog success action button color. |
| `scrollbar` | Scroll bar thumb (foreground) and track (background) colors. |
| `whitespace` | Non-visible whitespace characters (spaces, tabs, newlines) display color in the text editor. |
| `window_close_button` | Close 'x' button in window title bars and modal dialogs. |

*Formatting Rule for Editor Compatibility:* Theme definitions maintain compact single-line flow-style bracket lists (e.g., `title: [201, -1]`). This enables `bashedit`'s color preview features and `themeedit`'s parser to accurately parse inline bracketed color values and render live color swatch previews.

#### Color Mode Hierarchy & Adaptive Fallback:
HA Bash Menu automatically detects terminal color capabilities and gracefully
degrades across four display tiers:

1. **TrueColor (24-bit / 16.7M Colors):** Active when `COLORTERM=truecolor` or
   `COLORTERM=24bit` is present, rendering wallpaper images, smooth gradient
   transitions, and TrueColor theme swatches.
2. **256-Color Mode:** Active on standard modern terminals (`xterm-256color`,
   `tmux-256color`). Reads the theme's `256:` section using xterm palette indices
   `0`-`255`.
3. **16-Color Mode (Standard ANSI):** Active on 16-color terminals and standard
   Linux Virtual Consoles (`TERM=linux`, `console`). Maps elements directly to
   the theme's `16:` palette.
4. **8-Color Mode (Basic ANSI):** Minimum fallback tier for serial consoles and
   restricted environments, utilizing the theme's `8:` palette.

All built-in themes provide explicit `256`, `16`, and `8` color blocks, ensuring
optimal contrast and full legibility without external remapping tools.

#### Window Border & Divider Customization (`window:` and `divider:`)
Themes can optionally override the outer window frame border characters using
a top-level `window:` section, and customize horizontal divider characters
using a top-level `divider:` section. Authors may specify CP437 ASCII macros (e.g.,
`{ascii:205}`, `{ascii:221}`), Unicode characters, or Nerd Font glyphs. Any omitted
keys automatically fall back to standard single-line box drawing characters.

```yaml
pacman:
  indicator: "{nf:):#f0baf:}"
  divider:
    char: "{ascii:205}"                 # ═ (Double horizontal divider)
    length: "{window_width}"
  window:
    border_horizontal_top: "{ascii:223}"    # ▀ (Upper half block)
    border_horizontal_bottom: "{ascii:220}" # ▄ (Lower half block)
    border_vertical_left: "{ascii:221}"     # ▌ (Left half block)
    border_vertical_right: "{ascii:222}"    # ▐ (Right half block)
    border_top_left: "┌"
    border_top_right: "┐"
    border_bottom_left: "└"
    border_bottom_right: "┘"
  256:
    ...
```

| Window Border Attribute | Description | Default Fallback |
| :--- | :--- | :--- |
| `border_horizontal_top` | Top horizontal frame border character. | `border_horizontal` / `─` (`{ascii:196}`) |
| `border_horizontal_bottom` | Bottom horizontal frame border character. | `border_horizontal` / `─` (`{ascii:196}`) |
| `border_vertical_left` | Left vertical frame border character. | `border_vertical` / `│` (`{ascii:179}`) |
| `border_vertical_right` | Right vertical frame border character. | `border_vertical` / `│` (`{ascii:179}`) |
| `border_horizontal` | Shorthand for both top & bottom horizontal frame borders. | `─` (`{ascii:196}`) |
| `border_vertical` | Shorthand for both left & right vertical frame borders. | `│` (`{ascii:179}`) |
| `border_top_left` | Top-left corner frame character. | `┌` (`{ascii:218}`) |
| `border_top_right` | Top-right corner frame character. | `┐` (`{ascii:191}`) |
| `border_bottom_left` | Bottom-left corner frame character. | `└` (`{ascii:192}`) |
| `border_bottom_right` | Bottom-right corner frame character. | `┘` (`{ascii:217}`) |
| `border_tee_top` / `top_tee` | Top horizontal T-junction connector character. | `┬` (`{ascii:194}`) |
| `border_tee_bottom` / `bottom_tee` | Bottom horizontal T-junction connector character. | `┴` (`{ascii:193}`) |
| `border_tee_left` / `left_tee` | Left vertical T-junction connector character. | `├` (`{ascii:195}`) |
| `border_tee_right` / `right_tee` | Right vertical T-junction connector character. | `┤` (`{ascii:180}`) |
| `border_cross` | Grid intersection cross character. | `┼` (`{ascii:197}`) |
| `gutter_border` | Horizontal border character placed above status and help gutters. | `border_horizontal_bottom` / `─` (`{ascii:196}`) |
| `gutter_separator` | Vertical separator line between help and status gutters. | `border_vertical` / `│` (`{ascii:179}`) |
| `gutter_left_tee` | Left-side T-junction connector where gutter top border meets left window border. | `left_tee` / `├` (`{ascii:195}`) |
| `gutter_right_tee` | Right-side T-junction connector where gutter top border meets right window border. | `right_tee` / `┤` (`{ascii:180}`) |
| `gutter_top_tee` | Top-side T-junction connector where vertical separator meets gutter top border. | `top_tee` / `┬` (`{ascii:194}`) |
| `gutter_bottom_tee` | Bottom-side T-junction connector where vertical separator meets window bottom border. | `bottom_tee` / `┴` (`{ascii:193}`) |
| `title_left_cap` | Decorative left cap before the title text in header border. | `[` (bracket) |
| `title_right_cap` | Decorative right cap after the title text in header border. | `]` (bracket) |
| `shadow_char` | Drop shadow shade character for modal dialogs and popups. | `░` (`{ascii:176}`) |

##### Divider Styling, Placement & Width Semantics
Divider styling (`char` glyph, `length`, and color) is defined **strictly in the themes file** (`bashmenu.themes`) as the single source of truth. Individual menu items and plugin declarations simply place a divider using `{divider}` and automatically inherit the active theme's styling.

1. **Theme Definition (`bashmenu.themes`)**: Defined under each theme's `divider:` block (specifying `char` and `length`).
2. **Default Fallback**: If an active theme does not define a `divider:` block, the engine defaults to `char: "{ascii:196}"` of length `{window_width}`.
3. **No Per-Item Style Overrides**: Declared divider options in `bashmenu.mnu` or `bashmenu.yml` do not specify `char` or `length`; their appearance is controlled globally by the active theme.

###### Divider Placement & Strict `{divider}` Recognition
- **Menu Options (`bashmenu.mnu`)**: Dividers are declared strictly using `type: "{divider}"` (or unquoted `type: {divider}`). The plain string `"divider"` is **not** recognized.
- **Divider Presence**: Dividers only appear in a menu if explicitly declared as an item (`type: "{divider}"`). If a menu contains no divider items, **no dividers are rendered** in that menu.
- **Plugins (`bashmenu.yml`)**: Plugin display areas place dividers using `pretext: "{divider}"` or `posttext: "{divider}"`.

###### Divider Widths: `{window_width}` vs `{screen_width}`
- `{window_width}`: Standard shorter divider bounded within window borders and 2-character margins (`avail_w = max(20, w - 6)`). Framed by standard vertical borders (`border_vertical_left`/`border_vertical_right`) with 2-character padding on each side, and **without** border tees.
- `{screen_width}`: Full-width divider running from left border to right border (`w - 2`), overriding all margins. Connects directly to `left_tee` (`border_tee_left`) on the left and `right_tee` (`border_tee_right`) on the right without margin spaces.

### 4.4 Wallpaper Backgrounds & Image Preparation

HA Bash Menu features a TrueColor half-block (`▀`) wallpaper compositing engine that renders graphical background images directly behind menus, options, and status lines.

#### Configuration

Wallpapers can be specified either globally in user settings (`bashmenu.yml`) or bundled with a theme in `bashmenu.themes`:

```yaml
# In bashmenu.yml (User override):
settings:
  background:
    enabled: true
    image: "{bashmenu_dir}/assets/wallpapers/twilight.png"
    opacity: 0.35    # Float between 0.0 (transparent) and 1.0 (opaque)
    scaling: "cover" # "cover", "stretch", "fit", "center", or "tile"
    clustering: 0    # Integer 0 to 128 (default: 0). Default 0 preserves full photographic fidelity.
                     # Values > 0 merge subtle color differences for high-latency connections.
    quantization: 1  # Integer >= 1 (default: 1). 1 preserves full 24-bit TrueColor depth.
```

```yaml
# In bashmenu.themes (Theme-level default):
twilight:
  theme_background:
    image: "{bashmenu_dir}/assets/wallpapers/twilight.png"
    opacity: 0.35
    scaling: "cover"
    clustering: 0
```

> [!NOTE]
> If a theme defines a `theme_background`, it displays automatically. Users can disable it or choose their own custom wallpaper at any time by configuring `settings.background` in `bashmenu.yml`.

| Background Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Master switch to enable or disable wallpaper background compositing. |
| `image` | String | (theme) | Path to wallpaper image (PNG/JPG). Supports `{bashmenu_dir}` and placeholders. |
| `opacity` | Float | `0.35` | Blending factor ($0.0$ to $1.0$). Lower values blend more theme background tint. |
| `scaling` | String | `"cover"` | Scaling mode: `"cover"` (proportional fill with auto-crop), `"stretch"` (exact fit), `"fit"` (proportional inside window), `"center"` (1:1 original size), or `"tile"` (repeat pattern). |
| `clustering` | Integer | `0` | Horizontal delta threshold ($0$ to $128$). Default $0$ preserves true photographic detail without horizontal streaking. |
| `quantization` | Integer | `1` | Color channel quantization ($1$ to $32$). Default $1$ retains full 24-bit TrueColor depth without color banding. |

---

#### Image Preparation for Optimal Performance

Terminal emulators render text cells from left to right. When adjacent cells share identical foreground and background colors, terminal rendering emits only one ANSI color escape code for the entire horizontal block. When neighboring cells have different colors (common in raw unquantized photographs), the terminal must process a full 24-bit TrueColor escape sequence for **every single character**, which can saturate terminal PTY buffers and induce scrolling or cursor latency.

BashMenu's engine includes built-in horizontal delta clustering to mitigate this, but preparing your images with proper quantization ensures the fastest, snappiest navigation possible.

For decent, but quick to create wallpapers, use an app like Krita, set the width
to 80, height to 48. Use a maximum of 16 colors. Set the opacity in Krita to
100%, turn off Pen if using a mouse (press `<f5>`, select Opacity, uncheck 'Enable Pen
Settings'). Larger sizes work, but compression will introduce artifacts. If the menu feels
sluggish with the background image, with the configured theme loaded, open the theme editor
(`F8`) and press `<F4>` (or navigate to `Wallpaper Image` under `Theme:`) to configure background
wallpaper. Try setting opacity to 0.5, clustering to 32, and quantization to 16 in `bashmenu.yml`.

##### Recommended Image Types

1. **Flat Vector & Minimalist Art (Fastest — < 3 ms redraws)**
   - Artworks featuring broad horizontal color bands, vector silhouettes, flat-color anime/landscape art, or clean skies (such as `twilight.png`) yield the fewest color escape transitions and cell modifications, providing instantaneous keyboard response.

2. **Photographs & Complex Artwork**
   - Natural photos containing fine grain or dithering stippling should be pre-processed to remove high-frequency noise and prevent Floyd-Steinberg dither stippling.

##### ImageMagick Recipes

Before using photographs as wallpapers, optimize them using ImageMagick (`convert`):

* **Clean Posterized Photo (Smooth color bands, sharp edges):**
  ```bash
  convert input.jpg \
    -resize 480x360 \
    -blur 0x1.0 \
    +dither \
    -colors 24 \
    assets/wallpapers/wallpaper.png
  ```
  - `-blur 0x1.0`: Merges camera sensor noise and fine gradient steps into uniform color blocks.
  - `+dither`: **Crucial.** Strictly disables Floyd-Steinberg error-diffusion dithering, preventing checkerboard dot stippling that forces 1-character terminal draw calls.
  - `-colors 24`: Clamps the palette to broad unified regions that collapse into long horizontal spans.

* **Oil-Painting / Illustrated Aesthetic (Preserves bold silhouettes):**
  ```bash
  convert input.jpg \
    -resize 480x360 \
    -paint 4 \
    +dither \
    -colors 16 \
    assets/wallpapers/wallpaper.png
  ```
  - `-paint 4`: Applies a Kuwahara filter to group gradients into artistic flat regions while preserving primary subject edges.

* **Retro Terminal Pixel-Art Style:**
  ```bash
  convert input.jpg \
    -resize 80x48\! \
    -blur 0x0.5 \
    +dither \
    -colors 16 \
    -scale 480x360\! \
    assets/wallpapers/wallpaper.png
  ```
  - Directly matches the terminal's native half-block character resolution (80×48), yielding minimal ANSI escape sequences and instantaneous rendering with a distinctive retro aesthetic.

##### `pngquant` Recommendations

If using `pngquant` for PNG palette reduction:
```bash
# Always pass --nofs to disable Floyd-Steinberg dithering:
pngquant --nofs 24 --speed 1 --ext -clean.png input.png
```
> [!IMPORTANT]
> Never use default `pngquant` with dithering enabled on terminal wallpapers. Floyd-Steinberg dithering scatters single-pixel dots across gradients, breaking adjacent cell matching and degrading terminal rendering throughput.

---

## 5. Complete Menu Option Types Reference

| Type | Description & Behavior | Primary Attributes |
| :--- | :--------------------- | :----------------- |
| `submenu` | Opens nested list of menu choices | `submenu: {options: []}` |
| `command` | Runs a standard shell command string | `action`, `interactive` |
| `script` | Launches an external executable script | `action`, `external` |
| `config` | Prompts user to edit setting value | `key`, `picker`, `start_dir` |
| `toggle` | Interactive True/False/Cancel modal | `key`, `message`, `title` |
| `radio` | Select single setting value from radio button list modal | `key`, `options`, `title`, `message` |
| `editor` | Opens file inside built-in text editor | `action`, `show_whitespace` |
| `notebook` | Opens dynamic notes collection menu from YAML file | `file`, `icon` |
| `confirm` | Triggers a Yes/No confirmation dialog | `message`, `on_yes`, `on_no` |
| `message` | Displays an informational pop-up dialog | `message`, `title` |
| `toast` | Displays a non-blocking floating toast notification | `message`, `seconds`, `title` |
| `python` | Executes Python script or expression | `action`, `script` |
| `inject_block` | Installs/removes modular code segments | `template`, `target`, `block_id` |
| `theme_selector`| Launches list of registered themes | *(automatic)* |
| `{divider}` | Renders an aesthetic horizontal line separator styled by the theme | *(automatic)* |
| `back` | Navigates back to the preceding menu level| *(automatic)* |
| `exit` | Shuts down the menu interface completely | *(automatic)* |

### 5.1 Notebook Menu Option (`type: notebook`)
The `notebook` option type turns a menu entry into a dynamic notes collection submenu backed by a structured YAML file.

```yaml
  - Notes:
      type: notebook
      file: "{notebooks_dir}/notes.yml"
      icon: "{nf:/:#f1a7c}"
```

#### Notebook File Resolution
When `file` is specified, the application resolves it dynamically using system placeholders (such as `{notebooks_dir}` and `{bashmenu_dir}`). If `file` is omitted or a relative path is given, the engine checks the `notebooks/` directory first (`{bashmenu_dir}/notebooks/notes.yml`), followed by the application root directory (`{bashmenu_dir}/notes.yml`). If the file does not exist, it is automatically initialized with standard metadata upon first access.

#### Notebook YAML Schema
Notebooks utilize a clean, human-readable YAML structure with note titles represented as parent mapping keys:

```yaml
notebook:
  title: Notes
  format: markdown
  notes:
    - Daily Tasks:
        type: note
        format: markdown
        content: |-
          # Daily Tasks
          - [x] Review pull requests
          - [ ] Update documentation
    - Server Config:
        type: note
        format: markdown
        content: |-
          # Server Notes
          Primary gateway IP: 192.168.1.1
```

#### Interactive Markdown Preview & Task Lists
Notes formatted in Markdown support interactive GFM task-list checkboxes (`- [ ]` and `- [x]`):
* **Keyboard Navigation:** Navigate between checkboxes using the arrow keys or `Tab`.
* **Instant Toggle:** Press `Space` or click directly on any checkbox to toggle its checked state.
* **Automatic Persistence:** Checked states are updated in the YAML file on disk immediately.

#### Help Gutter & Note Actions
* **Create Note (`[N]`):** When entering a notebook submenu, the bottom help gutter displays `[N]: New Note`. Pressing `n` or `N` prompts for a note title and opens the note editor.
* **View Note (`ENTER`):** Opens `NoteViewerModal` with wrapped, formatted Markdown or BBCode text and interactive widgets.
* **Copy Note (`[C]` / `Ctrl+C`):** Copies the raw note markdown text to the system clipboard and shared cutbuffer, enabling instant cross-interface pasting into the built-in text editor (`bashedit`), terminal prompts, or external applications.
* **Edit Note (`[E]`):** Accessible directly from the note viewer to modify raw note text using the full-screen `NoteEditorModal` editor modal.
* **Delete Note (`[D]`):** In a notebook submenu, the help gutter displays `[D]: Delete`. Pressing `d` or `D` on a selected note prompts confirmation before removing it. Outside of a notebook, `[D]` is inactive and omitted from the gutter.

#### Smart Auto-Formatting, Ghost Text & Clipboard Interoperability
* **Auto-Formatting on Enter:** When editing notes in `NoteEditorModal`, pressing `Enter` automatically continues checkboxes (`- [ ]`), unordered bullets (`- `, `* `, `+ `), numbered lists (`1. ` -> `2. `), and blockquotes (`> `). Pressing `Enter` on an empty marker clears the marker (smart de-escalation).
* **Faint Ghost Text & Tab Completion:** When starting a checkbox marker (e.g., typing `- [ `), faint ghost text appears suggesting `] ` which can be completed by pressing `Tab`.
* **Unified Cutbuffer & Clipboard:** Notes and the built-in text editor (`bashedit`) share a unified clipboard subsystem utilizing platform tools (`wl-copy`/`wl-paste`, `xclip`, `xsel`, `pbcopy`/`pbpaste`, `termux`) with seamless fallback to `~/.cache/bashmenu/cutbuffer.txt` on headless systems.

#### Swap File Recovery & 3-Way Merge Conflict Resolution
To prevent data loss from terminal crashes, system reboots, or concurrent edits, the notebook editor maintains per-note swap files stored in the same directory as the notebook:
```text
.<notebook_stem>.note.<note_slug>.swp
```
Each swap file records the editing process PID, timestamp, the base snapshot of the note when editing began, and the modified buffer.

When opening a note that has an existing swap file, or when external changes to the notebook YAML file are detected, the system presents an interactive conflict resolution modal:
* **`[Merge]` (3-Way Non-Conflicting Merge):** Automatically performs a line-level 3-way merge (`merge_lines_3way`) between the original base snapshot, external on-disk changes, and uncommitted swap buffer modifications, preserving non-conflicting edits from both sources.
* **`[Overwrite]`:** Forces saving the local swap buffer to disk, superseding external changes.
* **`[Reload]`:** Discards the uncommitted swap file and reloads the current on-disk version.
* **`[Save As]`:** Prompts for a distinct path to save the recovered swap buffer safely without altering the notebook file.

Upon normal exit after saving or explicit discarding, swap files are deleted automatically.

### 5.2 Toast Notifications (`type: toast`)
Menu items can trigger non-blocking, self-dismissing toast notifications rendered in the bottom corner of the active screen:

```yaml
- Quick Status Toast:
    type: toast
    message: "Settings saved successfully!"
    seconds: 2.5
    title: "Notice"
```

| Property | Type | Description |
| :--- | :--- | :--- |
| `message` / `text` | String | The notification message to display. Supports macro and placeholder interpolation. |
| `seconds` / `timeout` / `duration` | Float | Number of seconds before the toast self-dismisses (default: `2.0`). |
| `title` / `label` | String | Optional bold title displayed atop the toast notification. |

---

## 6. Dynamic Directives & Variables

### 6.1 Dynamic Input Directives
Dynamic input directives trigger modal dialogs when selected,
substituting the placeholder in the action string prior to execution:

| Directive | Description | Supported Attributes |
| :--- | :--- | :--- |
| `{param}` | Prompts for a single-line parameter value using a text modal. | `title` (modal header), `prompt` (body text), `masked` (mask input with asterisks) |
| `{file_picker}` | Launches a visual file chooser dialog. | `title`, `start_dir` |
| `{file_picker:path:<dir>}` | Launches file chooser defaulting to specified starting directory. | `title`, `start_dir` |
| `{file_picker_new}` / `{file_picker:new}` | Launches a visual file chooser dialog that permits creating **new files** (by pressing `n` or `N`). | `title`, `start_dir` |
| `{dir_picker}` | Launches a visual directory chooser dialog. | `title`, `start_dir` |
| `{dir_picker:path:<dir>}` | Launches directory chooser defaulting to specified starting directory. | `title`, `start_dir` |
| `{dir_picker_new}` / `{dir_picker:new}` | Launches a visual directory chooser dialog that permits creating **new folders** (by pressing `n` or `N`). | `title`, `start_dir` |

#### Picker Auto-Starting Folders (Pro Tip):
You can specify the default starting folder for file and directory pickers in two convenient ways:
1. **Inline starting path syntax**: Embed `:path:<dir>` (or simply `:<dir>`) directly into the picker directive, such as `{file_picker:path:{bashmenu_dir}/assets/wallpapers}` or `{dir_picker:~/Projects}`. This inline option takes highest precedence over the item's generic `start_dir` property.
2. **Preceding path syntax**: Precede `{file_picker}` or `{dir_picker}` with a valid folder path in the action string (e.g., `"{templates_dir}/{file_picker}"` or `"/var/log/{file_picker}"`). The engine automatically parses the prefix, validates it, and launches the visual chooser inside that directory, completely avoiding double-prefixing.

Both starting path styles function seamlessly when combined with path decomposition modifiers and file/folder creation modes (e.g. `{file_picker:path:~/Pictures:stem}` or `{file_picker_new:path:~/notes}`).

#### 6.1.1 Path Decomposition Modifiers for Pickers

When passing selected files or folders to commands, you often need to alter the extension (e.g., converting wallpapers), reference just the filename, or target the containing directory. The engine provides built-in path decomposition modifiers evaluated prior to shell execution:

| Modifier | Target Macro | Description & Example (Selected: `/home/user/Pictures/wallpaper.png`) |
| :--- | :--- | :--- |
| **`{file_picker}`** | File | Full absolute path (`/home/user/Pictures/wallpaper.png`) |
| **`{file_picker:path:<dir>}`** | File | Full path, opening picker inside `<dir>` |
| **`{file_picker:stem}`** | File | Full path without file extension (`/home/user/Pictures/wallpaper`) |
| **`{file_picker:name}`** | File | Filename only with extension (`wallpaper.png`) |
| **`{file_picker:base}`** | File | Filename only without extension (`wallpaper`) |
| **`{file_picker:dir}`** | File | Parent directory path (`/home/user/Pictures`) |
| **`{file_picker:ext}`** | File | File extension with leading dot (`.png`, or empty if none) |
| **`{dir_picker:path:<dir>}`** | Dir | Full folder path, opening picker inside `<dir>` |
| **`{dir_picker:name}`** | Dir | Folder name only without parent path (`Pictures`) |
| **`{dir_picker:dir}`** | Dir | Parent directory of the chosen folder (`/home/user`) |

*(Note: Modifiers also function seamlessly with creation mode, e.g. `{file_picker_new:stem}` or `{file_picker:new:stem}`).*

##### Practical Example: Converting Wallpapers & Changing Extensions
```yaml
- Convert Image To BM Wallpaper:
    type: command
    start_dir: '~/Pictures'
    command: 'convert "{file_picker:path:{bashmenu_dir}/assets/wallpapers}" -resize 80x48\! -blur 0x0.5 +dither -colors 16 -scale 480x360\! "{file_picker:stem}.pixel.png"'
```
If the user selects `/home/user/Pictures/twilight.jpg`, `{file_picker:stem}.pixel.png` seamlessly resolves to `/home/user/Pictures/twilight.pixel.png`.

### 6.2 System & Path Placeholders
These variables are dynamically resolved using active configuration and environment values:

| Placeholder | Description |
| :--- | :--- |
| `{user}` / `{username}` | Current system username. |
| `{host}` / `{hostname}` | System host name. |
| `{user-mode}` | Current privilege mode (`"root"` if running with root/sudo privileges, otherwise `"user"`). |
| `{version}` | Script version (e.g., `0.0.1`). |
| `{home}` | User's absolute home directory path. |
| `{bashmenu_dir}` | User configuration root directory path (`~/.config/bashmenu`). |
| `{dev_dir}` / `{repo_dir}` | Local git repository / development workspace root directory path. |
| `{templates_dir}` / `{templates}` | Templates folder path (`settings.templates_dir`). |
| `{scripts_dir}` / `{scripts}` | Scripts folder path (`settings.scripts_dir`). |
| `{cache_dir}` / `{cache}` | Cache folder path (`settings.cache_dir`, default `~/.cache/bashmenu`). Sets `$CACHE_DIR`. |
| `{notebooks_dir}` / `{notebooks}` | Custom notes and notebook collection directory path (`{bashmenu_dir}/notebooks`). |
| `{bash_aliases}` | Path to `~/.bash_aliases`. |
| `{bashrc}` | Path to `~/.bashrc`. |
| `{zshrc}` | Path to `~/.zshrc`. |
| `{profile}` | Path to `~/.profile` (falls back to existing `.bash_profile` if `.profile` is absent). |
| `{bash_profile}` | Path to `~/.bash_profile` (falls back to existing `.profile` if `.bash_profile` is absent). |
| `{zprofile}` | Path to `~/.zprofile`. |
| `{shell_profile}` | Path to active user login shell profile. |
| `{vimrc}` | Path to `~/.vimrc`. |
| `{nanorc}` | Path to `~/.nanorc`. |
| `{powershell_profile}` | Path to PowerShell `$PROFILE` script. |
| `{prefix}` | Termux / system installation prefix (`$PREFIX` or `/usr`). |
| `{termux_properties}` | Path to `~/.termux/termux.properties`. |
| `{termux_storage}` | Path to Termux `~/storage` directory. |
| `{appdata}` | Path to Windows `%APPDATA%` directory. |
| `{userprofile}` | Path to Windows `%USERPROFILE%` directory. |
| `{divider}` | Themed horizontal divider line (defaults to `{ascii:196}` of width `{window_width}`). |
| `{date_time_12}` | 12-hour formatted date-time (e.g., `2026-09-15 03:00:00 PM`). |
| `{date_time_12_short}` | Short 12-hour formatted date-time (e.g., `2026-09-15 03:00 PM`). |
| `{date_time_24}` | 24-hour formatted date-time (e.g., `2026-09-15 15:00:00`). |
| `{date_time_24_short}` | Short 24-hour formatted date-time (e.g., `2026-09-15 15:00`). |
| `{date}` | Current date formatted as `YYYY-MM-DD`. |
| `{time_12}` | 12-hour formatted time (e.g., `03:00:00 PM`). |
| `{time_12_short}` | Short 12-hour formatted time (e.g., `03:00 PM`). |
| `{time_24}` | 24-hour formatted time (e.g., `15:00:00`). |
| `{time_24_short}` | Short 24-hour formatted time (e.g., `15:00`). |
| `{battery}` | Current battery percentage (e.g., `84%`, or `N/A` if no battery is detected). Performance-optimized with a 5-second cache to prevent rendering lag. |
| `{battery_stat}` / `{battery_status}` | Current battery charging status (e.g., `Charging`, `Discharging`, `Full`, `Not charging`, or `N/A`). Performance-optimized with a 5-second cache. |
| `{theme}` | Active color theme name from `bashmenu.yml` (e.g., `tokyo_night`, `dracula`). |
| `{localip}` | Primary outbound IPv4 address (e.g., `192.168.1.50`). Evaluated on demand via UDP socket probe with TTL cache. |
| `{version}` | Application release version string resolved from workspace root `Cargo.toml` (`workspace.package.version`) or compile-time package version. |
| `{utc_seconds}` | Current UTC time in seconds since epoch. |
| `{window_width}` | Current active window width in character columns. |
| `{window_height}` | Current active window height in character lines. |
| `{ascii:decimal}` | Prints characters by their decimal code (using CP437 for extended ASCII, e.g. `{ascii:168}` resolves to `¿`). |
| `{app.<key.path>}` / `{<key.path>}` | Resolves any configuration path from `bashmenu.yml` using the explicit `{app.<key>}` namespace (e.g., `{app.theme}`, `{app.settings.tabstop}`, `{app.user.postal_code}`) or shorthand format (e.g., `{settings.cache_dir}`, `{user.editor}`). |
| `{command:shell_cmd}` | Dynamic shell command execution placeholder. Runs `shell_cmd` via system shell, sanitizes and strips trailing whitespace/newlines, and replaces the tag with the command output (supports nested braces, 30.0s cache, and 3.0s execution timeout). |
| `{toast:SECONDS:"Message"}` / `{toast:"Message"}` | Configurable non-blocking toast notification directive. Displays a brief floating toast notification on the bottom-right corner for specified seconds (defaults to 1-2 seconds if omitted) and resolves to an empty string. Supports double quotes, single quotes, or unquoted strings (e.g. `{toast: 2: "Saved"}` or `{toast: "Done"}`). |
| `{env}` / `{environment}` | Displays an environment overview table modal showing active file locations for `bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, `shortcuts.yml`, `cheat.yml`, and application version. Can also be interpolated inline as a Markdown table. |
| `{info:<path>}` | Loads a user-defined Markdown file (supporting nested placeholders such as `{info:{bashmenu_dir}/info.md}` or tilde paths `{info:~/.config/bashmenu/info.md}`), parses optional YAML frontmatter (`title`, `width` %, `height` %), expands all placeholders in the body, and displays it in a popup modal. Automatically renders a vertical scrollbar when content exceeds the modal height, supporting keyboard (`Up`/`Down`/`PgUp`/`PgDn`/`Home`/`End`) and mouse (scroll wheel and scrollbar clicks/drags) scrolling. |
| `{bashmenu_yml}` | Full canonical path to the currently loaded `bashmenu.yml` configuration file. |
| `{bashmenu_mnu}` | Full canonical path to the currently loaded `bashmenu.mnu` menu definition file. |
| `{bashmenu_themes}` | Full canonical path to the currently loaded `bashmenu.themes` theme palette file. |
| `{shortcuts_yml}` | Full canonical path to the currently loaded `shortcuts.yml` keyboard shortcuts file. |
| `{cheat_yml}` | Full canonical path to the currently loaded `cheat.yml` cheatsheet file. |
| `{color_mode}` | Current terminal color depth mode (e.g., `3-bit (8 color)`, `16 color`, `256 color`, `truecolor`). |
| `{tty_mode}` | Current TTY session mode (e.g., `virtual console (tty)`, `pty`, `serial console (tty)`). |

### 6.3 Nerd Fonts & Emoji Adaptive Resolution
The application resolves icons dynamically according to terminal features and
user configurations using three properties: a fallback ASCII character, a Nerd
Font PUA code point, and a Unicode emoji icon:

```text
{nf:[character]:[nerd-font hex]:[emoji-glyph]}
```

#### Precedence of Evaluation & Display:
The layout engine processes this tag with the following strict hierarchy:
1. **Emoji Icon (Unicode capable)**: If the system is Unicode-capable,
   `settings.use_nerd_fonts` is enabled, and `emoji-glyph` is provided,
   it is chosen and displayed.
2. **Nerd Font Glyph (Nerd Fonts enabled)**: If `settings.use_nerd_fonts`
   is `true` in `bashmenu.yml` and a valid hex code point is provided
   (e.g., `#f0a4`, `$f059f`, `0xef09`), the Nerd Font glyph is displayed.
3. **Fallback Character**: If Nerd Fonts are disabled or no icon/hex is
   defined, the defined character(s) (e.g. `{nf:#}` -> `#`) are displayed.
4. **Fallback**: If all properties are unconfigured, resolves to empty string.

#### Icon Slot Width & Menu Text Alignment:
- Every menu option icon slot occupies a standardized **4-column** visual
  display width (`vis_w + pad_w = 4`):
  - 2-column wide emojis (`🚀`, `🎮`, `📥`, `🌐`, `❓`, `🚪`): `vis_w=2, pad_w=2`.
  - 1-column glyphs and text symbols (`󰖟`, `⚙`, `ℹ`, `🌤`, `#`): `vis_w=1, pad_w=3`.
  - No icon: 4 padding spaces (`"    "`).
- Menu option names and descriptions **always** align vertically at column 11
  (`prefix_w = 10` for single-digit shortcuts), guaranteeing consistent column
  alignment regardless of whether icons are emojis, Nerd Fonts, ASCII, or absent.
- Unicode Variation Selectors (`\ufe0f`, `\ufe0e`) are automatically stripped
  prior to display measurement so that monospace font cell dimensions in terminal
  emulators match layout engine calculations.

*Theme Indicator Syntax:* Uses the exact same bracketed syntax (e.g.,
`indicator: "{nf:[char]:[nerd-font hex]:[emoji-glyph]}"` inside the theme file).

### 6.4 Console Tag Rich Formatting
The application includes a rich formatting parser allowing developers to use inline, nested BBCode-style tags throughout options, headers, status gutters, and prompts.

#### Available Formatting Tags:

| Tag | Description |
| :--- | :--- |
| `[b]text[/b]` | Renders text in **bold**. |
| `[u]text[/u]` | Renders text with an **underline**. |
| `[dim]text[/dim]` | Renders text with **dimmed** contrast. |
| `[reverse]text[/reverse]` | Renders text in **reversed** foreground/background contrast. |
| `[color=color_name]text[/color]` | Renders text in a custom theme color. |
| `[table width=100%][tr][th]Col[/th][/tr][tr][td]Val[/td][/tr][/table]` | Renders an aligned Unicode box-drawing table (`┌─┬─┐`, `│ │ │`, `├─┼─┤`, `└─┴─┘`) with auto-sized or percentage column widths (e.g. 50/50, 33/33/33), bold headers, and in-cell text wrapping. |
| `[list][*]Item 1[*]Item 2[/list]` | Renders an unordered bulleted list (`• Item 1`). Closing `[/*]` tags are optional. |
| `[list=1][*]Item 1[*]Item 2[/list]` | Renders an ordered numbered list (`1. Item 1`, `2. Item 2`). |

#### Theme Colors Available:

The `[color=...]` tag accepts active theme semantic tokens, standard/bright ANSI constants, 256-color palette indices, and 24-bit TrueColor hex codes:

##### 1. Active Theme Semantic Tokens:
| Color Identifier | Target Element / Description |
| :--- | :--- |
| `color=text` | Standard text color of the active theme. |
| `color=border` | Border frame color. |
| `color=title` | Main title header color. |
| `color=highlight` | Selected/highlighted bar colors. |
| `color=footer` | Help footer line color. |
| `color=help_text` | Dedicated help text color in bottom gutter and modal footers. |
| `color=plugin` | Background plugin feed output text color. |
| `color=shortcut_key` | Direct action shortcut character badge color. |
| `color=accent` | Accent indicator/status badge color. |
| `color=divider` | Horizontal line divider color. |
| `color=background` | Window and dialog background color. |
| `color=selection` | Active item selection highlight color. |
| `color=gutter` | Status and help gutter text color. |
| `color=button_primary` | Primary action button color. |
| `color=button_error` | Error / danger button color. |
| `color=button_cancel` | Cancel / secondary button color. |
| `color=button_success` | Success / confirmation button color. |
| `color=window_close_button` | Window title bar close button ('x') color. |
| `color=scrollbar` | Scrollbar thumb and track styling. |
| `color=whitespace` | Non-visible editor whitespace display color. |

##### 2. Named ANSI & Bright Color Constants:
| Constant | Target ANSI Color |
| :--- | :--- |
| `COLOR_BLACK`, `COLOR_RED`, `COLOR_GREEN`, `COLOR_YELLOW` | Standard 16-color ANSI primary palette (0–3). |
| `COLOR_BLUE`, `COLOR_MAGENTA`, `COLOR_CYAN`, `COLOR_WHITE` | Standard 16-color ANSI primary palette (4–7). |
| `COLOR_GREY`, `COLOR_GRAY`, `COLOR_BRIGHT_BLACK` | Bright Black / Neutral Gray (ANSI 8). |
| `COLOR_BRIGHT_RED`, `COLOR_BRIGHT_GREEN`, `COLOR_BRIGHT_YELLOW` | High-intensity bright ANSI colors (9–11). |
| `COLOR_BRIGHT_BLUE`, `COLOR_BRIGHT_MAGENTA`, `COLOR_BRIGHT_CYAN` | High-intensity bright ANSI colors (12–14). |
| `COLOR_BRIGHT_WHITE` | High-intensity bright white (ANSI 15). |
| `COLOR_ORANGE`, `COLOR_PURPLE`, `COLOR_PINK`, `COLOR_BROWN` | Extended named color definitions. |

##### 3. 256-Color Palette Indices & TrueColor Hex Values:
* **256-Color Palette:** Integer index `0` to `255` (e.g. `[color=196]bright red[/color]`, `[color=214]orange[/color]`, `[color=61]slate blue[/color]`).
* **24-Bit TrueColor Hex:** Standard HTML/CSS hex format (e.g. `[color=#ff5555]soft red[/color]`, `[color=#bd93f9]purple[/color]`).

#### Key Features:
* **Nesting Support**: Tags can be nested seamlessly (e.g. `[b]bold text [color=accent]with accented[/color] highlight[/b]`).
* **Visible Width Safety**: Centering, padding, and layout checks automatically ignore tags, ensuring pixel-perfect alignments for any styled text.
* **Code Block & Backtick Tag Suppression**: Formatting tags inside inline code backticks `` `[b]code[/b]` `` and fenced code blocks ```` ```[color=red]code[/color]``` ```` are automatically suppressed and displayed literally as plain text.
* **Backslash Tag Escaping**: Preceding a tag bracket with a backslash `\[tag]` (e.g., `\[b]` or `\[/b]`) escapes the formatting parser, rendering the literal bracketed tag text.
* **`no_formatting` Directive / Flag**: Dialog screens and formatting conversion utilities accept an optional `no_formatting: bool = False` parameter to disable tag processing entirely when displaying raw unformatted text.

---

## 7. Advanced Integration Use Cases

### 7.1 Modular Block Injection (`type: inject_block`)
Managing templates injected cleanly inside shell profiles (`~/.bashrc`, etc.):
```yaml
- label: Configure Sudo Prompt
  type: inject_block
  template: "{templates_dir}/sudo_lecture.tmpl"
  target: "/etc/sudoers.d/lecture"
  block_id: "sudo_lecture_setting"
  user_mode: "root"
```
The template is injected wrapped in distinct boundaries:
```bash
# CODEBLOCK:sudo_lecture_setting:START
# (Injected template settings here)
# CODEBLOCK:sudo_lecture_setting:END
```
Selecting **Yes** installs or updates the settings block; selecting **No**
uninstalls (erases) the code block cleanly from the target file.

### 7.2 Subprocess Execution Modes (`execution_mode`, `stream`, `interactive`)
HA Bash Menu supports three distinct subprocess execution modes for commands and scripts:
1. **Console Mode (`execution_mode: console` / default):**
   Suspends the TUI raw mode and executes directly in the system terminal.
   - `alt_buffer: true` (default): Runs in the terminal's alternate screen buffer, preserving underlying terminal lines.
   - `quiet: true`: Suppresses the "Press [ENTER] to return to menu..." prompt upon completion, returning immediately to the menu.
2. **Streaming Output Mode (`stream: true` or `execution_mode: streaming`):**
   Executes the command in the background and streams standard output and standard error in real-time into an active `StreamOutputModal` dialog window inside the TUI.
   - `window_percent: N`: Configures the floating modal dialog size as a percentage of the terminal window (e.g. `window_percent: 80`; clamped between 20% and 100%, defaults to 65%).
3. **Interactive Mode (`interactive: true` or `execution_mode: interactive`):**
   Launches an interactive child terminal session with piped standard input, allowing live user interaction within the streaming modal view.

### 7.3 Multi-Platform Support & Environment Detection
HA Bash Menu incorporates an environment detection engine in `bashmenu.sh` enabling graceful degradation and multi-platform compatibility without manual configuration.

- **Universal Shebangs:** All shell scripts and extension utilities leverage `#!/usr/bin/env bash` for maximum portability.
- **Cross-Platform Package Wrappers:** By routing operations through wrappers like `scripts/system_update.sh`, the engine automatically translates dependency installs to the correct local package manager (`apt`, `dnf`, `pacman`, `pkg`, or `brew`), and smartly adds or omits `sudo` depending on whether it is running in standard Linux, macOS, WSL, or containerized user-spaces like Termux.
- **Graceful Degradation:** Features dependent on low-level system daemon frameworks (e.g., `systemd-resolved` DNS modification) check for Termux/WSL and fail gracefully, rather than crashing with environment errors.
- **Direct Linux Framebuffer Support (`/dev/fb0`):** When executed on raw Linux Virtual Consoles (`/dev/tty1`-`/dev/tty6`, `TERM=linux`, or `console`), HA Bash Menu can directly access the Linux framebuffer device without external emulators (such as `fbterm` or `kmscon`). By directly querying variable and fixed screen info (`FBIOGET_VSCREENINFO`, `FBIOGET_FSCREENINFO` ioctls) and memory mapping (`mmap`) the video buffer, the engine renders TrueColor 32-bit (ARGB/XRGB/BGRA) and 16-bit (RGB565) background graphics directly on minimal netbooks and embedded Linux devices. In accordance with project architecture invariants, direct framebuffer rendering is completely self-contained within native Rust crate modules and requires zero external tool installations.

### 7.4 Background Footer Plugins Architecture & Creation Guide

HA Bash Menu features an asynchronous, background-cached plugin architecture for displaying live feeds, status information, or quotes directly above the help footer.

#### How Background Plugins Work:
1. **Asynchronous Execution & Worker Threads**: Plugin scripts run in detached background worker threads (`std::thread::spawn`) with thread-safe `Arc<RwLock<...>>` memory caching and channel communication. This ensures network requests or slow command calls never freeze or lag the main TUI rendering loop.
2. **In-Memory Caching & Sleep Interval**: The output of each plugin script is cached in memory. The `sleep` setting (default: `300` seconds) controls how long cached output remains valid before spawning a background refresh thread. Setting `sleep: 0` forces live execution on every frame tick.
3. **Responsive Vertical Layout Priority**: Menu navigation options hold top rendering priority. The engine calculates available screen space between the menu options list and help footer (`max_plugin_rows`). On constrained terminal windows, plugin output lines are automatically wrapped, truncated, or hidden so menu navigation remains unhindered.
4. **Pretext & Posttext Wrapping**: Optional `pretext` and `posttext` strings can be rendered before and after script output. These support full placeholder interpolation (including `{user.divider}` or `{divider}` for horizontal rule lines).

#### Creating & Registering a Custom Plugin:

1. **Create the Plugin Script**:
   Write a shell script or executable program placed in `scripts_dir` (e.g. `scripts/my_plugin.sh`):
   ```bash
   #!/usr/bin/env bash
   # Example: scripts/my_plugin.sh
   UPTIME=$(uptime -p | sed 's/up //')
   echo -e "[b]System Uptime:[/b] [color=accent]${UPTIME}[/color]"
   ```
   *Note:* The script can output one or multiple lines, and may include rich BBCode formatting tags (e.g. `[b]`, `[color=accent]`) and placeholders.

2. **Register in `bashmenu.yml`**:
   Add the plugin definition under `settings.plugins` in `bashmenu.yml`:
   ```yaml
   settings:
     plugins:
       uptime_feed:
         script: "my_plugin.sh"
         sleep: 60
         pretext: '{user.divider}'
         posttext: '{user.divider}'
   ```

3. **Supported Plugin Definition Formats**:
   - **Simple String**: `my_plugin: "my_plugin.sh"` (uses default `sleep: 300`).
   - **Full Dictionary**:

     | Property | Description |
     | :--- | :--- |
     | `script` (or `command`, `cmd`, `path`, `file`) | Filename inside `scripts_dir`, absolute path, or shell command string. |
     | `sleep` | Cache duration in seconds (`0` for live refresh). |
     | `pretext` | Header text/divider rendered before script output. |
     | `posttext` | Footer text/divider rendered after script output. |
