//! Core pager implementation, rendering loop, and interactive controller.

use crate::ansi::clip_ansi_line;
use crate::args::PagerArgs;
use crate::editor::open_editor;
use crate::help::show_help_modal;
use crate::links::{extract_links, open_external_url, resolve_anchor};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Pager {
    pub lines: Vec<String>,
    pub top_line: usize, // 1-indexed
    pub total: usize,
    pub args: PagerArgs,
    pub target_file: Option<PathBuf>,
    pub last_search: String,
    pub message: String,
    pub cols: usize,
    pub rows: usize,
    pub page_rows: usize,
}

impl Pager {
    pub fn new(lines: Vec<String>, args: PagerArgs) -> Self {
        let total = lines.len();
        let target_file = if !args.files.is_empty() {
            Some(args.files[0].clone())
        } else {
            args.associated_file.clone()
        };

        let mut pager = Self {
            lines,
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
                self.message = format!("pattern not found: {}", pat);
            }
        }
    }

    pub fn clamp_top(&mut self) {
        let max = self.total.max(1);
        if self.top_line < 1 {
            self.top_line = 1;
        }
        if self.top_line > max {
            self.top_line = max;
        }
    }

    fn query_size(&mut self) {
        let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
        self.cols = (w as usize).max(20);
        self.rows = (h as usize).max(3);
        self.page_rows = self.rows.saturating_sub(1).max(1);
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, Hide)?;
        out.flush()?;

        self.query_size();
        self.clamp_top();

        let mut done = false;
        while !done {
            self.query_size();
            self.clamp_top();
            self.render()?;

            if let Event::Key(key) = event::read()? {
                done = self.handle_key(key)?;
            }
        }

        execute!(out, Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        out.flush()?;
        Ok(())
    }

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
            KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::F(1) => {
                show_help_modal(self.args.strip_colors)?;
            }
            _ => {
                self.message = format!("unbound key: {:?}", key.code);
                clear_message = false;
            }
        }

        if clear_message {
            self.message.clear();
        }

        self.clamp_top();
        Ok(false)
    }

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
            self.message = format!("pattern not found: {}", pat);
        }
    }

    fn search_next(&mut self, dir: i32) {
        if self.last_search.is_empty() {
            self.message = "no previous search pattern".to_string();
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
                self.message = format!("pattern not found: {}", self.last_search);
            }
        }
    }

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
                if display_str.len() > max_w {
                    display_str = format!("{}...", &display_str[..max_w.saturating_sub(3)]);
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
            if let Some(target_line) = resolve_anchor(&self.lines, &chosen.url, chosen.line_no) {
                self.top_line = target_line;
                self.message = format!("jumped to {}", chosen.url);
            } else {
                self.message = format!("anchor not found: {}", chosen.url);
            }
        } else if chosen.url.starts_with("http://") || chosen.url.starts_with("https://") {
            let _ = open_external_url(&chosen.url);
            self.message = format!("opened {}", chosen.url);
        } else {
            let path_str = chosen.url.trim_start_matches("file://");
            let target_path = Path::new(path_str);
            if target_path.exists() {
                self.message = format!("viewed {}", chosen.url);
            } else {
                self.message = format!("file not found: {}", chosen.url);
            }
        }

        Ok(())
    }

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

        let err_msg = open_editor(
            &target_file,
            self.top_line,
            self.args.editor_mode.as_deref(),
        )?;

        // Re-enter raw mode and alternate screen
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

    fn reload_spool(&mut self, target_file: &Path) {
        if let Some(ref cmd) = self.args.reload_cmd {
            if let Ok(output) = Command::new("sh").arg("-c").arg(cmd).output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    self.lines = text.lines().map(|s| s.to_string()).collect();
                    self.total = self.lines.len();
                    self.message = "reloaded".to_string();
                    return;
                }
            }
        }

        if let Ok(content) = std::fs::read_to_string(target_file) {
            if self.args.markdown {
                let rendered = crate::markdown::render_markdown(&content, self.cols);
                self.lines = rendered.lines().map(|s| s.to_string()).collect();
            } else {
                self.lines = content.lines().map(|s| s.to_string()).collect();
            }
            self.total = self.lines.len();
            self.message = "reloaded".to_string();
        }
    }
}
