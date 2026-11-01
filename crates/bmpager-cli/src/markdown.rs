//! Markdown renderer using pulldown-cmark producing ANSI terminal formatted output
//! matching the rich styling of `rich.sh`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use unicode_width::UnicodeWidthStr;

/// Render Markdown string into terminal ANSI text wrapped to `terminal_width`.
pub fn render_markdown(source: &str, terminal_width: usize) -> String {
    let width = terminal_width.clamp(40, 160);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(source, options);
    let mut renderer = MarkdownRenderer::new(width);
    renderer.run(parser);
    renderer.finish()
}

struct MarkdownRenderer {
    width: usize,
    output: String,
    current_line: String,
    heading_level: Option<HeadingLevel>,
    heading_text: String,
    in_code_block: bool,
    code_block_text: String,
    list_stack: Vec<ListState>,
    link_url: Option<String>,
    in_table: bool,
    table_head: bool,
    table_rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    in_blockquote: bool,
}

#[derive(Clone, Copy)]
enum ListState {
    Unordered,
    Ordered(u64),
}

impl MarkdownRenderer {
    fn new(width: usize) -> Self {
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

    fn flush_line(&mut self) {
        if !self.current_line.is_empty() {
            self.output.push_str(&self.current_line);
            self.output.push('\n');
            self.current_line.clear();
        }
    }

    fn ensure_blank_line(&mut self) {
        self.flush_line();
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

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
                Event::Text(text) => {
                    if self.heading_level.is_some() {
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
                        // Inline code styled distinctively
                        let styled = format!("\x1b[38;5;208;48;5;236m {} \x1b[0m", code);
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

    fn render_code_block(&mut self) {
        let border_color = "\x1b[38;5;240m";
        let text_color = "\x1b[38;5;223m";
        let reset = "\x1b[0m";

        let lines: Vec<&str> = self.code_block_text.lines().collect();
        let box_w = self.width.saturating_sub(2);

        self.output.push_str(&format!(
            "{}┌{}┐{}\n",
            border_color,
            "─".repeat(box_w),
            reset
        ));
        for line in lines {
            let vis = UnicodeWidthStr::width(line);
            let pad = box_w.saturating_sub(vis + 2);
            self.output.push_str(&format!(
                "{}│{}  {}{}{}  {}│{}\n",
                border_color,
                reset,
                text_color,
                line,
                reset,
                " ".repeat(pad),
                border_color
            ));
        }
        self.output.push_str(&format!(
            "{}└{}┘{}\n",
            border_color,
            "─".repeat(box_w),
            reset
        ));
    }

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
                    let widest = col_widths
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, &w)| w)
                        .map(|(c, _)| c)
                        .unwrap();
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

            // Separator line after header row: ├───┼───┤
            if r_idx == 0 && self.table_rows.len() > 1 {
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

    fn finish(mut self) -> String {
        self.flush_line();
        self.output
    }
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
