use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum LeftRecursionType {
    Direct,
    Indirect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LeftRecursionInfo {
    pub non_terminal: NonTerminalId,
    pub rec_type: LeftRecursionType,
    pub path: Vec<NonTerminalId>,
}

pub struct LeftRecursionDetector;

impl LeftRecursionDetector {
    pub fn new() -> Self {
        LeftRecursionDetector
    }

    /// 检测文法中是否存在左递归（包括直接和间接）
    pub fn detect(&self, cfg: &ContextFreeGrammar) -> Vec<LeftRecursionInfo> {
        let mut results = Vec::new();
        let n = cfg.non_terminals.len();

        // 构建依赖图：A -> B 表示 A 的产生式右部第一个非终结符是 B
        let mut adj: HashMap<NonTerminalId, Vec<NonTerminalId>> = HashMap::new();

        for production in &cfg.productions {
            if let Some(first) = production.rhs.first() {
                if let Symbol::NonTerminal(nt_id) = first {
                    adj.entry(production.lhs)
                        .or_default()
                        .push(*nt_id);
                }
            }
        }

        // 对每个非终结符进行DFS检测环
        for i in 0..n {
            let start = NonTerminalId(i);
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            self.dfs_detect(
                start,
                start,
                &adj,
                &mut visited,
                &mut path,
                &mut results,
            );
        }

        // 去重：只保留每个非终结符的第一次检测结果
        let mut seen = HashSet::new();
        results
            .into_iter()
            .filter(|info| {
                let key = info.non_terminal;
                if seen.contains(&key) {
                    false
                } else {
                    seen.insert(key);
                    true
                }
            })
            .collect()
    }

    fn dfs_detect(
        &self,
        current: NonTerminalId,
        target: NonTerminalId,
        adj: &HashMap<NonTerminalId, Vec<NonTerminalId>>,
        visited: &mut HashSet<NonTerminalId>,
        path: &mut Vec<NonTerminalId>,
        results: &mut Vec<LeftRecursionInfo>,
    ) {
        if visited.contains(&current) {
            // 检查是否形成环回到目标
            if current == target && path.len() >= 1 {
                let rec_type = if path.len() == 1 {
                    LeftRecursionType::Direct
                } else {
                    LeftRecursionType::Indirect
                };

                let mut cycle_path = path.clone();
                cycle_path.push(target);

                results.push(LeftRecursionInfo {
                    non_terminal: target,
                    rec_type,
                    path: cycle_path,
                });
            }
            return;
        }

        visited.insert(current);
        path.push(current);

        if let Some(neighbors) = adj.get(&current) {
            for &next in neighbors {
                self.dfs_detect(next, target, adj, visited, path, results);
            }
        }

        path.pop();
        visited.remove(&current);
    }

    /// 检测是否存在直接左递归
    pub fn detect_direct(&self, cfg: &ContextFreeGrammar) -> Vec<LeftRecursionInfo> {
        let mut results = Vec::new();

        for production in &cfg.productions {
            if let Some(first) = production.rhs.first() {
                if let Symbol::NonTerminal(nt_id) = first {
                    if *nt_id == production.lhs {
                        results.push(LeftRecursionInfo {
                            non_terminal: production.lhs,
                            rec_type: LeftRecursionType::Direct,
                            path: vec![production.lhs, production.lhs],
                        });
                    }
                }
            }
        }

        // 去重
        let mut seen = HashSet::new();
        results
            .into_iter()
            .filter(|info| {
                let key = info.non_terminal;
                if seen.contains(&key) {
                    false
                } else {
                    seen.insert(key);
                    true
                }
            })
            .collect()
    }
}

impl Default for LeftRecursionDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{ContextFreeGrammar, Production, Symbol};

    #[test]
    fn test_detect_direct_left_recursion() {
        let grammar_str = r#"
            Expr -> Expr + Term | Term
            Term -> Term * Factor | Factor
            Factor -> num
        "#;

        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        let detector = LeftRecursionDetector::new();
        let results = detector.detect(&cfg);

        assert!(!results.is_empty());

        let expr_rec = results
            .iter()
            .find(|r| r.non_terminal.0 == 0)
            .expect("Expr should have left recursion");
        assert_eq!(expr_rec.rec_type, LeftRecursionType::Direct);

        let term_rec = results
            .iter()
            .find(|r| r.non_terminal.0 == 1)
            .expect("Term should have left recursion");
        assert_eq!(term_rec.rec_type, LeftRecursionType::Direct);
    }

    #[test]
    fn test_detect_indirect_left_recursion() {
        let mut cfg = ContextFreeGrammar::new();

        let a = cfg.add_non_terminal("A");
        let b = cfg.add_non_terminal("B");
        let c = cfg.add_terminal("c");
        let d = cfg.add_terminal("d");

        cfg.set_start(a);

        // A -> B c | d
        cfg.add_production(Production::new(
            a,
            vec![Symbol::NonTerminal(b), Symbol::Terminal(c)],
        ));
        cfg.add_production(Production::new(a, vec![Symbol::Terminal(d)]));

        // B -> A b  (间接左递归: A -> B -> A)
        let b_terminal = cfg.add_terminal("b");
        cfg.add_production(Production::new(
            b,
            vec![Symbol::NonTerminal(a), Symbol::Terminal(b_terminal)],
        ));

        let detector = LeftRecursionDetector::new();
        let results = detector.detect(&cfg);

        assert!(!results.is_empty());

        let a_rec = results
            .iter()
            .find(|r| r.non_terminal == a)
            .expect("A should have indirect left recursion");
        assert_eq!(a_rec.rec_type, LeftRecursionType::Indirect);
    }

    #[test]
    fn test_no_left_recursion() {
        let grammar_str = r#"
            Expr -> Term + Expr | Term
            Term -> Factor * Term | Factor
            Factor -> num
        "#;

        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        let detector = LeftRecursionDetector::new();
        let results = detector.detect(&cfg);

        assert!(results.is_empty(), "This grammar should have no left recursion");
    }
}
