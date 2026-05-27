use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NfaStateId(pub usize);

pub type Symbol = Option<char>;

#[derive(Debug, Clone)]
pub struct NfaState {
    pub transitions: HashMap<Symbol, Vec<NfaStateId>>,
}

impl NfaState {
    pub fn new() -> Self {
        NfaState {
            transitions: HashMap::new(),
        }
    }

    pub fn add_transition(&mut self, symbol: Symbol, target: NfaStateId) {
        self.transitions
            .entry(symbol)
            .or_insert_with(Vec::new)
            .push(target);
    }
}

impl Default for NfaState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Nfa {
    pub states: Vec<NfaState>,
    pub start: NfaStateId,
    pub accepts: HashMap<NfaStateId, String>,
}

impl Nfa {
    pub fn new() -> Self {
        Nfa {
            states: Vec::new(),
            start: NfaStateId(0),
            accepts: HashMap::new(),
        }
    }

    pub fn add_state(&mut self) -> NfaStateId {
        let id = NfaStateId(self.states.len());
        self.states.push(NfaState::new());
        id
    }

    pub fn add_transition(&mut self, from: NfaStateId, symbol: Symbol, to: NfaStateId) {
        if let Some(state) = self.states.get_mut(from.0) {
            state.add_transition(symbol, to);
        }
    }

    pub fn mark_accept(&mut self, state: NfaStateId, token_name: String) {
        self.accepts.insert(state, token_name);
    }

    pub fn print(&self) {
        println!("NFA:");
        println!("  Start: {:?}", self.start);
        println!("  Accepts:");
        for (state, token) in &self.accepts {
            println!("    {:?} -> {}", state, token);
        }
        println!("  Transitions:");
        for (i, state) in self.states.iter().enumerate() {
            for (symbol, targets) in &state.transitions {
                let symbol_str = match symbol {
                    Some(ch) => format!("'{}'", ch),
                    None => "ε".to_string(),
                };
                for target in targets {
                    println!("    {:?} --{}--> {:?}", NfaStateId(i), symbol_str, target);
                }
            }
        }
    }
}

impl Default for Nfa {
    fn default() -> Self {
        Self::new()
    }
}
