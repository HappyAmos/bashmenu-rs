## Documentation Modernization & Legacy Python Cleanup
- [x] Audited and synchronized `bashmenu.md` and `bashmenu.1` against the Rust codebase (v0.1.2), replacing all legacy Textual, Rich, and Python runtime references with Ratatui architecture, double-buffered cell rendering, and native ANSI escape sequences.
- [x] Replaced Section 7.2 in `bashmenu.md` and `.SS In-Process Python Plugins` in `bashmenu.1` with comprehensive documentation of subprocess execution modes (`execution_mode`, `stream`, `interactive`, `window_percent`, `alt_buffer`, `quiet`).
- [x] Updated Section 2.1 utility scripts in `bashmenu.md` and `bashmenu.1` to match the exact active repository files in `scripts/`, removing obsolete references (`scripts/ascii.sh`, `scripts/gorillas.py`, `scripts/install_glow.sh`, `scripts/install_kmscon.sh`, `scripts/install_scripts.sh`, `scripts/ncurses_colors.py`, `scripts/pager.sh`, `scripts/rich.sh`, `scripts/theme_from_image.py`) and documenting new utilities (`scripts/sso-ssh.sh`, `scripts/temp.sh`).
- [x] Updated command invocations for `themeedit` (replacing `themeedit.py` and `theme_from_image.py` with native `<F5>` auto-derivation), `bashedit` (replacing `bashedit.py`), `bmpager`, and `cheat`.
- [x] Updated weather viewer commands in `bashmenu.mnu` to use `bmpager` rather than obsolete `scripts/pager.sh`.
- [x] Corrected default theme expectations in `crates/bashmenu-core/src/config.rs` and `schema_fixtures_test.rs` to match active default theme `alaska-train`.
- [x] Verified full workspace test suite (`cargo test --workspace`), format checking (`cargo fmt --all --check`), clippy lints (`cargo clippy --workspace --all-targets -- -D warnings`), shell script lints (`shellcheck *.sh scripts/*.sh`), and manpage validation (`man -l bashmenu.1`).

## Environment Inspection & Universal Macro Enhancements
- [x] Sourced `{version}` and `{which}` version information from repository `Cargo.toml` (`workspace.package.version`), with `env!("CARGO_PKG_VERSION")` fallback, and removed `{app.version}` to eliminate ambiguity.
- [x] Formatted `{which}` table headers with `[b]Information[/b]` and `[b]Truth[/b]`.
- [x] Implemented native Markdown rendering in `MessageModal` for headings (`##`) and pipe-separated tables (`|`) with box-drawing borders and BBCode span formatting.
- [x] Implemented `{which}` macro and placeholder displaying interactive environment paths message box.
- [x] Added `full_file_location` and explicit path overrides in `MacroContext::with_paths`.
- [x] Enhanced `MessageModal` width clamping up to 78 columns with wrapped BBCode span preservation for formatted tables.
- [x] Added menu item and shortcut interception in `bashmenu-cli` for `{which}` triggers.
- [x] Added unit tests in `crates/bashmenu-core/tests/macro_engine_test.rs` and integration tests in `crates/bashmenu-cli/tests/audit_items_test.rs`.
- [x] Updated 6-tier parity documentation (`PLACEHOLDER_HELP_TEXT`, `bashmenu.md`, `bashmenu.1`, `invariants.md`, `SKILL.md`).

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

## Phase 15: Theme Editor Collapsible Tree, Quiet Flag, Plugin Color, Flicker Elimination, `bmpager` Integration & TODO Protocol
- [x] **Menu Editor `quiet` Flag (`menuedit`, `bashmenu-core`, `bashmenu-cli`):**
  - Added `quiet: bool` to `MenuItem` schema and exposed `[x] Quiet Mode (No Pause)` in `ItemEditModal` for `command` and `script` items.
  - Updated `bashmenu-cli` console execution path to suppress `"Press [ENTER] to return to menu..."` when `item.quiet` is `true`.
- [x] **Plugin Text Color Token (`themeedit`, `bashmenu-core`, `bashmenu-tui`):**
  - Added `plugin` style token to `Theme`, `ResolvedThemeStyles`, wallpaper auto-derivation (`derive_colors_from_wallpaper`), and `themeedit` (`Plugin Text Color`), rendering plugin output lines with `self.styles.plugin`.
- [x] **Flicker-Free Subprocess Resume & Synchronized Redraw (`TerminalGuard`):**
  - Eliminated `terminal.clear()` (`\x1b[2J`) on subprocess return and `<F5>` refresh in favor of `guard.request_clear()` buffer sentinel invalidation and DEC Mode 2026 (`\x1b[?2026h`...`\x1b[?2026l`) synchronized output.
  - Added `BASHMENU_CHILD_ALT=1` coordination via `guard.suspend_keep_alt(...)` so child TUIs (`bashedit`, `menuedit`, `themeedit`, `bmpager`) remain smoothly in the parent's alternate screen buffer.
- [x] **Built-in Text Editor `<F6>` & Main Menu `<F1>` Delegation to `bmpager --markdown`:**
  - Updated `bashedit` `<F6>` Markdown preview to launch `bmpager --markdown --no-file <temp_file>` via `guard.suspend_keep_alt`.
  - Updated `bashmenu` `<F1>` Help action to launch `bmpager --markdown <bashmenu.md>` via `guard.suspend_keep_alt`.
- [x] **Theme Editor Collapsible Token Tree (`themeedit`):**
  - Grouped tokens into collapsible `Theme`, `Colors`, and `Borders` category nodes (`TokenCategory`, `TreeRow`), collapsed by default.
  - Added `Left`/`Right`/`Enter`/`Space` and mouse click toggling on category headers, plus automatic category expansion and token focus when clicking items in the preview pane (`select_token_by_key`).
  - Added integration test `test_collapsible_token_tree_default_and_navigation` in `crates/themeedit-cli/tests/themeedit_test.rs`.
- [x] **`bmpager` Fenced Code Block Frame Alignment (`bmpager`):**
  - Fixed `MarkdownRenderer::render_code_block` in `crates/bmpager-cli/src/markdown.rs` to account for all 4 inner margin spaces (`max_code_w = box_w.saturating_sub(4)`), expand tabs, and clip long code lines so the right vertical border `│` aligns strictly with `┐` and `┘`.
  - Added regression test `test_bmpager_code_block_frame_exact_border_alignment` in `crates/bmpager-cli/tests/bmpager_test.rs`.
- [x] **Documentation & Manpage Synchronization (`bashmenu.md`, `bashmenu.1`, `ymlcheck`):**
  - Synchronized `bashmenu.md` and `bashmenu.1` with all native Rust binaries (`bashmenu`, `menuedit`, `bashedit`, `themeedit`, `bmpager`, `ymlcheck`), keybindings, `{dev_dir}`, and `type: toast` / `{toast:...}`. Added `"toast"` to `VALID_OPTION_TYPES` in `ymlcheck`.
- [x] **CRITICAL TODO Lifecycle Protocol (`SKILL.md`, `AGENTS.md`, `DONE.md`):**
  - Documented mandatory `PENDING` task marking, adding unlisted user instructions to the TODO list, logging completed tasks to `DONE.md`, removing finished tasks when no more tasks remain, and re-checking the TODO list upon completion in `.agents/skills/bashmenu-rs-developer/SKILL.md` and `AGENTS.md`.

## Phase 16: Custom Keyboard Shortcuts (`shortcuts.yml`)
- [x] **Schema & Persistence (`bashmenu-core::shortcuts`):** `ShortcutsConfig` (`leader_key`, `leader_timeout_ms`, `show_leader_hint`, `shortcuts`) and `ShortcutEntry` (key, description, type, action/command/script/file, execution_mode, quiet, alt_buffer, window_percent, tabstop, timeout, enabled). `load_shortcuts` / `save_shortcuts` preserve `SHORTCUTS_HEADER`; `resolve_shortcuts_path` follows the config path hierarchy. Key normalization (`normalize_key_spec`) and validation (`validate_key_spec`) reject unmodified single keys, reserved navigation keys, `F1`–`F8`, and `F11` as direct bindings.
- [x] **Placeholder Parity:** `ShortcutEntry::to_menu_item()` routes shortcuts through the existing `MenuAction::Execute` pipeline. Added `{shortcuts_file}` / `{shortcuts}` placeholders. `{param}` now also accepts inline `{param:prompt=...}` / `{prompt...}` and `:masked`, and `execution_mode:` is accepted as an alias for `exec_mode:`.
- [x] **Leader Chord Engine & Popup (`bashmenu-tui::shortcuts_overlay`):** `ShortcutDispatcher` handles direct shortcuts, `<Ctrl+Space>` leader (normalizes `Char(' ')+CONTROL`, `Char('\0')`, `KeyCode::Null`), timeout expiry, a compact which-key popup overlay (keyboard and mouse selection), and unbound-key toasts. Wired into `bashmenu`; `<F5>` reloads `shortcuts.yml`. The marquee pauses while the leader popup is active.
- [x] **Standalone Editor (`shortcutedit` / `menuedit --shortcuts`):** Split-screen list + inspector with expanded macro preview; `ShortcutEditModal` (type, target + browse, execution mode, window %, quiet, alt buffer, enabled); `KeyCaptureModal` records direct combos or `<leader> <key>` chords; `LeaderSettingsModal`; add/edit/toggle/delete/reorder/save.
- [x] **Integration:** Added menu entries under Bashmenu Settings, `ymlcheck --shortcuts` (duplicate/reserved key detection), `install.sh` / `uninstall.sh` support for `shortcutedit` and `shortcuts.yml`, a fixture, plus `bashmenu.md` / `bashmenu.1` docs.
- [x] **Tests:** Added unit tests for key normalization/validation, entry conversion, Ctrl+Space normalization, and leader dispatch; a fixture roundtrip and inline `{param}` test; and a shortcut editor state/capture/modal test. fmt, clippy (`-D warnings`), all workspace tests, and shellcheck pass.

## Phase 17: `libyml` Scanner Memory Bounds Fix, Shortcut Editor Gutter/Borders Alignment & Periodic Progress Update Directive
- [x] **CRITICAL BUG: `libyml` Flow Scalar String Join Overflow Fix (`patches/libyml`):**
  - Resolved `String join would overflow memory bounds` panic in `libyml::scanner::yaml_parser_scan_flow_scalar` when parsing multi-line quoted scalars with line breaks.
  - Patched `JOIN!` macro in `patches/libyml/src/macros.rs` to loop `yaml_string_extend` reallocations rather than panicking when `string.pointer + b_len > string.end`.
  - Added cargo workspace patch `[patch.crates-io]` for `libyml`.
  - Added regression test `test_libyml_string_join_overflow_bug_fixed` in `crates/bashmenu-core/tests/schema_fixtures_test.rs`.
- [x] **Keyboard Shortcut Editor Gutter and Border Alignment (`shortcutedit`, `crates/menuedit-cli`):**
  - Fixed `render_shortcut_editor_screen` and `draw_connected_panel` in `crates/menuedit-cli/src/shortcut_editor.rs` so the bottom inner corners of the two split panels render right angles (`┘` for left panel, `└` for right panel) instead of tees (`┤` and `├`).
  - Added continuous outer side borders `│` on column 0 and column `width - 1` across all help gutter rows between the panel bottom and the bottom window border `└...┘`.
  - Added regression test `test_shortcut_editor_borders_and_gutter_alignment` in `crates/menuedit-cli/tests/menuedit_test.rs`.
- [x] **Long-Running Process Periodic Status Updates Directive (`SKILL.md`, `AGENTS.md`):**
  - Documented requirement to give a short blurb of progress at least every 2 minutes during processes/tasks exceeding 1 minute in `.agents/skills/bashmenu-rs-developer/SKILL.md` and `AGENTS.md`.

## Phase 18: Popup Modal Text Clamping & Word Wrapping Invariant (`shortcutedit`, `bashmenu-tui`)
- [x] **Popup Modal Text Clamping & Wrapping Invariant (`SKILL.md`, `AGENTS.md`):**
  - Documented invariant in `.agents/skills/bashmenu-rs-developer/SKILL.md` (Section 7) and `AGENTS.md` (Section 3): Text inside popup modals MUST NEVER exceed visible window width. In cases where it may exceed window width, text MUST be wrapped (e.g. using `wrap_prompt_text`) or safely clipped within available inner columns, preventing text from spilling outside borders or wrapping onto borders.
- [x] **Keyboard Shortcut Program Modal Wrapping (`shortcutedit`, `crates/menuedit-cli`, `crates/bashmenu-tui`):**
  - Made `wrap_prompt_text` public in `bashmenu_tui` and used it across `ConfirmModal`, `MessageModal`, and `WhichKeyPopup`.
  - In `KeyCaptureModal`, wrapped prompt strings and error messages using `wrap_prompt_text`, safely clipping status lines and footer hints.
  - In `LeaderSettingsModal`, wrapped error messages, clamped hints, and converted footer into a 2-line wrapped layout within available inner columns.
  - In `ShortcutEditModal`, clamped `composed_key`, dynamically formatted item type options, execution mode selectors, and window percent hints to available width; compacted checkboxes; wrapped error messages; and converted footer hints into a 2-line wrapped layout.
  - Added regression test `test_shortcut_modals_text_does_not_exceed_window_width` in `crates/menuedit-cli/tests/menuedit_test.rs`.

## Phase 19: Comprehensive Application Documentation & Code Commenting Overhaul
- [x] **Architecture & Root Documentation (`docs/ARCHITECTURE.md`, `README.md`):**
  - Updated `docs/ARCHITECTURE.md` to reflect all binaries (`bashmenu`, `menuedit`, `shortcutedit`, `themeedit`, `bashedit`, `bmpager`, `ymlcheck`), leader chord mechanics, wallpaper compositing, and verification workflows.
  - Updated `README.md` with complete command reference, prerequisite instructions, features, and direct execution examples.
- [x] **`bashmenu-core` Module Headers, API Examples & Line Commentary:**
  - Added module-level doc comments (`//!`) and executable doctests in `menu.rs` and `shortcuts.rs`.
  - Added step-by-step explanatory line comments inside `menu.rs` custom deserialization routines (distinguishing scalar shorthand, normalized label maps, and canonical YAML label-as-key dictionaries).
  - Added step-by-step explanatory line comments for the 6-tier macro interpolation loop in `macro_engine.rs`.
  - Added module headers and architecture notes in `shortcuts.rs` and `plugin.rs`.
- [x] **Interactive CLI Commentary & Event Loops:**
  - Added step-by-step commentary for field focus indexes (0..=13), navigation cycling, key capturing, and main screen actions in `crates/menuedit-cli/src/shortcut_editor.rs`.
  - Added subsystem architecture commentary for the main loop in `crates/bashmenu-cli/src/main.rs`.
