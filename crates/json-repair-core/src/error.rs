use std::fmt;

#[derive(Debug, Clone)]
pub struct JsonRepairError {
    pub message: String,
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
