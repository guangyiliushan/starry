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

use std::collections::HashMap;
use std::collections::VecDeque;

pub mod edge;
pub mod state;
pub mod builder;
pub mod compiler;

pub use edge::Edge;
pub use state::NFAState;
pub use builder::Builder;
pub use compiler::Compiler;

use crate::regex::hir::Hir;
use crate::state::{StateId, StateSet};
use ast::token::TokenKind;

/// 非确定性有限自动机
#[derive(Debug, Clone)]
pub struct NFA {
    states: Vec<NFAState>,
    start_state: StateId,
    accept_states: HashMap<StateId, TokenKind>,
}

impl NFA {
    pub fn new(
        start_state: StateId,
        states: Vec<NFAState>,
        accept_states: HashMap<StateId, TokenKind>,
    ) -> Self {
        Self {
            states,
            start_state,
            accept_states,
        }
    }

    /// 从 HIR 构建 NFA（使用 Thompson 构造）
    pub fn from_hir(hir: &Hir, token_kind: TokenKind) -> Self {
        let mut compiler = Compiler::new();
        let frag = compiler.compile_hir(hir);

        let builder = compiler.into_builder();
        let mut accept_states = HashMap::new();
        accept_states.insert(frag.end, token_kind);

        NFA {
            states: builder.into_states(),
            start_state: frag.start,
            accept_states,
        }
    }

    /// 从多个 HIR 规则构建组合 NFA
    pub fn from_hir_multi(rules: impl IntoIterator<Item = (Hir, TokenKind)>) -> Self {
        let mut global_builder = Builder::new();
        let global_start = global_builder.add_state();

        // Phase 1: compile each rule independently
        let mut sub_builders: Vec<(Builder, Fragment, TokenKind)> = Vec::new();
        for (hir, tk) in rules {
            let mut compiler = Compiler::new();
            let frag = compiler.compile_hir(&hir);
            sub_builders.push((compiler.into_builder(), frag, tk));
        }

        // Phase 2: calculate offsets for each sub-builder
        let mut offsets: Vec<usize> = Vec::new();
        let mut current_offset = global_builder.state_count();
        for (builder, _frag, _tk) in &sub_builders {
            offsets.push(current_offset);
            current_offset += builder.state_count();
        }

        // Phase 3: merge all sub-states with adjusted transitions
        let mut accept_states = HashMap::new();
        for (i, (builder, frag, tk)) in sub_builders.into_iter().enumerate() {
            let offset = offsets[i];
            let states = builder.into_states();

            // 3a: copy states to global builder
            for _state in &states {
                global_builder.add_state();
            }

            // 3b: re-add transitions with offset adjustment
            for state in &states {
                let adjusted_from = offset + state.id;

                for &eps_target in &state.epsilons {
                    global_builder.add_epsilon(adjusted_from, offset + eps_target);
                }

                for edge in &state.edges {
                    global_builder.add_edge(
                        adjusted_from,
                        edge.trans.clone(),
                        offset + edge.target,
                    );
                }
            }

            // 3c: epsilon from global start to sub-NFA start
            global_builder.add_epsilon(global_start, offset + frag.start);

            // 3d: record accept state
            accept_states.insert(offset + frag.end, tk);
        }

        let states = global_builder.into_states();
        NFA {
            states,
            start_state: global_start,
            accept_states,
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

    pub fn accept_states(&self) -> &HashMap<StateId, TokenKind> {
        &self.accept_states
    }

    pub fn is_accepting(&self, id: StateId) -> bool {
        self.accept_states.contains_key(&id)
    }

    pub fn token_kind(&self, id: StateId) -> Option<TokenKind> {
        self.accept_states.get(&id).copied()
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

/// NFA fragment used during compilation (re-exported from compiler)
pub use compiler::Fragment;

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
