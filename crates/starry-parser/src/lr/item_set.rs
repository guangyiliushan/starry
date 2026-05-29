use crate::cfg::{ContextFreeGrammar, Symbol};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::closure::{LR0Closure, LR1Closure};
use crate::lr::goto::{LR0Goto, LR1Goto};
use crate::lr::item::{LR0Item, LR1Item};
use crate::lr::lookahead::LookaheadCalculator;
use std::collections::HashSet;
use std::fmt;

/// LR(0) 项集：LR(0) 自动机的一个状态
///
/// 项集包含一组 LR(0) 项，通过闭包运算可以扩展为完整的项集
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LR0ItemSet {
    items: HashSet<LR0Item>,
}

impl LR0ItemSet {
    /// 创建空项集
    pub fn new() -> Self {
        LR0ItemSet {
            items: HashSet::new(),
        }
    }

    /// 从一组项创建项集
    pub fn from_items(items: impl IntoIterator<Item = LR0Item>) -> Self {
        LR0ItemSet {
            items: items.into_iter().collect(),
        }
    }

    /// 插入项到项集，返回是否为新项
    pub fn insert(&mut self, item: LR0Item) -> bool {
        self.items.insert(item)
    }

    /// 扩展项集
    pub fn extend(&mut self, items: impl IntoIterator<Item = LR0Item>) {
        self.items.extend(items);
    }

    /// 检查项是否存在于项集
    pub fn contains(&self, item: &LR0Item) -> bool {
        self.items.contains(item)
    }

    /// 获取项集大小
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查项集是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 遍历项
    pub fn iter(&self) -> impl Iterator<Item = &LR0Item> {
        self.items.iter()
    }

    /// 获取所有项
    pub fn items(&self) -> &HashSet<LR0Item> {
        &self.items
    }

    /// 获取核项（圆点不在位置 0 的项，或增广文法的初始产生式）
    pub fn kernel_items(&self) -> Vec<LR0Item> {
        self.items.iter().filter(|item| item.is_kernel()).copied().collect()
    }

    /// 计算项集的闭包（调用 closure.rs 中的实现）
    pub fn closure(&self, cfg: &ContextFreeGrammar) -> LR0ItemSet {
        LR0ItemSet::from_items(LR0Closure::compute(&self.items, cfg))
    }

    /// 计算 GOTO 转移（调用 goto.rs 中的实现）
    pub fn goto(&self, cfg: &ContextFreeGrammar, symbol: &Symbol) -> Option<LR0ItemSet> {
        LR0Goto::compute(&self.items, cfg, symbol).map(LR0ItemSet::from_items)
    }

    /// 获取核心（核项集合）
    pub fn core(&self) -> Vec<LR0Item> {
        self.kernel_items()
    }

    /// 计算核心的哈希值（用于 LALR 同心集合并）
    pub fn core_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let mut core_items: Vec<_> = self.kernel_items();
        core_items.sort();
        core_items.hash(&mut hasher);
        hasher.finish()
    }

    /// 打印项集（带文法信息）
    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(0) ItemSet ({} items):", self.len());
        let mut items: Vec<_> = self.items.iter().copied().collect();
        items.sort();
        for item in items {
            println!("  {}", item.format_with_grammar(cfg));
        }
    }
}

impl Default for LR0ItemSet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LR0ItemSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(0) ItemSet ({} items):", self.len())?;
        let mut items: Vec<_> = self.items.iter().copied().collect();
        items.sort();
        for item in items {
            writeln!(f, "  {}", item)?;
        }
        Ok(())
    }
}

/// LR(1) 项集：LR(1) 自动机的一个状态
///
/// 项集包含一组 LR(1) 项，通过闭包运算可以扩展为完整的项集
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LR1ItemSet {
    items: HashSet<LR1Item>,
}

impl LR1ItemSet {
    /// 创建空项集
    pub fn new() -> Self {
        LR1ItemSet {
            items: HashSet::new(),
        }
    }

    /// 从一组项创建项集
    pub fn from_items(items: impl IntoIterator<Item = LR1Item>) -> Self {
        LR1ItemSet {
            items: items.into_iter().collect(),
        }
    }

    /// 插入项到项集，返回是否为新项
    pub fn insert(&mut self, item: LR1Item) -> bool {
        self.items.insert(item)
    }

    /// 扩展项集
    pub fn extend(&mut self, items: impl IntoIterator<Item = LR1Item>) {
        self.items.extend(items);
    }

    /// 检查项是否存在于项集
    pub fn contains(&self, item: &LR1Item) -> bool {
        self.items.contains(item)
    }

    /// 获取项集大小
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查项集是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 遍历项
    pub fn iter(&self) -> impl Iterator<Item = &LR1Item> {
        self.items.iter()
    }

    /// 获取所有项
    pub fn items(&self) -> &HashSet<LR1Item> {
        &self.items
    }

    /// 获取核项
    pub fn kernel_items(&self) -> Vec<LR1Item> {
        self.items.iter().filter(|item| item.is_kernel()).cloned().collect()
    }

    /// 计算项集的闭包（调用 closure.rs 中的实现）
    pub fn closure(
        &self,
        cfg: &ContextFreeGrammar,
        calculator: &LookaheadCalculator,
    ) -> LR1ItemSet {
        LR1ItemSet::from_items(LR1Closure::compute(&self.items, cfg, calculator))
    }

