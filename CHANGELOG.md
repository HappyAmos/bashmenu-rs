# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `CHANGELOG.md` following Keep a Changelog format.
- `.github/dependabot.yml` for automated weekly Cargo and GitHub Actions dependency updates.

### Changed
- Hardened CI workflows (`rust-app.yml`, `shellcheck-app.yml`) with explicit `permissions: contents: read`.
- Expanded `.gitignore` to exclude secret/key files (`.env`, `*.pem`, `*.key`) and OS metadata (`.DS_Store`, `Thumbs.db`).

## [0.1.5] - 2026-11-22

### Fixed
- Fixed `libc::Ioctl` constant handling for Linux framebuffer ioctls to support musl targets.

## [0.1.3] - 2026-11-19

### Added
- Initial tagged release of the BashMenu-RS workspace and companion CLI tools (`bashmenu`, `menuedit`, `shortcutedit`, `themeedit`, `bashedit`, `bmpager`, `cheat`, `ymlcheck`).

[Unreleased]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/HappyAmos/bashmenu-rs/compare/v0.1.3...v0.1.5
[0.1.3]: https://github.com/HappyAmos/bashmenu-rs/releases/tag/v0.1.3
