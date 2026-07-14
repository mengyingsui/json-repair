use crate::repairer::{BracketStack, InputCursor, OutputBuffer};

/// Current parser state for the streaming string-state machine.
#[derive(Clone, Copy, PartialEq)]
enum ParserState {
    /// Outside any string; normal structural parsing.
    Normal,
    /// Inside a string body (between opening and closing quotes).
    InString,
    /// Just consumed a `\`; the next char is an escape payload.
    InStringEscaped,
}

// Control character threshold: U+0000…U+001F must be emitted as \uXXXX.
const CONTROL_CHAR_MAX: u32 = 0x20;
// Unicode surrogates (U+D800–U+DFFF) replaced with \ufffd.
const SURROGATE_LO: u32 = 0xD800;
const SURROGATE_HI: u32 = 0xDFFF;

// Characters that JSON allows as short escapes (\n, \t, \\, etc.).
fn is_valid_escape(ch: char) -> bool {
    matches!(ch, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't')
}

// Emit an escaped character.  If `\u` is followed by 4 hex digits
// the hex value is validated; surrogates are replaced with \ufffd.
pub(super) fn emit_escape(input: &mut InputCursor, output: &mut OutputBuffer, ch: char) {
    if is_valid_escape(ch) {
        output.emit_char('\\');
        output.emit_char(ch);
    } else if ch == 'u' && input.i + 5 <= input.text.len() {
        let mut hex_val: u32 = 0;
        let mut all_hex = true;
        for k in 1..=4 {
            if let Some(d) = input.char_at(input.i + k).to_digit(16) {
                hex_val = (hex_val << 4) | d;
            } else {
                all_hex = false;
                break;
            }
        }
        if all_hex {
            if (SURROGATE_LO..=SURROGATE_HI).contains(&hex_val) {
                output.emit_str("\\ufffd");
                input.i += 4;
            } else {
                output.emit_char('\\');
                output.emit_char('u');
            }
        } else {
            output.emit_str("\\\\");
            output.emit_char(ch);
        }
    } else if u32::from(ch) < CONTROL_CHAR_MAX {
        output.emit_unicode_escape(u32::from(ch));
    } else {
        output.emit_str("\\\\");
        output.emit_char(ch);
    }
}

// Handle `\`-escape inside a string.  External callers also use
// this for single-quoted strings via `single_quote_escape`.
fn handle_escaped(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    state: &mut ParserState,
    ch: char,
    single_quote_escape: bool,
) {
    if single_quote_escape && ch == '\'' {
        output.emit_char('\'');
    } else {
        emit_escape(input, output, ch);
    }
    *state = ParserState::InString;
    input.i += ch.len_utf8();
}

// Emit a character for an unquoted (bare) key/value body.
// Escapes `\`, `"`, newlines, tabs, and other control characters.
pub(super) fn emit_unquoted_char(_input: &mut InputCursor, output: &mut OutputBuffer, ch: char) {
    match ch {
        '\\' => output.emit_str("\\\\"),
        '"' => output.emit_str("\\\""),
        '\n' => output.out.push_str("\\n"),
        '\r' => output.out.push_str("\\r"),
        '\t' => output.out.push_str("\\t"),
        c if u32::from(c) < CONTROL_CHAR_MAX => {
            output.emit_unicode_escape(u32::from(c));
        }
        _ => output.emit_char(ch),
    }
}

// Emit one character of a string body, tracking `\`-escape state.
// Control chars and newlines are replaced with standard JSON escapes.
fn emit_string_body_char(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    state: &mut ParserState,
    ch: char,
    single_quote_escape: bool,
) {
    if *state == ParserState::InString && ch == '\\' {
        *state = ParserState::InStringEscaped;
        input.i += 1;
        return;
    }
    if *state == ParserState::InStringEscaped {
        handle_escaped(input, output, state, ch, single_quote_escape);
        return;
    }
    match ch {
        '\n' => {
            output.out.push_str("\\n");
            input.i += 1;
        }
        '\r' => {
            output.out.push_str("\\r");
            input.i += 1;
        }
        '\t' => {
            output.out.push_str("\\t");
            input.i += 1;
        }
        c if u32::from(c) < CONTROL_CHAR_MAX => {
            output.emit_unicode_escape(u32::from(c));
            input.i += 1;
        }
        _ => {
            output.emit_char(ch);
            input.i += ch.len_utf8();
        }
    }
}

