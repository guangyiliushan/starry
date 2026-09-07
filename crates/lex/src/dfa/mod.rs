//! DFA 模块：确定性有限自动机
//!
//! 边标签为**排序、不相交的字符区间** `Vec<(char, char, StateId)>`——补集
//! 字符类使字符域不可枚举，区间化是可行性而非风格选择；转移查找用
//! `partition_point` 二分，O(log n)。
//!
//! 接受信息单点住 [`State::token_kind`](crate::state::State)（state.rs 的
//! DFA 层定位由此兑现，[`StateGenerator`](crate::state::StateGenerator)
//! 由子集法用于状态分配）。
//!
//! # 确定性
//!
//! canonical Display（边按 (lo, hi) → target 升序）⇒ 同输入位级相同的 dump。

mod subset;

use std::fmt;

use ast::token::TokenKind;

use crate::display::write_escaped_char;
use crate::nfa::NFA;
use crate::state::{State, StateId};
use crate::transition::next_char;

/// 子集构造的状态数上限（指数爆炸防线，MAX_REPEAT 的 DFA 阶段对应物）
pub const MAX_DFA_STATES: usize = 65_536;

/// 子集构造/最小化的错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DfaError {
    /// 子集构造的状态数超过 [`MAX_DFA_STATES`]
    StateLimitExceeded { limit: usize },
}

impl fmt::Display for DfaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DfaError::StateLimitExceeded { limit } => {
                write!(f, "子集构造状态数超过上限 {limit}（语言的 DFA 可能指数膨胀）")
            }
        }
    }
}

impl std::error::Error for DfaError {}

/// 确定性有限自动机
///
/// 偏函数形式：区间之间的缺口即隐式死态（无显式死状态）；转移缺失由
/// [`DFA::next_state`] 返回 `None` 表达。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DFA {
    states: Vec<DfaState>,
    start: StateId,
}

/// DFA 状态：[`State`]（携带接受信息）+ 排序不相交的区间边
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DfaState {
    state: State,
    edges: Vec<(char, char, StateId)>,
}

impl DfaState {
    /// smart constructor：排序 + 不相交 + 同目标相邻（码点相邻或跨代理区
    /// 缺口）合并
    pub(crate) fn from_edges(state: State, mut edges: Vec<(char, char, StateId)>) -> Self {
        edges.sort_unstable();
        debug_assert!(
            edges.iter().all(|&(lo, hi, _)| lo <= hi),
            "DFA 边区间起点不能大于终点"
        );
        let mut merged: Vec<(char, char, StateId)> = Vec::with_capacity(edges.len());
        for (lo, hi, target) in edges {
            match merged.last_mut() {
                Some((_, last_hi, last_t))
                    if *last_t == target
                        && (*last_hi >= lo || next_char(*last_hi) == Some(lo)) =>
                {
                    if hi > *last_hi {
                        *last_hi = hi;
                    }
                }
                _ => merged.push((lo, hi, target)),
            }
        }
        Self { state, edges: merged }
    }

    /// c 命中的区间边（至多一个：有序 + 不相交）
    fn matches_char(&self, c: char) -> Option<StateId> {
        let i = self.edges.partition_point(|&(lo, _, _)| lo <= c);
        let &(lo, hi, target) = self.edges.get(i.checked_sub(1)?)?;
        (lo <= c && c <= hi).then_some(target)
    }
}

impl DFA {
    /// 子集法入口：NFA → DFA（龙书 §3.6.4）
    pub fn from_nfa(nfa: &NFA) -> Result<DFA, DfaError> {
        subset::construct(nfa)
    }

