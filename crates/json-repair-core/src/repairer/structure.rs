//! Object and array frame management.
//!
//! Implements the three structural loops — object, array, and implicit
//! top-level array — as free functions that operate only on the
//! sub-structs they need (`InputCursor`, `OutputBuffer`, and the bracket
//! stack).

use super::{InputCursor, OutputBuffer, ParseFrame, Tracer, comment, keys, string};

// Characters that can start an object key (quoted or bareword).
fn is_key_start(ch: char) -> bool {
    matches!(ch, '"' | '_' | '/' | '\'') || ch.is_ascii_alphabetic()
}

// Quick scan: does the text at `input.pos() + 1` look like a key
// (alphanumeric + underscore → optionally followed by `,` / `"` / `:`)?
fn looks_like_key(input: &InputCursor) -> bool {
    let mut j = input.pos() + 1;
    loop {
        match input.char_at(j) {
            Some(ch) if ch.is_alphanumeric() || ch == '_' => j += ch.len_utf8(),
            _ => break,
        }
    }
    while input
        .char_at(j)
        .is_some_and(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
    {
        j += 1;
    }
    matches!(input.char_at(j), Some(',' | '"' | ':' | '}'))
}

// Walk through the value string at `input.pos()` and return `true`
// when its closing quote is followed by `:`, meaning the string is
// actually a key and the preceding key should get an implicit null.
fn peek_quoted_key_at(input: &InputCursor, brackets: &[char], tracer: &mut Tracer) -> bool {
    let mut i = input.pos();
    if input.char_at(i) != Some('"') {
        return false;
    }
    i += 1;
    while let Some(c) = input.char_at(i) {
        if c == '\\' {
            i += 1;
            if let Some(esc) = input.char_at(i) {
                i += esc.len_utf8();
            }
            continue;
        }
        if c == '"' {
            let (is_closing, _) = string::check_closing_quote(input, i, false, brackets, tracer);
            if is_closing {
                let mut k = i + 1;
                if input.char_at(k) == Some('"') {
                    k += 1;
                }
                k = input.skip_ws_at(k);
                return matches!(input.char_at(k), Some(':'));
            }
            break;
        }
        i += c.len_utf8();
    }
    false
}

// Emit a closing bracket, trimming the trailing comma first.
// Asserts the bracket matches the top of the stack.
fn close_bracket(
    output: &mut OutputBuffer,
    brackets: &mut Vec<char>,
    bracket: char,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    output.trim_trailing_comma();
    output.emit_char(bracket);
    emit_trace!(
        tracer,
        crate::trace::TraceEvent::ContainerClosed {
            bracket,
            forced_at_eof: false,
        }
    );
    let popped = brackets.pop();
    debug_assert_eq!(
        popped,
        Some(bracket),
        "close_bracket: closing {bracket:?} but top of stack is not {bracket:?}"
    );
    if brackets.is_empty() {
        output.set_depth0_pos();
    }
}

/// Outcome of [`try_consume_mismatched_bracket`].
enum MismatchResult {
    /// A matching bracket was popped from the stack and emitted.
    Closed,
    /// The bracket stack was empty — nothing to close.
    NoBracket,
}

fn try_consume_mismatched_bracket(
    output: &mut OutputBuffer,
    brackets: &mut Vec<char>,
    input: &mut InputCursor,
    tracer: &mut Tracer,
) -> MismatchResult {
    let _ = tracer;
    output.trim_trailing_comma();
    let popped = brackets.pop();
    #[cfg(feature = "tracing")]
    if let Some(expected) = popped {
        if let Some(found) = input.cur() {
            emit_trace!(
                tracer,
                crate::trace::TraceEvent::MismatchedBracket {
                    expected: Some(expected),
                    found,
                }
            );
        }
    }
    match popped {
        // A matching bracket existed: emit it and record depth-0 position if done.
        Some(b) => {
            output.emit_char(b);
            if brackets.is_empty() {
                output.set_depth0_pos();
            }
            input.advance(1);
            MismatchResult::Closed
        }
        // Nothing to close: just skip the stray closing bracket.
        _ => {
            input.advance(1);
            MismatchResult::NoBracket
        }
    }
}

// Push a new container frame onto the stack and emit the opening bracket.
// The `resume_frame` is pushed beneath the new loop frame so the state
// machine returns to the right loop when the value completes.
fn push_container(
    output: &mut OutputBuffer,
    brackets: &mut Vec<char>,
    input: &mut InputCursor,
    stack: &mut Vec<ParseFrame>,
    ch: char,
    resume_frame: ParseFrame,
) {
    match ch {
        // Object container: emit `{`, push `}` closer, resume outer frame later.
        '{' => {
            output.emit_char('{');
            brackets.push('}');
            input.advance(1);
            stack.push(resume_frame);
            stack.push(ParseFrame::ObjectLoop(0));
        }
        // Array container: emit `[`, push `]` closer, resume outer frame later.
        '[' => {
            output.emit_char('[');
            brackets.push(']');
            input.advance(1);
            stack.push(resume_frame);
            stack.push(ParseFrame::ArrayLoop(0));
        }
        // Only `{` and `[` are valid here; anything else is a programming error.
        _ => unreachable!("push_container called with non-container char"),
    }
}

// Main loop for parsing an object (`{…}`).
// `count` is the number of key/value pairs seen so far.
pub(super) fn object_loop(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    brackets: &mut Vec<char>,
    stack: &mut Vec<ParseFrame>,
    count: usize,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    let mut expect_key = true;
    loop {
        input.skip_ws();
        let Some(ch) = input.cur() else {
            output.trim_trailing_comma();
            break;
        };
        // Skip redundant `{` and `:` inside the loop
        if expect_key && (ch == '{' || ch == ':') {
            input.advance(1);
            continue;
        }
        // Closing `}` — matched closer
        if ch == '}' {
            close_bracket(output, brackets, '}', tracer);
            input.advance(1);
            return;
        }
        // Comma separator
        if ch == ',' {
            if count > 0 && !output.ends_with(',') {
                output.emit_char(',');
            }
            input.advance(1);
            expect_key = true;
            continue;
        }
        if comment::is_comment_start(input, ch) {
            comment::skip_comment(input, tracer);
            continue;
        }
        // Lone `"` inside non-empty object — could be trailing comma artifact
        if ch == '"' && count > 0 {
            let j = input.skip_ws_at(input.pos() + 1);
            if input
                .char_at(j)
                .is_none_or(|c| matches!(c, '}' | ',' | ']' | ':'))
            {
                input.advance(1);
                continue;
            }
        }
        // `]` inside object is always a mismatch
        if ch == ']' {
            match try_consume_mismatched_bracket(output, brackets, input, tracer) {
                MismatchResult::Closed => {}
                MismatchResult::NoBracket => {}
            }
            return;
        }
        if expect_key {
            // No key-starter found — break out (may be junk or implicit close)
            if count > 0 && !is_key_start(ch) {
                break;
            }
            // Bareword that doesn't look like a key — probably a value
            if count > 0 && ch.is_ascii_alphabetic() && !looks_like_key(input) {
                break;
            }
            if count > 0 && output.needs_comma_in_output() {
                output.emit_char(',');
            }
            #[cfg(feature = "tracing")]
            let key_pos = input.pos();
            keys::parse_key(input, output, brackets, tracer);
            input.skip_ws();
            output.emit_char(':');
            if matches!(input.cur(), Some(':')) {
                input.advance(1);
            }
            input.skip_ws();
            let Some(vch) = input.cur() else {
                // Truncated after `key:` — set up resume frames
                stack.push(ParseFrame::ObjectLoop(count + 1));
                stack.push(ParseFrame::Value);
                return;
            };
            if vch == '{' || vch == '[' {
                push_container(
                    output,
                    brackets,
                    input,
                    stack,
                    vch,
                    ParseFrame::ObjectLoop(count + 1),
                );
                return;
            }
            if vch == '}' {
                output.emit_str("null");
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::ImplicitNull {
                        key_position: key_pos,
                    }
                );
                input.advance(1);
                stack.push(ParseFrame::ObjectLoop(count + 1));
                return;
            }
            if vch == '"' && peek_quoted_key_at(input, brackets, tracer) {
                output.emit_str("null");
                emit_trace!(
                    tracer,
                    crate::trace::TraceEvent::ImplicitNull {
                        key_position: input.pos(),
                    }
                );
                stack.push(ParseFrame::ObjectLoop(count + 1));
                return;
            }
            stack.push(ParseFrame::ObjectLoop(count + 1));
            stack.push(ParseFrame::Value);
            return;
        }
    }
    // Close the object if the bracket stack still expects `}`
    if input.pos() < input.len() && brackets.last().copied() == Some('}') {
        close_bracket(output, brackets, '}', tracer);
    }
}

