use crate::analysis::{FirstSet, FirstSetCalculator};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use std::collections::HashSet;
use std::fmt;

/// 向前看符号计算器
///
/// 用于计算 LR(1) 闭包中的向前看符号传播
/// 基于 FIRST 集和 NULLABLE 集
#[derive(Debug, Clone)]
pub struct LookaheadCalculator {
    first_sets: FirstSet,
    nullable: HashSet<NonTerminalId>,
    end_marker: TerminalId,
}

impl LookaheadCalculator {
    /// 从上下文无关文法创建向前看符号计算器
    pub fn new(cfg: &ContextFreeGrammar) -> Self {
        let nullable = FirstSetCalculator::compute_nullable(cfg);
        let first_sets = FirstSetCalculator::compute_with_nullable(cfg, &nullable);
        let end_marker = TerminalId(cfg.terminals.len());

        LookaheadCalculator {
            first_sets,
            nullable,
            end_marker,
        }
    }

    /// 获取结束标记（$）
    pub fn end_marker(&self) -> TerminalId {
        self.end_marker
    }

    /// 获取 FIRST 集
    pub fn first_set(&self) -> &FirstSet {
        &self.first_sets
    }

    /// 获取 NULLABLE 集
    pub fn nullable(&self) -> &HashSet<NonTerminalId> {
        &self.nullable
    }

