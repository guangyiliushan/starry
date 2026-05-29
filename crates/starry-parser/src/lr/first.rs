use crate::analysis::{FirstSet, FirstSetCalculator};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use std::collections::HashSet;

/// LR 专属 FIRST 集计算器
///
/// 扩展自 analysis::FirstSetCalculator，提供 LR 解析器特有的功能
pub struct LRFirstCalculator {
    first_sets: FirstSet,
    nullable: HashSet<NonTerminalId>,
    end_marker: TerminalId,
}

impl LRFirstCalculator {
    pub fn new(cfg: &ContextFreeGrammar) -> Self {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let end_marker = TerminalId(cfg.terminals.len());

        LRFirstCalculator {
            first_sets,
            nullable,
            end_marker,
        }
    }

    pub fn end_marker(&self) -> TerminalId {
        self.end_marker
    }

    pub fn first_set(&self) -> &FirstSet {
        &self.first_sets
    }

    pub fn nullable(&self) -> &HashSet<NonTerminalId> {
        &self.nullable
    }

    pub fn is_nullable(&self, nt: NonTerminalId) -> bool {
        self.nullable.contains(&nt)
    }

    pub fn compute_first_of_symbols(&self, symbols: &[Symbol]) -> HashSet<TerminalId> {
        let mut result = HashSet::new();

        for symbol in symbols {
            match symbol {
                Symbol::Terminal(term_id) => {
                    result.insert(*term_id);
                    return result;
                }
                Symbol::NonTerminal(nt_id) => {
                    for term_opt in self.first_sets.get(*nt_id) {
                        if let Some(term_id) = term_opt {
                            result.insert(*term_id);
                        }
                    }

                    if !self.nullable.contains(nt_id) {
                        return result;
                    }
                }
                Symbol::Epsilon => {}
            }
        }

        result
    }

    pub fn compute_first_of_symbols_with_end_marker(
        &self,
        symbols: &[Symbol],
        lookahead: TerminalId,
    ) -> HashSet<TerminalId> {
        let mut result = HashSet::new();

        for symbol in symbols {
            match symbol {
                Symbol::Terminal(term_id) => {
                    result.insert(*term_id);
                    return result;
                }
                Symbol::NonTerminal(nt_id) => {
                    for term_opt in self.first_sets.get(*nt_id) {
                        if let Some(term_id) = term_opt {
                            result.insert(*term_id);
                        }
                    }

                    if !self.nullable.contains(nt_id) {
                        return result;
                    }
                }
                Symbol::Epsilon => {}
            }
        }

        result.insert(lookahead);
        result
    }

    pub fn is_string_nullable(&self, symbols: &[Symbol]) -> bool {
        symbols.iter().all(|sym| {
            matches!(sym, Symbol::NonTerminal(nt) if self.nullable.contains(nt))
                || matches!(sym, Symbol::Epsilon)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr_first_calculator() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let calculator = LRFirstCalculator::new(&grammar);

        let t_id = grammar.non_terminal_map.get("T").unwrap();
        let symbols = vec![Symbol::NonTerminal(*t_id)];
        let first = calculator.compute_first_of_symbols(&symbols);

        assert!(!first.is_empty());
    }

    #[test]
    fn test_nullable_detection() {
        let grammar = parse_grammar("S -> A B\nA -> a | ε\nB -> b");
        let calculator = LRFirstCalculator::new(&grammar);

        let a_id = grammar.non_terminal_map.get("A").unwrap();
        assert!(calculator.is_nullable(*a_id));

        let b_id = grammar.non_terminal_map.get("B").unwrap();
        assert!(!calculator.is_nullable(*b_id));
    }
}
