use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NonTerminalId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Production {
    Epsilon,
    Terminal(char),
    TerminalNonTerminal(char, NonTerminalId),
}

#[derive(Debug, Clone)]
pub struct RegularGrammar {
    pub non_terminals: Vec<String>,
    pub start_symbol: NonTerminalId,
    pub productions: HashMap<NonTerminalId, Vec<Production>>,
}

impl RegularGrammar {
    pub fn new() -> Self {
        RegularGrammar {
            non_terminals: Vec::new(),
            start_symbol: NonTerminalId(0),
            productions: HashMap::new(),
        }
    }

    pub fn add_non_terminal(&mut self, name: &str) -> NonTerminalId {
        let id = NonTerminalId(self.non_terminals.len());
        self.non_terminals.push(name.to_string());
        id
    }

    pub fn set_start(&mut self, id: NonTerminalId) {
        self.start_symbol = id;
    }

    pub fn add_production(&mut self, non_terminal: NonTerminalId, production: Production) {
        self.productions
            .entry(non_terminal)
            .or_insert_with(Vec::new)
            .push(production);
    }
}

impl Default for RegularGrammar {
    fn default() -> Self {
        Self::new()
    }
}
