//! YAML frontmatter parsing and tag extraction from Markdown cheatsheets.

use std::fs;
use std::path::Path;

/// Parses YAML frontmatter from a Markdown file and extracts lowercased tags.
///
/// Supports tags defined as YAML sequences (`[tag1, tag2]`), YAML list items,
/// or comma-delimited strings (`tag1, tag2`).
pub fn extract_tags_from_frontmatter(file_path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Vec::new();
    }

    // Split after leading '---'
    let rest = &trimmed[3..];
    let end_idx = match rest.find("---") {
        Some(i) => i,
        None => return Vec::new(),
    };

    let yaml_str = &rest[..end_idx];
    let parsed: serde_yml::Value = match serde_yml::from_str(yaml_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    if let serde_yml::Value::Mapping(map) = parsed {
        let key = serde_yml::Value::String("tags".to_string());
        if let Some(tags_val) = map.get(&key) {
            match tags_val {
                serde_yml::Value::Sequence(seq) => {
                    let mut tags = Vec::new();
                    for item in seq {
                        let val = match item {
                            serde_yml::Value::String(s) => s.trim().to_lowercase(),
                            serde_yml::Value::Number(n) => n.to_string(),
                            serde_yml::Value::Bool(b) => b.to_string(),
                            _ => String::new(),
                        };
                        if !val.is_empty() {
                            tags.push(val);
                        }
                    }
                    return tags;
                }
                serde_yml::Value::String(s) => {
                    return s
                        .split(',')
                        .map(|t| t.trim().to_lowercase())
                        .filter(|t| !t.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_tags_sequence() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "---\ntitle: Bash\ntags: [ bash, exists ]\n---\n# Content"
        )
        .expect("write");

        let tags = extract_tags_from_frontmatter(file.path());
        assert_eq!(tags, vec!["bash", "exists"]);
    }

    #[test]
    fn test_extract_tags_multiline_list() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "---\ntitle: Git\ntags:\n  - GitHub\n  - VCS\n---\n# Content"
        )
        .expect("write");

        let tags = extract_tags_from_frontmatter(file.path());
        assert_eq!(tags, vec!["github", "vcs"]);
    }

    #[test]
    fn test_extract_tags_string() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "---\ntags: python, cli, dev-tools\n---\n# Content").expect("write");

        let tags = extract_tags_from_frontmatter(file.path());
        assert_eq!(tags, vec!["python", "cli", "dev-tools"]);
    }

    #[test]
    fn test_extract_tags_missing_frontmatter() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "# Just Markdown Header\nNo frontmatter here.").expect("write");

        let tags = extract_tags_from_frontmatter(file.path());
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_no_tags_field() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "---\ntitle: Test\n---\n# Content").expect("write");

        let tags = extract_tags_from_frontmatter(file.path());
        assert!(tags.is_empty());
    }
}
