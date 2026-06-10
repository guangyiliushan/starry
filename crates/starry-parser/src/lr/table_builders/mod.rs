use crate::cfg::{ContextFreeGrammar, TerminalId};
use crate::lr::action::ActionTable;
use crate::lr::goto::GotoTable;
use crate::lr::conflict::{Conflict, ConflictReport};
use std::fmt;

pub trait LRTable {
    fn action(&self) -> &ActionTable;
    fn goto(&self) -> &GotoTable;
    fn has_conflicts(&self) -> bool;
    fn state_count(&self) -> usize;
    fn terminal_count(&self) -> usize;
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct LrTableCore {
    pub action: ActionTable,
    pub goto: GotoTable,
    pub state_count: usize,
    pub terminal_count: usize,
    pub non_terminal_count: usize,
    pub conflicts: Vec<Conflict>,
}

impl LrTableCore {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LrTableCore {
            action: ActionTable::new(state_count, terminal_count),
            goto: GotoTable::new(state_count, non_terminal_count),
            state_count,
            terminal_count,
            non_terminal_count,
            conflicts: Vec::new(),
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_report(&self) -> ConflictReport {
        ConflictReport::new(self.conflicts.clone())
    }

    pub fn terminal_names(cfg: &ContextFreeGrammar) -> Vec<String> {
        (0..cfg.terminals.len())
            .map(|i| cfg.get_terminal_name(TerminalId(i)).to_string())
            .collect()
    }

    pub fn non_terminal_names(cfg: &ContextFreeGrammar) -> Vec<String> {
        (0..cfg.non_terminals.len())
            .map(|i| {
                cfg.get_non_terminal_name(crate::cfg::NonTerminalId(i))
                    .to_string()
            })
            .collect()
    }

    pub fn print_header(&self, cfg: &ContextFreeGrammar, label: &str) {
        println!("{}", label);
        println!();
        let tn = Self::terminal_names(cfg);
        let nn = Self::non_terminal_names(cfg);
        self.action.print(&tn);
        println!();
        self.goto.print(&nn);
    }

    pub fn fmt_header(&self, f: &mut fmt::Formatter<'_>, label: &str) -> fmt::Result {
        writeln!(f, "{}", label)?;
        writeln!(f, "{}", self.action)?;
        writeln!(f, "{}", self.goto)
    }
}

pub mod lr0;
pub mod slr1;
pub mod lr1;
pub mod lalr1;
