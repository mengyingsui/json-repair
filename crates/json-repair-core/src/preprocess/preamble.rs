//! Preamble normalization — stripping non-JSON content before the first
//! container (`{` / `[`).
//!
//! Handles Markdown code fences, `[TEXT]`-style metatags, Markdown link
//! parens, and unbraced `"key": value` patterns that lack a wrapping `{}`.

use std::borrow::Cow;

use crate::util::{char_at, utf8_char_len};
use memchr::memchr2;

// Skip Unicode whitespace from `pos`, return new position.
//
// Used only in the preprocessing phase to locate the first JSON container
// (`{` / `[`) — Unicode whitespace (U+00A0, U+2003, U+3000, U+FEFF, …) is
// commonly emitted by LLMs and must not prevent detection of the JSON start.
// The repairer itself still uses ASCII-only whitespace per RFC 8259.
fn skip_ws_at(text: &str, mut pos: usize) -> usize {
    while let Some(ch) = char_at(text, pos) {
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

// Skip a Markdown code fence (``` … ```).  If the language tag is
// "json" or absent, the fence is removed and parsing starts at the
// end-of-first-line.  Non-JSON fences are fully consumed (including
// the closing ```) to strip non-JSON content.
fn try_skip_code_fence(text: &str, pos: usize) -> Option<usize> {
    let n = text.len();
    let bytes = text.as_bytes();
    if pos + 2 >= n || !bytes[pos..].starts_with(b"```") {
        return None;
    }
    let mut i = pos + 3;
    let lang_start = i;
    // Consume rest of the opening line
    while i < n && bytes[i] != b'\n' {
        i += utf8_char_len(bytes[i]);
    }
    let is_json = matches!(text[lang_start..i].trim(), "" | "json");
    if i < n {
        i += 1;
    }
    // For non-JSON fences, skip to closing ```
    if !is_json {
        while i < n {
            if i + 2 < n && bytes[i..].starts_with(b"```") {
                i += 3;
                break;
            }
            i += utf8_char_len(bytes[i]);
        }
    }
    Some(i)
}

// Detect and skip `[TEXT]` metatags and `[text](url)` Markdown links.
// A metatag must be ≤128 chars, contain only alphanum/`_`/`-`, and
// must not contain `{` or `"`.
fn try_skip_metatag_or_link(text: &str, pos: usize) -> Option<usize> {
    debug_assert_eq!(char_at(text, pos), Some('['));
    let n = text.len();
    let bytes = text.as_bytes();
    let mut depth = 1i32;
    let mut j = pos + 1;
    let mut is_metatag = j < n;
    while j < n && depth > 0 {
        let b = bytes[j];
        match b {
            // Nested opening bracket: track depth so we find the matching `]`.
            b'[' => depth += 1,
            // Closing bracket: pop one level of nesting.
            b']' => depth -= 1,
            // JSON content inside means this is not a plain metatag.
            b'{' | b'"' => is_metatag = false,
            // Any other character is irrelevant for metatag detection.
            _ => {}
        }
        j += utf8_char_len(b);
    }
    // Meet the metatag criteria
    if depth == 0 && is_metatag && j - pos <= 128 {
        let inner = &text[pos + 1..j - 1];
        if !inner.is_empty()
            && inner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Some(j);
        }
    }
    // Not a metatag, but could be a Markdown link `[label](url)`
    if j < n && bytes[j] == b'(' {
        let mut k = j + 1;
        let mut link_depth = 1i32;
        while k < n && link_depth > 0 {
            let b = bytes[k];
            if b == b'(' {
                link_depth += 1;
            }
            if b == b')' {
                link_depth -= 1;
            }
            k += utf8_char_len(b);
        }
        return Some(k);
    }
    None
}

// Scan a double-quoted string from `pos` to its closing `"`,
// respecting `\`-escapes.  Returns the position after the closing `"`,
// or `n` if the string is unclosed.
fn scan_string(text: &str, pos: usize) -> usize {
    debug_assert_eq!(char_at(text, pos), Some('"'));
    let bytes = text.as_bytes();
    let n = text.len();
    let mut i = pos + 1;
    loop {
        match memchr2(b'"', b'\\', &bytes[i..]) {
            Some(off) => {
                i += off;
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < n {
                        i += utf8_char_len(bytes[i]);
                    }
                } else {
                    return i + 1;
                }
            }
            None => return n,
        }
    }
}

