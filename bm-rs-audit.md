# BashMenu-RS Documentation & Codebase Parity Audit Report

## 1. Executive Summary
A thorough audit was conducted across the user manual ([`bashmenu.md`](file:///home/andrew/github/bashmenu-rs/bashmenu.md)), the terminal man page ([`bashmenu.1`](file:///home/andrew/github/bashmenu-rs/bashmenu.1)), and all crates in the `bashmenu-rs` workspace (`bashmenu-core`, `bashmenu-tui`, `bashmenu-cli`, `menuedit-cli`, `themeedit-cli`, `bashedit-cli`, `bmpager-cli`, `ymlcheck-cli`).

Both [`bashmenu.md`](file:///home/andrew/github/bashmenu-rs/bashmenu.md) and [`bashmenu.1`](file:///home/andrew/github/bashmenu-rs/bashmenu.1) have been updated and synchronized to reflect the Rust binaries (`bashmenu`, `menuedit`, `bashedit`, `themeedit`, `bmpager`, `ymlcheck`), current keybindings, companion tool delegation, and all dynamic directives.

All items previously identified in Section 3 have been fully implemented, verified with unit/integration tests, and synchronized with 100% specification parity.

---

## 2. Synchronized in `bashmenu.md` & `bashmenu.1`
| Area | Previous State | Updated / Synchronized State |
| :--- | :--- | :--- |
| **Component Binaries** | Referenced legacy Python filenames (`bashmenu.py`, `menuedit.py`, `bashedit.py`, `themeedit.py`, `ymlcheck.py`) and omitted `bmpager`. | Updated to native Rust binaries (`bashmenu`, `menuedit`, `bashedit`, `themeedit`, `bmpager`, `ymlcheck`) across both `bashmenu.md` and `bashmenu.1`. |
| **Main Menu `<F1>` & `<F5>`** | `<F1>` documented as built-in modal; `<F5>` only mentioned basic reload. | Documented `<F1>` delegation to `bmpager --markdown bashmenu.md` and `<F5>` full system refresh (cache, config, menu, themes, wallpapers, gutters, and forced plugin refresh). |
| **Built-in Editor (`bashedit`) `<F6>`** | Referenced legacy `scripts/rich.sh`. | Updated to `bmpager --markdown --no-file`. |
| **Visual Theme Editor (`themeedit`)** | Referenced legacy Python `F2`–`F9` bindings. | Updated to match `themeedit` collapsible token tree (`Theme:`, `Colors:`, `Borders:`), `Left`/`Right` node expansion, preview click autofocus, `[` / `]` theme cycling, `TAB`/`1`-`5` multi-view preview switching, and `<F5>`/`<F9>` wallpaper palette derivation (including `plugin` text color). |
| **Interactive Pager (`bmpager`)** | No dedicated keybinding reference section in `bashmenu.md` or `bashmenu.1`. | Added Section 3.5 (`Interactive Terminal Pager (bmpager)`) in both `bashmenu.md` and `bashmenu.1`. |
| **Placeholders & Directives** | Missing `{dev_dir}` / `{repo_dir}` in both docs; `bashmenu.1` was missing `type: toast` and `{toast:SECONDS:"Message"}`. | Added `{dev_dir}` / `{repo_dir}` to both docs, and added `type: toast` plus `{toast:SECONDS:"Message"}` to `bashmenu.1`. |

---

## 3. Codebase Parity Audit: Implementation Status

### 3.1 Unimplemented Features (Now Fully Implemented & Tested)
1. **`type: inject_block` Runtime Execution ([`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs))**:
   - **Resolved State:** `MenuAction::Execute` parses `template`, `target`, `block_id`, expands placeholders, and presents an interactive `ConfirmModal`. Selecting **Yes** installs or updates `# CODEBLOCK:<id>:START ... # CODEBLOCK:<id>:END` with 0o755 script permissions; selecting **No** uninstalls the block cleanly.
2. **`type: message` Standalone Menu Item ([`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs))**:
   - **Resolved State:** Standalone top-level items with `type: message` (or `popup`/`info`) display `ActiveModal::Message` with interpolated title and message content.
3. **`settings.inactivity_timeout` Runtime Timer Loop ([`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs))**:
   - **Resolved State:** Main loop tracks user keystroke and mouse interactions. When idle duration reaches `settings.inactivity_timeout.milliseconds`, `command` is spawned with interpolated context.
4. **`user_mode: "root"` Privilege Elevation ([`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs))**:
   - **Resolved State:** Items specifying `user_mode: "root"` automatically prepend `sudo ` when executed by non-root users (`libc::geteuid() != 0`).

---

### 3.2 Partially Implemented Features (Now Fully Implemented & Tested)
1. **Configurable `settings.refresh_interval` & Adaptive Status Gutter Polling ([`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs), [`crates/bashmenu-tui/src/menu_view.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-tui/src/menu_view.rs))**:
   - **Resolved State:** Polling dynamically respects `settings.refresh_interval` while adapting to 1-second intervals if second-precision tokens (`{date_time_24}`, etc.) are active. Status gutter token wrapping enforces status precedence over help gutter to guarantee date/time visibility.
2. **Per-Item Editor Options (`show_whitespace`, `tab_to_spaces`) ([`crates/bashedit-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashedit-cli/src/main.rs), [`crates/bashmenu-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/bashmenu-cli/src/main.rs))**:
   - **Resolved State:** `bashedit` CLI supports `--show-whitespace` and `--tab-to-spaces <bool>` flags. `bashmenu` forwards per-item `show_whitespace` and `tab_to_spaces` YAML settings when invoking `bashedit`.
3. **`ymlcheck` Option Type List ([`crates/ymlcheck-cli/src/main.rs`](file:///home/andrew/github/bashmenu-rs/crates/ymlcheck-cli/src/main.rs))**:
   - **Resolved State:** `VALID_OPTION_TYPES` in `ymlcheck` includes `"toast"`, `"inject_block"`, and `"message"`, validated with unit tests.
