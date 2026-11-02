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
- **Spread Help Gutters, Connected Titles & Scrollbars:** Help gutters may use up to 2 lines above the bottom border, treating `|` as a word delimiter (`wrap_spread_help_gutter`) and spreading items across the full width. Panel titles must connect directly to borders (`┌─[ Title ]───`). Scrollable views (`menuedit` tree, `NoteViewerModal`, `NoteEditorModal`) must reserve 1 row above the bottom border and render a scrollbar 1 column inside the right border when content overflows.
- **Toast Notification Dispatch Invariant:** Non-blocking toast notifications (`{toast:SECONDS:"MESSAGE"}` or `type: toast`) must dock in the bottom-right corner above the status gutter. They must display the colored left accent block `▌` (`\u{258c}`), theme panel background, bold title, and wrapped body. They dismiss automatically on timeout or immediately on mouse click. When `{toast:...}` appears in a menu command/script and leaves an empty command string, never execute an external shell.
- **Marquee Ticker & Overflow Invariant:** Selected menu items overflowing available column width must scroll horizontally via `get_marquee_slice` using the active theme's `marquee.char` and `marquee.gap`. Navigation or selection changes must reset the marquee with a 4-tick (1.0s) pause delay. Non-selected items must truncate with an ellipsis (`…`). When any modal screen is active, the marquee ticker must pause.
- **TUI Border Multi-Byte Visual Width Invariant:** When constructing borders and calculating dialog padding, always measure text and icon width using visual terminal columns (`UnicodeWidthStr::width()`), never byte length (`.len()`). Multi-byte UTF-8 glyphs (`▲`, `▼`, `•`, `▌`) will otherwise offset corners and indent borders.

## 4. Testing & Code Quality Invariants
- Before committing, always verify:
  1. Formatting: `cargo fmt --all --check`
  2. Compiler & Clippy Lints: `cargo clippy --workspace --all-targets -- -D warnings` (treat all Clippy warnings as errors)
  3. Tests: `cargo test --workspace`
  4. Shell Scripts: `shellcheck *.sh`
- In terminal scripts, do not assume `/usr/local/bin` for non-root users; install user CLI tools to `$HOME/.local/bin`.

## 5. TODO Task Execution Ordering Invariant
- **Sequential TODO Execution:** Unless specifically labeled or explicitly instructed otherwise by the user, always tackle items from TODO lists (such as `notes.yml` or `TODO.md`) strictly in sequential order, top to bottom. Complete each item fully (including verification, commit, push, and remote CI check) before moving to the next.

