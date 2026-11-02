---
name: bashmenu-rs-developer
description: Core developer guidelines for the multi-platform BashMenu Rust
  (bashmenu-rs) project. Use when modifying Rust crates, Cargo workspaces,
  shell scripts, menu configurations, themes, or writing tests.
---

# Skill Name: BashMenu Rust Core Developer

## 1. Intent, Scope & Boundaries
- **Intent:** Activate when developing, refactoring, or writing tests for Rust crates (`crates/bashmenu-core`, `crates/bashmenu-tui`, `crates/bashmenu-cli`, `crates/menuedit-cli`, `crates/themeedit-cli`, `crates/bashedit-cli`, `crates/ymlcheck-cli`), shell scripts (`bashmenu.sh`, `install.sh`, `scripts/*.sh`), configurations (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`), or documentation.
- **Can:** Update cross-platform Rust/Bash code, Yaml configs, Ratatui TUI layouts, unit and integration tests in `crates/*/tests/` or `tests/`, and man/markdown documentation.
- **Cannot:** Introduce syntax/compile errors, uncaught `unwrap()`/`expect()` in runtime paths, or broken builds; guess paths or implementation details (ask for clarification); hardcode absolute user paths (always use expanding macros: `{home}`, `{bashmenu_dir}`, `{scripts_dir}`, `{templates_dir}`, `{cache_dir}`); bind `<F11>` (reserved by window managers for fullscreen; use `<F12>` or alternate unreserved keys).

---

## 2. Core Development Workflow & Invariants
- **Mandatory Git Commit & Push on All Changes:**
  - Every completed task, bug fix, feature addition, or refactoring step must conclude by creating a concise, descriptive git commit (`git commit -m "..."`) and pushing directly to GitHub (`git push origin main`).
  - Never leave changes uncommitted or unpushed in the local repository.
  - After pushing to `origin/main`, inspect or verify GitHub Actions CI when appropriate using GitHub CLI (`gh run list`).
- **Sequential TODO Execution & Lifecycle Invariant (CRITICAL):**
  - Unless specifically labeled (`CRITICAL`) or explicitly instructed otherwise by the user, always tackle items from project TODO lists (`~/.config/bashmenu/notebooks/notes.yml` under `'TODO (Agent: Allowed)'` or `.agents/skills/bashmenu-rs-developer/TODO.md`) strictly in sequential order, top to bottom.
  - **Add Ad-Hoc Instructions:** If given an actionable instruction by the user that is not yet in the TODO list, you MUST first add it to the TODO list (`~/.config/bashmenu/notebooks/notes.yml` under `'TODO (Agent: Allowed)'`) before starting work.
  - **Mark Active Tasks `PENDING`:** Before starting work on any TODO item, you MUST immediately mark it with `PENDING:` directly after the checkbox (`- [ ] PENDING: <task>`).
  - **Complete & Log to `DONE.md`:** When a task is completed (including verification, commit, push, and remote CI verification), you MUST remove the `PENDING:` label, mark the task as complete (`- [x]`), and append a summary of the completed task to `.agents/skills/bashmenu-rs-developer/DONE.md`.
  - **Prune Completed Tasks & Re-Check:** When there are no more tasks left to complete, remove finished (`- [x]`) tasks from the TODO list. Finally, when all tasks are completed, you MUST re-check the TODO list (`~/.config/bashmenu/notebooks/notes.yml` and `.agents/skills/bashmenu-rs-developer/TODO.md`) to ensure no new tasks were added while you were working.
- **Configuration Header Preservation Invariant (`bashmenu.yml`):**
  - Never use raw file writes with `serde_yml::to_string()` directly to overwrite `bashmenu.yml`.
  - Always call `bashmenu_core::config::save_config(path, config)` so that `CONFIG_HEADER` comments, documentation, and placeholder references are cleanly preserved at the top of the file.
- **Verification, Linting & Formatting:**
  - Format checking: `cargo fmt --all --check`
  - Clippy linting: `cargo clippy --workspace --all-targets -- -D warnings`
  - Test verification: `cargo test --workspace`
  - Shell script linting: `shellcheck *.sh scripts/*.sh`
- **GitHub Actions CI Workflow & Remote Test Verification:**
  - Automated testing and linting is offloaded to GitHub Actions:
    - Rust build, test & clippy: `.github/workflows/rust-app.yml` (stable Rust toolchain, x86_64 and aarch64 targets).
    - Shell script linting: `.github/workflows/shellcheck-app.yml`.
  - When testing changes, commit and push to `origin/main`, then track remote CI using GitHub CLI:
    - Query run ID: `gh run list --workflow=rust-app.yml --limit 1 --json databaseId -q '.[0].databaseId'` (or `--workflow=shellcheck-app.yml`)
    - Inspect failures: `gh run view <ID> --log-failed`
    - Watch run: `gh run watch <ID>`
    - Invariant: Never run `gh run view` or `gh run watch` without an explicit run ID as interactive prompts hang non-interactive agent shells.
- **Error Handling & Code Safety Guidelines:**
  - Treat all Clippy warnings as compiler errors (`-D warnings`).
  - **No Naked Panics in Production Code**: Avoid `.unwrap()` and `.expect()` in interactive runtime loops, macro evaluation, or file parsing. Return strongly-typed errors using `thiserror` (e.g. `BashMenuError`) or propagate via `anyhow::Result` / `color_eyre::Result` at the binary boundary.
  - **Terminal Panic Hook Invariant**: Always register a global panic hook (`std::panic::set_hook`) before initializing the terminal raw mode. If a panic occurs, the hook must immediately disable raw mode (`crossterm::terminal::disable_raw_mode()`), leave alternate screen (`LeaveAlternateScreen`), and show the cursor (`Show`) before printing the panic message, preventing corrupted terminal states.
- **Directory Invariants:**
  - Rust Crates: Modular code resides under `crates/` in the Cargo workspace:
    - `crates/bashmenu-core`: Shared domain logic, YAML configuration parsing, macro expansion, plugin runner.
    - `crates/bashmenu-tui`: Reusable Ratatui widgets, custom borders, drop-shadow frames, 2D pickers, modal screens.
    - `crates/bashmenu-cli`: Main executable binary (`bashmenu`).
    - `crates/menuedit-cli`: Visual menu tree editor executable (`menuedit`).
    - `crates/themeedit-cli`: Visual theme and palette editor executable (`themeedit`).
    - `crates/bashedit-cli`: Full-screen terminal text editor executable (`bashedit`).
    - `crates/bmpager-cli`: Standalone native terminal pager with CommonMark markdown and OSC 8 hyperlink rendering (`bmpager`).
    - `crates/ymlcheck-cli`: Schema validator and linter executable (`ymlcheck`).
  - Tests: Unit tests are colocated in modules (`#[cfg(test)] mod tests`); integration tests reside in `crates/<crate>/tests/` or top-level `tests/`.
  - Extension Scripts: Standalone games, plugins, and helper tools must reside in `scripts/`, never in root. Menu actions reference them via `{scripts_dir}/<name>`.
  - Notebooks: User notebooks and notes must reside in `notebooks/` (`{notebooks_dir}` / `{notebooks}`).
- **Dependency Hygiene:** Keep external crate dependencies lean. Prefer pure Rust crates (`image` without C dependencies, `serde_yml`, `crossterm`, `ratatui`, `unicode-width`, `ropey`, `thiserror`). Avoid pulling heavy, slow-to-compile framework dependencies when standard library primitives or lightweight crates suffice.
- **Sub-Agent Delegation Policy:** Use local `opencode` CLI sparingly and strictly for small, low-risk draft tasks (test case drafts, isolated prototypes). Treat all output as untrusted draft code requiring strict review, linting, and automated test validation.
- **Branching Discipline:** For structural overhauls (cross-platform compatibility, editor re-architecture, dependency reduction), always branch from git (e.g. `feat/<topic>`) and verify all tests and linters pass before merging.
- **Ratatui UI Performance & Responsiveness Invariants:**
  - **Immediate-Mode Buffer Rendering:** Ratatui renders directly into a double-buffered cell grid (`Buffer`). Diffing happens at the cell level, writing only changed cells to stdout. Never concatenate entire screen strings every frame.
  - **Zero-Delay Mouse Interaction:** Handle `MouseEventKind::Down(MouseButton::Left)` directly rather than waiting for `MouseUp` or `Click` so users get instantaneous visual feedback without perceptible latency.
  - **Instant Scrolling & Zero-Delay Focus:** Set viewport offsets directly without artificial transition animations.
  - **Pre-Baked / Cached Background Compositing:** In live preview widgets and main menus, cache base composited cell buffers. On cursor movement or highlight flashes, modify only the affected row cells rather than re-rendering background wallpapers or full canvas structures.
  - **Universal Selection & Clipboard (`bashmenu_tui::clipboard`):** All single-line (`TextInput`) and multi-line text inputs (`bashedit`, `NoteEditorModal`, `menuedit` multi-line fields) must support `Shift + Left/Right/Up/Down/Home/End` selection anchoring and `Ctrl+C`, `Ctrl+V`, `Ctrl+X`.
- **Inline Macro Parameter Syntax & Help Parity:**
  - Macro placeholders accept colon-separated inline configurations:
    - `{dev_dir}` (dynamic repository root path), `{bashmenu_dir}` (`~/.config/bashmenu`), `{utc_seconds}` (epoch seconds), `{command:<cmd>}` / `{cmd:<cmd>}` (cached shell stdout, cleared via `clear_command_cache()` on `<F5>`).
    - `{file_picker}` / `{dir_picker}` with `:new`, `:ext`, `:path`, `:dir` modifiers and tilde expansion.
    - `{param}` and its legacy alias `{prompt}` (interactive input modal; strips `[...]` bracket suffixes from item labels when deriving modal titles unless `title:` is explicitly set).
    - `{toast:3:"Message"}` (non-blocking toast notification).
  - When introducing or extending inline placeholder parameters, ensure full documentation parity:
    1. Update the Placeholders and Macros reference modal in `bashmenu-tui`.
    2. Update the manpage (`bashmenu.1`) and markdown guide (`bashmenu.md`).
    3. Ensure 6-tier variable interpolation (env, runtime, bashmenu config, system, theme, inline) functions correctly with nested placeholders.
- **Editor Swap File Placement Invariant (`bashedit`):**
  - Built-in text editors must colocate temporary swap files (`.<filename>.swp`) directly in the exact directory of the file being edited.
  - Never store editor swap files in `/tmp` or user root; swap files must be cleanly deleted on save or exit, and detected on launch for crash recovery.

---

## 3. Project Architecture & Components
| File / Directory | Description |
| :--- | :--- |
| `Cargo.toml` | Top-level Cargo workspace definition declaring member crates. |
| `crates/bashmenu-core` | Core logic: YAML parsing, Serde schemas, 6-tier macro interpolation, plugin scheduler. |
| `crates/bashmenu-tui` | Reusable Ratatui UI primitives: drop-shadow frames, modals, 2D pickers, BBCode/ANSI parser, theme styling. |
| `crates/bashmenu-cli` | Primary binary (`bashmenu`): main menu event loop, item dispatcher, subprocess supervisor. |
| `crates/menuedit-cli` | Visual tree editor binary for `bashmenu.mnu` (invoked via `F4` or standalone). |
| `crates/themeedit-cli` | Visual split-screen theme editor binary with 2D color and CP437 ASCII pickers (invoked via `F8` or standalone). |
| `crates/bashedit-cli` | Standalone terminal text editor binary using `ropey` text buffer and swap-file management. |
| `crates/ymlcheck-cli` | Schema validation and linting tool for `bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`. |
| `bashmenu.sh` | POSIX trampoline wrapper that executes the compiled binary, checking terminal capabilities and paths. |
| `bashmenu.yml` | Main Yaml configuration file (active theme, plugin layout, timeouts). |
| `bashmenu.mnu` | Yaml menu hierarchy using label-as-key `items:` schema. |
| `bashmenu.themes`| Theme palette, window borders, and divider definitions. |
| `scripts/` | Extension scripts (`cheat.sh`, `rich.sh`, `pager.sh`, `webopen.sh`, plugins, games). |
| `templates/` | Generic shell RC templates (`.bashrc`, `.vimrc`) free of personal hostnames, IPs, or absolute paths. |
| `tests/` | Workspace integration tests. |
| `install.sh` / `uninstall.sh` | POSIX `/bin/sh` installer and uninstaller building with `cargo build --release` or downloading binaries. |

---

## 4. Menu UI & Layout Invariants
- **Margins & Inner Buffer Clamping:** Left/right margins are exactly 2 characters; top margin is 2 lines. Treat rows as an inner buffer bounded to `avail_w = window_width - 6`. Content is truncated and padded to `avail_w` before attaching borders (`"│  "` and `"  │"`), preventing wrapped borders.
- **Icon Slot Width & Text Alignment Invariant:** Every option icon slot must occupy exactly **4 columns** (`vis_w + pad_w = 4`):
  - 2-column emojis (`🚀`, `🎮`, `🌐`): `vis_w = 2, pad_w = 2` (`format!("{icon}  ")`).
  - 1-column glyphs/symbols (`󰖟`, `⚙`, `ℹ`, `#`): `vis_w = 1, pad_w = 3` (`format!("{icon}   ")`).
  - No icon: exactly 4 spaces (`"    "`).
  - Use `unicode_width::UnicodeWidthStr::width()` to measure visible terminal column width.
  - Option descriptions strictly align at column 11 (`prefix_w = 10` for single-digit shortcuts).
- **Adaptive Glyph Resolution (`{nf:[char]:[nerd]:[emoji]}`):** Strict 4-tier resolution hierarchy:
  1. *Emoji tier*: Selected if defined and `settings.use_nerd_fonts: true`.
  2. *Nerd Font tier*: Selected if hex token defined (`#`, `$`, `0x`) and Nerd Fonts enabled.
  3. *Character tier*: Selected if defined or if Nerd Fonts disabled.
  4. *Fallback*: Empty string if unconfigured.
- **Variation Selector Normalization:** Always strip Unicode Variation Selectors (`.replace('\u{fe0f}', "").replace('\u{fe0e}', "")`) prior to cell width measurement and rendering to prevent cursor drift and border overflow.
- **Status & Help Gutter Constraints:**
  - 50% Width Limit: Neither status gutter (right) nor help gutter (left) may exceed 50% of available width (`max_gutter_w = std::cmp::max(5, avail_w / 2)`).
  - Full-Width Editor Help Gutters: In standalone editors (`bashedit`, `menuedit`, `themeedit`), help gutters span the full inner width across up to 2 lines above the bottom border using `wrap_spread_help_gutter` (treating `|` as a word delimiter).
  - Connected Panel Titles & Scrollbars: Panel titles format as `┌─[ Title ]───`. Scrollable panes reserve 1 row above the bottom border and display a scrollbar 1 column left of the right border when lines exceed viewport height.
  - Pipe Delimiter Wrapping: Items between `|` are treated as atomic badges. Status gutter items wrap bottom-up (overflow on line above).
  - Max Height: Maximum 2 rows (`max_lines = 2`); excess is discarded.
  - Sizing Formula: `gutter_rows = std::cmp::min(2, std::cmp::max(1, std::cmp::max(help_lines.len(), status_lines.len())))`; `total_content_rows = std::cmp::max(1, h.saturating_sub(4 + gutter_rows))`. Total rendered lines strictly equals terminal height `h`.
- **Plugin Buffer & Section Dividers:**
  - Plugin output is rendered into an independent area at least 1 line above the gutter, dynamic height up to 10 rows, matching the theme background.
  - Top divider (`divider: true` / `top_divider: true`): Deducts 1 row from `visible_option_rows` so scrolling never overlaps the divider.
  - With `{screen_width}` dividers, the plugin section expands to `std::cmp::max(20, w.saturating_sub(2))` at `x = 1`, and text lines indent 2 spaces (`"  "`) for menu column alignment.
- **50/50 Dual Marquee Partition Contract:**
  - **With Right-Aligned Bracket Directives (`[...]`):**
    - Available line width (`avail_w`) is strictly split 50/50 around the center:
      - Left partition: constrained from `0` to `center - 2` (`left_part_w = std::cmp::max(prefix_w + 1, center.saturating_sub(2))`).
      - Center margin: exactly 4 spaces (`"    "`) between `center - 2` and `center + 2`.
      - Right partition: constrained from `center + 2` to `avail_w` (`right_part_w = std::cmp::max(3, avail_w.saturating_sub(center + 2))`), right-aligned.
    - *Unselected Items:* Overflowing text in either partition is truncated with a Unicode ellipsis (`…`).
    - *Selected Items:* If the left label, right bracket, or both overflow their respective partitions, marquee scrolling activates simultaneously on both sides after a brief pause delay (4 ticks / 1.0 second).
  - **Without Right-Aligned Bracket Directives:**
    - The option text may utilize the entire available width up to `avail_w` (`label_avail_w = std::cmp::max(5, avail_w.saturating_sub(prefix_w))`).
    - *Unselected Items:* Overflowing text truncates with an ellipsis (`…`).
    - *Selected Items:* Overflows marquee across the full available row width.
  - **Marquee Tick Rate & Inactivity:**
    - Main loop timer ticks every 250ms (4Hz).
    - Reset ticker offset to 0 and restore 4-tick pause whenever row selection shifts.
    - Pause marquee updates completely while any modal dialog or secondary screen is displayed.
- **Toast Notification Rack Standards:**
  - Docked at bottom-right corner (`max_w = (avail_w / 2).clamp(28, 60)`).
  - Left boundary uses `▌` (`\u{258c}`) styled according to severity (`#4ebf71` Info, `#ffb86c` Warning, `#ff5555` Error).
  - Interpolates nested placeholders recursively before mounting.
  - Auto-expires on timeout; mouse click anywhere in bounding box dismisses instantly.

---

## 5. Themes, Window Borders & Palette Standards
- **Theme Selection, Alphabetical Ordering & Conversion:** Selecting a theme updates `theme:` in `bashmenu.yml`. Theme pickers (`ThemePickerModal`), theme cycling, and `save_themes()` must sort theme names alphabetically (case-insensitively). Convert theme color tokens (names, hex, 8-bit ANSI indices 0–255) into `ratatui::style::Color` variants (`Color::Rgb`, `Color::Indexed`, `Color::Reset`). Plugin rows render using `ResolvedThemeStyles.plugin` (`plugin` token, editable as `Plugin Text Color` in `themeedit`).
- **Theme Editor Lifecycle & Wallpaper Color Derivation (`themeedit`):** Supports `<F2>` Switch, `<F3>` New (`Theme::monochrome()`), `<F4>` Delete, `<F5>` Auto-Derive Colors from Wallpaper (`derive_colors_from_wallpaper`, clearing `menu_editor`/`editor`/`modals` overrides), `<F6>` Rename, `<Ctrl+S>` Save, and `<Ctrl+A>` Save As. Selecting a token flashes preview spans (`get_token_preview_spans`), clicking the preview focuses the token (`hit_test_preview`) with a toast notification, and wallpaper compositing applies across all preview modes (`Menu`, `TextEditor`, `Modals`, `MenuEditor`, `Toast`).
- **Depth Constraints & Aesthetics:** Run `ymlcheck --themes bashmenu.themes` to validate schemas. 16-color mode indices must stay within `0..15` (or `COLOR_*`), 8-color within `0..7`. Maintain cohesive, low-fatigue palettes (e.g. `tokyo_night`, `nord`, `everforest`).
- **Window Borders (`window:` in `bashmenu.themes`):**
  - Supported keys: `border_horizontal_top`, `border_horizontal_bottom`, `border_vertical_left`, `border_vertical_right` (fallbacks: `border_horizontal`, `border_vertical`), corners (`border_top_left`, `border_top_right`, `border_bottom_left`, `border_bottom_right`), tees (`left_tee`, `right_tee`, `top_tee`, `bottom_tee`), caps (`title_left_cap`, `title_right_cap`), and `shadow_char`.
  - Pass custom border characters to Ratatui's `ratatui::widgets::block::BorderType::Custom` or custom block renderers.
- **Dividers (`divider:` in `bashmenu.themes`):**
  - Theme-controlled: Defined strictly in `bashmenu.themes`. Fallback is `char: "{ascii:196}"` and `length: "{window_width}"`.
  - Strict Syntax: Menu items must declare `type: "{divider}"` (or unquoted `{divider}`). Plain `"divider"` is invalid. Dividers only appear if declared in `bashmenu.mnu` or plugin config.
  - Width Modes:
    - `{window_width}`: Inset within borders and 2-space margins (`avail_w = std::cmp::max(20, w.saturating_sub(6))`), framed by standard vertical borders without tees.
    - `{screen_width}`: Full-width border-to-border (`w.saturating_sub(2)`), connects directly to `left_tee`/`right_tee` without margin spaces. Foregrounds must match border color.
- **Theme-Configurable Marquee (`marquee:` in `bashmenu.themes`):**
  - Supported keys under `marquee:`:
    - `char`: Separator string or character repeated between cycles of scrolling text (e.g. `"~"`, `"•"`, `"»"`, `"·"`, `"-"`, `" "` or Nerd Font glyphs).
    - `gap`: Number of times `char` is repeated to form the spacer between loop cycles (integer >= 0, default `5`).
  - Fallback Standard: If a theme omits the `marquee:` section or if an unknown theme is queried, fall back strictly to 5 bullets (`char: "•"`, `gap: 5`).
- **CP437 ASCII Macro Interpolation Invariant (`{ascii:NNN}`):**
  - Theme properties for borders, indicators, and marquee characters may specify CP437 byte codes using `{ascii:NNN}` macros (e.g. `{ascii:216}`, `{ascii:196}`).
  - Any code reading theme glyphs must pass the string through CP437 byte translation (`parse_ascii_val` or regex `r"\{ascii:\s*(\d+)\s*\}"`) before displaying or formatting.
  - Raw `{ascii:NNN}` strings must NEVER appear in the user-facing menu options, indicators, or status lines.
- **Theme Renaming, Deletion & Direct Save Invariants (`themeedit`):**
  - **Direct Save Invariant (<kbd>Ctrl+S</kbd>):** If `theme_name` is set to any valid name (existing or newly named), <kbd>Ctrl+S</kbd> must immediately serialize and save to disk. It must NEVER divert to "Save As" unless the theme is truly untitled (`theme_name.is_empty() || theme_name == "untitled"`). Only <kbd>Ctrl+A</kbd> opens "Save As".
  - **Deletion:** Themes can be deleted from `bashmenu.themes` via <kbd>Ctrl+D</kbd> or <kbd>Delete</kbd>. Prompts confirmation via `ConfirmModal`. Invariants: Never allow deleting the only remaining theme; if deleting the active theme, safely fall back to another theme (e.g. `dracula`) and update `bashmenu.yml`.

---

## 6. Menu Definitions & Visual Editors (`bashmenu.mnu`, `menuedit`)
- **Menu Schema (`items:`):**
  - Use `items:` and label-as-key structure:
    ```yaml
    items:
      - '{divider}':
      - Applications:
          icon: '{nf:󰖟}'
          type: submenu
          submenu:
            title: Applications
            items:
              - Web Browser:
                  action: '{scripts_dir}/webopen.sh'
    ```
  - Lean Defaults: Prune default flags (`stream: false`, `masked: false`, `show_whitespace: false`, `refresh: false`, `no_formatting: false`, `external: false`, `alt_buffer: false`, non-editor `tabstop`, duplicate `command` keys).
  - Centralized Dump: Use `bashmenu_core::menu::dump_menu_yaml()` with 2-space indented sequences.
- **Visual Menu Editor (`menuedit`):**
  - Dynamic Type-Filtered Property Inspector: `ItemEditModal` filters visible controls by item type:
    - `command` / `script`: 3-way Execution Mode (`Console`, `Streaming`, `Interactive`), `Window Size %` (`StreamOutputModal`), `Alt Buffer`, and `Quiet Mode` (`quiet: true` suppresses `"Press [ENTER] to return to menu..."`). Legacy `interactive: true` defaults to `Console` mode unless `execution_mode: interactive` is explicitly set.
    - `confirm`: `on_yes:` and `on_no:` action inputs with `[ Browse... ]` pickers, rendered at runtime via a 3-button `ConfirmModal` (`Yes`, `No`, `Cancel`).
  - Selection Synchronization (`<F4>`): Launching `menuedit` from `bashmenu` passes `--select-path`, `--select-title`, and `--title-chain` to focus the active item automatically, and uses `ToastRack` notifications for status events.
  - Type Mutation (<kbd>Ctrl+T</kbd> / <kbd>t</kbd>): Reconfigure item types post-creation. Migrate payloads to target canonical keys (`command`, `script`, `editor` -> `file`, `config`/`toggle`/`radio` -> `key`, `python`) and prune obsolete keys. Submenu transitions handle container initialization and tree refresh.
  - Help Bar Footer: 2-line vertical container with two 1-line centered horizontal rows:
    - Row 1: `[a] Add`, `[e/ENTER] Edit`, `[t] Type`, `[SPACE] Toggle`, `[d] Delete`, `[m] Move Dn`, `[M] Move Up`.
    - Row 2: `[>] Indent`, `[<] Outdent`, `[CTRL+A] ASCII`, `[CTRL+P] Placeholders`, `[s] Save`, `[ESC/q] Exit`.

---

## 7. Ratatui Component Architecture & Modals
- **Classic Drop Shadow Geometry (`ModalFrame`, `ShadowWidget`):**
  - 2-cell right shadow (`start_y + 1` to `start_y + box_h`) and 1-row bottom shadow (`start_x + 2` to `start_x + box_w + 2`).
  - Uses `window.shadow_char` (default `░`) and theme `shadow: [fg, bg]`. Dimmed backdrop at 60%.
- **Interactive 16×16 2D Pickers (`ColorGridWidget`, `AsciiGridWidget`):**
  - For 256-entry domains (colors and CP437 ASCII 0–255), use 16×16 grids with arrow navigation (±1, ±16), Home/End, PgUp/PgDn (±64), and Enter.
  - Active cells display bold + underline. Mouse clicks resolve via `col = (event.column - grid_x) / cell_width; row = event.row - grid_y`.
- **Modal Geometry & Standards (80-Column Safe):**
  - Terminal Geometry Bounds: All modal dialogs must fit standard 80-column terminals:
    - Width: Inner dialog width must not exceed 78 columns (80 columns with right shadow).
    - Height: Inner dialog height must not exceed 27 to 29 rows to prevent vertical clipping on standard 80×24 / 80×28 / 80×30 terminals.
- **Standard Action Button Sizing:** Button variants apply 2-line borders. Never use single-line `height: 1` on bottom dialog action buttons. Use standard button rows with min-width 16 and distinct focus styling.
- **Button Semantic Alias Resolution:**
  - Theme definitions specify button colors under `button_primary`, `button_error`, `button_cancel`, and `button_success`.
  - Resolve fallbacks: `button_danger -> button_error` and `button_default -> button_cancel` before falling back to `button_primary`.

---

## 8. Performance & Rendering Engines
- **Pre-Baked TrueColor Wallpaper Canvas:**
  - Half-block (`▀`) TrueColor compositing using pure standard ANSI escape sequences. Never introduce Sixel, Kitty, or iTerm2 dependencies.
  - In Ratatui, wallpaper cells are loaded into a background buffer or rendered using `image` downsampling to cell coordinates where top half is FG and bottom half is BG.
- **Macro Interpolation Short-Circuiting:**
  - Short-circuit `interpolate_placeholders()` immediately if `text.is_empty() || !text.contains('{')`.
  - Expensive system probes (battery, local IP) must only evaluate if their placeholder (`"{battery}"`, `"{localip}"`) is present. Never probe network sockets at program startup.
- **Inactivity Timeout & Screensaver (`settings.inactivity_timeout`):**
  - Configured with `milliseconds` and optional `command`. No default command in `bashmenu.yml` (dormant unless configured).
  - Any key or mouse event resets the idle timer.
  - Run screensavers in alternate screen buffer with terminal raw mode suspended. On termination, re-enable raw mode, repaint UI, and reset timer.
- **Subprocess Spawning & Terminal Raw Mode Restoration:**
  - When spawning child TUIs (`bashedit`, `menuedit`, `themeedit`, `bmpager` — including `<F6>` Markdown preview in `bashedit` via `bmpager --markdown --no-file`) or `alt_buffer: true` console commands, use `guard.suspend_keep_alt(...)` (`BASHMENU_CHILD_ALT=1`) so neither parent nor child leaves the alternate screen buffer. Only leave the alternate screen (`guard.suspend(...)`) when `alt_buffer: false`.
  - On resume or `<F5>` full system refresh, never call `terminal.clear()` (`\x1b[2J`). Call `guard.request_clear()` to invalidate Ratatui's previous diff buffer (`\u{ffff}` sentinel + `swap_buffers()`) and draw through `guard.draw(...)` wrapped in DEC Mode 2026 Synchronized Output (`\x1b[?2026h\x1b[0m` ... `\x1b[?2026l`).

---

## 9. Shell Scripts, CLI Tools & Utilities
- **Script Header Standard:** Every shell script must begin with:
  ```bash
  #!/usr/bin/env bash
  # ==============================================================================
  # SCRIPT: <script_name>.sh
  # DESCRIPTION: <Brief, dry, and concise summary>
  # ==============================================================================
  ```
- **Self-Healing Launcher (`bashmenu.sh`):**
  - POSIX launcher script that detects architecture, checks for pre-compiled `bashmenu` binary in `$PATH` or `$HOME/.local/bin`, and runs it directly.
  - Non-interactive flags (`--check-env`, `--help`, `--version`) exit 0 without launching TUI.
- **CLI Executable Symlinks & Target Directory Resolution (`install.sh` / `bashmenu.sh`):**
  - Resolution Hierarchy:
    1. `$BASHMENU_BIN_DIR` (custom environment override).
    2. `$PREFIX/bin` (Termux environments).
    3. `/usr/local/bin` (when running as `root`, `id -u == 0`).
    4. `$HOME/.local/bin` (standard user binary directory).
    5. `/usr/local/bin` (fallback if no `$HOME` is defined and directory is writable).
- **BBCode Rich Formatting Parser:**
  - Bracketed tags: `[b]`, `[u]`, `[dim]`, `[reverse]`, `[color=...]`.
  - Parsed into `ratatui::text::Span` and `ratatui::style::Style` without intermediate HTML or heavyweight DOM structures.
- **YAML Validation Tool (`ymlcheck`):**
  - `ymlcheck <file>` validates syntax without schema checks.
  - `--themes` (`-t`) validates `bashmenu.themes` (palette depths, window, divider).
  - `--config` (`-c`) validates `bashmenu.yml`.
  - `--menu` (`-m`) validates `bashmenu.mnu` (items structure, dividers).

---

## 10. Documentation Standards & 6-Tier Parity
- **What To Document:** Every crate, module, struct, enum, and public function must have doc comments (`///`). Lines under 80 characters. Clear, concise, and precise description of purpose and parameters.
- **Tone & Formatting Constraints:** Maintain a professional, dry tone without promotional adjectives. Wrap documentation lines under 80 characters.
- **Universal 6-Tier Parity Invariant:** Every placeholder, macro, directive, and attribute must be 100% synchronized across all six documentation tiers:
  1. `crates/bashmenu-tui`: Placeholders and Macros reference modal.
  2. `crates/menuedit-cli`: Inspector guide help modal.
  3. `crates/bashedit-cli`: Editor manual modal.
  4. `crates/bashmenu-cli`: Main menu help modal (renders `bashmenu.md`).
  5. `bashmenu.md`: Reference documentation.
  6. `bashmenu.1`: Linux system man page.

---

## 11. Additional Directives
- **Continuation Directive & Agent Notebook Notes:** When you are done working on a task, always re-check both `.agents/skills/bashmenu-rs-developer/TODO.md` and `~/.config/bashmenu/notebooks/notes.yml` (fallback to repo `notebooks/notes.yml`) for notes titled `'TODO (Agent: Allowed)'` to make sure no new tasks have been added. Notes labeled as `Agent: Allowed` in their title are specifically meant for the agent.
- **Unlisted User Instructions:** If you are given an instruction by the user that is not in the TODO list, you MUST add the instruction to the TODO list (`~/.config/bashmenu/notebooks/notes.yml` under `'TODO (Agent: Allowed)'`) before executing it.
- **Pending Task Marker (`PENDING`):** When working on a TODO item, you MUST immediately mark the item as `PENDING` directly after the checkbox (e.g. `- [ ] PENDING: Fix character wrapping`). When you have completed a task, you MUST remove the `PENDING` task label and mark it as complete (`- [x] Fix character wrapping`).
- **Precedence:** `CRITICAL` / `CRITICAL BUG` has the highest precedence, followed by `BUG`, then `RESPONSIVENESS`, `UI`, `FEATURE`, `REQUEST` and finally `RESEARCH`.
- **Done Log Directive (`DONE.md`) & Task Cleanup:** Whenever a task is completed, append a summary of the completed task (what was changed/implemented and tests executed) to `.agents/skills/bashmenu-rs-developer/DONE.md`. If you have no more tasks to complete, remove finished (`- [x]`) tasks from the TODO list, and re-check the TODO list one final time to confirm no new tasks were added.
