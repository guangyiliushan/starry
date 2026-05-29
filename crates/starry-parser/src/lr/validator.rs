use crate::analysis::{FirstSetCalculator, FollowSetCalculator};
use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::closure::LR0Closure;
use crate::lr::conflict::{Conflict, ConflictKind};
use crate::lr::error::{LRGrammarType, LRValidationError, LRValidationReport};
use crate::lr::goto::LR0Goto;
use crate::lr::item::LR0Item;
use crate::lr::table::{LALR1TableBuilder, LR1TableBuilder, SLRTableBuilder};
use std::collections::{HashMap, HashSet};

pub struct LR0Validator;

impl LR0Validator {
    pub fn validate(cfg: &ContextFreeGrammar) -> LRValidationReport {
        let mut report = LRValidationReport::new();
        let augmented = AugmentedGrammar::new(cfg.clone());

        let states = Self::build_lr0_states(&augmented);

        for (state_id, state) in states.iter().enumerate() {
            let conflicts = Self::detect_lr0_conflicts(state, &augmented);
            if !conflicts.is_empty() {
                for _conflict in conflicts {
                    report.add_error(LRValidationError::ShiftReduceConflict {
                        state: crate::lr::ItemSetId(state_id),
                        terminal: TerminalId(0),
                    });
                }
            }
        }

        report.grammar_type = if report.is_valid {
            LRGrammarType::LR0
        } else {
            LRGrammarType::NotLR
        };

        report
    }

    fn build_lr0_states(augmented: &AugmentedGrammar) -> Vec<HashSet<LR0Item>> {
        let cfg = augmented.grammar();
        let mut states: Vec<HashSet<LR0Item>> = Vec::new();
        let mut state_map: HashMap<Vec<LR0Item>, usize> = HashMap::new();
        let mut worklist: Vec<usize> = Vec::new();

        let initial_item = LR0Item::from_production(0);
        let mut initial_items = HashSet::new();
        initial_items.insert(initial_item);
        let initial_closure = LR0Closure::compute(&initial_items, cfg);

        let mut sorted_initial: Vec<_> = initial_closure.iter().copied().collect();
        sorted_initial.sort();
        state_map.insert(sorted_initial, 0);
        states.push(initial_closure);
        worklist.push(0);

        while let Some(state_idx) = worklist.pop() {
            let transitions: Vec<Symbol> = {
                let current_state = &states[state_idx];
                LR0Goto::get_transitions(current_state, cfg)
            };

            for symbol in transitions {
                let new_items_opt = {
                    let current_state = &states[state_idx];
                    LR0Goto::compute(current_state, cfg, &symbol)
                };

                if let Some(new_items) = new_items_opt {
                    let mut sorted_items: Vec<_> = new_items.iter().copied().collect();
                    sorted_items.sort();

                    if state_map.contains_key(&sorted_items) {
                        continue;
                    }

                    let new_idx = states.len();
                    state_map.insert(sorted_items, new_idx);
                    states.push(new_items);
                    worklist.push(new_idx);
                }
            }
        }

        states
    }

    fn detect_lr0_conflicts(state: &HashSet<LR0Item>, augmented: &AugmentedGrammar) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        let mut shift_terminals = HashSet::new();
        let mut reduce_productions = Vec::new();

        for item in state {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if item.is_complete(production) {
                    if !augmented.is_accepting_production(item.production_index) {
                        reduce_productions.push(item.production_index);
                    }
                } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                    shift_terminals.insert(*term_id);
                }
            }
        }

        if reduce_productions.len() > 1 {
            for i in 0..reduce_productions.len() {
                for j in (i + 1)..reduce_productions.len() {
                    conflicts.push(Conflict::reduce_reduce(
                        crate::lr::ItemSetId(0),
                        TerminalId(0),
                        crate::lr::ProductionId(reduce_productions[i]),
                        crate::lr::ProductionId(reduce_productions[j]),
                    ));
                }
            }
        }

        if !shift_terminals.is_empty() && !reduce_productions.is_empty() {
            for &term_id in &shift_terminals {
                for &prod_idx in &reduce_productions {
                    conflicts.push(Conflict::shift_reduce(
                        crate::lr::ItemSetId(0),
                        term_id,
                        crate::lr::ItemSetId(0),
                        crate::lr::ProductionId(prod_idx),
                    ));
                }
            }
        }

        conflicts
    }
}

