//! CP437 ASCII translations, Nerd font / emoji adaptive glyph resolution,
//! and 4-column icon padding calculations.

use unicode_width::UnicodeWidthStr;

/// IBM PC CP437 character mapping table for codes 0..=255.
pub const CP437_TABLE: [char; 256] = [
    '\0', '☺', '☻', '♥', '♦', '♣', '♠', '•', '◘', '○', '◙', '♂', '♀', '♪', '♫', '☼', '►', '◄', '↕',
    '‼', '¶', '§', '▬', '↨', '↑', '↓', '→', '←', '∟', '↔', '▲', '▼', ' ', '!', '"', '#', '$', '%',
    '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8',
    '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K',
    'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^',
    '_', '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
    'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~', '⌂', 'Ç', 'ü', 'é', 'â', 'ä',
    'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù',
    'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬',
    '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜',
    '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨',
    '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π',
    'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±', '≥', '≤', '⌠', '⌡', '÷',
    '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{00A0}',
];

/// Translates a byte (0..=255) to its CP437 Unicode character.
pub fn cp437_to_char(byte: u8) -> char {
    CP437_TABLE[byte as usize]
}

/// Normalizes variation selectors (`\u{fe0f}` and `\u{fe0e}`) by stripping them.
pub fn normalize_variation_selectors(text: &str) -> String {
    text.replace(['\u{fe0f}', '\u{fe0e}'], "")
}

/// Parses `{ascii:NNN}` macros and replaces them with corresponding CP437 characters.
pub fn parse_ascii_val(text: &str) -> String {
    if !text.contains("{ascii:") {
        return text.to_string();
    }

    let re = regex::Regex::new(r"\{ascii:\s*(\d+)\s*\}").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        if let Ok(code) = caps[1].parse::<u32>() {
            let byte = (code % 256) as u8;
            cp437_to_char(byte).to_string()
        } else {
            caps[0].to_string()
        }
    })
    .to_string()
}

fn is_hex_token(token: &str) -> bool {
    let clean = token
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('$')
        .trim_start_matches("0x");
    !clean.is_empty() && clean.chars().all(|c| c.is_ascii_hexdigit())
}

/// Resolves an adaptive glyph directive `{nf:[char]:[nerd]:[emoji]}` based on whether
/// Nerd Fonts are enabled.
///
/// Hierarchy:
/// 1. Emoji tier: Selected if defined and `use_nerd_fonts == true`.
/// 2. Nerd Font tier: Selected if hex token defined (`#`, `$`, `0x`) and `use_nerd_fonts == true`.
/// 3. Character tier: Selected if defined or if Nerd Fonts disabled.
/// 4. Fallback: Empty string if unconfigured.
pub fn resolve_glyph(glyph_def: &str, use_nerd_fonts: bool) -> String {
    let parsed_def = parse_ascii_val(glyph_def);
    if !parsed_def.starts_with("{nf:") || !parsed_def.ends_with('}') {
        return parsed_def;
    }

    let inner = &glyph_def[4..glyph_def.len() - 1];
    let parts: Vec<&str> = inner.split(':').collect();

    let mut char_part = "";
    let mut hex_part = "";
    let mut emoji_part = "";

    if parts.len() >= 3 {
        char_part = parts[0];
        hex_part = parts[1];
        emoji_part = parts[2];
    } else if parts.len() == 2 {
        if parts[0].is_empty() {
            hex_part = parts[1];
        } else if is_hex_token(parts[0]) {
            hex_part = parts[0];
            emoji_part = parts[1];
        } else {
            char_part = parts[0];
            hex_part = parts[1];
        }
    } else if parts.len() == 1 {
        if is_hex_token(parts[0]) {
            hex_part = parts[0];
        } else {
            char_part = parts[0];
        }
    }

    if use_nerd_fonts {
        if !emoji_part.is_empty() {
            return emoji_part.to_string();
        }
        if !hex_part.is_empty() && is_hex_token(hex_part) {
            let clean = hex_part
                .trim()
                .trim_start_matches('#')
                .trim_start_matches('$')
                .trim_start_matches("0x");
            if let Ok(val) = u32::from_str_radix(clean, 16) {
                if let Some(ch) = char::from_u32(val) {
                    return ch.to_string();
                }
            }
        }
    }

    char_part.to_string()
}

/// Formats an icon into the strict 4-column slot width invariant.
///
/// Invariant:
/// - 2-column emojis (`🚀`): `vis_w = 2, pad_w = 2` -> `"🚀  "`
/// - 1-column glyphs/symbols (`⚙`): `vis_w = 1, pad_w = 3` -> `"⚙   "`
/// - No icon: exactly 4 spaces -> `"    "`
pub fn format_icon_slot(raw_icon: Option<&str>, use_nerd_fonts: bool) -> String {
    let icon_str = match raw_icon {
        None | Some("") => return "    ".to_string(),
        Some(s) => s,
    };

    let resolved = resolve_glyph(icon_str, use_nerd_fonts);
    let normalized = normalize_variation_selectors(&resolved);
    let vis_w = UnicodeWidthStr::width(normalized.as_str());

    if vis_w >= 4 {
        normalized
    } else {
        let pad = 4 - vis_w;
        format!("{}{}", normalized, " ".repeat(pad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp437_translation() {
        assert_eq!(cp437_to_char(196), '─');
        assert_eq!(cp437_to_char(218), '┌');
        assert_eq!(cp437_to_char(176), '░');
        assert_eq!(parse_ascii_val("Border: {ascii:196}"), "Border: ─");
    }

    #[test]
    fn test_resolve_glyph_with_nerd_fonts() {
        // Hex nerd font: #f11e7 -> codepoint 0xf11e7 (or standard nerd codepoint #f0669 -> 0xf0669)
        let resolved_nf = resolve_glyph("{nf::#f14d3:}", true);
        assert_eq!(resolved_nf, char::from_u32(0xf14d3).unwrap().to_string());

        // Emoji tier takes precedence when use_nerd_fonts is true
        let resolved_emoji = resolve_glyph("{nf:@:#ee90:🚀}", true);
        assert_eq!(resolved_emoji, "🚀");

        // When nerd fonts disabled, falls back to character tier
        let fallback = resolve_glyph("{nf:@:#ee90:🚀}", false);
        assert_eq!(fallback, "@");
    }

    #[test]
    fn test_icon_slot_4_column_invariant() {
        // 1-column glyph: 1 visible column + 3 spaces = 4
        let one_col = format_icon_slot(Some("⚙"), true);
        assert_eq!(UnicodeWidthStr::width(one_col.as_str()), 4);
        assert_eq!(one_col, "⚙   ");

        // 2-column emoji: 2 visible columns + 2 spaces = 4
        let two_col = format_icon_slot(Some("🚀"), true);
        assert_eq!(UnicodeWidthStr::width(two_col.as_str()), 4);
        assert_eq!(two_col, "🚀  ");

        // None: 4 spaces = 4
        let none_col = format_icon_slot(None, true);
        assert_eq!(UnicodeWidthStr::width(none_col.as_str()), 4);
        assert_eq!(none_col, "    ");
    }
}
