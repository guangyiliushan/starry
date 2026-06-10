use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::action::{Action, ProductionId};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::conflict::Conflict;
use crate::lr::goto::ItemSetId;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::states::LR0ItemSetCollection;
use crate::lr::table_builders::{LRTable, LrTableCore};
use std::fmt;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct LR0Table {
    core: LrTableCore,
}

impl Deref for LR0Table {
    type Target = LrTableCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for LR0Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl LRTable for LR0Table {
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

impl LR0Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LR0Table {
            core: LrTableCore::new(state_count, terminal_count, non_terminal_count),
        }
    }

    pub fn grammar_type(&self) -> LRGrammarType {
        if self.has_conflicts() {
            LRGrammarType::NotLR
        } else {
            LRGrammarType::LR0
        }
    }

    pub fn is_lr0(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        self.core.print_header(cfg, "LR(0) Parsing Table:");
        if !self.conflicts.is_empty() {
            println!();
            println!("Conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LR0Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.core.fmt_header(f, "LR(0) Parsing Table:")?;
        if !self.conflicts.is_empty() {
            writeln!(f, "\nConflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LR0TableBuilder;

impl LR0TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LR0Table {
        let collection = LR0ItemSetCollection::build_from_augmented(augmented);
        Self::build_from_collection(augmented, &collection)
    }

    pub fn build_from_collection(
        augmented: &AugmentedGrammar,
        collection: &LR0ItemSetCollection,
    ) -> LR0Table {
        let cfg = augmented.grammar();
        let state_count = collection.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();
        let end_marker = TerminalId(terminal_count);

        let mut table = LR0Table::new(state_count, terminal_count, non_terminal_count);
        table.goto.set_augmented_start(augmented.augmented_start());

        for (state_idx, item_set) in collection.states().iter().enumerate() {
            let state_id = ItemSetId(state_idx);

            for item in item_set.items() {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if augmented.is_accepting_production(item.production_index) {
                            table.action.set_accept(state_id, end_marker);
                        } else {
                            for term_idx in 0..=terminal_count {
                                let terminal = TerminalId(term_idx);
                                table.action.set_reduce(
                                    state_id,
                                    terminal,
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

        table.conflicts = Self::detect_conflicts(&table, collection, augmented);
        table
    }

    fn detect_conflicts(
        table: &LR0Table,
        collection: &LR0ItemSetCollection,
        augmented: &AugmentedGrammar,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in collection.states().iter().enumerate() {
            for term_idx in 0..=cfg.terminals.len() {
                let terminal = TerminalId(term_idx);
                let action = table.action.get(ItemSetId(state_id), terminal);

                if let Action::Shift(shift_state) = action {
                    for item in item_set.items() {
                        if let Some(production) = cfg.productions.get(item.production_index) {
                            if item.is_complete(production) {
                                conflicts.push(Conflict::shift_reduce(
                                    ItemSetId(state_id),
                                    terminal,
                                    *shift_state,
                                    ProductionId(item.production_index),
                                ));
                            }
                        }
                    }
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
    fn test_lr0_table_creation() {
        let table = LR0Table::new(5, 3, 2);

        assert_eq!(table.state_count, 5);
        assert_eq!(table.terminal_count, 3);
        assert_eq!(table.non_terminal_count, 2);
        assert!(!table.has_conflicts());
    }

    #[test]
    fn test_lr0_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR0TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr0());
    }
}
