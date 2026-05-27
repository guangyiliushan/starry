use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DfaStateId(pub usize);

#[derive(Debug, Clone)]
pub struct DfaState {
    pub transitions: HashMap<char, DfaStateId>,
}

impl DfaState {
    pub fn new() -> Self {
        DfaState {
            transitions: HashMap::new(),
        }
    }

    pub fn add_transition(&mut self, symbol: char, target: DfaStateId) {
        self.transitions.insert(symbol, target);
    }

    pub fn get_transition(&self, symbol: char) -> Option<DfaStateId> {
        self.transitions.get(&symbol).copied()
    }
}

impl Default for DfaState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Dfa {
    pub states: Vec<DfaState>,
    pub start: DfaStateId,
    pub accepts: HashMap<DfaStateId, String>,
}

impl Dfa {
    pub fn new() -> Self {
        Dfa {
            states: Vec::new(),
            start: DfaStateId(0),
            accepts: HashMap::new(),
        }
    }

    pub fn add_state(&mut self) -> DfaStateId {
        let id = DfaStateId(self.states.len());
        self.states.push(DfaState::new());
        id
    }

    pub fn add_transition(&mut self, from: DfaStateId, symbol: char, to: DfaStateId) {
        if let Some(state) = self.states.get_mut(from.0) {
            state.add_transition(symbol, to);
        }
    }

    pub fn mark_accept(&mut self, state: DfaStateId, token_name: String) {
        self.accepts.insert(state, token_name);
    }

    pub fn print(&self) {
        println!("DFA:");
        println!("  Start: {:?}", self.start);
        println!("  Accepts:");
        for (state, token) in &self.accepts {
            println!("    {:?} -> {}", state, token);
        }
        println!("  Transitions:");
        for (i, state) in self.states.iter().enumerate() {
            for (symbol, target) in &state.transitions {
                println!(
                    "    {:?} --'{}'--> {:?}",
                    DfaStateId(i),
                    symbol,
                    target
                );
            }
        }
    }
}

impl Default for Dfa {
    fn default() -> Self {
        Self::new()
    }
}
