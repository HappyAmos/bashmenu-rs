//! Interactive Markdown (`interactive: true`) block parsing, UI-RPC protocol,
//! and `bashmenu-bash` macro transpilation.

use crate::macro_engine::{
    find_message_macro_span, parse_inline_macro_fields, posix_shell_quote, scan_balanced_macro_end,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Environment variable carrying the Unix domain socket path for live UI-RPC.
pub const UI_RPC_SOCKET_ENV: &str = "BASHMENU_IPC_SOCK";

/// Specification for an embedded `{input:...}` form widget inside an
/// interactive Markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFieldSpec {
    /// Bash variable name assigned the field value on form submission (`id=...`).
    pub id: String,
    /// Pre-populated default text (`default="..."`, default `""`).
    pub default_value: String,
    /// Optional fixed column width (`width=XX`); `None` fills between margins.
    pub width: Option<u16>,
    /// Multi-line text area (`multiline=true`) vs single-line (`false`, default).
    pub multiline: bool,
    /// Show scrollbar when `multiline` is true (`scrollbar=true` by default;
    /// ignored when `multiline` is false).
    pub scrollbar: bool,
    /// Visible row height when `multiline` is true (`height=N`, default `5`).
    pub height: u16,
}

impl Default for InputFieldSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            default_value: String::new(),
            width: None,
            multiline: false,
            scrollbar: true,
            height: 5,
        }
    }
}

/// Button role for an interactive Markdown `{button:...}` control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonType {
    /// Submits/continues (`type="submit"`, default label `"Submit"`).
    #[default]
    Submit,
    /// Pauses/continues (`type="ok"`, default label `"OK"`).
    Ok,
    /// Exits/cancels the interactive document (`type="cancel"`, default label `"Cancel"`).
    Cancel,
}

/// Specification for an interactive `{button:...}` control in Markdown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ButtonSpec {
    /// Button role (`submit`, `ok`, `cancel`).
    pub button_type: ButtonType,
    /// Optional reserved code block ID (`action="exampleCodeBlock"`).
    pub action: Option<String>,
    /// Optional custom label override (`text="..."` / `label="..."`).
    pub label: Option<String>,
}

impl ButtonSpec {
    /// Returns the visible label text for this button.
    pub fn display_label(&self) -> &str {
        if let Some(ref custom) = self.label {
            if !custom.is_empty() {
                return custom.as_str();
            }
        }
        match self.button_type {
            ButtonType::Submit => "Submit",
            ButtonType::Ok => "OK",
            ButtonType::Cancel => "Cancel",
        }
    }
}

/// A named executable code block held in reserve (`bashmenu-bash:id="..."`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBlock {
    /// Language identifier after `bashmenu-` (lowercase, e.g. `"bash"`).
    pub language: String,
    /// Block identifier referenced by `{button: action="..."}`.
    pub id: String,
    /// Raw script source code inside the fence.
    pub code: String,
}

/// Layout orientation for a `{radio:...}` option group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadioLayout {
    /// Stacked vertically, one option per row (default).
    #[default]
    Vertical,
    /// Rendered inline horizontally on a single row.
    Horizontal,
}

/// Specification for an embedded `{radio:...}` single-choice widget.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RadioSpec {
    /// Bash variable identifier exported on submit and via `{id:<name>}`.
    pub id: String,
    /// Ordered selectable option labels/values.
    pub options: Vec<String>,
    /// Index of the initially selected option (`0..options.len()`).
    pub default_index: usize,
    /// Visual layout (`Vertical` or `Horizontal`).
    pub layout: RadioLayout,
}

impl RadioSpec {
    /// Returns the option text for `idx` (or `""` if out of bounds).
    pub fn value_at(&self, idx: usize) -> &str {
        self.options.get(idx).map(|s| s.as_str()).unwrap_or("")
    }
}

/// Specification for an embedded `{checkbox:...}` boolean toggle widget.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckboxSpec {
    /// Bash variable identifier exported as `"true"` or `"false"`.
    pub id: String,
    /// Visible label displayed beside `[x]` / `[ ]`.
    pub label: String,
    /// Initial checked state (default `false`).
    pub default_checked: bool,
}

/// A rendered element inside an interactive Markdown form document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownFormBlock {
    /// Standard static Markdown text (may contain live `{id:<name>}` placeholders).
    Text(String),
    /// Embedded interactive `{input:...}` widget with optional same-line label prefix.
    Input {
        /// Preceding text on the same line for single-line inline inputs
        /// (e.g. `"What is your name: "`). `None` for standalone or multi-line inputs.
        prefix: Option<String>,
        /// Parsed input widget specification.
        spec: InputFieldSpec,
    },
    /// Embedded `{radio:...}` single-choice option group.
    Radio(RadioSpec),
    /// Embedded `{checkbox:...}` boolean toggle widget.
    Checkbox(CheckboxSpec),
    /// One or more `{button:...}` controls rendered side-by-side on a single row.
    Buttons(Vec<ButtonSpec>),
}

/// A parsed multi-page interactive Markdown document with reserved named
/// `bashmenu-<lang>:id="..."` script blocks and `{clear}`-delimited pages.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InteractiveMarkdownDoc {
    /// Named code blocks held in reserve (never rendered into visible Markdown).
    pub reserved_scripts: std::collections::HashMap<String, ScriptBlock>,
    /// Ordered pages separated by `{clear}` (always at least 1 page if non-empty).
    pub pages: Vec<Vec<MarkdownFormBlock>>,
}

/// A segment of an interactive Markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownSegment {
    /// Standard static Markdown text rendered after script execution.
    Markdown(String),
    /// An executable fenced code block (`bashmenu-<lang>`, e.g. `bashmenu-bash`).
    Script {
        /// Language identifier after `bashmenu-` (lowercase, e.g. `"bash"`).
        language: String,
        /// Raw script source code inside the fence.
        code: String,
    },
}

/// Request sent from an executing `bashmenu-<lang>` script to the BashMenu TUI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum UiRpcRequest {
    /// Opens an interactive single-line text input modal (`{param:...}`).
    Param {
        /// Optional modal title (empty string omits the title badge).
        #[serde(default)]
        title: String,
        /// Prompt message displayed above the input box.
        #[serde(default)]
        text: String,
        /// Initial default value pre-populated in the input box.
        #[serde(default)]
        default_value: String,
        /// Whether input characters are masked (password mode).
        #[serde(default)]
        masked: bool,
    },
    /// Opens a centered popup message modal with OK and `[X]` (`{message:...}`).
    Message {
        /// Optional modal title (empty string omits the title badge).
        #[serde(default)]
        title: String,
        /// Message body text.
        #[serde(default)]
        text: String,
    },
    /// Displays a non-blocking toast notification (`{toast:...}`).
    Toast {
        /// Toast body message.
        #[serde(default)]
        text: String,
        /// Optional duration in seconds.
        #[serde(default)]
        duration_secs: Option<f64>,
    },
    /// Expands static BashMenu placeholders using the host's `MacroContext`.
    Placeholder {
        /// Raw template string containing `{user}`, `{home}`, etc.
        text: String,
    },
}

