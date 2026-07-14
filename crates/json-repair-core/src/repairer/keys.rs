use super::{BracketStack, InputCursor, OutputBuffer, string};

// Parse a JSON object key: quoted (double/single) or bareword.
pub(super) fn parse_key(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    brackets: &BracketStack,
) {
    input.skip_ws();
    if input.i >= input.text.len() {
        return;
    }
    let ch = input.cur();
    if ch == '"' {
        string::parse_string(input, output, brackets, true);
    } else if ch == '\'' {
        string::parse_single_quoted_string(input, output, brackets);
    } else {
        parse_unquoted_key(input, output);
    }
}

// Consume characters while `is_stop` returns false.
// Each consumed character is escaped and emitted via [`emit_unquoted_char`].
// Silently skips control characters that cannot appear in JSON at all
// (U+0000-U+0008, U+000B-U+000C, U+000E-U+001F) but preserves `\n`,
// `\r`, and `\t` which are valid JSON string content.
#[inline]
fn emit_bare_word(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    is_stop: impl Fn(char) -> bool,
) {
    while input.i < input.text.len() {
        let ch = input.cur();
        if is_stop(ch) {
            break;
        }
        let cv = u32::from(ch);
        if cv < 0x20 && !matches!(cv, 0x09 | 0x0A | 0x0D) {
            input.i += ch.len_utf8();
            continue;
        }
        string::emit_unquoted_char(input, output, ch);
        input.i += ch.len_utf8();
    }
}

// Parse an unquoted (bare) key: wrap in `"..."`, stop at structural
// characters.  Includes ZWSP (U+200B) as a stop so copy-pasted
// invisible characters don't leak into the key.
pub(super) fn parse_unquoted_key(input: &mut InputCursor, output: &mut OutputBuffer) {
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
    if input.i < input.text.len() && input.cur() == '"' {
        input.i += 1;
    }
}

// Parse an unquoted value (e.g. bareword after `:` that is not a
// literal or number).  Only stops at structural separators.
pub(super) fn parse_unquoted_value(input: &mut InputCursor, output: &mut OutputBuffer) {
    output.emit_char('"');
    emit_bare_word(input, output, |ch| matches!(ch, ',' | '}' | ']'));
    output.emit_char('"');
}
