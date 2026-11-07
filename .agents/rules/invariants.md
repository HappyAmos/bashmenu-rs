# BashMenu-RS Development Invariants & Guidelines

## 1. Mandatory Git Commit & Push Workflow
- **Commit & Push Requirement:** Every completed task, bug fix, feature, or refactoring step must conclude by creating a concise, descriptive git commit (`git commit -m "..."`) and pushing directly to GitHub (`git push origin main`).
- **Clean Workspace:** Never leave modified or newly created source/config files uncommitted or unpushed in the local repository.
- **Remote CI Verification:** After pushing to GitHub, verify remote CI passes using the GitHub CLI:
  - Query run ID: `gh run list --workflow=rust-app.yml --limit 1 --json databaseId -q '.[0].databaseId'`
  - Watch run: `gh run watch <ID>` (always pass an explicit ID to avoid interactive hangs).

## 2. Configuration & State Persistence
- **Single Source of Truth Config Path & `{dev_dir}`:** User configurations reside in `~/.config/bashmenu/` (`{bashmenu_dir}`), while `{dev_dir}` dynamically resolves the local git repository root directory (never hardcoded).
- **Config Header Preservation:** When saving or modifying `bashmenu.yml`, ALWAYS use `bashmenu_core::save_config(path, config)` instead of raw YAML dumps. This guarantees the top-level `CONFIG_HEADER` comment documentation is preserved.
- **Theme Editor Save Invariant (`themeedit`):** <kbd>Ctrl+S</kbd> must save directly to disk whenever `theme_name` is set. Only untitled themes divert to "Save As". <kbd>Ctrl+A</kbd> opens "Save As".
- **Alphabetical Theme Ordering & Palette Derivation:** Theme pickers (`ThemePickerModal`), theme cycling, and `save_themes()` must always sort themes alphabetically (case-insensitively). In `themeedit`, <kbd>F5</kbd> auto-derives theme colors (including `plugin` text color) from the configured background wallpaper image and clears view-specific sub-table overrides (`menu_editor`, `editor`, `modals`).
- **Colocated Swap Files (`bashedit`):** Text editor swap files `.<filename>.swp` must reside in the exact same directory as the target file being edited (never in `/tmp` or central directories). Clean exit removes the swap file. On startup, dead PID swap files prompt recovery, while active PID swap files warn of collision and open in read-only mode.
- **Theme Color Pair Transparency (-1):** In `bashmenu.themes`, background value `-1` in style pairs (e.g. `[255, -1]`) signifies transparent / container-inherited background. Never map `-1` to `Color::Reset` in style pairs as it emits ANSI `\x1b[49m` (black background wash) over colored modal surfaces.
- **3-Tier Theme Palette Parity (256 / 16 / 8):** Themes in `bashmenu.themes` must maintain full token parity across all three palette tiers: `256`, `16`, and `8`. Auto-derivation (`derive_colors_from_wallpaper`), new theme creation (`Theme::monochrome()`), and token updates (`set_color_token_value`) must automatically downsample colors into valid ANSI 16-color and 8-color constants (`COLOR_*`) using perceptual luminance and HSV hue classification, keeping fallback tiers synchronized without view-specific sub-table overrides (`menu_editor`, `editor`, `modals`).

