//! `cheat` - A lightweight terminal-based cheatsheet manager.
//!
//! Searches, lists, and displays Markdown cheatsheets based on exact path
//! matches, YAML frontmatter tags, or full-text content search.

mod args;
mod cheatsheet;
mod config;
mod frontmatter;
mod render;

use args::CliArgs;
use bashmenu_core::t;
use cheatsheet::{find_markdown_files, find_single_match, print_file_list, search_cheatsheets};
use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches};
use config::{get_config_path, get_root_directory};
use render::display_markdown;
use std::process::exit;

/// Builds the localized `clap::Command` for `cheat`.
fn build_cli_command() -> Command {
    CliArgs::command()
        .about(t!("cli.cheat.about"))
        .help_template(t!("cli.help_template"))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .mut_arg("query", |a| a.help(t!("cli.cheat.query")))
        .mut_arg("list", |a| a.help(t!("cli.cheat.list")))
        .mut_arg("search", |a| a.help(t!("cli.cheat.search")))
        .mut_arg("directory", |a| a.help(t!("cli.cheat.directory")))
        .mut_arg("locale", |a| a.help(t!("cli.cheat.locale")))
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help(t!("cli.common.help")),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .action(ArgAction::Version)
                .help(t!("cli.common.version")),
        )
}

/// CLI entrypoint for `cheat`.
fn main() {
    bashmenu_core::preinit_locale_from_args(false);
    let matches = build_cli_command().get_matches();
    let args = CliArgs::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    if args.locale.is_some() {
        bashmenu_core::init_locale_from_env(args.locale.as_deref(), None);
    }

    // 1. Locate or create configuration file
    let config_path = match get_config_path() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("{err}");
            exit(1);
        }
    };

    // 2. Validate root cheatsheets directory
    let root_dir = match get_root_directory(&config_path) {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("{err}");
            exit(1);
        }
    };

    // 3. Directory path inspection mode (-d / --directory)
    if args.directory {
        println!("{}", root_dir.display());
        return;
    }

    // 4. Discover and sort all Markdown files in root directory
    let md_files = find_markdown_files(&root_dir);

    // 5. List / Search Mode (-l / --list / -s / --search)
    if args.is_list_mode() {
        let list_query = args.resolve_list_query();

        // Case 1: Unfiltered listing mode
        if list_query.is_none() {
            if md_files.is_empty() {
                let dir_str = root_dir.display().to_string();
                println!("{}", t!("cheat.no_files_found", dir => &dir_str));
                return;
            }
            print_file_list(&md_files, &root_dir);
            return;
        }

        // Case 2: Filtered listing mode (search)
        if let Some(query) = list_query {
            let matches = search_cheatsheets(&md_files, &root_dir, &query);
            if matches.is_empty() {
                let dir_str = root_dir.display().to_string();
                eprintln!(
                    "{}",
                    t!("cheat.no_match_query", query => &query, dir => &dir_str)
                );
                exit(1);
            }
            print_file_list(&matches, &root_dir);
            return;
        }
    }

    // 6. Normal Display Mode
    let query_arg = match args.query {
        Some(q) if !q.trim().is_empty() => q,
        _ => {
            let _ = build_cli_command().print_help();
            println!();
            exit(1);
        }
    };

    let query = query_arg.to_lowercase();
    let query = query.strip_suffix(".md").unwrap_or(&query);

    match find_single_match(&md_files, &root_dir, query) {
        Some(file) => {
            display_markdown(&file);
        }
        None => {
            let dir_str = root_dir.display().to_string();
            eprintln!(
                "{}",
                t!("cheat.no_match_query", query => query, dir => &dir_str)
            );
            exit(1);
        }
    }
}
