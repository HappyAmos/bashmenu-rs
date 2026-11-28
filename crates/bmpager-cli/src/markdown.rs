//! Markdown renderer using pulldown-cmark producing ANSI terminal formatted output
//! matching the rich styling of `rich.sh`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// Maximum bytes downloaded for a single remote Markdown image (10 MiB).
const MAX_REMOTE_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// Global HTTP timeout for fetching a single remote Markdown image.
const REMOTE_IMAGE_TIMEOUT_SECS: u64 = 5;
/// Maximum decoded frames retained for a single animated GIF.
const MAX_GIF_FRAMES: usize = 60;

/// Pre-rendered multi-frame GIF animation anchored at a line index in the document.
#[derive(Debug, Clone)]
pub struct AnimatedImage {
    /// 0-based starting line index in the rendered document.
    pub start_line: usize,
    /// Pre-rendered half-block ANSI frames paired with per-frame display durations.
    pub frames: Vec<(Vec<String>, Duration)>,
    /// Active frame index (`0..frames.len()`).
    pub current_frame: usize,
    /// Next scheduled frame advance time.
    pub next_tick: Instant,
}

/// Rendered Markdown output paired with any embedded animated GIFs.
#[derive(Debug, Clone)]
pub struct RenderedMarkdown {
    /// Full ANSI-formatted Markdown document (with frame 0 embedded for GIFs).
    pub text: String,
    /// Multi-frame animated GIFs present in the document.
    pub animations: Vec<AnimatedImage>,
}

/// Render Markdown string into terminal ANSI text wrapped to `terminal_width`.
#[allow(dead_code)]
pub fn render_markdown(source: &str, terminal_width: usize) -> String {
    render_markdown_with_options(source, terminal_width, None, true, false)
}

/// Render Markdown string into terminal ANSI text wrapped to `terminal_width`,
/// resolving relative `![alt](path)` image tags against `base_dir` and rendering
/// them via Unicode half-block TrueColor ANSI when `allow_images` is `true`.
/// Remote `http(s)://` images are only fetched when `allow_remote_images` is `true`.
#[allow(dead_code)]
pub fn render_markdown_with_options(
    source: &str,
    terminal_width: usize,
    base_dir: Option<&Path>,
    allow_images: bool,
    allow_remote_images: bool,
) -> String {
    render_markdown_full(
        source,
        terminal_width,
        base_dir,
        allow_images,
        allow_remote_images,
    )
    .text
}

/// Render Markdown string into ANSI text plus any decoded multi-frame GIF animations.
pub fn render_markdown_full(
    source: &str,
    terminal_width: usize,
    base_dir: Option<&Path>,
    allow_images: bool,
    allow_remote_images: bool,
) -> RenderedMarkdown {
    let width = terminal_width.clamp(40, 160);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(source, options);
    let mut renderer = MarkdownRenderer::new(
        width,
        base_dir.map(Path::to_path_buf),
        allow_images,
        allow_remote_images,
    );
    renderer.run(parser);
    renderer.finish_full()
}

/// Stateful Markdown stream renderer converting Pulldown-Cmark events to ANSI-styled terminal text.
struct MarkdownRenderer {
    /// Terminal viewport column width for word wrapping
    width: usize,
    /// Accumulated ANSI output buffer
    output: String,
    /// Current pending visual line buffer
    current_line: String,
    /// Active heading level being processed
    heading_level: Option<HeadingLevel>,
    /// Accumulated heading text
    heading_text: String,
    /// Whether parser is currently inside a fenced code block
    in_code_block: bool,
    /// Accumulated code block text
    code_block_text: String,
    /// Active list nesting stack (unordered vs ordered with item index)
    list_stack: Vec<ListState>,
    /// Target URL for markdown links being rendered
    link_url: Option<String>,
    /// Target path/URL for a markdown image tag currently being parsed
    image_dest: Option<String>,
    /// Accumulated alt text for the active markdown image tag
    image_alt: String,
    /// Optional base directory for resolving relative image paths
    base_dir: Option<PathBuf>,
    /// Whether ANSI half-block image rendering is enabled (false under `--no-color`)
    allow_images: bool,
    /// Whether remote `http(s)://` image fetching into `{cache_dir}/images` is enabled
    allow_remote_images: bool,
    /// Pending 3-row crisp SVG badges (`([top, mid, bot], visual_width)`) to render side-by-side
    pending_badges: Vec<([String; 3], usize)>,
    /// Multi-frame animated GIFs collected during rendering
    animations: Vec<AnimatedImage>,
    /// Whether parser is inside a markdown table
    in_table: bool,
    /// Whether parser is currently in the table header row
    table_head: bool,
    /// Collected table row cells
    table_rows: Vec<Vec<String>>,
    /// Current table row cells accumulator
    current_row: Vec<String>,
    /// Current cell text accumulator
    current_cell: String,
    /// Bold text modifier state
    bold: bool,
    /// Italic text modifier state
    italic: bool,
    /// Strikethrough text modifier state
    strikethrough: bool,
    /// Blockquote nesting state
    in_blockquote: bool,
}

/// Active list context state in markdown rendering.
#[derive(Clone, Copy)]
enum ListState {
    /// Unordered bulleted list (`*` or `-`)
    Unordered,
    /// Ordered numbered list with current integer item number
    Ordered(u64),
}

impl MarkdownRenderer {
    /// Initializes a new MarkdownRenderer with the given viewport column width.
    fn new(
        width: usize,
        base_dir: Option<PathBuf>,
        allow_images: bool,
        allow_remote_images: bool,
    ) -> Self {
        Self {
            width,
            output: String::new(),
            current_line: String::new(),
            heading_level: None,
            heading_text: String::new(),
            in_code_block: false,
            code_block_text: String::new(),
            list_stack: Vec::new(),
            link_url: None,
            image_dest: None,
            image_alt: String::new(),
            base_dir,
            allow_images,
            allow_remote_images,
            pending_badges: Vec::new(),
            animations: Vec::new(),
            in_table: false,
            table_head: false,
            table_rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: String::new(),
            bold: false,
            italic: false,
            strikethrough: false,
            in_blockquote: false,
        }
    }

