# BashMenu-RS Development Invariants & Guidelines

## 1. Mandatory Git Commit & Push Workflow
- **Commit & Push Requirement:** Every completed task, bug fix, feature, or refactoring step must conclude by creating a concise, descriptive git commit (`git commit -m "..."`) and pushing directly to GitHub (`git push origin main`).
- **Clean Workspace:** Never leave modified or newly created source/config files uncommitted or unpushed in the local repository.
- **Remote CI Verification:** After pushing to GitHub, verify remote CI passes using the GitHub CLI:
  - Query run ID: `gh run list --workflow=rust-app.yml --limit 1 --json databaseId -q '.[0].databaseId'`
  - Watch run: `gh run watch <ID>` (always pass an explicit ID to avoid interactive hangs).

## 2. Configuration & State Persistence
- **Config Header Preservation:** When saving or modifying `bashmenu.yml`, ALWAYS use `bashmenu_core::save_config(path, config)` instead of raw YAML dumps. This guarantees the top-level `CONFIG_HEADER` comment documentation is preserved.
- **Theme Editor Save Invariant (`themeedit`):** <kbd>Ctrl+S</kbd> must save directly to disk whenever `theme_name` is set. Only untitled themes divert to "Save As". <kbd>Ctrl+A</kbd> opens "Save As".
- **Colocated Swap Files (`bashedit`):** Text editor swap files `.<filename>.swp` must reside in the exact same directory as the target file being edited (never in `/tmp` or central directories). Clean exit removes the swap file. On startup, dead PID swap files prompt recovery, while active PID swap files warn of collision and open in read-only mode.
- **Theme Color Pair Transparency (-1):** In `bashmenu.themes`, background value `-1` in style pairs (e.g. `[255, -1]`) signifies transparent / container-inherited background. Never map `-1` to `Color::Reset` in style pairs as it emits ANSI `\x1b[49m` (black background wash) over colored modal surfaces.

## 3. Terminal Safety & UI Performance Invariants
- **Terminal Panic Hook Invariant:** Always install the global panic recovery hook (`bashmenu_tui::install_panic_hook()`) before initializing terminal raw mode. The hook resets terminal raw mode, leaves the alternate screen buffer, and restores cursor visibility if a crash occurs.
- **Immediate Mouse Interaction:** Handle `MouseEventKind::Down(MouseButton::Left)` directly rather than waiting for `MouseUp` or `Click` so users get instantaneous feedback without perceptible lag.
- **Icon Slot Width Invariant:** Main menu icon slots must always be exactly 4 visual columns wide (`vis_w + pad_w = 4`) regardless of glyph width or variation selectors.
- **Menu Priority Over Plugins:** When terminal viewport height is constrained, plugin rows must automatically truncate or drop first so menu items and the status gutter remain completely visible.
- **Modal View Selection & Background Invariant:** Text inside modal frames must inherit the modal frame's `styles.background`. Active/highlighted items or selectable links in note viewers must use inverted text (`Modifier::REVERSED`) rather than glyph markers (`►`) or clashing highlight background fills.
- **Unicode-Safe In-Modal Text Editing:** In-modal text editors and viewers must soft-wrap at word boundaries (`wrap_chars_by_word`). Cursor navigation, slicing, and insertions must strictly operate on Unicode `char` counts (never byte slices) to prevent character corruption and panics. <kbd>End</kbd> key must navigate to the logical line end.
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
