use crate::analysis::first::{FirstSet, FirstSetCalculator, NullableSet};
use crate::analysis::follow::FollowSet;
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Production, Symbol, TerminalId};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct TableEntry {
    pub production: Production,
    pub production_index: usize,
}

#[derive(Debug, Clone)]
pub struct ParsingTable {
    table: HashMap<(NonTerminalId, TerminalId), TableEntry>,
    end_marker: TerminalId,
    conflicts: Vec<ParsingConflict>,
}

#[derive(Debug, Clone)]
pub struct ParsingConflict {
    pub non_terminal: NonTerminalId,
    pub terminal: TerminalId,
    pub existing_production: Production,
    pub new_production: Production,
}

impl ParsingTable {
    pub fn new(end_marker: TerminalId) -> Self {
        ParsingTable {
            table: HashMap::new(),
            end_marker,
            conflicts: Vec::new(),
        }
    }

    pub fn get(&self, nt: NonTerminalId, term: TerminalId) -> Option<&TableEntry> {
        self.table.get(&(nt, term))
    }

    pub fn contains(&self, nt: NonTerminalId, term: TerminalId) -> bool {
        self.table.contains_key(&(nt, term))
    }

    pub fn insert(
        &mut self,
        nt: NonTerminalId,
        term: TerminalId,
        entry: TableEntry,
    ) -> Result<(), ParsingConflict> {
        if let Some(existing) = self.table.get(&(nt, term)) {
            let conflict = ParsingConflict {
                non_terminal: nt,
                terminal: term,
                existing_production: existing.production.clone(),
                new_production: entry.production.clone(),
            };
            self.conflicts.push(conflict.clone());
            return Err(conflict);
        }
        self.table.insert((nt, term), entry);
        Ok(())
    }

    pub fn is_ll1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn conflicts(&self) -> &[ParsingConflict] {
        &self.conflicts
    }

    pub fn end_marker(&self) -> TerminalId {
        self.end_marker
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LL(1) Parsing Table:");
        println!("{:-<60}", "");

        let mut terminals: Vec<TerminalId> = cfg.terminals.iter().enumerate().map(|(i, _)| TerminalId(i)).collect();
        terminals.push(self.end_marker);

        print!("{:15}", "");
        for term in &terminals {
            let name = if *term == self.end_marker {
                "$".to_string()
            } else {
                cfg.get_terminal_name(*term).to_string()
            };
            print!("{:15}", name);
        }
        println!();

        for (i, nt_name) in cfg.non_terminals.iter().enumerate() {
            let nt_id = NonTerminalId(i);
            print!("{:15}", nt_name);

            for term in &terminals {
                if let Some(entry) = self.get(nt_id, *term) {
                    let prod_str = format_production(cfg, &entry.production);
                    print!("{:15}", prod_str);
                } else {
                    print!("{:15}", "");
                }
            }
            println!();
        }

        if !self.is_ll1() {
            println!("\nConflicts detected:");
            for conflict in &self.conflicts {
                let nt_name = cfg.get_non_terminal_name(conflict.non_terminal);
                let term_name = if conflict.terminal == self.end_marker {
                    "$"
                } else {
                    cfg.get_terminal_name(conflict.terminal)
                };
                println!(
                    "  M[{}, {}]: conflict between '{}' and '{}'",
                    nt_name,
                    term_name,
                    format_production(cfg, &conflict.existing_production),
                    format_production(cfg, &conflict.new_production)
                );
            }
        }
    }
}

fn format_production(cfg: &ContextFreeGrammar, prod: &Production) -> String {
    let lhs = cfg.get_non_terminal_name(prod.lhs);
    let rhs: Vec<String> = prod.rhs.iter().map(|sym| {
        match sym {
            Symbol::NonTerminal(nt) => cfg.get_non_terminal_name(*nt).to_string(),
            Symbol::Terminal(t) => cfg.get_terminal_name(*t).to_string(),
            Symbol::Epsilon => "ε".to_string(),
        }
    }).collect();
    format!("{} → {}", lhs, rhs.join(" "))
}

pub struct ParsingTableBuilder;

impl ParsingTableBuilder {
    pub fn new() -> Self {
        ParsingTableBuilder
    }

