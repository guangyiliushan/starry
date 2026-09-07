//! 状态管理模块
//!
//! 词法分析自动机的**状态层**基础类型（面向状态转移函数 / DFA）：
//! - [`StateId`] - 状态标识符
//! - [`StateSet`] - 状态集合（`BTreeSet`，迭代序确定，供子集法复现）
//! - [`State`] - 转移函数的状态（携带接受信息）
//! - [`StateGenerator`] - 状态 ID 生成器
//!
//! # 与 NFA 层的关系
//!
//! NFA 层（[`crate::nfa`]）用 `Vec<NFAState>` 表达图节点、用独立的
//! `accepting` 向量记录接受态；本模块的 `State`/`StateGenerator` 为
//! 子集法构造 DFA 预留——DFA 的一个状态对应“NFA 状态集合”（转移
//! 函数的目标），接受信息单点存放在 [`State::token_kind`]。两层各自
//! 单点定义接受概念，不共享结构。

use std::collections::BTreeSet;
use std::fmt;

use ast::token::TokenKind;

// ==================== 状态标识符 ====================

/// 状态标识符
///
/// 零成本抽象：可直接作数组索引，无额外内存开销。
pub type StateId = usize;

// ==================== 状态集合 ====================

/// 状态集合
///
/// `BTreeSet` 保证迭代序确定（子集法构造可复现），可直接比较相等、
/// 直接作为 HashMap 的键。
pub type StateSet = BTreeSet<StateId>;

// ==================== 状态定义 ====================

/// 转移函数的状态：携带接受信息
///
/// - 接受状态：`token_kind` 为 `Some`，自动机在此产生对应 Token
/// - 非接受状态：`token_kind` 为 `None`
///
/// # 示例
///
/// ```
/// use ast::token::TokenKind;
/// use lex::state::State;
///
/// let state = State::accepting(1, TokenKind::Identifier);
/// assert!(state.is_accepting());
/// assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// 状态 ID
    pub id: StateId,
    /// Token 类型（None 表示非接受状态）
    pub token_kind: Option<TokenKind>,
}

impl State {
    /// 创建新的非接受状态
    ///
    /// # 示例
    ///
    /// ```
    /// use lex::state::State;
    ///
    /// let state = State::new(0);
    /// assert_eq!(state.id, 0);
    /// assert!(!state.is_accepting());
    /// ```
    pub fn new(id: StateId) -> Self {
        Self {
            id,
            token_kind: None,
        }
    }

    /// 创建接受状态
    ///
    /// # 示例
    ///
    /// ```
    /// use ast::token::TokenKind;
    /// use lex::state::State;
    ///
    /// let state = State::accepting(1, TokenKind::Identifier);
    /// assert!(state.is_accepting());
    /// ```
    pub fn accepting(id: StateId, token_kind: TokenKind) -> Self {
        Self {
            id,
            token_kind: Some(token_kind),
        }
    }

    /// 检查是否为接受状态
    pub fn is_accepting(&self) -> bool {
        self.token_kind.is_some()
    }

    /// 获取 Token 类型
    ///
    /// # 示例
    ///
    /// ```
    /// use ast::token::TokenKind;
    /// use lex::state::State;
    ///
    /// let state = State::accepting(0, TokenKind::Identifier);
    /// assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));
    /// ```
    pub fn token_kind(&self) -> Option<&TokenKind> {
        self.token_kind.as_ref()
    }

    /// 设置 Token 类型（None 表示转为非接受状态）
    pub fn set_token_kind(&mut self, token_kind: Option<TokenKind>) {
        self.token_kind = token_kind;
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(kind) = &self.token_kind {
            write!(f, "S{}(accepting: {})", self.id, kind)
        } else {
            write!(f, "S{}", self.id)
        }
    }
}

// ==================== 状态生成器 ====================

/// 状态 ID 生成器：递增计数器
///
/// 编译器短期运行无需 ID 重用，单调递增即可；并发构建时按线程划分
/// 独立 ID 段（`with_start` 偏移）即可，无需加锁。
///
/// # 示例
///
/// ```
/// use lex::state::StateGenerator;
///
/// let mut generator = StateGenerator::new();
/// assert_eq!(generator.next(), 0);
/// assert_eq!(generator.next(), 1);
/// ```
pub struct StateGenerator {
    /// 下一个可用的状态 ID
    next_id: StateId,
}