// Heuristic: does `"` at byte position `pos` close the string or is it
// an embedded (unescaped) quote?  Uses explicit `pos` instead of
// `input.i` so callers (like `peek_quoted_key_at`) can probe at
// arbitrary positions without mutating the cursor.
// Returns `(is_closing, bareword_pos)`.
//
// `bareword_pos` is set when a `bareword":` pattern is found after
// the quote — used by `try_split_bareword_after_value`.
pub(super) fn check_closing_quote(
    input: &InputCursor,
    pos: usize,
    is_key: bool,
    brackets: &BracketStack,
) -> (bool, Option<usize>) {
    // For values, scan backward: if `{` or `[` appears before any other
    // visible char, the quote is the start of a string, not a closer.
    if !is_key {
        let mut p = pos;
        while p > 0 {
            p -= 1;
            let byte = input.text.as_bytes()[p];
            if byte == b'{' || byte == b'[' {
                return (false, None);
            }
            if !matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                break;
            }
        }
    }
    // Look ahead past horizontal whitespace
    let mut j = pos + 1;
    while j < input.text.len() && matches!(input.char_at(j), ' ' | '\t' | '\r') {
        j += 1;
    }
    // EOF → closing (truncated string completion)
    if j >= input.text.len() {
        return (true, None);
    }
    let nc = input.char_at(j);
    // Succeeded by structural char → likely a closing quote
    if matches!(nc, ',' | '}' | ']' | '\n') {
        // Comma is ambiguous: check that the char after the comma
        // is a valid value-starter — otherwise the quote was embedded.
        if nc == ',' && !is_key {
            let k = input.skip_ws_at(j + 1);
            if k < input.text.len() {
                let after = input.char_at(k);
                if !matches!(
                    after,
                    '"' | '{' | '[' | 't' | 'f' | 'n' | '-' | '}' | ']' | ','
                ) && !after.is_ascii_digit()
                {
                    return (false, None);
                }
            }
        }
        // `]`/`}` after a quote: check whether it's an embedded
        // bracket-inside-string, not the container closer.
        if (nc == ']' || nc == '}') && is_embedded_bracket_quote(input, j, nc, brackets) {
            return (false, None);
        }
        return (true, None);
    }
    // Another `"` immediately → closing (doubled `""` is a quote).
    if nc == '"' {
        return (true, None);
    }
    // Key-specific: `:`, `{`, `[` after quote → closing
    if is_key && matches!(nc, ':' | '{' | '[') {
        return (true, None);
    }
    // Value context with `:` after quote: the string may actually be a
    // key.  Only close when the char before `"` is printable non-ws;
    // control chars like `\r` in corrupt value body keep the string open.
    // Additionally scan backward: if `{` or `[` appears before any
    // structural separator (`,`, `}`, `]`, `:`), the quote opens a nested
    // key inside an embedded JSON structure — treat as embedded.
    if !is_key && nc == ':' {
        let mut p = pos;
        while p > 0 {
            p -= 1;
            match input.text.as_bytes()[p] {
                b'{' | b'[' => return (false, None),
                b',' | b'}' | b']' | b':' => break,
                _ => {}
            }
        }
        if pos > 0 {
            let prev = input.text.as_bytes()[pos - 1];
            if prev.is_ascii_graphic() || prev == b' ' || prev == b'_' || prev == b'"' {
                return (true, None);
            }
        }
        return (false, None);
    }
    // Bareword after quote: could be `key":value` (split point) or
    // a continuation of an unquoted string.
    if nc.is_ascii_alphabetic() || nc == '_' {
        let mut k = j;
        loop {
            let ch = input.char_at(k);
            if !(ch.is_alphanumeric() || ch == '_') {
                break;
            }
            k += ch.len_utf8();
        }
        k = input.skip_ws_at(k);
        if k < input.text.len() && input.char_at(k) == '"' {
            let bp = Some(k);
            k += 1;
            k = input.skip_ws_at(k);
            if k < input.text.len() && input.char_at(k) == ':' {
                // In key context, scan between this quote and the
                // bareword's ending `"`.  If no structural separator
                // (`,`, `}`, `]`, `:`, `\n`) exists in that span, the
                // entire span is one key with embedded quotes — don't
                // close here.  E.g. `"step"valxt":` → key is `step"valxt`.
                if is_key {
                    let mut p = pos + 1;
                    let mut has_sep = false;
                    while p < k {
                        let c = input.char_at(p);
                        if matches!(c, ',' | '}' | ']' | ':' | '\n') {
                            has_sep = true;
                            break;
                        }
                        p += c.len_utf8();
                    }
                    if !has_sep {
                        return (false, None);
                    }
                }
                return (true, bp);
            }
        }
    }
    (false, None)
}

