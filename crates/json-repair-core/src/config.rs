//! Configuration for JSON repair.
//!
//! [`RepairConfig`] controls tunable parameters of the repair algorithm.
//! Use [`RepairConfig::default`] for sensible defaults, or build a custom
//! configuration with the builder methods:
//!
//! ```
//! use json_repair_core::{RepairConfig, repair_json_with};
//!
//! let config = RepairConfig::default().with_max_depth(256);
//! let repaired = repair_json_with("[[[1]]]", &config).unwrap();
//! assert_eq!(repaired, "[[[1]]]");
//! ```

use crate::error::JsonRepairError;
use crate::preprocess;
use crate::repairer::Repairer;
#[cfg(feature = "tracing")]
use crate::trace::RepairTrace;

/// Default maximum parse depth.
///
/// Matches the historical hard-coded value.  Exceeding this returns
/// [`JsonRepairError`] with kind
/// [`DepthExceeded`](crate::error::JsonRepairErrorKind::DepthExceeded).
pub const DEFAULT_MAX_PARSE_DEPTH: usize = 512;

/// Tunable parameters for [`repair_json_with`](crate::repair_json_with).
///
/// All fields have defaults matching the historical behavior of
/// [`repair_json`](crate::repair_json).  Use the builder methods
/// (`with_*`) to override individual values.
#[derive(Debug, Clone, Copy)]
pub struct RepairConfig {
    max_depth: usize,
    #[allow(dead_code)] // Task 2/3 will read this field when the `tracing` feature is enabled.
    tracing: bool,
}

impl Default for RepairConfig {
    fn default() -> Self {
        RepairConfig {
            max_depth: DEFAULT_MAX_PARSE_DEPTH,
            tracing: false,
        }
    }
}

impl RepairConfig {
    /// Creates a new config with default values.
    ///
    /// Equivalent to [`Default::default`], but more discoverable.
    pub const fn new() -> Self {
        RepairConfig {
            max_depth: DEFAULT_MAX_PARSE_DEPTH,
            tracing: false,
        }
    }

    /// Sets the maximum nesting depth for objects/arrays.
    ///
    /// The repairer refuses to descend beyond `max_depth` levels of nesting
    /// to prevent stack overflow.  Inputs exceeding this limit return
    /// [`JsonRepairError`] with kind
    /// [`DepthExceeded`](crate::error::JsonRepairErrorKind::DepthExceeded).
    ///
    /// # Example
    ///
    /// ```
    /// use json_repair_core::{RepairConfig, repair_json_with};
    ///
    /// // Allow only 4 levels of nesting
    /// let config = RepairConfig::default().with_max_depth(4);
    /// let repaired = repair_json_with("[[[[1]]]]", &config).unwrap();
    /// assert_eq!(repaired, "[[[[1]]]]");
    /// ```
    #[must_use]
    pub const fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Returns the configured maximum parse depth.
    pub const fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Enables or disables repair tracing.
    ///
    /// When tracing is enabled, the repairer records a trace of repair
    /// events that can be inspected after repair.  This is disabled by
    /// default.
    ///
    /// This method is available only when the `tracing` Cargo feature is
    /// enabled.
    ///
    /// # Example
    ///
    /// ```
    /// use json_repair_core::RepairConfig;
    ///
    /// let config = RepairConfig::default().with_tracing(true);
    /// assert!(config.tracing());
    /// ```
    #[must_use]
    #[cfg(feature = "tracing")]
    pub const fn with_tracing(mut self, tracing: bool) -> Self {
        self.tracing = tracing;
        self
    }

    /// Returns whether repair tracing is enabled.
    ///
    /// This method is available only when the `tracing` Cargo feature is
    /// enabled.
    #[cfg(feature = "tracing")]
    pub const fn tracing(&self) -> bool {
        self.tracing
    }
}

