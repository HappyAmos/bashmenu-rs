# BashMenu-RS Porting Tasks (TODO)

## Phase 0: Workspace & Toolchain Setup
- [x] Initialize Cargo workspace `Cargo.toml` with `crates/bashmenu-core`, `crates/bashmenu-tui`, `crates/bashmenu-cli`, `crates/ymlcheck-cli`.
- [x] Set up rustfmt (`rustfmt.toml`), Clippy linter flags, and initial GitHub Actions CI workflow (`.github/workflows/rust-app.yml`).
- [x] Copy shared asset and configuration fixtures from Python repo (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`) into `tests/fixtures/`.
- [ ] Initialize stubs for `crates/menuedit-cli`, `crates/themeedit-cli`, `crates/bashedit-cli`.

## Phase 1: Core Data Models & Serialization (`bashmenu-core`)
- [x] Implement Serde YAML schemas for `bashmenu.yml` (`BashMenuConfig`).
- [x] Implement Serde YAML schemas for `bashmenu.mnu` (`MenuSchema`, `MenuItem`).
- [x] Implement Serde YAML schemas for `bashmenu.themes` (`ThemeCollection`, `Theme`, `WindowBorders`).
- [x] Implement `save_config()` with `CONFIG_HEADER` comment preservation invariant.
- [x] Colocate unit and integration tests reading actual `bashmenu.yml`, `bashmenu.mnu`, and `bashmenu.themes` files to ensure 100% schema parity.

## Phase 2: Macro & Interpolation Engine (`bashmenu-core`)
- [x] Implement 6-tier macro placeholder interpolation engine (`{env:...}`, `{home}`, `{bashmenu_dir}`, `{user.<key>}`, `{theme}`, etc.).
- [x] Implement `{nf:[char]:[nerd]:[emoji]}` adaptive glyph resolver.
- [x] Implement 4-column icon padding formatter using `unicode-width`.
- [x] Implement CP437 ASCII `{ascii:NNN}` macro translation.
- [x] Unit tests for all macro syntax variations.

## Phase 3: The `ymlcheck` CLI Tool (`crates/ymlcheck-cli`)
- [x] Port `ymlcheck.py` to `ymlcheck` binary using `clap` and `bashmenu-core`.
- [x] Verify validation flags: `--themes`, `--config`, `--menu`, and bare file syntax.

## Phase 4: Core Ratatui TUI Menu (`crates/bashmenu-tui`, `crates/bashmenu-cli`)
- [x] Terminal initialization, alternate screen buffer, raw mode, and panic hook setup.
- [x] Custom window border renderer and theme color applicator.
- [x] Main menu item rendering with 4-column icon alignment and status gutter.
- [x] Immediate `MouseDown` mouse event handler and zero-delay keyboard navigation (`j`/`k`, arrows, shortcuts).
- [x] Subprocess execution with terminal raw mode suspension and recovery.

## Phase 5: Modal Dialogs & Pickers (`crates/bashmenu-tui`)
- [x] Classic drop-shadow frame widget (`render_modal_frame`, `░` drop shadow).
- [x] Message, Confirm, and Input modals.
- [x] File and directory picker modal (`FilePickerModal`).
- [x] 16x16 2D color picker (`ColorGridModal`) and CP437 ASCII character picker (`AsciiGridModal`).

## Phase 6: Background Plugin Engine (`crates/bashmenu-core`)
- [ ] Periodic background plugin runner with caching and non-blocking status bar updates.

## Phase 7: Visual Editors Porting
- [ ] Port `menuedit` using `tui-tree-widget`.
- [ ] Port `themeedit` with 2D palette picker and live preview canvas.
- [ ] Port `bashedit` text editor with `ropey` and colocated `.<filename>.swp` swap files.

## Phase 8: Shell Integration & Installer
- [ ] Port launcher `bashmenu.sh` and installer `install.sh`.
