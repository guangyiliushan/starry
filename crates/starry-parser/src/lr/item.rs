use crate::cfg::{ContextFreeGrammar, Production, Symbol, TerminalId};
use std::fmt;

/// LR(0) 项：产生式 + 圆点位置
///
/// 表示形式：[A -> α·β]，其中 production_index 指向产生式 A -> αβ
/// dot_position 表示圆点在右部符号序列中的位置
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LR0Item {
    pub production_index: usize,
    pub dot_position: usize,
}

impl LR0Item {
    /// 创建一个新的 LR(0) 项
    pub fn new(production_index: usize, dot_position: usize) -> Self {
        LR0Item {
            production_index,
            dot_position,
        }
    }

    /// 从产生式创建初始项（圆点在位置 0）
    pub fn from_production(production_index: usize) -> Self {
        LR0Item {
            production_index,
            dot_position: 0,
        }
    }

    /// 判断是否为核项
    /// 核项定义：圆点不在位置 0（除非它是增广文法的初始产生式）
    pub fn is_kernel(&self) -> bool {
        self.dot_position > 0 || self.production_index == 0
    }

    /// 判断项是否完成（圆点在产生式末尾）
    pub fn is_complete(&self, production: &Production) -> bool {
        if production.is_epsilon() {
            return self.dot_position == 1;
        }
        self.dot_position >= production.rhs.len()
    }

    /// 获取圆点后的下一个符号
    pub fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol> {
        if production.is_epsilon() {
            if self.dot_position == 0 {
                return Some(&Symbol::Epsilon);
            }
            return None;
        }
        production.rhs.get(self.dot_position)
    }

    /// 将圆点向前移动一位
    pub fn advance(&self) -> Self {
        LR0Item {
            production_index: self.production_index,
            dot_position: self.dot_position + 1,
        }
    }

    /// 获取核心（LR(0) 项的核心就是自身）
    pub fn core(&self) -> Self {
        *self
    }

    /// 格式化项为可读字符串（带文法信息）
    pub fn format_with_grammar(&self, cfg: &ContextFreeGrammar) -> String {
        if let Some(production) = cfg.productions.get(self.production_index) {
            let lhs_name = cfg.get_non_terminal_name(production.lhs);
            format!("[{} -> {}]", lhs_name, self.format_rhs_with_dot(production, cfg))
        } else {
            format!("[{} · {}]", self.production_index, self.dot_position)
        }
    }

    /// 打印项
    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("{}", self.format_with_grammar(cfg));
    }

    /// 格式化右部并添加圆点
    fn format_rhs_with_dot(&self, production: &Production, cfg: &ContextFreeGrammar) -> String {
        if production.is_epsilon() {
            if self.dot_position == 0 {
                "· ε".to_string()
            } else {
                "ε ·".to_string()
            }
        } else {
            let parts: Vec<String> = production
                .rhs
                .iter()
                .enumerate()
                .map(|(i, sym)| {
                    let sym_str = match sym {
                        Symbol::NonTerminal(id) => cfg.get_non_terminal_name(*id).to_string(),
                        Symbol::Terminal(id) => cfg.get_terminal_name(*id).to_string(),
                        Symbol::Epsilon => "ε".to_string(),
                    };
                    if i == self.dot_position {
                        format!("· {}", sym_str)
                    } else {
                        sym_str
                    }
                })
                .collect();

            if self.dot_position >= production.rhs.len() {
                format!("{} ·", parts.join(" "))
            } else {
                parts.join(" ")
            }
        }
    }
}

impl fmt::Display for LR0Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} · {}]", self.production_index, self.dot_position)
    }
}

/// LR(1) 项：产生式 + 圆点位置 + 向前看符号
///
/// 表示形式：[A -> α·β, a]，其中 a 是向前看终结符
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LR1Item {
    pub production_index: usize,
    pub dot_position: usize,
    pub lookahead: TerminalId,
}

