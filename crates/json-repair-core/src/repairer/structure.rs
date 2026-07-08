use super::{ParseFrame, Repairer};

/// Whether `ch` can start a JSON value (string, number, literal, object, array).
#[inline]
fn is_value_start(ch: char) -> bool {
    matches!(
        ch,
        '"' | '{'
            | '['
            | '\''
            | 't'
            | 'f'
            | 'n'
            | 'T'
            | 'F'
            | 'N'
            | 'i'
            | 'I'
            | 'u'
            | 'U'
            | '-'
            | '.'
    ) || ch.is_ascii_digit()
}

/// Whether `ch` can start an object key (quoted, single-quoted, unquoted, or comment).
#[inline]
fn is_key_start(ch: char) -> bool {
    matches!(ch, '"' | '_' | '/' | '\'') || ch.is_ascii_alphabetic()
}

impl Repairer {
    /// Check whether the bare word at `self.i` looks like an object key:
    /// it must be followed by `,`, `"`, or `:` (after optional whitespace).
    ///
    /// Used to disambiguate `"word"` as a key vs. an unquoted string value.
    fn looks_like_key(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.n && (self.chars[j].is_alphanumeric() || self.chars[j] == '_') {
            j += 1;
        }
        while j < self.n && matches!(self.chars[j], ' ' | '\t' | '\r') {
            j += 1;
        }
        j < self.n && matches!(self.chars[j], ',' | '"' | ':')
    }

    /// Continue an object loop after a nested value has been completed.
    pub(super) fn resume_object(&mut self, stack: &mut Vec<ParseFrame>, prev_expect: bool) {
        self.expect_key = true;
        self.just_emitted_value = true;
        self.object_loop(stack, prev_expect, false);
    }

