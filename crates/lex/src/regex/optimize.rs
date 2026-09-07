//! HIR 优化器模块
//!
//! 不动点驱动的等价改写：任一规则命中即置 dirty 位，迭代到不动点或
//! 达到 [`MAX_PASSES`]。`optimize` 返回实际趟数，收敛性可观测。
//!
//! # 纪律条款
//!
//! - 方法接收者一律 `&mut self`——`changed` 才能是普通 `bool` 字段，
//!   无需 `Cell`（“共享可变”在本仓库不存在，见项目非目标清单）；
//! - 每条规则以 `fn(&mut self, ...) -> Option<Hir>` 表达：仅 `Some(_)`
//!   置 dirty 位。规则“命中但产出等价结构”时不得置位，否则伪 dirty
//!   会烧完趟数预算并触发收敛断言。

use crate::regex::hir::Hir;

/// 不动点迭代的趟数上限
const MAX_PASSES: usize = 8;

/// HIR 优化器
///
/// 当前启用两类等价改写：
/// - **消除空节点**：序列/选择中的 `Empty` 移除、单元素容器解包
/// - **提取公共前缀**：全序列选择的首元素提取（`abc|abd` → `a(b|d)`）
///
/// 重复规范化不在优化器职责内——唯一规范化点是 `translate` 的
/// `translate_repeat`。
///
/// # 示例
///
/// ```
/// use lex::regex::Optimizer;
/// use lex::regex::hir::Hir;
///
/// let hir = Hir::sequence(vec![
///     Hir::Literal('a'),
///     Hir::Empty,
///     Hir::Literal('b'),
/// ]);
///
/// let (optimized, passes) = Optimizer::new().optimize(&hir);
/// assert_eq!(optimized, Hir::sequence(vec![
///     Hir::Literal('a'),
///     Hir::Literal('b'),
/// ]));
/// assert!(passes >= 1 && passes <= 8);
/// ```
pub struct Optimizer {
    /// 本轮是否有规则命中（dirty 位）
    changed: bool,
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Optimizer {
    /// 创建优化器
    pub fn new() -> Self {
        Self { changed: false }
    }

    /// 优化 HIR，返回（优化结果，实际趟数）
    ///
    /// 迭代到不动点（一轮无规则命中）为止。达到 [`MAX_PASSES`] 仍未
    /// 收敛则 debug 断言失败：真实规则集应在个位数趟内收敛，
    /// “达上限”属于需要修规则而非静默通过的情形。
    pub fn optimize(&mut self, hir: &Hir) -> (Hir, usize) {
        let mut current = hir.clone();

        #[cfg(debug_assertions)]
        let initial_size = node_count(&current);

        for pass in 1..=MAX_PASSES {
            self.changed = false;
            current = self.optimize_expr(current);

            // 每趟不得增大：全部规则都是消除/提取类改写
            #[cfg(debug_assertions)]
            debug_assert!(
                node_count(&current) <= initial_size,
                "optimizer pass {pass} grew the tree"
            );

            if !self.changed {
                return (current, pass);
            }
        }

        debug_assert!(false, "optimizer did not converge in {MAX_PASSES} passes");
        (current, MAX_PASSES)
    }

    /// 按值递归：子树消费式改写，无 clone、无占位节点
    fn optimize_expr(&mut self, hir: Hir) -> Hir {
        match hir {
            Hir::Empty | Hir::Literal(_) | Hir::Class(_) => hir,
            Hir::Sequence(seq) => {
                let seq: Vec<Hir> = seq.into_iter().map(|e| self.optimize_expr(e)).collect();
                self.eliminate_empty_sequence(seq)
            }
            Hir::Choice(choices) => {
                let choices: Vec<Hir> =
                    choices.into_iter().map(|c| self.optimize_expr(c)).collect();
                match self.eliminate_empty_choice(choices) {
                    Hir::Choice(choices) => self.extract_common_prefix(choices),
                    other => other,
                }
            }
            Hir::ZeroOrMore(expr) => Hir::zero_or_more(self.optimize_expr(*expr)),
            Hir::OneOrMore(expr) => Hir::one_or_more(self.optimize_expr(*expr)),
            Hir::ZeroOrOne(expr) => Hir::zero_or_one(self.optimize_expr(*expr)),
            Hir::Repeat { expr, min, max } => Hir::Repeat {
                expr: Box::new(self.optimize_expr(*expr)),
                min,
                max,
            },
        }
    }

    /// 规则：移除序列中的空节点；单元素序列解包
    ///
    /// 返回 `None` 语义由 `changed` 位承载：仅真正发生改写时置位。
    fn eliminate_empty_sequence(&mut self, seq: Vec<Hir>) -> Hir {
        let original_len = seq.len();
        let kept: Vec<Hir> = seq.into_iter().filter(|e| !e.is_empty()).collect();
        let removed = original_len - kept.len();

        match kept.len() {
            0 => {
                self.changed = true;
                Hir::Empty
            }
            // Seq([x]) → x（含 removed == 0 的解包情形）
            1 => {
                self.changed = true;
                kept.into_iter().next().unwrap()
            }
            _ => {
                if removed > 0 {
                    self.changed = true;
                }
                Hir::Sequence(kept)
            }
        }
    }

    /// 规则：单元素选择解包
    ///
    /// 注意：Choice 的 Empty 替代项**不可移除**——`Choice([a, ε])` 去掉
    /// ε 会改变语言（丢失空串匹配）。Choice 不做语言改写，仅解包。
    fn eliminate_empty_choice(&mut self, choices: Vec<Hir>) -> Hir {
        if choices.len() == 1 {
            self.changed = true;
            choices.into_iter().next().unwrap()
        } else {
            Hir::Choice(choices)
        }
    }

    /// 规则：全序列选择提取公共首元素
    ///
    /// `Choice([Seq(a, b), Seq(a, c)])` → `Seq(a, Choice([b, c]))`。
    /// 提取后的新子树本趟不再重访（自底向上），交由外层不动点循环处理。
    fn extract_common_prefix(&mut self, choices: Vec<Hir>) -> Hir {
        if !choices.iter().all(|c| matches!(c, Hir::Sequence(_))) {
            return Hir::Choice(choices);
        }

        let first = match choices.first() {
            Some(Hir::Sequence(seq)) => match seq.first() {
                Some(elem) => elem.clone(),
                None => return Hir::Choice(choices),
            },
            _ => return Hir::Choice(choices),
        };

        let common = choices.iter().all(|c| match c {
            Hir::Sequence(seq) => seq.first() == Some(&first),
            _ => false,
        });
        if !common {
            return Hir::Choice(choices);
        }

        let rests: Vec<Hir> = choices
            .into_iter()
            .map(|c| match c {
                Hir::Sequence(seq) => Hir::sequence(seq.into_iter().skip(1).collect()),
                _ => unreachable!("已检查全部为 Sequence"),
            })
            .collect();

        self.changed = true;
        Hir::sequence(vec![first, Hir::choice(rests)])
    }
}

/// debug 专用：统计 HIR 节点数（含内部节点）
#[cfg(debug_assertions)]
fn node_count(hir: &Hir) -> usize {
    let children = hir.children();
    1 + children.iter().map(|c| node_count(c)).sum::<usize>()
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eliminate_empty_in_sequence() {
        let hir = Hir::sequence(vec![Hir::literal('a'), Hir::Empty, Hir::literal('b')]);
        let (optimized, _) = Optimizer::new().optimize(&hir);

        assert_eq!(
            optimized,
            Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')])
        );
    }

    #[test]
    fn test_sequence_and_choice_collapse() {
        let hir = Hir::sequence(vec![Hir::literal('a')]);
        let (optimized, _) = Optimizer::new().optimize(&hir);
        assert_eq!(optimized, Hir::literal('a'));

        // Choice 的 Empty 替代项是语言的一部分（ε），不可移除
        let hir = Hir::choice(vec![Hir::literal('a'), Hir::Empty]);
        let (optimized, _) = Optimizer::new().optimize(&hir);
        assert_eq!(optimized, hir);
    }

    #[test]
    fn test_simplify_repeat_zero_moves_to_translate() {
        // {0,0} → Empty 是 translate 的规范化职责；优化器对 Repeat
        // 只递归子树，不重复实现该规则
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 0,
            max: Some(0),
        };
        let (optimized, passes) = Optimizer::new().optimize(&hir);
        assert!(matches!(optimized, Hir::Repeat { min: 0, max: Some(0), .. }));
        assert_eq!(passes, 1);
    }

