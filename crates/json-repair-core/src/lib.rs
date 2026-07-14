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
    let trimmed = text.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && is_valid_json(text) {
        return Ok(text.to_string());
    }
    let text = preprocess::preprocess_json(text);
    let (text, start_i) = preprocess::normalize_preamble(text.as_ref());
    let text = text.into_owned();
    let mut repairer = Repairer::new(&text);
    repairer.input.i = start_i;
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
/// In debug builds, adds valid-JSON and idempotence checks.
/// In release builds, identical to `repair_json`.
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
            debug_assert!(
                serde_json::from_str::<serde_json::Value>(r).is_ok(),
                "repair_json_debug: result is not valid JSON: {r}"
            );
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
