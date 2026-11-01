//! Interactive scrollable Help & Manual modal for bmpager.

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::io::{stdout, Write};

const HELP_LINES: &[&str] = &[
    "BASHMENU PAGER - HELP & MANUAL",
    "",
    "USAGE:",
    "  command | bmpager [options]",
    "  bmpager [options] <file> [file...]",
    "",
    "DESCRIPTION:",
    "  Interactive full-screen text pager for terminal output and files.",
    "  Supports ANSI color preservation, line truncation, piped streams,",
    "  interactive hyperlinks, and seamless external editor integration.",
    "",
    "KEYBOARD SHORTCUTS:",
    "",
    "  Navigation & Scrolling",
    "    j, DOWN, ENTER   Scroll down one line",
    "    k, UP            Scroll up one line",
    "    SPACE, f, PGDN   Scroll down one full page (window)",
    "    b, PGUP          Scroll up one full page (window)",
    "    Ctrl-D           Scroll down half a page",
    "    Ctrl-U           Scroll up half a page",
    "    g, HOME          Jump to top of document (line 1)",
    "    G, END           Jump to bottom of document",
    "",
    "  Searching",
    "    /                Search forward (pattern or regular expression)",
    "    n                Repeat search forward for next match",
    "    N                Repeat search backward for previous match",
    "",
    "  Hyperlinks & Anchors",
    "    l, o             Open interactive link selector menu:",
    "                     - OSC 8 terminal hyperlinks",
    "                     - Markdown [label](url) links",
    "                     - Plain web URLs (http:// or https://)",
    "                     - Internal section anchors (#anchor)",
    "",
    "  Editor Integration",
    "    e, E             Open file in editor (default system $EDITOR or",
    "                     BM built-in editor); reloads automatically",
    "",
    "  Help & Exit",
    "    h, ?, F1         Open / close this scrolling help modal",
    "    q, Q, ESC        Close help modal / exit pager",
    "",
    "COMMAND LINE OPTIONS:",
    "  -h, --help         Print CLI help screen and exit",
    "  -V, --version      Display pager version",
    "  -m, --markdown     Render input formatted as Markdown",
    "  +N                 Start viewing at line number N",
    "  +/PATTERN          Start viewing at first line matching PATTERN",
    "  -c, --no-color     Strip all ANSI escape codes from display",
    "  --no-help          Omit shortcut hints from the command bar",
    "  --no-file          Disable editor launch and filename badge",
    "  --editor=MODE      Editor: 'default' ($EDITOR) or 'builtin' (BashEdit)",
    "  --max-lines=N      Maximum lines to buffer (default: 1,000,000)",
    "  --file=PATH        Target file to associate with piped input",
    "  --reload-cmd=CMD   Command to refresh spool after editing",
];

/// Displays the interactive scrollable help modal.
pub fn show_help_modal(strip_colors: bool) -> anyhow::Result<()> {
    let mut scroll_top: usize = 0;
    let total_lines = HELP_LINES.len();

    loop {
        let (cols_u16, rows_u16) = crossterm::terminal::size().unwrap_or((80, 24));
        let cols = (cols_u16 as usize).max(20);
        let rows = (rows_u16 as usize).max(8);

        let box_w = (cols.saturating_sub(6)).clamp(40.min(cols), 74);
        let box_h = (rows.saturating_sub(4)).clamp(8.min(rows), 22);

        let start_r = ((rows.saturating_sub(box_h)) / 2) + 1;
        let start_c = ((cols.saturating_sub(box_w)) / 2) + 1;

        let inner_w = (box_w.saturating_sub(6)).max(10);
        let inner_h = (box_h.saturating_sub(2)).max(2);

        let max_scroll = total_lines.saturating_sub(inner_h);
        if scroll_top > max_scroll {
            scroll_top = max_scroll;
        }

        let (b_style, t_style, s_style, r_style) = if strip_colors {
            ("", "", "", "")
        } else {
            ("\x1b[1;36m", "\x1b[1;37;44m", "\x1b[1;33m", "\x1b[0m")
        };

        let mut out = String::new();

        // Top border
        let title = " Pager Help & Manual ";
        let t_len = title.len();
        let r_len = box_w.saturating_sub(t_len + 3);
        let top_bar = "─".repeat(r_len);
        out.push_str(&format!(
            "\x1b[{};{}H{}┌─{}{}{}{}{}{}┐{}",
            start_r, start_c, b_style, t_style, title, r_style, b_style, top_bar, b_style, r_style
        ));

        // Content rows
        for r_idx in 0..inner_h {
            let line_idx = scroll_top + r_idx;
            let line_content = if line_idx < total_lines {
                HELP_LINES[line_idx]
            } else {
                ""
            };

            let is_heading = (line_content.ends_with(':')
                && line_content
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == ' ' || c == ':' || c == '&'))
                || line_idx == 0;
            let l_style = if is_heading { s_style } else { r_style };

            let vis = crate::ansi::visual_width(line_content);
            let clipped: String = line_content.chars().take(inner_w).collect();
            let pad = inner_w.saturating_sub(vis);

            let cur_r = start_r + 1 + r_idx;
            out.push_str(&format!(
                "\x1b[{};{}H{}│  {}{}{}{}  │{}",
                cur_r,
                start_c,
                b_style,
                l_style,
                clipped,
                " ".repeat(pad),
                b_style,
                r_style
            ));
        }

        // Bottom border
        let pct_str = if max_scroll == 0 {
            "All".to_string()
        } else if scroll_top == 0 {
            "Top".to_string()
        } else if scroll_top >= max_scroll {
            "Bot".to_string()
        } else {
            let pct = ((scroll_top + inner_h) * 100) / total_lines;
            format!("{}%", pct)
        };

        let hint = format!(" [▲▼/PgUp/PgDn: scroll • q/ESC: close] ({}) ", pct_str);
        let h_len = hint.len();
        let b_len = box_w.saturating_sub(h_len + 3);
        let bot_bar = "─".repeat(b_len);
        let bot_r = start_r + box_h - 1;

        out.push_str(&format!(
            "\x1b[{};{}H{}└─{}{}{}{}{}{}┘{}",
            bot_r, start_c, b_style, r_style, b_style, hint, b_style, bot_bar, b_style, r_style
        ));

        print!("{}", out);
        stdout().flush()?;

        // Wait for key event
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => {
                    if scroll_top < max_scroll {
                        scroll_top += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    scroll_top = scroll_top.saturating_sub(1);
                }
                KeyCode::Char(' ') | KeyCode::Char('f') | KeyCode::PageDown => {
                    scroll_top = (scroll_top + inner_h).min(max_scroll);
                }
                KeyCode::Char('b') | KeyCode::PageUp => {
                    scroll_top = scroll_top.saturating_sub(inner_h);
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_top = (scroll_top + (inner_h / 2)).min(max_scroll);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll_top = scroll_top.saturating_sub(inner_h / 2);
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    scroll_top = 0;
                }
                KeyCode::Char('G') | KeyCode::End => {
                    scroll_top = max_scroll;
                }
                KeyCode::Char('q')
                | KeyCode::Char('Q')
                | KeyCode::Esc
                | KeyCode::Char('h')
                | KeyCode::Char('?')
                | KeyCode::F(1) => {
                    break;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