/// Repair a malformed JSON string with a custom [`RepairConfig`].
///
/// Like [`repair_json`](crate::repair_json), but allows overriding tunable
/// parameters such as the maximum parse depth.
///
/// # Example
///
/// ```
/// use json_repair_core::{RepairConfig, repair_json_with};
///
/// let config = RepairConfig::default().with_max_depth(1024);
/// // Single-quoted strings are invalid JSON, so this exercises the repairer.
/// let repaired = repair_json_with(r#"{'key': 'value'}"#, &config).unwrap();
/// assert_eq!(repaired, r#"{"key":"value"}"#);
/// ```
///
/// # Errors
///
/// Returns [`JsonRepairError`] if the input is catastrophically malformed
/// or the configured `max_depth` is exceeded.
pub fn repair_json_with(text: &str, config: &RepairConfig) -> Result<String, JsonRepairError> {
    #[cfg(feature = "tracing")]
    {
        let (json, _) = repair_json_with_trace_impl(text, config, None)?;
        Ok(json)
    }
    #[cfg(not(feature = "tracing"))]
    {
        if text.trim().is_empty() {
            return Ok(String::new());
        }
        let trimmed = text.trim_start();
        if (trimmed.starts_with('{') || trimmed.starts_with('[')) && crate::is_valid_json(text) {
            return Ok(text.to_string());
        }
        let text = preprocess::preprocess_json(text);
        let (text, start_i) = preprocess::normalize_preamble(text.as_ref());
        let text = text.as_ref();
        let mut repairer = Repairer::new(text);
        repairer.input.set_pos(start_i);
        repairer.repair(config.max_depth())
    }
}

#[cfg(feature = "tracing")]
fn repair_json_with_trace_impl(
    text: &str,
    config: &RepairConfig,
    trace: Option<RepairTrace>,
) -> Result<(String, RepairTrace), JsonRepairError> {
    if text.trim().is_empty() {
        return Ok((String::new(), RepairTrace::new()));
    }
    let trimmed = text.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && crate::is_valid_json(text) {
        return Ok((text.to_string(), RepairTrace::new()));
    }
    let text = preprocess::preprocess_json(text);
    let (text, start_i) = preprocess::normalize_preamble(text.as_ref());
    let text = text.as_ref();
    let mut repairer = Repairer::new(text);
    repairer.input.set_pos(start_i);
    if let Some(t) = trace {
        repairer = repairer.with_trace(t);
    }
    let json = repairer.repair(config.max_depth())?;
    let trace = repairer.trace.take().unwrap_or_default();
    Ok((json, trace))
}

