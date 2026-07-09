//! Error type for the JSON repair process.

use std::fmt;

/// Error type for JSON repair failures.
///
/// Returned by [`repair_json`](crate::repair_json) when the input is
/// catastrophically malformed and cannot be repaired into valid JSON.
#[derive(Debug, Clone)]
pub struct JsonRepairError {
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Character offset in the input where the error occurred, if known.
    pub position: Option<usize>,
}

impl fmt::Display for JsonRepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(pos) = self.position {
            write!(f, "JSON repair error at position {}: {}", pos, self.message)
        } else {
            write!(f, "JSON repair error: {}", self.message)
        }
    }
}

impl std::error::Error for JsonRepairError {}
