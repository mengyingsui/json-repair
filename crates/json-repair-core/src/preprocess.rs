//! Input pre-processing before the main repair pass.
//!
//! Transforms common LLM output patterns that the streaming repairer cannot
//! handle efficiently in a single pass:
//! - `fix_colon_in_key`: split `"key:value"` → `"key":"value"` enclosed in a
//!   quoted string followed by `,`/`}`
//! - `fix_mixed_quotes`: normalize `','word":"` boundary (single→double quote
//!   mix) to `","word":"` so the parser sees a key after a comma

use std::borrow::Cow;

/// Get the character at byte position `pos` in `text`, with an ASCII fast path.
///
/// **Panics** if `pos` is out of bounds or in the middle of a multibyte
/// UTF-8 sequence.  Use in contexts where bounds are already validated.
#[inline]
pub(crate) fn char_at(text: &str, pos: usize) -> char {
    if text.as_bytes()[pos].is_ascii() {
        text.as_bytes()[pos] as char
    } else {
        text[pos..].chars().next().unwrap()
    }
}

/// Find the first quoted string containing `:` followed by `,` or `}`.
/// Returns `Some(open_pos)` where the problematic string starts, or `None`.
fn needs_colon_fix(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i] == b'"' {
            let open_pos = i;
            let mut has_colon = false;
            i += 1;
            while i < n && bytes[i] != b'"' {
                if bytes[i] == b':' {
                    has_colon = true;
                }
                i += 1;
            }
            if i < n {
                if has_colon {
                    let mut j = i + 1;
                    while j < n && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
                        j += 1;
                    }
                    if j < n && matches!(bytes[j], b',' | b'}') {
                        return Some(open_pos);
                    }
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Fix the `','word":"` mixed-quote boundary pattern in `text`.
///
/// When LLM output uses both `'` and `"` quote styles, a double-quoted string
/// value may contain `','word":"` where `'word'` was originally a single-quoted
/// key.  This pre-processing step splits it into `","word":"` so the parser
/// correctly treats `word` as the next key.
pub fn fix_mixed_quotes(text: &str) -> Cow<'_, str> {
    if !text.contains("','") {
        return Cow::Borrowed(text);
    }
    let n = text.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        let ch = char_at(text, i);
        if ch == '\''
            && i + 2 < n
            && text.as_bytes()[i + 1] == b','
            && text.as_bytes()[i + 2] == b'\''
        {
            let after_comma = i + 3;
            let mut k = after_comma;
            while k < n {
                let kc = char_at(text, k);
                if !(kc.is_alphanumeric() || kc == '_') {
                    break;
                }
                k += kc.len_utf8();
            }
            if k > after_comma
                && k + 2 < n
                && text.as_bytes()[k] == b'"'
                && text.as_bytes()[k + 1] == b':'
                && text.as_bytes()[k + 2] == b'"'
            {
                out.push('"');
                out.push(',');
                out.push('"');
                out.push_str(&text[after_comma..k]);
                out.push('"');
                out.push(':');
                out.push('"');
                i = k + 3;
                continue;
            }
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    if out == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(out)
    }
}

/// Split `"key:value"` into `"key":"value"` when followed by `,` or `}`.
///
/// Detects quoted strings that contain a colon where the content before the
/// colon is a valid bare key and the content after is a valid bare value,
/// and the string is followed by structural punctuation.
pub fn fix_colon_in_key(text: &str) -> Cow<'_, str> {
    let Some(open_pos) = needs_colon_fix(text) else {
        return Cow::Borrowed(text);
    };
    let n = text.len();
    let mut out = String::with_capacity(n);
    out.push_str(&text[..open_pos]);
    let mut i = open_pos;
    let bytes = text.as_bytes();
    while i < n {
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            let content_start = i;
            let mut has_colon = false;
            while i < n && bytes[i] != b'"' {
                if bytes[i] == b':' {
                    has_colon = true;
                }
                i += 1;
            }
            let content_end = i;
            if i < n {
                i += 1;
            }
            if has_colon {
                let mut j = i;
                while j < n && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n && matches!(bytes[j], b',' | b'}') {
                    let content_str = &text[content_start..content_end];
                    if let Some(colon_pos) = content_str.find(':') {
                        let key = &content_str[..colon_pos];
                        let val = &content_str[colon_pos + 1..];
                        if !key.is_empty()
                            && !val.is_empty()
                            && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                            && val.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        {
                            out.push('"');
                            out.push_str(key);
                            out.push_str("\":\"");
                            out.push_str(val);
                            out.push('"');
                            out.push_str(&text[i..j]);
                            out.push(bytes[j] as char);
                            out.push_str(&text[j + 1..]);
                            return Cow::Owned(out);
                        }
                    }
                }
            }
            out.push_str(&text[start..i]);
        } else {
            let ch = char_at(text, i);
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    if out == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(out)
    }
}
