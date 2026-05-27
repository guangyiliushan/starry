use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::dfa::{Dfa, DfaStateId};
use crate::nfa::{Nfa, NfaStateId};

pub struct SubsetConstruction;

impl SubsetConstruction {
    pub fn build(nfa: &Nfa) -> Dfa {
        let alphabet: BTreeSet<char> = nfa
            .states
            .iter()
            .flat_map(|state| state.transitions.keys())
            .filter_map(|symbol| *symbol)
            .collect();

        let start_closure = epsilon_closure(nfa, &[nfa.start]);
        let start_set = sorted(&start_closure);

        let mut dfa = Dfa::new();
        let start_state = dfa.add_state();
        dfa.start = start_state;

        let mut dfa_sets: Vec<Vec<NfaStateId>> = vec![start_set.clone()];
        let mut set_to_state: HashMap<Vec<NfaStateId>, DfaStateId> = HashMap::new();
        set_to_state.insert(start_set.clone(), start_state);

        if let Some(token_name) = find_highest_priority_token(nfa, &start_set) {
            dfa.mark_accept(start_state, token_name);
        }

        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(0);

        while let Some(index) = queue.pop_front() {
            let current_set = dfa_sets[index].clone();
            let current_state = set_to_state[&current_set];

            for &symbol in &alphabet {
                let moved = move_states(nfa, &current_set, symbol);
                let closure = epsilon_closure(nfa, &moved);
                let target_set = sorted(&closure);

                if target_set.is_empty() {
                    continue;
                }

                let target_state = if let Some(&state) = set_to_state.get(&target_set) {
                    state
                } else {
                    let new_state = dfa.add_state();
                    set_to_state.insert(target_set.clone(), new_state);
                    dfa_sets.push(target_set.clone());

                    if let Some(token_name) = find_highest_priority_token(nfa, &target_set) {
                        dfa.mark_accept(new_state, token_name);
                    }

                    queue.push_back(dfa_sets.len() - 1);
                    new_state
                };

                dfa.add_transition(current_state, symbol, target_state);
            }
        }

        dfa
    }
}

fn epsilon_closure(nfa: &Nfa, states: &[NfaStateId]) -> Vec<NfaStateId> {
    let mut closure: BTreeSet<NfaStateId> = states.iter().cloned().collect();
    let mut stack: Vec<NfaStateId> = states.to_vec();

    while let Some(state) = stack.pop() {
        if let Some(nfa_state) = nfa.states.get(state.0) {
            if let Some(targets) = nfa_state.transitions.get(&None) {
                for &target in targets {
                    if closure.insert(target) {
                        stack.push(target);
                    }
                }
            }
        }
    }

    closure.into_iter().collect()
}

fn move_states(nfa: &Nfa, states: &[NfaStateId], symbol: char) -> Vec<NfaStateId> {
    let mut result: BTreeSet<NfaStateId> = BTreeSet::new();
    for &state in states {
        if let Some(nfa_state) = nfa.states.get(state.0) {
            if let Some(targets) = nfa_state.transitions.get(&Some(symbol)) {
                for &target in targets {
                    result.insert(target);
                }
            }
        }
    }
    result.into_iter().collect()
}

fn sorted(states: &[NfaStateId]) -> Vec<NfaStateId> {
    let mut sorted_states = states.to_vec();
    sorted_states.sort_by_key(|state| state.0);
    sorted_states
}

fn find_highest_priority_token(nfa: &Nfa, states: &[NfaStateId]) -> Option<String> {
    for &state in states {
        if let Some(token_name) = nfa.accepts.get(&state) {
            return Some(token_name.clone());
        }
    }
    None
}

pub fn nfa_to_dfa(nfa: &Nfa) -> Dfa {
    SubsetConstruction::build(nfa)
}
