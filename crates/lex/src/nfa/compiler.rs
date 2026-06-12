//! NFA Compiler 模块
//!
//! 实现 Thompson 构造算法，将 HIR（高级中间表示）编译为 NFA 片段。
//!
//! # Thompson 构造
//!
//! 每个 HIR 变体对应一个 NFA 片段（Fragment），片段有唯一的 start 和 end 状态。
//! 通过组合这些片段构建完整的 NFA。
//!
//! # 设计特点
//!
//! - **Compiler 拥有 Builder**：通过 `into_builder()` 回收 Builder
//! - **递归构造**：递归处理子 HIR，返回 Fragment
//! - **全局状态 ID**：所有片段共享同一个 Builder 的状态计数器

use crate::regex::hir::Hir;
use crate::state::StateId;
use crate::transition::Transition;
use super::builder::Builder;

/// 编译结果：一个 NFA 片段
///
/// 片段包含唯一的起始状态和结束状态。
#[derive(Debug, Clone, Copy)]
pub struct Fragment {
    /// 起始状态
    pub start: StateId,
    /// 结束状态
    pub end: StateId,
}

impl Fragment {
    /// 创建新的片段
    pub fn new(start: StateId, end: StateId) -> Self {
        Self { start, end }
    }
}

/// NFA Compiler — Thompson 构造算法
///
/// 将 HIR 编译为 NFA 的编译器。
///
/// # 设计理念
///
/// - **拥有 Builder**：Compiler 持有 Builder 所有权，通过 `into_builder()` 可回收
/// - **递归构造**：`compile_hir()` 递归处理 HIR 树
///
/// # 示例
///
/// ```
/// # use lex::regex::hir::Hir;
/// # use lex::nfa::Compiler;
/// let hir = Hir::literal('a');
/// let mut compiler = Compiler::new();
/// let frag = compiler.compile_hir(&hir);
/// assert_ne!(frag.start, frag.end);
/// ```
#[derive(Debug)]
pub struct Compiler {
    builder: Builder,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// 创建新的 Compiler
    ///
    /// 内部创建空的 Builder。
    pub fn new() -> Self {
        Self {
            builder: Builder::new(),
        }
    }

    /// 从已有的 Builder 创建 Compiler
    ///
    /// 用于在已有 Builder 上继续构建（例如组合多个 NFA）。
    pub fn with_builder(builder: Builder) -> Self {
        Self { builder }
    }

    /// 消费 Compiler，返回 Builder
    ///
    /// 用于在编译完成后回收状态管理器。
    pub fn into_builder(self) -> Builder {
        self.builder
    }

    /// 获取 Builder 的不可变引用
    pub fn builder(&self) -> &Builder {
        &self.builder
    }

    // ==================== 核心编译方法 ====================

    /// 编译 HIR 为 NFA 片段
    ///
    /// 递归处理 HIR 树，返回包含 start 和 end 的 Fragment。
    ///
    /// # 参数
    ///
    /// - `hir` - 要编译的 HIR
    pub fn compile_hir(&mut self, hir: &Hir) -> Fragment {
        match hir {
            Hir::Empty => self.compile_empty(),
            Hir::Literal(c) => self.compile_literal(*c),
            Hir::Class(class) => self.compile_class(class),
            Hir::Sequence(seq) => self.compile_sequence(seq),
            Hir::Choice(choices) => self.compile_choice(choices),
            Hir::ZeroOrMore(expr) => self.compile_zero_or_more(expr),
            Hir::OneOrMore(expr) => self.compile_one_or_more(expr),
            Hir::ZeroOrOne(expr) => self.compile_zero_or_one(expr),
            Hir::Repeat { expr, min, max } => self.compile_repeat(expr, *min, *max),
        }
    }

    // ==================== 原子构造 ====================