    /// 计算 GOTO 转移（调用 goto.rs 中的实现）
    pub fn goto(
        &self,
        cfg: &ContextFreeGrammar,
        calculator: &LookaheadCalculator,
        symbol: &Symbol,
    ) -> Option<LR1ItemSet> {
        LR1Goto::compute(&self.items, cfg, calculator, symbol).map(LR1ItemSet::from_items)
    }

    /// 提取核心（忽略向前看符号）
    pub fn core(&self) -> LR0ItemSet {
        let mut core_set = LR0ItemSet::new();
        for item in self.kernel_items() {
            core_set.insert(item.core());
        }
        core_set
    }

    /// 合并向前看符号（用于 LALR 同心集合并）
    pub fn merge_lookaheads(&mut self, other: &LR1ItemSet) -> bool {
        let mut changed = false;

        for other_item in &other.items {
            let core = other_item.core();
            let mut found_match = false;

            for self_item in &self.items {
                if self_item.core() == core {
                    found_match = true;
                    if self_item.lookahead != other_item.lookahead {
                        let merged = LR1Item::new(
                            other_item.production_index,
                            other_item.dot_position,
                            other_item.lookahead,
                        );
                        if !self.items.contains(&merged) {
                            self.items.insert(merged);
                            changed = true;
                        }
                    }
                    break;
                }
            }

            if !found_match {
                self.items.insert(other_item.clone());
                changed = true;
            }
        }

        changed
    }

    /// 打印项集（带文法信息）
    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(1) ItemSet ({} items):", self.len());
        let mut items: Vec<_> = self.items.iter().cloned().collect();
        items.sort_by(|a, b| {
            a.production_index.cmp(&b.production_index)
                .then(a.dot_position.cmp(&b.dot_position))
                .then(a.lookahead.0.cmp(&b.lookahead.0))
        });
        for item in items {
            println!("  {}", item.format_with_grammar(cfg));
        }
    }
}

impl Default for LR1ItemSet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LR1ItemSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(1) ItemSet ({} items):", self.len())?;
        let mut items: Vec<_> = self.items.iter().cloned().collect();
        items.sort_by(|a, b| {
            a.production_index.cmp(&b.production_index)
                .then(a.dot_position.cmp(&b.dot_position))
                .then(a.lookahead.0.cmp(&b.lookahead.0))
        });
        for item in items {
            writeln!(f, "  {}", item)?;
        }
        Ok(())
    }
}

/// 创建初始 LR(0) 项集
pub fn create_initial_lr0_item_set(augmented: &AugmentedGrammar) -> LR0ItemSet {
    let mut item_set = LR0ItemSet::new();
    item_set.insert(LR0Item::from_production(0));
    item_set.closure(augmented.grammar())
}

/// 创建初始 LR(1) 项集
pub fn create_initial_lr1_item_set(
    augmented: &AugmentedGrammar,
    calculator: &LookaheadCalculator,
) -> LR1ItemSet {
    let end_marker = calculator.end_marker();
    let mut item_set = LR1ItemSet::new();
    item_set.insert(LR1Item::from_production(0, end_marker));
    item_set.closure(augmented.grammar(), calculator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::TerminalId;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr0_item_set_creation() {
        let mut set = LR0ItemSet::new();
        set.insert(LR0Item::new(0, 0));

        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_lr0_item_set_closure() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let initial = create_initial_lr0_item_set(&augmented);

        assert!(initial.len() > 1);
    }

    #[test]
    fn test_lr0_item_set_goto() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let initial = create_initial_lr0_item_set(&augmented);

        let t_id = augmented.grammar().non_terminal_map.get("T").unwrap();
        let goto_set = initial.goto(augmented.grammar(), &Symbol::NonTerminal(*t_id));

        assert!(goto_set.is_some());
    }

    #[test]
    fn test_lr1_item_set_creation() {
        let mut set = LR1ItemSet::new();
        set.insert(LR1Item::new(0, 0, TerminalId(0)));

        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_lr1_item_set_closure() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);
        let calculator = LookaheadCalculator::new(augmented.grammar());

        let initial = create_initial_lr1_item_set(&augmented, &calculator);

        assert!(initial.len() > 1);
    }

    #[test]
    fn test_lr1_item_set_goto() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);
        let calculator = LookaheadCalculator::new(augmented.grammar());

        let initial = create_initial_lr1_item_set(&augmented, &calculator);

        let t_id = augmented.grammar().non_terminal_map.get("T").unwrap();
        let goto_set = initial.goto(
            augmented.grammar(),
            &calculator,
            &Symbol::NonTerminal(*t_id),
        );

        assert!(goto_set.is_some());
    }

    #[test]
    fn test_kernel_items() {
        let mut set = LR0ItemSet::new();
        set.insert(LR0Item::new(0, 0));
        set.insert(LR0Item::new(1, 0));
        set.insert(LR0Item::new(1, 1));

        let kernel = set.kernel_items();

        assert_eq!(kernel.len(), 2);
    }

    #[test]
    fn test_core_extraction() {
        let mut lr1_set = LR1ItemSet::new();
        lr1_set.insert(LR1Item::new(0, 0, TerminalId(0)));
        lr1_set.insert(LR1Item::new(0, 0, TerminalId(1)));

        let core = lr1_set.core();

        assert_eq!(core.len(), 1);
    }

    #[test]
    fn test_display() {
        let mut set = LR0ItemSet::new();
        set.insert(LR0Item::new(0, 0));
        set.insert(LR0Item::new(1, 1));

        let display = format!("{}", set);
        assert!(display.contains("LR(0) ItemSet"));
        assert!(display.contains("[0 · 0]"));
        assert!(display.contains("[1 · 1]"));
    }
}
