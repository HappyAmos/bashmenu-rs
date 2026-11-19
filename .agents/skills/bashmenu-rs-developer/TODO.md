# BashMenu-RS Development Roadmap & Tasks (TODO)

## 1. UI & Configuration Enhancements
- [x] **Marquee Speed Configuration Setting (`Options & Settings -> Bashmenu Settings -> Marquee Speed`):**
  - Add `marquee_speed` configuration setting in `bashmenu-core` (`BashMenuConfig` / `Settings` schema).
  - Support configurable interval/speed (e.g., tick intervals or milliseconds per step: Slow / Normal / Fast / Custom) for title and panel marquee scrolling.
  - Wire into `MenuView` and modal marquee timing logic in `bashmenu-tui`.
  - Add interactive radio/select modal in `bashmenu-cli` under `Options & Settings -> Bashmenu Settings -> Marquee Speed`.
  - Preserve settings persistence in `bashmenu.yml` preserving config comments.
  - Maintain universal 6-tier documentation parity (`bashmenu.md`, `bashmenu.1`, `HelpModal`, etc.).

## 2. Community & Contributor Infrastructure (Localization Direction)
- [ ] **Dedicated "New Language Translation" GitHub Issue Template:**
  - Create `.github/ISSUE_TEMPLATE/translation_request.md` for community members to volunteer for or track progress on a specific language (e.g. French, German, Italian, Portuguese, Japanese).
- [ ] **Translation Quick-Start Guide (`TRANSLATIONS.md`):**
  - Create a dedicated guide detailing how non-programmers can copy `locales/en_us.yml`, translate keys, and test live using `bashmenu --locale <file>`.
  - Document `ymlcheck -l locales/<file>.yml` for self-validation.
- [ ] **Locale Auto-Discovery:**
  - Enhance `bashmenu-core` and `bashmenu-cli` to automatically discover and list available `.yml` locale files in `~/.config/bashmenu/locales/` and `/usr/share/bashmenu/locales/`.
  - Expose a dynamic locale switcher in `Options & Settings -> Language / Locale`.

## 3. CI/CD & Distribution
- [ ] **Automated GitHub Release Binaries Workflow:**
  - Create GitHub Actions workflow (`.github/workflows/release.yml`) to automatically compile, package, and upload release artifacts (x86_64 and aarch64 Linux binaries + man pages + assets) whenever a version tag (`v*.*.*`) is published.
