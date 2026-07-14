use super::{ParseFrame, Repairer, Stack, comment, keys, string};

// Characters that can start an object key (quoted or bareword).
#[inline]
fn is_key_start(ch: char) -> bool {
    matches!(ch, '"' | '_' | '/' | '\'') || ch.is_ascii_alphabetic()
}

impl Repairer<'_> {
    // Quick scan: does the text at `self.i + 1` look like a key
    // (alphanumeric + underscore → optionally followed by `,` / `"` / `:`)?
    fn looks_like_key(&self) -> bool {
        let mut j = self.input.i + 1;
        loop {
            let ch = self.input.char_at(j);
            if !(ch.is_alphanumeric() || ch == '_') {
                break;
            }
            j += ch.len_utf8();
        }
        while j < self.input.text.len() && matches!(self.input.char_at(j), ' ' | '\t' | '\r' | '\n')
        {
            j += 1;
        }
        j < self.input.text.len() && matches!(self.input.char_at(j), ',' | '"' | ':' | '}')
    }

    /// Walk through the value string at `self.input.i` and return `true`
    /// when its closing quote is followed by `:`, meaning the string is
    /// actually a key and the preceding key should get an implicit null.
    ///
    /// Save/restore `self.input.i` so parsing state is unchanged.
    fn peek_quoted_key_at(&mut self) -> bool {
        let saved = self.input.i;
        if saved >= self.input.text.len() || self.input.char_at(saved) != '"' {
            return false;
        }
        self.input.i += 1;
        let mut result = false;
        while self.input.i < self.input.text.len() {
            let c = self.input.cur();
            if c == '\\' {
                self.input.i += 1;
                if self.input.i < self.input.text.len() {
                    self.input.i += self.input.cur().len_utf8();
                }
                continue;
            }
            if c == '"' {
                let (is_closing, _) =
                    string::check_closing_quote(&self.input, false, &self.brackets);
                if is_closing {
                    let mut k = self.input.i + 1;
                    while k < self.input.text.len()
                        && matches!(self.input.char_at(k), ' ' | '\t' | '\r')
                    {
                        k += 1;
                    }
                    result = k < self.input.text.len() && self.input.char_at(k) == ':';
                }
                break;
            }
            self.input.i += c.len_utf8();
        }
        self.input.i = saved;
        result
    }

    // Emit a closing bracket, trimming the trailing comma first.
    // Asserts the bracket matches the top of the stack.
    fn close_bracket(&mut self, bracket: char) {
        self.output.trim_trailing_comma();
        self.output.emit_char(bracket);
        let popped = self.brackets.pop();
        debug_assert_eq!(
            popped,
            Some(bracket),
            "close_bracket: closing {bracket:?} but top of stack is not {bracket:?}"
        );
        self.update_depth0();
    }

    // Consume a bracket that doesn't match the expected closer.
    // Pops the expected bracket from the stack anyway and emits it.
    // Returns `true` if the stack was non-empty.
    fn try_consume_mismatched_bracket(&mut self) -> bool {
        self.output.trim_trailing_comma();
        let popped = self.brackets.pop();
        match popped {
            Some(b) => {
                self.output.emit_char(b);
                self.update_depth0();
                self.input.i += 1;
                true
            }
            _ => {
                self.input.i += 1;
                false
            }
        }
    }

    // Push a new container frame onto the stack and emit the opening bracket.
    // The `resume_frame` is pushed beneath the new loop frame so the state
    // machine returns to the right loop when the value completes.
    fn push_container(&mut self, stack: &mut Stack, ch: char, resume_frame: ParseFrame) {
        match ch {
            '{' => {
                self.output.emit_char('{');
                self.brackets.push('}');
                self.input.i += 1;
                stack.push(resume_frame);
                stack.push(ParseFrame::ObjectLoop(0));
            }
            '[' => {
                self.output.emit_char('[');
                self.brackets.push(']');
                self.input.i += 1;
                stack.push(resume_frame);
                stack.push(ParseFrame::ArrayLoop(0));
            }
            _ => unreachable!("push_container called with non-container char"),
        }
    }

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
                self.close_bracket('}');
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
                self.try_consume_mismatched_bracket();
                return;
            }
            if expect_key {
                // No key-starter found — break out (may be junk or implicit close)
                if count > 0 && !is_key_start(ch) {
                    break;
                }
                // Bareword that doesn't look like a key — probably a value
                if count > 0 && ch.is_ascii_alphabetic() && !self.looks_like_key() {
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
                    self.push_container(stack, vch, ParseFrame::ObjectLoop(count + 1));
                    return;
                }
                if vch == '}' {
                    self.output.emit_str("null");
                    self.input.i += 1;
                    stack.push(ParseFrame::ObjectLoop(count + 1));
                    return;
                }
                if vch == '"' && self.peek_quoted_key_at() {
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
            self.close_bracket('}');
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
                self.close_bracket(']');
                self.input.i += 1;
                return;
            }
            // `}` inside array — could be mismatch or nested object
            if ch == '}' {
                if self.try_consume_mismatched_bracket() {
                    return;
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
                    self.push_container(stack, ch, ParseFrame::ArrayLoop(count + 1));
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
