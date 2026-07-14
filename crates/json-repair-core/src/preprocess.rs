use std::borrow::Cow;

use memchr::memchr2;

/// Fast character-at-position without decoding the entire string.  ASCII
/// bytes map directly; multi-byte sequences fall back to the slow path.
pub(crate) fn char_at(text: &str, pos: usize) -> char {
    if text.as_bytes()[pos].is_ascii() {
        char::from_u32(u32::from(text.as_bytes()[pos])).unwrap()
    } else {
        text[pos..].chars().next().unwrap()
    }
}

// Detect `',bareword":` pattern inside mixed‑quote sections.
// E.g. `'foo','bar":"baz"` — the `'bar":` is the key for the next value.
// Returns `(bare_start, k)` spanning the bareword.
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

// Emit the transformed token for a mixed‑quote boundary:
// `'","bareword":"`  (closing `'`, comma, opening `"`, bare key, `":"`)
#[inline]
fn emit_mixed_quote_boundary(out: &mut String, text: &str, bare_start: usize, k: usize) {
    out.push('"');
    out.push(',');
    out.push('"');
    out.push_str(&text[bare_start..k]);
    out.push_str("\":\"");
}

// Fix colons that appear *inside* a key string by escaping the
// relevant characters.  Handles patterns like `"key:": value`.
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
    out.push(char::from_u32(u32::from(bytes[ws])).unwrap());
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
            out.push(char::from_u32(u32::from(bytes[i])).unwrap());
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

// ── Preamble normalization (code‑fence / metatag / unbraced‑key wrapping) ──

/// Returns the byte width of a UTF-8 character from its leading byte.
pub(crate) fn utf8_char_len(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

// Skip ASCII whitespace from `pos`, return new position.
fn skip_ws_at(text: &str, mut pos: usize) -> usize {
    let bytes = text.as_bytes();
    let n = text.len();
    while pos < n && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

// Skip a Markdown code fence (``` … ```).  If the language tag is
// "json" or absent, the fence is removed and parsing starts at the
// end-of-first-line.  Non-JSON fences are fully consumed (including
// the closing ```) to strip non-JSON content.
fn try_skip_code_fence(text: &str, pos: usize) -> Option<usize> {
    let n = text.len();
    if pos + 2 >= n || !text.as_bytes()[pos..].starts_with(b"```") {
        return None;
    }
    let mut i = pos + 3;
    let lang_start = i;
    // Consume rest of the opening line
    while i < n && char_at(text, i) != '\n' {
        i += char_at(text, i).len_utf8();
    }
    let is_json = matches!(text[lang_start..i].trim(), "" | "json");
    if i < n {
        i += 1;
    }
    // For non-JSON fences, skip to closing ```
    if !is_json {
        while i < n {
            if i + 2 < n && text.as_bytes()[i..].starts_with(b"```") {
                i += 3;
                break;
            }
            i += char_at(text, i).len_utf8();
        }
    }
    Some(i)
}

// Detect and skip `[TEXT]` metatags and `[text](url)` Markdown links.
// A metatag must be ≤128 chars, contain only alphanum/`_`/`-`, and
// must not contain `{` or `"`.
fn try_skip_metatag_or_link(text: &str, pos: usize) -> Option<usize> {
    debug_assert_eq!(char_at(text, pos), '[');
    let n = text.len();
    let mut depth = 1i32;
    let mut j = pos + 1;
    let mut is_metatag = j < n;
    while j < n && depth > 0 {
        let jc = char_at(text, j);
        match jc {
            '[' => depth += 1,
            ']' => depth -= 1,
            '{' | '"' => is_metatag = false,
            _ => {}
        }
        j += jc.len_utf8();
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
    if j < n && char_at(text, j) == '(' {
        let mut k = j + 1;
        let mut link_depth = 1i32;
        while k < n && link_depth > 0 {
            let kc = char_at(text, k);
            if kc == '(' {
                link_depth += 1;
            }
            if kc == ')' {
                link_depth -= 1;
            }
            k += kc.len_utf8();
        }
        return Some(k);
    }
    None
}

// Scan a double-quoted string from `pos` to its closing `"`,
// respecting `\`-escapes.  Returns the position after the closing `"`,
// or `n` if the string is unclosed.
fn scan_string(text: &str, pos: usize) -> usize {
    debug_assert_eq!(char_at(text, pos), '"');
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
                        let nb = bytes[i];
                        i += if nb.is_ascii() { 1 } else { utf8_char_len(nb) };
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

        let ch = char_at(text, i);
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
            if j < text.len() && char_at(text, j) == ':' && unbraced_start.is_none() {
                unbraced_start = Some(i);
            }
            i = end;
        } else {
            i += ch.len_utf8();
        }
    }
    if i >= text.len() {
        (Cow::Borrowed(text), 0)
    } else {
        debug_assert!(
            i < text.len() && (char_at(text, i) == '{' || char_at(text, i) == '['),
            "normalize_preamble: position does not point at JSON container start"
        );
        (Cow::Borrowed(text), i)
    }
}
