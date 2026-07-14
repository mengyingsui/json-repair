use super::{
    BracketStack, InputCursor, OutputBuffer, ParseFrame, Repairer, Stack, comment, keys, string,
};

// Characters that can start an object key (quoted or bareword).
fn is_key_start(ch: char) -> bool {
    matches!(ch, '"' | '_' | '/' | '\'') || ch.is_ascii_alphabetic()
}

// Quick scan: does the text at `input.i + 1` look like a key
// (alphanumeric + underscore → optionally followed by `,` / `"` / `:`)?
fn looks_like_key(input: &InputCursor) -> bool {
    let mut j = input.i + 1;
    loop {
        let ch = input.char_at(j);
        if !(ch.is_alphanumeric() || ch == '_') {
            break;
        }
        j += ch.len_utf8();
    }
    while j < input.text.len() && matches!(input.char_at(j), ' ' | '\t' | '\r' | '\n') {
        j += 1;
    }
    j < input.text.len() && matches!(input.char_at(j), ',' | '"' | ':' | '}')
}

// Walk through the value string at `input.i` and return `true`
// when its closing quote is followed by `:`, meaning the string is
// actually a key and the preceding key should get an implicit null.
fn peek_quoted_key_at(input: &InputCursor, brackets: &BracketStack) -> bool {
    let mut i = input.i;
    if i >= input.text.len() || input.char_at(i) != '"' {
        return false;
    }
    i += 1;
    while i < input.text.len() {
        let c = input.char_at(i);
        if c == '\\' {
            i += 1;
            if i < input.text.len() {
                i += input.char_at(i).len_utf8();
            }
            continue;
        }
        if c == '"' {
            let (is_closing, _) = string::check_closing_quote(input, i, false, brackets);
            if is_closing {
                let mut k = i + 1;
                if k < input.text.len() && input.char_at(k) == '"' {
                    k += 1;
                }
                k = input.skip_ws_at(k);
                return k < input.text.len() && input.char_at(k) == ':';
            }
            break;
        }
        i += c.len_utf8();
    }
    false
}

// Emit a closing bracket, trimming the trailing comma first.
// Asserts the bracket matches the top of the stack.
fn close_bracket(output: &mut OutputBuffer, brackets: &mut BracketStack, bracket: char) {
    output.trim_trailing_comma();
    output.emit_char(bracket);
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
    brackets: &mut BracketStack,
    input: &mut InputCursor,
) -> MismatchResult {
    output.trim_trailing_comma();
    let popped = brackets.pop();
    match popped {
        Some(b) => {
            output.emit_char(b);
            if brackets.is_empty() {
                output.set_depth0_pos();
            }
            input.i += 1;
            MismatchResult::Closed
        }
        _ => {
            input.i += 1;
            MismatchResult::NoBracket
        }
    }
}

// Push a new container frame onto the stack and emit the opening bracket.
// The `resume_frame` is pushed beneath the new loop frame so the state
// machine returns to the right loop when the value completes.
fn push_container(
    output: &mut OutputBuffer,
    brackets: &mut BracketStack,
    input: &mut InputCursor,
    stack: &mut Stack,
    ch: char,
    resume_frame: ParseFrame,
) {
    match ch {
        '{' => {
            output.emit_char('{');
            brackets.push('}');
            input.i += 1;
            stack.push(resume_frame);
            stack.push(ParseFrame::ObjectLoop(0));
        }
        '[' => {
            output.emit_char('[');
            brackets.push(']');
            input.i += 1;
            stack.push(resume_frame);
            stack.push(ParseFrame::ArrayLoop(0));
        }
        _ => unreachable!("push_container called with non-container char"),
    }
}