    #[test]
    fn test_extract_prefix_converges_within_budget() {
        // abc|abd|abe：pass1 提 a，pass2 提 b，pass3 不动点
        let hir = Hir::choice(vec![
            Hir::sequence(vec![Hir::literal('a'), Hir::literal('b'), Hir::literal('c')]),
            Hir::sequence(vec![Hir::literal('a'), Hir::literal('b'), Hir::literal('d')]),
            Hir::sequence(vec![Hir::literal('a'), Hir::literal('b'), Hir::literal('e')]),
        ]);

        let (optimized, passes) = Optimizer::new().optimize(&hir);

        // pass1 提 a，pass2 提 b，pass3 不动点。
        // 注意：优化器不做跨层扁平化（那是 Hir::sequence 构造器的职责），
        // 所以提取出的内层序列保持嵌套形态。
        // 用原始变体构造期望值：Hir::sequence 构造器会拍平嵌套序列，
        // 而优化器的消除规则直接产出 Hir::Sequence(kept)，保持嵌套。
        let expected = Hir::Sequence(vec![
            Hir::literal('a'),
            Hir::Sequence(vec![
                Hir::literal('b'),
                Hir::Choice(vec![
                    Hir::literal('c'),
                    Hir::literal('d'),
                    Hir::literal('e'),
                ]),
            ]),
        ]);
        assert_eq!(optimized, expected);
        assert!(passes >= 2 && passes <= MAX_PASSES);
    }

    #[test]
    fn test_optimize_is_idempotent() {
        let hir = Hir::sequence(vec![Hir::Empty, Hir::literal('a'), Hir::Empty]);

        let mut optimizer = Optimizer::new();
        let (once, _) = optimizer.optimize(&hir);
        let (twice, passes) = optimizer.optimize(&once);

        assert_eq!(once, twice);
        assert_eq!(passes, 1, "对已收敛的 HIR 再跑一趟应立即到达不动点");
    }

    #[test]
    fn test_optimize_noop_is_single_pass() {
        let hir = Hir::literal('a');
        let (_, passes) = Optimizer::new().optimize(&hir);
        assert_eq!(passes, 1);
    }
}