impl LR1Item {
    /// 创建一个新的 LR(1) 项
    pub fn new(production_index: usize, dot_position: usize, lookahead: TerminalId) -> Self {
        LR1Item {
            production_index,
            dot_position,
            lookahead,
        }
    }

    /// 从产生式和向前看符号创建初始项
    pub fn from_production(production_index: usize, lookahead: TerminalId) -> Self {
        LR1Item {
            production_index,
            dot_position: 0,
            lookahead,
        }
    }

    /// 判断是否为核项
    pub fn is_kernel(&self) -> bool {
        self.dot_position > 0 || self.production_index == 0
    }

    /// 判断项是否完成
    pub fn is_complete(&self, production: &Production) -> bool {
        if production.is_epsilon() {
            return self.dot_position == 1;
        }
        self.dot_position >= production.rhs.len()
    }

    /// 获取圆点后的下一个符号
    pub fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol> {
        if production.is_epsilon() {
            if self.dot_position == 0 {
                return Some(&Symbol::Epsilon);
            }
            return None;
        }
        production.rhs.get(self.dot_position)
    }

    /// 将圆点向前移动一位（保持向前看符号不变）
    pub fn advance(&self) -> Self {
        LR1Item {
            production_index: self.production_index,
            dot_position: self.dot_position + 1,
            lookahead: self.lookahead,
        }
    }

    /// 提取 LR(0) 核心（忽略向前看符号）
    pub fn core(&self) -> LR0Item {
        LR0Item::new(self.production_index, self.dot_position)
    }

    /// 创建具有不同向前看符号的新项
    pub fn with_lookahead(&self, lookahead: TerminalId) -> Self {
        LR1Item {
            production_index: self.production_index,
            dot_position: self.dot_position,
            lookahead,
        }
    }

    /// 格式化项为可读字符串（带文法信息）
    pub fn format_with_grammar(&self, cfg: &ContextFreeGrammar) -> String {
        if let Some(production) = cfg.productions.get(self.production_index) {
            let lhs_name = cfg.get_non_terminal_name(production.lhs);
            let lookahead_name = cfg.get_terminal_name(self.lookahead);
            let rhs_str = self.format_rhs_with_dot(production, cfg);
            format!("[{} -> {}, {}]", lhs_name, rhs_str, lookahead_name)
        } else {
            format!("[{} · {}, {}]", self.production_index, self.dot_position, self.lookahead.0)
        }
    }

    /// 打印项（带文法信息）
    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("{}", self.format_with_grammar(cfg));
    }

    /// 格式化右部并添加圆点
    fn format_rhs_with_dot(&self, production: &Production, cfg: &ContextFreeGrammar) -> String {
        if production.is_epsilon() {
            if self.dot_position == 0 {
                "· ε".to_string()
            } else {
                "ε ·".to_string()
            }
        } else {
            let parts: Vec<String> = production
                .rhs
                .iter()
                .enumerate()
                .map(|(i, sym)| {
                    let sym_str = match sym {
                        Symbol::NonTerminal(id) => cfg.get_non_terminal_name(*id).to_string(),
                        Symbol::Terminal(id) => cfg.get_terminal_name(*id).to_string(),
                        Symbol::Epsilon => "ε".to_string(),
                    };
                    if i == self.dot_position {
                        format!("· {}", sym_str)
                    } else {
                        sym_str
                    }
                })
                .collect();

            if self.dot_position >= production.rhs.len() {
                format!("{} ·", parts.join(" "))
            } else {
                parts.join(" ")
            }
        }
    }
}

impl fmt::Display for LR1Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} · {}, {}]",
            self.production_index, self.dot_position, self.lookahead.0
        )
    }
}

/// 统一的项类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    LR0(LR0Item),
    LR1(LR1Item),
}

impl Item {
    pub fn production_index(&self) -> usize {
        match self {
            Item::LR0(item) => item.production_index,
            Item::LR1(item) => item.production_index,
        }
    }

    pub fn dot_position(&self) -> usize {
        match self {
            Item::LR0(item) => item.dot_position,
            Item::LR1(item) => item.dot_position,
        }
    }