    /// Object loop: processes one element at a time.
    /// Returns when the object is complete or a nested-value parse is needed.
    pub(super) fn object_loop(
        &mut self,
        stack: &mut Vec<ParseFrame>,
        prev_expect: bool,
        first: bool,
    ) {
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.chars[self.i];
            if ch == '{' && self.expect_key {
                self.i += 1;
                continue;
            }
            if ch == ':' && self.expect_key {
                self.i += 1;
                continue;
            }
            if ch == '}' {
                self.trim_trailing_comma();
                self.emit_char('}');
                let popped = self.brackets.pop();
                debug_assert_eq!(
                    popped,
                    Some('}'),
                    "object_loop: closing }} but top of bracket stack is not }}"
                );
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.i += 1;
                self.expect_key = prev_expect;
                self.just_emitted_value = true;
                return;
            }
            if ch == ',' {
                if !first && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                self.expect_key = true;
                continue;
            }
            if ch == '/' || ch == '#' || (ch == '-' && self.peek_is("--")) {
                self.skip_comment();
                continue;
            }
            if ch == '"' && self.just_emitted_value {
                let mut j = self.i + 1;
                while j < self.n && self.chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= self.n || matches!(self.chars[j], '}' | ',' | ']' | ':') {
                    self.i += 1;
                    continue;
                }
            }
            if ch == ']' {
                self.trim_trailing_comma();
                let popped = self.brackets.pop();
                match popped {
                    Some('}') => {
                        self.emit_char('}');
                        if self.brackets.is_empty() {
                            self.last_depth0_pos = self.out_chars;
                        }
                        self.i += 1;
                        self.just_emitted_value = true;
                        self.expect_key = prev_expect;
                        return;
                    }
                    Some(']') => {
                        self.emit_char(']');
                        if self.brackets.is_empty() {
                            self.last_depth0_pos = self.out_chars;
                        }
                        self.i += 1;
                        self.just_emitted_value = true;
                        return;
                    }
                    _ => {
                        self.i += 1;
                        self.just_emitted_value = true;
                        return;
                    }
                }
            }
            if self.expect_key {
                if !first && !is_key_start(ch) {
                    break;
                }
                if ch.is_ascii_alphabetic() && !self.looks_like_key() {
                    break;
                }
                if !first
                    && self.just_emitted_value
                    && !self.out.ends_with(',')
                    && !self.out.ends_with('{')
                    && !self.out.ends_with('[')
                {
                    self.emit_char(',');
                }
                self.parse_key();
                self.skip_ws();
                if self.i < self.n && self.chars[self.i] == ':' {
                    self.emit_char(':');
                    self.i += 1;
                } else {
                    self.emit_char(':');
                }
                self.expect_key = false;
                stack.push(ParseFrame::ResumeObject { prev_expect });
                stack.push(ParseFrame::Value);
                return;
            } else {
                if !first && !is_value_start(ch) {
                    break;
                }
                if !first
                    && self.just_emitted_value
                    && !self.out.ends_with(',')
                    && !self.out.ends_with('{')
                    && !self.out.ends_with('[')
                    && ch != '}'
                    && ch != ']'
                    && ch != ','
                {
                    self.emit_char(',');
                }
                stack.push(ParseFrame::ResumeObject { prev_expect });
                stack.push(ParseFrame::Value);
                return;
            }
        }
        if self.i < self.n && self.brackets.last() == Some(&'}') {
            self.trim_trailing_comma();
            self.emit_char('}');
            self.brackets.pop();
            self.just_emitted_value = true;
        }
        self.expect_key = prev_expect;
    }

    /// Continue an array loop after a nested value has been completed.
    pub(super) fn resume_array(&mut self, stack: &mut Vec<ParseFrame>) {
        self.just_emitted_value = true;
        self.array_loop(stack, false);
    }

    pub(super) fn array_loop(&mut self, stack: &mut Vec<ParseFrame>, first: bool) {
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.chars[self.i];
            if ch == ']' {
                self.trim_trailing_comma();
                self.emit_char(']');
                let popped = self.brackets.pop();
                debug_assert_eq!(
                    popped,
                    Some(']'),
                    "array_loop: closing ] but top of bracket stack is not ]"
                );
                if self.brackets.is_empty() {
                    self.last_depth0_pos = self.out_chars;
                }
                self.i += 1;
                self.just_emitted_value = true;
                return;
            }
            if ch == '}' {
                self.trim_trailing_comma();
                let popped = self.brackets.pop();
                match popped {
                    Some(']') => {
                        self.emit_char(']');
                        if self.brackets.is_empty() {
                            self.last_depth0_pos = self.out_chars;
                        }
                        self.i += 1;
                        self.just_emitted_value = true;
                        return;
                    }
                    Some('}') => {
                        self.emit_char('}');
                        if self.brackets.is_empty() {
                            self.last_depth0_pos = self.out_chars;
                        }
                        self.i += 1;
                        self.just_emitted_value = true;
                        continue;
                    }
                    _ => {
                        self.i += 1;
                        self.just_emitted_value = true;
                        continue;
                    }
                }
            }
            if ch == ',' {
                if !first && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                continue;
            }
            if ch == '/' || ch == '#' || (ch == '-' && self.peek_is("--")) {
                self.skip_comment();
                continue;
            }
            if !first
                && self.just_emitted_value
                && !self.out.ends_with(',')
                && !self.out.ends_with('[')
                && !self.out.ends_with(':')
                && ch != ']'
            {
                self.emit_char(',');
            }
            // Parse value and come back
            stack.push(ParseFrame::ResumeArray);
            stack.push(ParseFrame::Value);
            return;
        }
    }

    pub(super) fn resume_implicit_array(&mut self, stack: &mut Vec<ParseFrame>, first: bool) {
        self.just_emitted_value = true;
        self.skip_ws();
        if self.i < self.n && self.chars[self.i] == ',' {
            self.i += 1;
        }
        self.implicit_array_loop(stack, first);
    }

    pub(super) fn implicit_array_loop(&mut self, stack: &mut Vec<ParseFrame>, first: bool) {
        self.skip_ws();
        if self.i < self.n && self.chars[self.i] == '{' {
            if !first {
                self.emit_char(',');
            }
            stack.push(ParseFrame::ResumeImplicitArray { first: false });
            stack.push(ParseFrame::Value);
            return;
        }
        if !first {
            self.trim_trailing_comma();
        }
        self.emit_char(']');
    }
}
