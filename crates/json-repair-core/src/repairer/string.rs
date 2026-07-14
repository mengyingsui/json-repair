//! String parsing — the main parse loops for double-quoted, single-quoted,
//! and triple-quoted strings.
//!
//! This module orchestrates the escape mechanics ([`escape`]) and the
//! closing-quote heuristics ([`closing`]) to consume string tokens from
//! the input and emit valid JSON strings into the output buffer.

mod closing;
mod escape;

pub(crate) use closing::check_closing_quote;
pub(crate) use escape::emit_unquoted_char;

use crate::repairer::{BracketStack, InputCursor, OutputBuffer};

use closing::{
    DoubleQuoteAction, SplitResult, ensure_closing_quote, handle_double_quote_escape,
    try_split_bareword_after_value,
};
use escape::{ParserState, emit_string_body_char};

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
    input.advance(1);
    while input.pos() < input.len() {
        let ch = input.cur();
        if ch == '"' {
            match handle_double_quote_escape(input, output) {
                DoubleQuoteAction::Consumed => continue,
                DoubleQuoteAction::NotDoubleQuote => {}
            }
            let (is_closing, bareword_pos) =
                check_closing_quote(input, input.pos(), is_key, brackets);
            if is_closing {
                output.emit_char('"');
                state = ParserState::Normal;
                match try_split_bareword_after_value(input, output, &mut state, bareword_pos) {
                    SplitResult::Split => return,
                    SplitResult::NoSplit => {}
                }
                input.advance(1);
                debug_assert!(
                    output.ends_with('"'),
                    "parse_string: output missing closing quote"
                );
                return;
            } else {
                output.emit_str("\\\"");
                input.advance(1);
                continue;
            }
        }
        if is_key && ch == '\0' {
            output.emit_unicode_escape(0);
            input.advance(1);
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
    input.advance(3);
    output.emit_char('"');
    while input.pos() < input.len() {
        if input.peek_is("\"\"\"") {
            let after = input.pos() + 3;
            // Avoid false match on `""""` (four quotes)
            if !(after < input.len() && input.char_at(after) == '"') {
                input.advance(3);
                output.emit_char('"');
                return;
            }
        }
        let ch = input.cur();
        if ch == '"' {
            output.emit_str("\\\"");
            input.advance(1);
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
    input.advance(1);
    while input.pos() < input.len() {
        let ch = input.cur();
        if ch == '\'' {
            let mut j = input.pos() + 1;
            while j < input.len() && matches!(input.char_at(j), ' ' | '\t' | '\r') {
                j += 1;
            }
            if j >= input.len() || matches!(input.char_at(j), ',' | '}' | ']' | ':' | '\n') {
                output.emit_char('"');
                input.advance(1);
                return;
            } else {
                // Not a structural closer — keep single quote as literal
                output.emit_char('\'');
                input.advance(1);
                continue;
            }
        }
        if ch == '"' {
            output.emit_str("\\\"");
            input.advance(1);
            continue;
        }
        emit_string_body_char(input, output, &mut state, ch, true);
        continue;
    }
    ensure_closing_quote(output, &mut state);
}
