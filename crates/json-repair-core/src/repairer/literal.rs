use super::Repairer;

impl Repairer {
    /// Case-insensitive prefix match against a pattern, starting at `self.i`.
    /// Returns the length of the match (pat len) or 0 if no match.
    #[inline]
    fn match_lit(&self, pat: &str) -> bool {
        let plen = pat.len();
        if self.i + plen > self.n {
            return false;
        }
        pat.bytes()
            .enumerate()
            .all(|(j, p)| self.chars[self.i + j].to_ascii_lowercase() == p as char)
    }

    pub(super) fn parse_literal(&mut self) {
        if self.match_lit("true") {
            self.emit_str("true");
            self.i += 4;
        } else if self.match_lit("false") {
            self.emit_str("false");
            self.i += 5;
        } else if self.match_lit("null") || self.match_lit("none") {
            self.emit_str("null");
            self.i += 4;
        } else if self.match_lit("undefined") {
            self.emit_str("null");
            self.i += 9;
        } else if self.match_lit("nan") {
            self.emit_str("null");
            self.i += 3;
        } else if self.match_lit("infinity") {
            self.emit_str("null");
            self.i += 8;
        } else if self.match_lit("+infinity") || self.match_lit("-infinity") {
            self.emit_str("null");
            self.i += 9;
        } else {
            self.parse_unquoted_value();
            return;
        }
        self.just_emitted_value = true;
    }
}
