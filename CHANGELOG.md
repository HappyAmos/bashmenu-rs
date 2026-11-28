# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.10] - 2026-11-28

### Added
- feat(bmpager): inline Markdown images, SVG badges, animated GIFs, and raw view toggle (#119)
- feat: add {imarkdown:<path>} alias and info/imarkdown types in menuedit (#117)
- feat: interactive Markdown engine (--imarkdown, forms, buttons, UI-RPC) (#115)
- feat(macro): add title/text/default key-value variants and no-title defaults for {message} and {param} (#113)
- feat(macro): add interactive {message:"..."/'...'/\`...\`} popup modal macro (#111)
- feat(bmpager): add live <w>/<W> soft-wrap toggle and resize reflow (#102)

### Fixed
- fix(tui): apply modal_yes_button and title theme tokens in MessageModal (#112)
- fix(security): download and SHA256-verify bootstrap installer scripts (#110)
- fix(security): guard {command:...} macros in previews, info files, and --local mode (#109)
- fix(security): anchor webopen.sh and resolve_script_path to trusted dirs only (#108)
- fix(security): use O_EXCL 0600 random temp file for bashedit Markdown preview (#107)
- fix(security): validate FRAMEBUFFER char device and remove chmod 0666 fallback (#106)
- fix(security): POSIX shell-quote param/picker substitutions and block recursive macro injection (#105)

### Changed
- docs: add Interactive Markdown guide and example to bashmenu.md and bashmenu.1 (#116)

## [0.1.9] - 2026-11-27

### Added
- feat(bmpager): add live `<w>`/`<W>` soft-wrap toggle and resize reflow (#102)

## [0.1.8] - 2026-11-27

### Added
- feat(cli,i18n): localize --help output across all 8 workspace binaries (#90)
- feat(tui,themeedit): add Derive From Background Image checkbox to ColorPickerModal (#84)
- feat(tui,menuedit): add Show Command option for Streaming/Interactive floating consoles (#83)
- feat(tui): embed vt100 terminal screen, live cursor, and full key forwarding in interactive floating windows (#81)
- feat(tui): allocate POSIX PTY for interactive StreamOutputModal floating windows (#80)
- feat(themeedit): add Apply to Base checkbox in color picker and use Nerd Font preview icons (#79)
- feat(themeedit): fix live chrome toasts/modals/tabs and add focused/unfocused input tokens (#77)
- feat(themeedit): add Input Box Text and Background tokens and 3:Modals faux input (#76)
- feat(themeedit): add Selected Tab Text and Selected Tab Background tokens (#74)
- feat(i18n): move multi-line help modal texts into YAML locale files (#73)

### Fixed
- fix(tui): use styles.help_text for ThemePickerModal help footer (#89)
- fix(tui,themeedit): align input_box text defaults (254/244) and ColorPickerModal ANSI index mapping (#87)
- fix(themeedit): preserve full sidebar color codes and fix input_box/selected_tab display (#86)
- fix(tui): answer VT100 terminal queries and normalize HVP in interactive PTY (#82)
- fix(themeedit,menuedit): fix 3:Modals text/help colors, tab bar clipping, and suite input boxes (#78)
- fix(notes): theme modal help gutters and add F8 Markdown reference in editor (#71)
- fix(themeedit): align 6:ThemeEdit preview hit-testing and highlight split (#70)
- fix(themeedit): isolate editor chrome theme from preview theme (#69)
- fix: package templates, fix Unicode help wrap, surface save errors, and fix remaining test races (#68)

### Changed
- chore(docs,themes): update README and default themes with ymlcheck token parity (#97)
- chore(defaults): update bashmenu.yml settings and plugin list (#75)
- docs: move ARCHITECTURE.md to repository root (#72)
- test: replace tautological/zero-assertion tests and serialize env-mutating tests (#67)
- refactor(cli): deduplicate locale/path/tool helpers and remove runtime unwraps (#66)
- refactor(core,tui): fix regex/backup/unwrap bugs and deduplicate shared modal helpers (#65)
- chore: clean up stale docs, manuals, defaults, and shell script slop (#64)

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

[Unreleased]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.10...HEAD
[0.1.10]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.3...v0.1.5
[0.1.3]: https://github.com/HappyAmos/bashmenu-rs/releases/tag/v0.1.3
