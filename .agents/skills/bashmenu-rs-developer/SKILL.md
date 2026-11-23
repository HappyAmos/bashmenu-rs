---
name: bashmenu-rs-developer
description: Core developer guidelines for the multi-platform BashMenu Rust
  (bashmenu-rs) project. Use when modifying Rust crates, Cargo workspaces,
  shell scripts, menu configurations, themes, or writing tests.
---

# Skill: BashMenu Rust Core Developer

## 1. Intent, Scope & Boundaries
- **Intent:** Activate when developing, refactoring, debugging, or writing tests for Rust workspace crates (`bashmenu-core`, `bashmenu-tui`, `bashmenu-cli`, `menuedit-cli`, `themeedit-cli`, `bashedit-cli`, `bmpager-cli`, `cheat-cli`, `ymlcheck-cli`), shell scripts (`bashmenu.sh`, `install.sh`, `scripts/*.sh`), configurations (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, `shortcuts.yml`), or documentation.
- **Can:** Update cross-platform Rust and Bash code, YAML configs, Ratatui TUI layouts, unit and integration tests (`crates/*/tests/`, `tests/`), and man/markdown documentation.
- **Cannot:** Introduce compile errors or warnings (`-D warnings`), unhandled `.unwrap()` / `.expect()` in interactive runtime paths, or broken builds; guess paths or implementation details (ask for clarification); hardcode absolute user paths (use dynamic macros `{home}`, `{bashmenu_dir}`, `{scripts_dir}`, `{dev_dir}`); bind `<F11>` (reserved for OS fullscreen; use `<F12>` or alternate unreserved keys); omit doc comments or help parity; create unescapable loops.

---

## 2. Development Workflow & Invariants Reference
- **Authoritative Rules:** All permanent operational invariants, constraints, safety checks, and task execution rules are centralized in `.agents/rules/invariants.md`. Consult that file for:
  - Mandatory PR-based git workflow (`branch` -> `gh pr create` -> `gh pr checks <PR> --watch` -> `gh pr merge --squash` -> sync `main`) and large-binary push hygiene (`.agents/rules/git-workflow.md`).
  - README badge/workflow parity: badges sit directly under `# BashMenu-RS` starting with the Release badge, and every GitHub Actions badge URL must match an existing `.github/workflows/*.yml` file (`rust-app.yml`, `shellcheck-app.yml`).
  - Sequential TODO execution lifecycle (`PENDING:` marking, updating `DONE.md`, pruning `- [x]`, and task precedence: `CRITICAL` > `BUG` > `RESPONSIVENESS` > `UI` > `FEATURE` > `REQUEST` > `RESEARCH`).
  - Single source config paths and repository `defaults/` default configurations (`defaults/bashmenu.yml`, `defaults/bashmenu.mnu`, `defaults/bashmenu.themes`, `defaults/shortcuts.yml`, `defaults/cheat.yml`, `defaults/notes.yml`).
  - Linux Framebuffer TrueColor & `--image` mode (`/dev/fb0`).
  - Application version `{version}` and environment table `{env}` / `assets/about.md`.
  - Config header preservation (`save_config`) and YAML structural integrity (no flat dotted keys).
  - Terminal safety: panic hook, flicker-free alternate screen suspension (`guard.suspend_keep_alt`), DEC Mode 2026 redraws, and tab stop traversal order (top-to-bottom, left-to-right).
  - Theme editor `<Ctrl+P>` glyph search and theme `default_icon` fallback.
  - Colocated swap files (`.<filename>.swp`) in `bashedit`.
  - Installer (`install.sh`) and uninstaller (`uninstall.s0000h`) contracts.
