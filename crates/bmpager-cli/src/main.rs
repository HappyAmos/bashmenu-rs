//! bmpager - High performance interactive full-screen text pager for terminal output and files.

mod ansi;
mod args;
mod editor;
mod help;
mod links;
mod markdown;
mod pager;

use args::parse_cli_args;
use bashmenu_core::t;
use pager::Pager;
use std::io::{stdin, stdout, IsTerminal, Read};
use std::process::exit;

/// CLI entrypoint for `bmpager`.
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

    bashmenu_core::init_locale_from_env(args.locale.as_deref(), None);

    // 3. Collect Raw Input
    let mut raw_content = String::new();
    let mut truncated = false;

    if !args.files.is_empty() {
        for f in &args.files {
            if !f.exists() {
                let p = f.display().to_string();
                eprintln!("bmpager: {}", t!("bmpager.no_such_file", path => &p));
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
                    let p = f.display().to_string();
                    let e = err.to_string();
                    eprintln!(
                        "bmpager: {}",
                        t!("bmpager.cannot_read", path => &p, err => &e)
                    );
                    exit(2);
                }
            }
        }
    } else if stdin().is_terminal() {
        args::print_usage();
        exit(2);
    } else {
        if let Err(err) = stdin().read_to_string(&mut raw_content) {
            let e = err.to_string();
            eprintln!("bmpager: {}", t!("bmpager.error_stdin", err => &e));
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
    let term_width = (cols as usize).clamp(20, 500);

    // 5. Format and split content into lines
    let md_base_dir = args
        .associated_file
        .as_ref()
        .or_else(|| args.files.first())
        .and_then(|p| p.parent())
        .map(std::path::Path::to_path_buf);
    let mut lines: Vec<String> = if is_markdown {
        let rendered = markdown::render_markdown_with_options(
            &raw_content,
            term_width,
            md_base_dir.as_deref(),
            !args.strip_colors,
            args.remote_images,
        );
        rendered.lines().map(|s| s.to_string()).collect()
    } else if args.wrap_text {
        ansi::wrap_text_content(&raw_content, term_width)
    } else {
        raw_content.lines().map(|s| s.to_string()).collect()
    };
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
            eprintln!(
                "bmpager: {}",
                t!("bmpager.input_truncated", count => args.max_lines)
            );
        }
        exit(0);
    }

    // 7. Interactive full-screen Pager
    let mut pager = Pager::new(lines, args).with_raw_content(raw_content);
    if truncated {
        pager.message = t!("bmpager.stored_lines", count => pager.args.max_lines);
    }

    if let Err(err) = pager.run() {
        eprintln!("bmpager: {}", err);
        exit(1);
    }
}
