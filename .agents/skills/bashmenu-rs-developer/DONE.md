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

## Phase 10: Background Wallpaper Compositing Engine
- [x] Implemented `WallpaperConfig` and `WallpaperSetting` schemas in `crates/bashmenu-core/src/theme.rs` and `crates/bashmenu-core/src/config.rs` with user-override precedence logic in `BashMenuConfig::resolve_wallpaper`.
- [x] Added lean pure-Rust `image` dependency with PNG and JPEG decoders.
- [x] Implemented `WallpaperCompositor` in `crates/bashmenu-tui/src/wallpaper.rs` supporting:
  - Macro placeholder and relative path resolution with `$BASHMENU_DIR` and fallback paths.
  - Aspect ratio scaling modes: `cover`, `fit`, `stretch`, `center`, and `tile`.
  - Opacity blending with theme base background color and color channel quantization.
  - Half-block (`▀`) TrueColor cell grid generation with internal mtime/dimension caching.
- [x] Implemented `composite_buffer` for Ratatui `Buffer`:
  - Padding space runs (length >= 2, borders, margins, empty rows) become `'▀'` TrueColor half-blocks.
  - Active selection highlights retain their explicit highlight styling.
  - Non-space characters (text labels, box borders, icons) preserve their glyph and foreground style while their background color is set to the underlying cell's blended average color.
- [x] Connected wallpaper engine into `MenuView::render` and `themeedit-cli` live preview canvas.
- [x] Colocated comprehensive unit and integration test suite `crates/bashmenu-tui/tests/wallpaper_test.rs` validating RGB conversions, path resolution, scaling modes, cache hit rates, padding space conversion, and user configuration overrides. Verified zero Clippy warnings and 100% test pass rate.

## Phase 11: Menu Selection & Highlight Bar Alignment
- [x] Implemented unbroken full-width menu highlight bar extending entirely across the window (within margins):
  - Ensured active item shortcut keys are styled with `item_style` (`self.styles.highlight`), removing the unhighlighted gap.
  - Submenu arrows (`»`) on active items use `item_style` (`self.styles.highlight`).
  - Added trailing padding with `item_style` spaces extending the highlight bar across the entire inner width (`inner_x` to `inner_x + avail_w`).
  - Preserved wide character (emojis, Nerd Fonts) continuation cells so background wallpaper compositing keeps active selection rows intact.
  - Updated `themeedit` live preview to render unbroken full-width active item highlight bars.
  - Added integration test `test_menu_highlight_bar_unbroken_full_width` in `crates/bashmenu-tui/tests/menu_navigation_test.rs` and active row highlight checks in `wallpaper_test.rs`. Zero Clippy warnings.

## Phase 12: 1:1 Parity for Notebooks, Text Editor, and Menu Editor
- [x] **Text Editor (`bashedit`) Empty Buffer Support:**
  - Added support for empty unsaved buffers (`[Untitled]`) when launching `bashedit` with no arguments.
  - Implemented `<Ctrl+A>` and `<Ctrl+S>` Save As workflow with `InputModal` to save untitled buffers directly to disk.
  - Updated `bashmenu-cli` to launch `bashedit` without args for blank documents rather than loading `bashmenu.yml`.
  - Added test `test_editor_buffer_empty_and_save_as` in `crates/bashedit-cli/tests/bashedit_test.rs`.
- [x] **Dynamic Notebooks & Notes Engine:**
  - Added `MenuItemType::Notebook`, `NotebookNote`, `NotebookEmpty`, and `Back` variants in `bashmenu-core`.
  - Implemented notebook file parsing and CRUD operations in `crates/bashmenu-core/src/notebook.rs` (`load_notebook`, `save_notebook`, `get_notes`, `add_or_update_note`, `rename_note`, `delete_note`, `build_notebook_submenu`).
  - Implemented `NoteViewerModal` with word-wrapping, dynamic interactive checkbox toggling (`[SPACE]`), show/hide completed (`[H]`), in-place editing with `bashedit` (`[E]`), rename (`[M]`), delete (`[D]`), and OSC 52 clipboard copy (`[C]`).
  - Added notebook management shortcuts (`[N]` New Note, `[M]` Rename, `[D]` Delete) and dynamic help gutter in `MenuView`.
  - Connected notebook modals and lifecycle actions in `bashmenu-cli`.
  - Added integration test `test_notebook_menu_interaction` in `crates/bashmenu-tui/tests/menu_navigation_test.rs` and unit test `test_notebook_crud_roundtrip` in `crates/bashmenu-core`.
