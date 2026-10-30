# BashMenu-RS

> Modern, lightning-fast terminal menu system and companion developer tools ported to **Rust + Bash scripting** using **Ratatui** and **Crossterm**.

[![Rust Application](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml/badge.svg)](https://github.com/HappyAmos/bashmenu-rs/actions/workflows/rust-app.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## Features

- **Blazing Fast Startup & Execution**: Zero Python runtime overhead, zero garbage collection pauses, and sub-10ms startup.
- **Pure Terminal TUI**: Built on [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm) with double-buffered cell rendering.
- **Dynamic 6-Tier Macro Engine**: Evaluates `{env:...}`, runtime paths (`{bashmenu_dir}`, `{scripts_dir}`, `{cache_dir}`), time badges, and dot-notation config keys (`{user.<key>}`).
- **Adaptive Glyph Resolver**: `{nf:[char]:[nerd]:[emoji]}` with strict 4-column visual icon alignment.
- **Background Plugin Engine**: Non-blocking asynchronous worker threads executing periodic shell scripts with table and sequential layout formatting.
- **Interactive Companion Tools**:
  - `bashmenu` (`bm`): Main interactive menu system with raw mode suspension for child processes.
  - `menuedit`: Visual menu hierarchy tree editor with add/edit/delete/move actions.
  - `themeedit`: Visual theme & palette editor with 2D 256-color picker, CP437 ASCII picker, and split-screen live preview.
  - `bashedit`: Full-screen terminal text editor with colocated `.<filename>.swp` swap files, crash recovery prompt, and read-only collision detection.
  - `ymlcheck`: Standalone YAML schema validator and linter.

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
# or using the shortcut:
bm
```

Launch companion tools:
```bash
# Validate configuration and theme files
ymlcheck --config bashmenu.yml --themes bashmenu.themes --menu bashmenu.mnu

# Edit menu hierarchy
menuedit bashmenu.mnu

# Edit themes and palettes with live preview
themeedit dracula

# Edit text files with crash recovery
bashedit script.sh
```

---

## Architecture

The project is structured as a multi-crate Cargo workspace:

```
crates/
├── bashmenu-core/    # Core data models (serde), macro engine, glyphs, plugin runner
├── bashmenu-tui/     # Reusable Ratatui widgets, custom borders, drop shadows, modals
├── bashmenu-cli/     # Main 'bashmenu' binary
├── ymlcheck-cli/     # Standalone 'ymlcheck' validator binary
├── menuedit-cli/     # Visual 'menuedit' tree editor binary
├── themeedit-cli/    # Visual 'themeedit' palette editor binary
└── bashedit-cli/     # Terminal 'bashedit' text editor binary
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
shellcheck *.sh
```

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
