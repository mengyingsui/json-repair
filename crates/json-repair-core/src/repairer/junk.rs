use crate::repairer::InputCursor;
use memchr::memchr2;

// Heuristic: input resembles an implicit object sequence
// (JSON5-like `{...}{...}{...}` without a wrapping array) when it has
// ≥2 top-level objects.
const IMPLICIT_SEQUENCE_MIN_LENGTH: usize = 128;
const IMPLICIT_SEQUENCE_MIN_COUNT: usize = 2;

// UTF-8 leading-byte → full character width for byte-length jumps
// inside strings.  We avoid char decoding to stay in byte-index space.
fn utf8_char_len(lead: u8) -> usize {
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

// Scan forward from `input.i` (must point at `{`) to decide whether
// the input is a sequence of concatenated objects.  Returns `true`
// when ≥2 top-level `{…}` blocks follow one another with only
// optional commas/whitespace between them.
pub(super) fn is_implicit_object_sequence(input: &InputCursor) -> bool {
    if input.i >= input.text.len() || input.cur() != '{' {
        return false;
    }
    let remaining = input.text.len() - input.i;
    if remaining < IMPLICIT_SEQUENCE_MIN_LENGTH {
        return false;
    }
    let mut j = input.i;
    let mut count = 0;
    let mut depth = 0usize;
    let mut in_string = false;
    let bytes = input.text.as_bytes();
    while j + 1 < input.text.len() {
        // Fast-scan through string contents using memchr for '"' and '\'
        if in_string {
            match memchr2(b'"', b'\\', &bytes[j..]) {
                Some(off) => {
                    j += off;
                    if bytes[j] == b'\\' {
                        j += 1;
                        if j < input.text.len() {
                            let nb = bytes[j];
                            j += if nb.is_ascii() { 1 } else { utf8_char_len(nb) };
                        }
                    } else {
                        in_string = false;
                        j += 1;
                    }
                }
                None => break,
            }
            continue;
        }
        let ch = input.char_at(j);
        if ch == '\\' {
            j += 1;
            if j < input.text.len() {
                j += input.char_at(j).len_utf8();
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            j += ch.len_utf8();
            continue;
        }
        if ch == '{' || ch == '[' {
            depth += 1;
            j += ch.len_utf8();
            continue;
        }
        if ch == '}' || ch == ']' {
            depth = depth.saturating_sub(1);
        }
        // At depth 0, after a `}`, check whether a `{` follows
        if ch == '}' && depth == 0 {
            let mut k = input.skip_ws_at(j + 1);
            // Consume optional comma separator
            if k < input.text.len() && input.char_at(k) == ',' {
                k = input.skip_ws_at(k + 1);
            }
            if k < input.text.len() && input.char_at(k) == '{' {
                count += 1;
                if count >= IMPLICIT_SEQUENCE_MIN_COUNT {
                    return true;
                }
                j = k;
                continue;
            }
        }
        j += ch.len_utf8();
    }
    false
}

// Scan for a top-level `,` followed by `{` — i.e. comma-separated
// objects that are too short for `is_implicit_object_sequence`.
// Uses `{`/`}`/`[`/`]` depth tracking only (no string state), which
// avoids false positives from embedded quotes inside strings.
pub(super) fn is_comma_separated_object_list(input: &InputCursor) -> bool {
    let bytes = input.text.as_bytes();
    let n = input.text.len();
    let mut i = input.i;
    let mut depth: i32 = 0;
    while i < n {
        match bytes[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b',' if depth == 0 => {
                let k = input.skip_ws_at(i + 1);
                if k < n && bytes[k] == b'{' {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

// Scan for a top-level comma separating non-bracket values (e.g.
// `1, 2, 3` or `"hello", "world"`).  Only triggers when no `{` or `[`
// brackets exist.  String tracking uses a simple toggle — sufficient
// for clean non-bracket inputs without embedded quotes.
pub(super) fn is_comma_separated_value_list(input: &InputCursor) -> bool {
    let bytes = input.text.as_bytes();
    let n = input.text.len();
    // Skip if any brackets exist — those cases use the object-specific
    // detection (which handles embedded quotes via bracket-only tracking).
    if bytes[input.i..].iter().any(|&b| b == b'{' || b == b'[') {
        return false;
    }
    let mut i = input.i;
    let mut in_string = false;
    while i < n {
        if in_string {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                // Only close string if `"` is followed by a structural
                // character (after optional horizontal whitespace).
                let mut k = i + 1;
                while k < n && matches!(bytes[k], b' ' | b'\t' | b'\r') {
                    k += 1;
                }
                if k >= n || matches!(bytes[k], b',' | b':' | b'\n') {
                    in_string = false;
                }
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            in_string = true;
        } else if bytes[i] == b',' {
            return true;
        }
        i += 1;
    }
    false
}
