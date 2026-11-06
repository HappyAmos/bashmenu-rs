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









