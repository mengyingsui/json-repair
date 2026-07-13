//! Object/array loop methods and nested-container dispatch.

use super::{ParseFrame, Repairer, Stack};

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

    /// Trim trailing comma, emit `bracket`, pop from bracket stack, and
    /// update depth tracking.
    /// Caller must advance `self.i` past the consumed bracket character.
    fn close_bracket(&mut self, bracket: char) {
        self.trim_trailing_comma();
        self.emit_char(bracket);
        let popped = self.brackets_pop();
        debug_assert_eq!(
            popped,
            Some(bracket),
            "close_bracket: closing {bracket:?} but top of stack is not {bracket:?}"
        );
        self.update_depth0();
    }

    /// Try to consume a mismatched closing bracket (e.g. `]` in an object,
    /// `}` in an array) by popping the bracket stack and emitting whatever
    /// is on top.  Returns `true` when the popped bracket was the *expected*
    /// one (meaning the current frame is done).
    fn try_consume_mismatched_bracket(&mut self) -> bool {
        self.trim_trailing_comma();
        let popped = self.brackets_pop();
        match popped {
            Some(b) => {
                self.emit_char(b);
                self.update_depth0();
                self.i += 1;
                true
            }
            _ => {
                self.i += 1;
                false
            }
        }
    }

    /// Push frames for a nested container value at the current position
    /// onto `stack`.  The caller must `return` immediately after this call
    /// so the main loop processes the inner container before resuming the
    /// parent via `resume_frame`.
    fn push_container(&mut self, stack: &mut Stack, ch: char, resume_frame: ParseFrame) {
        match ch {
            '{' => {
                self.emit_char('{');
                self.brackets_push('}');
                self.i += 1;
                stack.push(resume_frame);
                stack.push(ParseFrame::ObjectLoop(0));
            }
            '[' => {
                self.emit_char('[');
                self.brackets_push(']');
                self.i += 1;
                stack.push(resume_frame);
                stack.push(ParseFrame::ArrayLoop(0));
            }
            _ => unreachable!("push_container called with non-container char"),
        }
    }

    /// Object loop: processes one element at a time.
    /// `count` = number of elements already processed (0 = first).
    /// Always expects a key on entry.
    pub(super) fn object_loop(&mut self, stack: &mut Stack, count: usize) {
        let mut expect_key = true;
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.cur();
            if expect_key && (ch == '{' || ch == ':') {
                self.i += 1;
                continue;
            }
            if ch == '}' {
                self.close_bracket('}');
                self.i += 1;
                return;
            }
            if ch == ',' {
                if count > 0 && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                expect_key = true;
                continue;
            }
            if self.is_comment_start(ch) {
                self.skip_comment();
                continue;
            }
            // Orphan opening quote after a value: if " is followed by a
            // structural char, skip it.
            if ch == '"' && count > 0 {
                let j = self.skip_ws_at(self.i + 1);
                if j >= self.n || matches!(self.char_at(j), '}' | ',' | ']' | ':') {
                    self.i += 1;
                    continue;
                }
            }
            if ch == ']' {
                self.try_consume_mismatched_bracket();
                return;
            }
            if expect_key {
                if count > 0 && !is_key_start(ch) {
                    break;
                }
                if ch.is_ascii_alphabetic() && !self.looks_like_key() {
                    break;
                }
                if count > 0 && self.needs_comma_in_output() {
                    self.emit_char(',');
                }
                self.parse_key();
                self.skip_ws();
                self.emit_char(':');
                if self.i < self.n && self.cur() == ':' {
                    self.i += 1;
                }
                // Handle the value — check for nested containers
                self.skip_ws();
                if self.i >= self.n {
                    stack.push(ParseFrame::ObjectLoop(count + 1));
                    stack.push(ParseFrame::Value);
                    return;
                }
                let vch = self.cur();
                match vch {
                    '{' | '[' => {
                        self.push_container(stack, vch, ParseFrame::ObjectLoop(count + 1));
                        return;
                    }
                    _ => {
                        stack.push(ParseFrame::ObjectLoop(count + 1));
                        stack.push(ParseFrame::Value);
                        return;
                    }
                }
            }
        }
        if self.i < self.n && self.brackets_last() == Some('}') {
            self.close_bracket('}');
        }
    }

    /// Array loop: processes one element at a time.
    /// `count` = number of elements already processed (0 = first).
    pub(super) fn array_loop(&mut self, stack: &mut Stack, count: usize) {
        loop {
            self.skip_ws();
            if self.i >= self.n {
                self.trim_trailing_comma();
                break;
            }
            let ch = self.cur();
            if ch == ']' {
                self.close_bracket(']');
                self.i += 1;
                return;
            }
            if ch == '}' {
                if self.try_consume_mismatched_bracket() {
                    return;
                }
                continue;
            }
            if ch == ',' {
                if count > 0 && !self.out.ends_with(',') {
                    self.emit_char(',');
                }
                self.i += 1;
                continue;
            }
            if self.is_comment_start(ch) {
                self.skip_comment();
                continue;
            }
            if count > 0 && self.needs_comma_in_output() && !self.out.ends_with(':') && ch != ']' {
                self.emit_char(',');
            }

            // Check for nested containers
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

    /// Implicit-array loop: emit comma separators between top-level objects
    /// and close the synthetic `]` when done.
    /// `count` = number of top-level objects already processed (0 = first).
    pub(super) fn implicit_array_loop(&mut self, stack: &mut Stack, count: usize) {
        self.skip_ws();
        if count > 0 && self.i < self.n && self.cur() == ',' {
            self.i += 1;
            self.skip_ws();
        }
        if self.i < self.n && self.cur() == '{' {
            if count > 0 {
                self.emit_char(',');
            }
            stack.push(ParseFrame::ImplicitArrayLoop(count + 1));
            stack.push(ParseFrame::Value);
            return;
        }
        if count > 0 {
            self.trim_trailing_comma();
        }
    }
}