    pub fn is_kernel(&self) -> bool {
        match self {
            Item::LR0(item) => item.is_kernel(),
            Item::LR1(item) => item.is_kernel(),
        }
    }

    pub fn is_complete(&self, production: &Production) -> bool {
        match self {
            Item::LR0(item) => item.is_complete(production),
            Item::LR1(item) => item.is_complete(production),
        }
    }

    pub fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol> {
        match self {
            Item::LR0(item) => item.next_symbol(production),
            Item::LR1(item) => item.next_symbol(production),
        }
    }

    pub fn advance(&self) -> Self {
        match self {
            Item::LR0(item) => Item::LR0(item.advance()),
            Item::LR1(item) => Item::LR1(item.advance()),
        }
    }

    pub fn core(&self) -> LR0Item {
        match self {
            Item::LR0(item) => *item,
            Item::LR1(item) => item.core(),
        }
    }

    pub fn lookahead(&self) -> Option<TerminalId> {
        match self {
            Item::LR0(_) => None,
            Item::LR1(item) => Some(item.lookahead),
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Item::LR0(item) => write!(f, "{}", item),
            Item::LR1(item) => write!(f, "{}", item),
        }
    }
}

/// 项类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    LR0,
    LR1,
}

/// 项的统一 trait 接口
pub trait ItemTrait: Clone + PartialEq + Eq + std::hash::Hash + std::fmt::Debug {
    fn production_index(&self) -> usize;
    fn dot_position(&self) -> usize;
    fn is_kernel(&self) -> bool;
    fn is_complete(&self, production: &Production) -> bool;
    fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol>;
    fn advance(&self) -> Self;
    fn core(&self) -> LR0Item;
    fn lookahead(&self) -> Option<TerminalId>;
    fn kind() -> ItemKind;
}

impl ItemTrait for LR0Item {
    fn production_index(&self) -> usize {
        self.production_index
    }

    fn dot_position(&self) -> usize {
        self.dot_position
    }

    fn is_kernel(&self) -> bool {
        self.is_kernel()
    }

    fn is_complete(&self, production: &Production) -> bool {
        self.is_complete(production)
    }

    fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol> {
        self.next_symbol(production)
    }

    fn advance(&self) -> Self {
        self.advance()
    }

    fn core(&self) -> LR0Item {
        self.core()
    }

    fn lookahead(&self) -> Option<TerminalId> {
        None
    }

    fn kind() -> ItemKind {
        ItemKind::LR0
    }
}

impl ItemTrait for LR1Item {
    fn production_index(&self) -> usize {
        self.production_index
    }

    fn dot_position(&self) -> usize {
        self.dot_position
    }

    fn is_kernel(&self) -> bool {
        self.is_kernel()
    }

    fn is_complete(&self, production: &Production) -> bool {
        self.is_complete(production)
    }

    fn next_symbol<'a>(&self, production: &'a Production) -> Option<&'a Symbol> {
        self.next_symbol(production)
    }

    fn advance(&self) -> Self {
        self.advance()
    }

    fn core(&self) -> LR0Item {
        self.core()
    }

    fn lookahead(&self) -> Option<TerminalId> {
        Some(self.lookahead)
    }

    fn kind() -> ItemKind {
        ItemKind::LR1
    }
}

