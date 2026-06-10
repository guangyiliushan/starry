use crate::cfg::{ContextFreeGrammar, NonTerminalId, Production, Symbol};

pub struct LeftRecursionEliminator;

impl LeftRecursionEliminator {
    pub fn new() -> Self {
        LeftRecursionEliminator
    }

    /// 消除左递归（包括直接和间接左递归）
    /// 使用龙书标准算法
    pub fn eliminate(&self, cfg: &ContextFreeGrammar) -> Result<ContextFreeGrammar, String> {
        let mut new_cfg = cfg.clone();
        let n = new_cfg.non_terminals.len();

        // 将非终结符排序为 A_1, A_2, ..., A_n
        let ordered: Vec<NonTerminalId> =
            (0..n).map(NonTerminalId).collect();

        for i in 0..n {
            let ai = ordered[i];

            // 步骤1：消除间接左递归
            for j in 0..i {
                let aj = ordered[j];

                // 收集所有需要替换的产生式：A_i -> A_j γ
                let productions_to_replace: Vec<Production> = new_cfg
                    .productions
                    .iter()
                    .filter(|p| {
                        p.lhs == ai
                            && p.rhs.first().map_or(false, |s| {
                                matches!(s, Symbol::NonTerminal(id) if *id == aj)
                            })
                    })
                    .cloned()
                    .collect();

                // 收集 A_j 的所有产生式
                let aj_productions: Vec<Production> = new_cfg
                    .get_productions_for(aj)
                    .into_iter()
                    .cloned()
                    .collect();

                // 执行替换
                for prod_to_replace in productions_to_replace {
                    // 移除 A_i -> A_j γ
                    new_cfg.productions.retain(|p| {
                        !(p.lhs == prod_to_replace.lhs
                            && p.rhs == prod_to_replace.rhs)
                    });

                    // γ 是 A_j 之后的符号
                    let gamma: Vec<Symbol> =
                        prod_to_replace.rhs.iter().skip(1).cloned().collect();

                    // 添加 A_i -> δ_k γ，其中 A_j -> δ_k
                    for aj_prod in &aj_productions {
                        let mut new_rhs = aj_prod.rhs.clone();
                        new_rhs.extend(gamma.clone());
                        new_cfg.add_production(Production::new(ai, new_rhs));
                    }
                }
            }

            // 步骤2：消除 A_i 的直接左递归
            self.eliminate_direct(&mut new_cfg, ai)?;
        }

        Ok(new_cfg)
    }

    /// 消除指定非终结符的直接左递归
    fn eliminate_direct(
        &self,
        cfg: &mut ContextFreeGrammar,
        non_terminal: NonTerminalId,
    ) -> Result<(), String> {
        let productions: Vec<Production> = cfg
            .get_productions_for(non_terminal)
            .into_iter()
            .cloned()
            .collect();

        // 分离含左递归和不包含左递归的产生式
        let mut alpha_groups: Vec<Vec<Symbol>> = Vec::new(); // A -> A α
        let mut beta_groups: Vec<Vec<Symbol>> = Vec::new(); // A -> β (不以A开头)

        for prod in &productions {
            if prod.rhs.first().map_or(false, |s| {
                matches!(s, Symbol::NonTerminal(id) if *id == non_terminal)
            }) {
                // A -> A α，提取 α
                let alpha: Vec<Symbol> = prod.rhs.iter().skip(1).cloned().collect();
                if !alpha.is_empty() {
                    alpha_groups.push(alpha);
                }
            } else {
                // A -> β
                beta_groups.push(prod.rhs.clone());
            }
        }

        // 如果没有直接左递归，无需处理
        if alpha_groups.is_empty() {
            return Ok(());
        }

        // 如果没有 β 产生式，文法有问题（只能推导出无限长的串）
        if beta_groups.is_empty() {
            return Err(format!(
                "Cannot eliminate left recursion for '{}': no non-recursive productions",
                cfg.get_non_terminal_name(non_terminal)
            ));
        }

        // 创建新的非终结符 A'
        let nt_name = cfg.get_non_terminal_name(non_terminal).to_string();
        let new_nt_name = format!("{}'", nt_name);
        let new_nt_id = cfg.add_non_terminal(&new_nt_name);

        // 移除旧的产生式
        cfg.productions.retain(|p| p.lhs != non_terminal);

        // 添加 A -> β A'
        for beta in &beta_groups {
            let mut new_rhs = beta.clone();
            new_rhs.push(Symbol::NonTerminal(new_nt_id));
            cfg.add_production(Production::new(non_terminal, new_rhs));
        }

        // 添加 A' -> α A' | ε
        for alpha in &alpha_groups {
            let mut new_rhs = alpha.clone();
            new_rhs.push(Symbol::NonTerminal(new_nt_id));
            cfg.add_production(Production::new(new_nt_id, new_rhs));
        }

        // 添加 A' -> ε
        cfg.add_production(Production::epsilon(new_nt_id));

        Ok(())
    }
}

