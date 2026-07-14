//! Error type for the JSON repair process.

use std::fmt;

/// Category of [`JsonRepairError`].
///
/// Returned by [`JsonRepairError::kind`] so callers can programmatically
/// distinguish failure modes without parsing the error message.
///
/// # Examples
///
/// ```
/// use json_repair_core::{repair_json, error::JsonRepairErrorKind};
///
/// // 600-deep nesting exceeds DEFAULT_MAX_PARSE_DEPTH (512)
/// let input = format!("{}{}", "[".repeat(600), "]".repeat(600));
/// let err = repair_json(&input).unwrap_err();
/// assert!(matches!(
///     err.kind(),
///     JsonRepairErrorKind::DepthExceeded { max: 512, .. }
/// ));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonRepairErrorKind {
    /// Maximum parse depth exceeded.
    ///
    /// The repairer refuses to descend beyond `max` levels of nesting
    /// to prevent stack overflow.  `position` is the byte offset in the
    /// input where the offending nesting began.
    DepthExceeded {
        /// The configured maximum parse depth.
        max: usize,
        /// Byte offset in the input where depth was exceeded.
        position: usize,
    },

    /// The repaired output has unbalanced brackets.
    ///
    /// Indicates the repair algorithm could not produce a structurally
    /// valid JSON document (e.g. orphan closers, mismatched brackets
    /// that could not be reconciled).
    UnbalancedBrackets,
}

/// Error type for JSON repair failures.
///
/// Returned by [`repair_json`](crate::repair_json) when the input is
/// catastrophically malformed and cannot be repaired into valid JSON.
/// Inspect the [`kind`](Self::kind) to handle specific failure modes.
///
/// # Examples
///
/// ```
/// use json_repair_core::repair_json;
///
/// // 600-deep nesting exceeds the default max parse depth (512).
/// let deep = format!("{}1", "[".repeat(600));
/// let err = repair_json(&deep).unwrap_err();
/// println!("{err}");
/// ```
#[derive(Debug, Clone)]
pub struct JsonRepairError {
    kind: JsonRepairErrorKind,
}

impl JsonRepairError {
    /// Creates a new error of the given kind.
    pub(crate) fn new(kind: JsonRepairErrorKind) -> Self {
        JsonRepairError { kind }
    }

    /// Returns the category of this error.
    ///
    /// Use this to programmatically match on failure modes rather than
    /// parsing the [`Display`](fmt::Display) output.
    pub fn kind(&self) -> &JsonRepairErrorKind {
        &self.kind
    }

    /// Returns the byte offset in the input where the error occurred, if known.
    ///
    /// Returns `None` for errors that are not tied to a specific input
    /// position (e.g. [`JsonRepairErrorKind::UnbalancedBrackets`]).
    pub fn position(&self) -> Option<usize> {
        match self.kind {
            JsonRepairErrorKind::DepthExceeded { position, .. } => Some(position),
            JsonRepairErrorKind::UnbalancedBrackets => None,
        }
    }
}

impl fmt::Display for JsonRepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            JsonRepairErrorKind::DepthExceeded { max, position } => write!(
                f,
                "JSON repair error at position {position}: max parse depth of {max} exceeded"
            ),
            JsonRepairErrorKind::UnbalancedBrackets => {
                write!(
                    f,
                    "JSON repair error: repaired output has unbalanced brackets"
                )
            }
        }
    }
}

impl std::error::Error for JsonRepairError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_exceeded_position_known() {
        let err = JsonRepairError::new(JsonRepairErrorKind::DepthExceeded {
            max: 512,
            position: 42,
        });
        assert_eq!(err.position(), Some(42));
        assert!(matches!(
            err.kind(),
            JsonRepairErrorKind::DepthExceeded {
                max: 512,
                position: 42
            }
        ));
        let s = format!("{err}");
        assert!(s.contains("position 42"));
        assert!(s.contains("512"));
    }

    #[test]
    fn unbalanced_brackets_position_none() {
        let err = JsonRepairError::new(JsonRepairErrorKind::UnbalancedBrackets);
        assert_eq!(err.position(), None);
        assert!(matches!(
            err.kind(),
            JsonRepairErrorKind::UnbalancedBrackets
        ));
        let s = format!("{err}");
        assert!(s.contains("unbalanced brackets"));
    }

    #[test]
    fn kind_equality_for_assertions() {
        let a = JsonRepairError::new(JsonRepairErrorKind::UnbalancedBrackets);
        let b = JsonRepairError::new(JsonRepairErrorKind::UnbalancedBrackets);
        assert_eq!(a.kind(), b.kind());
    }

    #[test]
    fn depth_exceeded_kind_inequality() {
        let a = JsonRepairError::new(JsonRepairErrorKind::DepthExceeded {
            max: 512,
            position: 1,
        });
        let b = JsonRepairError::new(JsonRepairErrorKind::DepthExceeded {
            max: 512,
            position: 2,
        });
        assert_ne!(a.kind(), b.kind(), "different positions should be unequal");
    }

    #[test]
    fn display_includes_position_when_present() {
        let err = JsonRepairError::new(JsonRepairErrorKind::DepthExceeded {
            max: 100,
            position: 5,
        });
        let s = format!("{err}");
        assert!(s.contains("at position 5"));
    }

    #[test]
    fn display_omits_position_when_absent() {
        let err = JsonRepairError::new(JsonRepairErrorKind::UnbalancedBrackets);
        let s = format!("{err}");
        assert!(!s.contains("at position"));
    }
}