/// Skip non-JSON text before the first `{` or `[` and handle
/// unbraced-key input.
///
/// Handles Markdown code fences, `[TEXT_*]`-style metatags, Markdown
/// link parens, and unbraced `"key": value` patterns.
///
/// Returns `(Cow<str>, usize)` where the `Cow` wraps the (possibly
/// modified) input text, and `usize` is the starting position for the
/// parser.
pub(crate) fn normalize_preamble(text: &str) -> (Cow<'_, str>, usize) {
    let start = skip_ws_at(text, 0);
    let mut i = start;
    // Track whether we've seen `"key":` without a wrapping `{`
    let mut unbraced_start: Option<usize> = None;

    while i < text.len() {
        // Skip Markdown code fences that wrap the JSON
        if let Some(new_i) = try_skip_code_fence(text, i) {
            i = new_i;
            continue;
        }

        let ch = match char_at(text, i) {
            Some(c) => c,
            None => break,
        };
        // Found real JSON content
        if ch == '{' || ch == '[' {
            if ch == '[' {
                // Check for metatag/link at `[` — skip if so, break if real array
                if let Some(new_i) = try_skip_metatag_or_link(text, i) {
                    i = new_i;
                    continue;
                }
            }
            // Unbraced key mode: wrap everything in `{…}`
            if let Some(start_pos) = unbraced_start {
                let mut new_text = String::with_capacity(text.len() + 2);
                new_text.push_str(&text[..start_pos]);
                new_text.push('{');
                new_text.push_str(&text[start_pos..]);
                new_text.push('}');
                return (Cow::Owned(new_text), start_pos);
            }
            break;
        }
        // Track `"key":` patterns at the top level (without `{` wrapping)
        if ch == '"' {
            let end = scan_string(text, i);
            let j = skip_ws_at(text, end);
            if j < text.len() && char_at(text, j) == Some(':') && unbraced_start.is_none() {
                unbraced_start = Some(i);
            }
            i = end;
        } else {
            i += ch.len_utf8();
        }
    }
    if i >= text.len() {
        // Unbraced key mode: wrap everything in `{…}` even at EOF
        if let Some(start_pos) = unbraced_start {
            let mut new_text = String::with_capacity(text.len() + 2);
            new_text.push_str(&text[..start_pos]);
            new_text.push('{');
            new_text.push_str(&text[start_pos..]);
            new_text.push('}');
            return (Cow::Owned(new_text), start_pos);
        }
        (Cow::Borrowed(text), 0)
    } else {
        debug_assert!(
            i < text.len() && (char_at(text, i) == Some('{') || char_at(text, i) == Some('[')),
            "normalize_preamble: position does not point at JSON container start"
        );
        (Cow::Borrowed(text), i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── skip_ws_at (Unicode whitespace) ────────────────────────────────

    #[test]
    fn skip_ws_at_ascii_whitespace() {
        assert_eq!(skip_ws_at("   hello", 0), 3);
        assert_eq!(skip_ws_at("\t\n\rx", 0), 3);
    }

    #[test]
    fn skip_ws_at_no_leading_whitespace() {
        assert_eq!(skip_ws_at("hello", 0), 0);
    }

    #[test]
    fn skip_ws_at_unicode_nbsp() {
        // U+00A0 NO-BREAK SPACE
        assert_eq!(skip_ws_at("\u{00A0}hello", 0), 2);
    }

    #[test]
    fn skip_ws_at_unicode_em_space() {
        // U+2003 EM SPACE
        assert_eq!(skip_ws_at("\u{2003}hello", 0), 3);
    }

    #[test]
    fn skip_ws_at_unicode_ideographic_space() {
        // U+3000 IDEOGRAPHIC SPACE
        assert_eq!(skip_ws_at("\u{3000}hello", 0), 3);
    }

    #[test]
    fn skip_ws_at_unicode_bom() {
        // U+FEFF is NOT classified as whitespace by `char::is_whitespace`
        // (since Unicode 3.2 it is only a byte-order mark).  Verify it is
        // treated as a regular character and not skipped.
        assert_eq!(skip_ws_at("\u{FEFF}hello", 0), 0);
    }

    #[test]
    fn skip_ws_at_mixed_ascii_unicode() {
        // space(1) + U+00A0(2 bytes) + \t(1) + U+3000(3 bytes) = 7
        assert_eq!(skip_ws_at(" \u{00A0}\t\u{3000}hello", 0), 7);
    }

    #[test]
    fn skip_ws_at_empty_string() {
        assert_eq!(skip_ws_at("", 0), 0);
    }

    #[test]
    fn skip_ws_at_all_whitespace() {
        // 3 spaces + \t + U+00A0 (2 bytes) = 6 bytes total
        assert_eq!(skip_ws_at("   \t\u{00A0}", 0), 6);
    }

    // ── normalize_preamble ─────────────────────────────────────────────

    #[test]
    fn normalize_preamble_finds_object_start() {
        let (cow, pos) = normalize_preamble(r#"  {"a": 1}"#);
        assert_eq!(pos, 2);
        assert_eq!(&cow[pos..pos + 1], "{");
    }

    #[test]
    fn normalize_preamble_finds_array_start() {
        let (_, pos) = normalize_preamble(r#"[1, 2, 3]"#);
        assert_eq!(pos, 0);
    }

    #[test]
    fn normalize_preamble_skips_unicode_whitespace() {
        let (cow, pos) = normalize_preamble("\u{00A0}\u{3000}{\"a\":1}");
        assert_eq!(&cow[pos..pos + 1], "{");
    }

    #[test]
    fn normalize_preamble_strips_code_fence() {
        let (cow, pos) = normalize_preamble("```json\n{\"a\":1}\n```");
        assert_eq!(&cow[pos..pos + 1], "{");
    }

    #[test]
    fn normalize_preamble_strips_metatag() {
        let (cow, pos) = normalize_preamble("[TEXT_START]{\"a\":1}");
        assert_eq!(&cow[pos..pos + 1], "{");
    }

    #[test]
    fn normalize_preamble_wraps_unbraced_key() {
        let (cow, pos) = normalize_preamble(r#""key": "value""#);
        // Should wrap in `{...}` — pos points at the opening `{` we inserted
        assert_eq!(&cow[pos..pos + 1], "{", "got: {cow}");
        assert!(cow.contains(r#""key": "value""#), "got: {cow}");
    }

    #[test]
    fn normalize_preamble_empty_input() {
        let (_, pos) = normalize_preamble("");
        assert_eq!(pos, 0);
    }
}
