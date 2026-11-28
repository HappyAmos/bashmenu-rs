//! Markdown renderer using pulldown-cmark producing ANSI terminal formatted output
//! matching the rich styling of `rich.sh`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// Maximum bytes downloaded for a single remote Markdown image (10 MiB).
const MAX_REMOTE_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// Global HTTP timeout for fetching a single remote Markdown image.
const REMOTE_IMAGE_TIMEOUT_SECS: u64 = 5;

/// Render Markdown string into terminal ANSI text wrapped to `terminal_width`.
#[allow(dead_code)]
pub fn render_markdown(source: &str, terminal_width: usize) -> String {
    render_markdown_with_options(source, terminal_width, None, true, false)
}

/// Render Markdown string into terminal ANSI text wrapped to `terminal_width`,
/// resolving relative `![alt](path)` image tags against `base_dir` and rendering
/// them via Unicode half-block TrueColor ANSI when `allow_images` is `true`.
/// Remote `http(s)://` images are only fetched when `allow_remote_images` is `true`.
pub fn render_markdown_with_options(
    source: &str,
    terminal_width: usize,
    base_dir: Option<&Path>,
    allow_images: bool,
    allow_remote_images: bool,
) -> String {
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
    renderer.finish()
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

    /// Appends the current accumulated line to the formatted output and clears the buffer.
    fn flush_line(&mut self) {
        if !self.current_line.is_empty() {
            self.output.push_str(&self.current_line);
            self.output.push('\n');
            self.current_line.clear();
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
        let decoded = image::ImageReader::open(&resolved)
            .ok()
            .and_then(|r| r.with_guessed_format().ok())
            .and_then(|r| r.decode().ok());
        let Some(img) = decoded else {
            self.flush_line();
            self.ensure_blank_line();
            emit_fallback(&mut self.output);
            return;
        };

        let img_w = img.width().max(1);
        let img_h = img.height().max(1);
        let max_cols = self.width.saturating_sub(2).clamp(8, 120);
        let max_rows = 28usize;
        let scale_w = max_cols as f64 / img_w as f64;
        let scale_h = (max_rows * 2) as f64 / img_h as f64;
        let scale = scale_w.min(scale_h).min(1.0);
        let target_w = ((img_w as f64 * scale).round() as u32).max(1);
        let target_h = ((img_h as f64 * scale).round() as u32).max(2);

        let rgba = img
            .resize_exact(target_w, target_h, image::imageops::FilterType::Triangle)
            .to_rgba8();

        self.flush_line();
        self.ensure_blank_line();

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
            row.push_str("\x1b[0m\n");
            self.output.push_str(&row);
            y += 2;
        }

        if !alt_trimmed.is_empty() {
            self.output
                .push_str(&format!("\x1b[2;3m  {alt_trimmed}\x1b[0m\n"));
        }
    }

    /// Finalizes rendering, flushing any trailing text line, and returns formatted ANSI text.
    fn finish(mut self) -> String {
        self.flush_line();
        self.output
    }
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

    // Verify the payload is a supported raster image before persisting to cache.
    if image::load_from_memory(&bytes).is_err() {
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
