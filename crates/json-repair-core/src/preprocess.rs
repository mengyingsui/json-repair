use std::borrow::Cow;

/// Quick byte-level check: is there a quoted string containing `:` followed by `,` or `}`?
fn needs_colon_fix(text: &str) -> bool {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut in_str = false;
    let mut has_colon = false;
    let mut i = 0;
    while i < n {
        match bytes[i] {
            b'"' => {
                if in_str && has_colon {
                    let mut j = i + 1;
                    while j < n && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
                        j += 1;
                    }
                    if j < n && matches!(bytes[j], b',' | b'}') {
                        return true;
                    }
                }
                in_str = !in_str;
                has_colon = false;
            }
            b':' if in_str => has_colon = true,
            _ => {}
        }
        i += 1;
    }
    false
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
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 2 < n && chars[i] == '\'' && chars[i + 1] == ',' && chars[i + 2] == '\'' {
            let after_comma = i + 3;
            let mut k = after_comma;
            while k < n && (chars[k].is_alphanumeric() || chars[k] == '_') {
                k += 1;
            }
            if k > after_comma
                && k + 2 < n
                && chars[k] == '"'
                && chars[k + 1] == ':'
                && chars[k + 2] == '"'
            {
                out.push('"');
                out.push(',');
                out.push('"');
                let word: String = chars[after_comma..k].iter().collect();
                out.push_str(&word);
                out.push('"');
                out.push(':');
                out.push('"');
                i = k + 3;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
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
    if !needs_colon_fix(text) {
        return Cow::Borrowed(text);
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '"' {
            let start = i;
            i += 1;
            let mut content = Vec::new();
            let mut has_colon = false;
            while i < n && chars[i] != '"' {
                if chars[i] == ':' {
                    has_colon = true;
                }
                content.push(chars[i]);
                i += 1;
            }
            if i < n {
                i += 1;
            }
            if has_colon {
                let mut j = i;
                while j < n && chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n && (chars[j] == ',' || chars[j] == '}') {
                    let content_str: String = content.iter().collect();
                    if let Some(colon_pos) = content_str.find(':') {
                        let key = &content_str[..colon_pos];
                        let val = &content_str[colon_pos + 1..];
                        if !key.is_empty()
                            && !val.is_empty()
                            && key.chars().all(|c| c.is_alphanumeric() || c == '_')
                            && val.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            out.push('"');
                            out.push_str(key);
                            out.push_str("\":\"");
                            out.push_str(val);
                            out.push('"');
                            out.push_str(&chars[i..j].iter().collect::<String>());
                            out.push(chars[j]);
                            let rest: String = chars[j + 1..].iter().collect();
                            out.push_str(&rest);
                            return Cow::Owned(out);
                        }
                    }
                }
            }
            out.push_str(&chars[start..i].iter().collect::<String>());
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    if out == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(out)
    }
}
