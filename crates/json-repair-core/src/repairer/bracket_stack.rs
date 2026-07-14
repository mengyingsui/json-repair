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
