use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::dfa::{Dfa, DfaStateId};
use crate::nfa::{Nfa, NfaStateId};
use crate::subset;

pub struct HopcroftMinimizer;

impl HopcroftMinimizer {
    pub fn minimize(dfa: &Dfa) -> Dfa {
        let alphabet: BTreeSet<char> = dfa
            .states
            .iter()
            .flat_map(|state| state.transitions.keys())
            .copied()
            .collect();

        let mut partitions: Vec<BTreeSet<DfaStateId>> = Vec::new();

        let mut token_to_states: HashMap<String, BTreeSet<DfaStateId>> = HashMap::new();
        let mut non_accepting_states: BTreeSet<DfaStateId> = BTreeSet::new();

        for (i, _) in dfa.states.iter().enumerate() {
            let state_id = DfaStateId(i);
            if let Some(token_name) = dfa.accepts.get(&state_id) {
                token_to_states
                    .entry(token_name.clone())
                    .or_default()
                    .insert(state_id);
            } else {
                non_accepting_states.insert(state_id);
            }
        }

        for states in token_to_states.into_values() {
            partitions.push(states);
        }
        if !non_accepting_states.is_empty() {
            partitions.push(non_accepting_states);
        }

        let mut worklist: VecDeque<BTreeSet<DfaStateId>> = partitions.iter().cloned().collect();

        while let Some(current_partition) = worklist.pop_front() {
            for &symbol in &alphabet {
                let predecessors = find_predecessors(dfa, &current_partition, symbol);

                let mut new_partitions = Vec::new();
                let mut should_update_worklist = false;
                let mut partition_to_remove: Option<BTreeSet<DfaStateId>> = None;
                let mut partitions_to_add: Vec<BTreeSet<DfaStateId>> = Vec::new();

                for partition in &partitions {
                    let intersection: BTreeSet<DfaStateId> =
                        predecessors.intersection(partition).cloned().collect();
                    let difference: BTreeSet<DfaStateId> =
                        partition.difference(&predecessors).cloned().collect();

                    if !intersection.is_empty() && !difference.is_empty() {
                        new_partitions.push(intersection.clone());
                        new_partitions.push(difference.clone());

                        if worklist.contains(partition) {
                            partition_to_remove = Some(partition.clone());
                            partitions_to_add.push(intersection.clone());
                            partitions_to_add.push(difference);
                        } else if intersection.len() <= difference.len() {
                            partitions_to_add.push(intersection);
                        } else {
                            partitions_to_add.push(difference);
                        }
                        should_update_worklist = true;
                    } else {
                        new_partitions.push(partition.clone());
                    }
                }

                if should_update_worklist {
                    partitions = new_partitions;
                    if let Some(to_remove) = partition_to_remove {
                        worklist.retain(|set| set != &to_remove);
                    }
                    for partition_to_add in partitions_to_add {
                        worklist.push_back(partition_to_add);
                    }
                    break;
                }
            }
        }

        build_minimized_dfa(dfa, &partitions, &alphabet)
    }
}

pub struct MooreMinimizer;

impl MooreMinimizer {
    pub fn minimize(dfa: &Dfa) -> Dfa {
        let alphabet: BTreeSet<char> = dfa
            .states
            .iter()
            .flat_map(|state| state.transitions.keys())
            .copied()
            .collect();

        let all_states: BTreeSet<DfaStateId> =
            dfa.states.iter().enumerate().map(|(i, _)| DfaStateId(i)).collect();
        let mut partition: HashMap<DfaStateId, usize> = HashMap::new();

        let mut token_to_id: HashMap<String, usize> = HashMap::new();
        let mut next_id = 0;

        for &state in &all_states {
            if let Some(token_name) = dfa.accepts.get(&state) {
                let id = *token_to_id.entry(token_name.clone()).or_insert_with(|| {
                    let id = next_id;
                    next_id += 1;
                    id
                });
                partition.insert(state, id);
            } else {
                partition.insert(state, 0);
            }
        }

        loop {
            let mut new_partition: HashMap<DfaStateId, Vec<usize>> = HashMap::new();

            for &state in &all_states {
                let mut signature = vec![partition[&state]];

                for &symbol in &alphabet {
                    if let Some(next_state) = get_transition_target(dfa, state, symbol) {
                        signature.push(partition[&next_state]);
                    } else {
                        signature.push(usize::MAX);
                    }
                }

                new_partition.insert(state, signature);
            }

            let mut signature_to_id: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
            let mut new_partition_ids: HashMap<DfaStateId, usize> = HashMap::new();
            let mut next_id = 0;

            for (state, signature) in &new_partition {
                if let Some(&id) = signature_to_id.get(signature) {
                    new_partition_ids.insert(*state, id);
                } else {
                    signature_to_id.insert(signature.clone(), next_id);
                    new_partition_ids.insert(*state, next_id);
                    next_id += 1;
                }
            }

            if partition == new_partition_ids {
                break;
            }

            partition = new_partition_ids;
        }

        let partitions = build_partitions_from_ids(&partition);
        build_minimized_dfa(dfa, &partitions, &alphabet)
    }
}

pub struct BrzozowskiMinimizer;

impl BrzozowskiMinimizer {
    pub fn minimize(dfa: &Dfa) -> Dfa {
        let reversed_nfa = reverse_dfa(dfa);
        let intermediate_dfa = subset::SubsetConstruction::build(&reversed_nfa);
        let reversed_again = reverse_dfa(&intermediate_dfa);
        subset::SubsetConstruction::build(&reversed_again)
    }
}

