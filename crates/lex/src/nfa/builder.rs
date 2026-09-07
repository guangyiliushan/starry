//! NFA Builder 模块
//!
//! 底层状态管理器：负责状态 ID 分配、状态创建、转移添加，并通过
//! [`Builder::compile`]（Thompson 构造）把 [`Hir`] 编译为 NFA 片段。
//!
//! # 纪律条款
//!
//! - 方法接收者一律 `&mut self`；
//! - 未来若引入 `Builder<'a>`（借用 arena 或 `&'a mut Vec<NFAState>`），
//!   禁止把 `'a` 传播进方法接收者——否则首次调用后整个 Builder 被锁定
//!   到 `'a` 结束，后续只读调用也会被借用检查拦下。
//!
//! # 示例
//!
//! ```
//! # use lex::nfa::Builder;
//! # use lex::regex::hir::Hir;
//! let mut builder = Builder::new();
//! let frag = builder.compile(&Hir::literal('a'));
//! assert_ne!(frag.start, frag.end);
//! assert_eq!(builder.state_count(), 2);
//! ```

use crate::regex::hir::Hir;
use crate::state::StateId;
use crate::transition::Transition;
use super::edge::Edge;
use super::state::NFAState;

/// 编译结果：一个 NFA 片段（start 与 end 状态）
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

