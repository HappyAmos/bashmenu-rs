//! Core pager implementation, rendering loop, and interactive controller.

use crate::ansi::clip_ansi_line;
use crate::args::PagerArgs;
use crate::editor::open_editor;
use crate::help::show_help_modal;
use crate::links::{extract_links, open_external_url, resolve_anchor};
use bashmenu_core::t;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Interactive terminal pager state machine.
pub struct Pager {
    /// Document lines with formatting codes intact
    pub lines: Vec<String>,
    /// Original raw text content retained for lossless wrap toggling and resize reflow
    pub raw_content: String,
    /// Multi-frame animated GIFs anchored to line offsets in `lines`
    pub animations: Vec<crate::markdown::AnimatedImage>,
    /// 1-based index of the line currently at the top of the viewport
    pub top_line: usize,
    /// Total count of document lines
    pub total: usize,
    /// Command-line configuration and runtime flags
    pub args: PagerArgs,
    /// Target file path being paged, if backed by disk
    pub target_file: Option<PathBuf>,
    /// Last search query string
    pub last_search: String,
    /// Status bar notification message
    pub message: String,
    /// Terminal column width
    pub cols: usize,
    /// Terminal row height
    pub rows: usize,
    /// Number of document lines visible in a single screen page
    pub page_rows: usize,
}

/// RAII guard ensuring terminal raw mode and alternate screen are restored
/// on drop.
struct PagerTerminalGuard {
    keep_alt: bool,
}

impl Drop for PagerTerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        if !self.keep_alt {
            let _ = execute!(out, Show, LeaveAlternateScreen);
        }
        let _ = disable_raw_mode();
        let _ = out.flush();
        #[cfg(unix)]
        bashmenu_tui::ensure_terminal_sane();
    }
}

impl Pager {
    /// Initializes a new interactive pager with document lines and configuration arguments.
    pub fn new(lines: Vec<String>, args: PagerArgs) -> Self {
        let total = lines.len();
        let target_file = if !args.files.is_empty() {
            Some(args.files[0].clone())
        } else {
            args.associated_file.clone()
        };

        let raw_content = lines.join("\n");
        let mut pager = Self {
            lines,
            raw_content,
            animations: Vec::new(),
            top_line: 1,
            total,
            args,
            target_file,
            last_search: String::new(),
            message: String::new(),
            cols: 80,
            rows: 24,
            page_rows: 23,
        };

        pager.resolve_start();
        pager.clamp_top();
        pager
    }

    /// Attaches the original unformatted raw content so toggling wrap (`w`) or
    /// resizing the terminal reflows text losslessly.
    pub fn with_raw_content(mut self, raw_content: String) -> Self {
        self.raw_content = raw_content;
        self
    }

    /// Attaches decoded animated GIF metadata for viewport-aware playback.
    pub fn with_animations(mut self, animations: Vec<crate::markdown::AnimatedImage>) -> Self {
        self.animations = animations;
        self
    }

    /// Returns the shortest wait duration until the next visible animated GIF
    /// frame should advance, or `None` if no animations are currently visible.
    fn next_animation_timeout(&self) -> Option<Duration> {
        if self.animations.is_empty() {
            return None;
        }
        let view_start = self.top_line.saturating_sub(1);
        let view_end = view_start + self.page_rows;
        let now = Instant::now();
        let mut earliest: Option<Duration> = None;
        for anim in &self.animations {
            let h = anim.frames.first().map(|(l, _)| l.len()).unwrap_or(0);
            if h == 0 {
                continue;
            }
            let anim_end = anim.start_line + h;
            if anim.start_line < view_end && anim_end > view_start {
                let wait = anim.next_tick.saturating_duration_since(now);
                earliest = Some(match earliest {
                    Some(cur) => cur.min(wait),
                    None => wait,
                });
            }
        }
        earliest
    }

