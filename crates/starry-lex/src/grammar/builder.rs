use std::collections::HashMap;

use crate::dfa::{Dfa, DfaStateId};
use crate::nfa::{Nfa, NfaStateId};
use crate::regex::Regex;

use super::ast::{NonTerminalId, Production, RegularGrammar};

pub struct GrammarBuilder;

impl GrammarBuilder {
    pub fn from_regex(regex: &Regex) -> RegularGrammar {
        let nfa = crate::thompson::ThompsonBuilder::build_single(regex, "temp");
        Self::from_nfa(&nfa)
    }

    pub fn from_nfa(nfa: &Nfa) -> RegularGrammar {
        let mut grammar = RegularGrammar::new();
        let mut state_to_nonterminal: HashMap<NfaStateId, NonTerminalId> = HashMap::new();

        for (i, _) in nfa.states.iter().enumerate() {
            let state_id = NfaStateId(i);
            let name = if i == 0 {
                "S".to_string()
            } else {
                format!("A{}", i)
            };
            let nt_id = grammar.add_non_terminal(&name);
            state_to_nonterminal.insert(state_id, nt_id);
        }

        if let Some(&start_nt) = state_to_nonterminal.get(&nfa.start) {
            grammar.set_start(start_nt);
        }

        for (i, state) in nfa.states.iter().enumerate() {
            let from_state = NfaStateId(i);
            let from_nt = state_to_nonterminal[&from_state];

            for (symbol, targets) in &state.transitions {
                for &target_state in targets {
                    let to_nt = state_to_nonterminal[&target_state];
                    
                    match symbol {
                        Some(ch) => {
                            grammar.add_production(from_nt, Production::TerminalNonTerminal(*ch, to_nt));
                        }
                        None => {
                            grammar.add_production(from_nt, Production::TerminalNonTerminal('\0', to_nt));
                        }
                    }
                }
            }
        }

        for (&accept_state, _) in &nfa.accepts {
            if let Some(&accept_nt) = state_to_nonterminal.get(&accept_state) {
                grammar.add_production(accept_nt, Production::Epsilon);
            }
        }

        grammar
    }

    pub fn from_dfa(dfa: &Dfa) -> RegularGrammar {
        let mut grammar = RegularGrammar::new();
        let mut state_to_nonterminal: HashMap<DfaStateId, NonTerminalId> = HashMap::new();

        for (i, _) in dfa.states.iter().enumerate() {
            let state_id = DfaStateId(i);
            let name = if i == 0 {
                "S".to_string()
            } else {
                format!("A{}", i)
            };
            let nt_id = grammar.add_non_terminal(&name);
            state_to_nonterminal.insert(state_id, nt_id);
        }

        if let Some(&start_nt) = state_to_nonterminal.get(&dfa.start) {
            grammar.set_start(start_nt);
        }

        for (i, state) in dfa.states.iter().enumerate() {
            let from_state = DfaStateId(i);
            let from_nt = state_to_nonterminal[&from_state];

            for (&symbol, &to_state) in &state.transitions {
                let to_nt = state_to_nonterminal[&to_state];
                grammar.add_production(from_nt, Production::TerminalNonTerminal(symbol, to_nt));
            }
        }

        for (&accept_state, _) in &dfa.accepts {
            if let Some(&accept_nt) = state_to_nonterminal.get(&accept_state) {
                grammar.add_production(accept_nt, Production::Epsilon);
            }
        }

        grammar
    }
}
