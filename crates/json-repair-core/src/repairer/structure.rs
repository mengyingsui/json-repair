use super::{ParseFrame, Repairer};

impl Repairer {
    /// Continue an object loop after a nested value has been completed.
    pub(super) fn resume_object(
        &mut self,
        stack: &mut Vec<ParseFrame>,
        prev_expect: bool,
    ) {
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
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
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
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char('}');
                let popped = self.brackets.pop();
                debug_assert_eq!(
                    popped, Some('}'),
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
                if j >= self.n || "},\u{5d}:".contains(self.chars[j]) {
                    self.i += 1;
                    continue;
                }
            }
            if ch == ']' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
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
                if !first
                    && ch != '"'
                    && ch != '_'
                    && ch != '/'
                    && ch != '\''
                    && !ch.is_ascii_alphabetic()
                {
                    break;
                }
                if ch.is_ascii_alphabetic() {
                    let mut j = self.i + 1;
                    while j < self.n
                        && (self.chars[j].is_alphanumeric() || self.chars[j] == '_')
                    {
                        j += 1;
                    }
                    while j < self.n
                        && (self.chars[j] == ' '
                            || self.chars[j] == '\t'
                            || self.chars[j] == '\r')
                    {
                        j += 1;
                    }
                    if j >= self.n || !",\":".contains(self.chars[j]) {
                        break;
                    }
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
                // Parse value and come back
                stack.push(ParseFrame::ResumeObject { prev_expect });
                stack.push(ParseFrame::Value);
                return;
            } else {
                if !first
                    && ch != '"'
                    && ch != '{'
                    && ch != '['
                    && ch != '\''
                    && ch != 't'
                    && ch != 'f'
                    && ch != 'n'
                    && ch != 'T'
                    && ch != 'F'
                    && ch != 'N'
                    && ch != 'i'
                    && ch != 'I'
                    && ch != 'u'
                    && ch != 'U'
                    && ch != '-'
                    && ch != '.'
                    && !ch.is_ascii_digit()
                {
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
                // Parse value and come back
                stack.push(ParseFrame::ResumeObject { prev_expect });
                stack.push(ParseFrame::Value);
                return;
            }
        }
        if self.i < self.n && self.brackets.last() == Some(&'}') {
            if self.out.ends_with(',') {
                self.out.pop();
                self.out_chars -= 1;
            }
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
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                break;
            }
            let ch = self.chars[self.i];
            if ch == ']' {
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
                self.emit_char(']');
                let popped = self.brackets.pop();
                debug_assert_eq!(
                    popped, Some(']'),
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
                if self.out.ends_with(',') {
                    self.out.pop();
                    self.out_chars -= 1;
                }
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

    pub(super) fn resume_implicit_array(
        &mut self,
        stack: &mut Vec<ParseFrame>,
        first: bool,
    ) {
        self.just_emitted_value = true;
        self.skip_ws();
        if self.i < self.n && self.chars[self.i] == ',' {
            self.i += 1;
        }
        self.implicit_array_loop(stack, first);
    }

    pub(super) fn implicit_array_loop(
        &mut self,
        stack: &mut Vec<ParseFrame>,
        first: bool,
    ) {
        self.skip_ws();
        if self.i < self.n && self.chars[self.i] == '{' {
            if !first {
                self.emit_char(',');
            }
            stack.push(ParseFrame::ResumeImplicitArray { first: false });
            stack.push(ParseFrame::Value);
            return;
        }
        if !first && self.out.ends_with(',') {
            self.out.pop();
            self.out_chars -= 1;
        }
        self.emit_char(']');
    }
}
