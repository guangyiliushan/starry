use crate::analysis::{FollowSet, FollowSetCalculator};
use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::action::{ProductionId};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::conflict::Conflict;
use crate::lr::goto::ItemSetId;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::states::LR0ItemSetCollection;
use crate::lr::table_builders::{LRTable, LrTableCore};
use std::fmt;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct SLRTable {
    core: LrTableCore,
    pub resolved_conflicts: Vec<Conflict>,
}

impl Deref for SLRTable {
    type Target = LrTableCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for SLRTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl LRTable for SLRTable {
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

impl SLRTable {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        SLRTable {
            core: LrTableCore::new(state_count, terminal_count, non_terminal_count),
            resolved_conflicts: Vec::new(),
        }
    }

    pub fn grammar_type(&self) -> LRGrammarType {
        if self.conflicts.is_empty() {
            if self.resolved_conflicts.is_empty() {
                LRGrammarType::LR0
            } else {
                LRGrammarType::SLR1
            }
        } else {
            LRGrammarType::NotLR
        }
    }

    pub fn is_slr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn is_lr0(&self) -> bool {
        self.conflicts.is_empty() && self.resolved_conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        self.core.print_header(cfg, "SLR(1) Parsing Table:");
        if !self.resolved_conflicts.is_empty() {
            println!();
            println!("Resolved conflicts by FOLLOW sets:");
            for conflict in &self.resolved_conflicts {
                println!("  {}", conflict);
            }
        }
        if self.has_conflicts() {
            println!();
            println!("Unresolved conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for SLRTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.core.fmt_header(f, "SLR(1) Parsing Table:")?;
        if !self.resolved_conflicts.is_empty() {
            writeln!(f, "\nResolved conflicts by FOLLOW sets:")?;
            for conflict in &self.resolved_conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        if self.has_conflicts() {
            writeln!(f, "\nUnresolved conflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct SLRTableBuilder;

impl SLRTableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> SLRTable {
        let collection = LR0ItemSetCollection::build_from_augmented(augmented);
        Self::build_from_collection(augmented, &collection)
    }

    pub fn build_from_collection(
        augmented: &AugmentedGrammar,
        collection: &LR0ItemSetCollection,
    ) -> SLRTable {
        let cfg = augmented.grammar();
        let (_, _, follow_sets) = FollowSetCalculator::compute_all(cfg);
        let end_marker = TerminalId(cfg.terminals.len());

        let state_count = collection.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();

        let mut table = SLRTable::new(state_count, terminal_count, non_terminal_count);
        table.goto.set_augmented_start(augmented.augmented_start());

        for (state_idx, item_set) in collection.states().iter().enumerate() {
            Self::fill_table_for_state(
                &mut table,
                ItemSetId(state_idx),
                state_idx,
                item_set.items(),
                augmented,
                &follow_sets,
                end_marker,
                collection,
            );
        }

        let (conflicts, resolved) =
            Self::detect_and_resolve_conflicts(&table, collection, augmented, &follow_sets);
        table.conflicts = conflicts;
        table.resolved_conflicts = resolved;

        table
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_table_for_state(
        table: &mut SLRTable,
        state_id: ItemSetId,
        state_idx: usize,
        items: &std::collections::HashSet<crate::lr::item::LR0Item>,
        augmented: &AugmentedGrammar,
        follow_sets: &FollowSet,
        end_marker: TerminalId,
        collection: &LR0ItemSetCollection,
    ) {
        let cfg = augmented.grammar();

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if item.is_complete(production) {
                    if augmented.is_accepting_production(item.production_index) {
                        table.action.set_accept(state_id, end_marker);
                    } else {
                        let lhs = production.lhs;
                        for terminal in follow_sets.get(lhs) {
                            table.action.set_reduce(
                                state_id,
                                *terminal,
                                ProductionId(item.production_index),
                            );
                        }
                    }
                } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                    if let Some(next_state) =
                        collection.get_transition(state_idx, &Symbol::Terminal(*term_id))
                    {
                        table.action.set_shift(state_id, *term_id, ItemSetId(next_state));
                    }
                }
            }
        }

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                    if let Some(next_state) =
                        collection.get_transition(state_idx, &Symbol::NonTerminal(*nt_id))
                    {
                        table.goto.set(state_id, *nt_id, ItemSetId(next_state));
                    }
                }
            }
        }
    }

    fn detect_and_resolve_conflicts(
        _table: &SLRTable,
        collection: &LR0ItemSetCollection,
        augmented: &AugmentedGrammar,
        follow_sets: &FollowSet,
    ) -> (Vec<Conflict>, Vec<Conflict>) {
        let mut unresolved_conflicts = Vec::new();
        let resolved_conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in collection.states().iter().enumerate() {
            let mut shift_items: Vec<(TerminalId, ItemSetId)> = Vec::new();
            let mut reduce_items: Vec<(TerminalId, ProductionId)> = Vec::new();

            for item in item_set.items() {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if !augmented.is_accepting_production(item.production_index) {
                            let lhs = production.lhs;
                            for terminal in follow_sets.get(lhs) {
                                reduce_items.push((*terminal, ProductionId(item.production_index)));
                            }
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_id, &Symbol::Terminal(*term_id))
                        {
                            shift_items.push((*term_id, ItemSetId(next_state)));
                        }
                    }
                }
            }

            for (term, shift_state) in &shift_items {
                for (reduce_term, prod_id) in &reduce_items {
                    if term == reduce_term {
                        let conflict = Conflict::shift_reduce(
                            ItemSetId(state_id),
                            *term,
                            *shift_state,
                            *prod_id,
                        );
                        unresolved_conflicts.push(conflict);
                    }
                }
            }

            for i in 0..reduce_items.len() {
                for j in (i + 1)..reduce_items.len() {
                    if reduce_items[i].0 == reduce_items[j].0 {
                        let conflict = Conflict::reduce_reduce(
                            ItemSetId(state_id),
                            reduce_items[i].0,
                            reduce_items[i].1,
                            reduce_items[j].1,
                        );
                        unresolved_conflicts.push(conflict);
                    }
                }
            }
        }

        (unresolved_conflicts, resolved_conflicts)
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
    fn test_slr_table_creation() {
        let table = SLRTable::new(5, 3, 2);

        assert_eq!(table.state_count, 5);
        assert_eq!(table.terminal_count, 3);
        assert_eq!(table.non_terminal_count, 2);
        assert!(!table.has_conflicts());
    }

    #[test]
    fn test_slr_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = SLRTableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr0());
    }
}