/// 格式化项为可读字符串
///
/// 示例输出：`[E -> E · + T, $]`
pub fn format_item(
    item: &Item,
    production: &Production,
    lhs_name: &str,
    terminal_names: &[String],
) -> String {
    let dot_pos = item.dot_position();
    let lookahead_str = match item {
        Item::LR1(item) => format!(", {}", terminal_names[item.lookahead.0]),
        _ => String::new(),
    };

    let rhs_str = if production.is_epsilon() {
        if dot_pos == 0 {
            "· ε".to_string()
        } else {
            "ε ·".to_string()
        }
    } else {
        let parts: Vec<String> = production
            .rhs
            .iter()
            .enumerate()
            .map(|(i, sym)| {
                let sym_str = match sym {
                    Symbol::NonTerminal(id) => format!("N{}", id.0),
                    Symbol::Terminal(id) => terminal_names[id.0].clone(),
                    Symbol::Epsilon => "ε".to_string(),
                };
                if i == dot_pos {
                    format!("· {}", sym_str)
                } else {
                    sym_str
                }
            })
            .collect();

        if dot_pos >= production.rhs.len() {
            format!("{} ·", parts.join(" "))
        } else {
            parts.join(" ")
        }
    };

    format!("[{} -> {}{}]", lhs_name, rhs_str, lookahead_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::NonTerminalId;

    fn make_production() -> Production {
        let lhs_id = NonTerminalId(0);
        let rhs_symbols = vec![
            Symbol::NonTerminal(NonTerminalId(0)),
            Symbol::Terminal(TerminalId(0)),
            Symbol::NonTerminal(NonTerminalId(1)),
        ];
        Production::new(lhs_id, rhs_symbols)
    }

    #[test]
    fn test_lr0_item_creation() {
        let item = LR0Item::new(0, 0);
        assert_eq!(item.production_index, 0);
        assert_eq!(item.dot_position, 0);
        assert!(item.is_kernel());
    }

    #[test]
    fn test_lr0_item_advance() {
        let item = LR0Item::new(0, 0);
        let advanced = item.advance();
        assert_eq!(advanced.dot_position, 1);
    }

    #[test]
    fn test_lr0_item_next_symbol() {
        let prod = make_production();
        let item = LR0Item::new(0, 0);
        let next = item.next_symbol(&prod);
        assert!(next.is_some());
    }

    #[test]
    fn test_lr0_item_complete() {
        let prod = make_production();
        let item = LR0Item::new(0, 3);
        assert!(item.is_complete(&prod));
    }

    #[test]
    fn test_lr1_item_creation() {
        let lookahead = TerminalId(0);
        let item = LR1Item::new(0, 0, lookahead);
        assert_eq!(item.production_index, 0);
        assert_eq!(item.dot_position, 0);
        assert_eq!(item.lookahead, lookahead);
    }

    #[test]
    fn test_lr1_item_core() {
        let lookahead = TerminalId(0);
        let item = LR1Item::new(0, 2, lookahead);
        let core = item.core();
        assert_eq!(core.production_index, 0);
        assert_eq!(core.dot_position, 2);
    }

    #[test]
    fn test_lr1_item_advance() {
        let lookahead = TerminalId(0);
        let item = LR1Item::new(0, 0, lookahead);
        let advanced = item.advance();
        assert_eq!(advanced.dot_position, 1);
        assert_eq!(advanced.lookahead, lookahead);
    }

    #[test]
    fn test_item_enum() {
        let lr0 = Item::LR0(LR0Item::new(0, 0));
        let lr1 = Item::LR1(LR1Item::new(0, 0, TerminalId(0)));

        assert_eq!(lr0.production_index(), 0);
        assert_eq!(lr1.production_index(), 0);
        assert!(lr1.lookahead().is_some());
        assert!(lr0.lookahead().is_none());
    }

    #[test]
    fn test_kernel_item() {
        let item = LR0Item::new(0, 0);
        assert!(item.is_kernel());

        let item2 = LR0Item::new(1, 0);
        assert!(!item2.is_kernel());

        let item3 = LR0Item::new(1, 1);
        assert!(item3.is_kernel());
    }

    #[test]
    fn test_epsilon_item() {
        let prod = Production::epsilon(NonTerminalId(0));
        let item = LR0Item::new(0, 0);

        assert!(!item.is_complete(&prod));

        let advanced = item.advance();
        assert!(advanced.is_complete(&prod));
    }

    #[test]
    fn test_display() {
        let item = LR0Item::new(0, 1);
        assert_eq!(format!("{}", item), "[0 · 1]");

        let item1 = LR1Item::new(0, 1, TerminalId(2));
        assert_eq!(format!("{}", item1), "[0 · 1, 2]");
    }
}