/// Response returned from the BashMenu TUI back to the blocked script.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiRpcResponse {
    /// True if the request succeeded (false on error or cancellation).
    pub ok: bool,
    /// Returned string value (e.g. user input from `Param` or expanded placeholder).
    #[serde(default)]
    pub value: String,
    /// True if the user dismissed the modal via Esc or `[X]` cancellation.
    #[serde(default)]
    pub cancelled: bool,
}

/// Splits a Markdown body into static [`MarkdownSegment::Markdown`] chunks and
/// executable [`MarkdownSegment::Script`] blocks (`bashmenu-<lang>` fences).
///
/// Standard code blocks (e.g. ```` ```bash ````) are kept intact inside
/// [`MarkdownSegment::Markdown`].
pub fn parse_interactive_markdown(body: &str) -> Vec<MarkdownSegment> {
    let mut segments: Vec<MarkdownSegment> = Vec::new();
    let mut md_buf = String::new();
    let mut lines = body.split_inclusive('\n');

    while let Some(line) = lines.next() {
        let trimmed_start = line.trim_start_matches(' ');
        let tick_count = trimmed_start.bytes().take_while(|&b| b == b'`').count();
        if tick_count >= 3 {
            let after_ticks = trimmed_start[tick_count..].trim();
            let info_token = after_ticks.split_whitespace().next().unwrap_or("");
            let info_lower = info_token.to_ascii_lowercase();
            if let Some(lang) = info_lower.strip_prefix("bashmenu-") {
                if !lang.is_empty() {
                    if !md_buf.is_empty() {
                        segments.push(MarkdownSegment::Markdown(std::mem::take(&mut md_buf)));
                    }
                    let mut code_buf = String::new();
                    for inner_line in lines.by_ref() {
                        let inner_trim = inner_line.trim();
                        if inner_trim.len() >= tick_count && inner_trim.bytes().all(|b| b == b'`') {
                            break;
                        }
                        code_buf.push_str(inner_line);
                    }
                    segments.push(MarkdownSegment::Script {
                        language: lang.to_string(),
                        code: code_buf,
                    });
                    continue;
                }
            }
        }
        md_buf.push_str(line);
    }

    if !md_buf.is_empty() {
        segments.push(MarkdownSegment::Markdown(md_buf));
    }

    segments
}

/// Returns true if `name` is a valid POSIX/Bash variable identifier
/// (`^[a-zA-Z_][a-zA-Z0-9_]*$`).
pub fn is_valid_bash_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Parses a single quoted (`"`, `'`, `` ` ``) value from the start of `s`.
fn parse_input_quoted_value(s: &str) -> Option<(String, &str)> {
    let mut chars = s.char_indices();
    let (_, delim) = chars.next()?;
    if !matches!(delim, '"' | '\'' | '`') {
        return None;
    }
    let mut out = String::new();
    while let Some((off, ch)) = chars.next() {
        if ch == '\\' {
            if let Some((_, next_ch)) = chars.next() {
                match next_ch {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '\\' => out.push('\\'),
                    c if c == delim => out.push(c),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
            continue;
        }
        if ch == delim {
            return Some((out, &s[off + ch.len_utf8()..]));
        }
        out.push(ch);
    }
    None
}

/// Parses colon-separated `key = value` pairs inside `{input:...}`.
fn parse_input_macro_fields(inner: &str) -> InputFieldSpec {
    let mut spec = InputFieldSpec::default();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(stripped) = rest.strip_prefix(':') {
            rest = stripped.trim_start();
            continue;
        }
        if rest.is_empty() {
            break;
        }

        let mut matched_key: Option<(&str, &str)> = None;
        for key in ["default", "id", "width", "multiline", "scrollbar", "height"] {
            if let Some(after) = rest.strip_prefix(key) {
                let after_ws = after.trim_start();
                if after_ws.is_empty() || after_ws.starts_with('=') || after_ws.starts_with(':') {
                    matched_key = Some((key, after_ws));
                    break;
                }
            }
        }

        let Some((key, after_key)) = matched_key else {
            // Skip unknown token up to next `:`
            if let Some(colon) = rest.find(':') {
                rest = &rest[colon + 1..];
            } else {
                break;
            }
            continue;
        };

        if let Some(after_eq) = after_key.strip_prefix('=') {
            let val_start = after_eq.trim_start();
            let (val, next_rest) = if let Some((qval, rem)) = parse_input_quoted_value(val_start) {
                (qval, rem)
            } else {
                let mut depth = 0usize;
                let mut end_off = val_start.len();
                for (off, ch) in val_start.char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        ':' if depth == 0 => {
                            end_off = off;
                            break;
                        }
                        _ => {}
                    }
                }
                (
                    val_start[..end_off].trim().replace("\\n", "\n"),
                    &val_start[end_off..],
                )
            };

            match key {
                "default" => spec.default_value = val,
                "id" => spec.id = val.trim().to_string(),
                "width" => {
                    if let Ok(w) = val.trim().parse::<u16>() {
                        if w > 0 {
                            spec.width = Some(w);
                        }
                    }
                }
                "multiline" => {
                    spec.multiline = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "scrollbar" => {
                    spec.scrollbar = !val.eq_ignore_ascii_case("false") && val != "0";
                }
                "height" => {
                    if let Ok(h) = val.trim().parse::<u16>() {
                        if h > 0 {
                            spec.height = h.clamp(1, 50);
                        }
                    }
                }
                _ => {}
            }

            rest = next_rest.trim_start();
            if let Some(stripped) = rest.strip_prefix(':') {
                rest = stripped;
            }
        } else {
            match key {
                "multiline" => spec.multiline = true,
                "scrollbar" => spec.scrollbar = true,
                _ => {}
            }
            rest = after_key.trim_start();
            if let Some(stripped) = rest.strip_prefix(':') {
                rest = stripped;
            }
        }
    }

    spec
}

