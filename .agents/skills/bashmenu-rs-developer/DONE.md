# BashMenu-RS Completed Tasks (DONE)

## Phase 0: Project Initiation
- [x] Created `bashmenu-rs-developer` skill specification at `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Initialized phased `TODO.md` roadmap tracking for the Rust port.
- [x] Set up multi-crate Cargo workspace (`bashmenu-core`, `bashmenu-tui`, `bashmenu-cli`, `ymlcheck-cli`).
- [x] Configured `rustfmt.toml` and GitHub Actions CI workflow (`.github/workflows/rust-app.yml`).
- [x] Copied golden test fixtures (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`) into `tests/fixtures/`.
- [x] Implemented global terminal panic hook in `bashmenu-tui` and verified zero Clippy warnings across the workspace.

## Phase 1: Core Data Models & Serialization (`bashmenu-core`)
- [x] Defined `BashMenuConfig`, `Settings`, and `InactivityTimeout` in `config.rs`.
- [x] Implemented `save_config()` with `CONFIG_HEADER` comment preservation invariant.
- [x] Defined `MenuSchema`, `MenuItem`, and `Submenu` in `menu.rs` with custom label-as-key serialization and normalization.
- [x] Defined `ThemeCollection`, `Theme`, `WindowBorders`, `MarqueeConfig`, and palette structures in `theme.rs`.
- [x] Added unit tests and comprehensive integration tests in `crates/bashmenu-core/tests/schema_fixtures_test.rs` validating 100% compatibility against production `bashmenu.yml`, `bashmenu.mnu`, and `bashmenu.themes`. Verified zero Clippy warnings.

## Phase 2: Macro & Interpolation Engine (`bashmenu-core`)
- [x] Implemented CP437 ASCII macro translation (`{ascii:NNN}`) and full character table in `glyph.rs`.
- [x] Implemented `{nf:[char]:[nerd]:[emoji]}` adaptive glyph resolver with strict 4-tier precedence.
- [x] Implemented 4-column icon padding invariant (`vis_w + pad_w = 4`) and variation selector normalization.
- [x] Implemented recursive 6-tier macro placeholder interpolation engine in `macro_engine.rs` supporting environment variables, runtime paths (`{bashmenu_dir}`, `{scripts_dir}`, `{cache_dir}`), system time/date badges, and dot notation config paths (`{user.<key>}`, `{settings.<key>}`).
- [x] Added integration test suite `crates/bashmenu-core/tests/macro_engine_test.rs` testing icon column alignment and status gutter expansion. Zero Clippy warnings.

## Phase 3: The `ymlcheck` CLI Tool (`crates/ymlcheck-cli`)
- [x] Ported `ymlcheck.py` to compiled Rust executable `ymlcheck` with `clap` arguments (`--themes`, `--config`, `--menu`, and bare YAML syntax check).
- [x] Configured binary aliases in `crates/ymlcheck-cli/Cargo.toml` and `crates/bashmenu-cli/Cargo.toml` so built executables match canonical names (`ymlcheck` and `bashmenu`).
- [x] Verified successful validation against all production fixtures with exit code 0. Zero Clippy warnings.

## Phase 4: Core Ratatui TUI Menu (`crates/bashmenu-tui`, `crates/bashmenu-cli`)
- [x] Implemented `ResolvedThemeStyles` in `style.rs` mapping theme color tokens (256, 16, TrueColor hex, named ANSI) to Ratatui `Color` and `Style`.
- [x] Implemented `TerminalGuard` in `terminal.rs` managing raw mode, alternate screen buffer, mouse capture, and safe RAII restoration on drop.
- [x] Implemented `suspend()` method on `TerminalGuard` enabling seamless child process execution (`/bin/bash -c ...`) and full terminal re-initialization upon child termination.
- [x] Implemented `MenuView` in `menu_view.rs` featuring multi-level submenu navigation stacks, viewport scroll clamping, 4-column icon slots, divider skipping, and bottom status gutter.
- [x] Implemented immediate `MouseDown` left-click activation without waiting for `MouseUp` or `Click`.
- [x] Built interactive `bashmenu` executable in `crates/bashmenu-cli/src/main.rs`.
- [x] Added unit and integration test suite `menu_navigation_test.rs` covering navigation, divider jumping, submenu push/pop, immediate mouse click activation, and buffer rendering. Verified zero Clippy warnings across the workspace.

## Phase 5: Modal Dialogs & Pickers (`crates/bashmenu-tui`)
- [x] Implemented `render_modal_frame` in `modals/frame.rs` rendering centered dialog viewports with classic 2-column right and 1-row bottom `░` drop shadows and 80-column bounds clamping.
- [x] Implemented `ConfirmModal` (Yes/No) with keyboard toggles (`Tab`, arrows, `y`/`n`) and immediate mouse click activation.
- [x] Implemented `MessageModal` for alert notifications and help messages.
- [x] Implemented `InputModal` for interactive prompt strings with cursor navigation, inline editing, and Enter/Esc handling.
- [x] Implemented `FilePickerModal` for directory and file browsing with directory sorting, parent traversal (`..`), and file selection.
- [x] Implemented `ColorGridModal` (16x16 2D grid for 256 colors) and `AsciiGridModal` (16x16 2D grid for CP437 ASCII codes 0..=255) with keyboard navigation (±1, ±16, ±64) and direct mouse cell click support.
- [x] Added comprehensive integration test suite `modals_test.rs`. Verified zero Clippy warnings across the workspace.

