use crate::cfg::{ContextFreeGrammar, Symbol};
use crate::lr::item::{LR0Item, LR1Item};
use crate::lr::lookahead::LookaheadCalculator;
use std::collections::HashSet;

/// LR(0) 闭包计算器
pub struct LR0Closure;

impl LR0Closure {
    /// 计算项集的闭包
    ///
    /// 闭包运算：对每个项 [A -> α·Bβ]，添加所有 B 的产生式 [B -> ·γ]
    pub fn compute(items: &HashSet<LR0Item>, cfg: &ContextFreeGrammar) -> HashSet<LR0Item> {
        let mut closure_set = items.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let current_items: Vec<LR0Item> = closure_set.iter().copied().collect();

            for item in current_items {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        for (prod_idx, prod) in cfg.productions.iter().enumerate() {
                            if prod.lhs == *nt_id {
                                let new_item = LR0Item::from_production(prod_idx);
                                if closure_set.insert(new_item) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        closure_set
    }
}

/// LR(1) 闭包计算器
pub struct LR1Closure;

impl LR1Closure {
    /// 计算项集的闭包
    ///
    /// 闭包运算：对每个项 [A -> α·Bβ, a]，添加 [B -> ·γ, b]
    /// 其中 b ∈ FIRST(βa)
    pub fn compute(
        items: &HashSet<LR1Item>,
        cfg: &ContextFreeGrammar,
        calculator: &LookaheadCalculator,
    ) -> HashSet<LR1Item> {
        let mut closure_set = items.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let current_items: Vec<LR1Item> = closure_set.iter().cloned().collect();

            for item in current_items {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        let beta: Vec<Symbol> = production.rhs[item.dot_position + 1..].to_vec();
                        let new_lookaheads = calculator.propagate_lookahead(&beta, item.lookahead);

                        for (prod_idx, prod) in cfg.productions.iter().enumerate() {
                            if prod.lhs == *nt_id {
                                for &lookahead in &new_lookaheads {
                                    let new_item = LR1Item::from_production(prod_idx, lookahead);
                                    if closure_set.insert(new_item) {
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        closure_set
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
    fn test_lr0_closure() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let mut items = HashSet::new();
        items.insert(LR0Item::from_production(0));

        let closure = LR0Closure::compute(&items, augmented.grammar());

        assert!(closure.len() > 1);
    }

    #[test]
    fn test_lr1_closure() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);
        let calculator = LookaheadCalculator::new(augmented.grammar());

        let mut items = HashSet::new();
        items.insert(LR1Item::from_production(0, calculator.end_marker()));

        let closure = LR1Closure::compute(&items, augmented.grammar(), &calculator);

        assert!(closure.len() > 1);
    }
}