// Check whether `]`/`}` after a quote is actually the *start* of an
// embedded bracket-inside-string, not the container closer.
// E.g. `["]}]` — the `]` and `}` are inside the string value.
//
// Returns `true` when the bracket is "embedded" (i.e. not structural).
fn is_embedded_bracket_quote(
    input: &InputCursor,
    j: usize,
    nc: char,
    brackets: &BracketStack,
) -> bool {
    // Skip past any consecutive brackets
    let mut k = j;
    while k < input.text.len() && matches!(input.char_at(k), ']' | '}') {
        k += 1;
    }
    k = input.skip_ws_at(k);
    // If the run ends at EOF or another structural char, not embedded
    let structural = k >= input.text.len() || matches!(input.char_at(k), ',' | '}' | ']');
    if structural {
        return false;
    }
    // If the bracket doesn't match the stack top, it's embedded
    let matches_top = brackets.last() == Some(nc);
    if !matches_top {
        return true;
    }
    // Matches top: scan ahead for the mirror opener, treating
    // `"short_string",` as the interior of an embedded container.
    let open_bracket = if nc == ']' { '[' } else { '{' };
    let mut p = k;
    while p < input.text.len() {
        if input.char_at(p) == '"' {
            let q = input.skip_ws_at(p + 1);
            if q < input.text.len() && matches!(input.char_at(q), ',' | '}' | ']') {
                let need_colon = nc == '}' && input.char_at(q) == ',';
                let mut r = q;
                let mut in_str = false;
                let mut depth: i32 = 1;
                let mut saw_colon = false;
                while r < input.text.len() {
                    let rc = input.char_at(r);
                    if in_str {
                        if rc == '"' {
                            in_str = false;
                        }
                        r += rc.len_utf8();
                    } else if rc == '"' {
                        in_str = true;
                        r += 1;
                    } else if rc == nc {
                        depth -= 1;
                        if depth == 0 {
                            return !need_colon || saw_colon;
                        }
                        r += 1;
                    } else if rc == open_bracket {
                        depth += 1;
                        r += 1;
                    } else if rc == ':' {
                        saw_colon = true;
                        r += 1;
                    } else {
                        r += rc.len_utf8();
                    }
                }
                break;
            }
        }
        p += input.char_at(p).len_utf8();
    }
    false
}

/// Outcome of [`handle_double_quote_escape`].
enum DoubleQuoteAction {
    /// A `""` byte pair was consumed — `\"` was emitted, cursor advanced.
    Consumed,
    /// No double-quote pattern at the current position.
    NotDoubleQuote,
}

fn handle_double_quote_escape(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
) -> DoubleQuoteAction {
    if input.i + 1 >= input.text.len() || input.text.as_bytes()[input.i + 1] != b'"' {
        return DoubleQuoteAction::NotDoubleQuote;
    }
    output.emit_str("\\\"");
    input.i += 1;
    DoubleQuoteAction::Consumed
}

/// Outcome of [`try_split_bareword_after_value`].
enum SplitResult {
    /// The value was split — the closing quote was undone and re-emitted,
    /// leaving the bareword as part of an upcoming key.
    Split,
    /// No split was performed.
    NoSplit,
}

// When a `"` is followed by a bareword and then another `":`,
// split the value: emit `"` to end the current value, leaving
// the bareword as part of the following key.
// E.g. `"value_key":...` → first emit `"value"`, then treat
// `_key` as an unparsed key prefix.
fn try_split_bareword_after_value(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    state: &mut ParserState,
    bareword_quote_pos: Option<usize>,
) -> SplitResult {
    if input.i + 1 >= input.text.len() {
        return SplitResult::NoSplit;
    }
    let nc = char::from(input.text.as_bytes()[input.i + 1]);
    if !(nc.is_ascii_alphabetic() || nc == '_') {
        return SplitResult::NoSplit;
    }
    let k = bareword_quote_pos.unwrap_or_else(|| {
        let mut k = input.i + 1;
        loop {
            let kc = input.char_at(k);
            if !(kc.is_alphanumeric() || kc == '_') {
                break;
            }
            k += kc.len_utf8();
        }
        input.skip_ws_at(k)
    });
    if k >= input.text.len() || input.char_at(k) != '"' {
        return SplitResult::NoSplit;
    }
    let _ = output.out.pop();
    let trimmed = output
        .out
        .trim_end_matches(|c: char| c.is_ascii_whitespace())
        .len();
    if trimmed < output.out.len() {
        output.out.truncate(trimmed);
    }
    output.trim_trailing_comma();
    output.emit_char('"');
    *state = ParserState::Normal;
    SplitResult::Split
}

