use crate::cfg::{NonTerminalId, TerminalId};
use crate::lr::{ItemSetId, ProductionId};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackSymbol {
    State(ItemSetId),
    Terminal(TerminalId),
    NonTerminal(NonTerminalId),
}

impl StackSymbol {
    pub fn state(id: usize) -> Self {
        StackSymbol::State(ItemSetId(id))
    }

    pub fn terminal(id: TerminalId) -> Self {
        StackSymbol::Terminal(id)
    }

    pub fn non_terminal(id: NonTerminalId) -> Self {
        StackSymbol::NonTerminal(id)
    }

    pub fn is_state(&self) -> bool {
        matches!(self, StackSymbol::State(_))
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, StackSymbol::Terminal(_))
    }

    pub fn is_non_terminal(&self) -> bool {
        matches!(self, StackSymbol::NonTerminal(_))
    }

    pub fn state_id(&self) -> Option<ItemSetId> {
        match self {
            StackSymbol::State(id) => Some(*id),
            _ => None,
        }
    }

    pub fn terminal_id(&self) -> Option<TerminalId> {
        match self {
            StackSymbol::Terminal(id) => Some(*id),
            _ => None,
        }
    }

    pub fn non_terminal_id(&self) -> Option<NonTerminalId> {
        match self {
            StackSymbol::NonTerminal(id) => Some(*id),
            _ => None,
        }
    }
}

impl fmt::Display for StackSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackSymbol::State(id) => write!(f, "s{}", id.0),
            StackSymbol::Terminal(id) => write!(f, "t{}", id.0),
            StackSymbol::NonTerminal(id) => write!(f, "N{}", id.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LRStack {
    stack: Vec<StackSymbol>,
}

impl LRStack {
    pub fn new(initial_state: ItemSetId) -> Self {
        LRStack {
            stack: vec![StackSymbol::State(initial_state)],
        }
    }

    pub fn push(&mut self, symbol: StackSymbol) {
        self.stack.push(symbol);
    }

    pub fn pop(&mut self) -> Option<StackSymbol> {
        self.stack.pop()
    }

    pub fn pop_n(&mut self, n: usize) -> Vec<StackSymbol> {
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(sym) = self.stack.pop() {
                result.push(sym);
            }
        }
        result
    }

    pub fn top(&self) -> Option<&StackSymbol> {
        self.stack.last()
    }

    pub fn top_state(&self) -> Option<ItemSetId> {
        self.stack.iter().rev().find_map(|sym| sym.state_id())
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &StackSymbol> {
        self.stack.iter()
    }

    pub fn symbols(&self) -> &[StackSymbol] {
        &self.stack
    }

    pub fn push_state(&mut self, state: ItemSetId) {
        self.stack.push(StackSymbol::State(state));
    }

    pub fn push_terminal(&mut self, terminal: TerminalId) {
        self.stack.push(StackSymbol::Terminal(terminal));
    }

    pub fn push_non_terminal(&mut self, non_terminal: NonTerminalId) {
        self.stack.push(StackSymbol::NonTerminal(non_terminal));
    }
}

impl fmt::Display for LRStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, sym) in self.stack.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", sym)?;
        }
        write!(f, "]")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackAction {
    Shift(ItemSetId),
    Reduce(ProductionId, usize, NonTerminalId),
    Accept,
    Error,
}

impl StackAction {
    pub fn shift(state: usize) -> Self {
        StackAction::Shift(ItemSetId(state))
    }

    pub fn reduce(production: usize, rhs_len: usize, lhs: NonTerminalId) -> Self {
        StackAction::Reduce(ProductionId(production), rhs_len, lhs)
    }

    pub fn accept() -> Self {
        StackAction::Accept
    }

    pub fn error() -> Self {
        StackAction::Error
    }

    pub fn is_shift(&self) -> bool {
        matches!(self, StackAction::Shift(_))
    }

    pub fn is_reduce(&self) -> bool {
        matches!(self, StackAction::Reduce(_, _, _))
    }

    pub fn is_accept(&self) -> bool {
        matches!(self, StackAction::Accept)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, StackAction::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_symbol_creation() {
        let state = StackSymbol::state(0);
        assert!(state.is_state());
        assert_eq!(state.state_id(), Some(ItemSetId(0)));

        let term = StackSymbol::terminal(TerminalId(1));
        assert!(term.is_terminal());

        let nt = StackSymbol::non_terminal(NonTerminalId(2));
        assert!(nt.is_non_terminal());
    }

    #[test]
    fn test_lr_stack_operations() {
        let mut stack = LRStack::new(ItemSetId(0));

        assert_eq!(stack.len(), 1);
        assert_eq!(stack.top_state(), Some(ItemSetId(0)));

        stack.push_state(ItemSetId(1));
        assert_eq!(stack.len(), 2);

        stack.push_terminal(TerminalId(0));
        assert_eq!(stack.len(), 3);

        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_stack_display() {
        let mut stack = LRStack::new(ItemSetId(0));
        stack.push_terminal(TerminalId(1));
        stack.push_state(ItemSetId(2));

        let display = format!("{}", stack);
        assert!(display.contains("s0"));
        assert!(display.contains("t1"));
        assert!(display.contains("s2"));
    }
}