- [x] **Complete Workspace Documentation Coverage (`-W missing_docs`):**
  - Added comprehensive documentation comments (`///`) to every public and internal struct, enum, trait, field, function, and method across all workspace crates (`bashmenu-core`, `bashmenu-tui`, `menuedit-cli`, `themeedit-cli`, `bashedit-cli`, `bmpager-cli`, `ymlcheck-cli`).
  - Documented theme token schemas, border character mappings, style pairs, visual wrapped lines, multi-line inputs, swap file headers, modal prompt states, and selection anchors.
  - Achieved 100% clean check under `cargo clippy --workspace --all-targets -- -D warnings` with zero missing documentation warnings across production libraries.

## Phase 20: Exhaustive Codebase Function & Logic Documentation
- [x] **Universal Doc Comments Across All Internal & Private Functions:**
  - Conducted an automated AST line scan across all `.rs` files in `crates/*/src/` to identify every private function, helper, serde default generator, and trait implementation (`Default`, `Drop`, `From`, `Serialize`, `Deserialize`) lacking doc comments.
  - Added doc comments (`///`) and explanatory line comments to all 170+ identified functions across all crates:
    - `bashmenu-core`: `interpolate_internal`, `default_version`, `default_title`, `default_sleep`, `default_note_type`, `default_format`, `default_marquee_char`, `default_marquee_gap`, `default_opacity`, `default_scaling`, `default_quantization`, `default_leader_key`, `default_leader_timeout_ms`, `default_true`, `is_true`, `from`, `default`, `serialize`, `deserialize`, `inline_param_spec`, `find_inline_param_span`, `title_case`, `is_hex_token`, `get_local_time_strings`, `resolve_profile_path`, `resolve_shell_profile`, `get_primary_ip`, `get_battery_info`, `lookup_value_path`, `eval_command_substitution`, `expand_tilde_path`.
    - `bashmenu-tui`: `drain_stale_events`, `rgb_to_hsv`, `color_luminance`, `hue_distance`, `chord_keys_match`, `current_level`, `current_level_mut`, `get_help_raw`, `action_for_gutter_token`, `activate_shortcut_index`, `activate_selected`, `go_back`, `sync_yes`, `is_table_row`, `is_table_delimiter`, `parse_cells`, `render_table`, `current_line`, `insert_char`, `insert_str`, `handle_backspace`, `handle_delete`, `handle_enter`, `parse_ordered_list`, `detect_list_margin`, `wrap_chars_by_word_with_margin`, `wrap_chars_by_word`, `get_selectable_lines`, `parse_checkbox_line`, `parse_bullet_line`, `drop`.
    - `menuedit-cli`: `resolve_menu_path`, `resolve_script_path`, `main`, `run_glyphs_search`, `flatten_items`, `collect_all_submenu_paths`, `find_by_title_recursive`, `get_container_items_mut`, `draw_connected_panel`, `has_ctrl`, `render`, `default`.
    - `themeedit-cli`: `resolve_themes_path`, `main`, `open_selected_token_picker`, `hit_test_preview`, `sync_selected_idx_from_row`.
    - `bashedit-cli`: `locate_tool`, `launch_markdown_preview`, `resolve_path_tilde`, `make_save_as_modal`, `main`, `render_modal`, `show_save_toast`, `handle_active_modal`.
    - `bmpager-cli`: `print_usage`, `parse_cli_args`, `resolve_editor_mode`, `find_bashedit_exe`, `get_system_editor`, `new`, `flush_line`, `ensure_blank_line`, `emit_word`, `run`, `handle_styled_text`, `render_heading`, `render_code_block`, `render_table`, `finish`, `resolve_start`, `clamp_top`, `query_size`, `handle_key`, `build_bar`, `render`, `prompt_search`, `search_forward`, `search_next`, `choose_link`, `open_in_editor`, `reload_spool`, `default`.
    - `ymlcheck-cli`: `validate_yaml_syntax`, `validate_theme_file`, `validate_config_file`, `validate_menu_file`, `check_items`, `validate_shortcuts_file`, `main`.
  - Re-scanned to verify 0 undocumented non-test functions remain across the entire repository.
- [x] **Verification & Lifecycle Conformance:**
  - Verified `cargo fmt --all --check` passed cleanly.
  - Verified `cargo clippy --workspace --all-targets -- -D warnings` passed with 0 warnings.
  - Verified `cargo test --workspace` passed 100% (all 63+ tests and doc-tests passing).
  - Verified `shellcheck *.sh scripts/*.sh` passed cleanly.
  - Verified `./install.sh` built release binaries and installed cleanly.
  - Pushed to `main` (`80eb328`) and verified GitHub Actions CI run `41568390777` succeeded.

## Phase 21: Exhaustive Struct, Enum, Field, and Variant Documentation
- [x] **Universal Documentation of Structs, Enums, Type Aliases, and Constants:**
  - Scanned every `.rs` file across all workspace crates for un-documented structs, enums, unions, type aliases, and constants.
  - Added doc comments to:
    - CLI arguments: `Cli` across `bashmenu-cli`, `menuedit-cli`, `shortcutedit`, `themeedit-cli`, `bashedit-cli`, `ymlcheck-cli`.
    - Modal dialogs: `ActiveModal` and `ChildModal` variants across all interactive TUIs.
    - Data schemas and constants: `SHORTCUT_TYPES`, `VALID_16_COLORS`, `VALID_WINDOW_BORDER_KEYS`, `VALID_OPTION_TYPES`, `HELP_LINES`, `OSC8_RE`, `MD_LINK_RE`, `BARE_URL_RE`, `ANSI_CSI_RE`, `ANSI_OSC_RE`, `NF_RE`, `ENV_RE`, `DOT_RE`, `COMMAND_RE`, `FILE_PICKER_MACRO_RE`, `DIR_PICKER_MACRO_RE`, `PICKER_MACRO_RE`, `CMD_CACHE_TTL_SECS`, `CMD_CACHE`, `TOAST_RE`, `IN_MEMORY_CLIPBOARD`, `TEST_MODE`, `THREAD_CLIPBOARD`, `CHARSET`, `TABLE`.
    - Data models and internal helpers: `CompositorCacheKey`, `MenuLevel`, `DirItem`, `DocumentLink`, `RawItemProperties`, `InlineParamSpec`, `LocalTimeStrings`, `MarkdownRenderer`, `ListState`, `Pager`, `PagerArgs`, `FooterHitRegion`, `MenuEditFooterAction`, `FooterAction`.
- [x] **100% Struct Field and Enum Variant Documentation Coverage:**
  - Built an AST scanner specifically detecting any struct field (public or private) or enum variant lacking doc comments.
  - Documented all 27 identified struct fields and 9 enum variants across `WallpaperCompositor`, `ExtractedColor`, `ShortcutDispatcher`, `MenuView`, `TerminalGuard`, `ToastRack`, `StreamOutputModal`, `FilePickerModal`, `PluginEngine`, `ActiveModal`, and `FormRow`.
  - Re-scanned to confirm **0 undocumented struct fields and 0 undocumented enum variants** remain across the entire repository.
- [x] **Verification & Lifecycle Conformance:**
  - Verified `cargo fmt --all --check` passed cleanly.
  - Verified `cargo clippy --workspace --all-targets -- -D warnings` passed with 0 warnings.
  - Verified `cargo test --workspace` passed 100% across all crates.
  - Verified `shellcheck *.sh scripts/*.sh` passed cleanly.
  - Verified `./install.sh` built release binaries and installed cleanly to `$HOME/.local/bin`.
  - Pushed to `main` (`5a467d6`) and verified remote CI run `41570376526` passed with status `success`.

## Phase 24: Rules Reorganization (.agents/rules/invariants.md)
- [x] **Relocated Root `AGENTS.md` to `.agents/rules/invariants.md`:**
  - Moved `AGENTS.md` from the workspace root into `.agents/rules/invariants.md` conforming to industry and Antigravity standard conventions for workspace rules.
  - Updated documentation map in `docs/ARCHITECTURE.md`.
  - Updated `SKILL.md` 6-tier reference modal list to include `crates/bmpager-cli`.
  - Verified remote CI run `41747999484` succeeded on GitHub Actions.

- [x] **CLI Argument & Flag Parsing (`crates/bmpager-cli`):**
  - Added `wrap_text: bool` option to `PagerArgs`.
  - Added `-w`, `--wrap`, and `--wrap-text` flags to `parse_cli_args`.
  - Updated `print_usage` and interactive `<F1>` / `h` / `?` help modal to document `-w, --wrap`.
- [x] **ANSI-Preserving Word Wrapping Engine (`crates/bmpager-cli/src/ansi.rs`):**
  - Implemented `wrap_text_line` and `wrap_text_content` soft word-wrapping functions that measure visual column width, respect indentations, preserve active ANSI styling across lines, and hard-split words exceeding window width.
  - Connected word-wrapping to `bmpager`'s main processing pipeline when non-markdown text is viewed and `--wrap` is specified.
- [x] **Documentation & Test Parity:**
  - Added unit tests for plain text, indented text, and long unbroken words in `ansi.rs`.
  - Added CLI integration test `test_bmpager_wrap_flag_plain_text` in `crates/bmpager-cli/tests/bmpager_test.rs`.
  - Updated `bashmenu.md` and `bashmenu.1` manpage documenting the `-w` / `--wrap` / `--wrap-text` options.
- [x] **Verification & Remote CI:**
  - Verified `cargo fmt --all --check` passed cleanly.
  - Verified `cargo clippy --workspace --all-targets -- -D warnings` passed with 0 warnings.
  - Verified `cargo test --workspace` passed 100%.
  - Verified `shellcheck *.sh scripts/*.sh` passed cleanly.
  - Committed and pushed commit `8fa5d33` to `origin/main`.
  - Verified remote CI run `41743716695` passed with status `success`.

- [x] **Main Menu Help Gutter `<F9>` Integration (`crates/bashmenu-tui`, `crates/bashmenu-cli`):**
  - Added `[F9]: ShortcutEdit` to contextual help gutters across all menu states in `MenuView::get_help_raw`.
  - Removed redundant `[0-9/a-z]: Direct` help token from `get_help_raw` when shortcut badges are displayed, freeing up essential visual width.
  - Introduced `MenuAction::LaunchShortcutEditor` to `MenuAction` enum and wired `KeyCode::F(9)` inside `MenuView::handle_key`.
  - Updated gutter mouse click resolution in `MenuView::action_for_gutter_token` to prioritize `[F8]` (`ThemeEdit`) and `[F9]` (`ShortcutEdit`) before generic `Edit` (`menuedit`), ensuring reliable mouse clicks on `<F9>` launch `shortcutedit`.
  - Wired `MenuAction::LaunchShortcutEditor` inside `bashmenu-cli`'s main event loop to invoke `locate_tool("shortcutedit")` with fallback to `menuedit --shortcuts`.
  - Updated `bashmenu.md`, `bashmenu.1`, and the `<F1>` Help modal documentation to document `<F9>`.
- [x] **Verification & Remote CI:**
  - Verified `cargo fmt --all --check` passed cleanly.
  - Verified `cargo clippy --workspace --all-targets -- -D warnings` passed with 0 warnings.
  - Verified `cargo test --workspace` passed 100% (all 64+ tests and doc-tests passing).
  - Verified `shellcheck *.sh scripts/*.sh` passed cleanly.
  - Committed and pushed commit `36aa16e` to `origin/main`.
  - Verified GitHub Actions remote CI run `41739787382` passed with status `success`.

- [x] **Global Toast Message Timeout Migration (`crates/bashmenu-core`, `crates/bashmenu-tui`, `crates/bashmenu-cli`, `crates/themeedit-cli`):**
  - Moved `toast_timeout` setting from themes (`bashmenu.themes`) into global settings (`bashmenu.yml` under `settings: toast_timeout: 2.0`).
  - Added `toast_timeout: Option<f64>` to `Settings` struct in `crates/bashmenu-core/src/config.rs`.
  - Added dot-notation config mutation support for `settings.toast_timeout` and `settings.toast.timeout` (with automatic `app.` prefix resolution) in `BashMenuConfig::set_value`.
  - Updated `macro_engine.rs` toast extraction to fall back to `ctx.config.settings.toast_timeout.unwrap_or(2.0)`.
  - Updated `ResolvedThemeStyles` with `.with_toast_timeout(...)` and integrated it across `bashmenu-cli`'s startup, theme change modal, config input modal, and `<F5>` reload handler.
  - Added interactive configuration menu item in `bashmenu.mnu` (`Toast Message Timeout [[{app.settings.toast_timeout}] seconds]`) to configure toast timeout dynamically.
  - Removed toast timeout cycling and tokens from `themeedit` (`crates/themeedit-cli`) so theme editing focuses purely on visual styles.
  - Verified `cargo fmt`, `cargo clippy`, `cargo test`, `shellcheck`, and `ymlcheck` validation passed.
  - Committed and pushed changes to `origin/main` (commit `47db436`).

- [x] **Native Rust Cheatsheet Manager Binary (`crates/cheat-cli`, `cheat`):**
  - Created standalone Rust tool `cheat` (`crates/cheat-cli`) replacing `scripts/cheat.py` and `scripts/cheat.sh` with 1:1 parity of function.
  - Implemented command-line argument parsing (`args.rs`) with `clap` supporting optional queries, `-l`/`--list` (with optional query filter), `-s`/`--search`, and `-d`/`--directory`.
  - Implemented `.cheat.yml` configuration lookup and auto-creation (`config.rs`), supporting environment overrides (`CHEAT_CONFIG`), executable parent directory, user config directory, and home directory.
  - Implemented YAML frontmatter parser (`frontmatter.rs`) extracting lowercased tags from YAML sequences, multi-line lists, and comma-delimited strings.
  - Implemented cheatsheet discovery and search (`cheatsheet.rs`) with recursive `.md` file collection, case-insensitive relative path sorting, 3-priority search (stem/path match, tag match, full-text content match), and two-column terminal list formatting.
  - Implemented rendering pipeline (`render.rs`) prioritizing `rich.sh`, native `bmpager --markdown`, and raw terminal stdout printing.
  - Updated `scripts/cheat.sh` trampoline wrapper to prioritize executing the native `cheat` binary with self-referential loop protection and Python fallback.
  - Updated `install.sh` to compile and install `cheat` alongside existing workspace binaries.
  - Synchronized documentation in `bashmenu.md`, `bashmenu.1`, `assets/cheat.md`, and `SKILL.md`.
  - Added unit test suite and integration test suite (`tests/cli_tests.rs`). Verified all tests pass cleanly.

