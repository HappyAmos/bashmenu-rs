# Accessibility Statement

BashMenu-RS aims to be usable across a wide range of terminals, visual preferences, and input workflows.

## Built-In Accessibility Features

- **Full Keyboard-Only Navigation:** Every menu, modal dialog, editor (`bashedit`, `menuedit`, `themeedit`, `shortcutedit`), and pager (`bmpager`) can be operated entirely via keyboard without requiring a mouse. Dialogs follow predictable top-to-bottom, left-to-right `Tab` / `Shift+Tab` focus traversal and support `Esc` to cancel or dismiss.
- **Customizable Keyboard Shortcuts:** Global and menu shortcuts can be remapped via `shortcuts.yml` or the interactive `shortcutedit` tool.
- **Color Depth & High-Contrast Support:** Themes automatically adapt across TrueColor, 256-color, 16-color, and 8-color terminals, and include monochrome/high-contrast options. All colors can be customized per view using `themeedit`.
- **Plain-ASCII Icon Fallbacks:** Setting `settings.use_nerd_fonts: false` in `bashmenu.yml` replaces Nerd Font and emoji glyphs with plain ASCII fallback characters via adaptive `{nf:<char>:<hex>:<emoji>}` resolution.
- **Reduced Motion Controls:** Screen transitions (`settings.transition`) and modal animations (`settings.modal_animation`) can be disabled (`none`), and text marquee scrolling speed (`settings.marquee_speed`) is configurable.
- **No-Color Output:** `bmpager` supports `-c` / `--no-color` to strip ANSI color escape sequences when reading documentation or Markdown files.
- **Localization:** Interface strings support multiple locales (`locales/*.yml`) and respect `LANG` / `LC_ALL` or `--locale`.

## Known Limitations

- **Screen Readers:** Because BashMenu-RS renders a full-screen alternate-buffer TUI using `ratatui`, terminal screen readers (such as Orca or NVDA in terminal emulators) may experience reduced clarity compared to line-oriented CLI tools. Non-interactive CLI subcommands (`ymlcheck`, `cheat`, `bmpager --no-color`) provide plain-text output where applicable.
- **Framebuffer Image Mode:** Linux framebuffer wallpaper rendering (`/dev/fb0`) is purely decorative and optional; disabling wallpapers or running in standard text mode preserves full functionality.

## Reporting Accessibility Barriers

If you encounter an accessibility barrier or have suggestions to improve keyboard navigation, contrast, screen-reader friendliness, or documentation clarity, please [open an issue](https://github.com/HappyAmos/bashmenu-rs/issues/new/choose) and mention **accessibility** in the title or description.
