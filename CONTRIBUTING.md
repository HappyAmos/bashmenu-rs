# Contributing to BashMenu-RS

Thank you for your interest in contributing to **BashMenu-RS**! We welcome contributions from developers of all backgrounds, whether you're fixing a bug, adding a new theme or cheatsheet, improving terminal performance, or translating the application.

Please take a moment to review this guide before submitting issues or pull requests.

---

## Code of Conduct

All contributors and community participants are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please treat others with respect and professional courtesy.

---

## Getting Started

### 1. Prerequisites & Toolchain

To build and test BashMenu-RS, ensure you have:
- **Rust 1.80+** (stable): Install via [rustup.rs](https://rustup.rs).
- **Cargo tools:**
  ```bash
  rustup component add rustfmt clippy
  ```
- **ShellCheck:** For linting POSIX/Bash scripts (`sudo apt install shellcheck`, `brew install shellcheck`, or your package manager).
- **Make:** Optional, but recommended for running common developer workflows via `Makefile`.

### 2. Workspace Overview

BashMenu-RS is structured as a multi-crate Cargo workspace:

| Crate / Component | Role | Description |
| :--- | :--- | :--- |
| `crates/bashmenu-core` | Shared Library | Core data models, Serde loaders, 6-tier macro interpolation, glyph resolution, and notebook management. |
| `crates/bashmenu-tui` | Reusable TUI | Ratatui widgets, custom borders, drop shadows, 2D color/ASCII pickers, wallpaper compositing, and screen transitions. |
| `crates/bashmenu-cli` | Binary (`bashmenu`, `bm`) | The primary terminal menu event loop, process supervisor, and screensaver engine. |
| `crates/menuedit-cli` | Binary (`menuedit`, `shortcutedit`) | Visual menu hierarchy and leader keyboard chord editor. |
| `crates/themeedit-cli` | Binary (`themeedit`) | Visual split-screen theme and palette editor. |
| `crates/bashedit-cli` | Binary (`bashedit`) | Standalone text editor with Unicode buffer and colocated swap files (`.<file>.swp`). |
| `crates/bmpager-cli` | Binary (`bmpager`) | Terminal pager with Markdown rendering and OSC 8 hyperlinks. |
| `crates/cheat-cli` | Binary (`cheat`) | Interactive cheatsheet search engine and Markdown viewer. |
| `crates/ymlcheck-cli` | Binary (`ymlcheck`) | Schema validation CLI for YAML configuration and themes. |
| `scripts/` | Shell Tools | Companion helper scripts and extension utilities. |

---

## Development Workflow

We provide a convenient [Makefile](Makefile) to streamline common tasks:

```bash
# Run formatting, clippy, and shellcheck linters
make check

# Run the complete pre-commit suite (fmt, clippy, tests, shellcheck)
make verify

# Run workspace unit and integration tests
make test

# Format code in-place
make fmt-fix

# Build all binaries in release mode
make release
```

> **Tip for Fast Test Iteration:**
> Running `make test` executes tests across all crates, including image-clustering tests in `bashmenu-tui`. During local feature development, you can run targeted tests for faster iteration:
> ```bash
> cargo test -p bashmenu-core
> cargo test -p bashmenu-tui --test style_test
> ```

---

## Architectural & UX Invariants

BashMenu-RS enforces several non-negotiable engineering invariants to ensure stability, terminal safety, and visual polish across different terminal emulators:

### 1. Terminal Safety & Raw Mode
- **No Unhandled Panics in Raw Mode:** All entry points must install the panic hook (`bashmenu_tui::install_panic_hook()`) so that unexpected panics restore raw mode and leave the alternate screen cleanly.
- **Flicker-Free Child Suspension (`TerminalGuard`):** When launching external commands or child TUIs, use `guard.suspend_keep_alt(...)` for alternate-buffer programs and `guard.suspend(...)` for inline terminal commands. Never clear the screen with raw escape sequences (`\x1b[2J`); use DEC Mode 2026 synchronized output.
- **No Unhandled `.unwrap()` / `.expect()`:** Interactive UI event loops and runtime paths must never crash on user input or parsing failures. Use graceful defaults, fallback values, or display friendly toast/modal messages.

### 2. Layout & Visual Alignment
- **Strict 4-Column Icon Slot Invariant:** Every menu item icon slot must occupy exactly 4 columns (`vis_w + pad_w = 4`). Multi-width emojis (`vis_w = 2`) require 2 spaces padding, and single-width glyphs (`vis_w = 1`) require 3 spaces padding.
- **50/50 Marquee Partition:** Long menu option text exceeding available panel margins must marquee smoothly using the active theme's `marquee.char` and `marquee.gap` settings.
- **Tab Stop Traversal Order:** Multi-control dialogs and forms must traverse in strict top-to-bottom, left-to-right visual order on <kbd>Tab</kbd>, and reverse on <kbd>Shift+Tab</kbd>.

### 3. Universal 6-Tier Documentation Parity
Whenever you add or modify a placeholder, macro, directive, or configuration attribute, you **must** update all 6 documentation tiers to maintain 100% synchronization:
1. `crates/bashmenu-tui`: Placeholders and Macros reference modal.
2. `crates/menuedit-cli`: Inspector guide help modal.
3. `crates/bashedit-cli`: Editor manual modal.
4. `crates/bmpager-cli`: Standalone pager help modal.
5. `bashmenu.md`: Complete reference documentation.
6. `bashmenu.1`: Linux system manual page.

### 4. Zero Hardcoded User-Facing Text Invariant
All user-facing strings (button labels, modal titles, prompts, error notices, status gutters, inspector labels) must strictly resolve through the string table (`locales/en_us.yml`) via `t!(...)`. No raw user-facing string literals are permitted in TUI rendering or CLI message dispatch paths. This ensures all text can be translated into other languages seamlessly via a single locale file.

---

## Shell Script Guidelines

Shell scripts in the root directory or `scripts/` must:
- Include a descriptive script header:
  ```bash
  #!/usr/bin/env bash
  # ==============================================================================
  # SCRIPT: <script_name>.sh
  # DESCRIPTION: <Brief summary>
  # ==============================================================================
  ```
- Pass `shellcheck` with zero warnings.
- Avoid hardcoded user directories (use dynamic variables or macros like `{home}`, `{bashmenu_dir}`, `{scripts_dir}`).

---

## Translating BashMenu (Localization)

We welcome community translations! Contributing a new language requires zero Rust coding experience. For complete step-by-step instructions and FAQ, see the dedicated [Translation Quick-Start Guide](TRANSLATIONS.md).

Quick summary:
1. **Copy the Canonical Template:** Copy [`locales/en_us.yml`](locales/en_us.yml) to `locales/<lang_code>.yml` (e.g. `locales/fr_fr.yml`, `locales/de_de.yml`, `locales/ja_jp.yml`).
2. **Translate the Strings:** Translate the right-hand text values. Do **not** alter keys or dynamic placeholder variables (such as `{theme}`, `{line}`, `{file}`, `{title}`).
3. **Validate Your Translation:** Use `ymlcheck` with the `-l` / `--locale` flag to automatically verify syntax, 100% key completeness, and placeholder variable parity:
   ```bash
   cargo run -p ymlcheck-cli -- -l locales/<lang_code>.yml
   ```
4. **Submit a Pull Request:** Once `ymlcheck` confirms 100% key parity, open a PR with your translation file!

---

## Submitting Pull Requests

1. **Fork & Branch:** Create a feature or bugfix branch with a descriptive name (e.g. `feat/new-theme`, `fix/pager-osc8-wrap`).
2. **Verify Locally:** Run `make verify` before submitting. Pull requests must pass `cargo fmt`, `cargo clippy`, all workspace tests, and `shellcheck`.
3. **Write Clear Commit Messages:** Use concise, conventional commit prefixes (e.g., `feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
4. **Interactive Testing:** Test your changes in a real terminal emulator (e.g., Alacritty, Kitty, WezTerm, tmux) to verify visual rendering and keyboard navigation.
5. **Open a PR:** Fill out the [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md) and link any relevant issues.

Thank you for helping make BashMenu-RS better!