## 3. Terminal Safety & UI Performance Invariants
- **Terminal Panic Hook Invariant:** Always install the global panic recovery hook (`bashmenu_tui::install_panic_hook()`) before initializing terminal raw mode. The hook resets terminal raw mode, leaves the alternate screen buffer, and restores cursor visibility if a crash occurs.
- **Flicker-Free Alternate Screen Suspension & Synchronized Redraw (`TerminalGuard`):** When spawning child TUIs (`bashedit`, `menuedit`, `themeedit`, `bmpager`) or `alt_buffer: true` console commands, always use `guard.suspend_keep_alt(...)` (which sets `BASHMENU_CHILD_ALT=1` so child tools do not emit `LeaveAlternateScreen` on exit). Only use `guard.suspend(...)` when `alt_buffer: false`. Never call `terminal.clear()` (`\x1b[2J`) on resume or reload; use `guard.request_clear()` (buffer sentinel invalidation) and `guard.draw(...)` (DEC Mode 2026 `\x1b[?2026h\x1b[0m`...`\x1b[?2026l` synchronized output).
- **Full System Refresh (`<F5>`) Without Layout Jumps:** `<F5>` in `bashmenu` clears the command cache, reloads `bashmenu.yml`, `bashmenu.mnu` (preserving submenu/notebook navigation via `MenuView::reload_menu`), `bashmenu.themes`, wallpapers (reusing cached textures via `WallpaperCompositor::set_config` if unchanged), and status/help gutters, and triggers a forced plugin refresh (`trigger_refresh(&config, true)`) while keeping existing cached plugin rows visible until updated outputs arrive.
- **Companion Tool Delegation (`bashedit` `<F6>`):** Markdown preview (`<F6>`) in `bashedit` must delegate to `bmpager --markdown --no-file <temp_file>` via `guard.suspend_keep_alt` rather than using a duplicate built-in modal.
- **Immediate Mouse Interaction:** Handle `MouseEventKind::Down(MouseButton::Left)` directly rather than waiting for `MouseUp` or `Click` so users get instantaneous feedback without perceptible lag.
- **Icon Slot Width Invariant:** Main menu icon slots must always be exactly 4 visual columns wide (`vis_w + pad_w = 4`) regardless of glyph width or variation selectors.
- **Menu Priority Over Plugins:** When terminal viewport height is constrained, plugin rows must automatically truncate or drop first so menu items and the status gutter remain completely visible.
- **Modal View Selection & Background Invariant:** Text inside modal frames must inherit the modal frame's `styles.background`. Active/highlighted items or selectable links in note viewers must use inverted text (`Modifier::REVERSED`) rather than glyph markers (`►`) or clashing highlight background fills.
- **Unicode-Safe Text Editing & Universal Clipboard:** All single-line (`TextInput`) and multi-line text editors (`bashedit`, `NoteEditorModal`, `menuedit` multi-line fields) must operate on Unicode `char` counts (never byte slices), soft-wrap at word boundaries (`wrap_chars_by_word`), and support `Shift+Directional` selection plus `Ctrl+C` / `Ctrl+V` / `Ctrl+X` clipboard operations (`bashmenu_tui::clipboard`).
- **Spread Help Gutters, Strict Function Key Order & Dynamic Sizing:** Help gutters may use up to 2 lines above the bottom border, treating `|` as a word delimiter (`wrap_spread_help_gutter`) and spreading items across the full width. Navigation hint omission: do NOT include `[UP/DN]: Nav` in the help gutter across any view (standard menu, shortcuts mode, notebook view) to conserve critical line width. Strictly Ascending Order: Function keys `<F1>`, `<F2>`, `<F3>`, etc. listed in the help gutter must ALWAYS be listed in strictly ascending numerical order (from 1 to 12). Dynamic gutter sizing: gutter dimensions (`left_gutter_w`, `right_gutter_w`, `sep_x`) must dynamically balance width between the help gutter and status gutter based on active status tokens, avoiding a rigid 50/50 split that starves the help gutter. Panel titles must connect directly to borders (`┌─[ Title ]───`). Scrollable views (`menuedit` tree, `NoteViewerModal`, `NoteEditorModal`) must reserve 1 row above the bottom border and render a scrollbar 1 column inside the right border when content overflows.
- **Status Gutter Precedence & Date/Time Retention:** The status gutter takes layout precedence over the help gutter. In narrow terminals (e.g. 80 columns) with multiple active status tokens (user, battery, host, date/time), the help gutter yields columns to ensure no trailing status tokens or date/time stamps are dropped or truncated.
- **Toast Notification Dispatch Invariant:** Non-blocking toast notifications (`{toast:SECONDS:"MESSAGE"}` or `type: toast`) must dock in the bottom-right corner above the status gutter. They must display the colored left accent block `▌` (`\u{258c}`), theme panel background, bold title, and wrapped body. They dismiss automatically on timeout or immediately on mouse click. When `{toast:...}` appears in a menu command/script and leaves an empty command string, never execute an external shell.
- **Marquee Ticker & Overflow Invariant:** Selected menu items overflowing available column width must scroll horizontally via `get_marquee_slice` using the active theme's `marquee.char` and `marquee.gap`. Navigation or selection changes must reset the marquee with a 4-tick (1.0s) pause delay. Non-selected items must truncate with an ellipsis (`…`). When any modal screen is active, the marquee ticker must pause.
- **TUI Border Multi-Byte Visual Width Invariant:** When constructing borders and calculating dialog padding, always measure text and icon width using visual terminal columns (`UnicodeWidthStr::width()`), never byte length (`.len()`). Multi-byte UTF-8 glyphs (`▲`, `▼`, `•`, `▌`) will otherwise offset corners and indent borders.
- **Modal Text Clamping & Word Wrapping Invariant:** Text inside popup modals MUST NEVER exceed visible window width. In cases where it may exceed window width, text MUST be wrapped (e.g. using `wrap_prompt_text`) or safely clipped within available inner columns, preventing text from spilling outside borders or wrapping onto borders.
- **Modal Frame Close Button Invariant (`is_close_button_clicked`):** Every popup dialog rendered via `render_modal_frame` displays an `[X]` close button at the top-right of its frame. All modal `handle_mouse` implementations MUST test `is_close_button_clicked(inner, mouse.row, mouse.column)` and return `Some(None)` or a cancellation action on click. Parent event loops must pass proper viewport rects to modals so frame geometries resolve accurately.
- **File and Directory Picker Hidden Files Invariant (`FilePickerModal`):** `FilePickerModal` must support toggling dotfiles visibility via <kbd>h</kbd>, <kbd>H</kbd>, <kbd>.</kbd>, and <kbd>Ctrl+H</kbd>, and via an interactive clickable button (`[h] Show Hidden` / `[h] Hide Hidden`) on the bottom controls bar, preserving selected item paths across directory reloads.
- **Mandatory Screen Transitions & Zero-Dimension Guard Invariant:** Visual screen transitions (`settings.transition`: `fade`, `curtain`, `dissolve`, `matrix`, `slide_left`, `slide_right`, `blinds`, `iris`, `wipedown`) and speed profiles (`settings.transition_speed`: `fast`, `normal`, `slow`, or ms) must be active across all interface changes (launch/startup, submenu navigation, back/Esc, page up/down, modal open/close, app return) unless explicitly disabled on an item (`transition: false`). Transition drawing routines (`draw_with_transition`, `draw_with_transition_out`) must short-circuit immediately on zero-dimension viewports (`width == 0 || height == 0`) to prevent artificial sleep delays in headless or minimized test environments.
- **Editor Bounded Undo Stack Invariant (`bashedit`):** `EditorBuffer` caps undo history at 100 snapshots to prevent runaway heap allocations when editing large files. Undoing beyond the 100-snapshot bounded capacity must safely and gracefully return `false` without crashing.