    /// Advances any visible animated GIF whose frame deadline has elapsed,
    /// swapping its pre-rendered lines in place. Returns `true` if any frame changed.
    pub fn tick_animations(&mut self) -> bool {
        if self.animations.is_empty() {
            return false;
        }
        let view_start = self.top_line.saturating_sub(1);
        let view_end = view_start + self.page_rows;
        let now = Instant::now();
        let mut changed = false;

        for anim in &mut self.animations {
            if anim.frames.len() <= 1 {
                continue;
            }
            let h = anim.frames[0].0.len();
            let anim_end = anim.start_line + h;
            if anim.start_line < view_end && anim_end > view_start && now >= anim.next_tick {
                anim.current_frame = (anim.current_frame + 1) % anim.frames.len();
                let (ref frame_lines, delay) = anim.frames[anim.current_frame];
                for (offset, line) in frame_lines.iter().enumerate() {
                    if let Some(slot) = self.lines.get_mut(anim.start_line + offset) {
                        *slot = line.clone();
                    }
                }
                anim.next_tick = now + delay;
                changed = true;
            }
        }
        changed
    }

    /// Rebuilds display `lines` from `raw_content` using the current wrap/markdown
    /// mode and terminal width, clamping `top_line` safely.
    pub fn rebuild_lines(&mut self) {
        let width = self.cols.clamp(20, 500);
        let md_base_dir = self
            .args
            .associated_file
            .as_ref()
            .or_else(|| self.args.files.first())
            .and_then(|p| p.parent())
            .map(std::path::Path::to_path_buf);
        let mut rebuilt: Vec<String> = if self.args.markdown {
            let rendered = crate::markdown::render_markdown_full(
                &self.raw_content,
                width,
                md_base_dir.as_deref(),
                !self.args.strip_colors,
                self.args.remote_images,
            );
            self.animations = rendered.animations;
            rendered.text.lines().map(|s| s.to_string()).collect()
        } else if self.args.wrap_text {
            self.animations.clear();
            crate::ansi::wrap_text_content(&self.raw_content, width)
        } else {
            self.animations.clear();
            self.raw_content.lines().map(|s| s.to_string()).collect()
        };
        if rebuilt.len() > self.args.max_lines {
            rebuilt.truncate(self.args.max_lines);
        }
        self.lines = rebuilt;
        self.total = self.lines.len();
        self.clamp_top();
    }

    /// Toggles soft line wrapping (`w` / `W`) for plain and ANSI text documents.
    pub fn toggle_wrap(&mut self) {
        if self.args.markdown {
            self.message = t!("bmpager.wrap_markdown_always");
            return;
        }
        self.args.wrap_text = !self.args.wrap_text;
        self.rebuild_lines();
        self.message = if self.args.wrap_text {
            t!("bmpager.wrap_on")
        } else {
            t!("bmpager.wrap_off")
        };
    }

    /// Resolves starting line position from CLI arguments (`+N` or `+/PATTERN`).
    fn resolve_start(&mut self) {
        if let Some(line) = self.args.start_line {
            self.top_line = line.max(1);
        } else if let Some(ref pat) = self.args.start_pattern {
            let pat_lower = pat.to_lowercase();
            let mut found = None;
            for (idx, line) in self.lines.iter().enumerate() {
                if crate::ansi::strip_ansi(line)
                    .to_lowercase()
                    .contains(&pat_lower)
                {
                    found = Some(idx + 1);
                    break;
                }
            }
            if let Some(line) = found {
                self.top_line = line;
            } else {
                self.message = t!("bmpager.pattern_not_found", pattern => pat);
            }
        }
    }

    /// Clamps top visible line index within valid document bounds (1..=total).
    pub fn clamp_top(&mut self) {
        let max = self.total.max(1);
        if self.top_line < 1 {
            self.top_line = 1;
        }
        if self.top_line > max {
            self.top_line = max;
        }
    }

    /// Queries terminal column and row dimensions via crossterm.
    fn query_size(&mut self) {
        let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
        self.cols = (w as usize).max(20);
        self.rows = (h as usize).max(3);
        self.page_rows = self.rows.saturating_sub(1).max(1);
    }

