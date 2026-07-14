//! Mixed-quote boundary and colon-in-key repair.
//!
//! This transform runs as a single pass over the input, fixing two
//! LLM-specific malformations:
//!
//! - **Mixed-quote boundaries** — `'…','bareword":"…'` patterns where a
//!   single-quoted string is followed by a bareword key opening a
//!   double-quoted value.
//! - **Colons inside keys** — `"key:val"` followed by `,`/`}`, rewritten to
//!   `"key":"val"`.

use std::borrow::Cow;

use crate::util::char_at;

// Detect `',bareword":` pattern inside mixed-quote sections.
// E.g. `'foo','bar":"baz"` — the `'bar":` is the key for the next value.
// Returns `(bare_start, k)` spanning the bareword.
fn try_mixed_quote_boundary(bytes: &[u8], n: usize, pos: usize) -> Option<(usize, usize)> {
    if pos + 2 < n && bytes[pos] == b'\'' && bytes[pos + 1] == b',' && bytes[pos + 2] == b'\'' {
        let bare_start = pos + 3;
        let mut k = bare_start;
        while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
            k += 1;
        }
        if k > bare_start
            && k + 2 < n
            && bytes[k] == b'"'
            && bytes[k + 1] == b':'
            && bytes[k + 2] == b'"'
        {
            return Some((bare_start, k));
        }
    }
    None
}

// Emit the transformed token for a mixed-quote boundary:
// `'","bareword":"`  (closing `'`, comma, opening `"`, bare key, `":"`)
fn emit_mixed_quote_boundary(out: &mut String, text: &str, bare_start: usize, k: usize) {
    out.push('"');
    out.push(',');
    out.push('"');
    out.push_str(&text[bare_start..k]);
    out.push_str("\":\"");
}

// Fix colons that appear *inside* a key string by escaping the
// relevant characters.  Handles patterns like `"key:": value`.
fn try_fix_colon_in_key(
    content: &str,
    end: usize,
    text: &str,
    bytes: &[u8],
    n: usize,
    out: &mut String,
) -> Option<usize> {
    let cpos = content.find(':')?;
    let key = &content[..cpos];
    let val = &content[cpos + 1..];
    // Only accept simple bareword keys/values
    if key.is_empty()
        || val.is_empty()
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || !val.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }
    let mut ws = end;
    while ws < n && bytes[ws].is_ascii_whitespace() {
        ws += 1;
    }
    // Must be followed by structural separator
    if ws >= n || !matches!(bytes[ws], b',' | b'}') {
        return None;
    }
    // Rewrite `"key:val"` → `"key":"val"`
    out.push('"');
    out.push_str(key);
    out.push_str("\":\"");
    out.push_str(val);
    out.push('"');
    out.push_str(&text[end..ws]);
    out.push(char::from(bytes[ws]));
    Some(ws + 1)
}

/// Single-pass pre-processing: walks the input and fixes mixed-quote
/// boundaries (`'…','bareword":"…'` → valid JSON keys) and colons
/// embedded inside key strings.
pub(crate) fn preprocess_json(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    let mut modified = false;

    while i < n {
        // Check for mixed-quote boundary at current position
        if let Some((bare_start, k)) = try_mixed_quote_boundary(bytes, n, i) {
            modified = true;
            emit_mixed_quote_boundary(&mut out, text, bare_start, k);
            i = k + 3;
            continue;
        }

        // Inside a quoted string — scan for mixed-quote boundaries and
        // colons-in-keys
        if bytes[i] == b'"' {
            let string_start = i;
            let mut j = i + 1;
            let mut has_colon = false;

            let mut processed = false;
            while j < n {
                if let Some((bare_start, k)) = try_mixed_quote_boundary(bytes, n, j) {
                    modified = true;
                    out.push_str(&text[string_start..j]);
                    emit_mixed_quote_boundary(&mut out, text, bare_start, k);
                    i = k + 3;
                    processed = true;
                    break;
                }

                if bytes[j] == b':' {
                    has_colon = true;
                }
                if bytes[j] == b'"' {
                    let content_end = j;
                    let end = j + 1;

                    if has_colon {
                        let content = &text[string_start + 1..content_end];
                        if let Some(new_i) =
                            try_fix_colon_in_key(content, end, text, bytes, n, &mut out)
                        {
                            modified = true;
                            i = new_i;
                            processed = true;
                            break;
                        }
                    }

                    out.push_str(&text[string_start..end]);
                    i = end;
                    processed = true;
                    break;
                }

                j += 1;
            }

            // Unclosed string — copy the rest literally
            if !processed {
                out.push_str(&text[string_start..]);
                break;
            }
            continue;
        }

        if bytes[i].is_ascii() {
            out.push(char::from(bytes[i]));
            i += 1;
        } else {
            let ch = char_at(text, i);
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    if modified {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_no_change_returns_borrowed() {
        let text = r#"{"key": "value"}"#;
        match preprocess_json(text) {
            Cow::Borrowed(b) => assert_eq!(b, text),
            Cow::Owned(_) => panic!("should not allocate for clean input"),
        }
    }

    #[test]
    fn preprocess_fixes_colon_in_key() {
        // `"key:val"` followed by `,` → `"key":"val",`
        let text = r#"{"key:val", "b": 1}"#;
        let result = preprocess_json(text);
        match result {
            Cow::Owned(s) => {
                assert!(s.contains(r#""key":"val""#), "got: {s}");
            }
            Cow::Borrowed(_) => panic!("should have modified the text"),
        }
    }

    #[test]
    fn preprocess_fixes_mixed_quote_boundary() {
        // `'foo','bar":"baz"` — the `'bar":` is the key for the next value
        let text = r#"['foo','bar":"baz"]"#;
        let result = preprocess_json(text);
        match result {
            Cow::Owned(s) => {
                assert!(s.contains(r#""bar":""#), "got: {s}");
            }
            Cow::Borrowed(_) => panic!("should have modified the text"),
        }
    }
}