impl Repairer<'_> {
    // Main loop for parsing an object (`{…}`).
    // `count` is the number of key/value pairs seen so far.
    pub(super) fn object_loop(&mut self, stack: &mut Stack, count: usize) {
        let mut expect_key = true;
        loop {
            self.input.skip_ws();
            if self.input.i >= self.input.text.len() {
                self.output.trim_trailing_comma();
                break;
            }
            let ch = self.input.cur();
            // Skip redundant `{` and `:` inside the loop
            if expect_key && (ch == '{' || ch == ':') {
                self.input.i += 1;
                continue;
            }
            // Closing `}` — matched closer
            if ch == '}' {
                close_bracket(&mut self.output, &mut self.brackets, '}');
                self.input.i += 1;
                return;
            }
            // Comma separator
            if ch == ',' {
                if count > 0 && !self.output.ends_with(',') {
                    self.output.emit_char(',');
                }
                self.input.i += 1;
                expect_key = true;
                continue;
            }
            if comment::is_comment_start(&self.input, ch) {
                comment::skip_comment(&mut self.input);
                continue;
            }
            // Lone `"` inside non-empty object — could be trailing comma artifact
            if ch == '"' && count > 0 {
                let j = self.input.skip_ws_at(self.input.i + 1);
                if j >= self.input.text.len()
                    || matches!(self.input.char_at(j), '}' | ',' | ']' | ':')
                {
                    self.input.i += 1;
                    continue;
                }
            }
            // `]` inside object is always a mismatch
            if ch == ']' {
                match try_consume_mismatched_bracket(
                    &mut self.output,
                    &mut self.brackets,
                    &mut self.input,
                ) {
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
                if count > 0 && ch.is_ascii_alphabetic() && !looks_like_key(&self.input) {
                    break;
                }
                if count > 0 && self.output.needs_comma_in_output() {
                    self.output.emit_char(',');
                }
                keys::parse_key(&mut self.input, &mut self.output, &self.brackets);
                self.input.skip_ws();
                self.output.emit_char(':');
                if self.input.i < self.input.text.len() && self.input.cur() == ':' {
                    self.input.i += 1;
                }
                self.input.skip_ws();
                if self.input.i >= self.input.text.len() {
                    // Truncated after `key:` — set up resume frames
                    stack.push(ParseFrame::ObjectLoop(count + 1));
                    stack.push(ParseFrame::Value);
                    return;
                }
                let vch = self.input.cur();
                if vch == '{' || vch == '[' {
                    push_container(
                        &mut self.output,
                        &mut self.brackets,
                        &mut self.input,
                        stack,
                        vch,
                        ParseFrame::ObjectLoop(count + 1),
                    );
                    return;
                }
                if vch == '}' {
                    self.output.emit_str("null");
                    self.input.i += 1;
                    stack.push(ParseFrame::ObjectLoop(count + 1));
                    return;
                }
                if vch == '"' && peek_quoted_key_at(&self.input, &self.brackets) {
                    self.output.emit_str("null");
                    stack.push(ParseFrame::ObjectLoop(count + 1));
                    return;
                }
                stack.push(ParseFrame::ObjectLoop(count + 1));
                stack.push(ParseFrame::Value);
                return;
            }
        }
        // Close the object if the bracket stack still expects `}`
        if self.input.i < self.input.text.len() && self.brackets.last() == Some('}') {
            close_bracket(&mut self.output, &mut self.brackets, '}');
        }
    }

    // Main loop for parsing an array (`[…]`).
    pub(super) fn array_loop(&mut self, stack: &mut Stack, count: usize) {
        loop {
            self.input.skip_ws();
            if self.input.i >= self.input.text.len() {
                self.output.trim_trailing_comma();
                break;
            }
            let ch = self.input.cur();
            // Closing `]` — matched closer
            if ch == ']' {
                close_bracket(&mut self.output, &mut self.brackets, ']');
                self.input.i += 1;
                return;
            }
            // `}` inside array — could be mismatched or nested object
            if ch == '}' {
                match try_consume_mismatched_bracket(
                    &mut self.output,
                    &mut self.brackets,
                    &mut self.input,
                ) {
                    MismatchResult::Closed => return,
                    MismatchResult::NoBracket => {}
                }
                continue;
            }
            // Comma separator
            if ch == ',' {
                if count > 0 && !self.output.ends_with(',') {
                    self.output.emit_char(',');
                }
                self.input.i += 1;
                continue;
            }
            if comment::is_comment_start(&self.input, ch) {
                comment::skip_comment(&mut self.input);
                continue;
            }
            // Add implicit comma when a value follows a previous value
            if count > 0
                && self.output.needs_comma_in_output()
                && !self.output.ends_with(':')
                && ch != ']'
            {
                self.output.emit_char(',');
            }

            match ch {
                '{' | '[' => {
                    push_container(
                        &mut self.output,
                        &mut self.brackets,
                        &mut self.input,
                        stack,
                        ch,
                        ParseFrame::ArrayLoop(count + 1),
                    );
                    return;
                }
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
    pub(super) fn implicit_array_loop(&mut self, stack: &mut Stack, count: usize) {
        self.input.skip_ws();
        if self.input.i >= self.input.text.len() {
            if count > 0 {
                self.output.trim_trailing_comma();
            }
            return;
        }
        // Consume optional comma separator
        if count > 0 && self.input.cur() == ',' {
            self.input.i += 1;
            self.input.skip_ws();
        }
        if self.input.i >= self.input.text.len() {
            if count > 0 {
                self.output.trim_trailing_comma();
            }
            return;
        }
        // Parse the next element as a generic JSON value
        if count > 0 {
            self.output.emit_char(',');
        }
        stack.push(ParseFrame::ImplicitArrayLoop(count + 1));
        stack.push(ParseFrame::Value);
    }
}
