//! Command-line argument definitions for the `cheat` tool.

use clap::Parser;

/// Command-line arguments for cheatsheet search, listing, and rendering.
#[derive(Parser, Debug, Clone)]
#[command(name = "cheat", about = "Query and display cheatsheets.", version)]
pub struct CliArgs {
    /// Search query to display markdown file.
    #[arg(value_name = "query")]
    pub query: Option<String>,

    /// List all files with tags, or list matching files for an optional query.
    #[arg(
        short = 'l',
        long = "list",
        value_name = "LIST",
        num_args = 0..=1,
        default_missing_value = ""
    )]
    pub list: Option<String>,

    /// Search cheatsheets and list matching files with tags.
    #[arg(
        short = 's',
        long = "search",
        value_name = "SEARCH",
        num_args = 0..=1,
        default_missing_value = ""
    )]
    pub search: Option<String>,

    /// Print the configured cheatsheets directory path.
    #[arg(short = 'd', long = "directory")]
    pub directory: bool,
}

impl CliArgs {
    /// Determines whether the user requested list or search mode.
    pub fn is_list_mode(&self) -> bool {
        self.list.is_some() || self.search.is_some()
    }

    /// Resolves the optional search/filter query for list mode.
    ///
    /// Priority:
    /// 1. Value passed directly to `-s` / `--search`
    /// 2. Value passed directly to `-l` / `--list`
    /// 3. Positional query argument
    pub fn resolve_list_query(&self) -> Option<String> {
        let raw = if let Some(ref s) = self.search {
            if !s.is_empty() {
                Some(s.clone())
            } else {
                self.query.clone()
            }
        } else if let Some(ref l) = self.list {
            if !l.is_empty() {
                Some(l.clone())
            } else {
                self.query.clone()
            }
        } else {
            None
        };

        raw.map(|q| {
            let lower = q.to_lowercase();
            if let Some(stripped) = lower.strip_suffix(".md") {
                stripped.to_string()
            } else {
                lower
            }
        })
    }
}