    /// Flushes any accumulated 3-row Option B SVG badges horizontally side-by-side,
    /// wrapping to a new 3-row strip if the row exceeds `self.width`.
    fn flush_badges(&mut self) {
        if self.pending_badges.is_empty() {
            return;
        }
        let badges = std::mem::take(&mut self.pending_badges);
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            if !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            self.output.push('\n');
        }

        let mut row0 = String::new();
        let mut row1 = String::new();
        let mut row2 = String::new();
        let mut cur_w = 0usize;

        for (rows, bw) in badges {
            let sep_w = if cur_w > 0 { 1 } else { 0 };
            if cur_w > 0 && cur_w + sep_w + bw > self.width {
                self.output
                    .push_str(&format!("{row0}\x1b[0m\n{row1}\x1b[0m\n{row2}\x1b[0m\n"));
                row0.clear();
                row1.clear();
                row2.clear();
                cur_w = 0;
            }
            if cur_w > 0 {
                row0.push_str("\x1b[0m ");
                row1.push_str("\x1b[0m ");
                row2.push_str("\x1b[0m ");
                cur_w += 1;
            }
            row0.push_str(&rows[0]);
            row1.push_str(&rows[1]);
            row2.push_str(&rows[2]);
            cur_w += bw;
        }
        if cur_w > 0 {
            self.output
                .push_str(&format!("{row0}\x1b[0m\n{row1}\x1b[0m\n{row2}\x1b[0m\n"));
        }
    }

    /// Appends the current accumulated line to the formatted output and clears the buffer.
    fn flush_line(&mut self) {
        if !self.current_line.is_empty() {
            self.flush_badges();
            self.output.push_str(&self.current_line);
            self.output.push('\n');
            self.current_line.clear();
        } else {
            self.flush_badges();
        }
    }

    /// Ensures the output stream ends with an empty paragraph separator line.
    fn ensure_blank_line(&mut self) {
        self.flush_line();
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    /// Emits a word to the active line, soft-wrapping to a continuation line if line width is exceeded.
    fn emit_word(&mut self, word: &str) {
        self.flush_badges();
        let prefix = if self.in_blockquote {
            "\x1b[38;5;244m│ \x1b[0m"
        } else if let Some(state) = self.list_stack.last() {
            match state {
                ListState::Unordered => "  • ",
                ListState::Ordered(n) => &format!(" {}. ", n),
            }
        } else {
            ""
        };

        let current_vis = crate::ansi::visual_width(&self.current_line);
        let word_vis = crate::ansi::visual_width(word);

        if self.current_line.is_empty() {
            self.current_line.push_str(prefix);
            self.current_line.push_str(word);
        } else if current_vis + 1 + word_vis <= self.width {
            self.current_line.push(' ');
            self.current_line.push_str(word);
        } else {
            self.flush_line();
            let indent = if self.in_blockquote {
                "\x1b[38;5;244m│ \x1b[0m"
            } else if !self.list_stack.is_empty() {
                "    "
            } else {
                ""
            };
            self.current_line.push_str(indent);
            self.current_line.push_str(word);
        }
    }

    /// Iterates over parser events and translates markdown elements into ANSI formatted terminal text.
    fn run<'a>(&mut self, parser: impl Iterator<Item = Event<'a>>) {
        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    self.ensure_blank_line();
                    self.heading_level = Some(level);
                    self.heading_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some(level) = self.heading_level.take() {
                        self.render_heading(level);
                    }
                }
                Event::Start(Tag::Paragraph) => {
                    self.ensure_blank_line();
                }
                Event::End(TagEnd::Paragraph) => {
                    self.flush_line();
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    self.ensure_blank_line();
                    self.in_blockquote = true;
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    self.flush_line();
                    self.in_blockquote = false;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    self.ensure_blank_line();
                    self.in_code_block = true;
                    self.code_block_text.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    self.in_code_block = false;
                    self.render_code_block();
                }
                Event::Start(Tag::List(start)) => {
                    self.ensure_blank_line();
                    if let Some(start_num) = start {
                        self.list_stack.push(ListState::Ordered(start_num));
                    } else {
                        self.list_stack.push(ListState::Unordered);
                    }
                }
                Event::End(TagEnd::List(_)) => {
                    self.flush_line();
                    self.list_stack.pop();
                }
                Event::Start(Tag::Item) => {
                    self.flush_line();
                }
                Event::End(TagEnd::Item) => {
                    self.flush_line();
                    if let Some(ListState::Ordered(ref mut n)) = self.list_stack.last_mut() {
                        *n += 1;
                    }
                }
                Event::Start(Tag::Table(_)) => {
                    self.ensure_blank_line();
                    self.in_table = true;
                    self.table_rows.clear();
                }
                Event::End(TagEnd::Table) => {
                    self.in_table = false;
                    self.render_table();
                }
                Event::Start(Tag::TableHead) => {
                    self.table_head = true;
                    self.current_row.clear();
                }
                Event::End(TagEnd::TableHead) => {
                    self.table_head = false;
                    self.table_rows.push(std::mem::take(&mut self.current_row));
                }
                Event::Start(Tag::TableRow) => {
                    self.current_row.clear();
                }
                Event::End(TagEnd::TableRow) => {
                    self.table_rows.push(std::mem::take(&mut self.current_row));
                }
                Event::Start(Tag::TableCell) => {
                    self.current_cell.clear();
                }
                Event::End(TagEnd::TableCell) => {
                    self.current_row
                        .push(std::mem::take(&mut self.current_cell));
                }
                Event::Start(Tag::Emphasis) => {
                    self.italic = true;
                }
                Event::End(TagEnd::Emphasis) => {
                    self.italic = false;
                }
                Event::Start(Tag::Strong) => {
                    self.bold = true;
                }
                Event::End(TagEnd::Strong) => {
                    self.bold = false;
                }
                Event::Start(Tag::Strikethrough) => {
                    self.strikethrough = true;
                }
                Event::End(TagEnd::Strikethrough) => {
                    self.strikethrough = false;
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    self.link_url = Some(dest_url.to_string());
                }
                Event::End(TagEnd::Link) => {
                    self.link_url = None;
                }
                Event::Start(Tag::Image { dest_url, .. }) => {
                    self.image_dest = Some(dest_url.to_string());
                    self.image_alt.clear();
                }
                Event::End(TagEnd::Image) => {
                    if let Some(dest) = self.image_dest.take() {
                        let alt = std::mem::take(&mut self.image_alt);
                        self.emit_image(&dest, &alt);
                    }
                }
                Event::Text(text) => {
                    if self.image_dest.is_some() {
                        self.image_alt.push_str(&text);
                    } else if self.heading_level.is_some() {
                        self.heading_text.push_str(&text);
                    } else if self.in_code_block {
                        self.code_block_text.push_str(&text);
                    } else if self.in_table {
                        let mut formatted = String::new();
                        if self.bold {
                            formatted.push_str("\x1b[1m");
                        }
                        if self.italic {
                            formatted.push_str("\x1b[3m");
                        }
                        if let Some(ref url) = self.link_url {
                            formatted.push_str(&format!(
                                "\x1b]8;;{}\x1b\\\x1b[4;36m{}\x1b[0m\x1b]8;;\x1b\\",
                                url, text
                            ));
                        } else {
                            formatted.push_str(&text);
                        }
                        if (self.bold || self.italic) && self.link_url.is_none() {
                            formatted.push_str("\x1b[0m");
                        }
                        self.current_cell.push_str(&formatted);
                    } else {
                        self.handle_styled_text(&text);
                    }
                }
                Event::Code(code) => {
                    if self.heading_level.is_some() {
                        self.heading_text.push_str(&code);
                    } else if self.in_table {
                        self.current_cell.push_str(&format!("`{}`", code));
                    } else {
                        // Inline code styled distinctively (with OSC 8 link if inside link)
                        let styled = if let Some(ref url) = self.link_url {
                            format!(
                                "\x1b]8;;{}\x1b\\\x1b[38;5;208;48;5;236m `{}` \x1b[0m\x1b]8;;\x1b\\",
                                url, code
                            )
                        } else {
                            format!("\x1b[38;5;208;48;5;236m {} \x1b[0m", code)
                        };
                        self.emit_word(&styled);
                    }
                }
                Event::Rule => {
                    self.ensure_blank_line();
                    let rule = "─".repeat(self.width);
                    self.output
                        .push_str(&format!("\x1b[38;5;240m{}\x1b[0m\n", rule));
                }
                Event::SoftBreak => {
                    if self.heading_level.is_some() {
                        self.heading_text.push(' ');
                    } else if self.in_table {
                        self.current_cell.push(' ');
                    }
                }
                Event::HardBreak => {
                    if self.in_table {
                        self.current_cell.push(' ');
                    } else {
                        self.flush_line();
                    }
                }
                _ => {}
            }
        }
    }

    /// Applies active inline ANSI styles (bold, italic, strikethrough, OSC 8 links) to words.
    fn handle_styled_text(&mut self, text: &str) {
        let words = text.split_whitespace();
        for raw_word in words {
            let mut word = String::new();
            if self.bold {
                word.push_str("\x1b[1m");
            }
            if self.italic {
                word.push_str("\x1b[3m");
            }
            if self.strikethrough {
                word.push_str("\x1b[9m");
            }

            if let Some(ref url) = self.link_url {
                // Emit OSC 8 hyperlink with underline styling
                word.push_str(&format!(
                    "\x1b]8;;{}\x1b\\\x1b[4;36m{}\x1b[0m\x1b]8;;\x1b\\",
                    url, raw_word
                ));
            } else {
                word.push_str(raw_word);
                if self.bold || self.italic || self.strikethrough {
                    word.push_str("\x1b[0m");
                }
            }

            self.emit_word(&word);
        }
    }

    /// Renders heading levels H1..H6 with rich box borders and colored markers.
    fn render_heading(&mut self, level: HeadingLevel) {
        let text = self.heading_text.trim();
        match level {
            HeadingLevel::H1 => {
                // Match rich.sh heavy box panel:
                // ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
                // ┃ HA Bash Menu Specification & User Manual (v0.0.1)                            ┃
                // ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
                let box_inner_w = self.width.saturating_sub(4);
                let text_vis = UnicodeWidthStr::width(text);
                let pad = box_inner_w.saturating_sub(text_vis);

                let top_border = format!("┏{}┓", "━".repeat(box_inner_w + 2));
                let mid_content = format!("┃ \x1b[1;37m{}\x1b[0m{} ┃", text, " ".repeat(pad));
                let bot_border = format!("┗{}┛", "━".repeat(box_inner_w + 2));

                self.output
                    .push_str(&format!("\x1b[1;36m{}\x1b[0m\n", top_border));
                self.output
                    .push_str(&format!("\x1b[1;36m{}\x1b[0m\n", mid_content));
                self.output
                    .push_str(&format!("\x1b[1;36m{}\x1b[0m\n", bot_border));
            }
            HeadingLevel::H2 => {
                self.output.push('\n');
                self.output
                    .push_str(&format!("\x1b[1;96m## {}\x1b[0m\n", text));
            }
            HeadingLevel::H3 => {
                self.output.push('\n');
                self.output
                    .push_str(&format!("\x1b[1;93m### {}\x1b[0m\n", text));
            }
            HeadingLevel::H4 => {
                self.output
                    .push_str(&format!("\x1b[1;37m#### {}\x1b[0m\n", text));
            }
            HeadingLevel::H5 | HeadingLevel::H6 => {
                self.output
                    .push_str(&format!("\x1b[1;37m##### {}\x1b[0m\n", text));
            }
        }
    }

    /// Renders a syntax-highlighted code block enclosed in a box frame.
    fn render_code_block(&mut self) {
        let border_color = "\x1b[38;5;240m";
        let text_color = "\x1b[38;5;223m";
        let reset = "\x1b[0m";

        let lines: Vec<&str> = self.code_block_text.lines().collect();
        let box_w = self.width.saturating_sub(2);
        let max_code_w = box_w.saturating_sub(4);

        self.output.push_str(&format!(
            "{}┌{}┐{}\n",
            border_color,
            "─".repeat(box_w),
            reset
        ));
        for line in lines {
            let expanded = line.replace('\t', "    ");
            let mut clipped = String::new();
            let mut vis = 0usize;
            for ch in expanded.chars() {
                let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if vis + cw > max_code_w {
                    break;
                }
                clipped.push(ch);
                vis += cw;
            }
            let pad = max_code_w.saturating_sub(vis);
            self.output.push_str(&format!(
                "{}│{}  {}{}{}  {}{}│{}\n",
                border_color,
                reset,
                text_color,
                clipped,
                reset,
                " ".repeat(pad),
                border_color,
                reset
            ));
        }
        self.output.push_str(&format!(
            "{}└{}┘{}\n",
            border_color,
            "─".repeat(box_w),
            reset
        ));
    }

    /// Computes column widths and renders a multi-column markdown table with word wrapping and borders.
    fn render_table(&mut self) {
        if self.table_rows.is_empty() {
            return;
        }

        let num_cols = self.table_rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if num_cols == 0 {
            return;
        }

        // 1. Calculate natural width and minimum word width per column
        let mut nat_w = vec![0usize; num_cols];
        let mut min_w = vec![3usize; num_cols];

        for row in &self.table_rows {
            for (c, cell) in row.iter().enumerate() {
                if c >= num_cols {
                    break;
                }
                let vis = crate::ansi::visual_width(cell);
                nat_w[c] = nat_w[c].max(vis);
                for word in cell.split_whitespace() {
                    let w_vis = crate::ansi::visual_width(word);
                    min_w[c] = min_w[c].max(w_vis);
                }
            }
        }

        for w in &mut min_w {
            *w = (*w).clamp(3, 25);
        }

        let overhead = num_cols * 3 + 1;
        let avail_text_w = self.width.saturating_sub(overhead);

        let mut col_widths = nat_w.clone();
        if col_widths.iter().sum::<usize>() > avail_text_w && avail_text_w > 0 {
            // Shrink columns that exceed min_w down to avail_text_w
            while col_widths.iter().sum::<usize>() > avail_text_w {
                let widest_idx = col_widths
                    .iter()
                    .enumerate()
                    .filter(|(c, &w)| w > min_w[*c])
                    .max_by_key(|(_, &w)| w)
                    .map(|(c, _)| c);

                if let Some(c) = widest_idx {
                    col_widths[c] -= 1;
                } else {
                    let Some(widest) = col_widths
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, &w)| w)
                        .map(|(c, _)| c)
                    else {
                        break;
                    };
                    if col_widths[widest] <= 3 {
                        break;
                    }
                    col_widths[widest] -= 1;
                }
            }
        }

        let border_color = "\x1b[38;5;242m";
        let reset = "\x1b[0m";

        // Top border: ┌───┬───┐
        let mut top_line = format!("{}┌", border_color);
        for (i, w) in col_widths.iter().enumerate() {
            top_line.push_str(&"─".repeat(w + 2));
            if i + 1 < num_cols {
                top_line.push('┬');
            } else {
                top_line.push('┐');
            }
        }
        top_line.push_str(reset);
        self.output.push_str(&top_line);
        self.output.push('\n');

        // Rows
        for (r_idx, row) in self.table_rows.iter().enumerate() {
            let wrapped_cells: Vec<Vec<String>> = (0..num_cols)
                .map(|c| {
                    let cell_text = row.get(c).map(|s| s.as_str()).unwrap_or("");
                    wrap_cell_text(cell_text, col_widths[c])
                })
                .collect();

            let row_h = wrapped_cells.iter().map(|l| l.len()).max().unwrap_or(1);

            for h in 0..row_h {
                let mut row_line = format!("{}│{}", border_color, reset);
                for c in 0..num_cols {
                    let cell_line = wrapped_cells[c].get(h).map(|s| s.as_str()).unwrap_or("");
                    let cell_vis = crate::ansi::visual_width(cell_line);
                    let pad = col_widths[c].saturating_sub(cell_vis);

                    if r_idx == 0 {
                        row_line.push_str(&format!(
                            " \x1b[1;37m{}\x1b[0m{} {}│{}",
                            cell_line,
                            " ".repeat(pad),
                            border_color,
                            reset
                        ));
                    } else if c == 0 {
                        row_line.push_str(&format!(
                            " \x1b[1;96m{}\x1b[0m{} {}│{}",
                            cell_line,
                            " ".repeat(pad),
                            border_color,
                            reset
                        ));
                    } else {
                        row_line.push_str(&format!(
                            " {}{} {}│{}",
                            cell_line,
                            " ".repeat(pad),
                            border_color,
                            reset
                        ));
                    }
                }
                self.output.push_str(&row_line);
                self.output.push('\n');
            }

            // Separator line between rows (header and each data entry): ├───┼───┤
            if r_idx + 1 < self.table_rows.len() {
                let mut sep_line = format!("{}├", border_color);
                for (i, w) in col_widths.iter().enumerate() {
                    sep_line.push_str(&"─".repeat(w + 2));
                    if i + 1 < num_cols {
                        sep_line.push('┼');
                    } else {
                        sep_line.push('┤');
                    }
                }
                sep_line.push_str(reset);
                self.output.push_str(&sep_line);
                self.output.push('\n');
            }
        }

        // Bottom border: └───┴───┘
        let mut bot_line = format!("{}└", border_color);
        for (i, w) in col_widths.iter().enumerate() {
            bot_line.push_str(&"─".repeat(w + 2));
            if i + 1 < num_cols {
                bot_line.push('┴');
            } else {
                bot_line.push('┘');
            }
        }
        bot_line.push_str(reset);
        self.output.push_str(&bot_line);
        self.output.push('\n');
    }

    /// Renders a Markdown `![alt](dest)` image using Unicode half-block TrueColor
    /// ANSI cells (`▀`/`▄`) when a local image file can be decoded, or falls back
    /// cleanly to a compact `[Image: ...]` badge.
    fn emit_image(&mut self, dest: &str, alt: &str) {
        let dest_trimmed = dest.trim();
        let alt_trimmed = alt.trim();
        let fallback_label = if alt_trimmed.is_empty() {
            dest_trimmed.to_string()
        } else {
            format!("{alt_trimmed} ({dest_trimmed})")
        };

        if self.in_table {
            self.current_cell
                .push_str(&format!("[Image: {fallback_label}]"));
            return;
        }

        let is_http = dest_trimmed.starts_with("http://") || dest_trimmed.starts_with("https://");
        let is_data = dest_trimmed.starts_with("data:");

        let emit_fallback = |out: &mut String| {
            if is_http {
                out.push_str(&format!(
                    "\x1b]8;;{dest_trimmed}\x1b\\\x1b[2;36m[Image: {fallback_label}]\x1b[0m\x1b]8;;\x1b\\\n"
                ));
            } else {
                out.push_str(&format!("\x1b[2;33m[Image: {fallback_label}]\x1b[0m\n"));
            }
        };

        if !self.allow_images
            || is_data
            || dest_trimmed.is_empty()
            || (is_http && !self.allow_remote_images)
        {
            self.flush_line();
            self.ensure_blank_line();
            emit_fallback(&mut self.output);
            return;
        }

        let resolved = if is_http {
            let cache_dir = resolve_image_cache_dir();
            match fetch_remote_image_cached(dest_trimmed, &cache_dir) {
                Some(path) => path,
                None => {
                    self.flush_line();
                    self.ensure_blank_line();
                    emit_fallback(&mut self.output);
                    return;
                }
            }
        } else {
            resolve_markdown_image_path(dest_trimmed, self.base_dir.as_deref())
        };
        let Ok(raw_bytes) = std::fs::read(&resolved) else {
            self.flush_line();
            self.ensure_blank_line();
            emit_fallback(&mut self.output);
            return;
        };

        // Try crisp 3-row Option B badge rendering first for badge-like SVGs with text
        if let Some((mut badge_rows, badge_w)) = try_render_svg_badge(&raw_bytes) {
            if let Some(ref url) = self.link_url {
                for row in &mut badge_rows {
                    *row = format!("\x1b]8;;{url}\x1b\\{row}\x1b]8;;\x1b\\");
                }
            }
            if !self.current_line.is_empty() {
                self.flush_line();
            }
            self.pending_badges.push((badge_rows, badge_w));
            return;
        }

        let max_cols = self.width.saturating_sub(2).clamp(8, 120);
        let max_rows = 28usize;

        // Check for multi-frame animated GIFs
        if let Some(frames) = try_decode_animated_gif(&raw_bytes, max_cols, max_rows) {
            self.flush_line();
            self.ensure_blank_line();
            let start_line = self.output.bytes().filter(|&b| b == b'\n').count();
            if let Some((first_lines, first_delay)) = frames.first() {
                for line in first_lines {
                    self.output.push_str(line);
                    self.output.push('\n');
                }
                let next_tick = Instant::now() + *first_delay;
                self.animations.push(AnimatedImage {
                    start_line,
                    frames,
                    current_frame: 0,
                    next_tick,
                });
            }
            if !alt_trimmed.is_empty() {
                self.output
                    .push_str(&format!("\x1b[2;3m  {alt_trimmed}\x1b[0m\n"));
            }
            return;
        }

        let Some(rgba) = decode_image_or_svg(&raw_bytes, max_cols, max_rows) else {
            self.flush_line();
            self.ensure_blank_line();
            emit_fallback(&mut self.output);
            return;
        };

        self.flush_line();
        self.ensure_blank_line();

        for row in render_rgba_to_halfblocks(&rgba) {
            self.output.push_str(&row);
            self.output.push('\n');
        }

        if !alt_trimmed.is_empty() {
            self.output
                .push_str(&format!("\x1b[2;3m  {alt_trimmed}\x1b[0m\n"));
        }
    }

    /// Finalizes rendering, flushing any trailing text line, and returns formatted ANSI text
    /// alongside any decoded animated GIFs.
    fn finish_full(mut self) -> RenderedMarkdown {
        self.flush_line();
        RenderedMarkdown {
            text: self.output,
            animations: self.animations,
        }
    }
}

