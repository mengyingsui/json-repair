//! Streaming JSON repairer state machine and core types.
//!
//! This module implements the single-pass repair algorithm. The top-level
//! [`Repairer`] struct holds the input/parse state and produces
//! repaired JSON via [`Repairer::repair`](Repairer::repair). Submodules
//! handle specific repair concerns:
//!
//! - `comment` — inline and block comment removal
//! - `junk` — trailing/comma junk handling
//! - `keys` — unquoted key parsing
//! - `literal` — unquoted `true`/`false`/`null` / `Infinity` / `NaN`
//! - `number` — number parsing and normalization
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

/// Extra stack slots beyond `MAX_PARSE_DEPTH` so `object_loop`/`array_loop`
/// can push a frame before checking the depth limit.
const STACK_OVERHEAD: usize = 8;

/// Capacity of the bracket and parse-frame stacks.
const STACK_CAPACITY: usize = MAX_PARSE_DEPTH + STACK_OVERHEAD;

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
#[derive(Clone, Copy)]
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
/// state.  The [`Repairer::repair`] method drives the full repair; all
/// other methods are internal helpers called from it.
pub(crate) struct Repairer {
    /// Input text stored as a `String`; `self.i` indexes by byte (char boundary).
    text: String,
    /// Byte length of `text` (cached to avoid repeated `.len()` calls).
    n: usize,
    /// Current read cursor into `text` (always on a UTF-8 char boundary).
    i: usize,
    /// Repaired JSON output buffer.
    out: String,
    /// Stack of expected closing brackets (`}` or `]`) for open containers.
    brackets: [char; STACK_CAPACITY],
    /// Number of valid entries in `brackets`.
    brackets_len: usize,
    /// Whether the parser expects a key (inside an object, before `:`).
    expect_key: bool,
    /// Whether a value was just emitted (used for comma insertion logic).
    just_emitted_value: bool,

    /// Byte offset in `out` of the last position at depth 0 (for suffix
    /// junk trimming).
    last_depth0_pos: usize,
    /// Deferred error (set by helpers, checked by the main loop).
    error: Option<JsonRepairError>,
    /// Current string-state-machine state.
    state: ParserState,
    /// Cached position of the `"` after the bareword lookahead in
    /// `is_closing_quote`, reused by `parse_string` to avoid a redundant scan.
    lookahead_pos: Option<usize>,
}

impl Repairer {
    /// Create a new repairer for `text`.
    pub(crate) fn new(text: &str) -> Self {
        let text = text.to_string();
        let n = text.len();
        Repairer {
            text,
            n,
            i: 0,
            out: String::with_capacity(n.min(1 << 18)),
            brackets: ['\0'; STACK_CAPACITY],
            brackets_len: 0,
            expect_key: false,
            just_emitted_value: false,

            last_depth0_pos: 0,
            error: None,
            state: ParserState::Normal,
            lookahead_pos: None,
        }
    }

    /// Peek at the char `offset` positions ahead of the cursor (`\0` at EOF).
    fn peek(&self, offset: usize) -> char {
        self.text[self.i..].chars().nth(offset).unwrap_or('\0')
    }

    /// Char at the current cursor position (`\0` at EOF).
    #[inline]
    fn cur(&self) -> char {
        self.char_at(self.i)
    }

    /// Char at byte position `pos` (`\0` if out of bounds).
    #[inline]
    fn char_at(&self, pos: usize) -> char {
        if pos < self.n {
            crate::preprocess::char_at(&self.text, pos)
        } else {
            '\0'
        }
    }

    /// Check whether the next `s.len()` chars match `s`.
    ///
    /// Only correct for ASCII patterns (all call sites pass ASCII literals
    /// like `"\"\"\""`, `"--"`, `"//"`).  **Panics** on non-ASCII patterns —
    /// `s.len()` counts bytes, not chars, so the assertion prevents
    /// silent under-compare.
    fn peek_is(&self, s: &str) -> bool {
        assert!(
            s.is_ascii(),
            "peek_is: non-ASCII pattern {s:?} — use char count instead"
        );
        self.text[self.i..].starts_with(s)
    }

    /// Append a single char to `out`.
    fn emit_char(&mut self, c: char) {
        self.out.push(c);
    }

