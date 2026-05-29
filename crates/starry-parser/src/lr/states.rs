use crate::cfg::{ContextFreeGrammar, Symbol};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::goto::{LR0Goto, LR1Goto};
use crate::lr::item::{LR0Item, LR1Item};
use crate::lr::item_set::{LR0ItemSet, LR1ItemSet};
use crate::lr::lookahead::LookaheadCalculator;
use std::collections::HashMap;
use std::fmt;

pub struct LR0ItemSetCollection {
    states: Vec<LR0ItemSet>,
    transitions: HashMap<(usize, Symbol), usize>,
}

impl LR0ItemSetCollection {
    pub fn new() -> Self {
        LR0ItemSetCollection {
            states: Vec::new(),
            transitions: HashMap::new(),
        }
    }

    pub fn build(cfg: &ContextFreeGrammar) -> Self {
        let mut collection = LR0ItemSetCollection::new();
        let mut worklist: Vec<usize> = Vec::new();

        let initial_item = LR0Item::from_production(0);
        let mut initial_set = LR0ItemSet::new();
        initial_set.insert(initial_item);
        let initial_closure = initial_set.closure(cfg);

        collection.states.push(initial_closure);
        worklist.push(0);

        while let Some(state_idx) = worklist.pop() {
            let symbols = LR0Goto::get_transitions(collection.states[state_idx].items(), cfg);

            for symbol in symbols {
                if let Some(new_set) = collection.states[state_idx].goto(cfg, &symbol) {
                    let existing_idx = collection
                        .states
                        .iter()
                        .position(|s| s.items() == new_set.items());

                    if let Some(idx) = existing_idx {
                        collection.transitions.insert((state_idx, symbol.clone()), idx);
                    } else {
                        let new_idx = collection.states.len();
                        collection.transitions.insert((state_idx, symbol.clone()), new_idx);
                        collection.states.push(new_set);
                        worklist.push(new_idx);
                    }
                }
            }
        }

        collection
    }

    pub fn build_from_augmented(augmented: &AugmentedGrammar) -> Self {
        Self::build(augmented.grammar())
    }

    pub fn states(&self) -> &[LR0ItemSet] {
        &self.states
    }

    pub fn state(&self, index: usize) -> Option<&LR0ItemSet> {
        self.states.get(index)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn transitions(&self) -> &HashMap<(usize, Symbol), usize> {
        &self.transitions
    }

    pub fn get_transition(&self, state: usize, symbol: &Symbol) -> Option<usize> {
        self.transitions.get(&(state, symbol.clone())).copied()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(0) Item Set Collection ({} states):", self.len());
        for (idx, state) in self.states.iter().enumerate() {
            println!("\n I{}:", idx);
            state.print(cfg);
        }
        println!("\nTransitions: ");
        let mut sorted_transitions: Vec<_> = self.transitions.iter().collect();
        sorted_transitions.sort_by_key(|((state, _), target)| (*state, *target));
        for ((from_state, symbol), to_state) in sorted_transitions {
            let symbol_name = match symbol {
                Symbol::Terminal(t) => cfg.get_terminal_name(*t),
                Symbol::NonTerminal(n) => cfg.get_non_terminal_name(*n),
                Symbol::Epsilon => "ε",
            };
            println!("  State {} --{}--> State {}", from_state, symbol_name, to_state);
        }
    }
}

impl Default for LR0ItemSetCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LR0ItemSetCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(0) Item Set Collection ({} states):", self.len())?;
        for (idx, state) in self.states.iter().enumerate() {
            writeln!(f, "\n=== State {} ===", idx)?;
            write!(f, "{}", state)?;
        }
        Ok(())
    }
}

pub struct LR1ItemSetCollection {
    states: Vec<LR1ItemSet>,
    transitions: HashMap<(usize, Symbol), usize>,
}

impl LR1ItemSetCollection {
    pub fn new() -> Self {
        LR1ItemSetCollection {
            states: Vec::new(),
            transitions: HashMap::new(),
        }
    }

    pub fn build(cfg: &ContextFreeGrammar, calculator: &LookaheadCalculator) -> Self {
        let mut collection = LR1ItemSetCollection::new();
        let mut worklist: Vec<usize> = Vec::new();

        let end_marker = calculator.end_marker();
        let initial_item = LR1Item::from_production(0, end_marker);
        let mut initial_set = LR1ItemSet::new();
        initial_set.insert(initial_item);
        let initial_closure = initial_set.closure(cfg, calculator);

        collection.states.push(initial_closure);
        worklist.push(0);

        while let Some(state_idx) = worklist.pop() {
            let symbols = LR1Goto::get_transitions(collection.states[state_idx].items(), cfg);

            for symbol in symbols {
                if let Some(new_set) = collection.states[state_idx].goto(cfg, calculator, &symbol) {
                    let existing_idx = collection
                        .states
                        .iter()
                        .position(|s| s.items() == new_set.items());

                    if let Some(idx) = existing_idx {
                        collection.transitions.insert((state_idx, symbol.clone()), idx);
                    } else {
                        let new_idx = collection.states.len();
                        collection.transitions.insert((state_idx, symbol.clone()), new_idx);
                        collection.states.push(new_set);
                        worklist.push(new_idx);
                    }
                }
            }
        }

        collection
    }

    pub fn build_from_augmented(
        augmented: &AugmentedGrammar,
        calculator: &LookaheadCalculator,
    ) -> Self {
        Self::build(augmented.grammar(), calculator)
    }

    pub fn states(&self) -> &[LR1ItemSet] {
        &self.states
    }

    pub fn state(&self, index: usize) -> Option<&LR1ItemSet> {
        self.states.get(index)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn transitions(&self) -> &HashMap<(usize, Symbol), usize> {
        &self.transitions
    }

    pub fn get_transition(&self, state: usize, symbol: &Symbol) -> Option<usize> {
        self.transitions.get(&(state, symbol.clone())).copied()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(1) Item Set Collection ({} states):", self.len());
        for (idx, state) in self.states.iter().enumerate() {
            println!("\n I{}:", idx);
            state.print(cfg);
        }
        println!("\nTransitions: ");
        let mut sorted_transitions: Vec<_> = self.transitions.iter().collect();
        sorted_transitions.sort_by_key(|((state, _), target)| (*state, *target));
        for ((from_state, symbol), to_state) in sorted_transitions {
            let symbol_name = match symbol {
                Symbol::Terminal(t) => cfg.get_terminal_name(*t),
                Symbol::NonTerminal(n) => cfg.get_non_terminal_name(*n),
                Symbol::Epsilon => "ε",
            };
            println!("  State {} --{}--> State {}", from_state, symbol_name, to_state);
        }
    }
}

impl Default for LR1ItemSetCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LR1ItemSetCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(1) Item Set Collection ({} states):", self.len())?;
        for (idx, state) in self.states.iter().enumerate() {
            writeln!(f, "\n=== State {} ===", idx)?;
            write!(f, "{}", state)?;
        }
        Ok(())
    }
}
