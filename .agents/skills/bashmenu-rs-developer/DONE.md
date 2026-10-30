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

## Phase 2: Macro & Interpolation Engine (`bashmenu-core`)
- [x] Implemented CP437 ASCII macro translation (`{ascii:NNN}`) and full character table in `glyph.rs`.
- [x] Implemented `{nf:[char]:[nerd]:[emoji]}` adaptive glyph resolver with strict 4-tier precedence.
- [x] Implemented 4-column icon padding invariant (`vis_w + pad_w = 4`) and variation selector normalization.
- [x] Implemented recursive 6-tier macro placeholder interpolation engine in `macro_engine.rs` supporting environment variables, runtime paths (`{bashmenu_dir}`, `{scripts_dir}`, `{cache_dir}`), system time/date badges, and dot notation config paths (`{user.<key>}`, `{settings.<key>}`).
- [x] Added integration test suite `crates/bashmenu-core/tests/macro_engine_test.rs` testing icon column alignment and status gutter expansion. Zero Clippy warnings.

## Phase 3: The `ymlcheck` CLI Tool (`crates/ymlcheck-cli`)
- [x] Ported `ymlcheck.py` to compiled Rust executable `ymlcheck` with `clap` arguments (`--themes`, `--config`, `--menu`, and bare YAML syntax check).
- [x] Configured binary aliases in `crates/ymlcheck-cli/Cargo.toml` and `crates/bashmenu-cli/Cargo.toml` so built executables match canonical names (`ymlcheck` and `bashmenu`).
- [x] Verified successful validation against all production fixtures with exit code 0. Zero Clippy warnings.
