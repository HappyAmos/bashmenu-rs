# BashMenu-RS Architecture & Design Blueprint

## 1. Overview & System Goals
BashMenu-RS is a zero-dependency, compiled terminal menu system and companion developer tool suite implemented in **Rust + Bash scripting** using **Ratatui** and **Crossterm**. It is designed around four core tenets:
- **Instantaneous Startup & Zero Latency**: Sub-10ms cold start times, double-buffered cell diffing, and immediate `MouseDown` activation without perceptible delay.
- **Terminal Safety & Robustness**: Global panic recovery hooks (`std::panic::set_hook`), RAII terminal raw mode restoration (`TerminalGuard`), and DEC Mode 2026 synchronized output prevents flickering and terminal corruption.
- **Modular Workspace Architecture**: A strictly decoupled Cargo workspace separating serialization models, reusable Ratatui UI components, and standalone CLI utilities.
- **Extensible Configuration Hierarchy**: Macro expansion, customizable palettes, pluggable background tasks, notebook markdown management, and leader-based keyboard chording.

---

## 2. Cargo Workspace Architecture

```
bashmenu-rs/
├── Cargo.toml                    # Root workspace manifest
├── AGENTS.md                     # Agent development guidelines, rules, and invariants
├── rustfmt.toml                  # Workspace Rust formatting standard
├── bashmenu.sh                   # POSIX/Bash launcher trampoline
├── install.sh                    # Multi-binary release installer (~/.local/bin)
├── uninstall.sh                  # Uninstallation script
├── patches/libyml/               # Vendored libyml fix for flow scalar string join overflow
├── tests/
│   └── fixtures/                 # Golden production YAML fixtures (yml, mnu, themes, shortcuts)
└── crates/
    ├── bashmenu-core/            # Shared data models, Serde loaders, macro & plugin engines, notebooks
    ├── bashmenu-tui/             # Reusable Ratatui widgets, custom borders, drop shadows, modals, wallpaper
    ├── bashmenu-cli/             # Main interactive application executable (bashmenu / bm)
    ├── ymlcheck-cli/             # YAML schema validator and configuration linter (ymlcheck)
    ├── menuedit-cli/             # Visual menu tree editor (menuedit) & shortcut editor (shortcutedit)
    ├── themeedit-cli/            # Visual theme & palette editor with split preview (themeedit)
    ├── bashedit-cli/             # Terminal text editor with colocated swap files (bashedit)
    └── bmpager-cli/              # Native terminal pager with CommonMark & OSC 8 links (bmpager)
```

---

## 3. Subsystem Specifications

### 3.1 Data Models & Serialization (`bashmenu-core`)
- **`config.rs`**: Loads, modifies, and serializes `bashmenu.yml` (`BashMenuConfig`). Always preserves the top-level `CONFIG_HEADER` documentation comments via `save_config()`.
- **`menu.rs`**: Custom Serde deserializer mapping list-of-maps menu schemas into strongly-typed `MenuItem` vectors while retaining label-as-key YAML syntax. Supports nested submenus, commands, scripts, file editors, notebooks, toast popups, and confirmation dialogs.
- **`theme.rs`**: Multi-tier theme models supporting 256 colors, TrueColor RGB hex, 16-color ANSI fallbacks, marquee configuration, CP437 ASCII translations, and custom box-drawing borders.
- **`shortcuts.rs`**: Parses and serializes `shortcuts.yml` (`ShortcutsConfig`, `ShortcutEntry`). Enforces key normalization (`normalize_key_spec`), chord validation, and header comment preservation (`SHORTCUTS_HEADER`).
- **`notebook.rs`**: Parses YAML notebooks (`Notebook`, `NotebookNote`) with markdown formatting, multi-line notes, and CRUD state management.

### 3.2 Macro & Glyph Interpolation Engine (`bashmenu-core`)
- **6-Tier Recursive Evaluation**:
  1. CP437 ASCII `{ascii:NNN}` lookup (0..=255).
  2. Adaptive `{nf:[char]:[nerd]:[emoji]}` glyph resolver based on `use_nerd_fonts`.
  3. Environment variables `{env:NAME}`.
  4. Runtime builtins (`{home}`, `{bashmenu_dir}`, `{dev_dir}`, `{scripts_dir}`, `{notebooks_dir}`, `{cache_dir}`, `{shortcuts_file}`, `{shortcuts}`).
  5. Dot-notation configuration paths (`{user.<key>}`, `{settings.<key>}`, `{app.<key>}`).
  6. Dynamic directives: `{command:<cmd>}` (cached stdout), `{param}` / `{prompt}` (interactive input), `{file_picker}` / `{dir_picker}`, and `{toast:SECS:"MSG"}`.