- [x] **Visual Menu Editor (`menuedit`) 1:1 Tree & Inspector Hierarchy:**
  - Rebuilt `menuedit` with 50/50 side-by-side split screen matching Python original (`#tree_panel` on left, `#inspector_panel` on right).
  - Implemented collapsible tree hierarchy (`tree.rs`) with type badges (`[DIR]`, `[CMD]`, `[SCR]`, `[CFG]`, `[TGL]`, `[RAD]`, `[EDT]`, `[NBK]`, `[DIV]`), expand/collapse toggling (`[e]`), indent (`>`), outdent (`<`), move up/down (`M`/`m`), insert (`a`), delete (`d`), and live Property Inspector.
  - Implemented `ItemTypePickerModal` with 16 menu item types.
  - Implemented `ItemEditModal` with schema field inputs, execution mode checkboxes (`alt_buffer`, `no_formatting`, etc.), live preview, and ASCII table picker modal integration.
  - Added tests in `crates/menuedit-cli/tests/menuedit_test.rs` verifying property inspector formatting, tree nesting, indent/outdent, and type mutation.
- [x] **Notebook File Path Resolution & Installation Parity:**
  - Repaired truncated `notebooks/notes.yml` and verified valid YAML structure.
  - Enhanced `resolve_notebook_path` and `MacroContext::new` with fallbacks for user config directory (`~/.config/bashmenu/notebooks/notes.yml`).
  - Updated `install.sh` to initialize `$CONFIG_DIR/notebooks` with default notebooks fixtures.
  - Added user-facing error modal reporting on notebook load failures instead of silent dismissal.
- [x] Verified zero Clippy warnings across the entire workspace with `-D warnings`, full `cargo fmt` compliance, and 100% test pass rate.

## Phase 13: Notebook Note Editor Parity & Rendering Artifact Fixes
- [x] **Word-Boundary Text Wrapping in Note Editor and Note Viewer:**
  - Implemented word-boundary soft-wrapping (`wrap_chars_by_word`) in `NoteEditorModal` (`compute_visual_lines`), ensuring words are not sliced across visual rows.
  - Enhanced `textwrap_simple` in `NoteViewerModal` with word-level wrapping and graceful handling of continuous strings exceeding viewport width.
- [x] **Autocomplete & End-of-Line Enter Fixes (`NoteEditorModal`):**
  - Resolved autocomplete conflict where pressing <kbd>Enter</kbd> at the end of a checkbox line sliced characters incorrectly.
  - Converted string slicing and cursor indexing in `insert_char`, `handle_backspace`, `handle_delete`, and `handle_enter` to character-safe Unicode operations.
  - Fixed `<End>` key to navigate to the logical line end, enabling clean continuation creation on subsequent rows.
  - Added left-click mouse interaction (`MouseEventKind::Down(MouseButton::Left)`) in `handle_mouse`, accurately mapping mouse coordinates to target line and column.
- [x] **Theme Background Color Artifact Elimination & Inverted Selection:**
  - Resolved black text background artifact in modals where theme color pairs with `-1` background (e.g. `[255, -1]`) were parsed as `Color::Reset` instead of inheriting the modal container's background (`styles.background`).
  - Updated `parse_style_pair` in `crates/bashmenu-tui/src/style.rs` so `-1` leaves the background transparent/inherited instead of forcing terminal default black (`Color::Reset`).
  - Explicitly mapped modal text backgrounds to `styles.background` in `NoteViewerModal`, `HelpModal`, and `NoteEditorModal` so text always matches the popup window's theme background.
  - Replaced the `►` line indicator in `NoteViewerModal` with inverted text (`Modifier::REVERSED`), providing clean high-contrast highlighting of active items and links.