## 4. Testing & Code Quality Invariants
- Before committing, always verify:
  1. Formatting: `cargo fmt --all --check`
  2. Compiler & Clippy Lints: `cargo clippy --workspace --all-targets -- -D warnings` (treat all Clippy warnings as errors)
  3. Tests: `cargo test --workspace`
  4. Shell Scripts: `shellcheck *.sh scripts/*.sh`
- In terminal scripts, do not assume `/usr/local/bin` for non-root users; install user CLI tools to `$HOME/.local/bin`.
- **Shellcheck Scope Invariant:** Verify shell scripts using `shellcheck *.sh scripts/*.sh` so extension scripts in `scripts/` are checked alongside root scripts.
- **Self-Contained Implementation & No External Tool Dependencies Invariant:** BashMenu must never rely on external tools, external window managers, or foreign environments (e.g. `kmscon`, `fbterm`, desktop environments) to achieve core application goals. Everything must be implemented natively and self-contained within the Rust workspace crates. When running in environments lacking full 24-bit TrueColor (such as Linux VT console `TERM=linux`), the application must automatically detect capabilities and gracefully fall back to the configured 16-color and 8-color theme palette tiers.
- **Cargo Integration Test Binary Scoping (`CARGO_BIN_EXE_<bin>`):** Cargo sets `env!("CARGO_BIN_EXE_<bin>")` only for binaries defined within the same crate. CLI integration tests for specific binaries (e.g. `bashedit`, `bashmenu`, `bmpager`) must be located inside that specific crate's `tests/` directory.
- **Multi-Crate Cargo Binary Test Invocation (`-p <pkg> --bin <bin>`):** When invoking workspace executables via `Command::new("cargo")` in cross-crate integration tests, always pass `-p <package> --bin <binary>`. Calling `cargo run --bin <binary>` without `-p` fails in multi-crate workspaces when the target binary is not in the default root package.
- **Doc Comment Line Length Invariant (<80 chars):** All `///` and `//!` doc comments across all workspace crates must strictly wrap at or under 80 columns.
- **Forbidden Keybindings (<kbd>F11</kbd>):** Never bind `<F11>` anywhere in interactive navigation or child TUIs; `<F11>` is reserved by Linux desktop window managers for fullscreen toggle. Use `<F12>` or alternate unreserved keys.
- **Code Safety & Error Handling (No Naked Panics):** Avoid naked `.unwrap()` and `.expect()` in interactive runtime loops, macro interpolation, or file parsing. Return strongly-typed errors using `thiserror` (e.g. `BashMenuError`) or propagate via `anyhow::Result` / `color_eyre::Result` at the binary boundary.
- **No Endless Loops Without Escape Paths:** Never construct interactive event loops or modal states that lack a dedicated escape/exit key sequence (`ESC`, `q`, `Ctrl+C`).
- **No Hardcoded Absolute Paths:** Never hardcode absolute user paths; always use dynamic resolving macros (`{home}`, `{bashmenu_dir}`, `{scripts_dir}`, `{templates_dir}`, `{cache_dir}`, `{dev_dir}`).
- **CP437 ASCII Macro Interpolation Invariant (`{ascii:NNN}`):** Any code reading theme glyphs or configuration strings must translate `{ascii:NNN}` macros to CP437 bytes. Raw `{ascii:NNN}` strings must NEVER appear unexpanded in visible menu options, indicators, or status lines.
- **Universal Documentation & 6-Tier Parity Invariant:** Every crate, module, struct, enum, and public function must have `///` doc comments (wrapped under 80 characters). When introducing or extending placeholders, macros, directives, options, or flags, update all six documentation tiers:
  1. Built-in help/reference modal in `bashmenu-tui`
  2. Inspector guide help modal in `menuedit-cli`
  3. Editor manual modal in `bashedit-cli`
  4. Pager manual modal in `bmpager-cli`
  5. Markdown guide (`bashmenu.md`)
  6. Linux man page (`bashmenu.1`)

