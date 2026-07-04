use super::Repairer;

impl Repairer {
    pub(super) fn skip_comment(&mut self) {
        if self.peek_is("//") {
            while self.i < self.n && self.chars[self.i] != '\n' {
                self.i += 1;
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else if self.peek_is("/*") {
            self.i += 2;
            while self.i + 1 < self.n {
                if self.chars[self.i] == '*' && self.chars[self.i + 1] == '/' {
                    self.i += 2;
                    return;
                }
                self.i += 1;
            }
        } else if self.chars[self.i] == '#' || self.peek_is("--") {
            while self.i < self.n && self.chars[self.i] != '\n' {
                self.i += 1;
            }
            if self.i < self.n {
                self.i += 1;
            }
        } else {
            self.i += 1;
        }
    }
}
