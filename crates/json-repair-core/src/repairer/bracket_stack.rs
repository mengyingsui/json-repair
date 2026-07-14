//! Bracket depth tracking and matching.
//!
//! [`BracketStack`] maintains a stack of expected closing brackets
//! (`}` / `]`) and a running net depth counter.  This lets the repairer
//! match openers with closers, detect mismatched brackets, and close
//! truncated containers at EOF.

/// Stack of expected closing brackets with a live depth counter.
///
/// `brackets` holds the expected closing brackets in LIFO order.
/// `bracket_depth` is the net depth of emitted output (`+1` per push,
/// `-1` per pop) — it replaces a post-hoc output scan for balance
/// checking.
pub(crate) struct BracketStack {
    brackets: Vec<char>,
    bracket_depth: i32,
}

impl BracketStack {
    /// Creates an empty bracket stack.
    pub fn new() -> Self {
        BracketStack {
            brackets: Vec::new(),
            bracket_depth: 0,
        }
    }

    /// Pushes a closing bracket and increments the net depth counter.
    pub fn push(&mut self, c: char) {
        self.brackets.push(c);
        self.bracket_depth += 1;
    }

    /// Pops the top closing bracket and decrements the net depth.
    ///
    /// Returns `None` when the stack is empty (orphan closer).
    pub fn pop(&mut self) -> Option<char> {
        let result = self.brackets.pop();
        if result.is_some() {
            self.bracket_depth -= 1;
        }
        result
    }

    /// Returns the top closing bracket without removing it, or `None` if
    /// empty.
    pub fn last(&self) -> Option<char> {
        self.brackets.last().copied()
    }

    /// Returns `true` if the stack is empty (depth is zero).
    pub fn is_empty(&self) -> bool {
        self.brackets.is_empty()
    }

    /// Returns the net bracket depth of the emitted output.
    ///
    /// Must be zero at the end of a successful repair.
    pub fn depth(&self) -> i32 {
        self.bracket_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stack_is_empty() {
        let s = BracketStack::new();
        assert!(s.is_empty());
        assert_eq!(s.depth(), 0);
        assert_eq!(s.last(), None);
    }

    #[test]
    fn push_pop_roundtrip() {
        let mut s = BracketStack::new();
        s.push('}');
        assert!(!s.is_empty());
        assert_eq!(s.depth(), 1);
        assert_eq!(s.last(), Some('}'));
        assert_eq!(s.pop(), Some('}'));
        assert!(s.is_empty());
        assert_eq!(s.depth(), 0);
    }

    #[test]
    fn pop_empty_returns_none() {
        let mut s = BracketStack::new();
        assert_eq!(s.pop(), None);
        assert_eq!(s.depth(), 0);
    }

    #[test]
    fn nested_push_pop_preserves_order() {
        let mut s = BracketStack::new();
        s.push('}');
        s.push(']');
        assert_eq!(s.last(), Some(']'));
        assert_eq!(s.depth(), 2);
        assert_eq!(s.pop(), Some(']'));
        assert_eq!(s.last(), Some('}'));
        assert_eq!(s.depth(), 1);
        assert_eq!(s.pop(), Some('}'));
        assert_eq!(s.depth(), 0);
    }

    #[test]
    fn depth_tracks_push_pop() {
        let mut s = BracketStack::new();
        for _ in 0..5 {
            s.push('}');
        }
        assert_eq!(s.depth(), 5);
        for expected in (0..5).rev() {
            assert_eq!(s.pop(), Some('}'));
            assert_eq!(s.depth(), expected);
        }
    }
}
