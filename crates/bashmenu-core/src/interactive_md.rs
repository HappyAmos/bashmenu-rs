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
    GUESS="{param:'Guess a number (q to quit):'}"
    case "$GUESS" in
        [Qq]*) echo "Aborted."; exit 1;;
        *) {message:"You entered $GUESS"} ;;
    esac
done
"#;
        let out = transpile_bashmenu_bash(code, Path::new("/usr/local/bin/bashmenu"));
        assert!(out.contains("__BM_BIN='/usr/local/bin/bashmenu'"));
        assert!(out.contains("# comment with {param:'ignored'}"));
        assert!(
            out.contains("GUESS=\"$(__bm_param \"\" \"Guess a number (q to quit):\" \"\" \"0\")\"")
        );
        assert!(out.contains("*) __bm_message \"\" \"You entered $GUESS\" ;;"));
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
}