- [x] **Relocated Cheatsheet Configuration to `~/.config/bashmenu/cheat.yml`:**
  - Removed legacy `scripts/.cheat.yml`.
  - Added fixture template `tests/fixtures/cheat.yml` and updated `install.sh` to install `cheat.yml` into `~/.config/bashmenu/cheat.yml`.
  - Updated `cheat-cli` (`crates/cheat-cli/src/config.rs`) to load `cheat.yml` strictly from `~/.config/bashmenu/cheat.yml` (with `CHEAT_CONFIG` override and fixtures fallback), ensuring `scripts/` is never queried.
  - Updated `scripts/cheat.py` to target `~/.config/bashmenu/cheat.yml`.
  - Updated documentation across `bashmenu.md`, `bashmenu.1`, and `assets/cheat.md`.
  - Verified tests, Clippy, and formatting pass cleanly.

- [x] **Removed `cheat.py` & All Python Fallbacks:**
  - Removed `scripts/cheat.py` from repository.
  - Updated `scripts/cheat.sh` trampoline to strictly invoke the native `cheat` binary, outputting a compilation error if the binary is not found.
  - Removed references to `scripts/cheat.py` from `bashmenu.md`, `bashmenu.1`, and internal code comments.
  - Verified tests, Clippy, formatting, and Shellcheck pass cleanly.

- [x] **Theme Editor: Display Clicked Color Code in Preview Pane Toast:**
  - Added preview buffer tracking in `crates/themeedit-cli/src/main.rs` to snapshot the rendered cells of `right_area` directly from `render_preview_pane`.
  - Implemented `format_color_code(color: Color) -> String` in `crates/themeedit-cli/src/preview.rs` supporting RGB TrueColor hex (`#rrggbb`), ANSI 256 indexed colors (`N`), named colors (`cyan`, `black`, etc.), and `default` for `Color::Reset`.
  - Implemented `extract_preview_color_at(buf, col, row, fallback_styles) -> String` in `preview.rs` resolving cell background for whitespace, half-block foreground for composited wallpaper (`▀`), character foreground for glyphs/borders/text, and falling back gracefully to theme background or text colors.
  - Updated mouse click handling for preview pane in `main.rs` to display a 2-line toast notification:
    `Selected: <item label>`
    `Color Clicked: <color code>`
  - Added unit test suite `test_format_color_code_and_extract_preview_color` in `crates/themeedit-cli/tests/themeedit_test.rs`.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed changes to `origin/main` (commit `51e7f99`), verified GitHub Actions CI run `42137829166` passed.

- [x] **Theme Editor: TrueColor (24-bit RGB) Selection Support & Modal (`crates/bashmenu-tui`, `crates/themeedit-cli`):**
  - Implemented `ColorPickerModal` (`crates/bashmenu-tui/src/modals/color_picker.rs`) featuring a 3-tab interface:
    - Tab 1: `[1: 256 Palette]` (classic 16x16 ANSI 256 color grid with keyboard & mouse navigation).
    - Tab 2: `[2: TrueColor (HSV)]` (continuous 32-column x 14-row half-block Saturation/Value canvas with 28 vertical resolution steps, vertical Hue bar slider $0^\circ$–$360^\circ$, live swatch preview, high-contrast crosshair cursor, and RGB/HSV/ANSI breakdown).
    - Tab 3: `[3: Hex Code]` (direct 24-bit `#RRGGBB` hex code editor with live validation, swatch display, and channel stats).
  - Added math helpers `rgb_to_hsv`, `hsv_to_rgb`, and `parse_hex_rgb`.
  - Added support for keyboard cycling (`Tab` / `Shift+Tab`, number keys `1`/`2`/`3`, arrow keys, vim keys, `[` / `]`) and immediate mouse clicks across tabs, grids, canvases, and sliders.
  - Extended `ThemeEditorState` (`crates/themeedit-cli/src/lib.rs`) with `set_color_token_value(&mut self, key, color)` and `get_color_token_color(&self, key) -> Color`, storing 24-bit TrueColor hex strings (`#rrggbb`) or ANSI numbers in theme YAML.
  - Updated `get_token_value_display` to display TrueColor hex strings cleanly in the token sidebar.
  - Updated `crates/themeedit-cli/src/main.rs` to open `ColorPickerModal` with the current token's exact color and apply chosen TrueColor or ANSI colors.
  - Added unit and integration test suites in `crates/bashmenu-tui/tests/modals_test.rs` and `crates/themeedit-cli/tests/themeedit_test.rs`.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `6895fff`), verified remote GitHub Actions CI run `42144431341` passed.

- [x] **Theme Editor: Preview Toast 256 Decimal Code & Color Picker Close Button Fix:**
  - Updated `format_color_code` in `crates/themeedit-cli/src/preview.rs` to always include the 256 color code in decimal representation, e.g. `#rrggbb (256: N)` for TrueColor and `N (256: N)` for ANSI colors.
  - Fixed `[X]` close button handling in `ColorPickerModal` (`crates/bashmenu-tui/src/modals/color_picker.rs`) by integrating `is_close_button_clicked` check against the modal frame.
  - Updated `crates/themeedit-cli/src/main.rs` mouse event routing for `ColorPicker` to pass full viewport `area` for proper close button geometry detection.
  - Updated unit and integration tests in `crates/bashmenu-tui/tests/modals_test.rs` and `crates/themeedit-cli/tests/themeedit_test.rs`.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `43fda5c`), verified remote GitHub Actions CI run `42146591746` passed.

- [x] **Theme Editor: 16-Color and 8-Color Theme Variant Auto-Derivation:**
  - Extended theme color auto-derivation in `crates/themeedit-cli/src/lib.rs` (`derive_colors_from_wallpaper`) and `set_color_token_value` to automatically generate and downsample palettes to valid ANSI 16-color (`colors_16`) and 8-color (`colors_8`) theme tiers.
  - Implemented `rgb_to_ansi16_name`, `rgb_to_ansi8_name`, `downsample_palette_to_16`, and `downsample_palette_to_8` in `crates/bashmenu-tui/src/wallpaper.rs` mapping colors via HSV hue classification and relative perceptual luminance.
  - Added unit and integration tests in `crates/bashmenu-tui/tests/wallpaper_test.rs` and `crates/themeedit-cli/tests/themeedit_test.rs`.
  - Verified `ymlcheck -t bashmenu.themes` validates cleanly against schema standards.

- [x] **Theme Editor: Glyph & Color Grid Pickers Mouse Clickable Close Button Fix:**
  - Updated `AsciiGridModal::handle_mouse` and `ColorGridModal::handle_mouse` in `crates/bashmenu-tui/src/modals/grid_picker.rs` to detect mouse clicks on the `[X]` close button via `is_close_button_clicked`, returning `Some(None)` to dismiss the dialog.
  - Added unit test in `crates/bashmenu-tui/tests/modals_test.rs` verifying that clicking `[X]` on both `AsciiGridModal` and `ColorGridModal` dismisses them immediately.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `6747cdf`), verified remote GitHub Actions CI run `42149666239` passed.

- [x] **File and Directory Pickers: Hidden Files Toggle:**
  - Added `show_hidden: bool` field, `with_show_hidden(bool)` builder method, and `toggle_hidden()` to `FilePickerModal` (`crates/bashmenu-tui/src/modals/file_picker.rs`).
  - Added keybindings `h`, `H`, `.`, and `Ctrl+H` to toggle visibility of hidden dotfile entries during navigation.
  - Added interactive mouse button on bottom controls bar (`[h] Show Hidden` / `[h] Hide Hidden`) to toggle hidden files.
  - Added unit and integration test in `crates/bashmenu-tui/tests/modals_test.rs` verifying hidden file skipping, toggling via keys and mouse, and badge state changes.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `56a3239`), verified remote GitHub Actions CI run `42150154497` passed.

- [x] **Shortcuts: Hardcode F9 to Shortcut Editor & Reserve from shortcuts.yml:**
  - Removed `<F9>` entry from `shortcuts.yml`, `tests/fixtures/shortcuts.yml`, and `~/.config/bashmenu/shortcuts.yml`.
  - Added `"F9"` to `RESERVED_DIRECT_KEYS` in `crates/bashmenu-core/src/shortcuts.rs`, ensuring it cannot be intercepted or overwritten by user-defined shortcuts.
  - Ensured `<F9>` is hardcoded across all menu modes in `MenuView` (`crates/bashmenu-tui/src/menu_view.rs`) returning `MenuAction::LaunchShortcutEditor`.
  - Added `[F9]: ShortcutEdit` to the help gutter across all modes (standard, notebook, shortcuts view) with mouse clickable support.
  - Updated key capture prompt in `crates/menuedit-cli/src/shortcut_editor.rs` from `F9` to `F10`.
  - Updated documentation in `bashmenu.md` and `bashmenu.1`.
  - Added/updated unit tests in `crates/bashmenu-core`, `crates/bashmenu-tui`, and `crates/menuedit-cli`.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `acbecf5`), verified remote GitHub Actions CI run `42153477926` passed.

