use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol};
use crate::lr::closure::{LR0Closure, LR1Closure};
use crate::lr::item::{LR0Item, LR1Item};
use crate::lr::lookahead::LookaheadCalculator;
use std::collections::HashSet;
use std::fmt;

/// GOTO 转移结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoResult<T: Eq + std::hash::Hash> {
    pub items: HashSet<T>,
}

impl<T: Eq + std::hash::Hash> GotoResult<T> {
    pub fn new(items: HashSet<T>) -> Self {
        GotoResult { items }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// LR(0) GOTO 计算器
pub struct LR0Goto;

impl LR0Goto {
    /// 计算 GOTO(I, X)
    ///
    /// 对项集 I 中所有圆点后为符号 X 的项，将圆点后移一位，然后计算闭包
    pub fn compute(
        items: &HashSet<LR0Item>,
        cfg: &ContextFreeGrammar,
        symbol: &Symbol,
    ) -> Option<HashSet<LR0Item>> {
        let mut result = HashSet::new();

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(next_sym) = item.next_symbol(production) {
                    if next_sym == symbol {
                        result.insert(item.advance());
                    }
                }
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(LR0Closure::compute(&result, cfg))
        }
    }

    /// 获取项集的所有可能转移符号
    pub fn get_transitions(
        items: &HashSet<LR0Item>,
        cfg: &ContextFreeGrammar,
    ) -> Vec<Symbol> {
        let mut symbols = HashSet::new();

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(next_sym) = item.next_symbol(production) {
                    symbols.insert(next_sym.clone());
                }
            }
        }

        let mut result: Vec<Symbol> = symbols.into_iter().collect();
        result.sort_by(|a, b| {
            match (a, b) {
                (Symbol::Terminal(t1), Symbol::Terminal(t2)) => t1.0.cmp(&t2.0),
                (Symbol::NonTerminal(n1), Symbol::NonTerminal(n2)) => n1.0.cmp(&n2.0),
                (Symbol::Terminal(_), Symbol::NonTerminal(_)) => std::cmp::Ordering::Less,
                (Symbol::NonTerminal(_), Symbol::Terminal(_)) => std::cmp::Ordering::Greater,
                (Symbol::Epsilon, _) => std::cmp::Ordering::Less,
                (_, Symbol::Epsilon) => std::cmp::Ordering::Greater,
            }
        });
        result
    }
}

/// LR(1) GOTO 计算器
pub struct LR1Goto;

impl LR1Goto {
    /// 计算 GOTO(I, X)
    pub fn compute(
        items: &HashSet<LR1Item>,
        cfg: &ContextFreeGrammar,
        calculator: &LookaheadCalculator,
        symbol: &Symbol,
    ) -> Option<HashSet<LR1Item>> {
        let mut result = HashSet::new();

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(next_sym) = item.next_symbol(production) {
                    if next_sym == symbol {
                        result.insert(item.advance());
                    }
                }
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(LR1Closure::compute(&result, cfg, calculator))
        }
    }

    /// 获取项集的所有可能转移符号
    pub fn get_transitions(
        items: &HashSet<LR1Item>,
        cfg: &ContextFreeGrammar,
    ) -> Vec<Symbol> {
        let mut symbols = HashSet::new();

        for item in items {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(next_sym) = item.next_symbol(production) {
                    symbols.insert(next_sym.clone());
                }
            }
        }

        let mut result: Vec<Symbol> = symbols.into_iter().collect();
        result.sort_by(|a, b| {
            match (a, b) {
                (Symbol::Terminal(t1), Symbol::Terminal(t2)) => t1.0.cmp(&t2.0),
                (Symbol::NonTerminal(n1), Symbol::NonTerminal(n2)) => n1.0.cmp(&n2.0),
                (Symbol::Terminal(_), Symbol::NonTerminal(_)) => std::cmp::Ordering::Less,
                (Symbol::NonTerminal(_), Symbol::Terminal(_)) => std::cmp::Ordering::Greater,
                (Symbol::Epsilon, _) => std::cmp::Ordering::Less,
                (_, Symbol::Epsilon) => std::cmp::Ordering::Greater,
            }
        });
        result
    }
}

/// GOTO 表条目
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GotoEntry {
    Shift(ItemSetId),
    Error,
}

/// 项集 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemSetId(pub usize);

/// GOTO 表
///
/// GOTO 表是 goto 函数的表格化形式，供 LR 分析器在归约后
/// 根据栈顶状态和归约得到的非终结符查询下一个状态
#[derive(Debug, Clone)]
pub struct GotoTable {
    entries: Vec<Vec<Option<ItemSetId>>>,
    non_terminal_count: usize,
}

