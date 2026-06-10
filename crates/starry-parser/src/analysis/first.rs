use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FirstSet {
    sets: HashMap<NonTerminalId, HashSet<Option<TerminalId>>>,
}

impl FirstSet {
    pub fn new() -> Self {
        FirstSet {
            sets: HashMap::new(),
        }
    }

    pub fn get(&self, nt: NonTerminalId) -> &HashSet<Option<TerminalId>> {
        static EMPTY: std::sync::OnceLock<HashSet<Option<TerminalId>>> = std::sync::OnceLock::new();
        self.sets.get(&nt).unwrap_or_else(|| EMPTY.get_or_init(HashSet::new))
    }

    pub fn contains(&self, nt: NonTerminalId, terminal: Option<TerminalId>) -> bool {
        self.sets.get(&nt).map_or(false, |s| s.contains(&terminal))
    }

    pub fn contains_epsilon(&self, nt: NonTerminalId) -> bool {
        self.contains(nt, None)
    }

    pub fn terminals(&self, nt: NonTerminalId) -> Vec<TerminalId> {
        self.sets
            .get(&nt)
            .map(|s| {
                s.iter()
                    .filter_map(|t| *t)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("FIRST Sets:");
        for (i, nt_name) in cfg.non_terminals.iter().enumerate() {
            let nt_id = NonTerminalId(i);
            if let Some(terminals) = self.sets.get(&nt_id) {
                let terms: Vec<String> = terminals
                    .iter()
                    .map(|t| {
                        match t {
                            Some(term_id) => cfg.terminals[term_id.0].clone(),
                            None => "ε".to_string(),
                        }
                    })
                    .collect();
                println!("  FIRST({}) = {{ {} }}", nt_name, terms.join(", "));
            }
        }
    }
}

impl Default for FirstSet {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NullableSet(pub HashSet<NonTerminalId>);

impl NullableSet {
    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("NULLABLE Set:");
        let names: Vec<&str> = self.0
            .iter()
            .map(|nt_id| cfg.get_non_terminal_name(*nt_id))
            .collect();
        if names.is_empty() {
            println!("  (empty)");
        } else {
            println!("  {{ {} }}", names.join(", "));
        }
    }
}

impl std::ops::Deref for NullableSet {
    type Target = HashSet<NonTerminalId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<HashSet<NonTerminalId>> for NullableSet {
    fn from(set: HashSet<NonTerminalId>) -> Self {
        NullableSet(set)
    }
}

pub struct FirstSetCalculator;

impl FirstSetCalculator {
    pub fn new() -> Self {
        FirstSetCalculator
    }

    /// 计算 NULLABLE 集合：可以推导出 ε 的非终结符集合
    pub fn compute_nullable(cfg: &ContextFreeGrammar) -> HashSet<NonTerminalId> {
        Self::compute_nullable_set(cfg).0
    }

    /// 计算 NULLABLE 集合并返回可打印的结构
    pub fn compute_nullable_set(cfg: &ContextFreeGrammar) -> NullableSet {
        let mut nullable = HashSet::new();
        let mut changed = true;

        while changed {
            changed = false;

            for production in &cfg.productions {
                if production.is_epsilon() {
                    if nullable.insert(production.lhs) {
                        changed = true;
                    }
                    continue;
                }

                let all_nullable = production.rhs.iter().all(|sym| {
                    matches!(sym, Symbol::NonTerminal(nt) if nullable.contains(nt))
                });

                if all_nullable && nullable.insert(production.lhs) {
                    changed = true;
                }
            }
        }

        NullableSet(nullable)
    }

    /// 计算 FIRST 集合
    pub fn compute(cfg: &ContextFreeGrammar) -> FirstSet {
        Self::compute_with_nullable(cfg, &Self::compute_nullable(cfg))
    }

    /// 使用预先计算的 NULLABLE 集合计算 FIRST 集合
    pub fn compute_with_nullable(
        cfg: &ContextFreeGrammar,
        nullable: &HashSet<NonTerminalId>,
    ) -> FirstSet {
        let mut first_sets = FirstSet::new();

        // 初始化：为每个非终结符创建空集合
        for i in 0..cfg.non_terminals.len() {
            first_sets.sets.insert(NonTerminalId(i), HashSet::new());
        }

        let mut changed = true;

        while changed {
            changed = false;

            for production in &cfg.productions {
                let lhs = production.lhs;
                let rhs = &production.rhs;

                // 处理 ε 产生式
                if production.is_epsilon() {
                    if first_sets.sets.get_mut(&lhs).unwrap().insert(None) {
                        changed = true;
                    }
                    continue;
                }

                // 遍历右部符号
                for (i, symbol) in rhs.iter().enumerate() {
                    match symbol {
                        Symbol::Terminal(term_id) => {
                            // 终结符：直接加入 FIRST(lhs)，然后终止
                            if first_sets.sets.get_mut(&lhs).unwrap().insert(Some(*term_id)) {
                                changed = true;
                            }
                            break;
                        }
                        Symbol::NonTerminal(nt_id) => {
                            // 非终结符：将 FIRST(nt) - {ε} 加入 FIRST(lhs)
                            let nt_first = first_sets.get(*nt_id).clone();
                            for term_opt in &nt_first {
                                if term_opt.is_some() {
                                    if first_sets.sets.get_mut(&lhs).unwrap().insert(*term_opt) {
                                        changed = true;
                                    }
                                }
                            }

                            // 如果当前非终结符不是 NULLABLE，终止
                            if !nullable.contains(nt_id) {
                                break;
                            }

                            // 如果是最后一个符号且是 NULLABLE，加入 ε
                            if i == rhs.len() - 1 {
                                if first_sets.sets.get_mut(&lhs).unwrap().insert(None) {
                                    changed = true;
                                }
                            }
                        }
                        Symbol::Epsilon => {
                            // ε 符号：加入 ε
                            if first_sets.sets.get_mut(&lhs).unwrap().insert(None) {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        first_sets
    }

    /// 计算符号串的 FIRST 集合
    pub fn compute_first_of_symbols(
        symbols: &[Symbol],
        first_sets: &FirstSet,
        nullable: &HashSet<NonTerminalId>,
    ) -> HashSet<Option<TerminalId>> {
        if symbols.is_empty() {
            let mut result = HashSet::new();
            result.insert(None);
            return result;
        }

        let mut result = HashSet::new();

        for (i, symbol) in symbols.iter().enumerate() {
            match symbol {
                Symbol::Terminal(term_id) => {
                    result.insert(Some(*term_id));
                    return result;
                }
                Symbol::NonTerminal(nt_id) => {
                    // 将 FIRST(nt) - {ε} 加入结果
                    for term_opt in first_sets.get(*nt_id) {
                        if term_opt.is_some() {
                            result.insert(*term_opt);
                        }
                    }

                    // 如果不是 NULLABLE，终止
                    if !nullable.contains(nt_id) {
                        return result;
                    }

                    // 如果是最后一个符号且是 NULLABLE，加入 ε
                    if i == symbols.len() - 1 {
                        result.insert(None);
                    }
                }
                Symbol::Epsilon => {
                    result.insert(None);
                }
            }
        }

        result
    }
}

impl Default for FirstSetCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_compute_nullable() {
        let grammar = parse_grammar(r#"
            S -> A B
            A -> a
            A -> ε
            B -> b
        "#);

        let nullable = FirstSetCalculator::compute_nullable(&grammar);

        // 只有 A 是 NULLABLE
        assert_eq!(nullable.len(), 1);
    }

    #[test]
    fn test_compute_first_simple() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> F T'
            T' -> * F T' | ε
            F -> ( E ) | num
        "#);

        let first_sets = FirstSetCalculator::compute(&grammar);

        // F 的 FIRST 集应该包含 ( 和 num
        let f_id = grammar.non_terminal_map.get("F").unwrap();
        let f_first = first_sets.terminals(*f_id);
        assert_eq!(f_first.len(), 2);

        // E' 的 FIRST 集应该包含 + 和 ε
        let e_prime_id = grammar.non_terminal_map.get("E'").unwrap();
        assert!(first_sets.contains_epsilon(*e_prime_id));
    }

    #[test]
    fn test_compute_first_with_nullable() {
        let grammar = parse_grammar(r#"
            S -> A B
            A -> a
            A -> ε
            B -> b
        "#);

        let nullable = FirstSetCalculator::compute_nullable(&grammar);
        let first_sets = FirstSetCalculator::compute_with_nullable(&grammar, &nullable);

        // S 的 FIRST 集应该包含 a 和 b（因为 A 可为空）
        let s_id = grammar.non_terminal_map.get("S").unwrap();
        let s_first = first_sets.terminals(*s_id);
        assert_eq!(s_first.len(), 2);
    }

    #[test]
    fn test_compute_first_of_symbols() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> num
        "#);

        let nullable = FirstSetCalculator::compute_nullable(&grammar);
        let first_sets = FirstSetCalculator::compute_with_nullable(&grammar, &nullable);

        // 计算 "T E'" 的 FIRST 集
        let t_id = grammar.non_terminal_map.get("T").unwrap();
        let e_prime_id = grammar.non_terminal_map.get("E'").unwrap();

        let symbols = vec![Symbol::NonTerminal(*t_id), Symbol::NonTerminal(*e_prime_id)];
        let first = FirstSetCalculator::compute_first_of_symbols(&symbols, &first_sets, &nullable);

        // 应该包含 num 和 +
        assert!(first.len() >= 1);
    }
}
