//! Core library for repairing malformed JSON output from LLMs.
//!
//! The quickest way to repair JSON is [`repair_json`].
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

/// Errors produced during JSON repair.
pub mod error;

mod preprocess;
mod repairer;

pub use preprocess::{fix_colon_in_key, fix_mixed_quotes};

use error::JsonRepairError;
use repairer::Repairer;

/// Repair a malformed JSON string and return valid JSON.
///
/// This is the main entry point.  It returns `Ok(valid_json)` on success, or
/// `Err(JsonRepairError)` if repair produced text that is still invalid JSON.
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
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    if is_valid_json(text) {
        return Ok(text.to_string());
    }
    let text = fix_colon_in_key(text);
    let text = fix_mixed_quotes(text.as_ref());
    let mut repairer = Repairer::new(text.as_ref());
    repairer.repair()
}

/// Check if `text` is already valid JSON.
///
/// When `serde-validate` feature is enabled (default), uses `serde_json` for
/// accurate validation.  When disabled, accepts all non-empty text — the
/// repairer will handle validation during output emission.
#[cfg(feature = "serde-validate")]
fn is_valid_json(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok()
}

#[cfg(not(feature = "serde-validate"))]
fn is_valid_json(_text: &str) -> bool {
    false
}

/// Debug wrapper around `repair_json` with extra validation.
///
/// In debug builds, performs the same repair but adds:
/// - Valid JSON check on the result
/// - Idempotence check (second repair pass must match the first)
/// - All internal `debug_assert!` guards active
///
/// In release builds, identical to `repair_json`.
///
/// Use this during development and testing to catch regressions early.
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
#[cfg(debug_assertions)]
pub fn repair_json_debug(text: &str) -> Result<String, JsonRepairError> {
    let result = repair_json(text)?;
    if !result.is_empty() {
        debug_assert!(
            serde_json::from_str::<serde_json::Value>(&result).is_ok(),
            "repair_json_debug: result is not valid JSON: {result}"
        );
        let second = repair_json(&result)?;
        debug_assert_eq!(
            second, result,
            "repair_json_debug: repair is not idempotent"
        );
    }
    Ok(result)
}

#[cfg(not(debug_assertions))]
pub use repair_json as repair_json_debug;