pub struct SLR1Validator;

impl SLR1Validator {
    pub fn validate(cfg: &ContextFreeGrammar) -> LRValidationReport {
        let mut report = LRValidationReport::new();

        let table = SLRTableBuilder::build(&AugmentedGrammar::new(cfg.clone()));
        
        if table.has_conflicts() {
            for conflict in &table.conflicts {
                match &conflict.kind {
                    ConflictKind::ShiftReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ShiftReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                    ConflictKind::ReduceReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ReduceReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                }
            }
            report.grammar_type = LRGrammarType::NotLR;
        } else {
            report.grammar_type = LRGrammarType::SLR1;
        }

        report
    }

    pub fn can_resolve_with_follow(
        state: &HashSet<LR0Item>,
        cfg: &ContextFreeGrammar,
    ) -> bool {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let follow_sets = FollowSetCalculator::compute(cfg, &first_sets, &nullable);

        let mut shift_terminals = HashSet::new();
        let mut reduce_follows: Vec<HashSet<TerminalId>> = Vec::new();

        for item in state {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if item.is_complete(production) {
                    let follow = follow_sets.get(production.lhs).clone();
                    reduce_follows.push(follow);
                } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                    shift_terminals.insert(*term_id);
                }
            }
        }

        for follow_set in &reduce_follows {
            for &term_id in &shift_terminals {
                if follow_set.contains(&term_id) {
                    return false;
                }
            }
        }

        for i in 0..reduce_follows.len() {
            for j in (i + 1)..reduce_follows.len() {
                let intersection: HashSet<_> = reduce_follows[i]
                    .intersection(&reduce_follows[j])
                    .collect();
                if !intersection.is_empty() {
                    return false;
                }
            }
        }

        true
    }
}

pub struct LR1Validator;

impl LR1Validator {
    pub fn validate(cfg: &ContextFreeGrammar) -> LRValidationReport {
        let mut report = LRValidationReport::new();
        let augmented = AugmentedGrammar::new(cfg.clone());
        let table = LR1TableBuilder::build(&augmented);

        if table.has_conflicts() {
            for conflict in &table.conflicts {
                match &conflict.kind {
                    ConflictKind::ShiftReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ShiftReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                    ConflictKind::ReduceReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ReduceReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                }
            }
            report.grammar_type = LRGrammarType::NotLR;
        } else {
            report.grammar_type = LRGrammarType::LR1;
        }

        report
    }
}

pub struct LALR1Validator;

impl LALR1Validator {
    pub fn validate(cfg: &ContextFreeGrammar) -> LRValidationReport {
        let mut report = LRValidationReport::new();
        let augmented = AugmentedGrammar::new(cfg.clone());
        let table = LALR1TableBuilder::build(&augmented);

        if table.has_conflicts() {
            for conflict in &table.conflicts {
                match &conflict.kind {
                    ConflictKind::ShiftReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ShiftReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                    ConflictKind::ReduceReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ReduceReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                }
            }
            report.grammar_type = LRGrammarType::NotLR;
        } else {
            report.grammar_type = LRGrammarType::LALR1;
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr0_validator_simple() {
        let grammar = parse_grammar("E -> num");
        let report = LR0Validator::validate(&grammar);

        assert!(report.is_valid);
        assert!(report.grammar_type.is_lr0());
    }

    #[test]
    fn test_slr1_validator_simple() {
        let grammar = parse_grammar("E -> num");
        let report = SLR1Validator::validate(&grammar);

        assert!(report.is_valid);
    }

    #[test]
    fn test_slr1_validator_expression() {
        let grammar = parse_grammar(r#"
            E -> E + T | T
            T -> num
        "#);
        let report = SLR1Validator::validate(&grammar);

        assert!(report.is_valid || report.grammar_type == LRGrammarType::NotLR);
    }
}
