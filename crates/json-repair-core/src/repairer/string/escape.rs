//! String escape mechanics — emitting escaped characters and tracking
//! the `\`-escape state machine inside string bodies.
//!
//! This module is concerned only with *how* a character is written into
//! the output buffer, not with deciding whether a quote closes the
//! string (that logic lives in [`super::closing`]).

use crate::repairer::{InputCursor, OutputBuffer};

/// Current parser state for the streaming string-state machine.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ParserState {
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
    } else if ch == 'u' && input.pos() + 5 <= input.len() {
        let mut hex_val: u32 = 0;
        let mut all_hex = true;
        for k in 1..=4 {
            if let Some(d) = input.char_at(input.pos() + k).to_digit(16) {
                hex_val = (hex_val << 4) | d;
            } else {
                all_hex = false;
                break;
            }
        }
        if all_hex {
            if (SURROGATE_LO..=SURROGATE_HI).contains(&hex_val) {
                output.emit_str("\\ufffd");
                input.advance(4);
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
    input.advance(ch.len_utf8());
}

// Emit a character for an unquoted (bare) key/value body.
// Escapes `\`, `"`, newlines, tabs, and other control characters.
pub(crate) fn emit_unquoted_char(_input: &mut InputCursor, output: &mut OutputBuffer, ch: char) {
    match ch {
        '\\' => output.emit_str("\\\\"),
        '"' => output.emit_str("\\\""),
        '\n' => output.emit_str("\\n"),
        '\r' => output.emit_str("\\r"),
        '\t' => output.emit_str("\\t"),
        c if u32::from(c) < CONTROL_CHAR_MAX => {
            output.emit_unicode_escape(u32::from(c));
        }
        _ => output.emit_char(ch),
    }
}

// Emit one character of a string body, tracking `\`-escape state.
// Control chars and newlines are replaced with standard JSON escapes.
pub(super) fn emit_string_body_char(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    state: &mut ParserState,
    ch: char,
    single_quote_escape: bool,
) {
    if *state == ParserState::InString && ch == '\\' {
        *state = ParserState::InStringEscaped;
        input.advance(1);
        return;
    }
    if *state == ParserState::InStringEscaped {
        handle_escaped(input, output, state, ch, single_quote_escape);
        return;
    }
    match ch {
        '\n' => {
            output.emit_str("\\n");
            input.advance(1);
        }
        '\r' => {
            output.emit_str("\\r");
            input.advance(1);
        }
        '\t' => {
            output.emit_str("\\t");
            input.advance(1);
        }
        c if u32::from(c) < CONTROL_CHAR_MAX => {
            output.emit_unicode_escape(u32::from(c));
            input.advance(1);
        }
        _ => {
            output.emit_char(ch);
            input.advance(ch.len_utf8());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(text: &str) -> InputCursor<'_> {
        InputCursor::new(text)
    }

    // ── emit_unicode_escape ────────────────────────────────────────────

    #[test]
    fn emit_unicode_escape_basic_ascii() {
        let mut out = OutputBuffer::new(64);
        out.emit_unicode_escape(0x0041); // 'A'
        assert_eq!(out.take(), "\\u0041");
    }

    #[test]
    fn emit_unicode_escape_max_bmp() {
        let mut out = OutputBuffer::new(64);
        out.emit_unicode_escape(0xFFFF);
        assert_eq!(out.take(), "\\uffff");
    }

    #[test]
    fn emit_unicode_escape_zero() {
        let mut out = OutputBuffer::new(64);
        out.emit_unicode_escape(0x0000);
        assert_eq!(out.take(), "\\u0000");
    }

    // ── emit_unquoted_char ─────────────────────────────────────────────

    #[test]
    fn emit_unquoted_char_backslash() {
        let mut out = OutputBuffer::new(64);
        let mut input = cursor("");
        emit_unquoted_char(&mut input, &mut out, '\\');
        assert_eq!(out.take(), "\\\\");
    }

    #[test]
    fn emit_unquoted_char_double_quote() {
        let mut out = OutputBuffer::new(64);
        let mut input = cursor("");
        emit_unquoted_char(&mut input, &mut out, '"');
        assert_eq!(out.take(), "\\\"");
    }

    #[test]
    fn emit_unquoted_char_newline() {
        let mut out = OutputBuffer::new(64);
        let mut input = cursor("");
        emit_unquoted_char(&mut input, &mut out, '\n');
        assert_eq!(out.take(), "\\n");
    }

    #[test]
    fn emit_unquoted_char_plain_ascii() {
        let mut out = OutputBuffer::new(64);
        let mut input = cursor("");
        emit_unquoted_char(&mut input, &mut out, 'A');
        assert_eq!(out.take(), "A");
    }

    #[test]
    fn emit_unquoted_char_control_char() {
        let mut out = OutputBuffer::new(64);
        let mut input = cursor("");
        emit_unquoted_char(&mut input, &mut out, '\x01');
        assert_eq!(out.take(), "\\u0001");
    }

    // ── is_valid_escape ────────────────────────────────────────────────

    #[test]
    fn valid_escape_short_names() {
        assert!(is_valid_escape('"'));
        assert!(is_valid_escape('\\'));
        assert!(is_valid_escape('/'));
        assert!(is_valid_escape('b'));
        assert!(is_valid_escape('f'));
        assert!(is_valid_escape('n'));
        assert!(is_valid_escape('r'));
        assert!(is_valid_escape('t'));
    }

    #[test]
    fn invalid_escape_others() {
        assert!(!is_valid_escape('a'));
        assert!(!is_valid_escape('x'));
        assert!(!is_valid_escape('1'));
        assert!(!is_valid_escape('\n'));
    }
}
