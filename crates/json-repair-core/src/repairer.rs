//! Streaming JSON repairer state machine and core types.
//!
//! This module implements the single-pass repair algorithm. The top-level
//! [`Repairer`] struct holds the input/parse state and produces
//! repaired JSON via [`Repairer::repair`](Repairer::repair). Submodules
//! handle specific repair concerns:
//!
//! - `bracket_stack` — bracket depth tracking and matching
//! - `comment` — inline and block comment removal
//! - `input_cursor` — read-only cursor over the input text
//! - `junk` — trailing/comma junk handling
//! - `keys` — unquoted key parsing
//! - `literal` — unquoted `true`/`false`/`null` / `Infinity` / `NaN`
//! - `number` — number parsing and normalization
//! - `output_buffer` — output string builder with depth tracking
//! - `string` — string parsing with embedded-quote detection
//! - `structure` — object/array frame management

mod bracket_stack;
mod comment;
mod input_cursor;
mod junk;
mod keys;
mod literal;
mod number;
mod output_buffer;
mod string;
mod structure;

pub(crate) use bracket_stack::BracketStack;
pub(crate) use input_cursor::InputCursor;
pub(crate) use output_buffer::OutputBuffer;

use crate::error::JsonRepairError;

/// Maximum nesting depth for objects/arrays before the repairer gives up.
const MAX_PARSE_DEPTH: usize = 512;

/// Pre-allocate capacity for the parse-frame stack.
const STACK_CAPACITY: usize = MAX_PARSE_DEPTH + 8;

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
/// Composed of three sub-structs:
/// - [`InputCursor`] — read-only cursor over the input text
/// - [`OutputBuffer`] — output string builder with depth tracking
/// - [`BracketStack`] — bracket depth tracking and matching
pub(crate) struct Repairer<'a> {
    pub input: InputCursor<'a>,
    pub output: OutputBuffer,
    pub brackets: BracketStack,
}

impl<'a> Repairer<'a> {
    /// Create a new repairer for `text`.
    pub(crate) fn new(text: &'a str) -> Self {
        Repairer {
            input: InputCursor::new(text),
            output: OutputBuffer::new(text.len()),
            brackets: BracketStack::new(),
        }
    }

    /// Record the last depth-0 position when the bracket stack empties.
    #[inline]
    fn update_depth0(&mut self) {
        if self.brackets.is_empty() {
            self.output.set_depth0_pos();
        }
    }

    /// Pop and emit all remaining open brackets (close truncated containers).
    fn close_brackets(&mut self) {
        while let Some(b) = self.brackets.pop() {
            self.output.trim_trailing_comma();
            self.output.emit_char(b);
        }
        self.output.set_depth0_pos();
    }

    /// Peek ahead: does the number span starting at `self.input.i` end
    /// with `:` immediately after?  If so, the digit sequence is part of a
    /// time value (e.g. `10:30`) and should be parsed as an unquoted string.
    fn peek_colon_after_number(&self) -> bool {
        let end = number::scan_number_span(&self.input);
        end < self.input.text.len() && self.input.text.as_bytes()[end] == b':'
    }

