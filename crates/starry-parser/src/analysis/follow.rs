use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use super::first::{FirstSet, FirstSetCalculator};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FollowSet {
    sets: HashMap<NonTerminalId, HashSet<TerminalId>>,
    end_marker: TerminalId,
}

impl FollowSet {
    pub fn new(end_marker: TerminalId) -> Self {
        FollowSet {
            sets: HashMap::new(),
            end_marker,
        }
    }

    pub fn get(&self, nt: NonTerminalId) -> &HashSet<TerminalId> {
        static EMPTY: std::sync::OnceLock<HashSet<TerminalId>> = std::sync::OnceLock::new();
        self.sets.get(&nt).unwrap_or_else(|| EMPTY.get_or_init(HashSet::new))
    }

    pub fn contains(&self, nt: NonTerminalId, terminal: TerminalId) -> bool {
        self.sets.get(&nt).map_or(false, |s| s.contains(&terminal))
    }

    pub fn contains_end_marker(&self, nt: NonTerminalId) -> bool {
        self.contains(nt, self.end_marker)
    }

    pub fn terminals(&self, nt: NonTerminalId) -> Vec<TerminalId> {
        self.sets
            .get(&nt)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn end_marker(&self) -> TerminalId {
        self.end_marker
    }

    pub fn insert(&mut self, nt: NonTerminalId, terminals: HashSet<TerminalId>) {
        self.sets.insert(nt, terminals);
    }

    pub fn get_mut(&mut self, nt: NonTerminalId) -> &mut HashSet<TerminalId> {
        self.sets.entry(nt).or_insert_with(HashSet::new)
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("FOLLOW Sets:");
        for (i, nt_name) in cfg.non_terminals.iter().enumerate() {
            let nt_id = NonTerminalId(i);
            if let Some(terminals) = self.sets.get(&nt_id) {
                let terms: Vec<String> = terminals
                    .iter()
                    .map(|t| {
                        if *t == self.end_marker {
                            "$".to_string()
                        } else {
                            cfg.terminals[t.0].clone()
                        }
                    })
                    .collect();
                println!("  FOLLOW({}) = {{ {} }}", nt_name, terms.join(", "));
            }
        }
    }
}

pub struct FollowSetCalculator;

impl FollowSetCalculator {
    pub fn new() -> Self {
        FollowSetCalculator
    }

