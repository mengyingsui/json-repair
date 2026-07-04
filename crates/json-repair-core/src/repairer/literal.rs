use super::Repairer;

impl Repairer {
    pub(super) fn parse_literal(&mut self) {
        let end = (self.i + 9).min(self.n);
        let lower: String = self.chars[self.i..end]
            .iter()
            .collect::<String>()
            .to_lowercase();
        if lower.starts_with("true") {
            self.emit_str("true");
            self.i += 4;
        } else if lower.starts_with("false") {
            self.emit_str("false");
            self.i += 5;
        } else if lower.starts_with("null") || lower.starts_with("none") {
            self.emit_str("null");
            self.i += 4;
        } else if lower.starts_with("undefined") {
            self.emit_str("null");
            self.i += 9;
        } else if lower.starts_with("nan") {
            self.emit_str("null");
            self.i += 3;
        } else if lower.starts_with("infinity") || lower.starts_with("+infinity") {
            self.emit_str("null");
            self.i += 8;
        } else if lower.starts_with("-infinity") {
            self.emit_str("null");
            self.i += 9;
        } else {
            self.parse_unquoted_value();
            return;
        }
        self.just_emitted_value = true;
    }
}
