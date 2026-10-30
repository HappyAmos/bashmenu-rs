# BashMenu-RS Architecture & Design Blueprint

## 1. Overview & System Goals
BashMenu-RS is a zero-dependency, compiled terminal menu system and companion tool suite implemented in **Rust + Bash scripting** using **Ratatui** and **Crossterm**. It is designed to be:
- **Instantaneous**: Sub-10ms startup times and zero-latency input dispatch.
- **Robust**: Self-healing terminal state restoration, RAII raw mode management, and panic recovery hooks.
- **Modular**: Multi-crate Cargo workspace separating core logic, reusable TUI components, and individual tool binaries.

---

## 2. Cargo Workspace Architecture

```
bashmenu-rs/
├── Cargo.toml                    # Root workspace manifest
├── AGENTS.md                     # Agent development rules and invariants
├── rustfmt.toml                  # Code formatting standard
├── bashmenu.sh                   # POSIX/Bash launcher trampoline
├── install.sh                    # Multi-binary release installer
├── uninstall.sh                  # Uninstallation script
├── tests/
│   └── fixtures/                 # Golden production YAML fixtures (yml, mnu, themes)
└── crates/
    ├── bashmenu-core/            # Shared data models, serde loaders, macro & plugin engines
    ├── bashmenu-tui/             # Reusable Ratatui widgets, borders, drop shadows, modals
    ├── bashmenu-cli/             # Main application executable (bashmenu)
    ├── ymlcheck-cli/             # Schema validation and linter (ymlcheck)
    ├── menuedit-cli/             # Visual menu tree editor (menuedit)
    ├── themeedit-cli/            # Visual theme & palette editor (themeedit)
    └── bashedit-cli/             # Terminal text editor with colocated swap files (bashedit)
```

---

## 3. Subsystem Specifications

### 3.1 Data Serialization (`bashmenu-core`)
- **`config.rs`**: Loads and serializes `bashmenu.yml` (`BashMenuConfig`). Preserves top-level `CONFIG_HEADER` comment documentation via `save_config()`.
- **`menu.rs`**: Custom Serde deserializer mapping list-of-maps menu schemas into typed `MenuItem` vectors while retaining label-as-key YAML syntax.
- **`theme.rs`**: Multi-tier theme models supporting 256 colors, TrueColor hex, 16-color ANSI fallbacks, marquee configurations, and box-drawing borders.

### 3.2 Macro & Glyph Engine (`bashmenu-core`)
- **6-Tier Recursive Evaluation**:
  1. CP437 ASCII `{ascii:NNN}` lookup (0..=255).
  2. Adaptive `{nf:[char]:[nerd]:[emoji]}` glyph resolver based on `use_nerd_fonts`.
  3. Environment variables `{env:NAME}`.
  4. Runtime builtins (`{home}`, `{bashmenu_dir}`, `{scripts_dir}`, `{cache_dir}`, etc.).
  5. Dot notation paths (`{user.<key>}`, `{settings.<key>}`, `{app.<key>}`).
  6. System time badges (`{time_24}`, `{time_24_short}`, `{date}`).
- **Strict 4-Column Icon Slot Invariant**:
  - `format_icon_slot()` enforces visual width `vis_w + pad_w = 4` across all emojis, Nerd Font glyphs, and double-width characters.

### 3.3 Modal Dialogs Framework (`bashmenu-tui`)
- **Classic Drop Shadow Widget**: `render_modal_frame()` renders centered dialog viewports with 2-column right and 1-row bottom `░` drop shadows clamped to 80-column bounds.
- **Included Modals**:
  - `ConfirmModal` (Yes/No with keyboard toggles and mouse clicks).
  - `MessageModal` (Alert notifications and help popups).
  - `InputModal` (Interactive text input prompt).
  - `FilePickerModal` (Directory tree and file browser).
  - `ColorGridModal` (16×16 2D palette picker for 256 colors).
  - `AsciiGridModal` (16×16 2D grid for CP437 characters 0..=255).

### 3.4 Background Plugin Engine (`bashmenu-core`)
- **Thread-Safe Asynchronous Runner**:
  - Executes configured plugin shell scripts in background worker threads with a 5-second process timeout.
  - Caches stdout lines in `Arc<RwLock<HashMap<String, PluginCacheEntry>>>`.
  - Communicates completions over `std::sync::mpsc` channels to keep the UI event loop completely non-blocking.
- **Layout Formatter**:
  - Supports table layouts (2-col, 4-col, N-col) with automatic column width distribution and content truncation.
  - Respects standalone plugins, `{divider}` tags, and a 10-line maximum display clamp.
- **Menu Priority Invariant**:
  - Menu items always have 100% priority over plugins; plugin rows automatically shrink or drop to 0 if viewport height is constrained.

### 3.5 Visual Companion Editors
- **`themeedit`**:
  - Split-screen live preview canvas showing changes in real time.
  - Interactive 2D color picker (`ColorGridModal`) and CP437 ASCII picker (`AsciiGridModal`).
  - <kbd>Ctrl+S</kbd> direct save invariant (only untitled themes divert to "Save As").
- **`menuedit`**:
  - Interactive menu hierarchy tree editor with submenu dive and return.
  - Add, edit, delete, and move up/down (<kbd>Shift+K</kbd>/<kbd>Shift+J</kbd>) actions.
  - Persists directly to `bashmenu.mnu`.
- **`bashedit`**:
  - Colocated swap files `.<filename>.swp` in the exact directory of the target file.
  - JSON header metadata (`BASHEDIT-SWAP-v1`, PID, user, host, SHA-256).
  - Crash recovery prompt on startup and active PID collision warning (open read-only).
  - Auto-flush every 20 keystrokes and clean swap removal on exit.

---

## 4. Verification & Testing Standards
All pull requests and commits must satisfy:
1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace` (39 unit and integration tests)
4. `shellcheck *.sh`
5. GitHub Actions CI workflow: `.github/workflows/rust-app.yml`
