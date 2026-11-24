# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.6] - 2026-11-24

### Added
- Double-brace escaping (`{{placeholder}}` → `{placeholder}`, `{{{placeholder}}}` → `{value}`) in `bashmenu-core` macro interpolation while preserving nested single-brace macros.
- Tabular Placeholders & Macros help modal with live expanded values (`na` for platform-unavailable variables, `null` for empty values) and horizontal marquee scrolling for overflowing value cells across `bashedit`, `menuedit`, and `shortcutedit`.
- Documentation for YAML frontmatter options (`title`, `width`, `height`) in `{info:<path>}` Markdown modals across help modals, `assets/bashmenu.md`, and `assets/bashmenu.1`.
- `CHANGELOG.md` following Keep a Changelog format.
- `.github/dependabot.yml` for automated weekly Cargo and GitHub Actions dependency updates.

### Changed
- Migrated YAML parsing and serialization from deprecated `serde_yml` to pure-Rust `serde-saphyr` 2.2 (`#![forbid(unsafe_code)]`) with `serde_json` `preserve_order`, removing the vendored `patches/libyml/` directory and resolving `RUSTSEC-2025-0068`.
- Upgraded Rust dependencies (`ratatui` 0.30.2 / `lru` 0.18.5, `crossterm` 0.29.0, `sha2` 0.11.1, `regex` 1.13.2, `clap` 4.6.9, `unicode-width` 0.2.3) and GitHub Actions (`actions/checkout@v7`, `actions/upload-artifact@v7`, `actions/download-artifact@v8`, `softprops/action-gh-release@v3`).
- Hardened CI workflows (`rust-app.yml`, `shellcheck-app.yml`) with explicit `permissions: contents: read`.
- Expanded `.gitignore` to exclude secret/key files (`.env`, `*.pem`, `*.key`) and OS metadata (`.DS_Store`, `Thumbs.db`).

## [0.1.5] - 2026-11-22

### Fixed
- Fixed `libc::Ioctl` constant handling for Linux framebuffer ioctls to support musl targets.

## [0.1.3] - 2026-11-19

### Added
- Initial tagged release of the BashMenu-RS workspace and companion CLI tools (`bashmenu`, `menuedit`, `shortcutedit`, `themeedit`, `bashedit`, `bmpager`, `cheat`, `ymlcheck`).

[Unreleased]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.3...v0.1.5
[0.1.3]: https://github.com/HappyAmos/bashmenu-rs/releases/tag/v0.1.3
