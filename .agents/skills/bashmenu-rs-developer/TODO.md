# BashMenu-RS Development Roadmap & Tasks (TODO)

## 0. High-Priority Bugs
- [x] **[BUG] Strict `--repo` and `--local` Configuration & Notebook Path Isolation:**
  - Fix `MacroContext` and `resolve_notebook_path` so that `--repo` loads configurations and notebooks strictly from the repository directory (`{dev_dir}`) and `--local` strictly from the current working directory (`.`).
  - Ensure `~/.config/bashmenu` is never probed, read, or returned when `--repo` or `--local` is passed.
  - Populate root `notes.yml` as a sample notebook loaded when running `bm --repo`.

## 1. UI & Configuration Enhancements
- [x] **Marquee Speed Configuration Setting (`Options & Settings -> Bashmenu Settings -> Marquee Speed`):**
  - Add `marquee_speed` configuration setting in `bashmenu-core` (`BashMenuConfig` / `Settings` schema).
  - Support configurable interval/speed (e.g., tick intervals or milliseconds per step: Slow / Normal / Fast / Custom) for title and panel marquee scrolling.
  - Wire into `MenuView` and modal marquee timing logic in `bashmenu-tui`.
  - Add interactive radio/select modal in `bashmenu-cli` under `Options & Settings -> Bashmenu Settings -> Marquee Speed`.
  - Preserve settings persistence in `bashmenu.yml` preserving config comments.
  - Maintain universal 6-tier documentation parity (`bashmenu.md`, `bashmenu.1`, `HelpModal`, etc.).

## 2. Community & Contributor Infrastructure (Localization Direction)
- [x] **Dedicated "New Language Translation" GitHub Issue Template:**
  - Create `.github/ISSUE_TEMPLATE/translation_request.yml` for community members to volunteer for or track progress on a specific language (e.g. French, German, Italian, Portuguese, Japanese).
- [x] **Translation Quick-Start Guide (`TRANSLATIONS.md`):**
  - Create a dedicated guide detailing how non-programmers can copy `locales/en_us.yml`, translate keys, and test live using `bashmenu --locale <file>`.
  - Document `ymlcheck -l locales/<file>.yml` for self-validation.
- [x] **Locale Auto-Discovery:**
  - Enhance `bashmenu-core` and `bashmenu-cli` to automatically discover and list available `.yml` locale files in `~/.config/bashmenu/locales/` and `/usr/share/bashmenu/locales/`.
  - Expose a dynamic locale switcher in `Options & Settings -> Language / Locale`.

## 3. CI/CD & Distribution
- [x] **Automated GitHub Release Binaries Workflow:**
  - Create GitHub Actions workflow (`.github/workflows/release.yml`) to automatically compile, package, and upload release artifacts (x86_64 and aarch64 Linux binaries + man pages + assets) whenever a version tag (`v*.*.*`) is published.
