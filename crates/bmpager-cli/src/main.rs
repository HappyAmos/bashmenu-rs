//! bmpager - High performance interactive full-screen text pager for terminal output and files.

mod ansi;
mod args;
mod editor;
mod help;
mod links;
mod markdown;
mod pager;

use args::parse_cli_args;
use pager::Pager;
use std::io::{stdin, stdout, IsTerminal, Read};
use std::process::exit;

fn main() {
    // 1. Terminal Panic Hook Invariant (AGENTS.md)
    bashmenu_tui::install_panic_hook();

    // 2. Parse CLI Arguments
    let args = match parse_cli_args() {
        Ok(a) => a,
        Err(err) => {
            eprintln!("bmpager: {}", err);
            exit(2);
        }
    };

    // 3. Collect Raw Input
    let mut raw_content = String::new();
    let mut truncated = false;

    if !args.files.is_empty() {
        for f in &args.files {
            if !f.exists() {
                eprintln!("bmpager: no such file: {}", f.display());
                exit(2);
            }
            match std::fs::read_to_string(f) {
                Ok(content) => {
                    raw_content.push_str(&content);
                    if !raw_content.ends_with('\n') {
                        raw_content.push('\n');
                    }
                }
                Err(err) => {
                    eprintln!("bmpager: cannot read: {}: {}", f.display(), err);
                    exit(2);
                }
            }
        }
    } else if stdin().is_terminal() {
        args::print_usage();
        exit(2);
    } else {
        if let Err(err) = stdin().read_to_string(&mut raw_content) {
            eprintln!("bmpager: error reading stdin: {}", err);
            exit(2);
        }
    }

    // 4. Check Markdown flag or auto-detection
    let is_markdown = args.markdown
        || (args.files.len() == 1
            && args.files[0]
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
            && args.markdown);

    let (cols, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let final_content = if is_markdown {
        markdown::render_markdown(&raw_content, cols as usize)
    } else {
        raw_content
    };

    // 5. Split into lines and cap at max_lines
    let mut lines: Vec<String> = final_content.lines().map(|s| s.to_string()).collect();
    if lines.len() > args.max_lines {
        lines.truncate(args.max_lines);
        truncated = true;
    }

    // 6. Passthrough mode if stdout is not a TTY
    if !stdout().is_terminal() {
        use std::io::{ErrorKind, Write};
        let mut out = stdout();
        for line in &lines {
            let out_line = if args.strip_colors {
                ansi::strip_ansi(line)
            } else {
                line.clone()
            };
            if let Err(e) = writeln!(out, "{}", out_line) {
                if e.kind() == ErrorKind::BrokenPipe {
                    exit(0);
                }
            }
        }
        if truncated {
            eprintln!("bmpager: input truncated at {} lines", args.max_lines);
        }
        exit(0);
    }

    // 7. Interactive full-screen Pager
    let mut pager = Pager::new(lines, args);
    if truncated {
        pager.message = format!("stored only the first {} lines", pager.args.max_lines);
    }

    if let Err(err) = pager.run() {
        eprintln!("bmpager: {}", err);
        exit(1);
    }
}
