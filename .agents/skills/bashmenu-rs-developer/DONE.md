# BashMenu-RS Completed Tasks (DONE)

## Phase 0: Project Initiation
- [x] Created `bashmenu-rs-developer` skill specification at `.agents/skills/bashmenu-rs-developer/SKILL.md`.
- [x] Initialized phased `TODO.md` roadmap tracking for the Rust port.
- [x] Set up multi-crate Cargo workspace (`bashmenu-core`, `bashmenu-tui`, `bashmenu-cli`, `ymlcheck-cli`).
- [x] Configured `rustfmt.toml` and GitHub Actions CI workflow (`.github/workflows/rust-app.yml`).
- [x] Copied golden test fixtures (`bashmenu.yml`, `bashmenu.mnu`, `bashmenu.themes`) into `tests/fixtures/`.
- [x] Implemented global terminal panic hook in `bashmenu-tui` and verified zero Clippy warnings across the workspace.

## Phase 1: Core Data Models & Serialization (`bashmenu-core`)
- [x] Defined `BashMenuConfig`, `Settings`, and `InactivityTimeout` in `config.rs`.
- [x] Implemented `save_config()` with `CONFIG_HEADER` comment preservation invariant.
- [x] Defined `MenuSchema`, `MenuItem`, and `Submenu` in `menu.rs` with custom label-as-key serialization and normalization.
- [x] Defined `ThemeCollection`, `Theme`, `WindowBorders`, `MarqueeConfig`, and palette structures in `theme.rs`.
- [x] Added unit tests and comprehensive integration tests in `crates/bashmenu-core/tests/schema_fixtures_test.rs` validating 100% compatibility against production `bashmenu.yml`, `bashmenu.mnu`, and `bashmenu.themes`. Verified zero Clippy warnings.
