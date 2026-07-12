//! Input pre-processing before the main repair pass.
//!
//! [`preprocess_json`] transforms common LLM quote‑mixing patterns in a single
//! forward scan — it is called once by [`repair_json`](crate::repair_json).
//!
//! The two internal transform steps (mixed‑quote boundary repair and
//! colon‑in‑key splitting) were previously exposed as `fix_mixed_quotes` /
//! `fix_colon_in_key`; they are now private implementation details of
//! `preprocess_json`.

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

/// Detect a mixed-quote boundary `','[bareword]":"` at byte position `pos`.
/// Returns `Some((bare_start, k))` on match, where `bare_start..k` is the
/// bareword span.  `None` when no pattern is found.
#[inline]
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

/// Emit the mixed-quote boundary `","<bareword>":"` into `out`.
#[inline]
fn emit_mixed_quote_boundary(out: &mut String, text: &str, bare_start: usize, k: usize) {
    out.push('"');
    out.push(',');
    out.push('"');
    out.push_str(&text[bare_start..k]);
    out.push_str("\":\"");
}

/// Try to split a colon-in-key string `"key:value"` → `"key":"value"`.
/// `content` is the string body (between `"` delimiters), `end` is the
/// position in the input after the closing `"`, `text` / `bytes` / `n`
/// give the full input.  Writes the fixed span into `out` and returns
/// `Some(ws+1)` (new `i`) on success.
#[inline]
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
    if ws >= n || !matches!(bytes[ws], b',' | b'}') {
        return None;
    }
    out.push('"');
    out.push_str(key);
    out.push_str("\":\"");
    out.push_str(val);
    out.push('"');
    out.push_str(&text[end..ws]);
    out.push(bytes[ws] as char);
    Some(ws + 1)
}

/// Single-pass preprocessor: fixes mixed-quote boundaries AND colon-in-key
/// patterns in one forward scan.
///
/// The key insight is that the colon-in-key scanner must not **skip** past
/// `','` mixed-quote byte positions when it scans for the closing `"` of a
/// string.  Instead, it examines every byte inside the string, so a
/// mixed-quote trigger can be detected **before** the colon check
/// mistakenly uses the bareword's `"` as the string terminator.
///
/// Returns `Cow::Borrowed` when neither pattern is found.
pub(crate) fn preprocess_json(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    let mut modified = false;

    while i < n {
        // ── Mixed-quote pattern: ','[bareword]":" ──
        if let Some((bare_start, k)) = try_mixed_quote_boundary(bytes, n, i) {
            modified = true;
            emit_mixed_quote_boundary(&mut out, text, bare_start, k);
            i = k + 3;
            continue;
        }

        // ── Colon-in-key: "…key:value…" followed by , or } ──
        // Also detects mixed-quote boundaries inside the string content
        // so the colon check does not eat ',' bytes that belong to the
        // mixed-quote pattern.
        if bytes[i] == b'"' {
            let string_start = i;
            let mut j = i + 1;
            let mut has_colon = false;

            let mut processed = false;
            while j < n {
                // Mixed-quote INSIDE string content: the string actually
                // ends before the ',', and the bareword after is a new key.
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

            if !processed {
                out.push_str(&text[string_start..]);
                break;
            }
            continue;
        }

        // ── Default: copy one character ──
        if bytes[i].is_ascii() {
            out.push(bytes[i] as char);
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