/// Finds the byte bounds and parsed [`InputFieldSpec`] of the first `{input:...}`
/// (or `{input}`) macro in `text`, skipping double-brace escaped `{{input...}}`.
pub fn find_input_macro_span(text: &str) -> Option<(usize, usize, InputFieldSpec)> {
    const PREFIX: &str = "{input";
    let bytes = text.as_bytes();
    let mut search_from = 0usize;

    while let Some(rel) = text[search_from..].find(PREFIX) {
        let start = search_from + rel;
        if start > 0 && bytes[start - 1] == b'{' {
            search_from = start + PREFIX.len();
            continue;
        }
        let after = &text[start + PREFIX.len()..];
        if let Some(first_ch) = after.chars().next() {
            if first_ch == '}' || first_ch == ':' {
                if let Some(end) = scan_balanced_macro_end(text, start) {
                    let inner = if first_ch == ':' {
                        &text[start + PREFIX.len() + 1..end - 1]
                    } else {
                        ""
                    };
                    return Some((start, end, parse_input_macro_fields(inner)));
                }
            }
        }
        search_from = start + PREFIX.len();
    }

    None
}

/// Parses a Markdown document body (ignoring `bashmenu-<lang>` script blocks)
/// into ordered [`MarkdownFormBlock`]s of static text and `{input:...}` widgets.
///
/// Single-line `{input:...}` widgets following text on the same line preserve
/// that text as `prefix` so label and input render inline. `{input:...}` inside
/// standard non-`bashmenu-*` code fences stays literal Markdown text.
pub fn parse_markdown_form_blocks(body: &str) -> Vec<MarkdownFormBlock> {
    let mut blocks: Vec<MarkdownFormBlock> = Vec::new();
    for seg in parse_interactive_markdown(body) {
        if let MarkdownSegment::Markdown(md_chunk) = seg {
            let mut text_buf = String::new();
            let mut in_code_fence: Option<usize> = None;

            for raw_line in md_chunk.split_inclusive('\n') {
                let line = raw_line.trim_end_matches(['\r', '\n']);
                let trimmed_start = line.trim_start_matches(' ');
                let tick_count = trimmed_start.bytes().take_while(|&b| b == b'`').count();

                if let Some(open_ticks) = in_code_fence {
                    text_buf.push_str(raw_line);
                    let trimmed = line.trim();
                    if trimmed.len() >= open_ticks && trimmed.bytes().all(|b| b == b'`') {
                        in_code_fence = None;
                    }
                    continue;
                } else if tick_count >= 3 {
                    in_code_fence = Some(tick_count);
                    text_buf.push_str(raw_line);
                    continue;
                }

                if let Some((start, end, spec)) = find_input_macro_span(line) {
                    if !text_buf.is_empty() {
                        let flushed = text_buf.trim_matches(['\r', '\n']);
                        if !flushed.trim().is_empty() {
                            blocks.push(MarkdownFormBlock::Text(flushed.to_string()));
                        }
                        text_buf.clear();
                    }

                    let before = &line[..start];
                    let after = &line[end..];
                    if !spec.multiline && !before.trim().is_empty() {
                        blocks.push(MarkdownFormBlock::Input {
                            prefix: Some(before.to_string()),
                            spec,
                        });
                    } else {
                        if !before.trim().is_empty() {
                            blocks.push(MarkdownFormBlock::Text(before.trim_end().to_string()));
                        }
                        blocks.push(MarkdownFormBlock::Input { prefix: None, spec });
                    }
                    if !after.trim().is_empty() {
                        text_buf.push_str(after.trim_start());
                        text_buf.push('\n');
                    }
                } else {
                    text_buf.push_str(raw_line);
                }
            }

            if !text_buf.is_empty() {
                let flushed = text_buf.trim_matches(['\r', '\n']);
                if !flushed.trim().is_empty() {
                    blocks.push(MarkdownFormBlock::Text(flushed.to_string()));
                }
            }
        }
    }
    blocks
}

/// Replaces `{id:<name>}` placeholders in `text` using `vars` (resolving unknown
/// IDs to `""`), while skipping double-brace escaped `{{id:...}}` and lines
/// inside fenced code blocks.
pub fn substitute_id_placeholders(
    text: &str,
    vars: &std::collections::HashMap<String, String>,
) -> String {
    if !text.contains("{id:") {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut in_code_fence: Option<usize> = None;

    for raw_line in text.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed_start = line.trim_start_matches(' ');
        let tick_count = trimmed_start.bytes().take_while(|&b| b == b'`').count();

        if let Some(open_ticks) = in_code_fence {
            out.push_str(raw_line);
            let trimmed = line.trim();
            if trimmed.len() >= open_ticks && trimmed.bytes().all(|b| b == b'`') {
                in_code_fence = None;
            }
            continue;
        } else if tick_count >= 3 {
            in_code_fence = Some(tick_count);
            out.push_str(raw_line);
            continue;
        }

        let mut cursor = 0usize;
        let bytes = raw_line.as_bytes();
        while let Some(rel) = raw_line[cursor..].find("{id:") {
            let start = cursor + rel;
            if start > 0 && bytes[start - 1] == b'{' {
                out.push_str(&raw_line[cursor..start + 4]);
                cursor = start + 4;
                continue;
            }
            if let Some(end_rel) = raw_line[start + 4..].find('}') {
                let end = start + 4 + end_rel + 1;
                let var_name = raw_line[start + 4..start + 4 + end_rel].trim();
                out.push_str(&raw_line[cursor..start]);
                if let Some(val) = vars.get(var_name) {
                    out.push_str(val);
                }
                cursor = end;
            } else {
                break;
            }
        }
        out.push_str(&raw_line[cursor..]);
    }

    out
}

/// Parses a `{button:...}` macro inner payload into a [`ButtonSpec`].
fn parse_button_macro_fields(inner: &str) -> ButtonSpec {
    let mut spec = ButtonSpec::default();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(stripped) = rest.strip_prefix(':') {
            rest = stripped.trim_start();
            continue;
        }
        if rest.is_empty() {
            break;
        }

        let mut matched_key: Option<(&str, &str)> = None;
        for key in ["type", "action", "text", "label"] {
            if let Some(after) = rest.strip_prefix(key) {
                let after_ws = after.trim_start();
                if after_ws.is_empty() || after_ws.starts_with('=') || after_ws.starts_with(':') {
                    matched_key = Some((key, after_ws));
                    break;
                }
            }
        }

        let Some((key, after_key)) = matched_key else {
            if let Some(colon) = rest.find(':') {
                rest = &rest[colon + 1..];
            } else {
                break;
            }
            continue;
        };

        if let Some(after_eq) = after_key.strip_prefix('=') {
            let val_start = after_eq.trim_start();
            let (val, next_rest) = if let Some((qval, rem)) = parse_input_quoted_value(val_start) {
                (qval, rem)
            } else {
                let mut depth = 0usize;
                let mut end_off = val_start.len();
                for (off, ch) in val_start.char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        ':' if depth == 0 => {
                            end_off = off;
                            break;
                        }
                        _ => {}
                    }
                }
                (
                    val_start[..end_off].trim().to_string(),
                    &val_start[end_off..],
                )
            };

            match key {
                "type" => {
                    let lower = val.trim().to_ascii_lowercase();
                    spec.button_type = match lower.as_str() {
                        "ok" => ButtonType::Ok,
                        "cancel" => ButtonType::Cancel,
                        _ => ButtonType::Submit,
                    };
                }
                "action" => {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        spec.action = Some(trimmed.to_string());
                    }
                }
                "text" | "label" => {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        spec.label = Some(trimmed.to_string());
                    }
                }
                _ => {}
            }

            rest = next_rest.trim_start();
            if let Some(stripped) = rest.strip_prefix(':') {
                rest = stripped;
            }
        } else {
            rest = after_key.trim_start();
            if let Some(stripped) = rest.strip_prefix(':') {
                rest = stripped;
            }
        }
    }

    spec
}

