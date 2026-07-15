//! Integration tests for the optional `tracing` feature.
//!
//! These tests exercise the public `repair_json_with_trace` API and verify
//! that each [`TraceEvent`] variant is emitted for the expected malformed
//! input patterns.  They are compiled only when the `tracing` Cargo feature
//! is enabled.

#![cfg(feature = "tracing")]

use json_repair_core::{CommentStyle, RepairConfig, TraceEvent, repair_json_with_trace};

fn trace_for(input: &str) -> Vec<TraceEvent> {
    let config = RepairConfig::default().with_tracing(true);
    let (_, trace) = repair_json_with_trace(input, &config).unwrap();
    trace.events().to_vec()
}

#[test]
#[cfg(feature = "tracing")]
fn trace_line_comment_skipped() {
    let events = trace_for("// comment\n42");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::CommentSkipped {
            style: CommentStyle::Line,
            ..
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_block_comment_skipped() {
    let events = trace_for("/* comment */42");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::CommentSkipped {
            style: CommentStyle::Block,
            ..
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_hash_comment_skipped() {
    let events = trace_for("# comment\n42");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::CommentSkipped {
            style: CommentStyle::Hash,
            ..
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_dash_dash_comment_skipped() {
    let events = trace_for("-- comment\n42");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::CommentSkipped {
            style: CommentStyle::DashDash,
            ..
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_string_split_embedded_quote() {
    let events = trace_for(r#"{"key": "value with "embedded" quotes"}"#);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TraceEvent::StringSplit { .. }))
    );
}

#[test]
#[cfg(feature = "tracing")]
fn trace_container_closed_forced_object() {
    let events = trace_for(r#"{"a":1"#);
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ContainerClosed {
            bracket: '}',
            forced_at_eof: true,
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_container_closed_forced_array() {
    let events = trace_for("[1,2");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ContainerClosed {
            bracket: ']',
            forced_at_eof: true,
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_implicit_null() {
    let events = trace_for(r#"{"key":}"#);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TraceEvent::ImplicitNull { .. }))
    );
}

#[test]
#[cfg(feature = "tracing")]
fn trace_implicit_array_detected() {
    let events = trace_for(r#"{"a":1},{"b":2}"#);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TraceEvent::ImplicitArrayDetected { .. }))
    );
}

#[test]
#[cfg(feature = "tracing")]
fn trace_value_normalized_infinity_to_null() {
    let events = trace_for("Infinity");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ValueNormalized {
            kind: "infinity_to_null",
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_value_normalized_nan_to_null() {
    let events = trace_for("NaN");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ValueNormalized {
            kind: "nan_to_null",
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_value_normalized_time_value_as_string() {
    let events = trace_for(r#"{"time": 10:30}"#);
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ValueNormalized {
            kind: "time_value_as_string",
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_mismatched_bracket_object_expected() {
    let events = trace_for(r#"{"a":1]"#);
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::MismatchedBracket {
            expected: Some('}'),
            found: ']',
            ..
        }
    )));
}

#[test]
#[cfg(feature = "tracing")]
fn trace_mismatched_bracket_array_expected() {
    let events = trace_for("[1,2}");
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::MismatchedBracket {
            expected: Some(']'),
            found: '}',
            ..
        }
    )));
}
