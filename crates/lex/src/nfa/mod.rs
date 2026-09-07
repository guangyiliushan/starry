//! NFA 模块
//!
//! 非确定性有限自动机（NFA）的定义和操作。
//!
//! # 模块架构
//!
//! - **`edge`** - 转移边，将纯条件的 `Transition` 与目标 `StateId` 绑定
//! - **`state`** - NFA 状态，分离存储 epsilon 和非 epsilon 转移
//! - **`builder`** - 底层状态管理器，负责状态 ID 分配和转移添加
//! - **`compiler`** - Thompson 构造算法，将 HIR 编译为 NFA
//! - **`mod`** (本模块) - NFA 对外接口，epsilon_closure、match_prefix、from_hir

use std::collections::VecDeque;

pub mod edge;
pub mod state;
pub mod builder;

pub use edge::Edge;
pub use state::NFAState;
pub use builder::{Builder, Fragment};

use crate::regex::hir::Hir;
use crate::regex::optimize::Optimizer;
use crate::state::{StateId, StateSet};
use ast::token::TokenKind;

/// 非确定性有限自动机
#[derive(Debug, Clone)]
pub struct NFA {
    states: Vec<NFAState>,
    start_state: StateId,
    /// 接受态表：索引即 StateId（稠密编号配稠密向量，迭代序 = 状态分配序）
    accepting: Vec<Option<TokenKind>>,
}

impl NFA {
    pub fn new(
        start_state: StateId,
        states: Vec<NFAState>,
        accepting: Vec<Option<TokenKind>>,
    ) -> Self {
        Self {
            states,
            start_state,
            accepting,
        }
    }

    /// 从 HIR 构建 NFA（Thompson 构造；内部先跑优化器——优化的唯一入口）
    pub fn from_hir(hir: &Hir, token_kind: TokenKind) -> Self {
        let (hir, _) = Optimizer::new().optimize(hir);

        let mut builder = Builder::new();
        let frag = builder.compile(&hir);

        let mut accepting = vec![None; builder.state_count()];
        accepting[frag.end] = Some(token_kind);

        NFA {
            states: builder.into_states(),
            start_state: frag.start,
            accepting,
        }
    }

    /// 从多个 HIR 规则构建组合 NFA
    ///
    /// 单 Builder 编译：所有规则共享同一状态编号空间（计数器恒驻
    /// Builder），全局 start 用 epsilon 连到各规则片段，无需偏移与拷贝。
    pub fn from_hir_multi(rules: impl IntoIterator<Item = (Hir, TokenKind)>) -> Self {
        let mut builder = Builder::new();
        let global_start = builder.add_state();
        let mut accepting = vec![None];

        for (hir, token_kind) in rules {
            let (hir, _) = Optimizer::new().optimize(&hir);
            let frag = builder.compile(&hir);

            builder.add_epsilon(global_start, frag.start);
            accepting.resize(builder.state_count(), None);
            accepting[frag.end] = Some(token_kind);
        }

        let states = builder.into_states();
        NFA {
            states,
            start_state: global_start,
            accepting,
        }
    }

    // ==================== Query methods ====================