/// Converts an RGBA bitmap into TrueColor Unicode half-block (`▀`/`▄`) ANSI lines.
fn render_rgba_to_halfblocks(rgba: &image::RgbaImage) -> Vec<String> {
    let target_w = rgba.width();
    let target_h = rgba.height();
    let mut rows = Vec::with_capacity((target_h as usize).div_ceil(2));
    let mut y = 0u32;
    while y < target_h {
        let mut row = String::with_capacity(target_w as usize * 24);
        for x in 0..target_w {
            let top = rgba.get_pixel(x, y).0;
            let bot = if y + 1 < target_h {
                rgba.get_pixel(x, y + 1).0
            } else {
                [0, 0, 0, 0]
            };
            let top_vis = top[3] >= 128;
            let bot_vis = bot[3] >= 128;
            match (top_vis, bot_vis) {
                (false, false) => row.push_str("\x1b[0m "),
                (true, false) => row.push_str(&format!(
                    "\x1b[0m\x1b[38;2;{};{};{}m▀",
                    top[0], top[1], top[2]
                )),
                (false, true) => row.push_str(&format!(
                    "\x1b[0m\x1b[38;2;{};{};{}m▄",
                    bot[0], bot[1], bot[2]
                )),
                (true, true) => row.push_str(&format!(
                    "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                    top[0], top[1], top[2], bot[0], bot[1], bot[2]
                )),
            }
        }
        row.push_str("\x1b[0m");
        rows.push(row);
        y += 2;
    }
    rows
}

