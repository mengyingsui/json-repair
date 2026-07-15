//! Core library for repairing malformed JSON output from LLMs.
//!
//! The quickest way to repair JSON is [`repair_json`].  For tunable
//! parameters (e.g. custom max parse depth), use
//! [`repair_json_with`](crate::repair_json_with) with a [`RepairConfig`].
//!
//! This crate provides a single-pass streaming repairer that handles common
//! JSON errors produced by large language models:
//! - Missing quotes around keys/values
//! - Mixed single/double quotes
//! - Unescaped (embedded) quotes inside string values
//! - Trailing commas
//! - Truncated JSON
//! - Unquoted literals (`true`, `false`, `null`)
//! - Single-line and block comments (`//`, `/* ... */`, `#`, `--`)
//! - Consecutive colons or space-separated keys
//!
//! # Features
//!
//! - **`serde-validate`** *(default)* — Uses `serde_json` to fast-path inputs
//!   that are already valid JSON, skipping the repairer entirely.  Disable to
//!   remove the `serde_json` dependency at the cost of always running the full
//!   repair pass.
#![deny(missing_docs)]

/// Emit a trace event when the `tracing` feature is enabled.
#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! emit_trace {
    ($tracer:expr, $event:expr) => {
        if let Some(t) = $tracer {
            t.push($event);
        }
    };
}

/// No-op trace emission when the `tracing` feature is disabled.
#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! emit_trace {
    ($tracer:expr, $event:expr) => {};
}

/// Configuration for JSON repair.
pub mod config;

/// Errors produced during JSON repair.
///
/// Re-exports the [`error::JsonRepairError`] and [`error::JsonRepairErrorKind`]
/// types for convenience.
pub mod error;

/// Pretty-printing for already-valid JSON strings.
pub mod format;

mod preprocess;
mod repairer;
mod util;

/// Optional repair trace events and collection type.
#[cfg(feature = "tracing")]
pub mod trace;

#[cfg(feature = "tracing")]
pub use trace::{CommentStyle, RepairTrace, TraceEvent};

#[cfg(feature = "tracing")]
pub use config::repair_json_with_trace;
pub use config::{DEFAULT_MAX_PARSE_DEPTH, RepairConfig, repair_json_with};
pub use error::{JsonRepairError, JsonRepairErrorKind};
pub use format::format_json;

/// Repair a malformed JSON string and return valid JSON.
///
/// This is the main entry point.  It returns `Ok(valid_json)` on success, or
/// `Err(JsonRepairError)` if repair produced text that is still invalid JSON.
/// Uses the default [`RepairConfig`] — see [`repair_json_with`] for tuning.
///
/// # Example
///
/// ```
/// use json_repair_core::repair_json;
///
/// let broken = r#"{"key": "value with "embedded" quotes"}"#;
/// let repaired = repair_json(broken).unwrap();
/// assert_eq!(repaired, r#"{"key":"value with \"embedded\" quotes"}"#);
/// ```
///
/// # Errors
///
/// Returns `JsonRepairError` if input is catastrophically malformed or the
/// repair algorithm cannot produce valid JSON.
pub fn repair_json(text: &str) -> Result<String, JsonRepairError> {
    repair_json_with(text, &RepairConfig::default())
}

/// Check if `text` is already valid JSON.
///
/// When `serde-validate` feature is enabled (default), uses `serde_json` for
/// accurate validation.  When disabled, accepts all non-empty text — the
/// repairer will handle validation during output emission.
#[cfg(feature = "serde-validate")]
pub(crate) fn is_valid_json(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok()
}

#[cfg(not(feature = "serde-validate"))]
pub(crate) fn is_valid_json(_text: &str) -> bool {
    false
}

/// Debug wrapper around `repair_json` with an idempotence check.
///
/// In debug builds, verifies that repairing the output again yields the
/// same string (i.e. the repair result is a fixed point).  Valid-JSON
/// validation is already performed inside [`repair_json`] in debug builds,
/// so this wrapper only adds the second-pass idempotence assertion.
/// In release builds, identical to [`repair_json`].
///
/// # Example
///
/// ```
/// use json_repair_core::repair_json_debug;
///
/// let broken = r#"{'key': 'value'}"#;
/// let repaired = repair_json_debug(broken).unwrap();
/// assert_eq!(repaired, r#"{"key":"value"}"#);
/// ```
pub fn repair_json_debug(text: &str) -> Result<String, JsonRepairError> {
    let result = repair_json(text);
    #[cfg(debug_assertions)]
    if let Ok(ref r) = result {
        if !r.is_empty() {
            match repair_json(r) {
                Ok(second) => debug_assert_eq!(
                    second.as_str(),
                    r.as_str(),
                    "repair_json_debug: repair is not idempotent"
                ),
                Err(e) => debug_assert!(false, "repair_json_debug: second repair pass failed: {e}"),
            }
        }
    }
    result
}
