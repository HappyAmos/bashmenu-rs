//! Cheatsheet discovery, search prioritization, and listing utilities.

use crate::frontmatter::extract_tags_from_frontmatter;
use std::fs;
use std::path::{Path, PathBuf};

/// Recursively discovers and sorts all Markdown (`.md`) files in `root_dir`.
///
/// Files are sorted case-insensitively by their relative path from `root_dir`.
pub fn find_markdown_files(root_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_md_files(root_dir, &mut files);

    files.sort_by(|a, b| {
        let rel_a = a
            .strip_prefix(root_dir)
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let rel_b = b
            .strip_prefix(root_dir)
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        rel_a.cmp(&rel_b)
    });

    files
}

/// Helper recursive directory walker collecting `.md` files.
fn collect_md_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(&path, files);
        } else if path.is_file() {
            let is_md = path
                .extension()
                .map(|ext| ext.to_string_lossy().eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if is_md {
                files.push(path);
            }
        }
    }
}

/// Prints a list of cheatsheet files and tags in two neatly aligned columns.
pub fn print_file_list(files: &[PathBuf], root_dir: &Path) {
    if files.is_empty() {
        return;
    }

    let mut entries: Vec<(String, String)> = Vec::with_capacity(files.len());
    let mut max_len = 0;

    for file in files {
        let rel_path = match file.strip_prefix(root_dir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => file.to_string_lossy().to_string(),
        };
        let tags = extract_tags_from_frontmatter(file);
        let tags_str = if !tags.is_empty() {
            format!("[{}]", tags.join(", "))
        } else {
            String::new()
        };

        if rel_path.len() > max_len {
            max_len = rel_path.len();
        }
        entries.push((rel_path, tags_str));
    }

    let col_width = max_len + 4;

    for (rel_path, tags_str) in entries {
        if !tags_str.is_empty() {
            println!("{:<width$}{}", rel_path, tags_str, width = col_width);
        } else {
            println!("{}", rel_path);
        }
    }
}

/// Filters cheatsheet files matching a query string.
///
/// Matches against:
/// 1. Filename stem or relative path without `.md` extension
/// 2. YAML frontmatter tags
/// 3. Full-text content search
pub fn search_cheatsheets(files: &[PathBuf], root_dir: &Path, query: &str) -> Vec<PathBuf> {
    let q = query.to_lowercase();
    let mut matches = Vec::new();

    for file in files {
        let rel_path = match file.strip_prefix(root_dir) {
            Ok(p) => p,
            Err(_) => file.as_path(),
        };
        let rel_without_ext = rel_path.with_extension("");
        let rel_path_str = rel_without_ext.to_string_lossy().to_lowercase();
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Priority 1: Filename or relative path match
        if stem == q || rel_path_str == q {
            matches.push(file.clone());
            continue;
        }

        // Priority 2: Frontmatter tag match
        let tags = extract_tags_from_frontmatter(file);
        if tags.iter().any(|t| t == &q) {
            matches.push(file.clone());
            continue;
        }

        // Priority 3: Full-text content match
        if let Ok(content) = fs::read_to_string(file) {
            if content.to_lowercase().contains(&q) {
                matches.push(file.clone());
            }
        }
    }

    matches
}

/// Resolves the single highest-priority Markdown file matching `query`.
///
/// Priority order:
/// 1a. Fast exact relative path lookup (`<root>/<query>.md`)
/// 1b. Stem or relative path match in sorted file list
/// 2. Frontmatter tag match in sorted file list
/// 3. Full-text content match in sorted file list
pub fn find_single_match(files: &[PathBuf], root_dir: &Path, query: &str) -> Option<PathBuf> {
    let q = query.to_lowercase();

    // Priority 1a: Fast exact relative path lookup
    let exact_match = root_dir.join(format!("{q}.md"));
    if exact_match.is_file() {
        return Some(exact_match);
    }

    // Priority 1b: Stem or relative path match
    for file in files {
        let rel_path = match file.strip_prefix(root_dir) {
            Ok(p) => p,
            Err(_) => file.as_path(),
        };
        let rel_path_str = rel_path.with_extension("").to_string_lossy().to_lowercase();
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if stem == q || rel_path_str == q {
            return Some(file.clone());
        }
    }

    // Priority 2: Frontmatter tag match
    for file in files {
        let tags = extract_tags_from_frontmatter(file);
        if tags.iter().any(|t| t == &q) {
            return Some(file.clone());
        }
    }

    // Priority 3: Full-text content match
    for file in files {
        if let Ok(content) = fs::read_to_string(file) {
            if content.to_lowercase().contains(&q) {
                return Some(file.clone());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_find_markdown_files_and_sort() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        fs::write(root.join("zebra.md"), "# Zebra").expect("write");
        fs::write(root.join("apple.md"), "# Apple").expect("write");
        fs::write(root.join("ignore.txt"), "text").expect("write");

        let sub = root.join("sub");
        fs::create_dir(&sub).expect("create dir");
        fs::write(sub.join("banana.md"), "# Banana").expect("write");

        let files = find_markdown_files(root);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0], root.join("apple.md"));
        assert_eq!(files[1], sub.join("banana.md"));
        assert_eq!(files[2], root.join("zebra.md"));
    }

    #[test]
    fn test_search_cheatsheets_priorities() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        let f1 = root.join("bash.md");
        let mut w1 = fs::File::create(&f1).expect("create");
        writeln!(w1, "---\ntags: [shell]\n---\n# Bash").expect("write");

        let f2 = root.join("python.md");
        let mut w2 = fs::File::create(&f2).expect("create");
        writeln!(
            w2,
            "---\ntags: [scripting]\n---\nMentions bash shell script"
        )
        .expect("write");

        let files = find_markdown_files(root);

        // Search for 'bash': should match f1 (name) and f2 (content)
        let matches = search_cheatsheets(&files, root, "bash");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], f1);
        assert_eq!(matches[1], f2);

        // Search for tag 'shell': matches f1
        let matches_tag = search_cheatsheets(&files, root, "shell");
        assert_eq!(matches_tag.len(), 2); // f1 via tag, f2 via content "shell"

        // Single match for 'bash': f1 has priority
        let single = find_single_match(&files, root, "bash");
        assert_eq!(single, Some(f1));
    }
}
