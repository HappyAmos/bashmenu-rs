# BashMenu-RS

> Modern, lightning-fast terminal menu system and companion developer tools ported to **Rust + Bash scripting** using **Ratatui** and **Crossterm**.

[![Rust Application](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml)
[![ShellCheck](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## Features

- **Blazing Fast Startup & Execution**: Sub-10ms startup, instant mouse/keyboard response, and zero garbage collection pauses.
- **Pure Terminal TUI**: Built with [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm) using double-buffered cell diffing and DEC Mode 2026 synchronized output.
- **Dynamic 6-Tier Macro Engine**: Evaluates `{env:...}`, runtime paths (`{home}`, `{bashmenu_dir}`, `{dev_dir}`, `{scripts_dir}`, `{notebooks_dir}`), time badges, and dot-notation config keys (`{user.<key>}`).
- **Interactive Directives**: Supports `{command:<cmd>}` stdout caching, `{param}` / `{prompt}` modal inputs, `{file_picker}` / `{dir_picker}`, and `{toast:...}` notifications.
- **Adaptive Glyph Resolver**: `{nf:[char]:[nerd]:[emoji]}` resolves terminal icons based on your font configuration while maintaining a strict 4-column visual icon slot.
- **Background Plugin Engine**: Non-blocking asynchronous worker threads executing periodic shell scripts with multi-column table layouts and sequential list formatting.
- **TrueColor Wallpaper Compositor**: Half-block (`▀`) ANSI background rendering with dynamic brightness dimming and cell caching.
- **Global Leader Chords & Which-Key Overlay**: Configurable `<leader>` key (e.g. `<Ctrl+Space>`) with interactive popup discovery and custom bindings in `shortcuts.yml`.
- **Interactive Companion Tool Suite**:
  - `bashmenu` (`bm`): Main interactive menu system with submenu stacks, screen savers, and clean raw-mode process suspension.
  - `menuedit`: Visual menu hierarchy tree editor with add/edit/delete/move actions and type mutation.
  - `shortcutedit`: Visual keyboard shortcuts and leader chord editor.
  - `themeedit`: Visual theme & palette editor with 2D 256-color picker, CP437 ASCII picker, and split-screen live preview.
  - `bashedit`: Full-screen terminal text editor with colocated `.<filename>.swp` swap files, crash recovery prompts, and universal clipboard.
  - `bmpager`: Standalone terminal pager with CommonMark markdown parsing, fenced code block framing, OSC 8 clickable hyperlinks, and vim navigation.
  - `ymlcheck`: Standalone YAML schema validator and configuration linter.

---

## Installation

### Prerequisites
- Linux, macOS, FreeBSD, or WSL.
- Rust toolchain (1.80+ recommended): [https://rustup.rs](https://rustup.rs)

### Quick Install
```bash
git clone https://github.com/HappyAmos/bashmenu-rs.git
cd bashmenu-rs
./install.sh
```

The installer builds all binaries in release mode and installs them to `$HOME/.local/bin` (or `/usr/local/bin` if running as root).

---

## Quick Start

Launch the main menu:
```bash
bashmenu
# or using the quick shortcut:
bm
```

Launch companion tools directly:
```bash
# Validate configuration and theme files
ymlcheck --config bashmenu.yml --themes bashmenu.themes --menu bashmenu.mnu --shortcuts shortcuts.yml

# Edit menu hierarchy
menuedit bashmenu.mnu

# Edit keyboard shortcuts and leader chords
shortcutedit shortcuts.yml

# Edit themes and palettes with split-screen live preview
themeedit dracula

# Edit text files with crash recovery
bashedit script.sh

# Page markdown documents or files with clickable hyperlinks
bmpager --markdown README.md
```

---

## Architecture

The project is structured as a multi-crate Cargo workspace:

```
crates/
├── bashmenu-core/    # Shared data models (Serde), macro engine, glyphs, plugin runner, notebooks
├── bashmenu-tui/     # Reusable Ratatui widgets, custom borders, drop shadows, modals, wallpaper
├── bashmenu-cli/     # Main 'bashmenu' binary
├── ymlcheck-cli/     # Standalone 'ymlcheck' validator binary
├── menuedit-cli/     # Visual 'menuedit' and 'shortcutedit' binaries
├── themeedit-cli/    # Visual 'themeedit' palette editor binary
├── bashedit-cli/     # Terminal 'bashedit' text editor binary
└── bmpager-cli/      # Native terminal 'bmpager' markdown pager binary
```

For full architectural details, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Development & Testing

Run all unit and integration tests:
```bash
cargo test --workspace
```

Run linters and formatting checks:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
shellcheck *.sh scripts/*.sh
```
