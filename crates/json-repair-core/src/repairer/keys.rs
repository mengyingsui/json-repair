//! Object key parsing — quoted, single-quoted, and bareword keys.
//!
//! [`parse_key`] dispatches to the string parser for quoted keys and to
//! [`parse_unquoted_key`] / [`parse_unquoted_value`] for bareword tokens.

use super::{InputCursor, OutputBuffer, Tracer, string};

// Parse a JSON object key: quoted (double/single) or bareword.
pub(super) fn parse_key(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    brackets: &[char],
    tracer: &mut Tracer,
) {
    input.skip_ws();
    let Some(ch) = input.cur() else { return };
    if ch == '"' {
        string::parse_string(input, output, brackets, true, tracer);
    } else if ch == '\'' {
        string::parse_single_quoted_string(input, output, tracer);
    } else {
        parse_unquoted_key(input, output, tracer);
    }
}

// Consume characters while `is_stop` returns false.
// Each consumed character is escaped and emitted via [`emit_unquoted_char`].
// Silently skips control characters that cannot appear in JSON at all
// (U+0000-U+0008, U+000B-U+000C, U+000E-U+001F) but preserves `\n`,
// `\r`, and `\t` which are valid JSON string content.
fn emit_bare_word(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    is_stop: impl Fn(char) -> bool,
) {
    while let Some(ch) = input.cur() {
        if is_stop(ch) {
            break;
        }
        let cv = u32::from(ch);
        if cv < 0x20 && !matches!(cv, 0x09 | 0x0A | 0x0D) {
            input.advance(ch.len_utf8());
            continue;
        }
        string::emit_unquoted_char(output, ch);
        input.advance(ch.len_utf8());
    }
}

// Parse an unquoted (bare) key: wrap in `"..."`, stop at structural
// characters.  Includes ZWSP (U+200B) as a stop so copy-pasted
// invisible characters don't leak into the key.
pub(super) fn parse_unquoted_key(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    output.emit_char('"');
    emit_bare_word(input, output, |ch| {
        ch.is_ascii()
            && matches!(
                ch,
                ' ' | '\t' | '\r' | '\n' | ':' | '{' | '}' | '[' | ']' | ',' | '"' | '\'' | '/'
            )
            || ch == '\u{200b}'
    });
    output.emit_char('"');
    // Consume a trailing `"` if present (from the original input)
    if matches!(input.cur(), Some('"')) {
        input.advance(1);
    }
}

// Parse an unquoted value (e.g. bareword after `:` that is not a
// literal or number).  Only stops at structural separators.
// `]` is a stop only when the next byte is NOT alphanumeric (i.e.
// the `]` is structural, not part of the value content).
pub(super) fn parse_unquoted_value(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    emit_trace!(
        tracer,
        crate::trace::TraceEvent::ValueNormalized {
            kind: "unquoted_value_as_string",
        }
    );
    output.emit_char('"');
    loop {
        emit_bare_word(input, output, |ch| matches!(ch, ',' | '}' | ']'));
        if matches!(input.cur(), Some(']')) {
            let next = if input.pos() + 1 < input.len() {
                input.bytes()[input.pos() + 1]
            } else {
                0
            };
            if next.is_ascii_alphanumeric() || next == b'_' {
                string::emit_unquoted_char(output, ']');
                input.advance(1);
                continue;
            }
        }
        break;
    }
    output.emit_char('"');
}