## Phase 6: Background Plugin Engine (`crates/bashmenu-core`)
- [x] Implemented `PluginDef`, `PluginLayout`, `PluginConfig`, and `PluginCacheEntry` in `plugin.rs`.
- [x] Implemented thread-safe `PluginEngine` running periodic background worker threads with child process execution timeout (5s) and cached stdout lines.
- [x] Implemented output sanitization: null bytes removed, tabs expanded to 4 spaces, variation selectors stripped.
- [x] Implemented table and list layout formatting with column width distribution, standalone plugin handling, and 10-line display limit clamping.
- [x] Connected MPSC channel notifications to Ratatui event loop in `bashmenu-cli` for non-blocking dynamic UI updates.
- [x] Implemented menu priority over plugins invariant: plugin rows are automatically truncated or dropped first when viewport height is constrained.
- [x] Added comprehensive integration test suites in `plugin_engine_test.rs` and `menu_navigation_test.rs`. Verified zero Clippy warnings across the workspace.

## Phase 7: Visual Editors Porting
- [x] Ported `themeedit` (`crates/themeedit-cli`):
  - Split-screen live preview canvas showing changes in real time.
  - Interactive 2D color picker (`ColorGridModal`) for 256 colors and CP437 ASCII character picker (`AsciiGridModal`) for window border glyphs.
  - Enforced `<Ctrl+S>` direct disk save invariant (only untitled themes divert to Save As) and `<Ctrl+A>` Save As.
  - Added integration test suite `themeedit_test.rs` verifying state modifications and direct disk saving. Zero Clippy warnings.
- [x] Ported `menuedit` (`crates/menuedit-cli`):
  - Multi-level menu hierarchy navigation and editing with submenu dive and return.
  - Add item, edit label/command, delete item, and move item up/down (`swap_up`/`swap_down`).
  - Persistence to `bashmenu.mnu` via `save_menu()`.
  - Added integration test suite `menuedit_test.rs` covering navigation, reordering, adding, editing, deleting, and submenu unwinding. Zero Clippy warnings.
- [x] Ported `bashedit` (`crates/bashedit-cli`):
  - Colocated swap files `.<filename>.swp` in the exact directory of the target file with JSON header metadata (`BASHEDIT-SWAP-v1`, PID, user, host, SHA-256).
  - Startup crash recovery prompt (`[R]ecover`, `[D]elete`, `[O]pen read-only`, `[Q]uit`) and active PID collision warning.
  - Clean exit removes swap files; read-only mode protects other users' swap files and disallows saving.
  - Keystroke counter auto-flush (every 20 keystrokes) and manual save (`<Ctrl+S>`).
  - Added comprehensive test suite `bashedit_test.rs` covering swap file lifecycle, hashing, process PID checks, and read-only protection. Zero Clippy warnings.

## Phase 8: Shell Integration & Installer
- [x] Ported `bashmenu.sh` launcher trampoline with POSIX `/bin/sh` elevation to Bash, symlink resolution, UTF-8 locale normalization, automatic binary discovery (`target/release`, `~/.local/bin`, `target/debug`), and on-demand `cargo build --release` fallback.
- [x] Implemented `install.sh` for multi-crate release workspace compilation, copying all 5 binaries (`bashmenu`, `ymlcheck`, `menuedit`, `themeedit`, `bashedit`) plus `bm` shortcut to `$HOME/.local/bin`, deploying default config fixtures to `~/.config/bashmenu/`, and updating user shell RC files (`.bashrc`, `.zshrc`, `.profile`).
- [x] Implemented `uninstall.sh` removing all installed binaries, shortcuts, and cleanly cleaning configuration/cache directories with interactive or `-y` confirmation.
- [x] Added `shellcheck` verification to GitHub Actions CI workflow (`.github/workflows/rust-app.yml`). All shell scripts pass Shellcheck with 0 warnings.

## Phase 9: Asset Localization & Python Decoupling
- [x] Copied and localized all extension scripts (`scripts/`), assets (`assets/` including wallpapers, cheat sheet, and gifs), templates (`templates/`), notebooks (`notebooks/notes.yml`), documentation (`bashmenu.md`, `bashmenu.1`), and configurations (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`) into the local repository.
- [x] Updated `scripts/pager.sh` to execute the compiled `bashedit` Rust binary directly rather than attempting to call `bashedit.py`.
- [x] Updated `bashmenu.mnu` to invoke `menuedit` and `themeedit` compiled binaries and updated developer skill file paths to `bashmenu-rs-developer`.
- [x] Updated file resolution logic in `bashmenu-cli`, `menuedit-cli`, `themeedit-cli`, and `macro_engine` to respect `$BASHMENU_DIR`, user configurations in `~/.config/bashmenu/`, and local repository files before fallback.
- [x] Updated `install.sh` and `uninstall.sh` to install and cleanly remove localized `bmpager`, `cheat`, and `rich` companion shortcuts in `$HOME/.local/bin` pointing strictly to the local `bashmenu-rs` scripts.
- [x] Verified zero references pointing to `../bashmenu` or old python scripts, and validated that all Rust tests, Clippy lints, formatting, and Shellcheck pass with 0 warnings.