/// Decodes a multi-frame animated GIF into pre-rendered half-block ANSI frames
/// paired with their frame delays. Returns `None` for non-GIFs or single-frame GIFs.
fn try_decode_animated_gif(
    bytes: &[u8],
    max_cols: usize,
    max_rows: usize,
) -> Option<Vec<(Vec<String>, Duration)>> {
    if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }
    use image::AnimationDecoder;
    let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let raw_frames: Vec<image::Frame> = decoder
        .into_frames()
        .take(MAX_GIF_FRAMES)
        .filter_map(Result::ok)
        .collect();
    if raw_frames.len() <= 1 {
        return None;
    }

    let first_buf = raw_frames[0].buffer();
    let (target_w, target_h) = compute_target_dimensions(
        first_buf.width() as f64,
        first_buf.height() as f64,
        max_cols,
        max_rows,
        false,
    );

    let mut out = Vec::with_capacity(raw_frames.len());
    for frame in raw_frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let ms = if denom == 0 {
            100u64
        } else {
            (numer as u64 / denom as u64).clamp(50, 5000)
        };
        let resized = image::imageops::resize(
            frame.buffer(),
            target_w,
            target_h,
            image::imageops::FilterType::Triangle,
        );
        out.push((
            render_rgba_to_halfblocks(&resized),
            Duration::from_millis(ms),
        ));
    }
    Some(out)
}

