use std::collections::HashMap;

use crate::nfa::{Nfa, NfaStateId};
use crate::regex::Regex;

use super::ast::{NonTerminalId, Production, RegularGrammar};

impl RegularGrammar {
    pub fn to_nfa(&self) -> Nfa {
        let mut nfa = Nfa::new();
        let mut state_map: HashMap<NonTerminalId, NfaStateId> = HashMap::new();

        for (i, _) in self.non_terminals.iter().enumerate() {
            let state = nfa.add_state();
            state_map.insert(NonTerminalId(i), state);
        }

        let accept_state = nfa.add_state();

        for (&non_terminal, productions) in &self.productions {
            let from_state = state_map[&non_terminal];

            for production in productions {
                match production {
                    Production::Epsilon => {
                        nfa.add_transition(from_state, None, accept_state);
                    }
                    Production::Terminal(ch) => {
                        nfa.add_transition(from_state, Some(*ch), accept_state);
                    }
                    Production::TerminalNonTerminal(ch, target_non_terminal) => {
                        let to_state = state_map[target_non_terminal];
                        if *ch == '\0' {
                            nfa.add_transition(from_state, None, to_state);
                        } else {
                            nfa.add_transition(from_state, Some(*ch), to_state);
                        }
                    }
                }
            }
        }

        nfa.start = state_map[&self.start_symbol];
        nfa.mark_accept(accept_state, "grammar_accept".to_string());

        nfa
    }

    pub fn to_regex(&self) -> Result<Regex, String> {
        self.to_regex_arden()
    }

    fn to_regex_arden(&self) -> Result<Regex, String> {
        let n = self.non_terminals.len();
        if n == 0 {
            return Ok(Regex::Empty);
        }

        let mut equations: Vec<Vec<Option<Regex>>> = vec![vec![None; n + 1]; n];

        for (&non_terminal, productions) in &self.productions {
            let i = non_terminal.0;
            
            for production in productions {
                let term = match production {
                    Production::Epsilon => {
                        Some(Regex::Empty)
                    }
                    Production::Terminal(ch) => {
                        Some(Regex::Char(*ch))
                    }
                    Production::TerminalNonTerminal(ch, _target) => {
                        if *ch == '\0' {
                            Some(Regex::Empty)
                        } else {
                            Some(Regex::Char(*ch))
                        }
                    }
                };

                match production {
                    Production::Epsilon | Production::Terminal(_) => {
                        if let Some(existing) = &equations[i][n] {
                            equations[i][n] = Some(Regex::Union(
                                Box::new(existing.clone()),
                                Box::new(term.unwrap()),
                            ));
                        } else {
                            equations[i][n] = term;
                        }
                    }
                    Production::TerminalNonTerminal(_, target) => {
                        let j = target.0;
                        if let Some(existing) = &equations[i][j] {
                            equations[i][j] = Some(Regex::Union(
                                Box::new(existing.clone()),
                                Box::new(term.unwrap()),
                            ));
                        } else {
                            equations[i][j] = term;
                        }
                    }
                }
            }
        }

        for k in (0..n).rev() {
            if let Some(ref self_ref) = equations[k][k] {
                let star = Regex::Star(Box::new(self_ref.clone()));
                
                for j in 0..=n {
                    if j != k {
                        if let Some(ref eq_kj) = equations[k][j] {
                            let concat = Regex::Concat(
                                Box::new(star.clone()),
                                Box::new(eq_kj.clone()),
                            );
                            equations[k][j] = Some(concat);
                        }
                    }
                }
                equations[k][k] = None;
            }

            for i in 0..n {
                if i != k {
                    if let Some(eq_ik) = equations[i][k].clone() {
                        for j in 0..=n {
                            if j != k {
                                if let Some(eq_kj) = equations[k][j].clone() {
                                    let concat = Regex::Concat(
                                        Box::new(eq_ik.clone()),
                                        Box::new(eq_kj),
                                    );
                                    
                                    if let Some(eq_ij) = equations[i][j].clone() {
                                        equations[i][j] = Some(Regex::Union(
                                            Box::new(eq_ij),
                                            Box::new(concat),
                                        ));
                                    } else {
                                        equations[i][j] = Some(concat);
                                    }
                                }
                            }
                        }
                        equations[i][k] = None;
                    }
                }
            }
        }

        let start_idx = self.start_symbol.0;
        Ok(equations[start_idx][n].clone().unwrap_or(Regex::Empty))
    }
}