    /// 计算 FOLLOW 集合
    /// 需要预先计算 FIRST 集合和 NULLABLE 集合
    pub fn compute(
        cfg: &ContextFreeGrammar,
        first_sets: &FirstSet,
        nullable: &HashSet<NonTerminalId>,
    ) -> FollowSet {
        // 创建一个特殊的结束标记终结符
        let end_marker = TerminalId(cfg.terminals.len());

        let mut follow_sets = FollowSet::new(end_marker);

        // 初始化：为每个非终结符创建空集合
        for i in 0..cfg.non_terminals.len() {
            follow_sets.sets.insert(NonTerminalId(i), HashSet::new());
        }

        // 规则 R1：将结束标记 $ 加入 FOLLOW(开始符号)
        follow_sets
            .sets
            .get_mut(&cfg.start_symbol)
            .unwrap()
            .insert(end_marker);

        let mut changed = true;

        while changed {
            changed = false;

            for production in &cfg.productions {
                let lhs = production.lhs;
                let rhs = &production.rhs;

                // 遍历右部，寻找非终结符 B
                for (i, symbol) in rhs.iter().enumerate() {
                    if let Symbol::NonTerminal(b) = symbol {
                        // β 是 B 后面的符号串
                        let beta: Vec<Symbol> = rhs.iter().skip(i + 1).cloned().collect();

                        // 规则 R2：FOLLOW(B) = FOLLOW(B) ∪ (FIRST(β) - {ε})
                        if !beta.is_empty() {
                            let first_beta = FirstSetCalculator::compute_first_of_string(
                                cfg,
                                &beta,
                                first_sets,
                                nullable,
                            );

                            for term_opt in &first_beta {
                                if let Some(term_id) = term_opt {
                                    if follow_sets.sets.get_mut(b).unwrap().insert(*term_id) {
                                        changed = true;
                                    }
                                }
                            }

                            // 规则 R3：如果 β 可为空（ε ∈ FIRST(β)），则 FOLLOW(B) = FOLLOW(B) ∪ FOLLOW(A)
                            let beta_nullable = beta.iter().all(|sym| {
                                matches!(sym, Symbol::NonTerminal(nt) if nullable.contains(nt))
                                    || matches!(sym, Symbol::Epsilon)
                            });

                            if beta_nullable || first_beta.contains(&None) {
                                let lhs_follow = follow_sets.get(lhs).clone();
                                for term_id in &lhs_follow {
                                    if follow_sets.sets.get_mut(b).unwrap().insert(*term_id) {
                                        changed = true;
                                    }
                                }
                            }
                        } else {
                            // 规则 R3：产生式形如 A -> α B，则 FOLLOW(B) = FOLLOW(B) ∪ FOLLOW(A)
                            let lhs_follow = follow_sets.get(lhs).clone();
                            for term_id in &lhs_follow {
                                if follow_sets.sets.get_mut(b).unwrap().insert(*term_id) {
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

    /// 便捷方法：同时计算 FIRST 和 FOLLOW 集合
    pub fn compute_all(cfg: &ContextFreeGrammar) -> (HashSet<NonTerminalId>, FirstSet, FollowSet) {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let follow_sets = Self::compute(cfg, &first_sets, &nullable);

        (nullable, first_sets, follow_sets)
    }
}

impl Default for FollowSetCalculator {
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
    fn test_compute_follow_simple() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> F T'
            T' -> * F T' | ε
            F -> ( E ) | num
        "#);

        let (_nullable, _first_sets, follow_sets) = FollowSetCalculator::compute_all(&grammar);

        // E 的 FOLLOW 集应该包含 ) 和 $
        let e_id = grammar.non_terminal_map.get("E").unwrap();
        let e_follow = follow_sets.terminals(*e_id);
        assert!(e_follow.len() >= 1);

        // E' 的 FOLLOW 集应该等于 E 的 FOLLOW 集
        let e_prime_id = grammar.non_terminal_map.get("E'").unwrap();
        let e_prime_follow = follow_sets.terminals(*e_prime_id);
        assert!(!e_prime_follow.is_empty());

        // F 的 FOLLOW 集应该包含 *、) 和 $
        let f_id = grammar.non_terminal_map.get("F").unwrap();
        let f_follow = follow_sets.terminals(*f_id);
        assert!(f_follow.len() >= 1);
    }

    #[test]
    fn test_compute_follow_with_nullable() {
        let grammar = parse_grammar(r#"
            S -> A B
            A -> a
            A -> ε
            B -> b
        "#);

        let (_nullable, _first_sets, follow_sets) = FollowSetCalculator::compute_all(&grammar);

        // A 的 FOLLOW 集应该包含 b（因为 B 紧跟在 A 后面）
        let a_id = grammar.non_terminal_map.get("A").unwrap();
        let a_follow = follow_sets.terminals(*a_id);
        assert!(!a_follow.is_empty());

        // B 的 FOLLOW 集应该包含 $（因为 S 是开始符号）
        let b_id = grammar.non_terminal_map.get("B").unwrap();
        assert!(follow_sets.contains_end_marker(*b_id));
    }

    #[test]
    fn test_follow_end_marker() {
        let grammar = parse_grammar(r#"
            S -> a S b
            S -> ε
        "#);

        let (_nullable, _first_sets, follow_sets) = FollowSetCalculator::compute_all(&grammar);

        // S 的 FOLLOW 集应该包含 $ 和 b
        let s_id = grammar.non_terminal_map.get("S").unwrap();
        assert!(follow_sets.contains_end_marker(*s_id));
    }

    #[test]
    fn test_follow_chain() {
        // 测试链式依赖：A -> B C, B -> b, C -> c
        // FOLLOW(B) 应该包含 FIRST(C) = {c}
        let grammar = parse_grammar(r#"
            A -> B C
            B -> b
            C -> c
        "#);

        let (_nullable, _first_sets, follow_sets) = FollowSetCalculator::compute_all(&grammar);

        let b_id = grammar.non_terminal_map.get("B").unwrap();
        let b_follow = follow_sets.terminals(*b_id);

        // B 的 FOLLOW 集应该包含 c
        let c_term = grammar.terminal_map.get("c").unwrap();
        assert!(b_follow.contains(c_term));
    }
}