- [x] **Help Gutter & Install/Uninstall Lifecycle Invariants:**
  - Omitted `[UP/DN]: Nav` help gutter token across all menu modes in `MenuView` (`crates/bashmenu-tui/src/menu_view.rs`), saving screen width.
  - Repositioned priority keys `[F8]: ThemeEdit` and `[F9]: ShortcutEdit` immediately after `[F2]: Themes` in the help gutter, guaranteeing visibility on 2nd gutter row on standard 80- and 100-column terminals.
  - Implemented dynamic gutter width balancing (`calculate_gutter_dimensions`) so the help gutter is not starved by a static 50/50 split when status tokens are short or absent.
  - Updated theme preview help gutter in `crates/themeedit-cli/src/preview.rs` to match bashmenu gutter tokens.
  - Enhanced `crates/bashmenu-tui/tests/menu_navigation_test.rs` asserting `[UP/DN]: Nav` is omitted and `[F9]: ShortcutEdit` is visible and clickable at 100-column width.
  - Updated `install.sh` to install `bashmenu.1` manual page to `$MAN_DIR` (`~/.local/share/man/man1`) and explicitly list `cheat` under "Commands installed".
  - Updated `uninstall.sh` to clean up `$MAN_DIR/bashmenu.1`, default to keeping user settings/notebooks in `~/.config/bashmenu` (`[Y/n]` prompt), and support `--keep-settings` / `--keep-config` (`-k`), `--remove-settings`, and `-y`/`--yes`.
  - Updated [`.agents/skills/bashmenu-rs-developer/SKILL.md`](.agents/skills/bashmenu-rs-developer/SKILL.md) documenting install/uninstall lifecycle and help gutter layout invariants.
  - Passed `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, and `shellcheck *.sh scripts/*.sh`.
  - Committed and pushed changes (commit `2c8503e`), remote GitHub Actions CI run `42220198911` passed with `'success'`.

- [x] **Consolidate Rules into .agents/rules/invariants.md & Remove Root AGENTS.md:**
  - Merged installation/uninstallation invariants and help gutter priority/omission invariants into permanent rules file [`.agents/rules/invariants.md`](.agents/rules/invariants.md).
  - Deleted redundant `AGENTS.md` from the root directory to keep all agentic files tidy inside `.agents/`.
  - Updated [`.agents/skills/bashmenu-rs-developer/SKILL.md`](.agents/skills/bashmenu-rs-developer/SKILL.md) to explicitly designate `.agents/rules/invariants.md` as the default location for permanent rules and invariants.
  - Verified `cargo fmt`, `cargo clippy`, and `shellcheck` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `3ac3c58`), verified remote GitHub Actions CI run `42221099793` passed with `'success'`.

- [x] **Help Gutter Function Key Strictly Ascending Order (F1 to F12):**
  - Updated `MenuView::get_help_raw()` across all modes (standard, shortcuts, notebook) in `crates/bashmenu-tui/src/menu_view.rs` to ensure all function keys are strictly listed in ascending numerical order (`F1, F2, F3, F4, F5, F6, F7, F8, F9`).
  - Synchronized theme preview help gutter tokens in `crates/themeedit-cli/src/preview.rs` to maintain strict numerical order.
  - Added unit test assertion in `crates/bashmenu-tui/tests/menu_navigation_test.rs` asserting strict ascending numerical order (`pos_f1 < pos_f2 < ... < pos_f9`).
  - Updated permanent rules in `.agents/rules/invariants.md` and `.agents/skills/bashmenu-rs-developer/SKILL.md`.
  - Verified `cargo fmt`, `cargo clippy`, and `cargo test` pass with 0 warnings.
  - Committed and pushed to `origin/main` (commit `1a773d5`), verified remote GitHub Actions CI run `42222195860` passed with `'success'`.

- [x] **Theme Editor Background Wallpaper Configuration Parity with Python:**
  - Implemented interactive `BackgroundConfigModal` in `crates/bashmenu-tui/src/modals/background_config.rs` matching Python's `BackgroundConfigModal` with full 1:1 option parity:
    - Wallpaper Image file path input + `[ Browse... ]` button launching `FilePickerModal`.
    - Opacity factor input (`0.0` to `1.0`).
    - Scaling algorithm selector: `cover` (fill window), `stretch` (exact fit), `fit` (proportional), `center` (original size), `tile` (repeat pattern).
    - Rendering mode selector: `halfblock` (`▀▄` dual-pixel) and `quarterblock` (`▖▗` quad-pixel).
    - Color clustering distance input (`0`=none, `16` to `128`).
    - Palette quantization step input (`1`=off, `16` to `256`).
    - Action buttons: `[ Apply ]`, `[ Remove Wallpaper ]`, `[ Cancel ]`, plus top-right `[X]` frame close button.
  - Implemented authentic quadrant block compositing (`mode == "quarterblock"`) and color clustering (`clustering > 0`) in `WallpaperCompositor::get_grid` in `crates/bashmenu-tui/src/wallpaper.rs`.
  - Added individual wallpaper sub-tokens in `crates/themeedit-cli/src/lib.rs` under `TokenCategory::Theme` (`Wallpaper Image`, `Wallpaper Opacity`, `Wallpaper Scaling`, `Wallpaper Mode`, `Wallpaper Clustering`, `Wallpaper Quantization`), with live display, key matching, and live span highlights.
  - Wired `BackgroundConfigModal` and `BackgroundBrowse` into `crates/themeedit-cli/src/main.rs`, bound to shortcut <kbd>b</kbd>, <kbd>F4</kbd>, and Enter on any wallpaper token.
  - Added comprehensive unit and integration tests in `crates/bashmenu-tui/tests/background_config_modal_test.rs`, `crates/bashmenu-tui/tests/wallpaper_test.rs`, and `crates/themeedit-cli/tests/themeedit_test.rs`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings or errors.
  - Committed and pushed to `origin/main` (commit `2a7a483`), verified remote GitHub Actions CI run `42226434198` passed with `'success'`.

- [x] **Audit Parity Implementations (Inject Block, Message Modal, Root Elevation, Inactivity Timeout, Status Precedence):**
  - **Status Gutter Precedence & Date/Time Retention**: Updated `menu_view.rs` to compute visible text lengths and prioritize status gutter allocation over help gutter, guaranteeing date/time stamps are never silently dropped in narrow terminals. Verified with `test_status_gutter_precedence_and_date_time_retention`.
  - **Adaptive Polling & `settings.refresh_interval`**: Main event loop adaptively polls at 1s for second-precision placeholders (`{date_time_24}`, etc.) and throttles to `settings.refresh_interval` for minute-precision tokens.
  - **`type: inject_block` Runtime Execution**: Implemented `ActiveModal::ConfirmInjectBlock` in `bashmenu-cli`. Interpolates template and target paths; prompts user with `ConfirmModal` to install/update `# CODEBLOCK:<id>:START ... # CODEBLOCK:<id>:END` with 0o755 script permissions or cleanly remove. Verified with `test_inject_block_marker_injection_and_removal_cycle`.
  - **`type: message` Standalone Item**: Standalone top-level items with `type: message` / `popup` / `info` display `ActiveModal::Message` with interpolated title and message text.
  - **`user_mode: "root"` Automatic Elevation**: Non-root users executing an item with `user_mode: "root"` have the command automatically prepended with `sudo `.
  - **`settings.inactivity_timeout` Runtime Timer**: Main loop tracks key and mouse events via `Instant`, spawning `timeout.command` when idle duration reaches `timeout.milliseconds`. Verified with `test_inactivity_timeout_and_root_elevation_schema`.
  - **Per-Item Editor Options**: Added `--show-whitespace` and `--tab-to-spaces <bool>` flags to `bashedit-cli`, passed by `bashmenu` when launching `type: editor` items. Verified with `test_bashedit_cli_whitespace_and_tab_flags`.
  - **`ymlcheck` Option Type Parity**: Added `"toast"`, `"inject_block"`, and `"message"` to `VALID_OPTION_TYPES` in `ymlcheck-cli`, validated with unit tests.
  - **Verification & CI**: Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings or errors. Pushed commit `c7fc9df` to `origin/main`, verified remote GitHub Actions CI run `42324278916` passed with `'success'`.

- [x] **Extended Screen Transitions & Speed Configuration:**
  - Extended `TransitionEffect` enum with 6 new effects: `Dissolve` (retro Bayer 4x4 dither dissolve), `Matrix` (cascading green digital rain resolving into view), `SlideLeft` (horizontal slide wipe across columns from right to left), `SlideRight` (horizontal slide wipe across columns from left to right), `Blinds` (venetian blinds horizontal slats opening simultaneously), `Iris` (rectangular box aperture zoom expanding from center), and `WipeDown` (vertical top-to-bottom roll wipe).
  - Implemented both transition-in (`draw_with_transition`) and transition-out (`draw_with_transition_out`) algorithms for all effects directly against Ratatui's `Buffer` in `crates/bashmenu-tui/src/terminal.rs`.
  - Implemented `TransitionSpeed` enum (`Fast`, `Normal` [default], `Slow`, `Custom(u64)`) with tuned step delays in `crates/bashmenu-tui/src/transition.rs` and wired into `bashmenu-core` config schema (`settings.transition_speed`).
  - Ensured transitions are active between all interface changes (launch, submenu navigation, back/Esc, page up/down, modals open/close, and app launches/returns).
  - Added radio configuration items for both transition effect and transition speed in `bashmenu.mnu`, `tests/fixtures/bashmenu.mnu`, and `~/.config/bashmenu/bashmenu.mnu`.
  - Documented all new transitions and speed profiles in `bashmenu.md` and `bashmenu.1`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `shellcheck` pass with zero warnings or errors.

- [x] **Comprehensive Workspace Stress Testing & Boundary Hardening:**
  - Developed end-to-end stress test suites across all 7 workspace crates:
    - `bashmenu-core`: Deeply nested curly brace recursion (100 levels), 1MB strings with thousands of macro placeholders, binary and malformed token inputs, multi-threaded concurrent interpolation from 20 threads, large scale 2,000-line 3-way note merge with non-overlapping and conflicting changes, note swap file rapid write/load/remove cycles, 2,000-item YAML menu schema serialization roundtripping.
    - `bashmenu-tui`: Boundary viewport rendering (0x0, 1x1, 2x2, 80x1, 1x24, 300x200), extreme color dimming factors (-100.0 to 100.0), Bayer 4x4 dither bounds safety across 1,000,000 coordinate pairs, transition effect string fuzzing, 1,000-item menu view scrolling and paging under multi-size buffer renders. Guarded `draw_with_transition` and `draw_with_transition_out` against zero-dimension delays.
    - `bashedit-cli`: 10,000-line buffer navigation and multi-line searching, rapid undo/redo cycles, verified 100-snapshot bounded capacity behavior.
    - `bmpager-cli`: Deeply formatted 500-section markdown rendering, streaming 10,000-line piped inputs over non-TTY stdin.
    - `menuedit-cli`: Scale testing with 500 added items, rapid swap up/down reordering, and disk serialization roundtrip.
    - `themeedit-cli`: Adversarial non-existent paths and theme targets, visible row selection, category expansion toggling, 256-color palette mutation cycles.
    - `bashmenu-cli`: CLI binary validation for all 8 workspace executables (`--help`, `--version`), adversarial input testing for `ymlcheck` (non-existent paths, empty files, corrupt YAML, binary bytes).
  - Verified all tests pass with zero warnings in `cargo fmt`, `cargo clippy`, and `shellcheck`.

- [x] **Project-Wide Popup Modals Text Bounds Clamping & Word Wrapping Audit:**
  - Audited all modal frame rendering across `bashmenu-tui`, `menuedit-cli`, `bashedit-cli`, and `bmpager-cli`.
  - In `crates/bashmenu-tui/src/modals/frame.rs`, clamped title rendering with `UnicodeWidthStr::width` and truncated with an ellipsis `…` so long titles never collide with or overwrite the `[X]` close button or exceed the top border.
  - In `crates/bashmenu-tui/src/modals/input.rs`, updated `wrap_prompt_text` to handle zero width safely, chunk continuous words using Unicode visible column width, clamped prompt lines to inner width, and guarded `[ OK ]` and `[ Cancel ]` buttons within inner boundaries.
  - In `crates/bashmenu-tui/src/modals/note_conflict.rs`, wrapped messages with `wrap_prompt_text`, dynamically computed dialog height, and added responsive button sizing (`button_labels`) with clamping to prevent buttons or text from spilling past borders.
  - In `crates/bashmenu-tui/src/modals/confirm.rs`, guarded against invalid `clamp(min, max)` parameter ordering on narrow viewports and clamped action buttons within inner columns.
  - In `crates/bashmenu-tui/src/modals/radio.rs` and `toggle.rs`, wrapped message bodies with `wrap_prompt_text`, computed dynamic dialog heights, and clamped all button choices within inner bounds.
  - In `crates/bashmenu-tui/src/modals/message.rs`, guarded against clamp bounds panics and clamped the `[ OK ]` button.
  - In `crates/bashmenu-tui/src/modals/file_picker.rs`, clipped current directory banner path and bottom controls hints to inner columns.
  - In `crates/bashmenu-tui/src/modals/theme_picker.rs` and `stream_output.rs`, clipped row labels and footer status strings to inner bounds.
  - In `crates/bashmenu-tui/src/modals/grid_picker.rs` and `color_picker.rs`, added coordinate bounds checking to prevent 2D picker cells and swatch panels from drawing over dialog borders.
  - In `crates/menuedit-cli/src/item_edit.rs`, eliminated Rust format width traps in `FormRow::PreviewBox`, `FormRow::Input`, `FormRow::InputWithBrowse`, `FormRow::ExecMode`, `FormRow::Checkbox`, and `FormRow::Label` by explicitly clipping all text strings to available inner width.
  - In `crates/menuedit-cli/src/shortcut_editor.rs`, clamped action buttons, checkbox labels, and footer hints within inner boundaries.
  - In `crates/bashedit-cli/src/main.rs`, clipped swap recovery modal lines and prompt options within inner columns.
  - In `crates/bmpager-cli/src/pager.rs`, added UTF-8 safe character boundary truncation for link follow dialogs.
  - Added unit test `test_wrap_prompt_text_bounds_and_unicode` and integration test `test_all_modals_render_within_frame_bounds` in `crates/bashmenu-tui/tests/modals_test.rs` asserting zero right-border spillage across various viewports.
  - Verified 100% test pass rate across entire workspace with `cargo test --workspace`, zero warnings in `cargo clippy --workspace --all-targets -- -D warnings`, clean `cargo fmt --all --check`, and clean `shellcheck *.sh scripts/*.sh`.

- **Audit and Fix Modal Close Buttons and Help Gutters (Item #2)**:
  - Fixed dimension mismatches and `inner = if area.width <= dialog_w { area }` discrepancies across `crates/bashmenu-tui/src/modals/theme_picker.rs`, `help.rs`, `confirm.rs`, `toggle.rs`, `radio.rs`, `file_picker.rs`, `note_viewer.rs`, and `stream_output.rs` so `is_close_button_clicked` and footer coordinates match rendered frames on all terminal viewports.
  - Added click handling for footer help gutter action tokens (`[ESC/ENTER]: Close`, `[Ctrl+S] Save`, `[Ctrl+C] Copy`, `[Ctrl+V] Paste`, `[Esc] Cancel`, `[Enter] Select`) in `help.rs`, `note_editor.rs`, `theme_picker.rs`, `file_picker.rs`, `color_picker.rs`, `grid_picker.rs`, and `stream_output.rs`.
  - Added full mouse click support for `ItemTypePickerModal` in `crates/menuedit-cli/src/type_picker.rs` (handling `[X]`, list item clicks, and footer `[Enter] Select` / `[Esc] Cancel`).
  - Added full mouse click support in `crates/menuedit-cli/src/shortcut_editor.rs` for `KeyCaptureModal` (`[X]`, `[ENTER] Accept`, `[ESC] Cancel`), `LeaderSettingsModal` (`[X]`, buttons, checkboxes, inputs, footer), and `ShortcutEditModal` (`[X]`, buttons, checkboxes, inputs, footer).
  - Aligned modal dimensions in `crates/menuedit-cli/src/item_edit.rs` and `main.rs` (`AsciiGridModal` grid origin alignment).
  - Added mouse click support in `crates/bashedit-cli/src/main.rs` for `SwapRecovery` modal close button `[X]` and option actions.
  - Added comprehensive test suites `test_all_modals_close_button_and_footer_clickable` in `crates/bashmenu-tui/tests/modals_test.rs` and `test_menuedit_modals_close_button_and_footer_clickable` in `crates/menuedit-cli/tests/menuedit_test.rs`.
  - Rebuilt and reinstalled release binaries via `cargo build --release --workspace && ./install.sh`.
  - Pushed commit `74c4f62` and verified GitHub Actions CI run `42381487336` passed with `'success'`.

- **Eliminate Full Screen Transitions for Modals & Implement Modal Zoom Animations (Item #3)**:
  - Decoupled full screen transitions (`settings.transition`) from modal popups in `crates/bashmenu-cli/src/main.rs`: removed `modal_changed` from triggering `pending_transition`, and removed `draw_with_transition_out` from `MenuAction::ThemeSelector`. Full-screen transitions now apply strictly to screen navigation, submenu changes, paging, and external application launches/returns.
  - Implemented dedicated modal zoom animations in `crates/bashmenu-tui/src/terminal.rs`:
    - `animate_modal_grow`: interpolates expanding dialog frames outward from the initiating launch point (`(anchor_x, anchor_y)`) to the full modal rectangle over synchronized DEC Mode 2026 frames.
    - `animate_modal_shrink`: interpolates contracting dialog frames in reverse from the modal rectangle down to the launch anchor.
    - Anchor resolution: menu-activated modals anchor to the initiating menu item's row screen coordinates via `menu_view.selected_item_anchor(area)`; function key and leader shortcuts anchor symmetrically to the screen center.
  - Added `settings.modal_animation` (`zoom` [default], `none`) and `settings.modal_animation_speed` (`fast` [default], `normal`, `slow`, or ms) to `BashMenuConfig` schema in `crates/bashmenu-core/src/config.rs`, with seamless fallback to `transition_speed`.
  - Added interactive radio configuration options for modal animation effect and speed under "Transitions & Effects" in `bashmenu.mnu` and `tests/fixtures/bashmenu.mnu`.
  - Standardized `dialog_dimensions` across all modal types in `crates/bashmenu-tui/src/modals/`.
  - Added unit and integration tests in `crates/bashmenu-tui/tests/transition_test.rs` and `crates/bashmenu-cli/tests/audit_items_test.rs`.
  - Updated documentation in `bashmenu.md`, `bashmenu.1`, and `.agents/rules/invariants.md`.
  - Rebuilt release binaries and installed via `cargo build --release --workspace && ./install.sh`.

- **Multi-OS Rust Installation Prompt in `install.sh` (Item #4)**:
  - Investigated and resolved platform-specific Rust toolchain installation methods across Linux, macOS, FreeBSD, NetBSD, OpenBSD, and Windows POSIX environments (Git Bash / MSYS2 / MinGW).
  - Enhanced `install.sh` to detect missing `cargo` and interactively prompt the user (`[?] Would you like to install Rust now? [y/N]: `) when connected to an interactive terminal (`[ -t 0 ]`).
  - Added OS-aware installation logic:
    - On OpenBSD: uses native package manager `pkg_add rust` (with `doas` or root verification) since `rustup` does not support OpenBSD due to security architecture constraints (W^X and pledge/unveil).
    - On Linux, macOS, FreeBSD, NetBSD, and Windows POSIX shells: uses `curl` (with fallback to `wget`) piping to official `https://sh.rustup.rs` with automated `-s -- -y` flags.
  - Automatically sources `$HOME/.cargo/env` and exports `$HOME/.cargo/bin` to `PATH` in-process upon successful installation so `cargo build --release --workspace` can continue immediately without restarting the shell.
  - Verified non-interactive handling (e.g. CI / redirected input) exits cleanly with clear guidance.
  - Passed `shellcheck *.sh scripts/*.sh` with 0 warnings and `cargo test --workspace` with 100% pass rate.

- **Resolve Man Page Dangling Symlink and Missing Entry (Item #5)**:
  - Fixed `cp: not writing through dangling symlink` error in `install.sh` by explicitly removing any pre-existing dangling link or file at `$MAN_DIR/bashmenu.1` with `rm -f` prior to `cp -f`.
  - Added creation of the convenience man page alias `$MAN_DIR/bm.1 -> bashmenu.1` so `man bm` functions alongside `man bashmenu`.
  - Added cache refresh invocation for `mandb` when installed (`mandb -q -u`).
  - Added custom `MANPATH` verification warning in `install.sh` if a non-standard `MANPATH` environment variable is defined without `$HOME/.local/share/man`.
  - Updated `uninstall.sh` to remove both `bashmenu.1` and `bm.1` handling `[ -L ]` dangling symlinks properly.
  - Verified `shellcheck *.sh scripts/*.sh` passed with 0 warnings and `man -w bashmenu` / `man -w bm` resolve accurately.

- **Strict Separation Between Vanilla Default Template and User Config (Item #6)**:
  - Decoupled `menu` and `config` in `bashmenu-cli`, `menuedit-cli`, and `themeedit-cli` so running `bm` / `bashmenu` without `--menu` exclusively loads and saves to `~/.config/bashmenu/bashmenu.mnu` (auto-initializing from vanilla template if missing).
  - Ensured the repository's `bashmenu.mnu` is only loaded if specifically requested with `--menu <path>` or `--local`.
  - Removed legacy local file copy routines from `bashmenu.sh` that were polluting current working directories with template duplicates.
  - Ensured 100% template parity between repo root files (`bashmenu.mnu`, `bashmenu.yml`, `bashmenu.themes`) and test fixtures in `tests/fixtures/`.
  - Added `--update-menu` / `--reset-menu` flag to `install.sh` allowing users to upgrade an existing user menu to the latest vanilla layout with automatic `.bak` backup.
  - Updated `resolve_shortcuts_path` so user config directory takes strict precedence over `BASHMENU_DIR`.
  - Added integration test `test_user_config_isolation_and_vanilla_separation` in `crates/bashmenu-cli/tests/audit_items_test.rs`.

- **Netbook Theme Wallpaper Rendering Fixes (Quarterblock & Color Tiers)**:
  - Eliminated vertical grid lines ("venetian blinds") and font advance gaps: in `WallpaperCompositor::get_grid`, when a cell is uniform (such as `mask == 15` or `mask == 0` in quarterblock mode, or `r1 == r2 && g1 == g2 && b1 == b2` in halfblock mode), emits space `" "` with background fill `bg = avg_bg` instead of `"█"` or `"▀"`.
  - Added luminance variance contrast threshold (`lum_spread < 12.0`) to quarterblock binarization, preventing smooth continuous gradients (e.g., `alaska-mountain` sky) from devolving into pseudo-random dither maze patterns.
  - Implemented `supports_quadrant_blocks()` in `crates/bashmenu-tui/src/wallpaper.rs`: automatically detects Linux virtual consoles (`TERM=linux`, `console`, `dumb`) and non-UTF-8 environments lacking Unicode quadrant glyphs (`U+2596..U+259F`), cleanly falling back from `quarterblock` to CP437 `halfblock` mode (`"▀"` and `" "`).
  - Fixed `ColorTier::detect()` evaluation order: checks `TERM` (`linux`, `console`, `dumb`, `vt100`) before `COLORTERM` so inherited environment variables do not cause false `TrueColor` detection on basic consoles.
  - Added `downsample_color_to_tier()` and wired `color_tier` into `WallpaperCompositor`: downsamples cell colors (`fg`, `bg`, `avg_bg`) according to active `ColorTier` (`Color8`, `Color16`, `Color256`, `TrueColor`), eliminating saturated ANSI rainbow static across dark tones on 8/16/256-color displays.
  - Added integration tests in `crates/bashmenu-tui/tests/wallpaper_test.rs`: `test_wallpaper_compositor_console_fallback_to_halfblock`, `test_wallpaper_compositor_color_tier_downsampling`, and `test_color_tier_detect_priority`.

- **Nerd Font and Emoji Rendering Fix in Terminal Popup Modal (`glyphs.sh`)**:
  - Identified and fixed garbled UTF-8 character rendering in `crates/bashmenu-tui/src/ansi.rs`: `ansi_to_line` previously cast raw bytes with `bytes[idx] as char`, corrupting all multi-byte UTF-8 sequences (Nerd Font PUA glyphs and 4-byte emoji) into Latin-1 characters (`îª...` / `ð...`).
  - Rewrote ANSI parser in `ansi_to_line` using `text.chars().peekable()` to preserve full 32-bit Unicode scalar values, gracefully skip CSI/OSC escape sequences, and properly emulate carriage return line resets (`\r`).
  - Added tab sanitization (`line.replace('\t', "    ")`) in `StreamOutputModal` poll handler to prevent cell alignment artifacts.
  - Enhanced `StreamOutputModal` dialog title derivation in `crates/bashmenu-cli/src/main.rs`: falls back to cleaned menu label with bracketed commands stripped (e.g. `Search Nerd Font & Emoji`) instead of displaying the raw script path.
  - Extended `execution_mode` in `crates/bashmenu-core/src/menu.rs` to support `"popup"` and `"stream_output"`, and updated `is_streaming` to recognize `popup: true`.
  - Updated `scripts/glyphs.sh`: switched `NERD_URL` to secure `https://` upstream URL, added robust multi-OS UTF-8 locale detection, and added descriptive error messaging when neither `jq` nor `python3` is available.
  - Configured `Search Nerd Font & Emoji` in `bashmenu.mnu` and `tests/fixtures/bashmenu.mnu` with `prompt: 'Enter search term:'`, `stream: true`, and `window_percent: 75`.
  - Added unit tests in `crates/bashmenu-tui/src/ansi.rs` (`test_ansi_to_line_nerd_font_and_emoji`, `test_ansi_to_line_carriage_return_and_osc`), integration test in `crates/bashmenu-tui/tests/modals_test.rs` (`test_stream_output_modal_nerd_fonts_and_emoji`), and CLI test in `crates/bashmenu-cli/tests/audit_items_test.rs` (`test_search_nerd_font_and_emoji_menu_item_streaming`).
  - Verified 100% test pass rate across the workspace, zero warnings on Clippy and Rustfmt, zero warnings on Shellcheck, and deployed via `install.sh`.

- **Repo Root Config Deployment & Vanilla Menu Fixture Synchronization (`install.sh`)**:
  - Fixed `install.sh` config deployment source: changed `fixture="$SCRIPT_DIR/tests/fixtures/$cfg"` to prioritize the repository root files (`$SCRIPT_DIR/$cfg`) with fallback to `tests/fixtures/$cfg` only if missing.
  - Synchronized `tests/fixtures/bashmenu.mnu` with the clean repository root vanilla menu layout (`bashmenu.mnu`), eliminating stale developer shortcuts and custom entries.
  - Updated unit tests in `crates/bashmenu-core/src/menu.rs` and `crates/bashmenu-core/tests/schema_fixtures_test.rs` to validate the updated vanilla menu structure (`Notes` notebook item and `Applications` submenu).
  - Verified 100% test pass rate across the workspace, zero Clippy/Rustfmt warnings, zero Shellcheck warnings.

- **Fade Transition Color Flashing Fix on Non-TrueColor Terminals**:
  - Identified root cause of erratic color flashing and chromatic hue jumping during `Fade` transitions on netbook/basic terminals: `dim_color` in `crates/bashmenu-tui/src/transition.rs` treated `ColorTier::Color256` the same as `TrueColor`, emitting raw `Color::Rgb(dr, dg, db)` escape codes (`\x1b[38;2;...m`) that non-TrueColor terminals misparse as ANSI SGR color change commands.
  - Updated `dim_color` for `ColorTier::Color256` to downsample dimmed RGB values via `crate::wallpaper::rgb_to_xterm256(dr, dg, db)`, returning proper `Color::Indexed` codes.
  - Implemented spatial Bayer ordered dithering (`BAYER_4X4`) for `Fade` in `crates/bashmenu-tui/src/terminal.rs` (`draw_with_transition` and `draw_with_transition_out`) when running on `ColorTier::Color8` or `ColorTier::Color16`: replaces mathematical RGB dimming (which jumps through saturated primary hues due to lack of dark luminance ramps) with smooth ordered dithering to black.
  - Added unit test assertions in `crates/bashmenu-tui/src/transition.rs` (`test_dim_color`) and integration test assertions in `crates/bashmenu-tui/tests/transition_test.rs` (`test_transition_dim_color_levels`) verifying `Color256` outputs `Color::Indexed` and 8/16-color tiers never emit `Color::Rgb`.
  - Verified 100% test pass rate across the workspace, zero warnings on Clippy and Rustfmt, zero warnings on Shellcheck.

## Phase 19: Customizable Help Gutter Hover Delay, Decoupled Status Border, and Unbounded Expansion
- [x] Added `help_gutter_delay` configuration to `Settings` in `crates/bashmenu-core/src/config.rs` (with default 4.0s, helper method `help_gutter_delay()`, and `set_value` support for float and string representations).
- [x] Added `Help Gutter Hover Delay [[{app.settings.help_gutter_delay}] seconds]` setting under `Bashmenu Settings` in `bashmenu.mnu` and `tests/fixtures/bashmenu.mnu`.
- [x] Updated `HelpGutterHover` in `crates/bashmenu-tui/src/gutter.rs` with `set_duration()`, `duration()`, and instantaneous expansion support when duration is `0.0`.
- [x] Decoupled status gutter border from help gutter expansion in `crates/bashmenu-tui/src/menu_view.rs`: when the help gutter expands upward, the status gutter top border on row `normal_gutter_border_y` (from `sep_x` to right edge) and status content remain strictly anchored in place and are completely unaffected.
- [x] Implemented dynamic unbounded row expansion in `MenuView`: the help gutter expands upward to as many rows as needed to display all help items and shortcuts without truncation, covering background plugins on the left side of `sep_x` while cleanly stopping below visible menu items without interfering with menu navigation or click hits.
- [x] Updated `menuedit-cli`, `themeedit-cli`, and `shortcut_editor` to utilize `help_gutter_delay` from config.
- [x] Documented `settings.help_gutter_delay` and the decoupled expansion behavior in `bashmenu.md` and `bashmenu.1`.
- [x] Added tests in `crates/bashmenu-core/src/config.rs` (`test_help_gutter_delay_config_and_set_value`) and `crates/bashmenu-tui/tests/menu_navigation_test.rs` (`test_gutter_hover_zero_delay_instant_expansion`, `test_help_gutter_expansion_decouples_status_border_and_covers_plugins`).
- [x] Verified all tests pass, clippy clean, rustfmt clean, shellcheck clean, and verified remote GitHub Actions CI run `42416115070`.

## Phase 20: Theme Editor Right Click Color Dropper Tool
- [x] Added `extract_preview_cell_color` in `crates/themeedit-cli/src/preview.rs` to extract the effective Ratatui `Color` of the clicked preview cell (handling background, character glyphs, and wallpaper half-blocks).
- [x] Added `current_selected_token` in `crates/themeedit-cli/src/lib.rs` to inspect the active token in the collapsible sidebar tree.
- [x] Implemented right-click mouse handler (`MouseEventKind::Down(MouseButton::Right)`) in `crates/themeedit-cli/src/main.rs`: clicking anywhere in the preview pane samples the cell color, copies the formatted color code to the system clipboard via `bashmenu_tui::set_clipboard_text`, and if a color token is currently selected, assigns the sampled color via `state.set_color_token_value`.
- [x] Added visual feedback: 200ms reverse flash on clicked preview cell and informational toast notification displaying sampled color and target token (or informative toast if selected item is not a color setting).
- [x] Added `[R-Click] Dropper` to bottom footer legend in `themeedit`.
- [x] Updated man page `bashmenu.1` and `bashmenu.md` documentation.
- [x] Added unit and integration tests in `crates/themeedit-cli/tests/themeedit_test.rs` (`test_preview_color_dropper_sampling_and_assignment`).
- [x] Verified all workspace tests pass, clippy clean, rustfmt clean, and verified remote GitHub Actions CI run `42417064867`.

## Phase 21: Theme Editor Default Directory for Wallpaper Images
- [x] Implemented `default_wallpaper_dir()` in `crates/themeedit-cli/src/lib.rs` prioritizing the repository's `assets/wallpapers` directory via multiple discovery strategies (`$BASHMENU_DIR`, ancestor traversal from CWD and executable, compile-time `$CARGO_MANIFEST_DIR`, and standard user config / system share locations).
- [x] Implemented `relativize_wallpaper_path()` in `crates/themeedit-cli/src/lib.rs`: when picking files inside `assets/wallpapers`, automatically formats the stored theme image path as `assets/wallpapers/<filename>` regardless of the user's working directory.
- [x] Updated `BackgroundConfigAction::Browse` and `ActiveModal::BackgroundBrowse` in `crates/themeedit-cli/src/main.rs` to start in `default_wallpaper_dir()` (or the parent of the currently configured wallpaper if valid), and format selected images cleanly.
- [x] Added unit and integration tests in `crates/themeedit-cli/tests/themeedit_test.rs` (`test_default_wallpaper_dir_and_path_relativization`) verifying wallpaper directory resolution, file existence, path relativization, and `$BASHMENU_DIR` override.
- [x] Verified all tests pass, clippy clean, and rustfmt clean.

## Phase 22: Wallpaper Modal Setting Autofocus, Preview Panel Uniform Text Color, and Accent Highlight Correction
- [x] Added `with_focus(BackgroundFocus)` builder and `focus(&self)` getter to `BackgroundConfigModal` in `crates/bashmenu-tui/src/modals/background_config.rs`.
- [x] Updated `open_selected_token_picker` and `KeyCode::F(4) | KeyCode::Char('b')` in `crates/themeedit-cli/src/main.rs` to inspect the focused wallpaper token (`WallpaperImage`, `WallpaperOpacity`, `WallpaperScaling`, `WallpaperMode`, `WallpaperClustering`, `WallpaperQuantization`) and autofocus directly on that field when the modal opens.
- [x] Removed artificial `"1. "`, `"2. "`, `"3. "`, `"4. "` number prefixes styled in `shortcut_key` from menu items in `crates/themeedit-cli/src/preview.rs`: menu item text is now rendered in uniform `preview_styles.text` (and active item in `preview_styles.highlight`), eliminating two-tone split text.
- [x] Fixed `accent` color preview flash in `crates/themeedit-cli/src/lib.rs` (`get_token_preview_spans`): removed erroneous flashing of the plugins area; now precisely highlights the submenu chevron indicator glyph (`»`) on inactive submenu items.
- [x] Fixed mock plugins row in `crates/themeedit-cli/src/preview.rs` to use `preview_styles.plugin` instead of `preview_styles.accent`.
- [x] Added unit and integration tests in `crates/themeedit-cli/tests/themeedit_test.rs`: `test_wallpaper_modal_autofocus_on_selected_setting`, `test_preview_menu_items_uniform_text_color`, and updated `test_themeedit_token_preview_spans_and_flash`.
- [x] Synchronized `tests/fixtures/bashmenu.mnu` with the latest screensaver configuration options in `bashmenu.mnu`.
- [x] Verified all tests pass, clippy clean, and rustfmt clean.






















## Phase 23: Inactivity Timeout Timer Reset Upon Return from Programs and Sub-Tools
- [x] Reset `last_interaction = Instant::now()` immediately after returning from all external commands and console applications (`ExecutionMode::Console`, e.g. `mc`, shell scripts, commands).
- [x] Reset `last_interaction = Instant::now()` immediately after returning from internal sub-tools and editors:
  - `MenuAction::LaunchEditor` and in-menu editor launches (`bashedit`)
  - `MenuAction::LaunchThemeEditor` and in-menu theme editor launches (`themeedit`)
  - `MenuAction::EditMenu` and in-menu menu editor launches (`menuedit`)
  - `MenuAction::LaunchShortcutEditor` (`shortcutedit` / `menuedit --shortcuts`)
  - Note external editor launches in `NoteViewerModal` / `NoteConflictModal`
  - Documentation pager launches (`bmpager`)
  - Built-in inactivity screensaver command completion itself (`inactivity_timeout.command`)
- [x] Reset `last_interaction = Instant::now()` on `MenuAction::Reload`, `StreamOutputModal` closing, and `Event::FocusGained` events.
- [x] Added contract test in `crates/bashmenu-cli/tests/audit_items_test.rs` (`test_inactivity_timeout_timer_reset_contract`) asserting that 5+ minute simulated child execution durations never trigger the screensaver immediately upon return.
- [x] Verified 100% test pass rate across the workspace, zero Clippy warnings, and zero Rustfmt warnings.

## Phase 24: Menu Editor Item Type Modal Scrolling, Horizontal Text Scrolling & Mouse Selection
- [x] Commit `1801dcd`: Fixed Menu Editor item type picker modal scrolling/visibility and enabled horizontal text scrolling & mouse selection across all text input boxes.
- [x] Item Type Picker Modal (`crates/menuedit-cli/src/type_picker.rs`):
  - Corrected modal height sizing dynamically (`(ITEM_TYPES.len() + 4).min(area.height - 2)`) so all 16 item types (including `{divider}`) fit comfortably without truncation on standard terminals.
  - Added smooth vertical scrolling via `Up`/`Down`, `j`/`k`, `PageUp`/`PageDown`, `Home`/`End`, and mouse scroll wheel up/down (`ScrollUp`/`ScrollDown`).
  - Added `▲` and `▼` scroll indicator rendering when item list overflows the viewport.
  - Implemented direct mouse click selection on any visible row to select and confirm the item type immediately.
- [x] Horizontal Text Scrolling (`crates/bashmenu-tui/src/text_input.rs`, `crates/menuedit-cli/src/item_edit.rs`, `crates/menuedit-cli/src/shortcut_editor.rs`, `crates/themeedit-cli/src/main.rs`, `crates/bashmenu-tui/src/modals/background_config.rs`):
  - Added `scroll_offset` tracking to `TextInput` along with `ensure_cursor_visible(w)` and `scroll_for_width(w)`.
  - Replaced hardcoded `.take(field_w)` truncation with windowed character slicing `chars.iter().skip(scroll_offset).take(field_w)`.
  - Allowed Left/Right arrows and Home/End keys to reach both ends of long text strings without clipping or losing cursor visibility.
  - Maintained inverted selection highlighting across scrolled text.
- [x] Comprehensive Mouse Selection in Menu Editor Form (`crates/menuedit-cli/src/item_edit.rs`, `crates/menuedit-cli/src/shortcut_editor.rs`):
  - Left-clicking on any form text input now focuses the control and positions the cursor at the clicked character column taking `scroll_offset` into account.
  - Added mouse drag selection and Shift+LeftClick selection across inputs.
  - Left-clicking on any checkbox (`AltBufferChk`, `QuietChk`, `NoFormattingChk`, `MaskedChk`, `RefreshChk`, `ShowWhitespaceChk`, `ExternalChk`) directly toggles its checked state.
  - Left-clicking on `[Change Type (CTRL+T)]` opens the item type picker modal.
  - Left-clicking on `[Browse]` buttons triggers filesystem file/directory browsing.
  - Left-clicking on Execution Mode radio selector cycles streaming/interactive/console modes.
  - Multi-line radio options input supports clicking to set line row and column position.
- [x] Integration & Unit Tests:
  - Added `test_item_type_picker_modal_scrolling_and_all_items` in `crates/menuedit-cli/tests/menuedit_test.rs` testing all 16 types, `{divider}`, navigation keys, mouse wheel, and row clicking.
  - Added `test_item_edit_modal_horizontal_scrolling_and_mouse_interactions` in `crates/menuedit-cli/tests/menuedit_test.rs` testing long text scrolling, mouse focus, cursor placement, checkbox toggles, and button clicks.
- [x] Remote CI Run: `42433248218` passed completely with zero warnings or failures.

## Phase 25: Theme Editor Highlighter Setting Highlight Background Bar
- [x] Commit `16e9338`: Fixed Theme Editor highlighter configuring the highlight color as the text foreground instead of the margin-to-margin highlight bar background.
- [x] Highlight Palette Token Invariant (`crates/themeedit-cli/src/lib.rs`):
  - Updated `set_color_token_value()` for `"highlight"` to store the chosen color into index 1 (`bg`) of the sequence `[fg, bg]`, instead of index 0 (`fg`).
  - Added automatic high-contrast foreground calculation (`fg`) for the highlight text (preserving existing theme `fg` if contrasting, or using dark text for light bars / light text for dark bars) so menu item text remains clearly readable.
  - Synchronized `colors_16` and `colors_8` tiers with appropriate `COLOR_BLACK` or `COLOR_WHITE` high-contrast foregrounds and downsampled backgrounds.
  - Kept `"selection"` in sync with `"highlight"` whenever present in the theme palette.
- [x] Token Inspection & Color Retrieval (`crates/themeedit-cli/src/lib.rs`):
  - Updated `get_token_value_display()` to inspect index 1 (`bg`) of the `"highlight"` sequence so the tree property view displays the background bar color rather than the text color.
  - Updated `get_color_token_color()` and `get_color_token_u8()` for `"highlight"` to return the background bar color (index 1), ensuring the color picker and dropper initialize with and reference the highlight bar background.
- [x] Margin-to-Margin Highlight Rendering (`crates/themeedit-cli/src/preview.rs`):
  - Added full cell background fill (`buf[(x, content_start_y)].bg = bg`) across `p_inner_x..(p_inner_x + p_avail_w)` in `PreviewMode::Menu` matching `menu_view.rs`.
  - Added full tree cell background fill across the pane width in `PreviewMode::MenuEditor`.
- [x] Unit & Integration Tests (`crates/themeedit-cli/tests/themeedit_test.rs`):
  - Updated `test_create_theme_with_base_monochrome` to assert the monochrome highlight bar background (`255`).
  - Added `test_themeedit_highlighter_sets_margin_to_margin_background_bar` verifying initial background retrieval, setting highlight color to index 201, verifying contrasting foreground, and verifying that every cell in the preview pane across `p_avail_w` has `bg == Color::Indexed(201)`.
- [x] Remote CI Run: `42435677068` passed completely with zero warnings or failures.

## Phase 26: Highlight Text Color Theme Token in Theme Editor
- [x] Commit `915596f`: Added new Color Theme Token (`highlight_text`) in Theme Editor to customize text color on the selection highlight bar.
- [x] Theme Palette Invariant & Style Resolution (`crates/bashmenu-tui/src/style.rs`):
  - Updated `ResolvedThemeStyles::from_theme` to parse `p.get("highlight_text")` across color tiers, overriding `resolved.highlight.fg` while preserving the highlight bar background.
- [x] Theme Editor Token Tree & Palette Sync (`crates/themeedit-cli/src/lib.rs`):
  - Added `TokenItem { label: "Highlight Text Color", kind: TokenKind::Color("highlight_text") }` under `TokenCategory::Colors`.
  - Implemented `get_token_value_display()` for `"highlight_text"` to display the text foreground color from `highlight_text` or index 0 of `highlight`.
  - Implemented `get_color_token_color()` and `get_color_token_u8()` for `"highlight_text"` returning the text foreground color.
  - Implemented `set_color_token_value()` for `"highlight_text"` updating index 0 of the `highlight` sequence and `highlight_text` across `colors_256`, `colors_16`, and `colors_8` tiers, preserving the bar background and syncing `"selection"`.
  - Added `"highlight_text"` to `select_token_by_key()` with aliases `highlight_fg` and `highlight_foreground`.
  - Added preview spans in `get_token_preview_spans()` for `highlight_text` flashing `content_start_y, p_inner_x + 2..p_inner_x + 24`.
- [x] Interactive UI & Hit Testing (`crates/themeedit-cli/src/main.rs`):
  - Updated `open_selected_token_picker()` to display `"Pick Highlight Text Color"` for `highlight_text` and `"Pick Highlight Bar Color"` for `highlight`.
  - Updated `hit_test_preview()` on active menu row `content_start_y`: clicking the text portion (`p_inner_x + 2..p_inner_x + 24`) selects `"highlight_text"`, while clicking padding/bar selects `"highlight"`.
- [x] Unit & Integration Tests (`crates/themeedit-cli/tests/themeedit_test.rs`):
  - Added `test_themeedit_highlight_text_token_configuration` verifying token existence, initial foreground retrieval, alias selection, color update to index 46, YAML palette sequence update (`[46, 201]`), style resolution (`fg == 46`, `bg == 201`), and preview flashing spans.
- [x] Remote CI Run: `42436916241` passed completely with zero warnings or failures.

## Phase 27: Missing Supported Color Theme Tokens (Title, Window Close Button, Footer)
- [x] Commit `17047b6`: Added all remaining supported Color Theme Tokens (`title`, `window_close_button`, and `footer`) to the Theme Editor token tree and UI.
- [x] Token Tree Registration & Palette Support (`crates/themeedit-cli/src/lib.rs`):
  - Added `Title Color` (`title`), `Footer Color` (`footer`), and `Window Close Button` (`window_close_button`) tokens under `TokenCategory::Colors`.
  - Updated `create_default_theme()` to include initial color pairs for `title`, `footer`, and `window_close_button`.
  - Updated `get_color_token_color()` and `get_color_token_u8()` to resolve `title` and `footer` via active theme styles and palette mappings.
  - Updated `get_token_value_display()` to fall back to resolved active theme styles for all color tokens when not explicitly defined in 256-color map.
  - Added aliases in `select_token_by_key()`: `header` -> `title`, `bottom_bar` -> `footer`, and `close_button`/`close_btn`/`close` -> `window_close_button`.
  - Added preview flash spans in `get_token_preview_spans()` for `footer` (bottom border row) and `window_close_button` (top-right `[X]` button).
- [x] Interactive Preview & Modal Picker (`crates/themeedit-cli/src/main.rs`):
  - Configured custom picker modal titles in `open_selected_token_picker()`: `"Pick Title Color"`, `"Pick Footer Color"`, and `"Pick Window Close Button Color"`.
  - Updated `hit_test_preview()`:
    - In `PreviewMode::Menu`, clicking `[X]` at top-right selects `window_close_button`.
    - In `PreviewMode::Modals`, clicking dialog title selects `title` and clicking `[X]` selects `window_close_button`.
    - In `PreviewMode::MenuEditor`, clicking footer row selects `footer`.
- [x] Unit & Integration Tests (`crates/themeedit-cli/tests/themeedit_test.rs`):
  - Added `test_themeedit_title_window_close_button_footer_tokens` verifying token existence, alias selection, color retrieval, value updates in `colors_256`, preview flash spans, and default theme generation.
- [x] Remote CI Run: `42437847593` passed completely with zero warnings or failures.

## Phase 28: Learning Proposal & Invariants Codification (Phases 23-27)
- [x] Commit `81d3725`: Codified permanent developer rules and skills from completed Phases 23–27 into `.agents/rules/invariants.md` and `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Invariants Codified:
  - **Theme Selection Highlight Bar & Highlight Text Invariant (`highlight`, `highlight_text`):** Documented index 0 (`fg`) vs index 1 (`bg`) contract, margin-to-margin rendering across `p_avail_w`, contrasting text calculation, and 3-tier synchronization.
  - **Complete Theme Token Parity & Hit-Testing Invariant:** Mandated 1:1 parity between `ResolvedThemeStyles` and `ThemeEditorState::tokens` (`title`, `window_close_button`, `footer`, `highlight`, `highlight_text`, etc.), requiring matching preview spans, hit-test coordinates, and modal picker titles.
  - **Inactivity Timeout Reset Contract on External Program Return:** Mandated immediate `last_activity = Instant::now()` reset on return from all external commands, child subshells, editors, and companion CLI tools.
  - **Form Navigation, Text Input Horizontal Scrolling & Modal Viewports Invariant:** Mandated dynamic height calculation, screen height clamping, vertical scrolling with indicators (`▲`/`▼`) for option modals, horizontal `scroll_offset` tracking for `TextInput`, and mouse selection/drag/checkbox toggling.
  - **Contextual Modal Autofocus & Asset Directory Invariant:** Mandated autofocus on invoking setting when opening dialogs, and asset path discovery prioritizing repo wallpapers before system directories.
- [x] Developer Skill Updated:
  - Cross-referenced theme token parity, inactivity reset contracts, and horizontal text scrolling/modal viewport rules in `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Verification & CI:
  - Passed `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` with zero warnings or errors.
  - Remote GitHub Actions CI run `42441058572` passed completely with `'success'`.

## Phase 29: Input Cursor Text Color Parity, Faux Plugin Uniform Coloring & Theme Editor Shortcut Keys
- [x] Input Cursor Text Color Parity Across All Programs:
  - `crates/bashedit-cli/src/main.rs`: Derived cursor block color directly from `styles.text.fg` on editor canvas and trailing line ends rather than hardcoded `Color::White`, maintaining full parity with the edited text color.
  - `crates/bashmenu-tui/src/modals/note_editor.rs`: Derived cursor style and end-of-chunk space cursor directly from active `line_style.fg` and `styles.background` instead of `styles.highlight`.
  - `crates/menuedit-cli/src/item_edit.rs`: Replaced hardcoded black-on-white inverted styles in `FormRow::Input`, `FormRow::MultiLineInputRow`, and `FormRow::InputWithBrowse` with `box_style.add_modifier(Modifier::REVERSED)` and `inp_style.add_modifier(Modifier::REVERSED)`.
  - `crates/menuedit-cli/src/shortcut_editor.rs`: Guaranteed cursor cell reverses `styles.text` with editor background without inheriting underline modifier.
  - `crates/themeedit-cli/src/main.rs`: Unified trailing space cursor with `box_style.add_modifier(Modifier::REVERSED)`.
- [x] Theme Editor Faux Plugin Uniform Styling (`crates/themeedit-cli/src/preview.rs`):
  - Fixed faux plugin rows displaying in two colors by changing `Alienware`, `Mem`, and `Pi-3B` span styling from `preview_styles.divider` to `preview_styles.plugin`.
- [x] Theme Editor Preview Shortcut Keys (`crates/themeedit-cli/src/preview.rs`, `crates/themeedit-cli/src/lib.rs`, `crates/themeedit-cli/src/main.rs`):
  - Rendered shortcut indicators (`1. `, `2. `, `3. `, `4. `) on active and inactive menu preview rows using `preview_styles.shortcut_key` and active highlight style.
  - Updated `get_token_preview_spans` for `shortcut_key` token to flash the 4 shortcut key spans on the menu items.
  - Updated `hit_test_preview` to detect clicks on shortcut key columns (`p_inner_x + 6 .. p_inner_x + 9`), directly selecting `shortcut_key`.
- [x] Unit & Integration Tests:
  - Added `test_preview_faux_plugin_uniform_color_and_shortcuts` to `crates/themeedit-cli/tests/themeedit_test.rs` validating uniform plugin text foregrounds, preview shortcut key rendering, and token preview spans.
  - Updated `test_preview_menu_items_uniform_text_color` to account for shortcut key positioning and verify shortcut key foreground styling.
  - Workspace tests passing (`cargo test --workspace`), clippy clean (`-D warnings`), and fmt clean (`cargo fmt --all --check`).

## Phase 30: Menuedit Bottom-Right Corner Footer "xi" Border Overflow Fix
- [x] Root Cause Identified:
  - In `crates/bashmenu-tui/src/gutter.rs`, `spread_gutter_line_layout` and `wrap_spread_help_gutter_layout` previously called `visible_text_len(item)` from `crate::bbcode`.
  - In `crates/bashmenu-tui/src/bbcode.rs`, `visible_text_len` invokes `strip_bbcode(text).width()`. BBCode formatting recognizes `[s]` as a tag for strikethrough text and strips it.
  - When `menuedit` rendered its second footer row containing `[s] Save`, `visible_text_len("[s] Save")` stripped `[s]`, returning 5 instead of 8.
  - This 3-character undercount caused `min_line_len` to be 3 columns too short, allocating 3 extra space characters to line gaps and making the formatted line 3 columns longer than `avail_w`.
  - Furthermore, `ranges` calculated column positions using the undercounted width, shifting all subsequent tokens (`[ESC/q] Exit`) 3 columns to the left in `ranges`.
  - As a result, when `line_str` was printed to the buffer, `[ESC/q] Exit` extended past the inner boundary and overwrote the right window border with `xi`. When individual token highlights were printed at the shifted `ranges` position, the un-overwritten tail of `[ESC/q] Exit` from `line_str` remained visible as `Exitxi`.
- [x] Core Fix Implemented:
  - Created `gutter_text_width(s: &str) -> usize` using `unicode_width::UnicodeWidthStr::width` and `truncate_gutter_text(text: &str, max_w: usize) -> String` using character widths in `crates/bashmenu-tui/src/gutter.rs`.
  - Replaced all calls to `visible_text_len` and `truncate_to_width` across `gutter.rs` with `gutter_text_width` and `truncate_gutter_text`.
  - In `spread_gutter_line_layout`, clamped token ranges to `max_w` and skipped ranges beyond `max_w` when line length exceeds bounds.
  - Added defense-in-depth clamping in `crates/menuedit-cli/src/main.rs`: `line_str` is clamped to `avail_w`, and token labels are truncated if they extend beyond `inner_x + avail_w`, guaranteeing footer text can never overwrite the right window border.
- [x] Tests & Verification:
  - Added `test_wrap_spread_help_gutter_bbcode_conflict_brackets` in `crates/bashmenu-tui/src/gutter.rs` ensuring shortcut brackets like `[s] Save`, `[b] Back`, `[i] Info`, `[u] Undo` are never stripped or miscalculated.
  - Added `test_menuedit_footer_border_overflow_repro` in `crates/menuedit-cli/tests/menuedit_test.rs` checking all window widths from 40 to 200, explicitly asserting that right window border `│` remains intact and `"Exitxi"` is never present on any row.
  - Updated assertions in `test_menuedit_footer_spread_and_position` and `test_editor_legend_row_spread` to use `gutter_text_width`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings or errors.

## Phase 31: Theme Editor Indicator Configuration Token under Theme Category
- [x] Theme Indicator Token Relocation and Configuration:
  - Relocated `Indicator` token from below the color tokens into the global `Theme:` category in `crates/themeedit-cli/src/lib.rs`.
  - Moved `Background Color` from `Theme:` category into `Colors:` alongside `Title Color`.
  - Extended `select_token_by_key` to match `"indicator"`, `"selection_indicator"`, and `"cursor_indicator"`.
  - Implemented `ThemeEditorState::set_indicator(&mut self, indicator: String)` to configure `current_theme.indicator`, mark `is_modified = true`, and display a status message.
  - Enhanced `get_token_value_display` for `TokenKind::Indicator` to resolve glyphs with `bashmenu_core::resolve_glyph(raw, true)`, presenting both resolved glyphs and raw macros (e.g., `'≡' ({ascii:240})` or `'' ({nf:@::})`).
  - Added interactive `InputModal` for indicator customization in `crates/themeedit-cli/src/main.rs` (`ActiveModal::IndicatorInput`), accepting raw strings, Unicode characters, ASCII macros (`{ascii:NNN}`), and Nerd Font macros (`{nf:...}`).
  - Added `KeyCode::Delete` handling on `TokenKind::Indicator` to reset the indicator to default `"> "`.
  - Updated `get_token_preview_spans` for `TokenKind::Indicator` using `preview_styles.indicator_symbol.width().max(2) as u16` to accurately match indicator length.
- [x] Unit & Integration Tests:
  - Added `test_theme_indicator_token_configuration_and_persistence` in `crates/themeedit-cli/tests/themeedit_test.rs` validating category placement under `Theme:`, keyboard selection, Unicode character configuration, CP437 ASCII macro expansion, Nerd Font macro handling, save/load roundtripping to `.themes` file, and preview span dimensions.
  - Verified zero formatting issues (`cargo fmt --all --check`), zero Clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`), and 100% test pass across workspace (`cargo test --workspace`).

## Phase 32: Fix Screensaver Delay and Program Config Persistence in bashmenu.yml
- [x] Fixed issue where updating screensaver options (`[Edit Screensaver Delay]` and `[Edit Screensaver Program]`) in `Main Menu -> Options & Settings -> Bashmenu Settings` appended flattened strings to the end of `bashmenu.yml` instead of updating `settings.inactivity_timeout.milliseconds` and `settings.inactivity_timeout.command`.
- [x] Core Config Fix (`crates/bashmenu-core/src/config.rs`):
  - Updated `BashMenuConfig::set_value` to support `settings.inactivity_timeout.milliseconds` (and `app.settings.inactivity_timeout.milliseconds`) and `settings.inactivity_timeout.command` (and `app.settings.inactivity_timeout.command`), updating `self.settings.inactivity_timeout` directly in-place.
  - Added cleanup logic in `set_value` and `sanitize()` that clears any accidental/legacy flattened keys (`inactivity_timeout.milliseconds`, `inactivity_timeout.command`) from `self.settings.extra` and `self.extra`.
  - Added `BashMenuConfig::sanitize()` called automatically in `load_config()` to transparently migrate any existing flattened screensaver keys into `settings.inactivity_timeout`.
- [x] Menu Schema & Prompt Enhancements (`bashmenu.mnu`, `tests/fixtures/bashmenu.mnu`):
  - Added explicit `prompt: Screensaver command to execute on idle timeout` to `Edit Screensaver Program [{app.settings.inactivity_timeout.command}]` for modal display parity.
- [x] Tests & Verification:
  - Added `test_inactivity_timeout_set_value_and_sanitize` in `crates/bashmenu-core/src/config.rs` validating in-memory mutation, dot-path resolution via `get_value`, clean YAML serialization under `settings.inactivity_timeout:`, and migration of legacy flattened keys.
  - Validated schemas with `ymlcheck` on `bashmenu.yml`, `bashmenu.mnu`, and `bashmenu.themes`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (all passing).

## Phase 33: Codify Configuration Structural Integrity Invariant & Recursive Dotted Path Decomposition
- [x] Codified the **Configuration Structural Integrity Invariant (`bashmenu.yml`, `BashMenuConfig::set_value`)** in `.agents/rules/invariants.md` and `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Core Config Recursive Map Decomposition (`crates/bashmenu-core/src/config.rs`):
  - Implemented `insert_nested_yaml_value(map, path, value)` to decompose arbitrary dot-delimited configuration paths (e.g. `user.profile.contact.email`, `settings.custom.sub_option`) into strongly structured, nested `serde_yml::Mapping` objects.
  - Updated `BashMenuConfig::set_value` to use `insert_nested_yaml_value` across `settings.extra`, `user`, and `extra` maps, preventing any literal dotted string keys from ever being written flat into configuration YAML.
  - Enhanced `BashMenuConfig::sanitize()` to detect and automatically migrate any legacy dotted flat keys in `settings.extra` and `extra` into proper nested YAML mappings.
- [x] Tests & Verification:
  - Added unit test `test_arbitrary_nested_dot_paths_never_flatten` in `crates/bashmenu-core/src/config.rs` asserting nested dictionary creation, dot-path resolution via `get_value`, and clean, non-flat YAML serialization across multiple nesting tiers.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (all passing).

## Phase 34: Terminal Echo & Raw Mode Clean Restoration on Exit
- [x] Fixed issue where exiting `bashmenu` occasionally returned to the terminal with character echo disabled (`stty -echo`) where typed characters were invisible and the cursor did not advance until pressing `Enter`.
- [x] Terminal Guard & Canonical Cooked State Restoration (`crates/bashmenu-tui/src/terminal.rs`, `crates/bashmenu-tui/src/lib.rs`):
  - Implemented `ensure_terminal_sane()` using `libc::tcgetattr` and `libc::tcsetattr` with `libc::TCSADRAIN` across `STDIN_FILENO`, `STDOUT_FILENO`, `STDERR_FILENO`, and `/dev/tty` to unconditionally enforce `ECHO | ICANON | ISIG | ECHOE | ECHOK`, `ICRNL`, and `OPOST | ONLCR` whenever leaving raw mode back to the host shell.
  - Updated `TerminalGuard::drop()` to flush output, disable raw mode, and call `ensure_terminal_sane()` when returning to the host terminal.
  - Updated `TerminalGuard::suspend` and `TerminalGuard::suspend_keep_alt` to sanitize terminal attributes before calling `enable_raw_mode()`, preventing child commands (or crashed/interrupted subprocesses) that left raw mode enabled from corrupting crossterm's baseline snapshot.
  - Updated `install_panic_hook()` to call `ensure_terminal_sane()` ensuring terminal echo and canonical buffering are restored even if an uncaught panic occurs.
- [x] Child Tool & Background Process Hygiene:
  - Fixed `crates/bashedit-cli/src/main.rs`: Replaced 4 calls to `std::process::exit(0)` with clean event loop breaks (`tabs.clear()`), ensuring `TerminalGuard`'s `Drop` implementation executes properly and restores terminal state.
  - Fixed `crates/bmpager-cli/src/pager.rs`: Wrapped pager event loop in RAII `PagerTerminalGuard` with `Drop` calling `ensure_terminal_sane()`, and sanitized termios around external editor / child viewer suspensions.
  - Fixed `crates/bashmenu-core/src/plugin.rs` and `crates/bashmenu-core/src/macro_engine.rs`: Explicitly isolated child subprocess `stdin` via `.stdin(Stdio::null())` so background plugin fetches and `{command:...}` macro probes cannot inherit or interfere with controlling terminal state.
  - Fixed `crates/bashmenu-cli/src/main.rs`: Handled `Ctrl+C` (`KeyCode::Char('c')` + `KeyModifiers::CONTROL`) in the main menu to break cleanly via `MenuAction::Exit`.
- [x] Tests & Verification:
  - Added unit test `test_ensure_terminal_sane_callable` in `crates/bashmenu-tui/src/terminal.rs`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `shellcheck *.sh scripts/*.sh`.

## Phase 35: Theme Editor Root Frame Background Color Parity
- [x] Fixed issue where `themeedit` root window frame and buffer clear initialized with default/reset background color rather than the background color of the currently set theme.
- [x] Theme Editor Rendering & App Path Resolution (`crates/themeedit-cli/src/lib.rs`, `crates/themeedit-cli/src/main.rs`):
  - Implemented `render_root_frame(area, buf, theme_name, is_modified, styles)` in `crates/themeedit-cli/src/lib.rs` that explicitly sets `buf[(x, y)].set_bg(styles.background)` across all cells in `area` and renders the outer `Block` with `.style(Style::default().bg(styles.background))`.
  - Updated `main.rs` to call `render_root_frame()` and resolve configuration via `bashmenu_core::resolve_app_path(Path::new("bashmenu.yml"))`.
  - Updated `ThemeEditorState::new` in `crates/themeedit-cli/src/lib.rs` to query `resolve_app_path(Path::new("bashmenu.yml"))` to resolve the active theme configured in `bashmenu.yml` before fallback.
- [x] Tests & Verification:
  - Added unit test `test_themeedit_root_frame_background_color_matches_theme` in `crates/themeedit-cli/tests/themeedit_test.rs` asserting initial background fill matches the selected theme background and dynamically updates when changing `background`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (all 34 themeedit tests and all workspace crates passing), and `shellcheck *.sh scripts/*.sh`.

## Phase 36: BashEdit Toggleable Soft-Wrap (<F9>) with Word Wrapping and Visual Row Navigation
- [x] Implemented toggleable soft-wrap in `bashedit` with visual row navigation, whitespace word-boundary wrapping, and logical cursor preservation.
- [x] Core Visual Line Modeling & Wrapped Navigation (`crates/bashedit-cli/src/lib.rs`):
  - Defined `VisualLine` struct tracking logical document row (`doc_row`), chunk slice ranges (`start_col`, `end_col`), and chunk markers (`is_first_chunk`, `is_last_chunk`).
  - Implemented `wrap_line_ranges(chars, width)` enforcing word-boundary wrapping on whitespace with fallback splitting for long words exceeding viewport width.
  - Implemented `compute_visual_lines(lines, width)` to generate flattened visual row layouts for multi-line documents.
  - Implemented `cursor_to_visual(v_lines, cursor_row, cursor_col)` mapping document coordinates to visual row index and column offsets.
  - Implemented visual navigation primitives: `move_visual_up`, `move_visual_down`, `move_visual_home`, `move_visual_end`, `page_visual_up`, and `page_visual_down`.
- [x] Editor Canvas Rendering & Interaction (`crates/bashedit-cli/src/main.rs`):
  - Added CLI flag `--wrap` and footer action `FooterAction::ToggleWrap` (`"F9 Wrap"`).
  - Integrated visual line computation into canvas rendering: displays line number (`format!("{:>4} │ ", doc_row + 1)`) only on the initial chunk (`is_first_chunk`), while continuation chunks render a blank gutter (`"     │ "`).
  - Preserved logical cursor coordinates (`cursor_row`, `cursor_col`) across rendering and status display so document location (`Line A/B, Col C`) behaves as if unwrapped.
  - Mapped `F9` and `Alt+W` (plus footer mouse clicks) to toggle soft-wrap mode dynamically with status notifications.
  - Bound vertical navigation (`Up`, `Down`, `Home`, `End`, `PageUp`, `PageDown`) and mouse click/scroll wheel actions to visual rows when soft-wrap is enabled.
- [x] Documentation & Help Overlays (`crates/bashmenu-core/src/constants.rs`, `bashmenu.md`):
  - Updated `BASHEDIT_HELP_TEXT` to include `| F9 / Alt+W | Toggle Soft-Wrap |`.
  - Updated `bashmenu.md` Section 3.3 built-in editor keybindings table.
- [x] Tests & Verification:
  - Added unit test suite in `crates/bashedit-cli/tests/bashedit_test.rs` covering `test_wrap_line_ranges`, `test_compute_visual_lines`, `test_cursor_to_visual_mapping`, `test_visual_vertical_navigation`, `test_visual_home_and_end_navigation`, and `test_visual_page_navigation`.
  - Updated `test_editor_legend_row_spread` and `--help` CLI flag test.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (all 23 bashedit tests and full workspace passing), and `shellcheck *.sh scripts/*.sh`.

## Phase 37: Cross-Platform `sc` and `cmatrix` Installation Handling (`install_basics.sh`)
- [x] Implemented robust multi-platform installation, repository discovery, and symlink resolution for `sc` and `cmatrix` in `scripts/install_basics.sh`.
- [x] Package Name Translation & Alternative Fallbacks:
  - Added `ensure_sc_symlink()` and `is_pkg_installed()`: detects `sc` or its modern active fork `sc-im`, creating an automated symlink `sc -> sc-im` in `$(get_bin_dir)` when `sc-im` is installed so `sc` commands execute transparently across environments.
  - Termux (`pkg`): Handled `sc-im` package name mapping for `sc`.
  - macOS (`brew`): Handled `sc-im` formula mapping for `sc`.
  - Debian / Ubuntu (`apt`): Handled `universe` repository activation for `cmatrix` and fallback to `sc-im` if classic `sc` is absent.
  - Fedora / RHEL / CentOS / Rocky / AlmaLinux (`dnf` / `yum`): Automated EPEL repository enablement (`epel-release`) for `cmatrix` and `sc-im`.
  - Arch Linux (`pacman`): Handled `sc` / `sc-im` package resolution.
  - Alpine Linux (`apk`): Handled automatic enablement of community repository in `/etc/apk/repositories` for `cmatrix` and `sc-im`.
  - openSUSE (`zypper`): Handled `sc` / `sc-im` fallback.
- [x] Troubleshooting & Solution Guidance (`get_solution_for_pkg`):
  - Added dedicated resolution paths for `sc` and `cmatrix` with commands for each supported package manager.
- [x] Tests & Verification:
  - Verified `shellcheck *.sh scripts/*.sh` (zero warnings).
  - Verified `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`.



## Phase 38: Menuedit Insert/Delete Keys, `--repo` / `-r` CLI Option, and Single Root Defaults
- [x] Menu Editor Key Parity (`crates/menuedit-cli/src/main.rs`, `crates/menuedit-cli/src/shortcut_editor.rs`):
  - In addition to `<d>` initiating menu item deletion, `<Delete>` key now does as well (`KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete`).
  - In addition to `<a>` initiating adding a new menu item, `<Insert>` key now does as well (`KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Insert`).
  - Added `<Insert>` support to shortcut editor item creation for consistency.
  - Added unit test `test_menuedit_insert_and_delete_keys` in `crates/menuedit-cli/tests/menuedit_test.rs`.
- [x] Repository Root CLI Option (`--repo` / `-r` in `crates/bashmenu-cli/src/main.rs`):
  - Added `--repo` / `-r` flag to `Cli` struct in `bashmenu`.
  - When `--repo` / `-r` is passed, `bashmenu` resolves the repository root directory dynamically via `bashmenu_core::resolve_dev_dir(&home, &cfg_dir)` and loads `bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, and `shortcuts.yml` directly from the repo root directory.
  - Added integration test `test_bashmenu_repo_flag_in_help_and_parsing` in `crates/bashmenu-cli/tests/audit_items_test.rs`.
  - Updated `bashmenu.sh` trampoline launcher help to document `-r, --repo`.
- [x] Eradication of `/tests/fixtures` & Unification on Project Root Default Configs:
  - Completely removed the `/tests/fixtures` directory and removed any duplicated configuration templates.
  - Placed `cheat.yml` at project root alongside `bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, and `shortcuts.yml`.
  - Updated `crates/bashmenu-core/src/config.rs` (`resolve_app_path`) and `crates/cheat-cli/src/config.rs` (`get_config_path`) to check the project root directly via `resolve_dev_dir` rather than checking `tests/fixtures`.
  - Updated all unit and integration tests across `bashmenu-core`, `bashmenu-tui`, `menuedit-cli`, and `themeedit-cli` to load configuration files directly from the repository root directory.
  - Updated `install.sh` to copy default configuration files directly from `$SCRIPT_DIR` without falling back to `tests/fixtures`.
  - Updated `docs/ARCHITECTURE.md` architecture diagram.
- [x] Documentation & Invariants Synchronization:
  - Updated `.agents/rules/invariants.md` and `.agents/skills/bashmenu-rs-developer/SKILL.md` to codify that the true and only original configuration files live in the project root, document `--repo` / `-r`, and document `<Insert>` / `<Delete>` keybindings.
  - Updated `bashmenu.md` and `bashmenu.1` with documentation for `--repo` / `-r` and `<Insert>` / `<Delete>`.
- [x] Tests & Verification:
  - `cargo fmt --all --check` passed (zero formatting diffs).
  - `cargo clippy --workspace --all-targets -- -D warnings` passed (zero compiler warnings/errors).
  - `cargo test --workspace` passed (100% test pass across all 9 crates).
  - `shellcheck *.sh scripts/*.sh` passed (clean exit 0).

## Phase 39: Native TrueColor Linux Framebuffer (`/dev/fb0`) TUI Renderer & `-i` / `--image` Full-Resolution Wallpaper Mode
- [x] Framebuffer Font & Procedural Glyph Rasterizer (`crates/bashmenu-tui/src/fb_font.rs`):
  - Implemented self-contained 8x16 bitmap font covering printable ASCII and common UI symbols (`►`, `•`, `…`, `✕`, `✓`, `⚙`, `ℹ`, `★`, etc.) with bold thickening and underline support.
  - Implemented resolution-independent procedural pixel-exact rendering (`draw_procedural_glyph`) for Unicode Block Elements (`U+2580..=U+259F`: `▀`, `▄`, `█`, `▌`, `▐`, `░`, `▒`, `▓`, quadrant blocks `▖..=▟`) and Box-Drawing (`U+2500..=U+257F`: light, heavy, double, rounded, and dashed borders).
- [x] Dual-Mode Framebuffer Compositor (`crates/bashmenu-tui/src/framebuffer.rs`, `crates/bashmenu-tui/src/terminal.rs`, `crates/bashmenu-tui/src/menu_view.rs`):
  - Implemented `render_ratatui_buffer_to_slice` and `FramebufferDevice::render_buffer` with an off-screen RAM backbuffer for tear-free 32-bit/24-bit/16-bit TrueColor rendering directly to `/dev/fb0`.
  - Wrapped `Stdout` in `TerminalOutput` with atomic `suppress_stdout` inside `TerminalGuard` so `fbcon` never overwrites `/dev/fb0` with 8-color VT character cells while framebuffer rendering is active, and cleanly restores/clears `/dev/fb0` on `suspend`, `suspend_keep_alt`, and `Drop`.
  - Default mode renders text-based character-block wallpapers (`WallpaperCompositor`) and menu UI in 24-bit TrueColor.
  - Full-resolution image mode (`-i` / `--image` CLI flag or `settings.framebuffer: image`) pre-scales and opacity-blends the wallpaper at native framebuffer pixel resolution and composites the TrueColor TUI on top (`MenuView::set_framebuffer_image_mode(true)`).
- [x] CLI, Permissions, & Desktop Terminal Guard Parity (`crates/bashmenu-cli/src/main.rs`, `crates/bashmenu-tui/src/framebuffer.rs`, `install.sh`, `bashmenu.sh`, `bashmenu.mnu`, `bashmenu.md`, `bashmenu.1`):
  - Added `-i` / `--image` CLI flag to `bashmenu` and documented in `bashmenu.sh`, `bashmenu.md`, `bashmenu.1`, `.agents/rules/invariants.md`, and `.agents/skills/bashmenu-rs-developer/SKILL.md`.
  - Added desktop terminal guard (`is_desktop_terminal` / `evaluate_console_env`) ensuring `/dev/fb0` is never hijacked when running inside X11/Wayland GUI terminal emulators (`DISPLAY` / `WAYLAND_DISPLAY` / `XDG_SESSION_TYPE=x11|wayland`), while displaying an informational toast when `-i` is used in a desktop terminal.
  - Added `/dev/fb0` permission detection (`is_framebuffer_permission_denied`), interactive `sudo` / `doas` escalation (`ensure_framebuffer_access_interactive`), and `install.sh` setup (`video` group membership + `/etc/udev/rules.d/70-bashmenu-fb.rules` `uaccess` tag + immediate `setfacl` ACL) so virtual consoles (`Ctrl+Alt+F2`..`F6` and minimal Debian installs) render in 24-bit TrueColor without requiring a reboot.
  - Added `pwrite()` dirty-damage flushing alongside `mmap` in `FramebufferDevice::flush_backbuffer` for DRM shadow-buffered drivers (`i915drmfb`, `simpledrm`, `amdgpudrmfb`, `virtio_gpu`) and automatic Nerd Font PUA fallback on Linux virtual consoles.
  - Added unit and integration tests in `crates/bashmenu-tui/src/framebuffer.rs`, `crates/bashmenu-tui/tests/transition_test.rs`, and `crates/bashmenu-cli/tests/audit_items_test.rs`.
  - Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `shellcheck *.sh scripts/*.sh` pass with zero warnings.

## Rename `{which}` Macro to `{env}` / `{environment}`
- [x] Renamed `{which}` macro to `{env}` and `{environment}` in `crates/bashmenu-core/src/macro_engine.rs` (`format_which_info` -> `format_env_info`, interpolation for `{env}` and `{environment}` without conflicting with `{env:VAR}`).
- [x] Updated menu item and shortcut interception in `crates/bashmenu-cli/src/main.rs` to match `{env}`, `{environment}`, and item types `"env"` / `"environment"`.
- [x] Updated `shortcuts.yml` entry `<leader> w` description from `Which Version` to `Environment Info` and updated `action`/`script` to `{env}`.
- [x] Updated documentation and reference tables in `crates/bashmenu-core/src/constants.rs`, `bashmenu.md`, `bashmenu.1`, `.agents/rules/invariants.md`, and `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Updated tests in `crates/bashmenu-core/tests/macro_engine_test.rs` (`test_env_placeholder_resolution`) and `crates/bashmenu-cli/tests/audit_items_test.rs` (`test_env_environment_info_modal_formatting`).
- [x] Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings.

## Add Config Path, `{color_mode}`, and `{tty_mode}` Expanding Placeholders
- [x] Added `{bashmenu_yml}`, `{bashmenu_mnu}`, `{bashmenu_themes}`, `{shortcuts_yml}`, `{cheat_yml}`, `{color_mode}`, and `{tty_mode}` placeholders to `crates/bashmenu-core/src/macro_engine.rs` (plus `evaluate_color_mode`, `detect_color_mode`, `evaluate_tty_mode`, `detect_tty_mode`, and `MacroContext::with_color_mode` / `MacroContext::with_tty_mode`).
- [x] Re-exported the new detection helpers in `crates/bashmenu-core/src/lib.rs` and wired `active_color_mode` and `active_tty_mode` into `make_macro_ctx!` in `crates/bashmenu-cli/src/main.rs`.
- [x] Documented all 7 placeholders in `crates/bashmenu-core/src/constants.rs`, `bashmenu.md`, and `bashmenu.1`.
- [x] Added `test_config_file_color_and_tty_placeholders` in `crates/bashmenu-core/tests/macro_engine_test.rs`.
- [x] Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings.

## Load `{env}` Macro Content from `assets/about.md` with YAML Frontmatter
- [x] Fixed `{bashmenu_themes }` typo to `{bashmenu_themes}` in `assets/about.md`.
- [x] Added `parse_markdown_frontmatter`, `MacroContext::resolve_about_md_path`, and `MacroContext::render_env_about` in `crates/bashmenu-core/src/macro_engine.rs` (with compile-time `include_str!("../../../assets/about.md")` fallback) and updated `MacroContext::format_env_info` to render the Markdown body.
- [x] Updated `crates/bashmenu-cli/src/main.rs` `{env}` / `{environment}` modal interception to use `ctx.render_env_about()` for both the YAML frontmatter `title` and the interpolated Markdown body.
- [x] Updated `.agents/rules/invariants.md`, `.agents/skills/bashmenu-rs-developer/SKILL.md`, `crates/bashmenu-core/tests/macro_engine_test.rs`, and `crates/bashmenu-cli/tests/audit_items_test.rs`.
- [x] Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings.

## Add `{info:<path>}` Markdown Popup Macro with Frontmatter `width` / `height` Percentages
- [x] Added `MarkdownFrontmatter` (`title`, `width`, `height` percentage), `parse_markdown_frontmatter_full`, `find_info_macro_span`, `extract_info_macro_path`, `MacroContext::render_env_about_full`, and `MacroContext::render_info_file` (supporting nested placeholders and `~` tilde expansion) in `crates/bashmenu-core/src/macro_engine.rs` and re-exported in `crates/bashmenu-core/src/lib.rs`.
- [x] Wired `{info:<path>}` inline expansion in `interpolate_placeholders` and interactive popup modal interception in `crates/bashmenu-cli/src/main.rs` (for both `{env}` and `{info:<path>}`).
- [x] Added `width_percent` and `height_percent` overrides (`MessageModal::with_dimensions`) in `crates/bashmenu-tui/src/modals/message.rs`.
- [x] Documented `{info:<path>}` and frontmatter `title`, `width`, and `height` in `crates/bashmenu-core/src/constants.rs`, `bashmenu.md`, and `bashmenu.1`.
- [x] Added `test_info_macro_and_frontmatter_dimensions` in `crates/bashmenu-core/tests/macro_engine_test.rs` and `test_message_modal_percentage_dimensions` in `crates/bashmenu-tui/tests/modals_test.rs`.
- [x] Verified `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with zero warnings.