    pub fn build(
        cfg: &ContextFreeGrammar,
        first_sets: &FirstSet,
        follow_sets: &FollowSet,
        nullable: &NullableSet,
    ) -> Result<ParsingTable, Vec<ParsingConflict>> {
        let end_marker = follow_sets.end_marker();
        let mut table = ParsingTable::new(end_marker);

        for (prod_index, production) in cfg.productions.iter().enumerate() {
            let lhs = production.lhs;
            let rhs = &production.rhs;

            let first_alpha = FirstSetCalculator::compute_first_of_string(
                cfg,
                rhs,
                first_sets,
                nullable,
            );

            for term_opt in &first_alpha {
                if let Some(term_id) = term_opt {
                    let entry = TableEntry {
                        production: production.clone(),
                        production_index: prod_index,
                    };
                    let _ = table.insert(lhs, *term_id, entry);
                }
            }

            if first_alpha.contains(&None) {
                for follow_term in follow_sets.get(lhs) {
                    let entry = TableEntry {
                        production: production.clone(),
                        production_index: prod_index,
                    };
                    let _ = table.insert(lhs, *follow_term, entry);
                }
            }
        }

        if table.is_ll1() {
            Ok(table)
        } else {
            Err(table.conflicts().to_vec())
        }
    }

    pub fn build_from_grammar(cfg: &ContextFreeGrammar) -> Result<ParsingTable, Vec<ParsingConflict>> {
        let nullable = FirstSetCalculator::compute_nullable_set(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let follow_sets = crate::analysis::follow::FollowSetCalculator::compute(cfg, &first_sets, &nullable);

        Self::build(cfg, &first_sets, &follow_sets, &nullable)
    }

    pub fn check_grammar(cfg: &ContextFreeGrammar) -> Result<Vec<ParsingConflict>, String> {
        let nullable = FirstSetCalculator::compute_nullable_set(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let follow_sets = crate::analysis::follow::FollowSetCalculator::compute(cfg, &first_sets, &nullable);

        match Self::build(cfg, &first_sets, &follow_sets, &nullable) {
            Ok(_) => Ok(Vec::new()),
            Err(conflicts) => Ok(conflicts),
        }
    }
}

impl Default for ParsingTableBuilder {
    fn default() -> Self {
        Self::new()
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
    fn test_build_simple_table() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> F T'
            T' -> * F T' | ε
            F -> ( E ) | num
        "#);

        let result = ParsingTableBuilder::build_from_grammar(&grammar);
        assert!(result.is_ok(), "Grammar should be LL(1)");

        let table = result.unwrap();
        assert!(table.is_ll1());

        let e_id = grammar.non_terminal_map.get("E").unwrap();
        let lparen = grammar.terminal_map.get("(").unwrap();
        assert!(table.contains(*e_id, *lparen));
    }

    #[test]
    fn test_table_with_epsilon() {
        let grammar = parse_grammar(r#"
            S -> a A | ε
            A -> b
        "#);

        let result = ParsingTableBuilder::build_from_grammar(&grammar);
        assert!(result.is_ok());

        let table = result.unwrap();
        let s_id = grammar.non_terminal_map.get("S").unwrap();
        let a_term = grammar.terminal_map.get("a").unwrap();

        assert!(table.contains(*s_id, *a_term));
        assert!(table.contains(*s_id, table.end_marker()));
    }

    #[test]
    fn test_non_ll1_grammar() {
        let grammar = parse_grammar(r#"
            S -> a A | a B
            A -> c
            B -> d
        "#);

        let result = ParsingTableBuilder::build_from_grammar(&grammar);
        assert!(result.is_err(), "Grammar should NOT be LL(1) due to FIRST/FIRST conflict");
    }

    #[test]
    fn test_table_entry_retrieval() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E'
            E' -> ε
            T -> num
        "#);

        let table = ParsingTableBuilder::build_from_grammar(&grammar).unwrap();

        let e_prime_id = grammar.non_terminal_map.get("E'").unwrap();
        let plus = grammar.terminal_map.get("+").unwrap();

        let entry = table.get(*e_prime_id, *plus);
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.production.lhs, *e_prime_id);
        assert!(!entry.production.rhs.is_empty());
    }
}