/// Finds the byte bounds and parsed [`ButtonSpec`] of the first `{button:...}`
/// (or `{button}`) macro in `text`, skipping double-brace escaped `{{button...}}`.
pub fn find_button_macro_span(text: &str) -> Option<(usize, usize, ButtonSpec)> {
    const PREFIX: &str = "{button";
    let bytes = text.as_bytes();
    let mut search_from = 0usize;

    while let Some(rel) = text[search_from..].find(PREFIX) {
        let start = search_from + rel;
        if start > 0 && bytes[start - 1] == b'{' {
            search_from = start + PREFIX.len();
            continue;
        }
        let after = &text[start + PREFIX.len()..];
        if let Some(first_ch) = after.chars().next() {
            if first_ch == '}' || first_ch == ':' {
                if let Some(end) = scan_balanced_macro_end(text, start) {
                    let inner = if first_ch == ':' {
                        &text[start + PREFIX.len() + 1..end - 1]
                    } else {
                        ""
                    };
                    return Some((start, end, parse_button_macro_fields(inner)));
                }
            }
        }
        search_from = start + PREFIX.len();
    }

    None
}

/// Parses a `bashmenu-<lang>:id="..."` (or `bashmenu-<lang>:<id>`) fence header.
/// Returns `Some((language, optional_id))` for any `bashmenu-<lang>` fence.
fn parse_bashmenu_fence_header(after_ticks: &str) -> Option<(String, Option<String>)> {
    let trimmed = after_ticks.trim();
    let lower_prefix = trimmed.get(..9)?.to_ascii_lowercase();
    if lower_prefix != "bashmenu-" {
        return None;
    }
    let rest = &trimmed[9..];
    // Language ends at `:`, whitespace, or end of string
    let lang_end = rest
        .find(|c: char| c == ':' || c.is_ascii_whitespace())
        .unwrap_or(rest.len());
    let lang = rest[..lang_end].to_ascii_lowercase();
    if lang.is_empty() {
        return None;
    }
    let after_lang = &rest[lang_end..];
    if let Some(after_colon) = after_lang.strip_prefix(':') {
        let spec_str = after_colon.trim();
        let id = if let Some(after_id) = spec_str.strip_prefix("id") {
            let after_ws = after_id.trim_start();
            if let Some(after_eq) = after_ws.strip_prefix('=') {
                let val_start = after_eq.trim_start();
                if let Some((qval, _)) = parse_input_quoted_value(val_start) {
                    qval.trim().to_string()
                } else {
                    val_start
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim_matches(|c| matches!(c, '"' | '\'' | '`'))
                        .to_string()
                }
            } else {
                String::new()
            }
        } else {
            spec_str
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c| matches!(c, '"' | '\'' | '`'))
                .to_string()
        };
        let id_opt = if id.is_empty() { None } else { Some(id) };
        return Some((lang, id_opt));
    }
    Some((lang, None))
}

