//! NFA Builder 模块
//!
//! 底层状态管理器，负责状态 ID 分配、状态创建和转移添加。
//! 被 `Compiler` 使用，不直接暴露给最终用户。
//!
//! # 设计特点
//!
//! - **全局状态生成器**：使用递增计数器，保证状态 ID 全局唯一
//! - **Vec 存储**：状态以数组存储，StateId 即数组索引
//! - **方法语义清晰**：add_epsilon / add_edge 明确区分转移类型

use crate::state::StateId;
use crate::transition::Transition;
use super::edge::Edge;
use super::state::NFAState;

/// NFA Builder — 底层状态管理器
///
/// 负责分配状态 ID、创建状态、添加转移。
/// 被 Compiler 独占使用，通过 `into_states()` 交付给 NFA。
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
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::nfa::Builder;
    /// let builder = Builder::new();
    /// assert_eq!(builder.state_count(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
        }
    }

    // ==================== 状态管理 ====================

    /// 添加一个新状态，返回自动分配的 StateId
    ///
    /// StateId 等于当前状态数组的长度（递增计数）。
    pub fn add_state(&mut self) -> StateId {
        let id = self.states.len();
        self.states.push(NFAState::new(id));
        id
    }

    /// 批量添加 n 个状态
    ///
    /// 返回起始状态 ID（后续 n 个 ID 连续）。
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
    ///
    /// 将目标状态 ID 加入源状态的 epsilons 列表。
    ///
    /// # 参数
    ///
    /// - `from` - 源状态 ID
    /// - `to` - 目标状态 ID
    pub fn add_epsilon(&mut self, from: StateId, to: StateId) {
        self.states[from].epsilons.push(to);
    }

    /// 添加非 epsilon 转移边
    ///
    /// 将匹配条件和目标状态封装为 Edge 后加入源状态的 edges 列表。
    ///
    /// # 参数
    ///
    /// - `from` - 源状态 ID
    /// - `trans` - 匹配条件（Transition，纯条件）
    /// - `to` - 目标状态 ID
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

    // ==================== 交付 ====================

    /// 消费 Builder，返回所有状态
    ///
    /// 供 NFA 构造函数使用。
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
        assert_eq!(builder.state(0).id, 0);
        assert_eq!(builder.state(1).id, 1);
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
}