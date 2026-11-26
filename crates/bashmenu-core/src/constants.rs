//! Shared localized documentation, help texts, and reference tables.
//!
//! All multi-line help modal bodies live in the YAML locale files
//! (`locales/en_us.yml`, `locales/es_es.yml`, etc.) under the `help.*` keys.

use crate::macro_engine::{interpolate_placeholders, MacroContext};
use crate::t;

/// Returns the localized placeholder reference body (`help.placeholders_body`).
pub fn placeholder_help_text() -> String {
    t!("help.placeholders_body")
}

/// Returns the localized Markdown syntax reference (`help.markdown_reference`).
pub fn markdown_reference_text() -> String {
    t!("help.markdown_reference")
}

/// Returns the localized BashEdit keybindings table (`help.bashedit_controls`).
pub fn bashedit_help_text() -> String {
    t!("help.bashedit_controls")
}

/// Returns `true` when running inside a Termux Android environment.
fn is_termux_env() -> bool {
    std::env::var("TERMUX_VERSION").is_ok()
        || std::env::var("PREFIX")
            .map(|p| p.contains("com.termux"))
            .unwrap_or(false)
}

/// Returns `true` if a placeholder token does not apply to the current OS/environment.
fn is_placeholder_na(token: &str) -> bool {
    match token {
        "{termux_properties}" | "{termux_storage}" => !is_termux_env(),
        "{powershell_profile}" => !cfg!(windows) && std::env::var("PROFILE").is_err(),
        "{appdata}" => !cfg!(windows) && std::env::var("APPDATA").is_err(),
        "{userprofile}" => !cfg!(windows) && std::env::var("USERPROFILE").is_err(),
        "{prefix}" => cfg!(windows) && std::env::var("PREFIX").is_err(),
        "{bashrc}" | "{bash_aliases}" | "{zshrc}" | "{profile}" | "{bash_profile}"
        | "{zprofile}" | "{shell_profile}" | "{vimrc}" | "{nanorc}" => cfg!(windows),
        _ => false,
    }
}

/// Renders [`placeholder_help_text`] with live expanded values in 3-column tables.
///
/// Platform-unavailable placeholders display `na`; empty/unset values display `null`.
pub fn render_placeholder_help_text(ctx: &MacroContext) -> String {
    let raw = placeholder_help_text();
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let parts: Vec<&str> = trimmed.split('|').collect();
            if parts.len() == 5 {
                let c0 = parts[1].trim();
                let c1 = parts[2].trim();
                let c2 = parts[3].trim();
                let is_delim = parts[1..4]
                    .iter()
                    .all(|p| p.trim().chars().all(|ch| ch == '-' || ch == ':'));
                let is_header = !c0.contains('{') && !c2.contains('{');
                if is_delim || is_header {
                    out.push(line.to_string());
                    continue;
                }

                let col0 = interpolate_placeholders(c0, ctx);
                let col1 = interpolate_placeholders(c1, ctx);
                let col2 = if is_placeholder_na(c2) {
                    "na".to_string()
                } else {
                    let expanded = interpolate_placeholders(c2, ctx);
                    let clean = expanded.replace(['\r', '\n'], " ").replace('|', "/");
                    if matches!(c2, "{battery}" | "{battery_stat}" | "{battery_status}")
                        && clean.trim().eq_ignore_ascii_case("n/a")
                    {
                        "na".to_string()
                    } else if clean.trim().is_empty() {
                        "null".to_string()
                    } else {
                        clean
                    }
                };
                out.push(format!("| {} | {} | {} |", col0, col1, col2));
                continue;
            }
        }
        out.push(interpolate_placeholders(line, ctx));
    }
    out.join("\n")
}
