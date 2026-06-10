use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::action::ProductionId;
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::conflict::Conflict;
use crate::lr::goto::ItemSetId;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::lookahead::LookaheadCalculator;
use crate::lr::states::LR1ItemSetCollection;
use crate::lr::table_builders::{LRTable, LrTableCore};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct LR1Table {
    core: LrTableCore,
}

impl Deref for LR1Table {
    type Target = LrTableCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for LR1Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl LRTable for LR1Table {
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

impl LR1Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LR1Table {
            core: LrTableCore::new(state_count, terminal_count, non_terminal_count),
        }
    }

    pub fn grammar_type(&self) -> LRGrammarType {
        if self.conflicts.is_empty() {
            LRGrammarType::LR1
        } else {
            LRGrammarType::NotLR
        }
    }

    pub fn is_lr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        self.core.print_header(cfg, "LR(1) Parsing Table:");
        if !self.conflicts.is_empty() {
            println!();
            println!("Conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LR1Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.core.fmt_header(f, "LR(1) Parsing Table:")?;
        if !self.conflicts.is_empty() {
            writeln!(f, "\nConflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LR1TableBuilder;

impl LR1TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LR1Table {
        let cfg = augmented.grammar();
        let calculator = LookaheadCalculator::new(cfg);
        let collection = LR1ItemSetCollection::build_from_augmented(augmented, &calculator);
        Self::build_from_collection(augmented, &collection, &calculator)
    }

    pub fn build_from_collection(
        augmented: &AugmentedGrammar,
        collection: &LR1ItemSetCollection,
        calculator: &LookaheadCalculator,
    ) -> LR1Table {
        let cfg = augmented.grammar();
        let state_count = collection.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();
        let end_marker = calculator.end_marker();

        let mut table = LR1Table::new(state_count, terminal_count, non_terminal_count);
        table.goto.set_augmented_start(augmented.augmented_start());

        for (state_idx, item_set) in collection.states().iter().enumerate() {
            let state_id = ItemSetId(state_idx);

            for item in item_set.items() {
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
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::Terminal(*term_id))
                        {
                            table.action.set_shift(state_id, *term_id, ItemSetId(next_state));
                        }
                    } else if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::NonTerminal(*nt_id))
                        {
                            table.goto.set(state_id, *nt_id, ItemSetId(next_state));
                        }
                    }
                }
            }
        }

        table.conflicts =
            Self::detect_conflicts(&table, collection, augmented, end_marker);
        table
    }

    fn detect_conflicts(
        _table: &LR1Table,
        collection: &LR1ItemSetCollection,
        augmented: &AugmentedGrammar,
        end_marker: TerminalId,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in collection.states().iter().enumerate() {
            let mut shift_terms: HashMap<TerminalId, ItemSetId> = HashMap::new();
            let mut reduce_terms: HashMap<TerminalId, ProductionId> = HashMap::new();

            for item in item_set.items() {
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
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_id, &Symbol::Terminal(*term_id))
                        {
                            shift_terms.insert(*term_id, ItemSetId(next_state));
                        }
                    }
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
    fn test_lr1_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR1TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr1());
    }

    #[test]
    fn test_lr1_table_expression() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR1TableBuilder::build(&augmented);

        assert!(table.is_lr1());
    }
}