    /// Advance `self.i` past ASCII whitespace.
    fn skip_ws(&mut self) {
        while self.i < self.n && self.text.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// Return `pos` advanced past any ASCII whitespace (including `\n`).
    #[inline]
    fn skip_ws_at(&self, mut pos: usize) -> usize {
        while pos < self.n && self.text.as_bytes()[pos].is_ascii_whitespace() {
            pos += 1;
        }
        pos
    }

    /// Remove a trailing comma from `self.out` if present.
    ///
    /// Called before emitting a closing bracket (`}` or `]`) to avoid
    /// producing invalid JSON like `{"a":1,}`.
    #[inline]
    fn trim_trailing_comma(&mut self) {
        if self.out.ends_with(',') {
            self.out.pop();
        }
    }

    /// Whether a comma separator is needed before the next element.
    ///
    /// Encapsulates the common conditions shared by `object_loop` and
    /// `array_loop`: not the first element, a value was just emitted,
    /// and the output does not already end with a separator or opening
    /// bracket.  Callers add their own structural-char checks (e.g.
    /// `ch != ']'` in arrays) on top of this.
    #[inline]
    fn needs_separator(&self, first: bool) -> bool {
        if first || !self.just_emitted_value {
            return false;
        }
        !matches!(self.out.as_bytes().last(), Some(b',' | b'{' | b'['))
    }

    /// Write `\uXXXX` (the JSON escape for a control or non-ASCII char) to
    /// `self.out`.
    #[inline]
    fn emit_unicode_escape(&mut self, code: u32) {
        let _ = write!(self.out, "\\u{:04x}", code);
    }

    /// Push a closing bracket onto the bracket stack.
    #[inline]
    fn brackets_push(&mut self, c: char) {
        debug_assert!(self.brackets_len < STACK_CAPACITY, "bracket stack overflow");
        self.brackets[self.brackets_len] = c;
        self.brackets_len += 1;
    }

    /// Pop the top closing bracket from the bracket stack.
    #[inline]
    fn brackets_pop(&mut self) -> Option<char> {
        if self.brackets_len == 0 {
            None
        } else {
            self.brackets_len -= 1;
            Some(self.brackets[self.brackets_len])
        }
    }

    /// Peek at the top closing bracket without removing it.
    #[inline]
    fn brackets_last(&self) -> Option<char> {
        if self.brackets_len == 0 {
            None
        } else {
            Some(self.brackets[self.brackets_len - 1])
        }
    }

    /// Record the last depth-0 position when the bracket stack empties.
    ///
    /// Used after emitting a closing bracket to track where suffix-junk
    /// trimming should cut.
    #[inline]
    fn update_depth0(&mut self) {
        if self.brackets_len == 0 {
            self.last_depth0_pos = self.out.len();
        }
    }

    /// Pop and emit all remaining open brackets (close truncated containers).
    fn close_brackets(&mut self) {
        while let Some(b) = self.brackets_pop() {
            self.trim_trailing_comma();
            self.emit_char(b);
        }
        debug_assert!(
            self.brackets_len == 0,
            "close_brackets: unclosed brackets remain"
        );
        self.last_depth0_pos = self.out.len();
    }

    /// Append a `&str` to `out`.
    fn emit_str(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Parse one value (primitive, string, number, object, array).
    /// For objects/arrays, pushes the iteration frames onto the stack.
    fn run_value(&mut self, stack: &mut Stack) {
        self.skip_ws();
        if self.i >= self.n {
            self.emit_str("null");
            return;
        }

        let ch = self.cur();
        match ch {
            '{' => {
                self.emit_char('{');
                self.brackets_push('}');
                self.i += 1;
                let prev_expect = self.expect_key;
                self.expect_key = true;
                self.object_loop(stack, prev_expect, true);
            }
            '[' => {
                self.emit_char('[');
                self.brackets_push(']');
                self.i += 1;
                self.array_loop(stack, true);
            }
            '"' => {
                if self.peek_is("\"\"\"") && self.text[self.i + 3..].contains("\"\"\"") {
                    self.parse_triple_string();
                    return;
                }
                self.parse_string();
            }
            '\'' => self.parse_single_quoted_string(),
            't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U' => self.parse_literal(),
            '-' => {
                if self.peek_is("--") {
                    self.skip_comment();
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
                    if self.i < self.n && self.cur() == ':' {
                        self.emit_char(':');
                        self.i += 1;
                    } else if self.i < self.n && self.cur() != ':' {
                        self.emit_char(':');
                    }
                    self.expect_key = false;
                    stack.push(ParseFrame::Value);
                } else if ch.is_ascii_alphabetic() || ch == '_' {
                    self.parse_unquoted_value();
                } else {
                    self.i += ch.len_utf8();
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

        let mut stack = Stack::new();

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
            const MAX_VALIDATION_DEPTH: usize = 100;
            let bracket_depth = out.chars().filter(|&c| c == '{' || c == '[').count();
            if bracket_depth <= MAX_VALIDATION_DEPTH {
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

/// Fixed-capacity stack of parse frames, avoiding heap allocation.
pub(super) struct Stack {
    frames: [Option<ParseFrame>; STACK_CAPACITY],
    len: usize,
}

impl Stack {
    fn new() -> Self {
        Stack {
            frames: [const { None }; STACK_CAPACITY],
            len: 0,
        }
    }

    #[inline]
    fn push(&mut self, frame: ParseFrame) {
        debug_assert!(self.len < STACK_CAPACITY);
        self.frames[self.len] = Some(frame);
        self.len += 1;
    }

    #[inline]
    fn pop(&mut self) -> Option<ParseFrame> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            self.frames[self.len].take()
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
