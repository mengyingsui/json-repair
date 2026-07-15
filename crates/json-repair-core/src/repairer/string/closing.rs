//! Closing-quote heuristics — deciding whether a `"` ends the string or
//! is an embedded (unescaped) quote, plus bareword-split detection.
//!
//! These functions probe the input at arbitrary positions without
//! mutating the cursor, so they can be called from both the main parse
//! loop ([`super`]) and lookahead helpers in `structure.rs`.

use crate::repairer::{InputCursor, OutputBuffer, Tracer};

use super::escape::ParserState;

// Heuristic: does `"` at byte position `pos` close the string or is it
// an embedded (unescaped) quote?  Uses explicit `pos` instead of
// `input.pos()` so callers (like `peek_quoted_key_at`) can probe at
// arbitrary positions without mutating the cursor.
// Returns `(is_closing, bareword_pos)`.
//
// `bareword_pos` is set when a `bareword":` pattern is found after
// the quote — used by `try_split_bareword_after_value`.
pub(crate) fn check_closing_quote(
    input: &InputCursor,
    pos: usize,
    is_key: bool,
    brackets: &[char],
    tracer: &mut Tracer,
) -> (bool, Option<usize>) {
    let _ = tracer;
    // For values, scan backward: if `{` or `[` appears before any other
    // visible char, the quote is the start of a string, not a closer.
    if !is_key {
        let mut p = pos;
        while p > 0 {
            p -= 1;
            let byte = input.bytes()[p];
            if byte == b'{' || byte == b'[' {
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::StringSplit {
                        position: pos,
                        reason: "embedded_quote",
                    }
                );
                return (false, None);
            }
            if !matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                break;
            }
        }
    }
    // Look ahead past horizontal whitespace
    let mut j = pos + 1;
    while input
        .char_at(j)
        .is_some_and(|c| matches!(c, ' ' | '\t' | '\r'))
    {
        j += 1;
    }
    // EOF → closing (truncated string completion)
    let Some(nc) = input.char_at(j) else {
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::StringSplit {
                position: pos,
                reason: "closing_quote",
            }
        );
        return (true, None);
    };
    // Succeeded by structural char → likely a closing quote
    if matches!(nc, ',' | '}' | ']' | '\n') {
        // Comma is ambiguous: check that the char after the comma
        // is a valid value-starter — otherwise the quote was embedded.
        if nc == ',' && !is_key {
            let k = input.skip_ws_at(j + 1);
            if input.char_at(k).is_some_and(|after| {
                !matches!(
                    after,
                    '"' | '{' | '[' | 't' | 'f' | 'n' | '-' | '}' | ']' | ','
                ) && !after.is_ascii_digit()
            }) {
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::StringSplit {
                        position: pos,
                        reason: "embedded_quote",
                    }
                );
                return (false, None);
            }
        }
        // `]`/`}` after a quote: check whether it's an embedded
        // bracket-inside-string, not the container closer.
        if (nc == ']' || nc == '}') && is_embedded_bracket_quote(input, j, nc, brackets) {
            emit_trace!(
                tracer,
                crate::trace::TraceEvent::StringSplit {
                    position: pos,
                    reason: "embedded_quote",
                }
            );
            return (false, None);
        }
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::StringSplit {
                position: pos,
                reason: "closing_quote",
            }
        );
        return (true, None);
    }
    // Another `"` immediately → closing (doubled `""` is a quote).
    if nc == '"' {
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::StringSplit {
                position: pos,
                reason: "closing_quote",
            }
        );
        return (true, None);
    }
    // Key-specific: `:`, `{`, `[` after quote → closing
    if is_key && matches!(nc, ':' | '{' | '[') {
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::StringSplit {
                position: pos,
                reason: "closing_quote",
            }
        );
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
            match input.bytes()[p] {
                b'{' | b'[' => {
                    emit_trace!(
                        tracer,
                        crate::trace::TraceEvent::StringSplit {
                            position: pos,
                            reason: "embedded_quote",
                        }
                    );
                    return (false, None);
                }
                b',' | b'}' | b']' | b':' => break,
                _ => {}
            }
        }
        if pos > 0 {
            let prev = input.bytes()[pos - 1];
            if prev.is_ascii_graphic() || prev == b' ' || prev == b'_' || prev == b'"' {
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::StringSplit {
                        position: pos,
                        reason: "closing_quote",
                    }
                );
                return (true, None);
            }
        }
        emit_trace!(
            tracer,
            crate::trace::TraceEvent::StringSplit {
                position: pos,
                reason: "embedded_quote",
            }
        );
        return (false, None);
    }
    // Bareword after quote: could be `key":value` (split point) or
    // a continuation of an unquoted string.
    if nc.is_ascii_alphabetic() || nc == '_' {
        let mut k = j;
        while let Some(ch) = input.char_at(k) {
            if !(ch.is_alphanumeric() || ch == '_') {
                break;
            }
            k += ch.len_utf8();
        }
        k = input.skip_ws_at(k);
        if matches!(input.char_at(k), Some('"')) {
            let bp = Some(k);
            k += 1;
            k = input.skip_ws_at(k);
            if matches!(input.char_at(k), Some(':')) {
                // In key context, scan between this quote and the
                // bareword's ending `"`.  If no structural separator
                // (`,`, `}`, `]`, `:`, `\n`) exists in that span, the
                // entire span is one key with embedded quotes — don't
                // close here.  E.g. `"step"valxt":` → key is `step"valxt`.
                if is_key {
                    let mut p = pos + 1;
                    let mut has_sep = false;
                    while p < k {
                        let Some(c) = input.char_at(p) else { break };
                        if matches!(c, ',' | '}' | ']' | ':' | '\n') {
                            has_sep = true;
                            break;
                        }
                        p += c.len_utf8();
                    }
                    if !has_sep {
                        emit_trace!(
                            tracer,
                            crate::trace::TraceEvent::StringSplit {
                                position: pos,
                                reason: "embedded_quote",
                            }
                        );
                        return (false, None);
                    }
                }
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::StringSplit {
                        position: pos,
                        reason: "bareword_split",
                    }
                );
                return (true, bp);
            }
        }
    }
    emit_trace!(
        tracer,
        crate::trace::TraceEvent::StringSplit {
            position: pos,
            reason: "embedded_quote",
        }
    );
    (false, None)
}