/// NFA Builder — 状态管理 + Thompson 构造
///
/// 状态计数器就是 `states.len()`，恒驻 Builder 内部：多次 `compile()`
/// 共享同一编号空间，这是多规则免偏移合并的关键不变量。
///
/// # 示例
///
/// ```
/// # use lex::nfa::Builder;
/// # use lex::Transition;
/// let mut builder = Builder::new();
/// let s0 = builder.add_state();
/// let s1 = builder.add_state();
/// builder.add_edge(s0, Transition::char('a'), s1);
/// builder.add_epsilon(s0, s1);
/// assert_eq!(builder.state_count(), 2);
/// ```
#[derive(Debug)]
pub struct Builder {
    /// 所有 NFA 状态的存储，索引即 StateId
    states: Vec<NFAState>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// 创建新的 Builder
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
        }
    }

    // ==================== 状态管理 ====================

    /// 添加一个新状态，返回自动分配的 StateId（等于当前数组长度）
    pub fn add_state(&mut self) -> StateId {
        let id = self.states.len();
        self.states.push(NFAState::new(id));
        id
    }

    /// 批量添加 n 个状态，返回起始状态 ID（后续 n 个 ID 连续）
    pub fn add_states(&mut self, count: usize) -> StateId {
        let start = self.states.len();
        for i in 0..count {
            self.states.push(NFAState::new(start + i));
        }
        start
    }

    /// 获取状态总数
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    /// 获取状态的不可变引用
    pub fn state(&self, id: StateId) -> &NFAState {
        &self.states[id]
    }

    /// 获取状态的可变引用
    pub fn state_mut(&mut self, id: StateId) -> &mut NFAState {
        &mut self.states[id]
    }

    // ==================== 转移操作 ====================

    /// 添加 epsilon 转移
    pub fn add_epsilon(&mut self, from: StateId, to: StateId) {
        self.states[from].epsilons.push(to);
    }

    /// 添加非 epsilon 转移边
    pub fn add_edge(&mut self, from: StateId, trans: Transition, to: StateId) {
        self.states[from].edges.push(Edge::new(trans, to));
    }

    /// 添加字符转移（快捷方法）
    pub fn add_char(&mut self, from: StateId, c: char, to: StateId) {
        self.add_edge(from, Transition::char(c), to);
    }

    /// 添加字符类转移（快捷方法）
    pub fn add_class(
        &mut self,
        from: StateId,
        class: impl Into<crate::transition::CharClass>,
        to: StateId,
    ) {
        self.add_edge(from, Transition::char_class(class.into()), to);
    }

    /// 添加范围转移（快捷方法）
    pub fn add_range(&mut self, from: StateId, start: char, end: char, to: StateId) {
        self.add_edge(from, Transition::range(start, end), to);
    }

    // ==================== Thompson 构造 ====================

    /// 编译 HIR 为 NFA 片段（Thompson 构造）
    ///
    /// 多次调用共享同一状态编号空间；返回的 [`Fragment`] 挂在当前
    /// 编号空间上。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::nfa::Builder;
    /// # use lex::regex::hir::Hir;
    /// let mut builder = Builder::new();
    /// let frag = builder.compile(&Hir::sequence(vec![
    ///     Hir::literal('a'),
    ///     Hir::literal('b'),
    /// ]));
    /// assert_eq!(builder.state_count(), 4);
    /// ```
    pub fn compile(&mut self, hir: &Hir) -> Fragment {
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

    /// 空串：start --ε→ end
    fn compile_empty(&mut self) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        self.add_epsilon(s, e);
        Fragment::new(s, e)
    }

    /// 字面量：start --c→ end
    fn compile_literal(&mut self, c: char) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        self.add_char(s, c, e);
        Fragment::new(s, e)
    }

    /// 字符类：start --class→ end
    fn compile_class(&mut self, class: &crate::transition::CharClass) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        self.add_edge(s, Transition::char_class(class.clone()), e);
        Fragment::new(s, e)
    }

    /// 序列：A · B · C
    fn compile_sequence(&mut self, hirs: &[Hir]) -> Fragment {
        if hirs.is_empty() {
            return self.compile_empty();
        }

        let frags: Vec<Fragment> = hirs.iter().map(|h| self.compile(h)).collect();

        for i in 1..frags.len() {
            self.add_epsilon(frags[i - 1].end, frags[i].start);
        }

        Fragment::new(frags[0].start, frags.last().unwrap().end)
    }

    /// 选择：A | B | C
    fn compile_choice(&mut self, hirs: &[Hir]) -> Fragment {
        if hirs.is_empty() {
            return self.compile_empty();
        }

        let s = self.add_state();
        let e = self.add_state();

        for hir in hirs {
            let frag = self.compile(hir);
            self.add_epsilon(s, frag.start);
            self.add_epsilon(frag.end, e);
        }

        Fragment::new(s, e)
    }

    /// 零次或多次：A*
    fn compile_zero_or_more(&mut self, hir: &Hir) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        let frag = self.compile(hir);

        self.add_epsilon(s, e); // 零次
        self.add_epsilon(s, frag.start); // 进入循环
        self.add_epsilon(frag.end, frag.start); // 循环自环
        self.add_epsilon(frag.end, e); // 退出循环

        Fragment::new(s, e)
    }

    /// 一次或多次：A+
    fn compile_one_or_more(&mut self, hir: &Hir) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        let frag = self.compile(hir);

        self.add_epsilon(s, frag.start);
        self.add_epsilon(frag.end, frag.start); // 循环自环
        self.add_epsilon(frag.end, e);

        Fragment::new(s, e)
    }

    /// 零次或一次：A?
    fn compile_zero_or_one(&mut self, hir: &Hir) -> Fragment {
        let s = self.add_state();
        let e = self.add_state();
        let frag = self.compile(hir);

        self.add_epsilon(s, e); // 零次
        self.add_epsilon(s, frag.start);
        self.add_epsilon(frag.end, e);

        Fragment::new(s, e)
    }

    /// 重复指定次数：A{n,m}
    ///
    /// - `{n}` = 精确重复 n 次
    /// - `{min,max}` = A{min} · A?{max-min}
    /// - `{min,}` = A{min} · A*
    fn compile_repeat(&mut self, hir: &Hir, min: u32, max: Option<u32>) -> Fragment {
        match max {
            Some(m) if min == m => self.compile_exact(hir, min),
            Some(m) if min < m => {
                let required = if min > 0 {
                    self.compile_exact(hir, min)
                } else {
                    self.compile_empty()
                };

                let mut frags = vec![required];
                for _ in 0..(m - min) {
                    frags.push(self.compile_zero_or_one(hir));
                }
                for i in 1..frags.len() {
                    self.add_epsilon(frags[i - 1].end, frags[i].start);
                }
                Fragment::new(frags[0].start, frags.last().unwrap().end)
            }
            // min > max 的非法 HIR（构造方 bug）：退化为空串，避免下溢
            Some(_) => self.compile_empty(),
            None => {
                let required = if min > 0 {
                    self.compile_exact(hir, min)
                } else {
                    self.compile_empty()
                };
                let star = self.compile_zero_or_more(hir);

                self.add_epsilon(required.end, star.start);
                Fragment::new(required.start, star.end)
            }
        }
    }

    /// 编译精确重复 n 次
    fn compile_exact(&mut self, hir: &Hir, n: u32) -> Fragment {
        if n == 0 {
            return self.compile_empty();
        }

        let frags: Vec<Fragment> = (0..n).map(|_| self.compile(hir)).collect();

        for i in 1..frags.len() {
            self.add_epsilon(frags[i - 1].end, frags[i].start);
        }

        Fragment::new(frags[0].start, frags.last().unwrap().end)
    }

    // ==================== 交付 ====================

    /// 消费 Builder，返回所有状态
    pub fn into_states(self) -> Vec<NFAState> {
        self.states
    }

    /// 获取状态列表的不可变引用
    pub fn states(&self) -> &[NFAState] {
        &self.states
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new() {
        let builder = Builder::new();
        assert_eq!(builder.state_count(), 0);
    }

    #[test]
    fn test_add_state() {
        let mut builder = Builder::new();
        let s0 = builder.add_state();
        let s1 = builder.add_state();
        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(builder.state_count(), 2);
    }

    #[test]
    fn test_add_epsilon() {
        let mut builder = Builder::new();
        let s0 = builder.add_state();
        let s1 = builder.add_state();
        builder.add_epsilon(s0, s1);

        assert_eq!(builder.state(s0).epsilons, vec![s1]);
        assert!(builder.state(s1).epsilons.is_empty());
    }

    #[test]
    fn test_add_edge() {
        let mut builder = Builder::new();
        let s0 = builder.add_state();
        let s1 = builder.add_state();
        builder.add_char(s0, 'a', s1);

        assert_eq!(builder.state(s0).edges.len(), 1);
        assert_eq!(builder.state(s0).edges[0].target, s1);
    }

    #[test]
    fn test_add_states() {
        let mut builder = Builder::new();
        let start = builder.add_states(3);
        assert_eq!(start, 0);
        assert_eq!(builder.state_count(), 3);
        assert_eq!(builder.state(2).id, 2);
    }

    #[test]
    fn test_into_states() {
        let mut builder = Builder::new();
        builder.add_state();
        builder.add_state();
        let states = builder.into_states();
        assert_eq!(states.len(), 2);
    }

    #[test]
    fn test_compile_literal() {
        let mut builder = Builder::new();
        let frag = builder.compile(&Hir::literal('a'));

        assert_ne!(frag.start, frag.end);
        assert_eq!(builder.state_count(), 2);
    }

    #[test]
    fn test_compile_empty() {
        let mut builder = Builder::new();
        let frag = builder.compile(&Hir::Empty);

        assert_ne!(frag.start, frag.end);
        assert_eq!(builder.state_count(), 2);
    }

    #[test]
    fn test_compile_sequence() {
        let mut builder = Builder::new();
        let hir = Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]);
        let frag = builder.compile(&hir);

        assert_ne!(frag.start, frag.end);
        assert_eq!(builder.state_count(), 4);
    }

    #[test]
    fn test_compile_choice() {
        let mut builder = Builder::new();
        let hir = Hir::choice(vec![Hir::literal('a'), Hir::literal('b')]);
        let frag = builder.compile(&hir);

        assert_ne!(frag.start, frag.end);
        // 2 个子 NFA（每个 2 状态）+ 开始 + 结束
        assert_eq!(builder.state_count(), 6);
    }

    #[test]
    fn test_compile_zero_or_more() {
        let mut builder = Builder::new();
        let hir = Hir::zero_or_more(Hir::literal('a'));
        builder.compile(&hir);

        assert_eq!(builder.state_count(), 4);
    }

    #[test]
    fn test_compile_one_or_more_and_optional() {
        let mut builder = Builder::new();
        builder.compile(&Hir::one_or_more(Hir::literal('a')));
        assert_eq!(builder.state_count(), 4);

        let mut builder = Builder::new();
        builder.compile(&Hir::zero_or_one(Hir::literal('a')));
        assert_eq!(builder.state_count(), 4);
    }

    #[test]
    fn test_compile_class() {
        use crate::transition::CharClass;

        let mut builder = Builder::new();
        let hir = Hir::Class(CharClass::range('0', '9'));
        let frag = builder.compile(&hir);

        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_repeat_exact_reuses_exact_path() {
        let mut builder = Builder::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 3,
            max: Some(3),
        };
        builder.compile(&hir);

        // 3 * 2 = 6 个状态（min == m 走 compile_exact，无重复实现）
        assert_eq!(builder.state_count(), 6);
    }

    #[test]
    fn test_compile_repeat_zero_is_empty() {
        let mut builder = Builder::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 0,
            max: Some(0),
        };
        builder.compile(&hir);

        // 精确 0 次 = 空串片段
        assert_eq!(builder.state_count(), 2);
    }

    #[test]
    fn test_compile_repeat_range_and_unbounded() {
        let mut builder = Builder::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 2,
            max: Some(5),
        };
        let frag = builder.compile(&hir);
        assert_ne!(frag.start, frag.end);

        let mut builder = Builder::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 1,
            max: None,
        };
        let frag = builder.compile(&hir);
        assert_ne!(frag.start, frag.end);
    }

    #[test]
    fn test_compile_repeat_invalid_range_degrades() {
        // min > max 的非法 HIR：退化为空串而非下溢 panic
        let mut builder = Builder::new();
        let hir = Hir::Repeat {
            expr: Box::new(Hir::literal('a')),
            min: 3,
            max: Some(1),
        };
        builder.compile(&hir);
        assert_eq!(builder.state_count(), 2);
    }
}
