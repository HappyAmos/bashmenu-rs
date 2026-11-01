//! Command line arguments parser for bmpager.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PagerArgs {
    pub files: Vec<PathBuf>,
    pub strip_colors: bool,
    pub show_help_hints: bool,
    pub allow_file: bool,
    pub editor_mode: Option<String>,
    pub max_lines: usize,
    pub associated_file: Option<PathBuf>,
    pub reload_cmd: Option<String>,
    pub start_line: Option<usize>,
    pub start_pattern: Option<String>,
    pub markdown: bool,
}

impl Default for PagerArgs {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            strip_colors: false,
            show_help_hints: true,
            allow_file: true,
            editor_mode: None,
            max_lines: 1_000_000,
            associated_file: None,
            reload_cmd: None,
            start_line: None,
            start_pattern: None,
            markdown: false,
        }
    }
}

pub fn print_usage() {
    println!(
        "Usage: command | bmpager [options]\n       bmpager [options] <file> [file...]\n\n\
Interactive pager. Text is read from stdin or from the named files, then\n\
displayed one screenful at a time. Press SPACE for the next page, b for\n\
the previous page, arrow keys (or j/k) for single lines, and q to quit.\n\n\
Options:\n  \
  -h, --help        Show this help and exit.\n  \
  -V, --version     Print the version and exit.\n  \
  -m, --markdown    Render input formatted as Markdown.\n  \
  +N                Start with line N at the top of the window.\n  \
  +/PATTERN         Start at the first line containing PATTERN.\n  \
  -c, --no-color    Discard ANSI escape sequences instead of rendering them.\n  \
  --no-help         Omit the shortcut hints from the command bar.\n  \
  --no-file         Do not show filename badge or allow editing current file.\n  \
  --editor=MODE     Editor mode: 'default' ($EDITOR) or 'builtin' (BashEdit).\n  \
  --builtin-editor  Use built-in editor (bashedit).\n  \
  --system-editor   Use system default editor ($EDITOR / $VISUAL).\n  \
  --max-lines=N     Store at most N input lines (default 1000000).\n  \
  --file=PATH       Associated file path to open when editing piped streams.\n  \
  --reload-cmd=CMD  Command to refresh spool after editing piped streams.\n\n\
Keys:\n  \
  DOWN j            Next line            UP k            Previous line\n  \
  SPACE f PgDn      Next page            b PgUp          Previous page\n  \
  Ctrl-D            Half page down       Ctrl-U          Half page up\n  \
  g Home            Jump to TOP          G End           Jump to BOT\n  \
  /                 Search forward       n / N           Next / prev match\n  \
  l o               Follow / open link   e               Open in editor\n  \
  h ? F1            Help manual modal    q Q Ctrl-C Esc  Quit"
    );
}

pub fn parse_cli_args() -> Result<PagerArgs, String> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mut args = PagerArgs::default();
    let mut i = 0;

    while i < raw_args.len() {
        let arg = &raw_args[i];

        if arg == "-h" || arg == "--help" {
            print_usage();
            std::process::exit(0);
        } else if arg == "-V" || arg == "--version" {
            println!("bmpager {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        } else if arg == "-m" || arg == "--markdown" {
            args.markdown = true;
        } else if arg == "-c" || arg == "--no-color" || arg == "--no-colour" {
            args.strip_colors = true;
        } else if arg == "--no-help" {
            args.show_help_hints = false;
        } else if arg == "--no-file" || arg == "--no-edit" || arg == "--view-only" {
            args.allow_file = false;
        } else if arg == "--builtin-editor" || arg == "--bm-editor" {
            args.editor_mode = Some("builtin".to_string());
        } else if arg == "--system-editor" || arg == "--default-editor" {
            args.editor_mode = Some("default".to_string());
        } else if let Some(val) = arg.strip_prefix("--editor=") {
            args.editor_mode = Some(val.to_string());
        } else if arg == "--editor" {
            i += 1;
            if i >= raw_args.len() {
                return Err("--editor needs a value".to_string());
            }
            args.editor_mode = Some(raw_args[i].clone());
        } else if let Some(val) = arg.strip_prefix("--max-lines=") {
            args.max_lines = val
                .parse::<usize>()
                .map_err(|_| format!("invalid line limit: {}", val))?;
        } else if arg == "--max-lines" {
            i += 1;
            if i >= raw_args.len() {
                return Err("--max-lines needs a value".to_string());
            }
            args.max_lines = raw_args[i]
                .parse::<usize>()
                .map_err(|_| format!("invalid line limit: {}", raw_args[i]))?;
        } else if let Some(val) = arg.strip_prefix("--file=") {
            args.associated_file = Some(PathBuf::from(val));
        } else if arg == "--file" {
            i += 1;
            if i >= raw_args.len() {
                return Err("--file needs a value".to_string());
            }
            args.associated_file = Some(PathBuf::from(&raw_args[i]));
        } else if let Some(val) = arg.strip_prefix("--edit-target=") {
            args.associated_file = Some(PathBuf::from(val));
        } else if arg == "--edit-target" {
            i += 1;
            if i >= raw_args.len() {
                return Err("--edit-target needs a value".to_string());
            }
            args.associated_file = Some(PathBuf::from(&raw_args[i]));
        } else if let Some(val) = arg.strip_prefix("--reload-cmd=") {
            args.reload_cmd = Some(val.to_string());
        } else if arg == "--reload-cmd" {
            i += 1;
            if i >= raw_args.len() {
                return Err("--reload-cmd needs a value".to_string());
            }
            args.reload_cmd = Some(raw_args[i].clone());
        } else if let Some(pat) = arg.strip_prefix("+/") {
            args.start_pattern = Some(pat.to_string());
        } else if let Some(line_str) = arg.strip_prefix('+') {
            if line_str.is_empty() {
                return Err("invalid argument: +".to_string());
            }
            let n = line_str
                .parse::<usize>()
                .map_err(|_| format!("invalid line number: +{}", line_str))?;
            args.start_line = Some(n);
        } else if arg == "--" {
            i += 1;
            while i < raw_args.len() {
                args.files.push(PathBuf::from(&raw_args[i]));
                i += 1;
            }
            break;
        } else if arg.starts_with('-') {
            return Err(format!("unknown option: {}", arg));
        } else {
            args.files.push(PathBuf::from(arg));
        }

        i += 1;
    }

    Ok(args)
}