impl GotoTable {
    pub fn new(state_count: usize, non_terminal_count: usize) -> Self {
        GotoTable {
            entries: vec![vec![None; non_terminal_count]; state_count],
            non_terminal_count,
        }
    }

    pub fn get(&self, state: ItemSetId, nt: NonTerminalId) -> Option<ItemSetId> {
        self.entries[state.0].get(nt.0).copied().flatten()
    }

    pub fn set(&mut self, state: ItemSetId, nt: NonTerminalId, next_state: ItemSetId) {
        if nt.0 < self.non_terminal_count {
            self.entries[state.0][nt.0] = Some(next_state);
        }
    }

    pub fn state_count(&self) -> usize {
        self.entries.len()
    }

    pub fn non_terminal_count(&self) -> usize {
        self.non_terminal_count
    }

    pub fn print(&self, non_terminal_names: &[String]) {
        let col_widths = self.calculate_column_widths(non_terminal_names);
        
        println!("GOTO Table:");
        
        print!("{:>w$} |", "State", w = 5);
        for nt in 0..self.non_terminal_count {
            let header = non_terminal_names.get(nt).cloned().unwrap_or_else(|| format!("N{}", nt));
            print!(" {:>w$} |", header, w = col_widths[nt]);
        }
        println!();
        
        print!("{:-<5}|", "");
        for nt in 0..self.non_terminal_count {
            print!("{:-<w$}|", "", w = col_widths[nt] + 2);
        }
        println!();
        
        for (state, row) in self.entries.iter().enumerate() {
            print!("{:>5} |", state);
            for (nt, entry) in row.iter().enumerate() {
                let entry_str = match entry {
                    Some(s) => format!("{}", s.0),
                    None => "".to_string(),
                };
                print!(" {:>w$} |", entry_str, w = col_widths[nt]);
            }
            println!();
        }
    }

    fn calculate_column_widths(&self, non_terminal_names: &[String]) -> Vec<usize> {
        let mut widths = Vec::with_capacity(self.non_terminal_count);
        
        for nt in 0..self.non_terminal_count {
            let header_width = non_terminal_names.get(nt).map(|s| s.len()).unwrap_or(2);
            
            let mut max_entry_width = 1;
            for row in &self.entries {
                if let Some(Some(state)) = row.get(nt) {
                    max_entry_width = max_entry_width.max(state.0.to_string().len());
                }
            }
            
            widths.push(header_width.max(max_entry_width));
        }
        
        widths
    }
}

impl fmt::Display for GotoTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "GOTO Table:")?;
        write!(f, "     ")?;
        for nt in 0..self.non_terminal_count {
            write!(f, "N{}   ", nt)?;
        }
        writeln!(f)?;

        for (state, row) in self.entries.iter().enumerate() {
            write!(f, "{:3}: ", state)?;
            for entry in row {
                match entry {
                    Some(s) => write!(f, "{:4} ", s.0)?,
                    None => write!(f, "     ")?,
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;
    use crate::lr::augmented::AugmentedGrammar;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr0_goto() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let mut items = HashSet::new();
        items.insert(LR0Item::from_production(0));
        let closure = LR0Closure::compute(&items, augmented.grammar());

        let t_id = augmented.grammar().non_terminal_map.get("T").unwrap();
        let goto_result = LR0Goto::compute(&closure, augmented.grammar(), &Symbol::NonTerminal(*t_id));

        assert!(goto_result.is_some());
    }

    #[test]
    fn test_lr0_get_transitions() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let mut items = HashSet::new();
        items.insert(LR0Item::from_production(0));
        let closure = LR0Closure::compute(&items, augmented.grammar());

        let transitions = LR0Goto::get_transitions(&closure, augmented.grammar());

        assert!(!transitions.is_empty());
    }

    #[test]
    fn test_lr1_goto() {
        let grammar = parse_grammar("E -> T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);
        let calculator = LookaheadCalculator::new(augmented.grammar());

        let mut items = HashSet::new();
        items.insert(LR1Item::from_production(0, calculator.end_marker()));
        let closure = LR1Closure::compute(&items, augmented.grammar(), &calculator);

        let t_id = augmented.grammar().non_terminal_map.get("T").unwrap();
        let goto_result = LR1Goto::compute(
            &closure,
            augmented.grammar(),
            &calculator,
            &Symbol::NonTerminal(*t_id),
        );

        assert!(goto_result.is_some());
    }

    #[test]
    fn test_goto_table() {
        let mut table = GotoTable::new(3, 2);

        table.set(ItemSetId(0), NonTerminalId(0), ItemSetId(1));

        assert_eq!(table.get(ItemSetId(0), NonTerminalId(0)), Some(ItemSetId(1)));
        assert_eq!(table.get(ItemSetId(0), NonTerminalId(1)), None);
    }
}
