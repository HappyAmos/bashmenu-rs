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
use cheatsheet::{find_markdown_files, find_single_match, print_file_list, search_cheatsheets};
use clap::{CommandFactory, Parser};
use config::{get_config_path, get_root_directory};
use render::display_markdown;
use std::process::exit;

/// CLI entrypoint for `cheat`.
fn main() {
    let args = CliArgs::parse();

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
                println!("No markdown files found under {}", root_dir.display());
                return;
            }
            print_file_list(&md_files, &root_dir);
            return;
        }

        // Case 2: Filtered listing mode (search)
        if let Some(query) = list_query {
            let matches = search_cheatsheets(&md_files, &root_dir, &query);
            if matches.is_empty() {
                eprintln!(
                    "No markdown file matched '{query}' under {}",
                    root_dir.display()
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
            let _ = CliArgs::command().print_help();
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
            eprintln!(
                "No markdown file matched '{query}' under {}",
                root_dir.display()
            );
            exit(1);
        }
    }
}
