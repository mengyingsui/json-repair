//! Object/array frame management and value resume logic.

use super::{ParseFrame, Repairer, Stack};

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

/// Whether `ch` can start an object key (quoted, single-quoted, or unquoted bare word).
///
/// Note: comment-start characters (`/`, `#`, `-`) are handled upstream before
/// this function is called and are not included here.
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
        loop {
            let ch = self.char_at(j);
            if !(ch.is_alphanumeric() || ch == '_') {
                break;
            }
            j += ch.len_utf8();
        }
        while j < self.n && matches!(self.char_at(j), ' ' | '\t' | '\r') {
            j += 1;
        }
        j < self.n && matches!(self.char_at(j), ',' | '"' | ':')
    }

    /// Continue an object loop after a nested value has been completed.
    pub(super) fn resume_object(&mut self, stack: &mut Stack, prev_expect: bool) {
        self.expect_key = true;
        self.just_emitted_value = true;
        self.object_loop(stack, prev_expect, false);
    }

    /// Object loop: processes one element at a time.
    /// Returns when the object is complete or a nested-value parse is needed.
    pub(super) fn object_loop(&mut self, stack: &mut Stack, prev_expect: bool, first: bool) {
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.cur();
            if self.expect_key && (ch == '{' || ch == ':') {
                self.i += 1;
                continue;
            }
            if ch == '}' {
                self.trim_trailing_comma();
                self.emit_char('}');
                let popped = self.brackets_pop();
                debug_assert_eq!(
                    popped,
                    Some('}'),
                    "object_loop: closing }} but top of bracket stack is not }}"
                );
                self.update_depth0();
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
            if self.is_comment_start(ch) {
                self.skip_comment();
                continue;
            }
            if ch == '"' && self.just_emitted_value {
                let j = self.skip_ws_at(self.i + 1);
                if j >= self.n || matches!(self.char_at(j), '}' | ',' | ']' | ':') {
                    self.i += 1;
                    continue;
                }
            }
            if ch == ']' {
                self.trim_trailing_comma();
                let popped = self.brackets_pop();
                match popped {
                    Some('}') => {
                        self.emit_char('}');
                        self.update_depth0();
                        self.i += 1;
                        self.just_emitted_value = true;
                        self.expect_key = prev_expect;
                        return;
                    }
                    Some(']') => {
                        // Defensive: unreachable in practice (object_loop runs
                        // only when the innermost bracket is an object `}`),
                        // but kept for consistency with `array_loop`.
                        self.emit_char(']');
                        self.update_depth0();
                        self.i += 1;
                        self.just_emitted_value = true;
                        self.expect_key = prev_expect;
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
                if self.needs_separator(first) {
                    self.emit_char(',');
                }
                self.parse_key();
                self.skip_ws();
                if self.i < self.n && self.cur() == ':' {
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
                if self.needs_separator(first) && ch != '}' && ch != ']' && ch != ',' {
                    self.emit_char(',');
                }
                stack.push(ParseFrame::ResumeObject { prev_expect });
                stack.push(ParseFrame::Value);
                return;
            }
        }
        if self.i < self.n && self.brackets_last() == Some('}') {
            self.trim_trailing_comma();
            self.emit_char('}');
            self.brackets_pop();
            self.just_emitted_value = true;
        }
        self.expect_key = prev_expect;
    }

    /// Continue an array loop after a nested value has been completed.
    pub(super) fn resume_array(&mut self, stack: &mut Stack) {
        self.just_emitted_value = true;
        self.array_loop(stack, false);
    }

    /// Array loop: processes one element at a time.
    /// Returns when the array is complete or a nested-value parse is needed.
    pub(super) fn array_loop(&mut self, stack: &mut Stack, first: bool) {
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.cur();
            if ch == ']' {
                self.trim_trailing_comma();
                self.emit_char(']');
                let popped = self.brackets_pop();
                debug_assert_eq!(
                    popped,
                    Some(']'),
                    "array_loop: closing ] but top of bracket stack is not ]"
                );
                self.update_depth0();
                self.i += 1;
                self.just_emitted_value = true;
                return;
            }
            if ch == '}' {
                self.trim_trailing_comma();
                let popped = self.brackets_pop();
                match popped {
                    Some(']') => {
                        self.emit_char(']');
                        self.update_depth0();
                        self.i += 1;
                        self.just_emitted_value = true;
                        return;
                    }
                    Some('}') => {
                        self.emit_char('}');
                        self.update_depth0();
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
            if self.is_comment_start(ch) {
                self.skip_comment();
                continue;
            }
            if self.needs_separator(first) && !self.out.ends_with(':') && ch != ']' {
                self.emit_char(',');
            }
            // Parse value and come back
            stack.push(ParseFrame::ResumeArray);
            stack.push(ParseFrame::Value);
            return;
        }
    }

    /// Resume an implicit-array loop after a top-level object completes.
    pub(super) fn resume_implicit_array(&mut self, stack: &mut Stack, first: bool) {
        self.just_emitted_value = true;
        self.skip_ws();
        if self.i < self.n && self.cur() == ',' {
            self.i += 1;
        }
        self.implicit_array_loop(stack, first);
    }

    /// Implicit-array loop: emit comma separators between top-level objects
    /// and close the synthetic `]` when done.
    pub(super) fn implicit_array_loop(&mut self, stack: &mut Stack, first: bool) {
        self.skip_ws();
        if self.i < self.n && self.cur() == '{' {
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