    /// Parse one value (primitive, string, number, object, array).
    ///
    /// Containers (`{` / `[`) push an iteration frame (`ObjectLoop` /
    /// `ArrayLoop`) and return — they are **not** processed recursively.
    /// Structural closers/separators (`}`/`]`/`,`) at a value position are
    /// orphans and produce `null`.
    fn run_value(&mut self, stack: &mut Stack) {
        self.input.skip_ws();
        if self.input.i >= self.input.text.len() {
            self.output.emit_str("null");
            return;
        }

        let ch = self.input.cur();
        match ch {
            '{' => {
                self.output.emit_char('{');
                self.brackets.push('}');
                self.input.i += 1;
                stack.push(ParseFrame::ObjectLoop(0));
            }
            '[' => {
                self.output.emit_char('[');
                self.brackets.push(']');
                self.input.i += 1;
                stack.push(ParseFrame::ArrayLoop(0));
            }
            '"' => {
                if self.input.peek_is("\"\"\"")
                    && self.input.text[self.input.i + 3..].contains("\"\"\"")
                {
                    string::parse_triple_string(&mut self.input, &mut self.output, &self.brackets);
                    return;
                }
                string::parse_string(&mut self.input, &mut self.output, &self.brackets, false);
            }
            '\'' => string::parse_single_quoted_string(
                &mut self.input,
                &mut self.output,
                &self.brackets,
            ),
            't' | 'f' | 'n' | 'T' | 'F' | 'N' | 'i' | 'I' | 'u' | 'U' => {
                literal::parse_literal(&mut self.input, &mut self.output)
            }
            '-' => {
                if self.input.peek_is("--") {
                    comment::skip_comment(&mut self.input);
                    stack.push(ParseFrame::Value);
                } else if self.peek_colon_after_number() {
                    keys::parse_unquoted_value(&mut self.input, &mut self.output);
                } else {
                    number::parse_number(&mut self.input, &mut self.output);
                }
            }
            '.' | '0'..='9' => {
                if self.peek_colon_after_number() {
                    keys::parse_unquoted_value(&mut self.input, &mut self.output);
                } else {
                    number::parse_number(&mut self.input, &mut self.output);
                }
            }
            '/' | '#' => {
                comment::skip_comment(&mut self.input);
                stack.push(ParseFrame::Value);
            }
            '}' | ']' | ',' => {
                self.output.emit_str("null");
                self.input.i += 1;
            }
            _ => {
                if ch.is_ascii_alphabetic() || ch == '_' {
                    keys::parse_unquoted_value(&mut self.input, &mut self.output);
                } else {
                    self.input.i += ch.len_utf8();
                    stack.push(ParseFrame::Value);
                }
            }
        }
    }

    /// Run the full single-pass repair on the input text.
    ///
    /// `normalize_preamble` must have been called on the input before
    /// constructing the Repairer.
    ///
    /// Returns `Ok(valid_json)` on success, or `Err` if the input is
    /// catastrophically malformed (e.g. parse depth exceeded).  In debug
    /// builds, the result is additionally validated as parseable JSON.
    pub(crate) fn repair(&mut self) -> Result<String, JsonRepairError> {
        if self.input.is_empty() {
            return Ok(String::new());
        }

        let mut stack = Stack::new();

        if junk::is_implicit_object_sequence(&self.input)
            || junk::is_comma_separated_object_list(&self.input)
            || junk::is_comma_separated_value_list(&self.input)
        {
            self.output.emit_char('[');
            self.brackets.push(']');
            stack.push(ParseFrame::ImplicitArrayLoop(0));
        } else {
            stack.push(ParseFrame::Value);
        }

        while let Some(frame) = stack.pop() {
            let current_depth = stack.len() + 1;
            if current_depth > MAX_PARSE_DEPTH {
                return Err(JsonRepairError {
                    message: format!(
                        "max parse depth of {MAX_PARSE_DEPTH} exceeded at position {}",
                        self.input.i
                    ),
                    position: Some(self.input.i),
                });
            }

            match frame {
                ParseFrame::Value => self.run_value(&mut stack),
                ParseFrame::ObjectLoop(count) => self.object_loop(&mut stack, count),
                ParseFrame::ArrayLoop(count) => self.array_loop(&mut stack, count),
                ParseFrame::ImplicitArrayLoop(count) => self.implicit_array_loop(&mut stack, count),
            }
        }

        self.close_brackets();
        self.output.trim_suffix_junk();
        let out = self.output.take();
        if self.brackets.depth() != 0 {
            return Err(JsonRepairError {
                message: "repaired output has unbalanced brackets".to_string(),
                position: None,
            });
        }
        Self::debug_validate_output(&out)?;
        Ok(out)
    }
}

impl Repairer<'_> {
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

/// Stack of parse frames.
pub(super) struct Stack {
    frames: Vec<ParseFrame>,
}

impl Stack {
    fn new() -> Self {
        Stack {
            frames: Vec::with_capacity(STACK_CAPACITY),
        }
    }

    /// Pushes a parse frame onto the stack.
    #[inline]
    pub(super) fn push(&mut self, frame: ParseFrame) {
        self.frames.push(frame);
    }

    /// Pops the top parse frame from the stack.
    ///
    /// Returns `None` when the stack is empty.
    #[inline]
    pub(super) fn pop(&mut self) -> Option<ParseFrame> {
        self.frames.pop()
    }

    /// Returns the number of frames currently on the stack.
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.frames.len()
    }
}