/// Parses an interactive Markdown document into reserved named
/// `bashmenu-<lang>:id="..."` code blocks and `{clear}`-delimited pages of
/// [`MarkdownFormBlock`]s (`Text`, `Input`, `Buttons`).
pub fn parse_interactive_markdown_doc(body: &str) -> InteractiveMarkdownDoc {
    let mut reserved_scripts = std::collections::HashMap::new();
    let mut pages: Vec<Vec<MarkdownFormBlock>> = Vec::new();
    let mut current_page: Vec<MarkdownFormBlock> = Vec::new();
    let mut text_buf = String::new();
    let mut in_std_fence: Option<usize> = None;

    let flush_text = |text_buf: &mut String, page: &mut Vec<MarkdownFormBlock>| {
        if !text_buf.is_empty() {
            let flushed = text_buf.trim_matches(['\r', '\n']);
            if !flushed.trim().is_empty() {
                page.push(MarkdownFormBlock::Text(flushed.to_string()));
            }
            text_buf.clear();
        }
    };

    let mut lines = body.split_inclusive('\n');
    while let Some(raw_line) = lines.next() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed_start = line.trim_start_matches(' ');
        let tick_count = trimmed_start.bytes().take_while(|&b| b == b'`').count();

        if let Some(open_ticks) = in_std_fence {
            text_buf.push_str(raw_line);
            let trimmed = line.trim();
            if trimmed.len() >= open_ticks && trimmed.bytes().all(|b| b == b'`') {
                in_std_fence = None;
            }
            continue;
        }

        if tick_count >= 3 {
            let after_ticks = &trimmed_start[tick_count..];
            if let Some((lang, id_opt)) = parse_bashmenu_fence_header(after_ticks) {
                let mut code_buf = String::new();
                for inner_line in lines.by_ref() {
                    let inner_trim = inner_line.trim();
                    if inner_trim.len() >= tick_count && inner_trim.bytes().all(|b| b == b'`') {
                        break;
                    }
                    code_buf.push_str(inner_line);
                }
                if let Some(id) = id_opt {
                    reserved_scripts.insert(
                        id.clone(),
                        ScriptBlock {
                            language: lang,
                            id,
                            code: code_buf,
                        },
                    );
                }
                continue;
            } else {
                in_std_fence = Some(tick_count);
                text_buf.push_str(raw_line);
                continue;
            }
        }

        // Check for standalone `{clear}` page break (not `{{clear}}`)
        if line.trim() == "{clear}" {
            flush_text(&mut text_buf, &mut current_page);
            pages.push(std::mem::take(&mut current_page));
            continue;
        }

        // Check for `{button:...}` row
        if find_button_macro_span(line).is_some() {
            flush_text(&mut text_buf, &mut current_page);
            let mut buttons = Vec::new();
            let mut cursor = 0usize;
            let mut leftover = String::new();
            while cursor < line.len() {
                if let Some((rel_s, rel_e, bspec)) = find_button_macro_span(&line[cursor..]) {
                    let before = &line[cursor..cursor + rel_s];
                    if !before.trim().is_empty() {
                        leftover.push_str(before);
                    }
                    buttons.push(bspec);
                    cursor += rel_e;
                } else {
                    let tail = &line[cursor..];
                    if !tail.trim().is_empty() {
                        leftover.push_str(tail);
                    }
                    break;
                }
            }
            if !leftover.trim().is_empty() {
                current_page.push(MarkdownFormBlock::Text(leftover.trim().to_string()));
            }
            if !buttons.is_empty() {
                current_page.push(MarkdownFormBlock::Buttons(buttons));
            }
            continue;
        }

        // Check for `{radio:...}` widget
        if let Some((start, end, spec)) = find_radio_macro_span(line) {
            flush_text(&mut text_buf, &mut current_page);
            let before = &line[..start];
            let after = &line[end..];
            if !before.trim().is_empty() {
                current_page.push(MarkdownFormBlock::Text(before.trim_end().to_string()));
            }
            current_page.push(MarkdownFormBlock::Radio(spec));
            if !after.trim().is_empty() {
                text_buf.push_str(after.trim_start());
                text_buf.push('\n');
            }
            continue;
        }

        // Check for `{checkbox:...}` widget
        if let Some((start, end, spec)) = find_checkbox_macro_span(line) {
            flush_text(&mut text_buf, &mut current_page);
            let before = &line[..start];
            let after = &line[end..];
            if !before.trim().is_empty() {
                current_page.push(MarkdownFormBlock::Text(before.trim_end().to_string()));
            }
            current_page.push(MarkdownFormBlock::Checkbox(spec));
            if !after.trim().is_empty() {
                text_buf.push_str(after.trim_start());
                text_buf.push('\n');
            }
            continue;
        }

        // Check for `{input:...}` widget
        if let Some((start, end, spec)) = find_input_macro_span(line) {
            flush_text(&mut text_buf, &mut current_page);
            let before = &line[..start];
            let after = &line[end..];
            if !spec.multiline && !before.trim().is_empty() {
                current_page.push(MarkdownFormBlock::Input {
                    prefix: Some(before.to_string()),
                    spec,
                });
            } else {
                if !before.trim().is_empty() {
                    current_page.push(MarkdownFormBlock::Text(before.trim_end().to_string()));
                }
                current_page.push(MarkdownFormBlock::Input { prefix: None, spec });
            }
            if !after.trim().is_empty() {
                text_buf.push_str(after.trim_start());
                text_buf.push('\n');
            }
            continue;
        }

        text_buf.push_str(raw_line);
    }

    flush_text(&mut text_buf, &mut current_page);
    if !current_page.is_empty() || pages.is_empty() {
        pages.push(current_page);
    }

    InteractiveMarkdownDoc {
        reserved_scripts,
        pages,
    }
}

/// Splits colon-separated arguments inside `{radio:...}` or `{checkbox:...}`,
/// preserving quoted strings (`"`, `'`, `` ` ``) that may contain colons.
/// Returns `(raw_segment, was_quoted)` pairs.
fn split_colon_macro_args(inner: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(stripped) = rest.strip_prefix(':') {
            rest = stripped.trim_start();
            continue;
        }
        if rest.is_empty() {
            break;
        }

        if matches!(rest.as_bytes().first(), Some(b'"' | b'\'' | b'`')) {
            if let Some((val, tail)) = parse_input_quoted_value(rest) {
                out.push((val, true));
                rest = tail;
                continue;
            }
        }

        // Scan until next unquoted `:`
        let mut quote: Option<char> = None;
        let mut escaped = false;
        let mut split_at = rest.len();
        for (idx, ch) in rest.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && quote.is_some() {
                escaped = true;
                continue;
            }
            if let Some(q) = quote {
                if ch == q {
                    quote = None;
                }
            } else if matches!(ch, '"' | '\'' | '`') {
                quote = Some(ch);
            } else if ch == ':' {
                split_at = idx;
                break;
            }
        }
        let seg = rest[..split_at].trim();
        if !seg.is_empty() {
            out.push((seg.to_string(), false));
        }
        rest = if split_at < rest.len() {
            &rest[split_at + 1..]
        } else {
            ""
        };
    }

    out
}

/// Parses an optional `key = value` segment (unquoting `value` if quoted).
fn parse_kv_arg(seg: &str, allowed_keys: &[&str]) -> Option<(String, String)> {
    for &key in allowed_keys {
        if let Some(after) = seg.strip_prefix(key) {
            let after_ws = after.trim_start();
            if let Some(val_part) = after_ws.strip_prefix('=') {
                let val_trim = val_part.trim();
                let val = if matches!(val_trim.as_bytes().first(), Some(b'"' | b'\'' | b'`')) {
                    parse_input_quoted_value(val_trim)
                        .map(|(v, _)| v)
                        .unwrap_or_else(|| val_trim.to_string())
                } else {
                    val_trim.to_string()
                };
                return Some((key.to_string(), val));
            }
        }
    }
    None
}

/// Parses `{radio:...}` inner fields supporting both positional shorthand
/// (`{radio: rdoValue: "Example 1": "Example 2"}`) and key-value attributes
/// (`id="..."`, `default="..."`, `layout="vertical"|"horizontal"`).
fn parse_radio_macro_fields(inner: &str) -> RadioSpec {
    let mut spec = RadioSpec::default();
    let mut default_val: Option<String> = None;

    for (seg, quoted) in split_colon_macro_args(inner) {
        if !quoted {
            if let Some((k, v)) = parse_kv_arg(&seg, &["id", "default", "layout"]) {
                match k.as_str() {
                    "id" => spec.id = v,
                    "default" => default_val = Some(v),
                    "layout" => {
                        spec.layout = match v.to_ascii_lowercase().as_str() {
                            "horizontal" | "inline" | "row" => RadioLayout::Horizontal,
                            _ => RadioLayout::Vertical,
                        };
                    }
                    _ => {}
                }
                continue;
            }
            if spec.id.is_empty() && spec.options.is_empty() && is_valid_bash_identifier(&seg) {
                spec.id = seg;
                continue;
            }
        }
        spec.options.push(seg);
    }

    if let Some(dv) = default_val {
        if let Some(idx) = spec.options.iter().position(|o| o == &dv) {
            spec.default_index = idx;
        }
    }

    spec
}