- [x] **Main Menu Help Modal Width Expansion:**
  - Updated `HelpModal::render` in `crates/bashmenu-tui/src/modals/help.rs` to compute dialog width as 90% of screen width (`(area.width as f32 * 0.90)` clamped within margins).
  - Relaxed the hard-coded 78-column cap in `render_modal_frame` in `crates/bashmenu-tui/src/modals/frame.rs` to allow dialog width to scale with screen width while maintaining shadow margins.
  - Added integration test `test_help_modal_renders_at_90_percent_width` in `crates/bashmenu-tui/tests/modals_test.rs`.
- [x] **Tests & Invariants Verification:**
  - Added unit and integration tests in `crates/bashmenu-tui/tests/modals_test.rs` covering word-boundary wrapping, End-key navigation, autocomplete enter continuation, mouse-click cursor positioning, and heading style background isolation.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (all 61 tests passing).

## Phase 14: Submenu Type Inference, Window Close Button [X], and Notes Navigation
- [x] **Submenu Type Inference:**
  - Fixed issue where `Bashmenu Settings` inside `Options & Settings` failed to open due to missing explicit `type: submenu`.
  - Updated `crates/bashmenu-core/src/menu.rs` deserializer to automatically infer `MenuItemType::Submenu` whenever `raw_props.submenu.is_some()` and type was omitted.
  - Updated `crates/bashmenu-tui/src/menu_view.rs` to treat items with `item.submenu.is_some()` as submenus when activating and rendering `»`.
  - Added explicit `type: submenu` in `bashmenu.mnu` and `tests/fixtures/bashmenu.mnu`.
  - Added integration test `test_nested_submenu_without_explicit_type` in `menu_navigation_test.rs`.
- [x] **Universal Modal Close Button [X]:**
  - Added `window_close_button: Style` to `ResolvedThemeStyles` in `crates/bashmenu-tui/src/style.rs` resolved from theme palettes.
  - Updated `render_modal_frame` in `crates/bashmenu-tui/src/modals/frame.rs` to render `[X]` just to the left of the top-right border corner (`[` and `]` in border style, `X` in `styles.window_close_button`).
  - Added `is_close_button_clicked` helper in `frame.rs` and wired close button click detection into `HelpModal`, `NoteViewerModal`, `NoteEditorModal`, `ConfirmModal`, `InputModal`, `ThemePickerModal`, and `FilePickerModal`.
  - Added integration test `test_modal_frame_close_button_rendering_and_clicking` in `modals_test.rs`.
- [x] **Notes Checkbox Wrapping Margin:**
  - Updated `NoteViewerModal` and `NoteEditorModal` to prepend continuation lines of wrapped checkbox items with a matching margin (indent + 6 spaces) so wrapped text aligns under the checkbox text.
  - Added unit tests in `modals_test.rs` validating margin indentation on continuation lines.
- [x] **Note Viewer Navigation & Selection Fixes:**
  - Fixed issue in `NoteViewerModal` where hidden completed checkboxes (when `show_completed` is false) remained selectable.
  - Updated line navigation (`Up`, `Down`, `Home`, `End`, `PageUp`, `PageDown`) to step only across visible items.
  - Updated checkbox highlight style when selected (`is_sel`) to match the text foreground and background scheme, preventing clashing color blocks.
- [x] **Modal Indicator Cleanup:**
  - Removed remaining `►` glyph indicators in `ThemePickerModal`, `FilePickerModal`, and `ColorGridModal`.
  - Added integration test `test_modal_no_triangle_indicator` in `modals_test.rs`.
- [x] Verified zero Clippy warnings, full formatting compliance, and 100% test pass rate across 70 tests.
