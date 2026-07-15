//! Streaming JSON repairer state machine and core types.
//!
//! This module implements the single-pass repair algorithm. The top-level
//! [`Repairer`] struct holds the input/parse state and produces
//! repaired JSON via [`Repairer::repair`](Repairer::repair). Submodules
//! handle specific repair concerns:
//!
//! - `comment` — inline and block comment removal
//! - `input_cursor` — read-only cursor over the input text
//! - `keys` — unquoted key parsing
//! - `literal` — unquoted `true`/`false`/`null` / `Infinity` / `NaN`
//! - `number` — number parsing and normalization
//! - `output_buffer` — output string builder with depth tracking
//! - `sequence` — top-level implicit-array and comma-separated list detection
//! - `string` — string parsing with embedded-quote detection
//! - `structure` — object/array frame management

mod comment;
mod input_cursor;
mod keys;
mod literal;
mod number;
mod output_buffer;
mod sequence;
mod string;
mod structure;

pub(crate) use input_cursor::InputCursor;
pub(crate) use output_buffer::OutputBuffer;

use crate::error::JsonRepairError;

#[cfg(feature = "tracing")]
use crate::trace::{RepairTrace, TraceEvent};

#[cfg(feature = "tracing")]
pub(crate) type Tracer<'a> = Option<&'a mut RepairTrace>;
#[cfg(not(feature = "tracing"))]
pub(crate) type Tracer<'a> = ();

/// Pre-allocate capacity for the parse-frame stack.
///
/// Sized for the default maximum parse depth (see
/// [`DEFAULT_MAX_PARSE_DEPTH`](crate::DEFAULT_MAX_PARSE_DEPTH)).  A larger
/// `max_depth` passed to [`repair`](Self::repair) simply causes the stack
/// to grow beyond this initial capacity — `Vec` handles it transparently.
const STACK_CAPACITY: usize = crate::DEFAULT_MAX_PARSE_DEPTH + 8;

/// Stack frame for the iterative (non-recursive) parse loop.
///
/// Each variant carries only the state needed to resume when the frame is
/// popped — no global flags on `Repairer` are consulted or mutated.
#[derive(Clone, Copy, Debug)]
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
/// Composed of two sub-structs plus a bracket stack:
/// - [`InputCursor`] — read-only cursor over the input text
/// - [`OutputBuffer`] — output string builder with depth tracking
/// - `Vec<char>` — expected closing brackets (`}` / `]`)
pub(crate) struct Repairer<'a> {
    pub(crate) input: InputCursor<'a>,
    pub(crate) output: OutputBuffer,
    pub(crate) brackets: Vec<char>,
    #[cfg(feature = "tracing")]
    pub(crate) trace: Option<RepairTrace>,
}

impl<'a> Repairer<'a> {
    /// Create a new repairer for `text`.
    pub(crate) fn new(text: &'a str) -> Self {
        Repairer {
            input: InputCursor::new(text),
            output: OutputBuffer::new(text.len()),
            brackets: Vec::new(),
            #[cfg(feature = "tracing")]
            trace: None,
        }
    }

    /// Attach an initial [`RepairTrace`] buffer to the repairer.
    #[cfg(feature = "tracing")]
    #[must_use]
    pub(crate) fn with_trace(mut self, trace: RepairTrace) -> Self {
        self.trace = Some(trace);
        self
    }

    /// Pop and emit all remaining open brackets (close truncated containers).
    fn close_brackets(&mut self, tracer: &mut Tracer) {
        let _ = tracer;
        while let Some(b) = self.brackets.pop() {
            self.output.trim_trailing_comma();
            self.output.emit_char(b);
            emit_trace!(
                tracer,
                TraceEvent::ContainerClosed {
                    bracket: b,
                    forced_at_eof: true,
                }
            );
        }
        self.output.set_depth0_pos();
    }

    /// Peek ahead: does the number span starting at `self.input.pos()` end
    /// with `:` immediately after?  If so, the digit sequence is part of a
    /// time value (e.g. `10:30`) and should be parsed as an unquoted string.
    fn peek_colon_after_number(&self) -> bool {
        let end = number::scan_number_span(&self.input);
        end < self.input.len() && self.input.bytes()[end] == b':'
    }

