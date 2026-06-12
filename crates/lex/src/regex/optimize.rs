//! HIR 优化器模块
//!
//! 该模块提供 HIR 的优化功能，包括合并相邻字面量、消除空序列等。
//!
//! # 设计特点
//!
//! - **合并优化**：合并相邻的字面量序列为单字串
//! - **消除优化**：移除无效的空序列和空选择
//! - **简化优化**：简化重复构造
//! - **前缀提取**：提取选择中的公共前缀
//!
//! # 核心类型
//!
//! - [`Optimizer`] - HIR 优化器
//!
//! # 示例
//!
//! ```
//! use lex::regex::Optimizer;
//! use lex::regex::hir::Hir;
//!
//! let hir = Hir::sequence(vec![Hir::Literal('a'), Hir::Literal('b')]);
//! let mut optimizer = Optimizer::new();
//! let optimized = optimizer.optimize(&hir);
//! ```

use crate::regex::hir::Hir;
use crate::transition::CharClass;

// ==================== 优化器 ====================

/// HIR 优化器
///
/// 提供多种 HIR 优化策略，可以单独启用或禁用。
///
/// # 优化策略
///
/// - **合并字面量**：将序列中相邻的字面量合并为字符类
/// - **消除空节点**：移除序列和选择中的空节点
/// - **简化重复**：将无效的重复优化掉
/// - **提取前缀**：在选择中提取公共前缀
///
/// # 示例
///
/// ```
/// # use lex::regex::Optimizer;
/// # use lex::regex::hir::Hir;
///
/// let hir = Hir::sequence(vec![Hir::Literal('a'), Hir::Literal('b')]);
/// let mut optimizer = Optimizer::new();
/// let optimized = optimizer.optimize(&hir);
///
/// println!("{:?}", optimized);
/// ```
pub struct Optimizer {
    /// 是否合并相邻字面量
    pub merge_literals: bool,
    /// 是否消除空节点
    pub eliminate_empty: bool,
    /// 是否简化重复
    pub simplify_repeat: bool,
    /// 是否提取公共前缀
    pub extract_prefix: bool,
}

impl Default for Optimizer {
    fn default() -> Self {
        Self {
            merge_literals: true,
            eliminate_empty: true,
            simplify_repeat: true,
            extract_prefix: true,
        }
    }
}

impl Optimizer {
    /// 创建启用所有优化的优化器
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Optimizer;
    ///
    /// let optimizer = Optimizer::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 优化 HIR
    ///
    /// 应用所有启用的优化策略。
    ///
    /// # 参数
    ///
    /// - `hir` - 要优化的 HIR
    ///
    /// # 返回
    ///
    /// 优化后的 HIR
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Optimizer;
    /// # use lex::regex::hir::Hir;
    ///
    /// let hir = Hir::sequence(vec![
    ///     Hir::Literal('a'),
    ///     Hir::Empty,
    ///     Hir::Literal('b'),
    /// ]);
    ///
    /// let mut optimizer = Optimizer::new();
    /// let optimized = optimizer.optimize(&hir);
    ///
    /// // 应该移除中间的 Empty
    /// assert_eq!(optimized, Hir::sequence(vec![
    ///     Hir::Literal('a'),
    ///     Hir::Literal('b'),
    /// ]));
    /// ```
    pub fn optimize(&mut self, hir: &Hir) -> Hir {
        let mut result = hir.clone();
        
        // 递归优化子节点
        result = self.optimize_recursive(result);
        
        // 先消除空节点（清理结构）
        if self.eliminate_empty {
            result = self.eliminate_empty_nodes(result);
        }
        
        // 再应用独立的优化策略
        if self.merge_literals {
            result = self.merge_adjacent_literals(result);
        }
        
        if self.simplify_repeat {
            result = self.simplify_repeats(result);
        }
        
        if self.extract_prefix {
            result = self.extract_common_prefix(result);
        }
        
        result
    }

    /// 递归优化所有子节点
    fn optimize_recursive(&mut self, mut hir: Hir) -> Hir {
        for child in hir.children_mut() {
            *child = self.optimize_recursive(child.clone());
        }
        hir
    }

    /// 合并相邻的字面量
    ///
    /// 将序列中相邻的多个字面量合并为字符类。
    /// 单个字面量保持不变。
    ///
    /// # 示例
    ///
    /// 输入：`Sequence([Literal('a'), Literal('b'), Literal('c')])`
    /// 输出：`Class([a, b, c])`
    fn merge_adjacent_literals(&self, hir: Hir) -> Hir {
        if let Hir::Sequence(seq) = hir {
            let mut result = Vec::new();
            let mut current_literals = Vec::new();

            for elem in seq {
                match elem {
                    Hir::Literal(c) => {
                        current_literals.push(c);
                    }
                    _ => {
                        // 合并当前的字面量
                        Self::push_literals(&mut result, &mut current_literals);
                        result.push(elem);
                    }
                }
            }

            // 处理剩余的字面量
            Self::push_literals(&mut result, &mut current_literals);

            return if result.len() == 1 {
                result.into_iter().next().unwrap()
            } else {
                Hir::Sequence(result)
            };
        }

        hir
    }

    /// 辅助函数：将字面量列表推入结果
    fn push_literals(result: &mut Vec<Hir>, literals: &mut Vec<char>) {
        if literals.is_empty() {
            return;
        }

        if literals.len() == 1 {
            // 单个字面量保持不变
            result.push(Hir::Literal(literals[0]));
        } else {
            // 多个字面量合并为字符类
            let mut class = CharClass::new();
            for &c in literals.iter() {
                class = class.union(&CharClass::single(c));
            }
            result.push(Hir::Class(class));
        }

        literals.clear();
    }