    /// Runs the interactive pager event loop until the user exits.
    pub fn run(&mut self) -> anyhow::Result<()> {
        let keep_alt = std::env::var("BASHMENU_CHILD_ALT").as_deref() == Ok("1");
        enable_raw_mode()?;
        let mut out = stdout();
        if keep_alt {
            execute!(out, Hide)?;
        } else {
            execute!(out, EnterAlternateScreen, Hide)?;
        }
        out.flush()?;
        let _guard = PagerTerminalGuard { keep_alt };

        self.query_size();
        self.clamp_top();

        let mut done = false;
        while !done {
            self.query_size();
            self.clamp_top();
            self.render()?;

            if let Some(wait) = self.next_animation_timeout() {
                if !event::poll(wait)? {
                    self.tick_animations();
                    continue;
                }
            }

            match event::read()? {
                Event::Key(key) => {
                    done = self.handle_key(key)?;
                }
                Event::Resize(_, _) => {
                    self.query_size();
                    if self.args.wrap_text || self.args.markdown {
                        self.rebuild_lines();
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Processes keyboard shortcuts, cursor navigation, and search queries. Returns `true` to exit.
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<bool> {
        let mut clear_message = true;

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(true);
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => {
                self.top_line += 1;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.top_line = self.top_line.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::Char('f') | KeyCode::PageDown => {
                self.top_line += self.page_rows;
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                self.top_line = self.top_line.saturating_sub(self.page_rows);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.top_line += (self.page_rows / 2).max(1);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.top_line = self.top_line.saturating_sub((self.page_rows / 2).max(1));
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.top_line = 1;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.top_line = self.total;
            }
            KeyCode::Char('/') => {
                self.prompt_search()?;
                clear_message = false;
            }
            KeyCode::Char('n') => {
                self.search_next(1);
                clear_message = false;
            }
            KeyCode::Char('N') => {
                self.search_next(-1);
                clear_message = false;
            }
            KeyCode::Char('l') | KeyCode::Char('o') => {
                self.choose_link()?;
                clear_message = false;
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.open_in_editor()?;
                clear_message = false;
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                self.toggle_wrap();
                clear_message = false;
            }
            KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::F(1) => {
                show_help_modal(self.args.strip_colors)?;
            }
            _ => {
                let key_str = format!("{:?}", key.code);
                self.message = t!("bmpager.unbound_key", key => &key_str);
                clear_message = false;
            }
        }

        if clear_message {
            self.message.clear();
        }

        self.clamp_top();
        Ok(false)
    }

    /// Formats the bottom command and status bar string.
    fn build_bar(&self, last_shown: usize) -> String {
        let label = if self.total == 0 {
            "EMPTY".to_string()
        } else if self.top_line <= 1 {
            "TOP".to_string()
        } else if self.top_line >= self.total {
            "BOT".to_string()
        } else {
            let pct = ((self.top_line * 100 + self.total / 2) / self.total).min(100);
            format!("{:>3}%", pct)
        };

        let left = if self.total > 0 {
            format!("{}  {}-{}/{}", label, self.top_line, last_shown, self.total)
        } else {
            label
        };

        let badge = if self.args.allow_file {
            let fname = if !self.args.files.is_empty() {
                let name = self.args.files[0]
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                let extra = if self.args.files.len() > 1 {
                    format!(" (+{})", self.args.files.len() - 1)
                } else {
                    String::new()
                };
                format!("{}{}", name, extra)
            } else if let Some(ref orig) = self.args.associated_file {
                orig.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };

            if !fname.is_empty() {
                let max_fname = if self.cols < 40 {
                    8
                } else if self.cols < 60 {
                    14
                } else if self.cols < 80 {
                    20
                } else {
                    28
                };

                let display_fname = if fname.len() > max_fname {
                    format!("{}...", &fname[..max_fname.saturating_sub(3)])
                } else {
                    fname
                };
                format!("[ {} ]", display_fname)
            } else {
                "[ <stdin> ]".to_string()
            }
        } else {
            String::new()
        };

        let center_start = if !badge.is_empty() {
            (self.cols.saturating_sub(badge.len())) / 2
        } else {
            0
        };
        let center_end = center_start + badge.len();
        let avail_right = if !badge.is_empty() {
            self.cols.saturating_sub(center_end + 1)
        } else {
            self.cols.saturating_sub(left.len() + 2)
        };

        let mut right = String::new();
        if !self.message.is_empty() {
            right = if self.message.len() > avail_right && avail_right > 4 {
                format!("{}...", &self.message[..avail_right - 3])
            } else {
                self.message.clone()
            };
        } else if self.args.show_help_hints {
            let full_hints = if self.args.allow_file {
                "j/k line  SPACE pgDn  b pgUp  / search  l link  e edit  h/? help  q quit"
            } else {
                "j/k line  SPACE pgDn  b pgUp  / search  l link  h/? help  q quit"
            };

            if avail_right >= 62 {
                right = full_hints.to_string();
            } else if avail_right >= 42 {
                right = "SPACE pgDn  b pgUp  / find  q quit".to_string();
            } else if avail_right >= 24 {
                right = "SPACE pgDn  / find  q".to_string();
            } else if avail_right >= 14 {
                right = "h help  q quit".to_string();
            } else if avail_right >= 6 {
                right = "q quit".to_string();
            }
        }

        let mut bar = if !badge.is_empty() {
            if center_start > left.len() {
                let pad_left = center_start - left.len();
                let pad_right = self.cols.saturating_sub(center_end + right.len());
                format!(
                    "{}{}{}{}{}",
                    left,
                    " ".repeat(pad_left),
                    badge,
                    " ".repeat(pad_right),
                    right
                )
            } else {
                let prefix = format!("{} {}", left, badge);
                if prefix.len() < self.cols {
                    if !right.is_empty() && prefix.len() + 1 + right.len() <= self.cols {
                        let pad = self.cols - prefix.len() - right.len();
                        format!("{}{}{}", prefix, " ".repeat(pad), right)
                    } else {
                        let pad = self.cols - prefix.len();
                        format!("{}{}", prefix, " ".repeat(pad))
                    }
                } else {
                    prefix.chars().take(self.cols).collect()
                }
            }
        } else if !right.is_empty() && left.len() + 1 + right.len() <= self.cols {
            let pad = self.cols - left.len() - right.len();
            format!("{}{}{}", left, " ".repeat(pad), right)
        } else {
            left.chars().take(self.cols).collect()
        };

        if bar.len() < self.cols {
            let pad = self.cols - bar.len();
            bar.push_str(&" ".repeat(pad));
        } else if bar.len() > self.cols {
            bar = bar.chars().take(self.cols).collect();
        }

        bar
    }

    /// Redraws visible document lines, trailing tildes, and status bar into stdout.
    fn render(&self) -> anyhow::Result<()> {
        let last_shown = (self.top_line + self.page_rows - 1).min(self.total);
        let bar_line = self.build_bar(last_shown);

        let mut frame = String::with_capacity(self.rows * (self.cols + 16));
        let mut row = 1;

        for line_idx in self.top_line..=last_shown {
            let raw_line = self
                .lines
                .get(line_idx - 1)
                .map(|s| s.as_str())
                .unwrap_or("");
            let clipped = clip_ansi_line(raw_line, self.cols, self.args.strip_colors);
            frame.push_str(&format!("\x1b[{};1H{}\x1b[0m\x1b[K", row, clipped));
            row += 1;
        }

        // Tilde markers past EOF
        while row <= self.page_rows {
            frame.push_str(&format!("\x1b[{};1H~\x1b[0m\x1b[K", row));
            row += 1;
        }

        // Status command bar on the last terminal row
        frame.push_str(&format!(
            "\x1b[{};1H\x1b[7m{}\x1b[0m\x1b[K",
            self.rows, bar_line
        ));

        let mut out = stdout();
        print!("{}", frame);
        out.flush()?;
        Ok(())
    }

    /// Displays interactive forward search query input prompt on the command line.
    fn prompt_search(&mut self) -> anyhow::Result<()> {
        let mut query = String::new();
        let mut out = stdout();

        loop {
            print!("\x1b[{};1H\x1b[7m/{}\x1b[K\x1b[0m", self.rows, query);
            out.flush()?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => break,
                    KeyCode::Esc => return Ok(()),
                    KeyCode::Backspace => {
                        query.pop();
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                    }
                    _ => {}
                }
            }
        }

        if !query.is_empty() {
            self.last_search = query.clone();
            self.search_forward(&query, self.top_line + 1);
        }

        Ok(())
    }

    /// Searches forward from `start_line` for the given text pattern, updating top line on match.
    fn search_forward(&mut self, pat: &str, start_line: usize) {
        let pat_lower = pat.to_lowercase();
        let mut hit = None;

        for (idx, line) in self
            .lines
            .iter()
            .enumerate()
            .skip(start_line.saturating_sub(1))
        {
            let clean = crate::ansi::strip_ansi(line).to_lowercase();
            if clean.contains(&pat_lower) {
                hit = Some(idx + 1);
                break;
            }
        }

        if hit.is_none() && start_line > 1 {
            for (idx, line) in self.lines.iter().enumerate().take(start_line - 1) {
                let clean = crate::ansi::strip_ansi(line).to_lowercase();
                if clean.contains(&pat_lower) {
                    hit = Some(idx + 1);
                    break;
                }
            }
        }

        if let Some(target) = hit {
            self.top_line = target;
            self.message = format!("/{}", pat);
        } else {
            self.message = t!("bmpager.pattern_not_found", pattern => pat);
        }
    }

    /// Repeats the active search query forward (`dir >= 0`) or backward (`dir < 0`),
    /// wrapping around the document boundaries if necessary.
    fn search_next(&mut self, dir: i32) {
        if self.last_search.is_empty() {
            self.message = t!("bmpager.no_previous_search");
            return;
        }

        if dir >= 0 {
            let pat = self.last_search.clone();
            self.search_forward(&pat, self.top_line + 1);
        } else {
            let pat_lower = self.last_search.to_lowercase();
            let mut hit = None;

            if self.top_line > 1 {
                for (idx, line) in self.lines.iter().enumerate().take(self.top_line - 1).rev() {
                    let clean = crate::ansi::strip_ansi(line).to_lowercase();
                    if clean.contains(&pat_lower) {
                        hit = Some(idx + 1);
                        break;
                    }
                }
            }

            if hit.is_none() {
                for (idx, line) in self.lines.iter().enumerate().rev() {
                    let clean = crate::ansi::strip_ansi(line).to_lowercase();
                    if clean.contains(&pat_lower) {
                        hit = Some(idx + 1);
                        break;
                    }
                }
            }

            if let Some(target) = hit {
                self.top_line = target;
                self.message = format!("?{}", self.last_search);
            } else {
                self.message = t!("bmpager.pattern_not_found", pattern => &self.last_search);
            }
        }
    }

    /// Displays an interactive link picker dialog and navigates to or opens the chosen target.
    fn choose_link(&mut self) -> anyhow::Result<()> {
        let links = extract_links(&self.lines);
        if links.is_empty() {
            self.message = "no links found in document".to_string();
            return Ok(());
        }

        let count = links.len();
        let mut sel = 0;
        for (i, link) in links.iter().enumerate() {
            if link.line_no >= self.top_line {
                sel = i;
                break;
            }
        }

        let mut offset = 0;
        let mut out = stdout();

        loop {
            self.query_size();
            let max_display = (self.rows.saturating_sub(4)).max(3);

            if sel < offset {
                offset = sel;
            } else if sel >= offset + max_display {
                offset = sel - max_display + 1;
            }

            let mut modal_frame = String::new();
            modal_frame.push_str("\x1b[2J\x1b[1;1H\x1b[7m── Follow Link (");
            modal_frame.push_str(&format!(
                "{} found, j/k/arrows, 1-9/Enter to select, ESC to cancel) ──\x1b[0m\x1b[K\n",
                count
            ));

            let mut row = 2;
            for (d_idx, link) in links.iter().enumerate().skip(offset).take(max_display) {
                let num = d_idx + 1;
                let mut display_str = format!(" [{}] {} ({})", num, link.text, link.url);
                let max_w = self.cols.saturating_sub(2);
                let char_count = display_str.chars().count();
                if char_count > max_w {
                    let mut clipped: String =
                        display_str.chars().take(max_w.saturating_sub(3)).collect();
                    clipped.push_str("...");
                    display_str = clipped;
                }

                if d_idx == sel {
                    modal_frame.push_str(&format!(
                        "\x1b[{};1H\x1b[1;37;44m>{}\x1b[0m\x1b[K\n",
                        row, display_str
                    ));
                } else {
                    modal_frame
                        .push_str(&format!("\x1b[{};1H  {}\x1b[0m\x1b[K\n", row, display_str));
                }
                row += 1;
            }

            while row < self.rows {
                modal_frame.push_str(&format!("\x1b[{};1H\x1b[K\n", row));
                row += 1;
            }

            modal_frame.push_str(&format!(
                "\x1b[{};1H\x1b[7mSelect link [1-{}] or ENTER on selection (ESC to cancel): \x1b[K\x1b[0m",
                self.rows, count
            ));

            print!("{}", modal_frame);
            out.flush()?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        sel = sel.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if sel + 1 < count {
                            sel += 1;
                        }
                    }
                    KeyCode::Enter => break,
                    KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                        let num = (c as u8 - b'0') as usize;
                        if num <= count {
                            sel = num - 1;
                            break;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                        print!("\x1b[2J");
                        out.flush()?;
                        return Ok(());
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        print!("\x1b[2J");
                        out.flush()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        print!("\x1b[2J");
        out.flush()?;

        let chosen = &links[sel];
        if chosen.url.starts_with('#') {
            if let Some(target_line) =
                resolve_anchor(&self.lines, &chosen.url, &chosen.text, chosen.line_no)
            {
                self.top_line = target_line;
                self.message = t!("bmpager.jumped_to", target => &chosen.url);
            } else {
                self.message = t!("bmpager.anchor_not_found", target => &chosen.url);
            }
        } else if chosen.url.starts_with("http://") || chosen.url.starts_with("https://") {
            let _ = open_external_url(&chosen.url);
            self.message = t!("bmpager.opened", target => &chosen.url);
        } else if let Some(full_file) = crate::links::resolve_local_file(&chosen.url) {
            // Temporarily leave raw mode and alternate screen to launch child viewer
            execute!(stdout(), Show, LeaveAlternateScreen)?;
            disable_raw_mode()?;
            #[cfg(unix)]
            bashmenu_tui::ensure_terminal_sane();

            let mut cmd =
                Command::new(std::env::current_exe().unwrap_or_else(|_| PathBuf::from("bmpager")));
            if full_file.extension().map(|e| e == "md").unwrap_or(false) {
                cmd.arg("-m");
            }
            cmd.arg(&full_file);
            let _ = cmd.status();

            #[cfg(unix)]
            bashmenu_tui::ensure_terminal_sane();
            enable_raw_mode()?;
            execute!(stdout(), EnterAlternateScreen, Hide)?;
            stdout().flush()?;
            self.message = t!("bmpager.viewed", target => &chosen.url);
        } else {
            self.message = t!("bmpager.file_not_found", target => &chosen.url);
        }

        Ok(())
    }

    /// Spawns the configured text editor on the currently displayed file, suspending raw mode.
    fn open_in_editor(&mut self) -> anyhow::Result<()> {
        if !self.args.allow_file {
            self.message = "editing disabled".to_string();
            return Ok(());
        }

        let target_file = match self.target_file {
            Some(ref p) => p.clone(),
            None => {
                self.message = "cannot edit piped input".to_string();
                return Ok(());
            }
        };

        // Temporarily leave raw mode and alternate screen
        execute!(stdout(), Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        #[cfg(unix)]
        bashmenu_tui::ensure_terminal_sane();

        let err_msg = open_editor(
            &target_file,
            self.top_line,
            self.args.editor_mode.as_deref(),
        )?;

        // Re-enter raw mode and alternate screen
        #[cfg(unix)]
        bashmenu_tui::ensure_terminal_sane();
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        stdout().flush()?;

        if let Some(err) = err_msg {
            self.message = err;
        } else {
            // Reload file content
            self.reload_spool(&target_file);
        }

        self.query_size();
        self.clamp_top();
        Ok(())
    }

    /// Reloads the document lines from the file on disk or re-runs the reload command.
    fn reload_spool(&mut self, target_file: &Path) {
        if let Some(ref cmd) = self.args.reload_cmd {
            if let Ok(output) = Command::new("sh").arg("-c").arg(cmd).output() {
                if output.status.success() {
                    self.raw_content = String::from_utf8_lossy(&output.stdout).into_owned();
                    self.rebuild_lines();
                    self.message = "reloaded".to_string();
                    return;
                }
            }
        }

        if let Ok(content) = std::fs::read_to_string(target_file) {
            self.raw_content = content;
            self.rebuild_lines();
            self.message = "reloaded".to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pager_toggle_wrap_key_reflows_and_restores_lines() {
        let raw = "one two three four five six seven eight nine ten eleven twelve".to_string();
        let mut pager = Pager::new(vec![raw.clone()], PagerArgs::default()).with_raw_content(raw);
        pager.cols = 20;
        assert!(!pager.args.wrap_text);
        assert_eq!(pager.total, 1);

        // Press 'w' -> enables soft wrapping and splits long line
        let exit = pager
            .handle_key(crossterm::event::KeyEvent::new(
                KeyCode::Char('w'),
                KeyModifiers::NONE,
            ))
            .unwrap();
        assert!(!exit);
        assert!(pager.args.wrap_text);
        assert!(
            pager.total > 1,
            "Enabling wrap must split 62-col line at width 20 (got {} lines)",
            pager.total
        );
        assert!(!pager.message.is_empty());

        // Press 'w' again -> disables soft wrapping and restores single line
        pager
            .handle_key(crossterm::event::KeyEvent::new(
                KeyCode::Char('w'),
                KeyModifiers::NONE,
            ))
            .unwrap();
        assert!(!pager.args.wrap_text);
        assert_eq!(pager.total, 1);
    }

    #[test]
    fn test_pager_animated_gif_frame_tick() {
        let dir = std::env::temp_dir().join(format!("bmpager_anim_gif_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let gif_path = dir.join("pulse.gif");

        // Build a 2-frame 4x4 animated GIF (frame 0 red, frame 1 blue)
        {
            let file = std::fs::File::create(&gif_path).expect("create gif");
            let mut enc = image::codecs::gif::GifEncoder::new(file);
            let mut f0_img = image::RgbaImage::new(4, 4);
            for px in f0_img.pixels_mut() {
                *px = image::Rgba([255, 0, 0, 255]);
            }
            let mut f1_img = image::RgbaImage::new(4, 4);
            for px in f1_img.pixels_mut() {
                *px = image::Rgba([0, 0, 255, 255]);
            }
            let delay = image::Delay::from_numer_denom_ms(100, 1);
            enc.encode_frame(image::Frame::from_parts(f0_img, 0, 0, delay))
                .expect("encode f0");
            enc.encode_frame(image::Frame::from_parts(f1_img, 0, 0, delay))
                .expect("encode f1");
        }

        let rendered = crate::markdown::render_markdown_full(
            "# Anim\n\n![Pulse](pulse.gif)\n",
            80,
            Some(&dir),
            true,
            false,
        );
        assert_eq!(rendered.animations.len(), 1);
        assert_eq!(rendered.animations[0].frames.len(), 2);

        let lines: Vec<String> = rendered.text.lines().map(String::from).collect();
        let mut pager =
            Pager::new(lines, PagerArgs::default()).with_animations(rendered.animations);
        let start = pager.animations[0].start_line;
        assert!(
            pager.lines[start].contains("38;2;255;0;0"),
            "frame 0 should be red: {}",
            pager.lines[start]
        );

        // Force deadline to now and tick -> frame 1 (blue) swaps in place
        pager.animations[0].next_tick = Instant::now();
        assert!(pager.tick_animations());
        assert_eq!(pager.animations[0].current_frame, 1);
        assert!(
            pager.lines[start].contains("38;2;0;0;255"),
            "frame 1 should be blue: {}",
            pager.lines[start]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