impl Default for StateGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl StateGenerator {
    /// 创建新的状态生成器，从 0 开始
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    /// 创建新的状态生成器，从指定 ID 开始（组合多个来源时划分 ID 段）
    ///
    /// # 示例
    ///
    /// ```
    /// use lex::state::StateGenerator;
    ///
    /// let mut generator = StateGenerator::with_start(100);
    /// assert_eq!(generator.next(), 100);
    /// assert_eq!(generator.next(), 101);
    /// ```
    pub fn with_start(start: StateId) -> Self {
        Self { next_id: start }
    }

    /// 生成下一个状态 ID
    pub fn next(&mut self) -> StateId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 查看下一个将要生成的状态 ID（不生成）
    ///
    /// # 示例
    ///
    /// ```
    /// use lex::state::StateGenerator;
    ///
    /// let generator = StateGenerator::new();
    /// assert_eq!(generator.peek(), 0);
    /// ```
    pub fn peek(&self) -> StateId {
        self.next_id
    }

    /// 批量生成多个状态 ID，返回连续 ID 向量
    ///
    /// # 示例
    ///
    /// ```
    /// use lex::state::StateGenerator;
    ///
    /// let mut generator = StateGenerator::new();
    /// assert_eq!(generator.next_batch(3), vec![0, 1, 2]);
    /// ```
    pub fn next_batch(&mut self, count: usize) -> Vec<StateId> {
        let start = self.next_id;
        self.next_id += count;
        (start..self.next_id).collect()
    }

    /// 重置生成器到指定 ID（注意：可能导致 ID 重复）
    pub fn reset(&mut self, id: StateId) {
        self.next_id = id;
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_new() {
        let state = State::new(0);
        assert_eq!(state.id, 0);
        assert!(!state.is_accepting());
        assert!(state.token_kind.is_none());
    }

    #[test]
    fn test_state_accepting() {
        let state = State::accepting(1, TokenKind::Identifier);
        assert_eq!(state.id, 1);
        assert!(state.is_accepting());
        assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));
    }

    #[test]
    fn test_state_set_token_kind() {
        let mut state = State::new(0);
        assert!(!state.is_accepting());

        state.set_token_kind(Some(TokenKind::Identifier));
        assert!(state.is_accepting());
        assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));

        state.set_token_kind(None);
        assert!(!state.is_accepting());
    }

    #[test]
    fn test_state_display() {
        let normal = State::new(0);
        assert_eq!(normal.to_string(), "S0");

        let accepting = State::accepting(1, TokenKind::Identifier);
        assert_eq!(accepting.to_string(), "S1(accepting: identifier)");
    }

    #[test]
    fn test_state_generator() {
        let mut generator = StateGenerator::new();
        assert_eq!(generator.peek(), 0);
        assert_eq!(generator.next(), 0);
        assert_eq!(generator.peek(), 1);
        assert_eq!(generator.next(), 1);

        assert_eq!(generator.next_batch(3), vec![2, 3, 4]);
        assert_eq!(generator.peek(), 5);

        generator.reset(10);
        assert_eq!(generator.next(), 10);
        assert_eq!(generator.next(), 11);
    }

    #[test]
    fn test_state_generator_with_start() {
        let mut generator = StateGenerator::with_start(100);
        assert_eq!(generator.peek(), 100);
        assert_eq!(generator.next(), 100);
        assert_eq!(generator.next(), 101);
    }

    #[test]
    fn test_state_set_operations() {
        let set1: StateSet = BTreeSet::from([0, 1, 2]);
        let set2: StateSet = BTreeSet::from([2, 3, 4]);

        // 并集/交集/差集走 std 迭代器
        let union: StateSet = set1.union(&set2).copied().collect();
        assert_eq!(union.len(), 5);

        let intersection: StateSet = set1.intersection(&set2).copied().collect();
        assert_eq!(intersection, BTreeSet::from([2]));

        let difference: StateSet = set1.difference(&set2).copied().collect();
        assert_eq!(difference, BTreeSet::from([0, 1]));
    }
}