    /// 空串：start --ε→ end
    fn compile_empty(&mut self) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        self.builder.add_epsilon(s, e);
        Fragment::new(s, e)
    }

    /// 字面量：start --c→ end
    fn compile_literal(&mut self, c: char) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        self.builder.add_char(s, c, e);
        Fragment::new(s, e)
    }

    /// 字符类：start --class→ end
    fn compile_class(&mut self, class: &crate::transition::CharClass) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        self.builder.add_edge(s, Transition::char_class(class.clone()), e);
        Fragment::new(s, e)
    }

    // ==================== 复合构造 ====================

    /// 序列：A · B · C
    ///
    /// 实现：A.end --ε→ B.start, B.end --ε→ C.start
    /// 返回：Fragment(A.start, C.end)
    fn compile_sequence(&mut self, hirs: &[Hir]) -> Fragment {
        if hirs.is_empty() {
            return self.compile_empty();
        }

        let frags: Vec<Fragment> = hirs.iter()
            .map(|h| self.compile_hir(h))
            .collect();

        // 用 epsilon 连接相邻片段
        for i in 1..frags.len() {
            self.builder.add_epsilon(frags[i - 1].end, frags[i].start);
        }

        Fragment::new(
            frags.first().unwrap().start,
            frags.last().unwrap().end,
        )
    }

    /// 选择：A | B | C
    ///
    /// 实现：
    ///   new_start --ε→ A.start
    ///   new_start --ε→ B.start
    ///   new_start --ε→ C.start
    ///   A.end --ε→ new_end
    ///   B.end --ε→ new_end
    ///   C.end --ε→ new_end
    fn compile_choice(&mut self, hirs: &[Hir]) -> Fragment {
        if hirs.is_empty() {
            return self.compile_empty();
        }

        let s = self.builder.add_state();  // 新的起始状态
        let e = self.builder.add_state();  // 新的结束状态

        for hir in hirs {
            let frag = self.compile_hir(hir);
            self.builder.add_epsilon(s, frag.start);
            self.builder.add_epsilon(frag.end, e);
        }

        Fragment::new(s, e)
    }

    // ==================== 重复构造 ====================

    /// 零次或多次：A*
    ///
    /// 实现（Thompson 经典构造）：
    ///   new_start --ε→ new_end          （零次匹配）
    ///   new_start --ε→ A.start         （进入循环）
    ///   A.end --ε→ A.start             （循环自环）
    ///   A.end --ε→ new_end             （退出循环）
    fn compile_zero_or_more(&mut self, hir: &Hir) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        let frag = self.compile_hir(hir);

        self.builder.add_epsilon(s, e);            // 零次
        self.builder.add_epsilon(s, frag.start);   // 进入
        self.builder.add_epsilon(frag.end, frag.start); // 自环
        self.builder.add_epsilon(frag.end, e);     // 退出

        Fragment::new(s, e)
    }

    /// 一次或多次：A+
    ///
    /// 实现：A · A*（等价于先匹配一次，再星号）
    ///   new_start --ε→ A.start
    ///   A.end --ε→ A.start            （循环自环）
    ///   A.end --ε→ new_end
    fn compile_one_or_more(&mut self, hir: &Hir) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        let frag = self.compile_hir(hir);

        self.builder.add_epsilon(s, frag.start);
        self.builder.add_epsilon(frag.end, frag.start); // 自环
        self.builder.add_epsilon(frag.end, e);

        Fragment::new(s, e)
    }

    /// 零次或一次：A?
    ///
    /// 实现：
    ///   new_start --ε→ new_end          （零次匹配）
    ///   new_start --ε→ A.start
    ///   A.end --ε→ new_end
    fn compile_zero_or_one(&mut self, hir: &Hir) -> Fragment {
        let s = self.builder.add_state();
        let e = self.builder.add_state();
        let frag = self.compile_hir(hir);

        self.builder.add_epsilon(s, e);          // 零次
        self.builder.add_epsilon(s, frag.start);
        self.builder.add_epsilon(frag.end, e);

        Fragment::new(s, e)
    }

    /// 重复指定次数：A{n,m}
    ///
    /// 实现策略：
    /// - min 次精确匹配的序列
    /// - max - min 次可选匹配（每个都是 A?）
    /// - 如果 max 为 None，则外加 A*
    fn compile_repeat(&mut self, hir: &Hir, min: u32, max: Option<u32>) -> Fragment {
        match max {
            Some(m) if min == m => {
                // 精确重复 n 次
                if min == 0 {
                    return self.compile_empty();
                }
                let mut frags = Vec::new();
                for _ in 0..min {
                    frags.push(self.compile_hir(hir));
                }
                for i in 1..frags.len() {
                    self.builder.add_epsilon(frags[i - 1].end, frags[i].start);
                }
                Fragment::new(frags[0].start, frags.last().unwrap().end)
            }
            Some(m) => {
                // A{min,max} = A{min} · A?{max-min}
                let required = if min > 0 {
                    self.compile_exact(hir, min)
                } else {
                    self.compile_empty()
                };

                let optional_count = m - min;
                let mut frags = vec![required];
                for _ in 0..optional_count {
                    frags.push(self.compile_zero_or_one(hir));
                }
                for i in 1..frags.len() {
                    self.builder.add_epsilon(frags[i - 1].end, frags[i].start);
                }
                Fragment::new(frags[0].start, frags.last().unwrap().end)
            }
            None => {
                // A{min,} = A{min} · A*
                let required = if min > 0 {
                    self.compile_exact(hir, min)
                } else {
                    self.compile_empty()
                };
                let star = self.compile_zero_or_more(hir);

                self.builder.add_epsilon(required.end, star.start);
                Fragment::new(required.start, star.end)
            }
        }
    }

    /// 编译精确重复 n 次
    fn compile_exact(&mut self, hir: &Hir, n: u32) -> Fragment {
        if n == 0 {
            return self.compile_empty();
        }

        let frags: Vec<Fragment> = (0..n)
            .map(|_| self.compile_hir(hir))
            .collect();

        for i in 1..frags.len() {
            self.builder.add_epsilon(frags[i - 1].end, frags[i].start);
        }

        Fragment::new(frags[0].start, frags.last().unwrap().end)
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::CharClass;

    #[test]
    fn test_compile_literal() {
        let mut compiler = Compiler::new();
        let hir = Hir::literal('a');
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        assert_eq!(compiler.builder().state_count(), 2);
    }

    #[test]
    fn test_compile_empty() {
        let mut compiler = Compiler::new();
        let hir = Hir::Empty;
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        // 空串：两个状态 + 一个 epsilon
        assert_eq!(compiler.builder().state_count(), 2);
    }

    #[test]
    fn test_compile_sequence() {
        let mut compiler = Compiler::new();
        let hir = Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]);
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        assert_eq!(compiler.builder().state_count(), 4);
    }

    #[test]
    fn test_compile_choice() {
        let mut compiler = Compiler::new();
        let hir = Hir::choice(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]);
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        // 2 个子 NFA（每个 2 状态）+ 1 开始 + 1 结束 = 6
        assert_eq!(compiler.builder().state_count(), 6);
    }

    #[test]
    fn test_compile_zero_or_more() {
        let mut compiler = Compiler::new();
        let hir = Hir::zero_or_more(Hir::literal('a'));
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        // 2 (字面量) + 2 (星号包裹) = 4
        assert_eq!(compiler.builder().state_count(), 4);
    }

    #[test]
    fn test_compile_one_or_more() {
        let mut compiler = Compiler::new();
        let hir = Hir::one_or_more(Hir::literal('a'));
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_zero_or_one() {
        let mut compiler = Compiler::new();
        let hir = Hir::zero_or_one(Hir::literal('a'));
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_class() {
        let mut compiler = Compiler::new();
        let class = CharClass::range('0', '9');
        let hir = Hir::Class(class);
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_repeat_exact() {
        let mut compiler = Compiler::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 3,
            max: Some(3),
        };
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
        // 3 * 2 = 6 个状态
        assert_eq!(compiler.builder().state_count(), 6);
    }

    #[test]
    fn test_compile_repeat_range() {
        let mut compiler = Compiler::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 2,
            max: Some(5),
        };
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_repeat_unbounded() {
        let mut compiler = Compiler::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 1,
            max: None,
        };
        let frag = compiler.compile_hir(&hir);
        
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_into_builder() {
        let mut compiler = Compiler::new();
        let _ = compiler.compile_hir(&Hir::literal('a'));
        let builder = compiler.into_builder();
        
        assert_eq!(builder.state_count(), 2);
    }
}