## 5. TODO Task Execution & Lifecycle Invariants (CRITICAL)
- **Sequential TODO Execution:** Unless specifically labeled (`CRITICAL`) or explicitly instructed otherwise by the user, always tackle items from TODO lists (`~/.config/bashmenu/notebooks/notes.yml` under `'TODO (Agent: Allowed)'` or `.agents/skills/bashmenu-rs-developer/TODO.md`) strictly in sequential order, top to bottom. Complete each item fully (including verification, commit, push, and remote CI check) before moving to the next.
- **Add Unlisted Instructions:** If given an instruction that is not in a TODO list, you MUST add the instruction to the TODO list before working on it.
- **Pending Marker (`PENDING`):** When working on a TODO item, you MUST mark it as `PENDING` (`- [ ] PENDING: ...`). When completed, remove the `PENDING` label and mark it as complete (`- [x] ...`).
- **Log to `DONE.md` & Prune Finished Tasks:** Append a summary of every completed task to `.agents/skills/bashmenu-rs-developer/DONE.md`. When there are no more tasks to complete, remove finished (`- [x]`) tasks from the TODO list, and re-check the TODO list to make sure no new tasks have been added.
- **Long-Running Process Periodic Status Updates:** When working on long running processes (tasks that are more than 1 minute to complete), give a short blurb of what you are working on at least every 2 minutes.

## 6. Installation & Uninstallation Invariants

### Installer (`install.sh`)
- **Workspace Compilation:** Must compile all binaries in release mode (`cargo build --release --workspace`).
- **Binary Installation:** Must install all 8 workspace binaries into `$BIN_DIR` (`$HOME/.local/bin` or `$PREFIX/bin` or `/usr/local/bin`):
  - `bashmenu` (Main menu TUI)
  - `ymlcheck` (YAML validation CLI)
  - `menuedit` (Visual menu editor)
  - `shortcutedit` (Visual shortcut editor)
  - `themeedit` (Visual theme and palette editor)
  - `bashedit` (Swap-safe text editor)
  - `bmpager` (Native terminal pager with markdown support)
  - `cheat` (Interactive cheatsheet browser)
- **Convenience Symlinks:** Must link `bm` -> `bashmenu` and `rich` -> `rich.sh`.
- **Manual Page Installation:** Must install `bashmenu.1` to `$MAN_DIR` (`$HOME/.local/share/man/man1` or system equivalent).
- **Configuration Seeding:** Must copy initial configuration fixtures (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`, `shortcuts.yml`, `cheat.yml`) into `~/.config/bashmenu/` without overwriting existing user files.
- **Output Banner:** The summary of installed commands displayed to the user must explicitly list `cheat` along with all other tools.

### Uninstaller (`uninstall.sh`)
- **Complete Binary Cleanup:** Must remove all installed binaries and symlinks (`bashmenu`, `bm`, `ymlcheck`, `menuedit`, `shortcutedit`, `themeedit`, `bashedit`, `bmpager`, `cheat`, `rich`) from `$BIN_DIR`.
- **Manual Page Cleanup:** Must remove installed manual page `bashmenu.1` from `$MAN_DIR`.
- **Settings Preservation:**
  - Interactive mode must prompt the user whether to keep their settings and notebooks in `~/.config/bashmenu` with a default of **Yes (Keep)** (`[Y/n]`).
  - Supports non-interactive flags:
    - `-k`, `--keep-settings`, `--keep-config`: Explicitly preserves user settings.
    - `--remove-settings`, `--remove-config`: Explicitly removes user settings.
    - `-y`, `--yes`: Non-interactive uninstall.


