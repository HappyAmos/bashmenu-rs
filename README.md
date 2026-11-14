# BashMenu-RS

> Modern, lightning-fast terminal menu system and companion developer tools in **Rust + Bash scripting** using **Ratatui** and **Crossterm**.

[![Rust Application](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml)
[![ShellCheck](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/shellcheck-app.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## What On Earth Is This?

This started as a Bash menu script to create reproducible installs for my homelab, then a Python version, then a Rust version. This rust version was vibe coded using Google Antigravity. While I throw as much as I can at it, it should be considered to be in an alpha state. In addition to being a script launcher, over time I've incorporated a built-in editor (because why not), a visual theme editor, a keyboard shortcuts editor (default is a <ctrl> + <space> leader key then a shortcut key), visual menu editor, a custom pager that does a decent job of rendering basic Markdown in a terminal, a cheatsheet management tool (list, search, etc.) which uses the built-in pager tool to display Markdown, and a plethora of small utility scripts.

Installation is fairly simple if you already have Rust installed. Just clone this Github repository, then run the install.sh script. The script copies binaries and symlinks to ~/.local/bin/. Configuration files are stored in ~/.config/bashmenu, while cache is installed in ~/.cache/bashmenu. A utility script is included in the repository root, uninstall.sh, that removes binaries, symlinks, and cache from ~/.local/bin, ~/.config/bashmenu, and ~/.cache/bashmenu, respectively. The repository is not removed when uninstalling.

## Features

- **Pure Terminal TUI**
- **Configurable Visual Screen Transitions**
- **Direct Linux Framebuffer Support (`/dev/fb0`)**
- **Automated Color Resolution Fallback**
- **Notebook Concurrent Conflict Resolution & 3-Way Merge**
- **TrueColor Wallpaper Compositor**
- **Macro Engine**
- **Interactive Directives**
- **Adaptive Nerdfont & Glyph Resolver**
- **Background Plugin Engine**
- **Global Leader Chords & Which-Key Overlay**

- **Interactive Companion Tool Suite**:
  - `bashmenu` (`bm`): Main interactive menu system with nested submenu stacks, screensavers, and clean raw-mode process suspension.
  - `menuedit`: Visual menu hierarchy tree editor with add/edit/delete/move actions and type mutation.
  - `shortcutedit`: Visual keyboard shortcuts and leader chord editor.
  - `themeedit`: Visual theme & palette editor with 2D 256-color picker, CP437 ASCII picker, and split-screen live preview.
  - `bashedit`: Full-screen terminal text editor with colocated `.<filename>.swp` swap files, crash recovery prompts, and universal clipboard.
  - `bmpager`: Standalone terminal pager with markdown parsing, fenced code block framing, clickable hyperlinks, soft word wrap and shortcuts to edit.
  - `cheat`: CLI Markdown cheatsheet search engine and interactive viewer.
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
# Validate configuration, theme, menu, and shortcut files
ymlcheck --config bashmenu.yml
ymlcheck --themes bashmenu.themes
ymlcheck --menu bashmenu.mnu
ymlcheck --shortcuts shortcuts.yml

# Edit menu hierarchy
menuedit -f bashmenu.mnu

# Edit keyboard shortcuts and leader chords
shortcutedit -f shortcuts.yml

# Edit themes and palettes with split-screen live preview
themeedit -t dracula

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