fn find_predecessors(dfa: &Dfa, states: &BTreeSet<DfaStateId>, symbol: char) -> BTreeSet<DfaStateId> {
    let mut predecessors = BTreeSet::new();
    for (i, state) in dfa.states.iter().enumerate() {
        if let Some(&target) = state.transitions.get(&symbol) {
            if states.contains(&target) {
                predecessors.insert(DfaStateId(i));
            }
        }
    }
    predecessors
}

fn get_transition_target(dfa: &Dfa, state: DfaStateId, symbol: char) -> Option<DfaStateId> {
    dfa.states.get(state.0)?.transitions.get(&symbol).copied()
}

fn build_partitions_from_ids(partition: &HashMap<DfaStateId, usize>) -> Vec<BTreeSet<DfaStateId>> {
    let mut partitions_map: HashMap<usize, BTreeSet<DfaStateId>> = HashMap::new();

    for (state, &id) in partition {
        partitions_map.entry(id).or_default().insert(*state);
    }

    partitions_map.into_values().collect()
}

fn build_minimized_dfa(
    dfa: &Dfa,
    partitions: &[BTreeSet<DfaStateId>],
    alphabet: &BTreeSet<char>,
) -> Dfa {
    let mut state_mapping: HashMap<DfaStateId, DfaStateId> = HashMap::new();

    for (index, partition) in partitions.iter().enumerate() {
        let new_state = DfaStateId(index);
        for &state in partition {
            state_mapping.insert(state, new_state);
        }
    }

    let mut new_dfa = Dfa::new();

    for _ in 0..partitions.len() {
        new_dfa.add_state();
    }

    let new_start = state_mapping[&dfa.start];
    new_dfa.start = new_start;

    let mut processed_transitions: HashSet<(DfaStateId, char, DfaStateId)> = HashSet::new();

    for partition in partitions {
        if let Some(&representative) = partition.iter().next() {
            let new_state = state_mapping[&representative];

            if let Some(token_name) = dfa.accepts.get(&representative) {
                new_dfa.mark_accept(new_state, token_name.clone());
            }

            for &symbol in alphabet {
                if let Some(target) = get_transition_target(dfa, representative, symbol) {
                    let new_target = state_mapping[&target];
                    if processed_transitions.insert((new_state, symbol, new_target)) {
                        new_dfa.add_transition(new_state, symbol, new_target);
                    }
                }
            }
        }
    }

    new_dfa
}

fn reverse_dfa(dfa: &Dfa) -> Nfa {
    let mut nfa = Nfa::new();

    for _ in 0..dfa.states.len() {
        nfa.add_state();
    }

    for (i, state) in dfa.states.iter().enumerate() {
        for (&symbol, &target) in &state.transitions {
            nfa.add_transition(
                NfaStateId(target.0),
                Some(symbol),
                NfaStateId(i),
            );
        }
    }

    let new_start = if dfa.accepts.len() == 1 {
        let accept_state = *dfa.accepts.keys().next().unwrap();
        NfaStateId(accept_state.0)
    } else {
        let new_start_state = nfa.add_state();
        for &accept_state in dfa.accepts.keys() {
            nfa.add_transition(new_start_state, None, NfaStateId(accept_state.0));
        }
        new_start_state
    };

    nfa.start = new_start;
    nfa.mark_accept(NfaStateId(dfa.start.0), "reversed_accept".to_string());

    nfa
}

pub fn hopcroft(dfa: &Dfa) -> Dfa {
    HopcroftMinimizer::minimize(dfa)
}

pub fn moore(dfa: &Dfa) -> Dfa {
    MooreMinimizer::minimize(dfa)
}

pub fn brzozowski(dfa: &Dfa) -> Dfa {
    BrzozowskiMinimizer::minimize(dfa)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dfa() -> Dfa {
        let mut dfa = Dfa::new();

        let s0 = dfa.add_state();
        let s1 = dfa.add_state();
        let s2 = dfa.add_state();
        let s3 = dfa.add_state();

        dfa.start = s0;
        dfa.mark_accept(s3, "test_token".to_string());

        dfa.add_transition(s0, 'a', s1);
        dfa.add_transition(s0, 'b', s2);
        dfa.add_transition(s1, 'a', s3);
        dfa.add_transition(s1, 'b', s2);
        dfa.add_transition(s2, 'a', s1);
        dfa.add_transition(s2, 'b', s3);
        dfa.add_transition(s3, 'a', s3);
        dfa.add_transition(s3, 'b', s3);

        dfa
    }

    #[test]
    fn test_hopcroft() {
        let dfa = create_test_dfa();
        let minimized = HopcroftMinimizer::minimize(&dfa);
        assert!(minimized.states.len() <= dfa.states.len());
    }

    #[test]
    fn test_moore() {
        let dfa = create_test_dfa();
        let minimized = MooreMinimizer::minimize(&dfa);
        assert!(minimized.states.len() <= dfa.states.len());
    }

    #[test]
    fn test_brzozowski() {
        let dfa = create_test_dfa();
        let minimized = BrzozowskiMinimizer::minimize(&dfa);
        assert!(minimized.states.len() <= dfa.states.len());
    }
}