- **Pre-Commit Verification Suite:** Always verify before committing:
  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  shellcheck *.sh scripts/*.sh
  wc -c .agents/rules/invariants.md  # Must be <= 24,000 bytes
  ```
- **Dependency Hygiene:** Prefer pure Rust crates (`image`, `serde_yml`, `crossterm`, `ratatui`, `unicode-width`, `thiserror`). Keep external dependencies lean; avoid heavy frameworks.
- **Directory Invariants:** All agent rules, skills, and tasks reside strictly in `.agents/`. Do not create root `AGENTS.md`.

---

## 3. Architecture & Workspace Components
| Crate / File | Type | Description |
| :--- | :--- | :--- |
| `crates/bashmenu-core` | Library | YAML configuration, Serde schemas, 6-tier macro interpolation, glyph resolution, plugins. |
| `crates/bashmenu-tui` | Library | Ratatui widgets, custom borders, drop-shadow frames, 2D pickers, modals, ANSI/BBCode styling. |
| `crates/bashmenu-cli` | Binary (`bashmenu`) | Main menu event loop, item dispatcher, child process supervisor. |
| `crates/menuedit-cli` | Binary (`menuedit`) | Visual menu tree editor for `bashmenu.mnu` (`<F4>` or standalone). |
| `crates/themeedit-cli` | Binary (`themeedit`) | Visual split-screen theme editor with 2D color and ASCII pickers (`<F8>` or standalone). |
| `crates/bashedit-cli` | Binary (`bashedit`) | Standalone text editor with Unicode buffer (`EditorBuffer`) and colocated swap files. |
| `crates/bmpager-cli` | Binary (`bmpager`) | Native terminal pager with CommonMark and OSC 8 hyperlinks. |
| `crates/cheat-cli` | Binary (`cheat`) | Interactive cheatsheet search and terminal viewer. |
| `crates/ymlcheck-cli` | Binary (`ymlcheck`) | Schema validation CLI for YAML configurations. |
| `bashmenu.sh` | Shell | POSIX wrapper checking capabilities, paths, and binary execution. |
| `defaults/bashmenu.yml` | Config | Active theme, plugin layout, timeouts, transitions, and settings. |
| `defaults/bashmenu.mnu` | Config | Menu hierarchy using label-as-key `items:` schema. |
| `defaults/bashmenu.themes` | Config | Theme palettes (256/16/8 tiers), activity overrides, window borders, dividers, marquees. |
| `scripts/` | Shell | Extension tools and plugins (`glyphs.sh`, `webopen.sh`, `unicode.sh`, games). |
| `templates/` | Templates | Shell configs (`autoexec.sh.tmpl`, `bash_aliases.tmpl`) free of personal hostnames or paths. |

---

## 4. Menu UI, Layout & Visual Width Standards
- **Margins & Inner Buffer Clamping:** Left/right margins are 2 characters; top margin is 2 lines. Rows operate as an inner buffer bounded to `avail_w = window_width - 6`. Content is clamped to `avail_w` before attaching borders (`"│  "` and `"  │"`).
- **Icon Slot Width & Alignment Invariant:** Every menu item icon slot must occupy exactly **4 columns** (`vis_w + pad_w = 4`):
  - 2-column emojis (`🚀`, `🎮`): `vis_w = 2, pad_w = 2` (`format!("{icon}  ")`).
  - 1-column glyphs (`󰖟`, `⚙`, `#`): `vis_w = 1, pad_w = 3` (`format!("{icon}   ")`).
  - No icon: exactly 4 spaces (`"    "`).
  - Descriptions align at column 11 (`prefix_w = 10` for single-digit shortcuts).
- **Adaptive Glyph Resolution (`{nf:[char]:[nerd]:[emoji]}`):**
  1. *Emoji tier*: Used if defined and `settings.use_nerd_fonts: true`.
  2. *Nerd Font tier*: Used if hex token defined (`#`, `$`, `0x`) and Nerd Fonts enabled.
  3. *Character tier*: Used if defined or if Nerd Fonts disabled.
  4. *Fallback*: Uses theme `default_icon` (`{nf:>:#f105}`).
- **Variation Selector Normalization:** Strip Unicode Variation Selectors (`.replace('\u{fe0f}', "").replace('\u{fe0e}', "")`) prior to visual column width measurement (`UnicodeWidthStr::width()`) to prevent cursor drift and border breakage.
- **Status & Help Gutters:**
  - Neither status gutter (right) nor help gutter (left) may exceed 50% of available width (`max_gutter_w = std::cmp::max(5, avail_w / 2)`).
  - Status gutter takes layout precedence over help gutter in narrow terminals.
  - Default function keys `<F1>` through `<F12>` ship in ascending order (`defaults/shortcuts.yml`), while `MenuView` preserves the user's configured `shortcuts.yml` order. Omit `[UP/DN]: Nav` across all views.
  - Sizing formula: `gutter_rows = std::cmp::min(2, std::cmp::max(1, std::cmp::max(help_lines.len(), status_lines.len())))`. Total rendered lines strictly equals terminal height `h`.
- **Universal Marquee Overflow Invariant & 50/50 Partition Contract:**
  - Text in panels, windows, viewports, lists, and titles (e.g. menu items, plugin rows, tree nodes, inspector lines, sidebar tokens, preview panes) exceeding panel or window width margins must marquee using the active theme's `marquee.char` and `marquee.gap`.
  - Input text boxes (`TextInput`) are explicitly exempted and scroll horizontally via cursor navigation without marqueeing.
  - With `[...]` brackets: `avail_w` splits 50/50 around center (left: `0..center-2`, center margin: 4 spaces, right: `center+2..avail_w`). Selected items scroll overflowing text simultaneously after a 4-tick (1.0s) delay; unselected truncate with `…`.
  - Without brackets: Option text uses full width up to `avail_w`.
  - Marquee pauses while any modal dialog is active.
  - Any test, whether menu item or description in coinciding pane must enable marquee for data that exceeds panel or window width.
- **Toasts (`{toast:SECONDS:"MESSAGE"}`):** Bottom-right rack (`max_w = (avail_w / 2).clamp(28, 60)`), left accent block `▌` (`\u{258c}`), theme panel background, bold title, dismisses on timeout or mouse click.

---

## 5. Themes, Activity Overrides & Palette Standards
- **Palette Depth Parity (256 / 16 / 8):** Themes must maintain full token parity across `256`, `16`, and `8` palette tiers. Color updates downsample automatically using perceptual luminance and HSV classification into canonical `COLOR_*` constants.
- **Theme Color Pair Transparency (`-1`):** Background index `-1` in style pairs (`[255, -1]`) signifies transparent / container-inherited background. Never map `-1` to `Color::Reset` in style pairs as it emits ANSI `\x1b[49m` over colored surfaces.
- **Per-Activity Theme Overrides:**
  - `ThemeActivity::{MainMenu, BashEdit, Modals, MenuEdit, Toasts, ThemeEdit, ShortcutEdit}` support `-1` / `[-1, -1]` inheritance from base tokens.
  - Activity overrides isolate child tool chrome, editor lines, modal frames, and toast surfaces.
- **Auto-Derivation (`<F5>`):** Auto-derives palette colors from wallpaper image, clears view-specific sub-table overrides, and prompts confirmation before overwriting.
- **CP437 ASCII Macro Interpolation (`{ascii:NNN}`):** Theme properties translate `{ascii:NNN}` to CP437 bytes. Raw `{ascii:NNN}` strings must never appear unexpanded in UI.
- **Window Borders & Dividers:**
  - Border keys: `border_horizontal_top`, `border_horizontal_bottom`, `border_vertical_left`, `border_vertical_right`, corners, tees, caps, `shadow_char`.
  - Divider keys: `divider: { char: "...", length: "{window_width}" | "{screen_width}" }`. In `bashmenu.mnu`, menu items declare `type: "{divider}"`.
- **Theme Editor Save & Deletion:**
  - <kbd>Ctrl+S</kbd> saves directly to disk if `theme_name` is set; only untitled themes open "Save As". <kbd>Ctrl+A</kbd> opens "Save As".
  - Never allow deleting the sole remaining theme; deleting active theme safely falls back to another theme and updates `bashmenu.yml`.

---

## 6. Menu Definitions & Visual Menu Editor (`bashmenu.mnu`, `menuedit`)
- **Menu Hierarchy Schema (`items:`):**
  - Sequence of maps with label as key:
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
  - Serialization: Use `bashmenu_core::menu::dump_menu_yaml()` with 2-space indented sequences and lean defaults (prune default boolean flags).
- **Visual Menu Editor (`menuedit`):**
  - Type-filtered inspector (`ItemEditModal`): filters visible controls based on item type (`command`, `script`, `submenu`, `toggle`, `radio`, `confirm`).
  - Adding/Duplicating/Deleting keys: <kbd>a</kbd> / <kbd>Insert</kbd> adds node; <kbd>d</kbd> / <kbd>D</kbd> duplicates node in-place below it; <kbd>Delete</kbd> prompts confirmation to delete node.
  - Type mutation (<kbd>Ctrl+T</kbd> / <kbd>t</kbd>): converts node types, migrating canonical payloads (`action`, `file`, `key`) and pruning obsolete fields.
  - Synchronization: `<F4>` passes `--select-path`, `--select-title`, and `--title-chain` to autofocus active item.

---

## 7. Ratatui Components, Drop Shadows & Modals
- **Drop Shadow Geometry (`ModalFrame`, `ShadowWidget`):** 2-cell right shadow (`start_y + 1` to `start_y + box_h`) and 1-row bottom shadow (`start_x + 2` to `start_x + box_w + 2`). Uses `window.shadow_char` (default `░`) with theme shadow style.
- **Interactive 16×16 2D Pickers (`ColorGridWidget`, `AsciiGridWidget`):** Grids for 256-entry domains with arrow navigation (±1, ±16), Home/End, PgUp/PgDn (±64), and direct mouse click coordinate mapping.
- **Modal Viewports & Bounds:** Inner dialog width <= 78 cols (80 cols with shadow); inner height <= 27-29 rows. Text inside modals must wrap (`wrap_prompt_text`) or clip within available inner columns, never overflowing borders.
- **Close Button Standards (`is_close_button_clicked`):** All `render_modal_frame` dialogs display `[X]` at top-right. Modal `handle_mouse` must detect clicks via `is_close_button_clicked(inner, mouse.row, mouse.column)` and return cancellation/dismissal.
- **Button Sizing & Focus:** Dialog action buttons use 2-line borders (never single-line `height: 1`), minimum width 16, and clear focus styling.
- **Tab Stop Traversal:** Strictly top-to-bottom, left-to-right visual order. <kbd>Tab</kbd> advances, <kbd>Shift+Tab</kbd> reverses.

---

## 8. Performance, Screen Transitions & Child TUIs
- **Immediate-Mode Diffing:** Ratatui renders directly into double-buffered cell grids (`Buffer`), writing only changed cells. Never construct full-screen strings.
- **Mouse Handling:** Handle `MouseEventKind::Down(MouseButton::Left)` directly for instantaneous feedback without lag.
- **Macro Short-Circuiting:** Short-circuit `interpolate_placeholders()` immediately if `text.is_empty() || !text.contains('{')`. Never probe network or system sockets at startup unless placeholder is present.
- **Child TUI Suspension (`TerminalGuard`):** When spawning child TUIs (`bashedit`, `menuedit`, `themeedit`, `bmpager`) or `alt_buffer: true` commands, use `guard.suspend_keep_alt(...)` (`BASHMENU_CHILD_ALT=1`). Use `guard.suspend(...)` only when `alt_buffer: false`. Never call `terminal.clear()` on reload; use `guard.request_clear()` and `guard.draw(...)` (DEC Mode 2026 synchronized output).
- **Inactivity Timeout Reset:** Spawning external programs suspends idle tracking; returning to event loop immediately resets `last_activity = Instant::now()`.
- **Zero-Dimension Guard:** Screen transitions (`draw_with_transition`, `draw_with_transition_out`) and modal animations (`draw_modal_grow`, `draw_modal_shrink`) must short-circuit on zero dimensions (`width == 0 || height == 0`).

---

## 9. Shell Scripts & Companion Utilities
- **Script Header Standard:** Shell scripts must begin with:
  ```bash
  #!/usr/bin/env bash
  # ==============================================================================
  # SCRIPT: <script_name>.sh
  # DESCRIPTION: <Brief, dry summary>
  # ==============================================================================
  ```
- **Self-Healing Launcher (`bashmenu.sh`):** Detects architecture, checks pre-compiled binary in `$PATH` or `$HOME/.local/bin`, and runs directly. Non-interactive flags (`--check-env`, `--help`, `--version`) exit 0 without launching TUI.
- **Installer (`install.sh`) & Uninstaller (`uninstall.sh`):**
  - Installs/uninstalls all 8 binaries: `bashmenu`, `ymlcheck`, `menuedit`, `shortcutedit`, `themeedit`, `bashedit`, `bmpager`, `cheat`.
  - Links `bm` -> `bashmenu`. Installs manual page `bashmenu.1`.
  - Installer `--merge` / `--merge-menu`: runs interactive semantic merge (`menuedit --merge`) with `.bak` backup and `ymlcheck -m` schema verification.
  - Installer `--update-menu`: overwrites user menu with vanilla template after saving `.bak` backup.
  - Uninstaller preserves user settings by default (`[Y/n]`), supporting `-k`/`--keep-settings`.
- **YAML Validation (`ymlcheck`):**
  - `--yaml` / `-y`: syntax validation.
  - `--themes` / `-t`: validates `bashmenu.themes`.
  - `--config` / `-c`: validates `bashmenu.yml`.
  - `--menu` / `-m`: validates `bashmenu.mnu`.
  - `--shortcuts` / `-s`: validates `shortcuts.yml`.

---

## 10. Documentation Standards & 6-Tier Parity
- **Code Comments:** Every crate, module, struct, enum, and public function must have `///` doc comments wrapped strictly under 80 columns in a professional, dry tone.
- **Universal 6-Tier Parity Invariant:** Every placeholder, macro, directive, and attribute must be 100% synchronized across all six documentation tiers:
  1. `crates/bashmenu-tui`: Placeholders and Macros reference modal.
  2. `crates/menuedit-cli`: Inspector guide help modal.
  3. `crates/bashedit-cli`: Editor manual modal.
  4. `crates/bmpager-cli`: Standalone pager help modal.
  5. `assets/bashmenu.md`: Reference documentation.
  6. `assets/bashmenu.1`: Linux system man page.