//! Optional repair trace events.
//!
//! This module is gated behind the `tracing` feature.  When enabled, the repairer
//! can record high-level events describing the transformations it applied to the
//! input.  The concrete [`TraceEvent`] values are intentionally simple so that
//! callers can inspect, log, or serialize them as needed.

#![cfg(feature = "tracing")]

use core::fmt::{self, Display};

/// A recorded event from the JSON repair process.
#[derive(Debug, Clone)]
pub enum TraceEvent {
    /// A comment was skipped.
    CommentSkipped {
        /// Style of the skipped comment.
        style: CommentStyle,
        /// Byte position in the original input where the comment started.
        start: usize,
    },

    /// A string was split into two values at the given position.
    StringSplit {
        /// Byte position where the split occurred.
        position: usize,
        /// Short static reason for the split.
        reason: &'static str,
    },

    /// A container was closed, either naturally or forcibly at EOF.
    ContainerClosed {
        /// The closing bracket character (`]` or `}`).
        bracket: char,
        /// `true` if the bracket was inserted because the input ended.
        forced_at_eof: bool,
    },

    /// A `null` value was inserted for an object key without a value.
    ImplicitNull {
        /// Byte position of the key in the original input.
        key_position: usize,
    },

    /// The repairer detected an implicit array (e.g. comma-separated bare values).
    ImplicitArrayDetected {
        /// Short static reason for the detection.
        reason: &'static str,
    },

    /// A value was normalized (e.g. a Python literal was converted to JSON).
    ValueNormalized {
        /// Static tag describing the kind of normalization.
        kind: &'static str,
    },

    /// A closing bracket did not match the currently open container.
    MismatchedBracket {
        /// The bracket expected by the currently open container, if any.
        expected: Option<char>,
        /// The bracket actually found in the input.
        found: char,
    },
}

/// Style of a comment encountered during repair.
#[derive(Debug, Clone)]
pub enum CommentStyle {
    /// `//` line comment.
    Line,
    /// `/* ... */` block comment.
    Block,
    /// `#` line comment.
    Hash,
    /// `--` line comment.
    DashDash,
}

/// A collection of repair trace events.
#[derive(Debug, Clone, Default)]
pub struct RepairTrace {
    events: Vec<TraceEvent>,
}

impl RepairTrace {
    /// Create a new, empty trace.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Access the recorded events.
    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    /// Append a new event to the trace.
    #[allow(dead_code)]
    pub(crate) fn push(&mut self, event: TraceEvent) {
        self.events.push(event);
    }
}

impl Display for RepairTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepairTrace({} events)", self.events.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_trace_is_empty() {
        let trace = RepairTrace::new();
        assert!(trace.events().is_empty());
    }

    #[test]
    fn push_records_event() {
        let mut trace = RepairTrace::new();
        trace.push(TraceEvent::ImplicitNull { key_position: 42 });
        assert_eq!(trace.events().len(), 1);
        assert!(matches!(
            trace.events()[0],
            TraceEvent::ImplicitNull { key_position: 42 }
        ));
    }

    #[test]
    fn display_shows_event_count() {
        let mut trace = RepairTrace::new();
        trace.push(TraceEvent::ValueNormalized { kind: "inf" });
        trace.push(TraceEvent::CommentSkipped {
            style: CommentStyle::Line,
            start: 0,
        });
        let output = trace.to_string();
        assert!(output.contains("RepairTrace(2 events)"));
    }
}