/// Resolves a Markdown image path against `base_dir` (expanding leading `~/`).
fn resolve_markdown_image_path(raw: &str, base_dir: Option<&Path>) -> PathBuf {
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(rest)
        } else {
            PathBuf::from(raw)
        }
    } else {
        PathBuf::from(raw)
    };
    if expanded.is_absolute() {
        expanded
    } else if let Some(dir) = base_dir {
        dir.join(expanded)
    } else {
        expanded
    }
}

/// Resolves the `{cache_dir}/images` directory used to cache downloaded web images.
fn resolve_image_cache_dir() -> PathBuf {
    let cfg_dir = bashmenu_core::get_user_config_dir();
    let cfg_path = cfg_dir.join("bashmenu.yml");
    let base_cache = bashmenu_core::load_config(&cfg_path)
        .ok()
        .and_then(|c| c.settings.cache_dir)
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let expanded = if let Some(rest) = s.strip_prefix("~/") {
                if let Ok(home) = std::env::var("HOME") {
                    PathBuf::from(home).join(rest)
                } else {
                    PathBuf::from(&s)
                }
            } else {
                PathBuf::from(&s)
            };
            if expanded.is_absolute() {
                expanded
            } else {
                cfg_dir.join(expanded)
            }
        })
        .unwrap_or_else(|| cfg_dir.join("cache"));
    base_cache.join("images")
}

