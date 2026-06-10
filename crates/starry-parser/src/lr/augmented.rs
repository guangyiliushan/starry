use crate::cfg::{ContextFreeGrammar, NonTerminalId, Production, Symbol};
use std::fmt;

/// 增广文法起始符号前缀
const AUGMENTED_START_PREFIX: &str = "S'";

/// 增广文法
///
/// 为 LR 解析器构建的增广文法，添加新的起始符号 S' -> S
/// 这样可以统一处理接受条件：当 S' -> S· 时接受
#[derive(Debug, Clone)]
pub struct AugmentedGrammar {
    cfg: ContextFreeGrammar,
    augmented_start: NonTerminalId,
    original_start: NonTerminalId,
}

impl AugmentedGrammar {
    /// 从上下文无关文法创建增广文法
    pub fn new(cfg: ContextFreeGrammar) -> Self {
        let original_start = cfg.start_symbol;
        let mut augmented_cfg = cfg;

        let augmented_start_name = Self::generate_augmented_start_name(&augmented_cfg);
        let augmented_start = Self::add_augmented_start(&mut augmented_cfg, augmented_start_name, original_start);

        AugmentedGrammar {
            cfg: augmented_cfg,
            augmented_start,
            original_start,
        }
    }

    /// 从字符串解析并创建增广文法
    pub fn from_str(input: &str) -> Result<Self, String> {
        let cfg = ContextFreeGrammar::parse(input)?;
        Ok(Self::new(cfg))
    }

    /// 生成不冲突的增广起始符号名称
    fn generate_augmented_start_name(cfg: &ContextFreeGrammar) -> String {
        let base_name = AUGMENTED_START_PREFIX.to_string();

        if !cfg.non_terminal_map.contains_key(&base_name) {
            return base_name;
        }

        let mut counter = 1;
        loop {
            let name = format!("{}{}", AUGMENTED_START_PREFIX, counter);
            if !cfg.non_terminal_map.contains_key(&name) {
                return name;
            }
            counter += 1;
        }
    }

    /// 添加增广起始符号和产生式 S' -> S
    fn add_augmented_start(
        cfg: &mut ContextFreeGrammar,
        name: String,
        original_start: NonTerminalId,
    ) -> NonTerminalId {
        let augmented_start = NonTerminalId(cfg.non_terminals.len());
        cfg.non_terminals.push(name.clone());
        cfg.non_terminal_map.insert(name, augmented_start);

        let new_production = Production::new(
            augmented_start,
            vec![Symbol::NonTerminal(original_start)],
        );
        cfg.productions.insert(0, new_production);

        cfg.start_symbol = augmented_start;

        augmented_start
    }

    /// 获取增广起始符号
    pub fn augmented_start(&self) -> NonTerminalId {
        self.augmented_start
    }

    /// 获取原始起始符号
    pub fn original_start(&self) -> NonTerminalId {
        self.original_start
    }

    /// 获取起始产生式索引（始终为 0）
    pub fn start_production_index(&self) -> usize {
        0
    }

    /// 获取底层文法
    pub fn grammar(&self) -> &ContextFreeGrammar {
        &self.cfg
    }

    /// 消耗自身并返回底层文法
    pub fn into_grammar(self) -> ContextFreeGrammar {
        self.cfg
    }

    /// 获取非终结符名称
    pub fn get_non_terminal_name(&self, id: NonTerminalId) -> &str {
        self.cfg.get_non_terminal_name(id)
    }

    /// 获取终结符名称
    pub fn get_terminal_name(&self, id: crate::cfg::TerminalId) -> &str {
        self.cfg.get_terminal_name(id)
    }

    /// 获取所有产生式
    pub fn productions(&self) -> &[Production] {
        &self.cfg.productions
    }

    /// 获取指定索引的产生式
    pub fn get_production(&self, index: usize) -> Option<&Production> {
        self.cfg.productions.get(index)
    }

    /// 判断是否为增广起始符号
    pub fn is_augmented_start(&self, id: NonTerminalId) -> bool {
        id == self.augmented_start
    }