/// Finds the first unescaped `{radio:...}` macro span in `s` with at least one option.
pub fn find_radio_macro_span(s: &str) -> Option<(usize, usize, RadioSpec)> {
    let mut search_from = 0usize;
    while let Some(rel) = s[search_from..].find("{radio:") {
        let start = search_from + rel;
        if start > 0 && s.as_bytes()[start - 1] == b'{' {
            search_from = start + 7;
            continue;
        }
        if let Some(end) = scan_balanced_macro_end(s, start) {
            let inner = &s[start + 7..end - 1];
            let spec = parse_radio_macro_fields(inner);
            if !spec.options.is_empty() {
                return Some((start, end, spec));
            }
            search_from = end;
            continue;
        }
        search_from = start + 7;
    }
    None
}

/// Parses `{checkbox:...}` inner fields supporting both positional shorthand
/// (`{checkbox: enableLogs: "Enable verbose logging": true}`) and key-value
/// attributes (`id="..."`, `label="..."`/`text="..."`, `default=true`/`checked=true`).
fn parse_checkbox_macro_fields(inner: &str) -> CheckboxSpec {
    let mut spec = CheckboxSpec::default();

    for (seg, quoted) in split_colon_macro_args(inner) {
        if !quoted {
            if let Some((k, v)) = parse_kv_arg(&seg, &["id", "label", "text", "default", "checked"])
            {
                match k.as_str() {
                    "id" => spec.id = v,
                    "label" | "text" => spec.label = v,
                    "default" | "checked" => {
                        spec.default_checked =
                            matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
                    }
                    _ => {}
                }
                continue;
            }
            if spec.id.is_empty() && spec.label.is_empty() && is_valid_bash_identifier(&seg) {
                spec.id = seg;
                continue;
            }
            if !spec.label.is_empty() {
                let lower = seg.to_ascii_lowercase();
                if matches!(lower.as_str(), "true" | "1" | "yes" | "on") {
                    spec.default_checked = true;
                    continue;
                } else if matches!(lower.as_str(), "false" | "0" | "no" | "off") {
                    spec.default_checked = false;
                    continue;
                }
            }
        }
        if spec.label.is_empty() {
            spec.label = seg;
        }
    }

    spec
}

/// Finds the first unescaped `{checkbox:...}` macro span in `s`.
pub fn find_checkbox_macro_span(s: &str) -> Option<(usize, usize, CheckboxSpec)> {
    let mut search_from = 0usize;
    while let Some(rel) = s[search_from..].find("{checkbox:") {
        let start = search_from + rel;
        if start > 0 && s.as_bytes()[start - 1] == b'{' {
            search_from = start + 10;
            continue;
        }
        if let Some(end) = scan_balanced_macro_end(s, start) {
            let inner = &s[start + 10..end - 1];
            let spec = parse_checkbox_macro_fields(inner);
            if !spec.id.is_empty() || !spec.label.is_empty() {
                return Some((start, end, spec));
            }
            search_from = end;
            continue;
        }
        search_from = start + 10;
    }
    None
}

/// Returns true if the Markdown body uses interactive form features (`{input}`,
/// `{radio}`, `{checkbox}`, `{button}`, `{clear}`, or reserved
/// `bashmenu-<lang>:id="..."` blocks).
pub fn has_interactive_markdown_form(body: &str) -> bool {
    let doc = parse_interactive_markdown_doc(body);
    !doc.reserved_scripts.is_empty()
        || doc.pages.len() > 1
        || doc.pages.iter().flatten().any(|b| {
            matches!(
                b,
                MarkdownFormBlock::Input { .. }
                    | MarkdownFormBlock::Radio(_)
                    | MarkdownFormBlock::Checkbox(_)
                    | MarkdownFormBlock::Buttons(_)
            )
        })
}

/// Returns true if the Markdown body contains at least one embedded `{input...}` widget.
pub fn has_markdown_form_inputs(body: &str) -> bool {
    parse_markdown_form_blocks(body)
        .iter()
        .any(|b| matches!(b, MarkdownFormBlock::Input { .. }))
}