/// Downloads an `http(s)://` image natively via `ureq` (rustls) into
/// `cache_dir/<sha256>.img`, returning the cached file path on success.
fn fetch_remote_image_cached(url: &str, cache_dir: &Path) -> Option<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let digest = hasher.finalize();
    let hash_hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let cache_path = cache_dir.join(format!("{hash_hex}.img"));
    if cache_path.is_file() {
        return Some(cache_path);
    }

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(REMOTE_IMAGE_TIMEOUT_SECS)))
        .max_redirects(5)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut resp = agent
        .get(url)
        .header("User-Agent", concat!("bmpager/", env!("CARGO_PKG_VERSION")))
        .call()
        .ok()?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_REMOTE_IMAGE_BYTES)
        .read_to_vec()
        .ok()?;

    // Verify the payload is a supported raster image or valid SVG before caching.
    if image::load_from_memory(&bytes).is_err() && !is_valid_svg_bytes(&bytes) {
        return None;
    }

    if std::fs::create_dir_all(cache_dir).is_err() {
        return None;
    }
    let tmp_path = cache_dir.join(format!("{hash_hex}.tmp.{}", std::process::id()));
    if std::fs::write(&tmp_path, &bytes).is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }
    if std::fs::rename(&tmp_path, &cache_path).is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }
    Some(cache_path)
}

/// Shared lazily-loaded system font database for SVG `<text>` rendering.
fn svg_fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    std::sync::Arc::clone(DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        // Ensure generic CSS font families (e.g. `sans-serif` used by shields.io)
        // always resolve even on minimal Linux systems.
        let first_family = db
            .faces()
            .find_map(|f| f.families.first().map(|(name, _)| name.clone()));
        if let Some(first_family) = first_family {
            let preferred_sans = ["DejaVu Sans", "Liberation Sans", "Noto Sans", "Arial"]
                .into_iter()
                .find(|cand| {
                    db.faces()
                        .any(|f| f.families.iter().any(|(n, _)| n == cand))
                })
                .map(String::from)
                .unwrap_or_else(|| first_family.clone());
            db.set_sans_serif_family(&preferred_sans);
            db.set_serif_family(&preferred_sans);
            db.set_monospace_family(&first_family);
        }
        std::sync::Arc::new(db)
    }))
}