// Main loop for parsing an array (`[…]`).
pub(super) fn array_loop(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    brackets: &mut Vec<char>,
    stack: &mut Vec<ParseFrame>,
    count: usize,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    loop {
        input.skip_ws();
        let Some(ch) = input.cur() else {
            output.trim_trailing_comma();
            break;
        };
        // Closing `]` — matched closer
        if ch == ']' {
            close_bracket(output, brackets, ']', tracer);
            input.advance(1);
            return;
        }
        // `}` inside array — could be mismatched or nested object
        if ch == '}' {
            match try_consume_mismatched_bracket(output, brackets, input, tracer) {
                MismatchResult::Closed => return,
                MismatchResult::NoBracket => {}
            }
            continue;
        }
        // Comma separator
        if ch == ',' {
            if count > 0 && !output.ends_with(',') {
                output.emit_char(',');
            }
            input.advance(1);
            continue;
        }
        if comment::is_comment_start(input, ch) {
            comment::skip_comment(input, tracer);
            continue;
        }
        // Add implicit comma when a value follows a previous value
        if count > 0 && output.needs_comma_in_output() && !output.ends_with(':') && ch != ']' {
            output.emit_char(',');
        }

        match ch {
            // Nested container: push it and resume this array afterwards.
            '{' | '[' => {
                push_container(
                    output,
                    brackets,
                    input,
                    stack,
                    ch,
                    ParseFrame::ArrayLoop(count + 1),
                );
                return;
            }
            // Primitive or string value: handle it then come back for the next element.
            _ => {
                stack.push(ParseFrame::ArrayLoop(count + 1));
                stack.push(ParseFrame::Value);
                return;
            }
        }
    }
}

// Implicit array loop for adjacent or comma-separated top-level values.
// After detecting multiple JSON values (objects/numbers/strings etc.)
// at the top level, wraps each as an array element.
pub(super) fn implicit_array_loop(
    input: &mut InputCursor,
    output: &mut OutputBuffer,
    stack: &mut Vec<ParseFrame>,
    count: usize,
    tracer: &mut Tracer,
) {
    let _ = tracer;
    input.skip_ws();
    if input.pos() >= input.len() {
        if count > 0 {
            output.trim_trailing_comma();
        }
        return;
    }
    // Consume optional comma separator
    if count > 0 && matches!(input.cur(), Some(',')) {
        input.advance(1);
        input.skip_ws();
    }
    if input.pos() >= input.len() {
        if count > 0 {
            output.trim_trailing_comma();
        }
        return;
    }
    // Parse the next element as a generic JSON value
    if count > 0 {
        output.emit_char(',');
    }
    stack.push(ParseFrame::ImplicitArrayLoop(count + 1));
    stack.push(ParseFrame::Value);
}
