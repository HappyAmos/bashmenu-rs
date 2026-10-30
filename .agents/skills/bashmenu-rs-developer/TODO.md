# BashMenu-RS Porting Tasks (TODO)

## Phase 0: Workspace & Toolchain Setup
- [ ] Initialize Cargo workspace `Cargo.toml` with `crates/bashmenu-core`, `crates/bashmenu-tui`, `crates/bashmenu-cli`, `crates/menuedit-cli`, `crates/themeedit-cli`, `crates/bashedit-cli`, `crates/ymlcheck-cli`.
- [ ] Set up rustfmt (`rustfmt.toml`), Clippy linter flags, and initial GitHub Actions CI workflow (`.github/workflows/rust-app.yml`).
- [ ] Copy shared asset and configuration fixtures from Python repo (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`) into a `fixtures/` or root directory for testing.

## Phase 1: Core Data Models & Serialization (`bashmenu-core`)
- [ ] Implement Serde YAML schemas for `bashmenu.yml` (`BashMenuConfig`).
- [ ] Implement Serde YAML schemas for `bashmenu.mnu` (`MenuSchema`, `MenuItem`).
- [ ] Implement Serde YAML schemas for `bashmenu.themes` (`ThemeDefinitions`, `WindowBorders`, `ThemeColors`).
- [ ] Implement `save_config()` with `CONFIG_HEADER` comment preservation invariant.
- [ ] Colocate unit tests reading actual `bashmenu.yml`, `bashmenu.mnu`, and `bashmenu.themes` files to ensure 100% schema parity.

## Phase 2: Macro & Interpolation Engine (`bashmenu-core`)
- [ ] Implement 6-tier macro placeholder interpolation engine (`{env:...}`, `{home}`, `{bashmenu_dir}`, `{user.<key>}`, `{theme}`, etc.).
- [ ] Implement `{nf:[char]:[nerd]:[emoji]}` adaptive glyph resolver.
- [ ] Implement 4-column icon padding formatter using `unicode-width`.
- [ ] Implement CP437 ASCII `{ascii:NNN}` macro translation.
- [ ] Unit tests for all macro syntax variations.

## Phase 3: The `ymlcheck` CLI Tool (`crates/ymlcheck-cli`)
- [ ] Port `ymlcheck.py` to `ymlcheck` binary using `clap` and `bashmenu-core`.
- [ ] Verify validation flags: `--themes`, `--config`, `--menu`.

## Phase 4: Core Ratatui TUI Menu (`crates/bashmenu-tui`, `crates/bashmenu-cli`)
- [ ] Terminal initialization, alternate screen buffer, raw mode, and panic hook setup.
- [ ] Custom window border renderer and theme color applicator.
- [ ] Main menu item rendering with 4-column icon alignment and 50/50 dual marquee scrolling.
- [ ] Immediate `MouseDown` mouse event handler and zero-delay keyboard navigation (`j`/`k`, arrows, shortcuts).
- [ ] Subprocess execution with terminal raw mode suspension and recovery.

## Phase 5: Modal Dialogs & Pickers (`crates/bashmenu-tui`)
- [ ] Classic drop-shadow frame widget (`ModalFrame`, `ShadowWidget`).
- [ ] Message, Confirm, Input, Toggle, and Radio modals.
- [ ] File and directory picker modal.
- [ ] 16x16 2D color picker and CP437 ASCII character picker.

## Phase 6: Background Plugin Engine (`crates/bashmenu-core`)
- [ ] Periodic background plugin runner with caching and non-blocking status bar updates.

## Phase 7: Visual Editors Porting
- [ ] Port `menuedit` using `tui-tree-widget`.
- [ ] Port `themeedit` with 2D palette picker and live preview canvas.
- [ ] Port `bashedit` text editor with `ropey` and colocated `.<filename>.swp` swap files.

## Phase 8: Shell Integration & Installer
- [ ] Port launcher `bashmenu.sh` and installer `install.sh`.