/// Repair malformed JSON and return the repair trace.
///
/// Like [`repair_json_with`], but returns both the repaired JSON and a
/// [`RepairTrace`] containing the events emitted during repair.
///
/// # Errors
///
/// Returns `JsonRepairError` with kind [`TracingDisabled`](crate::error::JsonRepairErrorKind::TracingDisabled)
/// if `config.tracing()` is `false`.
#[cfg(feature = "tracing")]
pub fn repair_json_with_trace(
    text: &str,
    config: &RepairConfig,
) -> Result<(String, RepairTrace), JsonRepairError> {
    if !config.tracing() {
        return Err(JsonRepairError::new(
            crate::error::JsonRepairErrorKind::TracingDisabled,
        ));
    }
    repair_json_with_trace_impl(text, config, Some(RepairTrace::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::JsonRepairErrorKind;
    use serde::Deserialize;

    #[test]
    fn default_config_matches_constant() {
        let config = RepairConfig::default();
        assert_eq!(config.max_depth(), DEFAULT_MAX_PARSE_DEPTH);
        assert_eq!(config.max_depth(), 512);
    }

    #[test]
    fn new_equals_default() {
        assert_eq!(
            RepairConfig::new().max_depth(),
            RepairConfig::default().max_depth()
        );
    }

    #[test]
    fn with_max_depth_overrides() {
        let config = RepairConfig::default().with_max_depth(256);
        assert_eq!(config.max_depth(), 256);
    }

    #[test]
    fn with_max_depth_is_must_use() {
        // Builder returns a new value; original is unchanged
        let original = RepairConfig::default();
        let modified = original.with_max_depth(8);
        assert_eq!(original.max_depth(), 512, "original should be unchanged");
        assert_eq!(modified.max_depth(), 8);
    }

    #[test]
    fn with_max_depth_chained() {
        let config = RepairConfig::default()
            .with_max_depth(100)
            .with_max_depth(200);
        assert_eq!(config.max_depth(), 200);
    }

    #[test]
    fn repair_json_with_default_matches_repair_json() {
        let input = r#"{"key": "value"}"#;
        let a = crate::repair_json(input).unwrap();
        let b = repair_json_with(input, &RepairConfig::default()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn repair_json_with_low_max_depth_rejects_deep_nesting() {
        // Truncated nesting — invalid JSON, bypasses serde-validate fast-path.
        // 10 unclosed `[` push 10 frames onto the parse stack.
        let input = "[".repeat(10) + "1";
        let config = RepairConfig::default().with_max_depth(4);
        let err = repair_json_with(&input, &config).unwrap_err();
        assert!(matches!(
            err.kind(),
            JsonRepairErrorKind::DepthExceeded { max: 4, .. }
        ));
    }

    #[test]
    fn repair_json_with_high_max_depth_accepts_deep_nesting() {
        let input = format!("{}{}", "[".repeat(100), "]".repeat(100));
        let config = RepairConfig::default().with_max_depth(200);
        let repaired = repair_json_with(&input, &config).unwrap();
        // serde_json has its own default recursion limit (128); disable it
        // via Deserializer so we can validate 100-deep output.
        let mut de = serde_json::Deserializer::from_str(&repaired);
        de.disable_recursion_limit();
        let parsed: Result<serde_json::Value, _> = Deserialize::deserialize(&mut de);
        assert!(parsed.is_ok(), "deeply nested output must be valid JSON");
    }

    #[test]
    fn repair_json_with_empty_input() {
        let config = RepairConfig::default();
        assert_eq!(repair_json_with("", &config).unwrap(), "");
        assert_eq!(repair_json_with("   ", &config).unwrap(), "");
    }

    #[test]
    fn repair_json_with_broken_input() {
        let input = r#"{'key': 'value'}"#;
        let config = RepairConfig::default();
        let repaired = repair_json_with(input, &config).unwrap();
        assert_eq!(repaired, r#"{"key":"value"}"#);
    }

    #[test]
    fn repair_json_with_one_max_depth_repairs_shallow_value() {
        // A bare value pushes one Value frame (depth 1); max_depth=1 allows it.
        // Use single-quoted string to bypass serde-validate fast-path.
        let config = RepairConfig::default().with_max_depth(1);
        let repaired = repair_json_with("'hello'", &config).unwrap();
        assert_eq!(repaired, "\"hello\"");
    }

    #[test]
    fn repair_json_with_zero_max_depth_rejects_any_frame() {
        // max_depth=0 rejects even the initial Value frame (depth 1 > 0).
        // Use single-quoted string to bypass serde-validate fast-path.
        let config = RepairConfig::default().with_max_depth(0);
        let err = repair_json_with("'hello'", &config).unwrap_err();
        assert!(matches!(
            err.kind(),
            JsonRepairErrorKind::DepthExceeded { max: 0, .. }
        ));
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn default_tracing_is_false() {
        let config = RepairConfig::default();
        assert!(!config.tracing());
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn with_tracing_enables_tracing() {
        let config = RepairConfig::default().with_tracing(true);
        assert!(config.tracing());
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn with_tracing_can_disable_tracing() {
        let config = RepairConfig::default()
            .with_tracing(true)
            .with_tracing(false);
        assert!(!config.tracing());
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn repair_json_with_trace_requires_tracing_enabled() {
        let config = RepairConfig::default();
        let err = repair_json_with_trace("{'key': 'value'}", &config).unwrap_err();
        assert!(matches!(err.kind(), JsonRepairErrorKind::TracingDisabled));
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn repair_json_with_trace_returns_trace() {
        let config = RepairConfig::default().with_tracing(true);
        let (json, trace) = repair_json_with_trace("Infinity", &config).unwrap();
        assert_eq!(json, "null");
        assert!(!trace.events().is_empty());
        assert!(trace.events().iter().any(|event| matches!(
            event,
            crate::trace::TraceEvent::ValueNormalized {
                kind: "infinity_to_null",
            }
        )));
    }
}