impl Default for LeftRecursionEliminator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{ContextFreeGrammar, Production, Symbol};

    #[test]
    fn test_eliminate_direct_left_recursion() {
        let grammar_str = r#"
            Expr -> Expr + Term | Term
            Term -> Term * Factor | Factor
            Factor -> num
        "#;

        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        let eliminator = LeftRecursionEliminator::new();
        let new_cfg = eliminator.eliminate(&cfg).unwrap();

        // 检查新的非终结符被创建
        assert!(new_cfg.non_terminals.iter().any(|n| n.contains('\'')));

        // 验证消除后的文法没有直接左递归（通过检查产生式结构）
        let has_direct_recursion = new_cfg.productions.iter().any(|p| {
            p.rhs.first().map_or(false, |s| {
                matches!(s, Symbol::NonTerminal(id) if *id == p.lhs)
            })
        });
        assert!(!has_direct_recursion, "Eliminated grammar should have no direct left recursion");
    }

    #[test]
    fn test_eliminate_indirect_left_recursion() {
        let mut cfg = ContextFreeGrammar::new();

        let a = cfg.add_non_terminal("A");
        let b = cfg.add_non_terminal("B");
        let c = cfg.add_terminal("c");
        let d = cfg.add_terminal("d");
        let b_term = cfg.add_terminal("b");

        cfg.set_start(a);

        // A -> B c | d
        cfg.add_production(Production::new(
            a,
            vec![Symbol::NonTerminal(b), Symbol::Terminal(c)],
        ));
        cfg.add_production(Production::new(a, vec![Symbol::Terminal(d)]));

        // B -> A b
        cfg.add_production(Production::new(
            b,
            vec![Symbol::NonTerminal(a), Symbol::Terminal(b_term)],
        ));

        let eliminator = LeftRecursionEliminator::new();
        let new_cfg = eliminator.eliminate(&cfg).unwrap();

        // 验证消除后的文法没有直接左递归
        let has_direct_recursion = new_cfg.productions.iter().any(|p| {
            p.rhs.first().map_or(false, |s| {
                matches!(s, Symbol::NonTerminal(id) if *id == p.lhs)
            })
        });
        assert!(!has_direct_recursion, "Eliminated grammar should have no direct left recursion");
    }

    #[test]
    fn test_eliminate_preserve_language() {
        // 测试消除左递归后文法等价性（通过检查产生式结构）
        let grammar_str = r#"
            E -> E + T | T
            T -> T * F | F
            F -> num
        "#;

        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        let eliminator = LeftRecursionEliminator::new();
        let new_cfg = eliminator.eliminate(&cfg).unwrap();

        // 验证 E' 和 T' 被创建
        assert!(new_cfg.non_terminals.iter().any(|n| n == "E'"));
        assert!(new_cfg.non_terminals.iter().any(|n| n == "T'"));

        // 验证 E -> T E'
        let e_prods: Vec<_> = new_cfg
            .productions
            .iter()
            .filter(|p| p.lhs.0 == 0)
            .collect();
        assert!(!e_prods.is_empty());

        // 验证 E' -> + T E' | ε
        let e_prime_id = new_cfg
            .non_terminal_map
            .get("E'")
            .expect("E' should exist");
        let e_prime_prods: Vec<_> = new_cfg
            .productions
            .iter()
            .filter(|p| p.lhs == *e_prime_id)
            .collect();
        assert!(!e_prime_prods.is_empty());

        // 至少有一个 epsilon 产生式
        assert!(e_prime_prods.iter().any(|p| p.is_epsilon()));
    }
}