    pub fn start(&self) -> StateId {
        self.start
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn is_accepting(&self, id: StateId) -> bool {
        self.states[id].state.is_accepting()
    }

    pub fn token_kind(&self, id: StateId) -> Option<TokenKind> {
        self.states[id].state.token_kind
    }

    /// 状态的区间边（排序、不相交、同目标相邻已合并）
    pub fn edges(&self, id: StateId) -> &[(char, char, StateId)] {
        &self.states[id].edges
    }

    /// 确定性转移：c 命中区间边则返回目标，区间间缺口返回 `None`
    pub fn next_state(&self, id: StateId, c: char) -> Option<StateId> {
        self.states.get(id)?.matches_char(c)
    }

    /// 最长匹配：返回最长接受前缀的**字节长度**
    pub fn match_prefix(&self, input: &str) -> Option<usize> {
        self.longest_match(input).map(|(len, _)| len)
    }

    /// 最长匹配：返回（字节长度，接受态颜色）
    pub fn longest_match(&self, input: &str) -> Option<(usize, TokenKind)> {
        let mut current = self.start;
        let mut best = self.states[current]
            .state
            .token_kind
            .map(|kind| (0usize, kind));
        for (offset, c) in input.char_indices() {
            match self.next_state(current, c) {
                Some(next) => {
                    current = next;
                    if let Some(kind) = self.states[current].state.token_kind {
                        best = Some((offset + c.len_utf8(), kind));
                    }
                }
                None => break,
            }
        }
        best
    }

    #[cfg(debug_assertions)]
    fn assert_all_reachable(&self) {
        let mut seen = vec![false; self.states.len()];
        seen[self.start] = true;
        let mut stack = vec![self.start];
        while let Some(p) = stack.pop() {
            for &(_, _, t) in &self.states[p].edges {
                if !seen[t] {
                    seen[t] = true;
                    stack.push(t);
                }
            }
        }
        debug_assert!(seen.iter().all(|&s| s), "DFA 存在不可达状态");
    }
}

impl fmt::Display for DFA {
    /// 确定性 dump：边按 (lo, hi) → target 升序，字符经 escape_debug
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, state) in self.states.iter().enumerate() {
            for &(lo, hi, target) in &state.edges {
                write!(f, "S{id} --")?;
                write_escaped_char(f, lo)?;
                if hi != lo {
                    write!(f, "-")?;
                    write_escaped_char(f, hi)?;
                }
                writeln!(f, "--> S{target}")?;
            }
        }
        let tag = if self.is_accepting(self.start) {
            " (accepting)"
        } else {
            ""
        };
        write!(f, "start: S{}{tag}", self.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nfa::NFA;
    use crate::regex::parse::parse;
    use crate::regex::Translate;

    fn nfa_from(pattern: &str) -> NFA {
        let ast = parse(pattern).unwrap();
        let hir = Translate::new().translate(&ast);
        NFA::from_hir(&hir, TokenKind::Identifier)
    }

    fn dfa_from(pattern: &str) -> DFA {
        DFA::from_nfa(&nfa_from(pattern)).unwrap()
    }

    #[test]
    fn test_golden_subset_states() {
        // 龙书 §3.6 经典样例 (a|b)*abb：子集构造 5 态
        assert_eq!(dfa_from("(a|b)*abb").state_count(), 5);
    }

    #[test]
    fn test_deterministic_dump() {
        let d1 = dfa_from("(a|b)*abb").to_string();
        let d2 = dfa_from("(a|b)*abb").to_string();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_cut_point_includes_successor() {
        // 切点 = {lo} ∪ {next_char(hi)}：区间闭端必须被原子覆盖
        let dfa = dfa_from("[a-z]");
        for c in 'a'..='z' {
            assert_eq!(dfa.match_prefix(&c.to_string()), Some(1), "字符 {c}");
        }
        assert_eq!(dfa.match_prefix("{"), None);

        // 切点反例（(a,z) vs (a,b),(d,z)）：原子不得跨界
        let dfa = dfa_from("[ab]|[d-z]");
        assert_eq!(dfa.match_prefix("a"), Some(1));
        assert_eq!(dfa.match_prefix("b"), Some(1));
        assert_eq!(dfa.match_prefix("c"), None);
        assert_eq!(dfa.match_prefix("d"), Some(1));
        assert_eq!(dfa.match_prefix("z"), Some(1));
    }

    #[test]
    fn test_negated_class() {
        let dfa = dfa_from("[^a]");
        assert_eq!(dfa.match_prefix("b"), Some(1));
        assert_eq!(dfa.match_prefix("a"), None);
        // 非 ASCII（含代理区邻域）不 panic
        assert_eq!(dfa.match_prefix("é"), Some(2));
        assert_eq!(dfa.match_prefix("\u{E000}"), Some(3));
    }

    #[test]
    fn test_case_insensitive_dfa() {
        let dfa = dfa_from("(?i)abc");
        assert_eq!(dfa.match_prefix("aBc"), Some(3));
    }

    #[test]
    fn test_state_limit() {
        // 语言的最小 DFA 指数膨胀：BFS 秒级触帽返回 Err
        let err = DFA::from_nfa(&nfa_from("(a|b)*a(a|b){999}")).unwrap_err();
        assert_eq!(
            err,
            DfaError::StateLimitExceeded {
                limit: MAX_DFA_STATES
            }
        );
    }

    #[test]
    fn test_nfa_dfa_equivalence_byte_semantics() {
        let patterns = ["(a|b)*abb", "[a-z]+", "(?i)abc", "a?b", "[^a]"];
        let inputs = [
            "", "a", "b", "ab", "abb", "aabb", "abc", "ABC", "aBc", "é", "\u{E000}", "🦀", "zzz",
        ];
        for pattern in patterns {
            let nfa = nfa_from(pattern);
            let dfa = DFA::from_nfa(&nfa).unwrap();
            for input in inputs {
                assert_eq!(
                    nfa.match_prefix(input),
                    dfa.match_prefix(input),
                    "pattern {pattern:?} input {input:?}"
                );
            }
        }
        // 字节语义：非 ASCII 的长度按字节计（é = 2 字节）
        assert_eq!(dfa_from("é").match_prefix("é"), Some(2));
    }

    #[test]
    fn test_empty_language_class_is_single_state() {
        // 补集为空：无语义区间 → 无原子 → 1 态非接受无边
        let dfa = dfa_from("[^\u{0}-\u{10FFFF}]");
        assert_eq!(dfa.state_count(), 1);
        assert!(dfa.edges(0).is_empty());
        assert!(!dfa.is_accepting(0));
        assert_eq!(dfa.match_prefix("a"), None);
    }

    #[test]
    fn test_longest_match_color() {
        let dfa = dfa_from("ab");
        assert_eq!(
            dfa.longest_match("ab"),
            Some((2, TokenKind::Identifier))
        );
        assert_eq!(dfa.longest_match("a"), None);
    }
}