// Check whether `]`/`}` after a quote is actually the *start* of an
// embedded bracket-inside-string, not the container closer.
// E.g. `["]}]` — the `]` and `}` are inside the string value.
//
// Returns `true` when the bracket is "embedded" (i.e. not structural).
fn is_embedded_bracket_quote(input: &InputCursor, j: usize, nc: char, brackets: &[char]) -> bool {
    // Skip past any consecutive brackets
    let mut k = j;
    while input.char_at(k).is_some_and(|c| matches!(c, ']' | '}')) {
        k += 1;
    }
    k = input.skip_ws_at(k);
    // If the run ends at EOF or another structural char, not embedded
    let structural = input
        .char_at(k)
        .is_none_or(|c| matches!(c, ',' | '}' | ']'));
    if structural {
        return false;
    }
    // If the bracket doesn't match the stack top, it's embedded
    let matches_top = brackets.last().copied() == Some(nc);
    if !matches_top {
        return true;
    }
    // Matches top: scan ahead for the mirror opener, treating
    // `"short_string",` as the interior of an embedded container.
    let open_bracket = if nc == ']' { '[' } else { '{' };
    let mut p = k;
    while p < input.len() {
        if matches!(input.char_at(p), Some('"')) {
            let q = input.skip_ws_at(p + 1);
            if input
                .char_at(q)
                .is_some_and(|c| matches!(c, ',' | '}' | ']'))
            {
                let need_colon = nc == '}' && matches!(input.char_at(q), Some(','));
                let mut r = q;
                let mut in_str = false;
                let mut depth: i32 = 1;
                let mut saw_colon = false;
                while r < input.len() {
                    let Some(rc) = input.char_at(r) else { break };
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
        let Some(pc) = input.char_at(p) else { break };
        p += pc.len_utf8();
    }
    false
}

/// Outcome of [`handle_double_quote_escape`].
pub(super) enum DoubleQuoteAction {
    /// A `""` byte pair was consumed — `\"` was emitted, cursor advanced.
    Consumed,
    /// No double-quote pattern at the current position.
    NotDoubleQuote,
}

pub(super) fn handle_double_quote_escape(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
) -> DoubleQuoteAction {
    if input.pos() + 1 >= input.len() || input.bytes()[input.pos() + 1] != b'"' {
        return DoubleQuoteAction::NotDoubleQuote;
    }
    output.emit_str("\\\"");
    input.advance(1);
    DoubleQuoteAction::Consumed
}

/// Outcome of [`try_split_bareword_after_value`].
pub(super) enum SplitResult {
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
pub(super) fn try_split_bareword_after_value(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    state: &mut ParserState,
    bareword_quote_pos: Option<usize>,
    tracer: &mut Tracer,
) -> SplitResult {
    let _ = tracer;
    if input.pos() + 1 >= input.len() {
        return SplitResult::NoSplit;
    }
    let nc = char::from(input.bytes()[input.pos() + 1]);
    if !(nc.is_ascii_alphabetic() || nc == '_') {
        return SplitResult::NoSplit;
    }
    let k = bareword_quote_pos.unwrap_or_else(|| {
        let mut k = input.pos() + 1;
        while let Some(kc) = input.char_at(k) {
            if !(kc.is_alphanumeric() || kc == '_') {
                break;
            }
            k += kc.len_utf8();
        }
        input.skip_ws_at(k)
    });
    if input.char_at(k) != Some('"') {
        return SplitResult::NoSplit;
    }
    // Undo the closing quote we just emitted, plus any trailing whitespace.
    output.pop();
    output.trim_trailing_whitespace();
    output.trim_trailing_comma();
    output.emit_char('"');
    *state = ParserState::Normal;
    emit_trace!(
        tracer,
        crate::trace::TraceEvent::StringSplit {
            position: input.pos(),
            reason: "value_bareword_split",
        }
    );
    SplitResult::Split
}

// EOF while parsing string body — close the string.
pub(super) fn ensure_closing_quote(output: &mut OutputBuffer, state: &mut ParserState) {
    *state = ParserState::Normal;
    output.emit_char('"');
}

#[cfg(test)]
#[allow(clippy::let_unit_value, clippy::unused_unit)]
mod tests {
    use super::*;

    fn cursor(text: &str) -> InputCursor<'_> {
        InputCursor::new(text)
    }

    fn empty_brackets() -> Vec<char> {
        Vec::new()
    }

    fn tracer() -> Tracer<'static> {
        #[cfg(feature = "tracing")]
        {
            None
        }
        #[cfg(not(feature = "tracing"))]
        {
            ()
        }
    }

    fn check_q(
        input: &InputCursor,
        pos: usize,
        is_key: bool,
        brackets: &[char],
    ) -> (bool, Option<usize>) {
        let mut t = tracer();
        check_closing_quote(input, pos, is_key, brackets, &mut t)
    }

    // ── check_closing_quote: value context (is_key=false) ──────────────

    #[test]
    fn closing_quote_value_eof() {
        // `"value"` at EOF → closing
        let input = cursor(r#""value""#);
        let (is_closing, _) = check_q(&input, 7, false, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn closing_quote_value_followed_by_comma() {
        // `"value",` → closing (comma + value starter `"`)
        let input = cursor(r#""value", "next""#);
        let (is_closing, _) = check_q(&input, 6, false, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn closing_quote_value_followed_by_close_brace() {
        // `"value"}` → closing
        let input = cursor(r#""value"}"#);
        let (is_closing, _) = check_q(&input, 6, false, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn closing_quote_value_followed_by_close_bracket() {
        // `"value"]` → closing
        let input = cursor(r#""value"]"#);
        let (is_closing, _) = check_q(&input, 6, false, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn closing_quote_value_followed_by_newline() {
        // `"value"\n` → closing
        let input = cursor("\"value\"\n");
        let (is_closing, _) = check_q(&input, 6, false, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn embedded_quote_value_followed_by_alpha() {
        // `"say "hello"` — `"` followed by `hello` → embedded
        let input = cursor(r#"{"msg":"say "hello""}"#);
        let pos = input.text().find(r#""hello"#).unwrap();
        let mut t = tracer();
        let (is_closing, _) = check_closing_quote(&input, pos, false, &empty_brackets(), &mut t);
        assert!(!is_closing, "quote before `hello` should be embedded");
    }

    #[test]
    fn closing_quote_value_doubled_quote() {
        // `""` → closing (the second `"` closes an empty string)
        let input = cursor(r#"""#);
        let mut t = tracer();
        let (is_closing, _) = check_closing_quote(&input, 0, false, &empty_brackets(), &mut t);
        assert!(is_closing);
    }

    // ── check_closing_quote: key context (is_key=true) ─────────────────

    #[test]
    fn closing_quote_key_followed_by_colon() {
        // `"key":` → closing in key context
        let input = cursor(r#""key":"#);
        let (is_closing, _) = check_q(&input, 4, true, &empty_brackets());
        assert!(is_closing);
    }

    #[test]
    fn closing_quote_key_followed_by_open_brace() {
        // `"key"{` → closing in key context
        let input = cursor(r#""key"{"#);
        let (is_closing, _) = check_q(&input, 4, true, &empty_brackets());
        assert!(is_closing);
    }

    // ── check_closing_quote: bareword split ────────────────────────────

    #[test]
    fn closing_quote_value_then_bareword_then_quote_colon() {
        // `"value"key":` → first `"` closes, bareword becomes next key
        let input = cursor(r#""value"key":"#);
        let (is_closing, bareword_pos) = check_q(&input, 6, false, &empty_brackets());
        assert!(is_closing);
        assert!(bareword_pos.is_some(), "should detect bareword split point");
    }
}
