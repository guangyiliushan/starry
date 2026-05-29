use crate::cfg::ContextFreeGrammar;
use crate::lr::item::{LR0Item, LR1Item};
use crate::lr::lookahead::LookaheadCalculator;
use std::fmt;

pub struct AllLR0Items {
    items: Vec<LR0Item>,
}

impl AllLR0Items {
    pub fn new(items: Vec<LR0Item>) -> Self {
        AllLR0Items { items }
    }

    pub fn build(cfg: &ContextFreeGrammar) -> Self {
        let mut items = Vec::new();
        for (prod_idx, production) in cfg.productions.iter().enumerate() {
            if production.is_epsilon() {
                items.push(LR0Item::new(prod_idx, 0));
                items.push(LR0Item::new(prod_idx, 1));
            } else {
                for dot_pos in 0..=production.rhs.len() {
                    items.push(LR0Item::new(prod_idx, dot_pos));
                }
            }
        }
        AllLR0Items { items }
    }

    pub fn items(&self) -> &[LR0Item] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LR0Item> {
        self.items.iter()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("All LR(0) Items ({} items):", self.len());
        for item in &self.items {
            println!("  {}", item.format_with_grammar(cfg));
        }
    }
}

impl fmt::Display for AllLR0Items {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "All LR(0) Items ({} items):", self.len())?;
        for item in &self.items {
            writeln!(f, "  {}", item)?;
        }
        Ok(())
    }
}

pub struct AllLR1Items {
    items: Vec<LR1Item>,
}

impl AllLR1Items {
    pub fn new(items: Vec<LR1Item>) -> Self {
        AllLR1Items { items }
    }

    pub fn build(cfg: &ContextFreeGrammar, calculator: &LookaheadCalculator) -> Self {
        let mut items = Vec::new();
        let all_terminals: Vec<_> = (0..cfg.terminals.len())
            .map(|i| crate::cfg::TerminalId(i))
            .chain(std::iter::once(calculator.end_marker()))
            .collect();

        for (prod_idx, production) in cfg.productions.iter().enumerate() {
            if production.is_epsilon() {
                for &lookahead in &all_terminals {
                    items.push(LR1Item::new(prod_idx, 0, lookahead));
                    items.push(LR1Item::new(prod_idx, 1, lookahead));
                }
            } else {
                for dot_pos in 0..=production.rhs.len() {
                    for &lookahead in &all_terminals {
                        items.push(LR1Item::new(prod_idx, dot_pos, lookahead));
                    }
                }
            }
        }
        AllLR1Items { items }
    }

    pub fn items(&self) -> &[LR1Item] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LR1Item> {
        self.items.iter()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("All LR(1) Items ({} items):", self.len());
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort_by(|a, b| {
            a.production_index
                .cmp(&b.production_index)
                .then(a.dot_position.cmp(&b.dot_position))
                .then(a.lookahead.0.cmp(&b.lookahead.0))
        });
        for item in items {
            println!("  {}", item.format_with_grammar(cfg));
        }
    }
}

impl fmt::Display for AllLR1Items {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "All LR(1) Items ({} items):", self.len())?;
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort_by(|a, b| {
            a.production_index
                .cmp(&b.production_index)
                .then(a.dot_position.cmp(&b.dot_position))
                .then(a.lookahead.0.cmp(&b.lookahead.0))
        });
        for item in items {
            writeln!(f, "  {}", item)?;
        }
        Ok(())
    }
}
