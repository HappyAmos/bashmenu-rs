# Developer Guidelines for AI Agents (AGENTS.md)

This document provides developer guidelines and operational invariants for AI agents working in the `bashmenu-rs` codebase. The authoritative skill definition is located at [`.agents/skills/bashmenu-rs-developer/SKILL.md`](.agents/skills/bashmenu-rs-developer/SKILL.md).

---

## 1. Installation & Uninstallation Invariants

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

---

## 2. Help Gutter Layout & Shortcut Keys

- **Navigation Hint Omission:** Do not include `[UP/DN]: Nav` in the help gutter across any view (standard menu, shortcuts mode, notebook view). Arrow and vim navigation is intuitive and omitting this text frees critical screen real estate.
- **Priority Placement for F8 and F9:** Hardcoded keys `[F8]: ThemeEdit` and `[F9]: ShortcutEdit` must be positioned immediately following `[F2]: Themes`. This ensures that even on narrow or standard 80- and 100-column terminals, both editors remain visible on the 2nd gutter line and fully clickable via mouse events.
- **Dynamic Gutter Sizing:** Gutter dimensions (`left_gutter_w`, `right_gutter_w`, `sep_x`) must dynamically balance width between the help gutter and status gutter based on active status tokens, rather than enforcing a rigid 50/50 split that starves the help gutter.

---

## 3. General Development Lifecycle

1. **Format:** `cargo fmt --all --check`
2. **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings allowed)
3. **Tests:** `cargo test --workspace`
4. **Shellcheck:** `shellcheck *.sh scripts/*.sh`
5. **Git Push & CI:** All changes must be committed and pushed to `origin/main`, and GitHub Actions CI run must complete with status `'success'`.
6. **Task Tracking:** Always maintain `~/.config/bashmenu/notebooks/notes.yml` (`PENDING:` marker while in progress, pruned upon completion) and log completed tasks to `.agents/skills/bashmenu-rs-developer/DONE.md`.
