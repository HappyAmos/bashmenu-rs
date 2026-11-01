//! Hyperlink discovery, navigation, and modal selector.

use regex::Regex;
use std::process::Command;
use std::sync::LazyLock;

static OSC8_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\]8;[^\x1b\x07]*;([^\x1b\x07]+)\x1b\\(.*?)\x1b\]8;;\x1b\\").unwrap()
});
static MD_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());
static BARE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://[^\s)\]]+").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    pub url: String,
    pub text: String,
    pub line_no: usize, // 1-indexed
}

/// Scans document lines for OSC 8 hyperlinks, Markdown links, and bare URLs.
pub fn extract_links(lines: &[String]) -> Vec<DocumentLink> {
    let mut links = Vec::new();

    // 1. First scan for OSC 8 hyperlinks
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        for cap in OSC8_RE.captures_iter(line) {
            let url = cap[1].trim().to_string();
            let raw_text = &cap[2];
            let clean = crate::ansi::strip_ansi(raw_text).trim().to_string();
            let text = if clean.is_empty() { url.clone() } else { clean };
            links.push(DocumentLink { url, text, line_no });
        }
    }

    // 2. If no OSC 8 links found, scan for Markdown and bare URLs
    if links.is_empty() {
        for (idx, line) in lines.iter().enumerate() {
            let line_no = idx + 1;
            let clean = crate::ansi::strip_ansi(line);
            for cap in MD_LINK_RE.captures_iter(&clean) {
                let text = cap[1].trim().to_string();
                let url = cap[2].trim().to_string();
                links.push(DocumentLink { url, text, line_no });
            }
            for cap in BARE_URL_RE.captures_iter(&clean) {
                let url = cap[0].trim().to_string();
                links.push(DocumentLink {
                    url: url.clone(),
                    text: url,
                    line_no,
                });
            }
        }
    }

    // Deduplicate / consolidate consecutive identical URLs on the same line
    let mut consolidated: Vec<DocumentLink> = Vec::new();
    for link in links {
        if let Some(last) = consolidated.last_mut() {
            if last.url == link.url && last.line_no == link.line_no {
                last.text = format!("{} {}", last.text, link.text);
                continue;
            }
        }
        consolidated.push(link);
    }

    consolidated
}

/// Normalizes text for fuzzy heading/anchor matching.
pub fn normalize_for_anchor(s: &str) -> String {
    let clean = crate::ansi::strip_ansi(s);
    let mut result = String::new();
    for c in clean.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(' ');
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Resolves an internal anchor link `#target` to a 1-indexed line number in the document.
pub fn resolve_anchor(lines: &[String], anchor: &str, source_line: usize) -> Option<usize> {
    let norm_target = normalize_for_anchor(anchor.trim_start_matches('#'));
    if norm_target.is_empty() {
        return None;
    }

    let mut best_line = None;
    let mut best_score = -1;

    // Search from source_line forward first, then from beginning
    let ranges: [Box<dyn Iterator<Item = (usize, &String)>>; 2] = [
        Box::new(lines.iter().enumerate().skip(source_line)),
        Box::new(lines.iter().enumerate().take(source_line)),
    ];

    for range in ranges {
        for (idx, line) in range {
            let clean = crate::ansi::strip_ansi(line).trim().to_string();
            if clean.starts_with('•') || clean.starts_with('-') || clean.is_empty() {
                continue;
            }

            let norm_line = normalize_for_anchor(&clean);
            if norm_line.is_empty() {
                continue;
            }

            let mut score = 0;
            if norm_line.starts_with(&norm_target) || norm_target.starts_with(&norm_line) {
                score += 100;
            } else {
                let tw: Vec<&str> = norm_target.split_whitespace().collect();
                let lw: Vec<&str> = norm_line.split_whitespace().collect();
                if tw.len() >= 2 && lw.len() >= 2 && tw[..2] == lw[..2] {
                    score += 80;
                } else if !tw.is_empty() && !lw.is_empty() && tw[0] == lw[0] {
                    score += 40;
                }
                let common_count = tw.iter().filter(|w| lw.contains(w)).count();
                score += (common_count * 5) as i32;
            }

            if score > best_score && score >= 40 {
                best_score = score;
                best_line = Some(idx + 1);
                if score >= 90 {
                    return best_line;
                }
            }
        }
    }

    best_line
}

/// Opens an external URL or file in a background process.
pub fn open_external_url(url: &str) -> anyhow::Result<()> {
    if url.starts_with("http://") || url.starts_with("https://") {
        if std::path::Path::new("scripts/webopen.sh").is_file() {
            let _ = Command::new("scripts/webopen.sh").arg(url).spawn();
        } else if let Ok(mut child) = Command::new("xdg-open").arg(url).spawn() {
            let _ = child.wait();
        } else {
            let _ = Command::new("python3")
                .arg("-m")
                .arg("webbrowser")
                .arg(url)
                .spawn();
        }
    }
    Ok(())
}