    /// 判断是否为接受产生式（S' -> S）
    pub fn is_accepting_production(&self, production_index: usize) -> bool {
        production_index == 0
    }

    /// 获取所有非终结符
    pub fn non_terminals(&self) -> &[String] {
        &self.cfg.non_terminals
    }

    /// 获取所有终结符
    pub fn terminals(&self) -> &[String] {
        &self.cfg.terminals
    }

    /// 获取终结符映射
    pub fn terminal_map(&self) -> &std::collections::HashMap<String, crate::cfg::TerminalId> {
        &self.cfg.terminal_map
    }

    /// 获取非终结符映射
    pub fn non_terminal_map(&self) -> &std::collections::HashMap<String, NonTerminalId> {
        &self.cfg.non_terminal_map
    }
}

impl std::ops::Deref for AugmentedGrammar {
    type Target = ContextFreeGrammar;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}

impl fmt::Display for AugmentedGrammar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Augmented Grammar:")?;
        writeln!(f, "  Start: {} (augmented)", self.get_non_terminal_name(self.augmented_start))?;
        writeln!(f, "  Original Start: {}", self.get_non_terminal_name(self.original_start))?;
        writeln!(f, "  Productions:")?;

        for (i, production) in self.productions().iter().enumerate() {
            let lhs = self.get_non_terminal_name(production.lhs);
            let rhs: Vec<String> = production.rhs.iter().map(|sym| {
                match sym {
                    Symbol::NonTerminal(id) => self.get_non_terminal_name(*id).to_string(),
                    Symbol::Terminal(id) => self.get_terminal_name(*id).to_string(),
                    Symbol::Epsilon => "ε".to_string(),
                }
            }).collect();
            let marker = if i == 0 { " (augmented)" } else { "" };
            writeln!(f, "    {}: {} -> {}{}", i, lhs, rhs.join(" "), marker)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_augmented_grammar_creation() {
        let cfg = ContextFreeGrammar::parse("E -> E + T | T\nT -> num").unwrap();
        let augmented = AugmentedGrammar::new(cfg);

        assert!(augmented.is_augmented_start(augmented.augmented_start()));
        assert_eq!(augmented.productions().len(), 4);
        assert_eq!(augmented.productions()[0].lhs, augmented.augmented_start());
    }

    #[test]
    fn test_augmented_start_name_generation() {
        let cfg = ContextFreeGrammar::parse("E -> a").unwrap();
        let augmented = AugmentedGrammar::new(cfg);

        assert_eq!(augmented.get_non_terminal_name(augmented.augmented_start()), "S'");
    }

    #[test]
    fn test_augmented_start_name_conflict() {
        let cfg = ContextFreeGrammar::parse("S' -> a\nE -> b").unwrap();
        let augmented = AugmentedGrammar::new(cfg);

        let augmented_start_name = augmented.get_non_terminal_name(augmented.augmented_start());
        assert_ne!(augmented_start_name, "S'");
        assert!(augmented_start_name.starts_with("S'"));
    }

    #[test]
    fn test_accepting_production() {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let augmented = AugmentedGrammar::new(cfg);

        assert!(augmented.is_accepting_production(0));
        assert!(!augmented.is_accepting_production(1));
    }

    #[test]
    fn test_from_str() {
        let augmented = AugmentedGrammar::from_str("E -> num").unwrap();

        assert_eq!(augmented.productions().len(), 2);
        assert!(augmented.is_augmented_start(augmented.augmented_start()));
    }

    #[test]
    fn test_original_start_preserved() {
        let cfg = ContextFreeGrammar::parse("E -> T\nT -> num").unwrap();
        let original_start = cfg.start_symbol;

        let augmented = AugmentedGrammar::new(cfg);

        assert_eq!(augmented.original_start(), original_start);
        assert_ne!(augmented.augmented_start(), augmented.original_start());
    }

    #[test]
    fn test_display() {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let augmented = AugmentedGrammar::new(cfg);

        let display = format!("{}", augmented);
        assert!(display.contains("Augmented Grammar"));
        assert!(display.contains("S'"));
        assert!(display.contains("E -> num"));
    }
}
