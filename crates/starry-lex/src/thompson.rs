use crate::nfa::{Nfa, NfaStateId};
use crate::regex::Regex;

pub struct ThompsonBuilder;

impl ThompsonBuilder {
    pub fn build_single(regex: &Regex, token_name: &str) -> Nfa {
        let mut converter = RegexToNfaConverter::new();
        let nfa = converter.convert(regex);
        let mut result_nfa = Nfa::new();
        
        for state in nfa.states {
            result_nfa.states.push(state);
        }
        
        result_nfa.start = nfa.start;
        
        if let Some(&accept_state) = nfa.accepts.keys().next() {
            result_nfa.mark_accept(accept_state, token_name.to_string());
        }
        
        result_nfa
    }

    pub fn union(nfas: Vec<Nfa>) -> Nfa {
        if nfas.is_empty() {
            return Nfa::new();
        }

        if nfas.len() == 1 {
            return nfas.into_iter().next().unwrap();
        }

        let mut final_nfa = Nfa::new();
        let new_start = final_nfa.add_state();
        final_nfa.start = new_start;

        for nfa in nfas {
            let offset = final_nfa.states.len();

            for state in nfa.states {
                let mut new_state = state.clone();
                for targets in new_state.transitions.values_mut() {
                    for target in targets.iter_mut() {
                        target.0 += offset;
                    }
                }
                final_nfa.states.push(new_state);
            }

            let old_start = NfaStateId(nfa.start.0 + offset);
            final_nfa.add_transition(new_start, None, old_start);

            for (accept_state, token_name) in nfa.accepts {
                let new_accept_state = NfaStateId(accept_state.0 + offset);
                final_nfa.mark_accept(new_accept_state, token_name);
            }
        }

        final_nfa
    }
}

struct RegexToNfaConverter ;

impl RegexToNfaConverter {
    fn new() -> Self {
        RegexToNfaConverter
    }

    fn convert(&mut self, regex: &Regex) -> Nfa {
        match regex {
            Regex::Empty => self.build_empty(),
            Regex::Char(char) => self.build_char(*char),
            Regex::Concat(left_regex, right_regex) => self.build_concat(left_regex, right_regex),
            Regex::Union(left_regex, right_regex) => self.build_union(left_regex, right_regex),
            Regex::Star(regex) => self.build_star(regex),
        }
    }

    fn build_empty(&mut self) -> Nfa {
        let mut nfa = Nfa::new();
        let start = nfa.add_state();
        let accept = nfa.add_state();
        nfa.start = start;
        nfa.add_transition(start, None, accept);
        nfa
    }

    fn build_char(&mut self, char: char) -> Nfa {
        let mut nfa = Nfa::new();
        let start = nfa.add_state();
        let accept = nfa.add_state();
        nfa.start = start;
        nfa.add_transition(start, Some(char), accept);
        nfa.mark_accept(accept, "temp".to_string());
        nfa
    }

    fn build_concat(&mut self, left_regex: &Regex, right_regex: &Regex) -> Nfa {
        let mut left_nfa = self.convert(left_regex);
        let right_nfa = self.convert(right_regex);

        let offset = left_nfa.states.len();

        for state in right_nfa.states {
            let mut new_state = state.clone();
            for targets in new_state.transitions.values_mut() {
                for target in targets.iter_mut() {
                    target.0 += offset;
                }
            }
            left_nfa.states.push(new_state);
        }

        let right_start = NfaStateId(right_nfa.start.0 + offset);
        if let Some(&left_accept) = left_nfa.accepts.keys().next() {
            left_nfa.add_transition(left_accept, None, right_start);
            left_nfa.accepts.remove(&left_accept);
        }

        for (accept_state, token_name) in right_nfa.accepts {
            let new_accept_state = NfaStateId(accept_state.0 + offset);
            left_nfa.mark_accept(new_accept_state, token_name);
        }

        left_nfa
    }

    fn build_union(&mut self, left_regex: &Regex, right_regex: &Regex) -> Nfa {
        let left_nfa = self.convert(left_regex);
        let right_nfa = self.convert(right_regex);

        let mut final_nfa = Nfa::new();
        let start = final_nfa.add_state();
        let accept = final_nfa.add_state();
        final_nfa.start = start;

        let left_offset = final_nfa.states.len();
        for state in left_nfa.states {
            let mut new_state = state.clone();
            for targets in new_state.transitions.values_mut() {
                for target in targets.iter_mut() {
                    target.0 += left_offset;
                }
            }
            final_nfa.states.push(new_state);
        }

        let left_start = NfaStateId(left_nfa.start.0 + left_offset);
        final_nfa.add_transition(start, None, left_start);

        if let Some(&left_accept) = left_nfa.accepts.keys().next() {
            let new_left_accept = NfaStateId(left_accept.0 + left_offset);
            final_nfa.add_transition(new_left_accept, None, accept);
        }

        let right_offset = final_nfa.states.len();
        for state in right_nfa.states {
            let mut new_state = state.clone();
            for targets in new_state.transitions.values_mut() {
                for target in targets.iter_mut() {
                    target.0 += right_offset;
                }
            }
            final_nfa.states.push(new_state);
        }

        let right_start = NfaStateId(right_nfa.start.0 + right_offset);
        final_nfa.add_transition(start, None, right_start);

        if let Some(&right_accept) = right_nfa.accepts.keys().next() {
            let new_right_accept = NfaStateId(right_accept.0 + right_offset);
            final_nfa.add_transition(new_right_accept, None, accept);
        }

        final_nfa.mark_accept(accept, "union".to_string());
        final_nfa
    }

    fn build_star(&mut self, regex: &Regex) -> Nfa {
        let inner_nfa = self.convert(regex);

        let mut final_nfa = Nfa::new();
        let start = final_nfa.add_state();
        let accept = final_nfa.add_state();
        final_nfa.start = start;

        let offset = final_nfa.states.len();
        for state in inner_nfa.states {
            let mut new_state = state.clone();
            for targets in new_state.transitions.values_mut() {
                for target in targets.iter_mut() {
                    target.0 += offset;
                }
            }
            final_nfa.states.push(new_state);
        }

        let inner_start = NfaStateId(inner_nfa.start.0 + offset);
        final_nfa.add_transition(start, None, inner_start);
        final_nfa.add_transition(start, None, accept);

        if let Some(&inner_accept) = inner_nfa.accepts.keys().next() {
            let new_inner_accept = NfaStateId(inner_accept.0 + offset);
            final_nfa.add_transition(new_inner_accept, None, inner_start);
            final_nfa.add_transition(new_inner_accept, None, accept);
        }

        final_nfa.mark_accept(accept, "star".to_string());
        final_nfa
    }
}

pub fn regex_to_nfa(regex: &Regex, token_name: &str) -> Nfa {
    ThompsonBuilder::build_single(regex, token_name)
}