// EOF while parsing string body — close the string.
fn ensure_closing_quote(output: &mut OutputBuffer, state: &mut ParserState) {
    *state = ParserState::Normal;
    output.emit_char('"');
}

// Parse a double-quoted string, detecting embedded (unescaped)
// quotes via `check_closing_quote`.
pub(super) fn parse_string(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    brackets: &BracketStack,
    is_key: bool,
) {
    let mut state = ParserState::InString;
    output.emit_char('"');
    input.i += 1;
    while input.i < input.text.len() {
        let ch = input.cur();
        if ch == '"' {
            match handle_double_quote_escape(input, output) {
                DoubleQuoteAction::Consumed => continue,
                DoubleQuoteAction::NotDoubleQuote => {}
            }
            let (is_closing, bareword_pos) = check_closing_quote(input, input.i, is_key, brackets);
            if is_closing {
                output.emit_char('"');
                state = ParserState::Normal;
                match try_split_bareword_after_value(input, output, &mut state, bareword_pos) {
                    SplitResult::Split => return,
                    SplitResult::NoSplit => {}
                }
                input.i += 1;
                debug_assert!(
                    output.ends_with('"'),
                    "parse_string: output missing closing quote"
                );
                return;
            } else {
                output.emit_str("\\\"");
                input.i += 1;
                continue;
            }
        }
        if is_key && ch == '\0' {
            output.emit_unicode_escape(0);
            input.i += 1;
            output.emit_char('"');
            match try_split_bareword_after_value(input, output, &mut state, None) {
                SplitResult::Split => return,
                SplitResult::NoSplit => {}
            }
            return;
        }
        emit_string_body_char(input, output, &mut state, ch, false);
        continue;
    }
    ensure_closing_quote(output, &mut state);
}

// Parse Python-style `"""..."""` as a double-quoted string.
pub(super) fn parse_triple_string(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    _brackets: &BracketStack,
) {
    let mut state = ParserState::InString;
    input.i += 3;
    output.emit_char('"');
    while input.i < input.text.len() {
        if input.peek_is("\"\"\"") {
            let after = input.i + 3;
            // Avoid false match on `""""` (four quotes)
            if !(after < input.text.len() && input.char_at(after) == '"') {
                input.i += 3;
                output.emit_char('"');
                return;
            }
        }
        let ch = input.cur();
        if ch == '"' {
            output.emit_str("\\\"");
            input.i += 1;
            continue;
        }
        emit_string_body_char(input, output, &mut state, ch, false);
        continue;
    }
    ensure_closing_quote(output, &mut state);
}

// Parse a single-quoted string (`'...'`) as a double-quoted string.
// Emit `'` literally if the next non-ws char isn't structural.
pub(super) fn parse_single_quoted_string(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    _brackets: &BracketStack,
) {
    let mut state = ParserState::InString;
    output.emit_char('"');
    input.i += 1;
    while input.i < input.text.len() {
        let ch = input.cur();
        if ch == '\'' {
            let mut j = input.i + 1;
            while j < input.text.len() && matches!(input.char_at(j), ' ' | '\t' | '\r') {
                j += 1;
            }
            if j >= input.text.len() || matches!(input.char_at(j), ',' | '}' | ']' | ':' | '\n') {
                output.emit_char('"');
                input.i += 1;
                return;
            } else {
                // Not a structural closer — keep single quote as literal
                output.emit_char('\'');
                input.i += 1;
                continue;
            }
        }
        if ch == '"' {
            output.emit_str("\\\"");
            input.i += 1;
            continue;
        }
        emit_string_body_char(input, output, &mut state, ch, true);
        continue;
    }
    ensure_closing_quote(output, &mut state);
}
