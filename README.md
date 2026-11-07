# BashMenu-RS

> Modern, lightning-fast terminal menu system and companion developer tools in **Rust + Bash scripting** using **Ratatui** and **Crossterm**.

[![Rust Application](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml)
[![ShellCheck](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## Features

- **Blazing Fast Startup & Execution**: Sub-10ms cold start, instantaneous mouse and keyboard interaction, and zero garbage collection pauses.
- **Pure Terminal TUI**: Built with [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm) using double-buffered cell diffing and atomic DEC Mode 2026 synchronized output.
- **Configurable Visual Screen Transitions**: Smooth animated transitions (`fade`, `curtain`, `instant`, `none`) on startup, menu page/submenu navigation, notebook viewing, modal popups, and application switching (with `transition: false` bypass support).
- **Direct Linux Framebuffer Support (`/dev/fb0`)**: Native, self-contained hardware video memory blitting (`mmap` + ioctl) for truecolor graphics directly on Linux kernel consoles without requiring external terminal emulators.
- **Automated Color Resolution Fallback**: Adaptive `ColorTier` detection (`Color8`, `Color16`, `Color256`, `TrueColor`) with Euclidean RGB downsampling to standard 16 and 8 ANSI colors on minimal consoles (`TERM=linux`, VT100, etc.).
- **Notebook Concurrent Conflict Resolution & 3-Way Merge**: Atomic note swap files (`.<stem>.note.<slug>.swp`) and smart 3-way line merge resolution (`[ Merge ]`, `[ Overwrite Disk ]`, `[ Reload Disk ]`, `[ Save As Copy ]`) preventing multi-agent or concurrent editing overwrites.
- **TrueColor Wallpaper Compositor**: Half-block (`▀`) and quarter-block ANSI background rendering with aspect scaling (cover, fit, stretch, center, tile), color blending, clustering, and quantization.
- **Dynamic 6-Tier Macro Engine**: Evaluates `{env:...}`, runtime paths (`{home}`, `{bashmenu_dir}`, `{dev_dir}`, `{scripts_dir}`, `{notebooks_dir}`), time badges, and dot-notation config keys (`{user.<key>}`, `{settings.<key>}`).
- **Interactive Directives**: Supports `{command:<cmd>}` cached stdout, `{param}` / `{prompt}` modal inputs, `{file_picker}` / `{dir_picker}`, and `{toast:...}` notifications.
- **Adaptive Glyph Resolver**: `{nf:[char]:[nerd]:[emoji]}` resolves icons based on font configuration while strictly maintaining a 4-column visual icon slot.
- **Background Plugin Engine**: Non-blocking asynchronous worker threads executing periodic shell scripts with multi-column table layouts and sequential list formatting.
- **Global Leader Chords & Which-Key Overlay**: Configurable `<leader>` key (e.g. `<Ctrl+Space>`) with interactive popup discovery and custom bindings in `shortcuts.yml`.
- **Interactive Companion Tool Suite**:
  - `bashmenu` (`bm`): Main interactive menu system with nested submenu stacks, screensavers, and clean raw-mode process suspension.
  - `menuedit`: Visual menu hierarchy tree editor with add/edit/delete/move actions and type mutation.
  - `shortcutedit`: Visual keyboard shortcuts and leader chord editor.
  - `themeedit`: Visual theme & palette editor with 2D 256-color picker, CP437 ASCII picker, and split-screen live preview.
  - `bashedit`: Full-screen terminal text editor with colocated `.<filename>.swp` swap files, crash recovery prompts, and universal clipboard.
  - `bmpager`: Standalone terminal pager with CommonMark markdown parsing, fenced code block framing, OSC 8 clickable hyperlinks, and vim navigation.
  - `cheat`: Fast CLI Markdown cheatsheet search engine and interactive viewer.
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

The installer builds all workspace binaries in release mode and installs them to `$HOME/.local/bin` (or `/usr/local/bin` if running as root).

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

# Search and view cheatsheets
cheat rust
```

---

## Architecture

The project is structured as a multi-crate Cargo workspace:

```
crates/
├── bashmenu-core/    # Shared data models (Serde), macro engine, glyphs, plugin runner, notebooks
├── bashmenu-tui/     # Reusable Ratatui widgets, custom borders, modals, wallpaper, transitions, fb
├── bashmenu-cli/     # Main 'bashmenu' binary
├── ymlcheck-cli/     # Standalone 'ymlcheck' validator binary
├── menuedit-cli/     # Visual 'menuedit' and 'shortcutedit' binaries
├── themeedit-cli/    # Visual 'themeedit' palette editor binary
├── bashedit-cli/     # Terminal 'bashedit' text editor binary
├── bmpager-cli/      # Native terminal 'bmpager' markdown pager binary
└── cheat-cli/        # Interactive 'cheat' cheatsheet lookup binary
```

For full architectural details, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).  
For the complete user manual and specification, see [bashmenu.md](bashmenu.md).

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