    /// 消除空节点
    ///
    /// 从序列和选择中移除空节点。
    fn eliminate_empty_nodes(&self, hir: Hir) -> Hir {
        match hir {
            Hir::Sequence(seq) => {
                let filtered: Vec<Hir> = seq.into_iter()
                    .filter(|e| !e.is_empty())
                    .collect();
                
                if filtered.is_empty() {
                    Hir::Empty
                } else if filtered.len() == 1 {
                    filtered.into_iter().next().unwrap()
                } else {
                    Hir::Sequence(filtered)
                }
            }
            Hir::Choice(choices) => {
                let filtered: Vec<Hir> = choices.into_iter()
                    .filter(|e| !e.is_empty())
                    .collect();
                
                if filtered.is_empty() {
                    Hir::Empty
                } else if filtered.len() == 1 {
                    filtered.into_iter().next().unwrap()
                } else {
                    Hir::Choice(filtered)
                }
            }
            other => other,
        }
    }

    /// 简化重复
    ///
    /// 移除无效的重复，例如 `a{0,0}` → `Empty`。
    fn simplify_repeats(&self, hir: Hir) -> Hir {
        match hir {
            Hir::Repeat { expr, min, max } => {
                // {0,0} → Empty
                if min == 0 && max == Some(0) {
                    return Hir::Empty;
                }
                
                // {0,1} → ZeroOrOne
                if min == 0 && max == Some(1) {
                    return Hir::zero_or_one(*expr);
                }
                
                // {1,None} → OneOrMore
                if min == 1 && max.is_none() {
                    return Hir::one_or_more(*expr);
                }
                
                // {0,None} → ZeroOrMore
                if min == 0 && max.is_none() {
                    return Hir::zero_or_more(*expr);
                }
                
                // 如果 min == max，展开为序列（小次数）
                if max == Some(min) && min <= 10 {
                    let mut seq = Vec::with_capacity(min as usize);
                    for _ in 0..min {
                        seq.push(*expr.clone());
                    }
                    return Hir::sequence(seq);
                }
                
                Hir::Repeat { expr, min, max }
            }
            other => other,
        }
    }

    /// 提取公共前缀
    ///
    /// 在选择中提取公共前缀。
    ///
    /// # 示例
    ///
    /// 输入：`Choice([Sequence([a, b]), Sequence([a, c])])`
    /// 输出：`Sequence([a, Choice([b, c])])`
    fn extract_common_prefix(&self, hir: Hir) -> Hir {
        if let Hir::Choice(choices) = hir {
            if choices.is_empty() {
                return Hir::Choice(choices);
            }
            
            // 检查是否所有选择都是序列
            let is_all_sequence = choices.iter().all(|c| matches!(c, Hir::Sequence(_)));
            
            if is_all_sequence {
                let sequences: Vec<&Vec<Hir>> = choices.iter()
                    .map(|c| {
                        if let Hir::Sequence(seq) = c {
                            seq
                        } else {
                            unreachable!()
                        }
                    })
                    .collect();
                
                // 检查是否有公共前缀
                if let Some(first) = sequences.first() {
                    if let Some(first_elem) = first.first() {
                        let is_common = sequences.iter().all(|seq| {
                            seq.first().map_or(false, |e| e == first_elem)
                        });
                        
                        if is_common {
                            // 提取公共前缀
                            let mut rest_choices = Vec::new();
                            for seq in sequences {
                                let rest: Vec<Hir> = seq.iter().skip(1).cloned().collect();
                                rest_choices.push(Hir::sequence(rest));
                            }
                            
                            return Hir::sequence(vec![
                                first_elem.clone(),
                                Hir::choice(rest_choices),
                            ]);
                        }
                    }
                }
            }
            
            return Hir::Choice(choices);
        }
        
        hir
    }
}

// ==================== 便捷函数 ====================

/// 优化 HIR（便捷函数）
///
/// # 参数
///
/// - `hir` - 要优化的 HIR
///
/// # 返回
///
/// 优化后的 HIR
///
/// # 示例
///
/// ```
/// # use lex::regex::optimize;
/// # use lex::regex::hir::Hir;
///
/// let hir = Hir::sequence(vec![
///     Hir::Literal('a'),
///     Hir::Empty,
///     Hir::Literal('b'),
/// ]);
///
/// let optimized = optimize::optimize_hir(&hir);
/// ```
pub fn optimize_hir(hir: &Hir) -> Hir {
    Optimizer::new().optimize(hir)
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eliminate_empty_in_sequence() {
        let hir = Hir::sequence(vec![
            Hir::literal('a'),
            Hir::Empty,
            Hir::literal('b'),
        ]);
        
        let mut optimizer = Optimizer::new();
        optimizer.merge_literals = false; // 禁用合并字面量
        let optimized = optimizer.optimize(&hir);
        
        assert_eq!(optimized, Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
    }

    #[test]
    fn test_simplify_repeat_zero() {
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 0,
            max: Some(0),
        };
        
        let optimized = Optimizer::new().optimize(&hir);
        
        assert_eq!(optimized, Hir::Empty);
    }

    #[test]
    fn test_optimize_with_empty() {
        let hir = Hir::sequence(vec![
            Hir::Empty,
            Hir::literal('a'),
            Hir::Empty,
        ]);
        
        let mut optimizer = Optimizer::new();
        optimizer.merge_literals = false;
        let optimized = optimizer.optimize(&hir);
        
        assert_eq!(optimized, Hir::literal('a'));
    }

    #[test]
    fn test_optimize_noop() {
        let hir = Hir::literal('a');
        let optimized = optimize_hir(&hir);
        
        assert_eq!(optimized, Hir::literal('a'));
    }
}