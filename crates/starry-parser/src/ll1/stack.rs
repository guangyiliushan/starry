use crate::cfg::{NonTerminalId, TerminalId};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackSymbol {
    NonTerminal(NonTerminalId),
    Terminal(TerminalId),
    EndMarker,
    ProductionEnd {
        non_terminal: NonTerminalId,
        production_index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ParseStack {
    stack: VecDeque<StackSymbol>,
}

impl ParseStack {
    pub fn new() -> Self {
        ParseStack {
            stack: VecDeque::new(),
        }
    }

    pub fn initialize(&mut self, start_symbol: NonTerminalId) {
        self.stack.clear();
        self.stack.push_front(StackSymbol::EndMarker);
        self.stack.push_front(StackSymbol::NonTerminal(start_symbol));
    }

    pub fn push_front(&mut self, symbol: StackSymbol) {
        self.stack.push_front(symbol);
    }

    pub fn pop_front(&mut self) -> Option<StackSymbol> {
        self.stack.pop_front()
    }

    pub fn front(&self) -> Option<&StackSymbol> {
        self.stack.front()
    }

    pub fn front_cloned(&self) -> Option<StackSymbol> {
        self.stack.front().cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &StackSymbol> {
        self.stack.iter()
    }

    pub fn iter_rev(&self) -> impl Iterator<Item = &StackSymbol> {
        self.stack.iter().rev()
    }

    pub fn push_production(&mut self, rhs: &[crate::cfg::Symbol]) {
        for symbol in rhs.iter().rev() {
            match symbol {
                crate::cfg::Symbol::NonTerminal(nt_id) => {
                    self.push_front(StackSymbol::NonTerminal(*nt_id));
                }
                crate::cfg::Symbol::Terminal(term_id) => {
                    self.push_front(StackSymbol::Terminal(*term_id));
                }
                crate::cfg::Symbol::Epsilon => {}
            }
        }
    }
}

impl Default for ParseStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_operations() {
        let mut stack = ParseStack::new();
        
        assert!(stack.is_empty());
        
        stack.push_front(StackSymbol::EndMarker);
        assert_eq!(stack.len(), 1);
        
        let popped = stack.pop_front();
        assert!(matches!(popped, Some(StackSymbol::EndMarker)));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_initialize() {
        let mut stack = ParseStack::new();
        stack.initialize(NonTerminalId(0));
        
        assert_eq!(stack.len(), 2);
        
        let front = stack.front_cloned();
        assert!(matches!(front, Some(StackSymbol::NonTerminal(NonTerminalId(0)))));
    }
}
