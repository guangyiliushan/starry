use crate::analysis::{FirstSet, FirstSetCalculator, FollowSet};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use std::collections::HashSet;

/// LR 专属 FOLLOW 集计算器
///
/// 扩展自 analysis::FollowSetCalculator，提供 LR 解析器特有的功能
pub struct LRFollowCalculator;

impl LRFollowCalculator {
    /// 计算 FOLLOW 集合
    pub fn compute(cfg: &ContextFreeGrammar) -> FollowSet {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        Self::compute_with_first(cfg, &first_sets, &nullable)
    }

    /// 使用预先计算的 FIRST 集合计算 FOLLOW 集合
    pub fn compute_with_first(
        cfg: &ContextFreeGrammar,
        first_sets: &FirstSet,
        nullable: &HashSet<NonTerminalId>,
    ) -> FollowSet {
        let end_marker = TerminalId(cfg.terminals.len());
        let mut follow_sets = FollowSet::new(end_marker);

        for i in 0..cfg.non_terminals.len() {
            follow_sets.insert(NonTerminalId(i), HashSet::new());
        }

        follow_sets.get_mut(cfg.start_symbol).insert(end_marker);

        let mut changed = true;
        while changed {
            changed = false;

            for production in &cfg.productions {
                let lhs = production.lhs;
                let rhs = &production.rhs;

                for (i, symbol) in rhs.iter().enumerate() {
                    if let Symbol::NonTerminal(b) = symbol {
                        let beta: Vec<Symbol> = rhs.iter().skip(i + 1).cloned().collect();

                        if !beta.is_empty() {
                            let first_beta = FirstSetCalculator::compute_first_of_string(
                                cfg,
                                &beta,
                                first_sets,
                                nullable,
                            );

                            for term_opt in &first_beta {
                                if let Some(term_id) = term_opt {
                                    if follow_sets.get_mut(*b).insert(*term_id) {
                                        changed = true;
                                    }
                                }
                            }

                            let beta_nullable = beta.iter().all(|sym| {
                                matches!(sym, Symbol::NonTerminal(nt) if nullable.contains(nt))
                                    || matches!(sym, Symbol::Epsilon)
                            });

                            if beta_nullable || first_beta.contains(&None) {
                                let lhs_follow = follow_sets.get(lhs).clone();
                                for term_id in &lhs_follow {
                                    if follow_sets.get_mut(*b).insert(*term_id) {
                                        changed = true;
                                    }
                                }
                            }
                        } else {
                            let lhs_follow = follow_sets.get(lhs).clone();
                            for term_id in &lhs_follow {
                                if follow_sets.get_mut(*b).insert(*term_id) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        follow_sets
    }

    /// 同时计算 FIRST 和 FOLLOW 集合
    pub fn compute_all(cfg: &ContextFreeGrammar) -> (HashSet<NonTerminalId>, FirstSet, FollowSet) {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let follow_sets = Self::compute_with_first(cfg, &first_sets, &nullable);

        (nullable, first_sets, follow_sets)
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
    fn test_lr_follow_calculator() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let follow_sets = LRFollowCalculator::compute(&grammar);

        let e_id = grammar.non_terminal_map.get("E").unwrap();
        assert!(follow_sets.contains_end_marker(*e_id));
    }

    #[test]
    fn test_follow_with_nullable() {
        let grammar = parse_grammar("S -> A B\nA -> a | ε\nB -> b");
        let follow_sets = LRFollowCalculator::compute(&grammar);

        let a_id = grammar.non_terminal_map.get("A").unwrap();
        let a_follow = follow_sets.terminals(*a_id);
        assert!(!a_follow.is_empty());
    }
}