/// Escapes `s` for inclusion inside a Bash double-quoted string while keeping
/// `$` unescaped so runtime Bash variables (`$GUESS`, `${VAR}`) still expand.
fn escape_for_bash_double_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '`' => out.push_str("\\`"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

/// Transpiles a single non-comment Bash line by rewriting `{param:...}`,
/// `{prompt:...}`, and `{message:...}` macros into live `__bm_*` IPC calls.
fn transpile_bash_line(line: &str) -> String {
    if line.trim_start().starts_with('#') {
        return line.to_string();
    }

    let mut out = line.to_string();

    // 1. Rewrite `{param...}` and `{prompt...}` into `$(__bm_param ...)`
    let mut search_from = 0usize;
    while search_from < out.len() {
        let mut earliest: Option<(usize, usize, String)> = None;
        for prefix in ["{param", "{prompt"] {
            let mut pos = search_from;
            while let Some(rel) = out[pos..].find(prefix) {
                let start = pos + rel;
                if start > 0 && out.as_bytes()[start - 1] == b'{' {
                    pos = start + prefix.len();
                    continue;
                }
                let after = &out[start + prefix.len()..];
                if let Some(first_ch) = after.chars().next() {
                    if first_ch == '}' || first_ch == ':' {
                        if let Some(end) = scan_balanced_macro_end(&out, start) {
                            let inner = if first_ch == ':' {
                                out[start + prefix.len() + 1..end - 1].to_string()
                            } else {
                                String::new()
                            };
                            if earliest.as_ref().map(|e| start < e.0).unwrap_or(true) {
                                earliest = Some((start, end, inner));
                            }
                            break;
                        }
                    }
                }
                pos = start + prefix.len();
            }
        }
        let Some((start, end, inner)) = earliest else {
            break;
        };
        let fields = parse_inline_macro_fields(&inner);
        let title = escape_for_bash_double_quotes(fields.title.as_deref().unwrap_or(""));
        let text = escape_for_bash_double_quotes(fields.text.as_deref().unwrap_or(""));
        let def = escape_for_bash_double_quotes(fields.default_value.as_deref().unwrap_or(""));
        let masked = if fields.masked { "1" } else { "0" };
        let replacement = format!("$(__bm_param \"{title}\" \"{text}\" \"{def}\" \"{masked}\")");
        out.replace_range(start..end, &replacement);
        search_from = start + replacement.len();
    }

    // 2. Rewrite `{message:...}` into `__bm_message "<title>" "<text>"`
    search_from = 0;
    while search_from < out.len() {
        let Some((rel_start, rel_end, data)) = find_message_macro_span(&out[search_from..]) else {
            break;
        };
        let start = search_from + rel_start;
        let end = search_from + rel_end;
        let title = escape_for_bash_double_quotes(data.title.as_deref().unwrap_or(""));
        let text = escape_for_bash_double_quotes(&data.text);
        let replacement = format!("__bm_message \"{title}\" \"{text}\"");
        out.replace_range(start..end, &replacement);
        search_from = start + replacement.len();
    }

    // 3. Rewrite `{id:<VAR>}` into `${<VAR>:-}` so form input variables expand
    //    both inside `{param:...}` / `{message:...}` strings and regular Bash lines.
    search_from = 0;
    while search_from < out.len() {
        let Some(rel_start) = out[search_from..].find("{id:") else {
            break;
        };
        let start = search_from + rel_start;
        if start > 0 && out.as_bytes()[start - 1] == b'{' {
            search_from = start + 4;
            continue;
        }
        let after_prefix = &out[start + 4..];
        if let Some(rel_close) = after_prefix.find('}') {
            let var_name = after_prefix[..rel_close].trim();
            if is_valid_bash_identifier(var_name) {
                let end = start + 4 + rel_close + 1;
                let replacement = format!("${{{var_name}:-}}");
                out.replace_range(start..end, &replacement);
                search_from = start + replacement.len();
                continue;
            }
        }
        search_from = start + 4;
    }

    out
}

/// Transpiles a `bashmenu-bash` code block into a runnable Bash script wired to
/// `bashmenu`'s UI-RPC socket via `self_bin`.
pub fn transpile_bashmenu_bash(raw_code: &str, self_bin: &Path) -> String {
    let bin_quoted = posix_shell_quote(&self_bin.to_string_lossy());
    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str(&format!("__BM_BIN={bin_quoted}\n"));
    script.push_str("__bm_param() { \"$__BM_BIN\" --ui-rpc-param \"$1\" \"$2\" \"$3\" \"$4\"; }\n");
    script.push_str("__bm_message() { \"$__BM_BIN\" --ui-rpc-message \"$1\" \"$2\"; }\n");
    script.push_str("__bm_toast() { \"$__BM_BIN\" --ui-rpc-toast \"$1\" \"$2\"; }\n");
    script.push_str("__bm_placeholder() { \"$__BM_BIN\" --ui-rpc-placeholder \"$1\"; }\n");
    script.push_str("bm_param() { __bm_param \"${1:-}\" \"${2:-}\" \"${3:-}\" \"${4:-0}\"; }\n");
    script.push_str("bm_message() { __bm_message \"${1:-}\" \"${2:-}\"; }\n");
    script.push_str("bm_toast() { __bm_toast \"${1:-}\" \"${2:-}\"; }\n");

    let mut first_line = true;
    for line in raw_code.split_inclusive('\n') {
        if first_line && line.starts_with("#!") {
            first_line = false;
            continue;
        }
        first_line = false;
        let has_nl = line.ends_with('\n');
        let core = line.trim_end_matches(['\r', '\n']);
        script.push_str(&transpile_bash_line(core));
        if has_nl {
            script.push('\n');
        }
    }

    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interactive_markdown_splits_bashmenu_bash_only() {
        let md = r#"Intro text

````bashmenu-bash  <-- special fence
while true; do
    GUESS="{param:'Guess a number (q to quit):'}"
    {message:"You entered $GUESS"}
done
````

```bash
echo "standard code block stays markdown"
```

# Footer
You are user: {user}
"#;
        let segments = parse_interactive_markdown(md);
        assert_eq!(segments.len(), 3);
        assert!(matches!(&segments[0], MarkdownSegment::Markdown(s) if s.contains("Intro text")));
        match &segments[1] {
            MarkdownSegment::Script { language, code } => {
                assert_eq!(language, "bash");
                assert!(code.contains("while true; do"));
                assert!(code.contains("{message:\"You entered $GUESS\"}"));
            }
            other => panic!("expected Script segment, got {other:?}"),
        }
        match &segments[2] {
            MarkdownSegment::Markdown(s) => {
                assert!(s.contains("```bash"));
                assert!(s.contains("You are user: {user}"));
            }
            other => panic!("expected Markdown segment, got {other:?}"),
        }
    }

    #[test]
    fn test_transpile_bashmenu_bash_preserves_runtime_vars() {
        let code = r#"#!/usr/bin/env bash
# comment with {param:'ignored'}
while true; do
    GUESS="{param:'Guess a number {id:guestName} (q to quit):'}"
    case "$GUESS" in
        [Qq]*) echo "Aborted."; exit 1;;
        *) {message:"You entered $GUESS. Age: {id: guestAge} (escaped: {{id:guestName}})"} ;;
    esac
done
"#;
        let out = transpile_bashmenu_bash(code, Path::new("/usr/local/bin/bashmenu"));
        assert!(out.contains("__BM_BIN='/usr/local/bin/bashmenu'"));
        assert!(out.contains("# comment with {param:'ignored'}"));
        assert!(out.contains(
            "GUESS=\"$(__bm_param \"\" \"Guess a number ${guestName:-} (q to quit):\" \"\" \"0\")\""
        ));
        assert!(out.contains(
            "*) __bm_message \"\" \"You entered $GUESS. Age: ${guestAge:-} (escaped: {{id:guestName}})\" ;;"
        ));
    }

    #[test]
    fn test_parse_input_macro_defaults_and_options() {
        let (_, _, full) = find_input_macro_span(
            r#"{input: default="hello" : id = bashVariableName : width=40 : multiline=true : scrollbar=false : height=8}"#,
        )
        .expect("parse full input");
        assert_eq!(full.default_value, "hello");
        assert_eq!(full.id, "bashVariableName");
        assert_eq!(full.width, Some(40));
        assert!(full.multiline);
        assert!(!full.scrollbar);
        assert_eq!(full.height, 8);

        // Verify defaults: default="", width=None, multiline=false, scrollbar=true, height=5
        let (_, _, minimal) =
            find_input_macro_span("{input: id = myVar }").expect("parse minimal input");
        assert_eq!(minimal.id, "myVar");
        assert_eq!(minimal.default_value, "");
        assert_eq!(minimal.width, None);
        assert!(!minimal.multiline);
        assert!(minimal.scrollbar);
        assert_eq!(minimal.height, 5);

        // Identifier validation
        assert!(is_valid_bash_identifier("bashVariableName"));
        assert!(is_valid_bash_identifier("_var_2"));
        assert!(!is_valid_bash_identifier("1bad"));
        assert!(!is_valid_bash_identifier("bad-name"));
        assert!(!is_valid_bash_identifier(""));

        // Inline label prefix & {id:...} live placeholder substitution
        let md = "\
What is your name: {input: default=\"andrew\": id=\"guestName\"}
According to my records, your name is \"{id:guestName}\" and age is \"{id: guestAge}\".
```bash
# Literal in code fence: {id:guestName} {input: id=\"ignored\"}
```
Escaped: {{id:guestName}}
";
        let blocks = parse_markdown_form_blocks(md);
        assert!(matches!(
            &blocks[0],
            MarkdownFormBlock::Input { prefix: Some(p), spec }
                if p == "What is your name: " && spec.id == "guestName"
        ));
        let mut vars = std::collections::HashMap::new();
        vars.insert("guestName".to_string(), "Bob".to_string());
        let rendered = substitute_id_placeholders(
            match &blocks[1] {
                MarkdownFormBlock::Text(t) => t,
                _ => panic!("expected text block"),
            },
            &vars,
        );
        assert!(rendered.contains("your name is \"Bob\" and age is \"\"."));
        assert!(rendered.contains("# Literal in code fence: {id:guestName}"));
        assert!(rendered.contains("Escaped: {{id:guestName}}"));
    }

    #[test]
    fn test_parse_button_macro_and_reserved_scripts_with_clear() {
        let (_, _, b_sub) =
            find_button_macro_span(r#"{button: type="submit" : action="saveUser"}"#)
                .expect("parse submit button");
        assert_eq!(b_sub.button_type, ButtonType::Submit);
        assert_eq!(b_sub.display_label(), "Submit");
        assert_eq!(b_sub.action.as_deref(), Some("saveUser"));

        let (_, _, b_ok) =
            find_button_macro_span("{button: type='ok': text='Continue'}").expect("parse ok");
        assert_eq!(b_ok.button_type, ButtonType::Ok);
        assert_eq!(b_ok.display_label(), "Continue");
        assert_eq!(b_ok.action, None);

        let (_, _, b_cancel) = find_button_macro_span("{button: type=`cancel`: action=`onAbort`}")
            .expect("parse cancel");
        assert_eq!(b_cancel.button_type, ButtonType::Cancel);
        assert_eq!(b_cancel.display_label(), "Cancel");
        assert_eq!(b_cancel.action.as_deref(), Some("onAbort"));

        // Fence header parsing
        assert_eq!(
            parse_bashmenu_fence_header(r#"bashmenu-bash:id="exampleCodeBlock""#),
            Some(("bash".to_string(), Some("exampleCodeBlock".to_string())))
        );
        assert_eq!(
            parse_bashmenu_fence_header("bashmenu-bash"),
            Some(("bash".to_string(), None))
        );
        assert_eq!(
            parse_bashmenu_fence_header("xbashmenu-bash:id=\"no\""),
            None
        );
        assert_eq!(parse_bashmenu_fence_header("bash"), None);

        // Full multi-page document with reserved script, horizontal buttons, and {clear}
        let md = "\
```bashmenu-bash:id=\"exampleCodeBlock\"
{message: \"Hello $userName\"}
```
# Page 1
Name: {input: default=\"World\": id=\"userName\"}
{button: type=\"submit\": action=\"exampleCodeBlock\"} {button: type=\"cancel\"}
{clear}
# Page 2
Welcome, {id:userName}!
```xbashmenu-bash:id=\"visibleSample\"
echo \"shown literally\"
```
{button: type=\"ok\"}
";
        assert!(has_interactive_markdown_form(md));
        let doc = parse_interactive_markdown_doc(md);
        assert_eq!(doc.reserved_scripts.len(), 1);
        let script = doc
            .reserved_scripts
            .get("exampleCodeBlock")
            .expect("reserved script stored");
        assert_eq!(script.language, "bash");
        assert!(script.code.contains("{message: \"Hello $userName\"}"));

        assert_eq!(doc.pages.len(), 2);
        // Reserved bashmenu-bash block must not leak into page 1 text
        assert!(!doc.pages[0].iter().any(|b| matches!(
            b,
            MarkdownFormBlock::Text(t) if t.contains("exampleCodeBlock")
        )));
        // Page 1 has inline button pair
        assert!(doc.pages[0].iter().any(|b| matches!(
            b,
            MarkdownFormBlock::Buttons(btns)
                if btns.len() == 2
                    && btns[0].button_type == ButtonType::Submit
                    && btns[0].action.as_deref() == Some("exampleCodeBlock")
                    && btns[1].button_type == ButtonType::Cancel
        )));
        // Page 2 keeps xbashmenu-bash visible as normal Markdown
        assert!(doc.pages[1].iter().any(|b| matches!(
            b,
            MarkdownFormBlock::Text(t) if t.contains("xbashmenu-bash") && t.contains("shown literally")
        )));
    }

    #[test]
    fn test_parse_radio_and_checkbox_macros() {
        // Shorthand positional radio
        let (_, _, r1) = find_radio_macro_span(r#"{radio: rdoValue: "Example 1": "Example 2"}"#)
            .expect("parse shorthand radio");
        assert_eq!(r1.id, "rdoValue");
        assert_eq!(r1.options, vec!["Example 1", "Example 2"]);
        assert_eq!(r1.default_index, 0);
        assert_eq!(r1.layout, RadioLayout::Vertical);

        // Key-value radio with default and horizontal layout
        let (_, _, r2) = find_radio_macro_span(
            r#"{radio: id="env": default="Prod": layout="horizontal": "Dev": "Staging": "Prod"}"#,
        )
        .expect("parse kv radio");
        assert_eq!(r2.id, "env");
        assert_eq!(r2.options, vec!["Dev", "Staging", "Prod"]);
        assert_eq!(r2.default_index, 2);
        assert_eq!(r2.layout, RadioLayout::Horizontal);

        // Shorthand positional checkbox
        let (_, _, c1) =
            find_checkbox_macro_span(r#"{checkbox: enableLogs: "Enable verbose logging": true}"#)
                .expect("parse shorthand checkbox");
        assert_eq!(c1.id, "enableLogs");
        assert_eq!(c1.label, "Enable verbose logging");
        assert!(c1.default_checked);

        // Key-value checkbox
        let (_, _, c2) =
            find_checkbox_macro_span("{checkbox: id='dryRun': label='Dry run': default=false}")
                .expect("parse kv checkbox");
        assert_eq!(c2.id, "dryRun");
        assert_eq!(c2.label, "Dry run");
        assert!(!c2.default_checked);

        // Escaped macros are ignored
        assert!(find_radio_macro_span(r#"{{radio: rdoValue: "A": "B"}}"#).is_none());
        assert!(find_checkbox_macro_span(r#"{{checkbox: flag: "Label"}}"#).is_none());
    }
}