    /// Parse one value (primitive, string, number, object, array).
    ///
    /// Containers (`{` / `[`) push an iteration frame (`ObjectLoop` /
    /// `ArrayLoop`) and return — they are **not** processed recursively.
    /// Structural closers/separators (`}`/`]`/`,`) at a value position are
    /// orphans and produce `null`.
    fn run_value(&mut self, stack: &mut Vec<ParseFrame>, tracer: &mut Tracer) {
        self.input.skip_ws();
        if self.input.pos() >= self.input.len() {
            self.output.emit_str("null");
            return;
        }

        let ch = self.input.cur();
        match ch {
            // Object container: emit opener, push closer, resume inside object.
            Some('{') => {
                self.output.emit_char('{');
                self.brackets.push('}');
                self.input.advance(1);
                stack.push(ParseFrame::ObjectLoop(0));
            }
            // Array container: emit opener, push closer, resume inside array.
            Some('[') => {
                self.output.emit_char('[');
                self.brackets.push(']');
                self.input.advance(1);
                stack.push(ParseFrame::ArrayLoop(0));
            }
            // Double-quoted string (standard or Python-style triple-quoted).
            Some('"') => {
                if self.input.peek_is("\"\"\"")
                    && self.input.text()[self.input.pos() + 3..].contains("\"\"\"")
                {
                    string::parse_triple_string(&mut self.input, &mut self.output, tracer);
                    return;
                }
                string::parse_string(
                    &mut self.input,
                    &mut self.output,
                    &self.brackets,
                    false,
                    tracer,
                );
            }
            // Single-quoted string: rewrite as a double-quoted JSON string.
            Some('\'') => {
                string::parse_single_quoted_string(&mut self.input, &mut self.output, tracer);
            }
            // Unquoted literal: true / false / null / Infinity / NaN.
            Some(ch) if literal::is_literal_start(ch) => {
                literal::parse_literal(&mut self.input, &mut self.output, tracer);
            }
            // Leading minus: could be -Infinity, a comment, a number,
            // or a time-like key.
            Some('-') => {
                if literal::try_parse_signed_infinity(
                    &mut self.input,
                    &mut self.output,
                    '-',
                    tracer,
                ) {
                    // `-Infinity` as a value → JSON null.
                } else if self.input.peek_is("--") {
                    comment::skip_comment(&mut self.input, tracer);
                    stack.push(ParseFrame::Value);
                } else if self.peek_colon_after_number() {
                    emit_trace!(
                        tracer,
                        TraceEvent::ValueNormalized {
                            kind: "time_value_as_string",
                        }
                    );
                    keys::parse_unquoted_value(&mut self.input, &mut self.output, tracer);
                } else {
                    number::parse_number(&mut self.input, &mut self.output, tracer);
                }
            }
            // Leading plus: +Infinity is a JSON5-style literal value.
            Some('+') => {
                if !literal::try_parse_signed_infinity(
                    &mut self.input,
                    &mut self.output,
                    '+',
                    tracer,
                ) {
                    // Not +Infinity — skip the stray plus and try again.
                    self.input.advance(1);
                    stack.push(ParseFrame::Value);
                }
            }
            // Number (or time-like unquoted value if followed by ':').
            Some(ch) if number::is_number_start(ch) => {
                if self.peek_colon_after_number() {
                    emit_trace!(
                        tracer,
                        TraceEvent::ValueNormalized {
                            kind: "time_value_as_string",
                        }
                    );
                    keys::parse_unquoted_value(&mut self.input, &mut self.output, tracer);
                } else {
                    number::parse_number(&mut self.input, &mut self.output, tracer);
                }
            }
            // Comment start: consume it and re-enter as a fresh value position.
            Some('/') | Some('#') => {
                comment::skip_comment(&mut self.input, tracer);
                stack.push(ParseFrame::Value);
            }
            // Orphan structural token at value position: emit null and skip.
            Some('}' | ']' | ',') => {
                self.output.emit_str("null");
                self.input.advance(1);
            }
            // Bareword value/identifier, or an unrecognized byte to skip.
            Some(ch) => {
                if ch.is_ascii_alphabetic() || ch == '_' {
                    keys::parse_unquoted_value(&mut self.input, &mut self.output, tracer);
                } else {
                    self.input.advance(ch.len_utf8());
                    stack.push(ParseFrame::Value);
                }
            }
            // EOF at a value position is unreachable here: `pos < len` is
            // checked above, so `cur()` returns `Some`.  Defensive no-op to
            // satisfy match exhaustiveness.
            None => {}
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
    pub(crate) fn repair(&mut self, max_depth: usize) -> Result<String, JsonRepairError> {
        if self.input.is_empty() {
            return Ok(String::new());
        }

        #[cfg(feature = "tracing")]
        let mut trace = self.trace.take().unwrap_or_default();
        #[cfg(feature = "tracing")]
        let mut tracer: Tracer = Some(&mut trace);
        #[cfg(not(feature = "tracing"))]
        let mut tracer = ();

        let mut stack: Vec<ParseFrame> = Vec::with_capacity(STACK_CAPACITY);

        if sequence::is_implicit_object_sequence(&self.input, &mut tracer)
            || sequence::is_comma_separated_object_list(&self.input, &mut tracer)
            || sequence::is_comma_separated_value_list(&self.input, &mut tracer)
        {
            self.output.emit_char('[');
            self.brackets.push(']');
            stack.push(ParseFrame::ImplicitArrayLoop(0));
        } else {
            stack.push(ParseFrame::Value);
        }

        while let Some(frame) = stack.pop() {
            let current_depth = stack.len() + 1;
            if current_depth > max_depth {
                #[cfg(feature = "tracing")]
                {
                    self.trace = Some(trace);
                }
                return Err(JsonRepairError::new(
                    crate::error::JsonRepairErrorKind::DepthExceeded {
                        max: max_depth,
                        position: self.input.pos(),
                    },
                ));
            }

            match frame {
                // Fresh value position: dispatch on the next input character.
                ParseFrame::Value => self.run_value(&mut stack, &mut tracer),
                // Inside an object: expect a key/value pair or the closing brace.
                ParseFrame::ObjectLoop(count) => structure::object_loop(
                    &mut self.input,
                    &mut self.output,
                    &mut self.brackets,
                    &mut stack,
                    count,
                    &mut tracer,
                ),
                // Inside an array: expect a value or the closing bracket.
                ParseFrame::ArrayLoop(count) => structure::array_loop(
                    &mut self.input,
                    &mut self.output,
                    &mut self.brackets,
                    &mut stack,
                    count,
                    &mut tracer,
                ),
                // Top-level sequence of objects/values: wrap in an implicit array.
                ParseFrame::ImplicitArrayLoop(count) => structure::implicit_array_loop(
                    &mut self.input,
                    &mut self.output,
                    &mut stack,
                    count,
                    &mut tracer,
                ),
            }
        }

        self.close_brackets(&mut tracer);
        self.output.trim_suffix_junk();
        let out = self.output.take();
        if self.brackets.last().is_some() {
            #[cfg(feature = "tracing")]
            {
                self.trace = Some(trace);
            }
            return Err(JsonRepairError::new(
                crate::error::JsonRepairErrorKind::UnbalancedBrackets,
            ));
        }
        #[cfg(feature = "tracing")]
        {
            self.trace = Some(trace);
        }
        Self::debug_validate_output(&out)?;
        Ok(out)
    }
}

impl Repairer<'_> {
    /// Debug-only validation: for shallow output, parse with serde_json.
    /// Bracket balance is tracked by `brackets.len()` in all build profiles.
    #[cfg(debug_assertions)]
    fn debug_validate_output(out: &str) -> Result<(), JsonRepairError> {
        const MAX_VALIDATION_DEPTH: usize = 100;
        let bracket_depth = out.chars().filter(|&c| c == '{' || c == '[').count();
        if bracket_depth <= MAX_VALIDATION_DEPTH {
            #[cfg(feature = "serde-validate")]
            if let Err(e) = serde_json::from_str::<serde_json::Value>(out) {
                debug_assert!(
                    false,
                    "repair result is not valid JSON (depth={}): {}\n---\n{}\n---",
                    bracket_depth, e, out
                );
            }
        }
        Ok(())
    }

    /// Release-mode no-op (validation skipped for performance).
    #[cfg(not(debug_assertions))]
    fn debug_validate_output(_out: &str) -> Result<(), JsonRepairError> {
        Ok(())
    }
}