    /// 计算符号串的 FIRST 集（包含 ε）
    ///
    /// 返回 HashSet<Option<TerminalId>>，其中 None 表示 ε
    pub fn compute_first_of_symbols(&self, symbols: &[Symbol]) -> HashSet<Option<TerminalId>> {
        let mut result = HashSet::new();

        for (i, symbol) in symbols.iter().enumerate() {
            match symbol {
                Symbol::Terminal(term_id) => {
                    result.insert(Some(*term_id));
                    return result;
                }
                Symbol::NonTerminal(nt_id) => {
                    for term_opt in self.first_sets.get(*nt_id) {
                        if term_opt.is_some() {
                            result.insert(*term_opt);
                        }
                    }

                    if !self.nullable.contains(nt_id) {
                        return result;
                    }

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

    /// 计算符号串的 FIRST 集（不包含 ε，使用指定的向前看符号替代）
    ///
    /// 如果符号串可以为空，则将 lookahead 加入结果
    pub fn compute_first_of_symbols_with_lookahead(
        &self,
        symbols: &[Symbol],
        lookahead: TerminalId,
    ) -> HashSet<TerminalId> {
        let mut result = HashSet::new();

        for symbol in symbols.iter() {
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

    /// 传播向前看符号
    ///
    /// 计算 FIRST(β) ∪ {lookahead}，其中 β 是圆点后的符号串
    /// 用于 LR(1) 闭包中向前看符号的传播
    pub fn propagate_lookahead(
        &self,
        beta: &[Symbol],
        lookahead: TerminalId,
    ) -> HashSet<TerminalId> {
        let mut result = HashSet::new();

        for symbol in beta.iter() {
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

    /// 判断非终结符是否可为空
    pub fn is_nullable(&self, nt: NonTerminalId) -> bool {
        self.nullable.contains(&nt)
    }

    /// 判断符号串是否可为空
    pub fn is_string_nullable(&self, symbols: &[Symbol]) -> bool {
        symbols.iter().all(|sym| {
            matches!(sym, Symbol::NonTerminal(nt) if self.nullable.contains(nt))
                || matches!(sym, Symbol::Epsilon)
        })
    }
}

/// 计算闭包中的向前看符号
///
/// 便捷函数，委托给 LookaheadCalculator::propagate_lookahead
pub fn compute_closure_lookaheads(
    _cfg: &ContextFreeGrammar,
    calculator: &LookaheadCalculator,
    beta: &[Symbol],
    lookahead: TerminalId,
) -> HashSet<TerminalId> {
    calculator.propagate_lookahead(beta, lookahead)
}

/// 向前看符号集合
///
/// 用于表示 LR(1) 项中的向前看符号集合
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookaheadSet {
    terminals: HashSet<TerminalId>,
}

impl LookaheadSet {
    /// 创建空集合
    pub fn new() -> Self {
        LookaheadSet {
            terminals: HashSet::new(),
        }
    }

    /// 从单个终结符创建集合
    pub fn from_terminal(terminal: TerminalId) -> Self {
        let mut set = Self::new();
        set.terminals.insert(terminal);
        set
    }

    /// 从终结符迭代器创建集合
    pub fn from_terminals(terminals: impl IntoIterator<Item = TerminalId>) -> Self {
        LookaheadSet {
            terminals: terminals.into_iter().collect(),
        }
    }

    /// 创建结束标记集合
    pub fn end_marker(cfg: &ContextFreeGrammar) -> Self {
        Self::from_terminal(TerminalId(cfg.terminals.len()))
    }

    /// 插入终结符，返回是否为新元素
    pub fn insert(&mut self, terminal: TerminalId) -> bool {
        self.terminals.insert(terminal)
    }

    /// 扩展集合，返回是否有变化
    pub fn extend(&mut self, other: &LookaheadSet) -> bool {
        let old_len = self.terminals.len();
        self.terminals.extend(&other.terminals);
        self.terminals.len() > old_len
    }

    /// 检查是否包含指定终结符
    pub fn contains(&self, terminal: TerminalId) -> bool {
        self.terminals.contains(&terminal)
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.terminals.is_empty()
    }

    /// 获取集合大小
    pub fn len(&self) -> usize {
        self.terminals.len()
    }

    /// 遍历终结符
    pub fn iter(&self) -> impl Iterator<Item = TerminalId> + '_ {
        self.terminals.iter().copied()
    }

    /// 获取终结符集合
    pub fn terminals(&self) -> &HashSet<TerminalId> {
        &self.terminals
    }

    /// 并集
    pub fn union(&self, other: &LookaheadSet) -> LookaheadSet {
        LookaheadSet {
            terminals: self.terminals.union(&other.terminals).copied().collect(),
        }
    }

    /// 交集
    pub fn intersection(&self, other: &LookaheadSet) -> LookaheadSet {
        LookaheadSet {
            terminals: self.terminals.intersection(&other.terminals).copied().collect(),
        }
    }

    /// 差集
    pub fn difference(&self, other: &LookaheadSet) -> LookaheadSet {
        LookaheadSet {
            terminals: self.terminals.difference(&other.terminals).copied().collect(),
        }
    }
}

impl Default for LookaheadSet {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<TerminalId> for LookaheadSet {
    fn from_iter<I: IntoIterator<Item = TerminalId>>(iter: I) -> Self {
        LookaheadSet {
            terminals: iter.into_iter().collect(),
        }
    }
}

impl fmt::Display for LookaheadSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut terms: Vec<_> = self.terminals.iter().map(|t| t.0.to_string()).collect();
        terms.sort();
        write!(f, "{{ {} }}", terms.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lookahead_calculator_creation() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let calculator = LookaheadCalculator::new(&grammar);

        assert!(calculator.first_set().terminals(NonTerminalId(0)).len() > 0);
    }

    #[test]
    fn test_compute_first_of_symbols() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let calculator = LookaheadCalculator::new(&grammar);

        let t_id = grammar.non_terminal_map.get("T").unwrap();
        let symbols = vec![Symbol::NonTerminal(*t_id)];
        let first = calculator.compute_first_of_symbols(&symbols);

        assert!(!first.is_empty());
    }

    #[test]
    fn test_propagate_lookahead() {
        let grammar = parse_grammar("E -> T E'\nE' -> + T | ε\nT -> num");
        let calculator = LookaheadCalculator::new(&grammar);

        let t_id = grammar.non_terminal_map.get("T").unwrap();
        let e_prime_id = grammar.non_terminal_map.get("E'").unwrap();

        let beta = vec![Symbol::NonTerminal(*e_prime_id), Symbol::NonTerminal(*t_id)];
        let lookahead = TerminalId(0);

        let result = calculator.propagate_lookahead(&beta, lookahead);

        assert!(!result.is_empty());
    }

    #[test]
    fn test_lookahead_set_operations() {
        let mut set1 = LookaheadSet::from_terminal(TerminalId(0));
        set1.insert(TerminalId(1));

        let mut set2 = LookaheadSet::from_terminal(TerminalId(1));
        set2.insert(TerminalId(2));

        let union = set1.union(&set2);
        assert_eq!(union.len(), 3);

        let intersection = set1.intersection(&set2);
        assert_eq!(intersection.len(), 1);
    }

    #[test]
    fn test_nullable_detection() {
        let grammar = parse_grammar("S -> A B\nA -> a | ε\nB -> b");
        let calculator = LookaheadCalculator::new(&grammar);

        let a_id = grammar.non_terminal_map.get("A").unwrap();
        assert!(calculator.is_nullable(*a_id));

        let b_id = grammar.non_terminal_map.get("B").unwrap();
        assert!(!calculator.is_nullable(*b_id));
    }

    #[test]
    fn test_end_marker() {
        let grammar = parse_grammar("E -> num");
        let calculator = LookaheadCalculator::new(&grammar);

        let end_marker = calculator.end_marker();
        assert_eq!(end_marker, TerminalId(grammar.terminals.len()));
    }

    #[test]
    fn test_display() {
        let mut set = LookaheadSet::new();
        set.insert(TerminalId(0));
        set.insert(TerminalId(2));

        let display = format!("{}", set);
        assert!(display.contains("0"));
        assert!(display.contains("2"));
    }
}