    pub fn start_state(&self) -> StateId {
        self.start_state
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn state(&self, id: StateId) -> &NFAState {
        &self.states[id]
    }

    pub fn states(&self) -> &[NFAState] {
        &self.states
    }

    pub fn accepting(&self) -> &[Option<TokenKind>] {
        &self.accepting
    }

    pub fn is_accepting(&self, id: StateId) -> bool {
        self.accepting[id].is_some()
    }

    pub fn token_kind(&self, id: StateId) -> Option<TokenKind> {
        self.accepting[id]
    }

    // ==================== Epsilon closure ====================

    /// Compute epsilon closure of a set of states (BFS + StateSet)
    pub fn epsilon_closure(&self, states: &StateSet) -> StateSet {
        let mut closure = StateSet::new();
        let mut queue: VecDeque<StateId> = VecDeque::new();

        for &s in states {
            closure.insert(s);
            queue.push_back(s);
        }

        while let Some(id) = queue.pop_front() {
            for &eps_target in &self.states[id].epsilons {
                if closure.insert(eps_target) {
                    queue.push_back(eps_target);
                }
            }
        }

        closure
    }

    /// Compute epsilon closure of a single state
    pub fn epsilon_closure_of(&self, state: StateId) -> StateSet {
        let mut set = StateSet::new();
        set.insert(state);
        self.epsilon_closure(&set)
    }

    /// Move: states reachable via non-epsilon transition on char c
    pub fn move_on(&self, states: &StateSet, c: char) -> StateSet {
        let mut next = StateSet::new();
        for &s in states {
            for edge in &self.states[s].edges {
                if edge.trans.matches(c) {
                    next.insert(edge.target);
                }
            }
        }
        next
    }

    /// Single step: move_on + epsilon_closure
    pub fn step(&self, states: &StateSet, c: char) -> StateSet {
        let next = self.move_on(states, c);
        self.epsilon_closure(&next)
    }

    // ==================== Matching ====================

    /// Attempt to match a prefix of the input
    ///
    /// Returns the length of the longest match, or None.
    pub fn match_prefix(&self, input: &[char]) -> Option<usize> {
        let start_set = self.epsilon_closure_of(self.start_state);
        let mut current = start_set;
        let mut last_accept: Option<usize> = None;

        for &s in &current {
            if self.is_accepting(s) {
                last_accept = Some(0);
                break;
            }
        }

        for (i, &c) in input.iter().enumerate() {
            let next = self.move_on(&current, c);
            if next.is_empty() {
                break;
            }
            current = self.epsilon_closure(&next);

            for &s in &current {
                if self.is_accepting(s) {
                    last_accept = Some(i + 1);
                    break;
                }
            }
        }

        last_accept
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_hir_literal() {
        let hir = Hir::literal('a');
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        assert_eq!(nfa.state_count(), 2);
        assert!(nfa.is_accepting(1));
    }

    #[test]
    fn test_from_hir_sequence() {
        let hir = Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]);
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        assert_eq!(nfa.state_count(), 4);
        assert!(nfa.is_accepting(3));
    }

    #[test]
    fn test_epsilon_closure_literal() {
        let hir = Hir::literal('a');
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let closure = nfa.epsilon_closure_of(0);
        assert_eq!(closure.len(), 1);
    }

    #[test]
    fn test_epsilon_closure_choice() {
        let hir = Hir::choice(vec![Hir::literal('a'), Hir::literal('b')]);
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let closure = nfa.epsilon_closure_of(nfa.start_state());
        assert!(closure.len() >= 2);
    }

    #[test]
    fn test_epsilon_closure_star() {
        let hir = Hir::zero_or_more(Hir::literal('a'));
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let closure = nfa.epsilon_closure_of(nfa.start_state());
        assert!(closure.len() >= 2);
    }

    #[test]
    fn test_move_on() {
        let hir = Hir::literal('a');
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let start_set = nfa.epsilon_closure_of(nfa.start_state());
        let next = nfa.move_on(&start_set, 'a');
        assert_eq!(next.len(), 1);
    }

    #[test]
    fn test_match_prefix_literal() {
        let hir = Hir::literal('a');
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let input: Vec<char> = "a".chars().collect();
        assert_eq!(nfa.match_prefix(&input), Some(1));
        let bad: Vec<char> = "b".chars().collect();
        assert_eq!(nfa.match_prefix(&bad), None);
    }

    #[test]
    fn test_match_prefix_sequence() {
        let hir = Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]);
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let input: Vec<char> = "ab".chars().collect();
        assert_eq!(nfa.match_prefix(&input), Some(2));
        let bad: Vec<char> = "ac".chars().collect();
        assert_eq!(nfa.match_prefix(&bad), None);
    }

    #[test]
    fn test_match_prefix_star() {
        let hir = Hir::zero_or_more(Hir::literal('a'));
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let empty: Vec<char> = vec![];
        assert_eq!(nfa.match_prefix(&empty), Some(0));
        let aaa: Vec<char> = "aaa".chars().collect();
        assert_eq!(nfa.match_prefix(&aaa), Some(3));
    }

    #[test]
    fn test_from_hir_multi() {
        let rules = vec![
            (Hir::literal('a'), TokenKind::Identifier),
            (Hir::literal('b'), TokenKind::Identifier),
        ];
        let nfa = NFA::from_hir_multi(rules);
        assert!(nfa.state_count() >= 5); // 1 global_start + 2*2 sub-states
    }
}
