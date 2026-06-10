use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::action::{Action, ProductionId};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::conflict::Conflict;
use crate::lr::core_set::CoreSetMerger;
use crate::lr::goto::ItemSetId;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::lookahead::LookaheadCalculator;
use crate::lr::states::LR1ItemSetCollection;
use crate::lr::table_builders::{LRTable, LrTableCore};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct LALR1Table {
    core: LrTableCore,
    pub merged_states: usize,
}

impl Deref for LALR1Table {
    type Target = LrTableCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for LALR1Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl LRTable for LALR1Table {
    fn action(&self) -> &crate::lr::action::ActionTable {
        &self.core.action
    }

    fn goto(&self) -> &crate::lr::goto::GotoTable {
        &self.core.goto
    }

    fn has_conflicts(&self) -> bool {
        self.core.has_conflicts()
    }

    fn state_count(&self) -> usize {
        self.core.state_count
    }

    fn terminal_count(&self) -> usize {
        self.core.terminal_count
    }
}

impl LALR1Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LALR1Table {
            core: LrTableCore::new(state_count, terminal_count, non_terminal_count),
            merged_states: 0,
        }
    }

    pub fn grammar_type(&self) -> LRGrammarType {
        if self.conflicts.is_empty() {
            LRGrammarType::LALR1
        } else {
            LRGrammarType::NotLR
        }
    }

    pub fn is_lalr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        let label = format!(
            "LALR(1) Parsing Table ({} merged states):",
            self.merged_states
        );
        self.core.print_header(cfg, &label);
        if !self.conflicts.is_empty() {
            println!();
            println!("Conflicts introduced by merging:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LALR1Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = format!(
            "LALR(1) Parsing Table ({} merged states):",
            self.merged_states
        );
        self.core.fmt_header(f, &label)?;
        if !self.conflicts.is_empty() {
            writeln!(f, "\nConflicts introduced by merging:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LALR1TableBuilder;

impl LALR1TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LALR1Table {
        let cfg = augmented.grammar();
        let calculator = LookaheadCalculator::new(cfg);
        let lr1_collection = LR1ItemSetCollection::build_from_augmented(augmented, &calculator);
        Self::build_from_lr1_collection(augmented, &lr1_collection, &calculator)
    }

    pub fn build_from_lr1_collection(
        augmented: &AugmentedGrammar,
        lr1_collection: &LR1ItemSetCollection,
        calculator: &LookaheadCalculator,
    ) -> LALR1Table {
        let cfg = augmented.grammar();
        let end_marker = calculator.end_marker();

        let mut merger = CoreSetMerger::new();
        let mut state_mapping: Vec<usize> = Vec::new();

        for item_set in lr1_collection.states() {
            let merged_id = merger.add_or_merge(item_set.items().clone());
            state_mapping.push(merged_id);
        }

        let merged_states = merger.into_states();
        let original_count = lr1_collection.len();
        let merged_count = merged_states.len();

        let mut table =
            LALR1Table::new(merged_count, cfg.terminals.len(), cfg.non_terminals.len());
        table.merged_states = original_count - merged_count;
        table.goto.set_augmented_start(augmented.augmented_start());

        let mut merged_to_rep: Vec<usize> = vec![0; merged_count];
        for (orig_id, &merged_id) in state_mapping.iter().enumerate() {
            merged_to_rep[merged_id] = orig_id;
        }

        for (merged_id, item_set) in merged_states.iter().enumerate() {
            let state_id = ItemSetId(merged_id);
            let rep_orig_id = merged_to_rep[merged_id];

            for item in item_set {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if augmented.is_accepting_production(item.production_index) {
                            if item.lookahead == end_marker {
                                table.action.set_accept(state_id, end_marker);
                            }
                        } else {
                            table.action.set_reduce(
                                state_id,
                                item.lookahead,
                                ProductionId(item.production_index),
                            );
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_orig) = lr1_collection
                            .get_transition(rep_orig_id, &Symbol::Terminal(*term_id))
                        {
                            let next_merged = state_mapping[next_orig];
                            table
                                .action
                                .set_shift(state_id, *term_id, ItemSetId(next_merged));
                        }
                    } else if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        if let Some(next_orig) = lr1_collection
                            .get_transition(rep_orig_id, &Symbol::NonTerminal(*nt_id))
                        {
                            let next_merged = state_mapping[next_orig];
                            table.goto.set(state_id, *nt_id, ItemSetId(next_merged));
                        }
                    }
                }
            }
        }

        table.conflicts =
            Self::detect_conflicts(&table, &merged_states, augmented, end_marker);
        table
    }

    fn detect_conflicts(
        table: &LALR1Table,
        merged_states: &[HashSet<crate::lr::item::LR1Item>],
        augmented: &AugmentedGrammar,
        end_marker: TerminalId,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in merged_states.iter().enumerate() {
            let mut shift_terms: HashMap<TerminalId, ItemSetId> = HashMap::new();
            let mut reduce_terms: HashMap<TerminalId, ProductionId> = HashMap::new();

            for item in item_set {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if !augmented.is_accepting_production(item.production_index)
                            || item.lookahead != end_marker
                        {
                            if let Some(existing) = reduce_terms.get(&item.lookahead) {
                                if *existing != ProductionId(item.production_index) {
                                    conflicts.push(Conflict::reduce_reduce(
                                        ItemSetId(state_id),
                                        item.lookahead,
                                        *existing,
                                        ProductionId(item.production_index),
                                    ));
                                }
                            } else {
                                reduce_terms
                                    .insert(item.lookahead, ProductionId(item.production_index));
                            }
                        }
                    }
                }
            }

            for term_idx in 0..=cfg.terminals.len() {
                let terminal = TerminalId(term_idx);
                let action = table.action.get(ItemSetId(state_id), terminal);
                if let Action::Shift(shift_state) = action {
                    shift_terms.insert(terminal, *shift_state);
                }
            }

            for (term, shift_state) in &shift_terms {
                if let Some(prod_id) = reduce_terms.get(term) {
                    conflicts.push(Conflict::shift_reduce(
                        ItemSetId(state_id),
                        *term,
                        *shift_state,
                        *prod_id,
                    ));
                }
            }
        }

        conflicts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lalr1_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LALR1TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lalr1());
    }

    #[test]
    fn test_lalr1_table_expression() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LALR1TableBuilder::build(&augmented);

        assert!(table.is_lalr1());
    }
}