- **Strict 4-Column Icon Slot Invariant**:
  - `format_icon_slot()` guarantees visual width `vis_w + pad_w = 4` across all emojis, Nerd Font glyphs, and double-width characters.

### 3.3 Modal Dialogs & TUI Primitives (`bashmenu-tui`)
- **Classic Drop Shadow Widget**: `render_modal_frame()` renders centered dialog viewports with 2-column right and 1-row bottom `░` drop shadows clamped to 80-column bounds with `[X]` close buttons.
- **Modal Component Library**:
  - `ConfirmModal`: 3-button (Yes/No/Cancel) confirmation dialog.
  - `MessageModal`: Informational alerts and multi-line message dialogs.
  - `InputModal`: Single-line text input prompt with cursor navigation and word-wrapped prompts.
  - `FilePickerModal`: Directory tree and file system browser with sorting and dynamic filtering.
  - `ColorGridModal`: 16×16 2D palette picker for 256-color terminals.
  - `AsciiGridModal`: 16×16 2D grid for CP437 characters 0..=255.
  - `NoteViewerModal` & `NoteEditorModal`: Rich markdown note inspection and full-featured text editing with word wrapping, selection, and clipboard support.
  - `RadioModal` & `ToggleModal`: Single-select radio lists and boolean settings toggles.
  - `StreamOutputModal`: Live streaming command output viewer with percentage-based viewport sizing.
- **Modal Text Clamping & Word Wrapping Invariant**:
  - Text inside popup modals MUST NEVER exceed visible window width. Prompts, error messages, and status lines are word-wrapped via `wrap_prompt_text()` or clamped to available inner columns.
- **Wallpaper Compositor**:
  - `WallpaperCompositor` renders TrueColor half-block (`▀`) backgrounds with dynamic brightness dimming and cell caching.
- **Non-Blocking Toast System**:
  - `ToastRack` manages dockable notifications in the bottom-right corner with auto-dismiss timers and mouse click dismissal.
- **Leader Shortcuts & Which-Key Overlay**:
  - `ShortcutDispatcher` handles global `<leader>` chording (e.g. `<Ctrl+Space>`), timeout expiration, and renders the `WhichKeyPopup` overlay.

### 3.4 Background Plugin Engine (`bashmenu-core`)
- **Thread-Safe Asynchronous Runner**:
  - Executes configured plugin shell scripts in background worker threads with a 5-second execution timeout.
  - Caches stdout lines in `Arc<RwLock<HashMap<String, PluginCacheEntry>>>`.
  - Non-blocking channel notifications (`std::sync::mpsc`) wake the UI event loop for real-time redraws.
- **Layout Formatter**:
  - Supports table layouts (2-col, 4-col, N-col) with automatic column width distribution and content truncation.
  - Respects standalone plugins, `{divider}` tags, and a 10-line maximum display clamp.
- **Menu Priority Invariant**:
  - Menu items always have 100% priority over plugins; plugin rows automatically shrink or drop to 0 if viewport height is constrained.

### 3.5 Standalone & Companion CLI Tools
- **`bashmenu` (`bm`)**: Primary interactive menu system with submenu stacks, dynamic plugin rendering, screen savers, and child process suspension.
- **`menuedit`**: Visual menu hierarchy tree editor with submenu dive/return, add/edit/delete/move actions, and type mutation.
- **`shortcutedit` (`menuedit --shortcuts`)**: Split-screen keyboard shortcuts editor with chord capture (`KeyCaptureModal`), leader configuration, and execution mode selection.
- **`themeedit`**: Visual theme & palette editor with split-screen live preview, 2D color/ASCII pickers, and `<Ctrl+S>` direct disk save.
- **`bashedit`**: Full-screen terminal text editor utilizing `ropey` rope buffers, colocated `.<filename>.swp` swap files, crash recovery prompts, and universal clipboard support.
- **`bmpager`**: Standalone native terminal pager with CommonMark markdown parsing, fenced code block framing, OSC 8 clickable hyperlinks, search navigation, and vim-like keybindings.
- **`ymlcheck`**: Standalone YAML schema validator and linter for `bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, and `shortcuts.yml`.

---

## 4. Verification & Testing Standards
All pull requests and commits must satisfy:
1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace` (all unit, integration, and doc tests)
4. `shellcheck *.sh scripts/*.sh`
5. GitHub Actions CI workflows: `.github/workflows/rust-app.yml` and `.github/workflows/shellcheck-app.yml`
