# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7] - 2026-11-25

### Added
- New theme color tokens across `bashmenu-core`, `bashmenu-tui`, `bashedit`, `shortcutedit`, and `themeedit`: `selected_tab`, `unselected_tab`, `line_numbers`, `modal_yes_button`, `modal_no_button`, `modal_cancel_button`, and `key_color`.
- `<F2>` Theme Picker modal and `[F2] Themes` help-gutter shortcut (with mouse click support) in `themeedit`.
- Scaled wallpaper compositing inside the `6:ThemeEdit` miniature `[ Main Menu ]` preview pane.
- Cross-platform macOS and Windows compilation guards and CI matrix, Dependabot grouping, SHA-pinned GitHub Actions, `ACCESSIBILITY.md`, and automated weekly release workflow.

### Changed
- **Performance (Phases 1–3):**
  - Snapshot-once screen transitions (`TestBackend` offscreen buffer), per-transition `ColorTier` detection (`dim_color_for_tier`), wall-clock deadline pacing (`TransitionPacer`) with a 45 ms adaptive circuit-breaker, and 64 KiB buffered stdout writes.
  - Capped `WallpaperCompositor` cached proxy textures to `640×360` via aspect-preserving `DynamicImage::thumbnail` and shared `raw_image` / `cached_grid` via `Arc`.
  - Replaced 20 Hz idle polling with deadline-driven `event::poll`, gated marquee ticking to actual text overflow, and deduplicated unchanged status-gutter redraws.
- Replaced `[ Save (CTRL+S) ]` with `[ OK ]` / `[ Cancel ]` in `menuedit`'s Edit Properties modal and updated `assets/bashmenu.md` and `assets/bashmenu.1`.
- Updated `README.md` tagline to highlight the cheat sheet and markdown pager.

### Fixed
- Fixed `themeedit` live preview hit-testing, left border click overlap, `Shift+Tab` tab cycling, mode-aware preview flash spans across all 7 tabs, direct tab-bar clicking, active-theme styling of sidebar/preview dropdowns, and bottom-left/bottom-right anchoring of the faux help gutter and status bar in `6:ThemeEdit`.
- Fixed Notebook viewer/editor `##`/`###` heading colors, closed code-block right borders, trailing cursor visibility at right margin, scrollbar padding, and help-gutter T-connector divider alignment.
- Fixed leading title border dash (`─[ Title ]`) to use border color instead of title color.

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

[Unreleased]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.7...HEAD
[0.1.7]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.3...v0.1.5
[0.1.3]: https://github.com/HappyAmos/bashmenu-rs/releases/tag/v0.1.3