/// Returns `true` if `bytes` parses as a valid SVG / SVGZ document.
fn is_valid_svg_bytes(bytes: &[u8]) -> bool {
    let opt = resvg::usvg::Options::default();
    resvg::usvg::Tree::from_data(bytes, &opt).is_ok()
}

/// Extracted text segment from a badge-like SVG (`shields.io`, GitHub Actions, etc.).
struct SvgBadgeSegment {
    text: String,
    center_x: f32,
    fg_rgb: Option<(u8, u8, u8)>,
    opacity: f32,
}

fn collect_svg_text_segments(group: &resvg::usvg::Group, out: &mut Vec<SvgBadgeSegment>) {
    for node in group.children() {
        match node {
            resvg::usvg::Node::Group(g) => collect_svg_text_segments(g, out),
            resvg::usvg::Node::Text(text) => {
                let mut label = String::new();
                let mut fg_rgb: Option<(u8, u8, u8)> = None;
                let mut best_opacity = 0.0f32;
                for chunk in text.chunks() {
                    let s = chunk.text().trim();
                    if !s.is_empty() {
                        if !label.is_empty() {
                            label.push(' ');
                        }
                        label.push_str(s);
                    }
                    for span in chunk.spans() {
                        if let Some(fill) = span.fill() {
                            let op = fill.opacity().get();
                            if op > best_opacity {
                                best_opacity = op;
                                if let resvg::usvg::Paint::Color(c) = fill.paint() {
                                    fg_rgb = Some((c.red, c.green, c.blue));
                                }
                            }
                        }
                    }
                }
                if !label.is_empty() && best_opacity >= 0.5 {
                    let bbox = text.abs_bounding_box();
                    let center_x = bbox.x() + bbox.width() * 0.5;
                    out.push(SvgBadgeSegment {
                        text: label,
                        center_x,
                        fg_rgb,
                        opacity: best_opacity,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Detects badge-like SVGs (`height <= 48px`, wide aspect ratio, containing
/// legible `<text>` elements) and renders them as a crisp 3-row Option B badge
/// (`▄` top cap, bold TrueColor terminal text middle row, `▀` bottom cap) using
/// the exact background and foreground colors sampled from the SVG.
fn try_render_svg_badge(bytes: &[u8]) -> Option<([String; 3], usize)> {
    let opt = resvg::usvg::Options {
        fontdb: svg_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(bytes, &opt).ok()?;
    let size = tree.size();
    let w = size.width();
    let h = size.height();
    if h > 48.0 || h < 8.0 || w < h * 1.5 {
        return None;
    }

    let mut raw_segs = Vec::new();
    collect_svg_text_segments(tree.root(), &mut raw_segs);
    if raw_segs.is_empty() {
        return None;
    }

    raw_segs.sort_by(|a, b| {
        a.center_x
            .partial_cmp(&b.center_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deduplicate identical text at nearly the same x (e.g., shadow/duplicate layers)
    let mut segs: Vec<SvgBadgeSegment> = Vec::new();
    for seg in raw_segs {
        if let Some(prev) = segs.last_mut() {
            if prev.text == seg.text && (prev.center_x - seg.center_x).abs() < 6.0 {
                if seg.opacity >= prev.opacity {
                    *prev = seg;
                }
                continue;
            }
        }
        segs.push(seg);
    }
    if segs.is_empty() || segs.len() > 6 {
        return None;
    }

    // Rasterize at native CSS resolution to sample exact segment background colors
    let pw = (w.round() as u32).max(4);
    let ph = (h.round() as u32).max(4);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let data = pixmap.data();
    let sample_rgb = |x: u32, y: u32| -> Option<(u8, u8, u8)> {
        let sx = x.min(pw - 1);
        let sy = y.min(ph - 1);
        let idx = ((sy * pw + sx) * 4) as usize;
        let a = data[idx + 3];
        if a < 128 {
            return None;
        }
        let af = a as f32 / 255.0;
        Some((
            (data[idx] as f32 / af).round().min(255.0) as u8,
            (data[idx + 1] as f32 / af).round().min(255.0) as u8,
            (data[idx + 2] as f32 / af).round().min(255.0) as u8,
        ))
    };

    let mut row0 = String::new();
    let mut row1 = String::new();
    let mut row2 = String::new();
    let mut total_w = 0usize;

    for seg in segs {
        let sx = (seg.center_x.round().max(2.0) as u32).min(pw.saturating_sub(2));
        // Sample near top/bottom edge (2px inset) where pure background rect sits
        let (br, bg, bb) = sample_rgb(sx, 2.min(ph - 1))
            .or_else(|| sample_rgb(sx, ph.saturating_sub(3)))
            .unwrap_or((85, 85, 85));
        let (fr, fg, fb) = seg.fg_rgb.unwrap_or_else(|| {
            let luma = 0.299 * br as f32 + 0.587 * bg as f32 + 0.114 * bb as f32;
            if luma > 160.0 {
                (20, 20, 20)
            } else {
                (255, 255, 255)
            }
        });

        let padded = format!(" {} ", seg.text);
        let seg_w = UnicodeWidthStr::width(padded.as_str());
        row0.push_str(&format!(
            "\x1b[0m\x1b[38;2;{br};{bg};{bb}m{}",
            "▄".repeat(seg_w)
        ));
        row1.push_str(&format!(
            "\x1b[38;2;{fr};{fg};{fb};48;2;{br};{bg};{bb};1m{padded}\x1b[0m"
        ));
        row2.push_str(&format!(
            "\x1b[0m\x1b[38;2;{br};{bg};{bb}m{}",
            "▀".repeat(seg_w)
        ));
        total_w += seg_w;
    }

    Some(([row0, row1, row2], total_w))
}

/// Computes natural half-block pixel dimensions `(target_w, target_h)` for an
/// image of intrinsic size `(src_w, src_h)` within `(max_cols, max_rows * 2)`.
///
/// Uses ~3 CSS pixels per half-block cell (`~2.5x` typical browser display size
/// so badges and icons stay compact yet legible, while large photos/diagrams
/// still scale cleanly up to the viewport bounds).
fn compute_target_dimensions(
    src_w: f64,
    src_h: f64,
    max_cols: usize,
    max_rows: usize,
    is_svg: bool,
) -> (u32, u32) {
    let w = src_w.max(1.0);
    let h = src_h.max(1.0);
    let max_px_w = max_cols.max(1) as f64;
    let max_px_h = (max_rows.max(1) * 2) as f64;

    // For SVGs and small raster badges/icons (<= 64px tall), map ~3 source px
    // to 1 half-block pixel so shields.io badges (e.g. 110x20) render at ~37x3
    // terminal cells instead of 110x10. Tiny swatches (<= 24px) keep 1:1 scale.
    let natural_scale: f64 = if (is_svg || h <= 64.0) && (w > 24.0 || h > 24.0) {
        1.0 / 3.0
    } else {
        1.0
    };

    let fit_scale = (max_px_w / w).min(max_px_h / h);
    let scale = natural_scale.min(fit_scale);

    let target_w = ((w * scale).round() as u32).max(1);
    let target_h = ((h * scale).round() as u32).max(2);
    (target_w, target_h)
}

/// Decodes raster image bytes (PNG/JPEG/WebP/GIF) or rasterizes an SVG/SVGZ
/// into an RGBA buffer sized to fit `(max_cols, max_rows * 2)` pixels.
fn decode_image_or_svg(bytes: &[u8], max_cols: usize, max_rows: usize) -> Option<image::RgbaImage> {
    if let Ok(img) = image::load_from_memory(bytes) {
        let (target_w, target_h) = compute_target_dimensions(
            img.width() as f64,
            img.height() as f64,
            max_cols,
            max_rows,
            false,
        );
        return Some(
            img.resize_exact(target_w, target_h, image::imageops::FilterType::Triangle)
                .to_rgba8(),
        );
    }

    let opt = resvg::usvg::Options {
        fontdb: svg_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(bytes, &opt).ok()?;
    let size = tree.size();
    let (target_w, target_h) = compute_target_dimensions(
        size.width() as f64,
        size.height() as f64,
        max_cols,
        max_rows,
        true,
    );

    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_w, target_h)?;
    let sx = target_w as f32 / size.width().max(1.0);
    let sy = target_h as f32 / size.height().max(1.0);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );

    // Un-premultiply tiny-skia's premultiplied RGBA pixels into straight RGBA.
    let mut out = image::RgbaImage::new(target_w, target_h);
    let src = pixmap.data();
    for y in 0..target_h {
        for x in 0..target_w {
            let idx = ((y * target_w + x) * 4) as usize;
            let a = src[idx + 3];
            let (r, g, b) = if a == 0 {
                (0, 0, 0)
            } else if a == 255 {
                (src[idx], src[idx + 1], src[idx + 2])
            } else {
                let af = a as f32 / 255.0;
                (
                    (src[idx] as f32 / af).round().min(255.0) as u8,
                    (src[idx + 1] as f32 / af).round().min(255.0) as u8,
                    (src[idx + 2] as f32 / af).round().min(255.0) as u8,
                )
            };
            out.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    }
    Some(out)
}

/// Word-wraps a table cell string to fit within `max_width` visual terminal cells,
/// preserving ANSI formatting sequences and hard-breaking words that exceed `max_width`.
fn wrap_cell_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![String::new()];
    }

    if crate::ansi::visual_width(trimmed) <= max_width {
        return vec![trimmed.to_string()];
    }

    let mut lines = Vec::new();
    let mut cur_line = String::new();
    let mut cur_vis = 0;

    for word in trimmed.split_whitespace() {
        let word_vis = crate::ansi::visual_width(word);
        if cur_line.is_empty() {
            if word_vis <= max_width {
                cur_line.push_str(word);
                cur_vis = word_vis;
            } else {
                let mut rem = word.to_string();
                while !rem.is_empty() {
                    let (chunk, tail) = crate::ansi::split_ansi_line(&rem, max_width);
                    if chunk.is_empty() {
                        let mut chars = rem.chars();
                        if let Some(ch) = chars.next() {
                            lines.push(ch.to_string());
                            rem = chars.as_str().to_string();
                        } else {
                            break;
                        }
                    } else {
                        lines.push(chunk);
                        rem = tail;
                    }
                }
            }
        } else if cur_vis + 1 + word_vis <= max_width {
            cur_line.push(' ');
            cur_line.push_str(word);
            cur_vis += 1 + word_vis;
        } else {
            lines.push(cur_line);
            cur_line = String::new();
            cur_vis = 0;
            if word_vis <= max_width {
                cur_line.push_str(word);
                cur_vis = word_vis;
            } else {
                let mut rem = word.to_string();
                while !rem.is_empty() {
                    let (chunk, tail) = crate::ansi::split_ansi_line(&rem, max_width);
                    if chunk.is_empty() {
                        let mut chars = rem.chars();
                        if let Some(ch) = chars.next() {
                            lines.push(ch.to_string());
                            rem = chars.as_str().to_string();
                        } else {
                            break;
                        }
                    } else {
                        lines.push(chunk);
                        rem = tail;
                    }
                }
            }
        }
    }

    if !cur_line.is_empty() {
        lines.push(cur_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
