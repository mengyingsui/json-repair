mod comment;
mod junk;
mod keys;
mod literal;
mod number;
mod string;
mod structure;

use crate::error::JsonRepairError;

const MAX_PARSE_DEPTH: usize = 512;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ParserState {
    Normal,
    InString,
    InStringEscaped,
}

pub(crate) enum ParseFrame {
    Value,
    ResumeObject { prev_expect: bool },
    ResumeArray,
    ResumeImplicitArray { first: bool },
}

pub(crate) struct Repairer {
    chars: Vec<char>,
    n: usize,
    i: usize,
    out: String,
    brackets: Vec<char>,
    expect_key: bool,
    just_emitted_value: bool,
    out_chars: usize,
    last_depth0_pos: usize,
    error: Option<JsonRepairError>,
    state: ParserState,
}

impl Repairer {
    pub(crate) fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        Repairer {
            chars,
            n,
            i: 0,
            out: String::with_capacity(n),
            brackets: Vec::new(),
            expect_key: false,
            just_emitted_value: false,
            out_chars: 0,
            last_depth0_pos: 0,
            error: None,
            state: ParserState::Normal,
        }
    }

    fn peek(&self, offset: usize) -> char {
        let pos = self.i + offset;
        if pos < self.n {
            self.chars[pos]
        } else {
            '\0'
        }
    }

    fn peek_is(&self, s: &str) -> bool {
        let end = self.i + s.chars().count();
        if end > self.n {
            return false;
        }
        for (j, c) in s.chars().enumerate() {
            if self.chars[self.i + j] != c {
                return false;
            }
        }
        true
    }

    fn emit_char(&mut self, c: char) {
        self.out.push(c);
        self.out_chars += c.len_utf8();
        debug_assert_eq!(
            self.out.len(),
            self.out_chars,
            "out_chars byte counter out of sync with output buffer"
        );
    }

    fn skip_ws(&mut self) {
        while self.i < self.n && self.chars[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn close_brackets(&mut self) {
        while let Some(b) = self.brackets.pop() {
            if self.out.ends_with(',') {
                self.out.pop();
                self.out_chars -= 1;
            }
            self.emit_char(b);
        }
        debug_assert!(self.brackets.is_empty(), "close_brackets: unclosed brackets remain");
        self.last_depth0_pos = self.out_chars;
        debug_assert!(
            self.last_depth0_pos <= self.out.len(),
            "last_depth0_pos exceeds output length"
        );
    }

    fn emit_str(&mut self, s: &str) {
        self.out.push_str(s);
        self.out_chars += s.len();
        debug_assert_eq!(
            self.out.len(),
            self.out_chars,
            "out_chars byte counter out of sync with output buffer"
        );
    }

    /// Parse one value (primitive, string, number, object, array).
    /// For objects/arrays, pushes the iteration frames onto the stack.
    fn run_value(&mut self, stack: &mut Vec<ParseFrame>) {
        self.skip_ws();
        if self.i >= self.n {
            self.emit_str("null");
            return;
        }

        let ch = self.chars[self.i];
        match ch {
            '{' => {
                self.emit_char('{');
                self.brackets.push('}');
                self.i += 1;
                let prev_expect = self.expect_key;
                self.expect_key = true;
                self.object_loop(stack, prev_expect, true);
            }
            '[' => {
                self.emit_char('[');
                self.brackets.push(']');
                self.i += 1;
                self.array_loop(stack, true);
            }
            '"' => {
                if self.peek_is("\"\"\"") {
                    let rest: String = self.chars[self.i + 3..].iter().collect();
                    if rest.contains("\"\"\"") {
                        self.parse_triple_string();
                        return;
                    }
                }
                self.parse_string();
            }
            '\'' => self.parse_single_quoted_string(),
            't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U' => {
                self.parse_literal()
            }
            '-' => {
                if self.peek_is("--") {
                    self.skip_comment();
                    // tail-recurse by pushing back to the stack
                    stack.push(ParseFrame::Value);
                } else {
                    self.parse_number();
                }
            }
            '.' | '0'..='9' => self.parse_number(),
            '/' | '#' => {
                self.skip_comment();
                stack.push(ParseFrame::Value);
            }
            '}' | ']' | ',' => {
                self.emit_str("null");
            }
            _ => {
                if self.expect_key && (ch.is_ascii_alphabetic() || ch == '_') {
                    self.parse_unquoted_key();
                    self.skip_ws();
                    if self.i < self.n && self.chars[self.i] == ':' {
                        self.emit_char(':');
                        self.i += 1;
                    } else if self.i < self.n && self.chars[self.i] != ':' {
                        self.emit_char(':');
                    }
                    self.expect_key = false;
                    // After parsing a key, immediately parse the value
                    stack.push(ParseFrame::Value);
                } else if ch.is_ascii_alphabetic() || ch == '_' {
                    self.parse_unquoted_value();
                } else {
                    self.i += 1;
                    stack.push(ParseFrame::Value);
                }
            }
        }
    }

    pub(crate) fn repair(&mut self) -> Result<String, JsonRepairError> {
        self.skip_prefix_junk();
        if self.i >= self.n {
            return Ok(String::new());
        }

        let mut stack: Vec<ParseFrame> = Vec::new();

        if self.is_implicit_object_sequence() {
            self.emit_char('[');
            stack.push(ParseFrame::ResumeImplicitArray { first: true });
        } else {
            stack.push(ParseFrame::Value);
        }

        while let Some(frame) = stack.pop() {
            if let Some(err) = self.error.take() {
                return Err(err);
            }

            let current_depth = stack.len() + 1;
            if current_depth > MAX_PARSE_DEPTH {
                return Err(JsonRepairError {
                    message: format!(
                        "max parse depth of {MAX_PARSE_DEPTH} exceeded at position {}",
                        self.i
                    ),
                    position: Some(self.i),
                });
            }

            match frame {
                ParseFrame::Value => self.run_value(&mut stack),
                ParseFrame::ResumeObject { prev_expect } => {
                    self.resume_object(&mut stack, prev_expect);
                }
                ParseFrame::ResumeArray => {
                    self.resume_array(&mut stack);
                }
                ParseFrame::ResumeImplicitArray { first } => {
                    self.resume_implicit_array(&mut stack, first);
                }
            }
        }

        if let Some(err) = self.error.take() {
            return Err(err);
        }

        self.close_brackets();
        self.skip_suffix_junk();
        let out = std::mem::take(&mut self.out);
        if !Repairer::is_output_balanced(&out) {
            return Err(JsonRepairError {
                message: "repaired output has unbalanced brackets".to_string(),
                position: None,
            });
        }
        #[cfg(debug_assertions)]
        {
            let bracket_depth = out.chars().filter(|&c| c == '{' || c == '[').count();
            if bracket_depth <= 100 {
                if let Err(e) = serde_json::from_str::<serde_json::Value>(&out) {
                    debug_assert!(
                        false,
                        "repair result is not valid JSON (depth={}): {}\n---\n{}\n---",
                        bracket_depth, e, &out
                    );
                }
            }
        }
        Ok(out)
    }
}

impl Repairer {
    fn is_output_balanced(s: &str) -> bool {
        let mut stack: Vec<char> = Vec::new();
        let mut in_string = false;
        let mut esc = false;
        for c in s.chars() {
            if esc {
                esc = false;
                continue;
            }
            if c == '\\' {
                esc = true;
                continue;
            }
            if c == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }
            match c {
                '{' | '[' => stack.push(c),
                '}' if stack.pop() == Some('{') => {}
                '}' => return false,
                ']' if stack.pop() == Some('[') => {}
                ']' => return false,
                _ => {}
            }
        }
        stack.is_empty()
    }
}
