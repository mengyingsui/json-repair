//! Streaming JSON repairer state machine and core types.
//!
//! This module implements the single-pass repair algorithm. The top-level
//! [`Repairer`](self::Repairer) struct holds the input/parse state and produces
//! repaired JSON via [`Repairer::repair`](self::Repairer::repair). Sub-modules
//! handle specific repair concerns:
//!
//! - `comment` — inline and block comment removal
//! - `junk` — trailing/comma junk handling
//! - `keys` — unquoted key parsing
//! - `literal` — unquoted `true`/`false`/`null` / `Infinity` / `NaN`
//! - `number` — number parsing and normalisation
//! - `string` — string parsing with embedded-quote detection
//! - `structure` — object/array frame management

mod comment;
mod junk;
mod keys;
mod literal;
mod number;
mod string;
mod structure;

use std::fmt::Write;

use crate::error::JsonRepairError;

/// Maximum nesting depth for objects/arrays before the repairer gives up.
const MAX_PARSE_DEPTH: usize = 512;

/// Current parser state for the streaming string-state machine.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ParserState {
    /// Outside any string; normal structural parsing.
    Normal,
    /// Inside a string body (between opening and closing quotes).
    InString,
    /// Just consumed a `\`; the next char is an escape payload.
    InStringEscaped,
}

/// Stack frame for the iterative (non-recursive) parse loop.
///
/// Each variant represents a "come back here after the current value is
/// fully parsed" resumption point.  The main loop pops frames and dispatches
/// to the corresponding `resume_*` method.
pub(crate) enum ParseFrame {
    /// Parse a fresh value (the entry point for any JSON value).
    Value,
    /// Resume an object loop after a member value completes.
    ResumeObject { prev_expect: bool },
    /// Resume an array loop after an element completes.
    ResumeArray,
    /// Resume an implicit-array loop (comma-separated top-level objects).
    ResumeImplicitArray { first: bool },
}

/// Single-pass streaming JSON repairer.
///
/// Holds the input char slice, output buffer, bracket stack, and parse
/// state.  The [`Repairer::repair`](self::Repairer::repair) method drives the
/// full repair; all other methods are internal helpers called from it.
pub(crate) struct Repairer {
    /// Input text decomposed into a `Vec<char>` for O(1) indexing.
    chars: Vec<char>,
    /// Length of `chars` (cached to avoid repeated `.len()` calls).
    n: usize,
    /// Current read cursor into `chars`.
    i: usize,
    /// Repaired JSON output buffer.
    out: String,
    /// Stack of expected closing brackets (`}` or `]`) for open containers.
    brackets: Vec<char>,
    /// Whether the parser expects a key (inside an object, before `:`).
    expect_key: bool,
    /// Whether a value was just emitted (used for comma insertion logic).
    just_emitted_value: bool,
    /// Byte length of `out` (cached; `String::len()` is O(1) but this avoids
    /// re-fetching and serves as a sync invariant via `debug_assert_eq!`).
    out_chars: usize,
    /// Byte offset in `out` of the last position at depth 0 (for suffix
    /// junk trimming).
    last_depth0_pos: usize,
    /// Deferred error (set by helpers, checked by the main loop).
    error: Option<JsonRepairError>,
    /// Current string-state-machine state.
    state: ParserState,
}

impl Repairer {
    /// Create a new repairer for `text`, pre-decomposing it into chars.
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

    /// Peek at the char `offset` positions ahead of the cursor (`\0` at EOF).
    fn peek(&self, offset: usize) -> char {
        let pos = self.i + offset;
        if pos < self.n { self.chars[pos] } else { '\0' }
    }

    /// Check whether the next `s.len()` chars match `s`.
    ///
    /// Only correct for ASCII patterns (all call sites pass ASCII literals
    /// like `"\"\"\""`, `"--"`, `"//"`).  A non-ASCII pattern would silently
    /// under-compare because `s.len()` counts bytes, not chars.
    fn peek_is(&self, s: &str) -> bool {
        debug_assert!(
            s.is_ascii(),
            "peek_is: non-ASCII pattern {s:?} — use char count instead"
        );
        let end = self.i + s.len();
        if end > self.n {
            return false;
        }
        for (j, b) in s.bytes().enumerate() {
            if self.chars[self.i + j] != b as char {
                return false;
            }
        }
        true
    }

    /// Append a single char to `out` and update the byte counter.
    fn emit_char(&mut self, c: char) {
        self.out.push(c);
        self.out_chars += c.len_utf8();
        debug_assert_eq!(
            self.out.len(),
            self.out_chars,
            "out_chars byte counter out of sync with output buffer"
        );
    }

    /// Advance `self.i` past ASCII whitespace.
    fn skip_ws(&mut self) {
        while self.i < self.n && self.chars[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// Remove a trailing comma from `self.out` if present.
    ///
    /// Called before emitting a closing bracket (`}` or `]`) to avoid
    /// producing invalid JSON like `{"a":1,}`.
    #[inline]
    fn trim_trailing_comma(&mut self) {
        if self.out.ends_with(',') {
            self.out.pop();
            self.out_chars -= 1;
        }
    }

    /// Write `\uXXXX` (the JSON escape for a control or non-ASCII char) to
    /// `self.out` and update the byte counter.
    ///
    /// Centralises the `write!(self.out, "\\u{:04x}", …) + out_chars += 6`
    /// pattern so the escape length is maintained in exactly one place.
    #[inline]
    fn emit_unicode_escape(&mut self, code: u32) {
        let _ = write!(self.out, "\\u{:04x}", code);
        self.out_chars += 6;
    }

    /// Pop and emit all remaining open brackets (close truncated containers).
    fn close_brackets(&mut self) {
        while let Some(b) = self.brackets.pop() {
            self.trim_trailing_comma();
            self.emit_char(b);
        }
        debug_assert!(
            self.brackets.is_empty(),
            "close_brackets: unclosed brackets remain"
        );
        self.last_depth0_pos = self.out_chars;
        debug_assert!(
            self.last_depth0_pos <= self.out.len(),
            "last_depth0_pos exceeds output length"
        );
    }

    /// Append a `&str` to `out` and update the byte counter.
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
            't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U' => self.parse_literal(),
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

    /// Run the full single-pass repair on the input text.
    ///
    /// Returns `Ok(valid_json)` on success, or `Err` if the input is
    /// catastrophically malformed (e.g. parse depth exceeded).  In debug
    /// builds, the result is additionally validated as parseable JSON.
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
        #[cfg(debug_assertions)]
        {
            if !Repairer::is_output_balanced(&out) {
                return Err(JsonRepairError {
                    message: "repaired output has unbalanced brackets".to_string(),
                    position: None,
                });
            }
            let bracket_depth = out.chars().filter(|&c| c == '{' || c == '[').count();
            if bracket_depth <= 100 {
                if let Err(e) = serde_json::from_str::<serde_json::Value>(&out) {
                    debug_assert!(
                        false,
                        "repair result is not valid JSON (depth={}): {}\n---\n{}\n---",
                        bracket_depth, e, out
                    );
                }
            }
        }
        Ok(out)
    }
}

impl Repairer {
    /// Check that every `{`/`[` in `s` has a matching `}`/`]`, respecting
    /// string boundaries and escape sequences.  Used only in debug builds.
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    fn is_output_balanced(s: &str) -> bool {
        let mut stack = vec![];
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
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' if stack.pop() != Some(c) => {
                    return false;
                }
                _ => {}
            }
        }
        stack.is_empty()
    }
}
