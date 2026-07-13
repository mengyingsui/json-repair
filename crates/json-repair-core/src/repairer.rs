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

use crate::error::JsonRepairError;

/// Maximum nesting depth for objects/arrays before the repairer gives up.
const MAX_PARSE_DEPTH: usize = 512;

/// Extra stack slots beyond `MAX_PARSE_DEPTH` so `object_loop`/`array_loop`
/// can push a frame before checking the depth limit.
const STACK_OVERHEAD: usize = 8;

/// Capacity of the bracket and parse-frame stacks.
const STACK_CAPACITY: usize = MAX_PARSE_DEPTH + STACK_OVERHEAD;

/// Initial capacity for the output buffer (capped at 256 KiB).
const INITIAL_OUTPUT_CAP: usize = 256 * 1024;

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
/// Each variant carries only the state needed to resume when the frame is
/// popped — no global flags on `Repairer` are consulted or mutated.
#[derive(Clone, Copy)]
pub(crate) enum ParseFrame {
    /// Parse a fresh value (the entry point for any JSON value).
    Value,
    /// Process the next element of an object.  `usize` = number of elements
    /// already processed (used for comma insertion: `count > 0` → not first).
    ObjectLoop(usize),
    /// Process the next element of an array.  `usize` = number of elements
    /// already processed (comma logic: `count > 0`).
    ArrayLoop(usize),
    /// Process the next top-level object in an implicit-array sequence.
    /// `usize` = number of objects already emitted.
    ImplicitArrayLoop(usize),
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

    /// Byte offset in `out` of the last position at depth 0 (for suffix
    /// junk trimming).
    last_depth0_pos: usize,
    /// Net bracket depth of emitted output: +1 per `brackets_push`, -1 per
    /// `brackets_pop`.  Must be zero after `close_brackets()` — replaces the
    /// `is_output_balanced` scan.
    bracket_depth: i32,
    /// Deferred error (set by helpers, checked by the main loop).
    error: Option<JsonRepairError>,
    /// Current string-state-machine state.
    state: ParserState,
}

/// Decode a hex nibble (0–15) into its ASCII hex character.
#[inline]
fn hex_nibble(v: u32) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    HEX[(v & 0xF) as usize] as char
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
            out: String::with_capacity(n.min(INITIAL_OUTPUT_CAP)),
            brackets: ['\0'; STACK_CAPACITY],
            brackets_len: 0,
            last_depth0_pos: 0,
            bracket_depth: 0,
            error: None,
            state: ParserState::Normal,
        }
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

    /// Whether a comma separator is needed in the output before the next
    /// element, based solely on the last byte of `out` — a pure byte check
    /// with no implicit contracts.
    ///
    /// The caller is responsible for checking element count (e.g. `count > 0`).
    #[inline]
    fn needs_comma_in_output(&self) -> bool {
        !matches!(self.out.as_bytes().last(), Some(b',' | b'{' | b'['))
    }

    /// Write `\uXXXX` (the JSON escape for a control or non-ASCII char) to
    /// `self.out`.
    #[inline]
    fn emit_unicode_escape(&mut self, code: u32) {
        self.out.push_str("\\u");
        self.out.push(hex_nibble(code >> 12));
        self.out.push(hex_nibble(code >> 8));
        self.out.push(hex_nibble(code >> 4));
        self.out.push(hex_nibble(code));
    }

    /// Push a closing bracket onto the bracket stack.
    #[inline]
    fn brackets_push(&mut self, c: char) {
        assert!(self.brackets_len < STACK_CAPACITY, "bracket stack overflow");
        self.brackets[self.brackets_len] = c;
        self.brackets_len += 1;
        self.bracket_depth += 1;
    }

    /// Pop the top closing bracket from the bracket stack.
    #[inline]
    fn brackets_pop(&mut self) -> Option<char> {
        if self.brackets_len == 0 {
            None
        } else {
            self.brackets_len -= 1;
            self.bracket_depth -= 1;
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
        assert!(
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
    ///
    /// Containers (`{` / `[`) push an iteration frame (`ObjectLoop` /
    /// `ArrayLoop`) and return — they are **not** processed recursively.
    /// Structural closers/separators (`}`/`]`/`,`) at a value position are
    /// orphans and produce `null`.
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
                stack.push(ParseFrame::ObjectLoop(0));
            }
            '[' => {
                self.emit_char('[');
                self.brackets_push(']');
                self.i += 1;
                stack.push(ParseFrame::ArrayLoop(0));
            }
            '"' => {
                if self.peek_is("\"\"\"") && self.text[self.i + 3..].contains("\"\"\"") {
                    self.parse_triple_string();
                    return;
                }
                self.parse_string(false);
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
                if ch.is_ascii_alphabetic() || ch == '_' {
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
        self.normalize_preamble();
        if self.i >= self.n {
            return Ok(String::new());
        }

        let mut stack = Stack::new();

        if self.is_implicit_object_sequence() {
            self.emit_char('[');
            self.brackets_push(']');
            stack.push(ParseFrame::ImplicitArrayLoop(0));
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
                ParseFrame::ObjectLoop(count) => self.object_loop(&mut stack, count),
                ParseFrame::ArrayLoop(count) => self.array_loop(&mut stack, count),
                ParseFrame::ImplicitArrayLoop(count) => self.implicit_array_loop(&mut stack, count),
            }
        }

        if let Some(err) = self.error.take() {
            return Err(err);
        }

        self.close_brackets();
        self.skip_suffix_junk();
        let out = std::mem::take(&mut self.out);
        if self.bracket_depth != 0 {
            return Err(JsonRepairError {
                message: "repaired output has unbalanced brackets".to_string(),
                position: None,
            });
        }
        Self::debug_validate_output(&out)?;
        Ok(out)
    }
}

impl Repairer {
    /// Debug-only validation: for shallow output, parse with serde_json.
    /// Bracket balance is tracked by `bracket_depth` in all build profiles.
    #[cfg(debug_assertions)]
    fn debug_validate_output(out: &str) -> Result<(), JsonRepairError> {
        Self::validate_serde_json(out);
        Ok(())
    }

    /// Non-debug stub (always succeeds).
    #[cfg(not(debug_assertions))]
    fn debug_validate_output(_out: &str) -> Result<(), JsonRepairError> {
        Ok(())
    }

    /// When `serde-validate` is enabled, parse the output with serde_json
    /// as a sanity check (triggers a panic in debug builds on failure).
    /// Only compiled in debug builds — release mode uses the no-op stub.
    #[cfg(all(feature = "serde-validate", debug_assertions))]
    fn validate_serde_json(out: &str) {
        const MAX_VALIDATION_DEPTH: usize = 100;
        let bracket_depth = out.chars().filter(|&c| c == '{' || c == '[').count();
        if bracket_depth <= MAX_VALIDATION_DEPTH {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(out) {
                debug_assert!(
                    false,
                    "repair result is not valid JSON (depth={}): {}\n---\n{}\n---",
                    bracket_depth, e, out
                );
            }
        }
    }

    /// Stub when serde-validate is disabled.
    #[cfg(not(feature = "serde-validate"))]
    fn validate_serde_json(_out: &str) {}
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
        assert!(self.len < STACK_CAPACITY);
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
