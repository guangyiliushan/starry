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

pub mod minimize;

mod subset;

use std::fmt;

use ast::token::TokenKind;

use crate::display::write_escaped_char;
use crate::nfa::NFA;
use crate::state::{State, StateId};
use crate::transition::next_char;

/// 子集构造的状态数上限（指数爆炸防线，MAX_REPEAT 的 DFA 阶段对应物）
pub const MAX_DFA_STATES: usize = 65_536;

/// 虚拟死态：等价检查中的 total 化哨兵（测试专用，永不物化；
/// 最小化内部以 dead_i 槽位等价实现）
#[cfg(test)]
pub(crate) const DEAD: StateId = u32::MAX as usize;

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

    /// 最小化（默认 Hopcroft；另见 [`minimize`] 模块的 Moore / Brzozowski）
    pub fn minimize(&self) -> DFA {
        minimize::hopcroft(self)
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

    /// 字母表原子分区（最小化细化与等价检查的探测点来源）
    pub(crate) fn alphabet_atoms(&self) -> Vec<(char, char)> {
        let mut cuts = std::collections::BTreeSet::new();
        for s in &self.states {
            for &(lo, hi, _) in &s.edges {
                cuts.insert(lo);
                if let Some(succ) = crate::transition::next_char(hi) {
                    cuts.insert(succ);
                }
            }
        }
        cuts.insert(char::MAX); // 终末切点
        let cuts: Vec<char> = cuts.into_iter().collect();
        let mut atoms: Vec<(char, char)> = cuts
            .windows(2)
            .map(|w| {
                (
                    w[0],
                    crate::transition::prev_char(w[1]).expect("非最大切点必有前驱"),
                )
            })
            .collect();
        if let Some(&last) = cuts.last() {
            atoms.push((last, char::MAX));
        }
        atoms
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

    /// canonical 空语言 DFA：1 态非接受、无边
    ///
    /// （无边与自环 trap 语义等价；canonical 钉无边，确定性测试才不假失败）
    pub(crate) fn empty_language() -> DFA {
        DFA {
            states: vec![DfaState::from_edges(State::new(0), Vec::new())],
            start: 0,
        }
    }

    /// 删除无法到达接受态的死态（co-reachability 过滤），
    /// 保留原相对顺序重编号。brzozowski 输出后处理使用。
    pub(crate) fn prune_dead(&self) -> DFA {
        use std::collections::VecDeque;
        let n = self.state_count();
        let mut keep = vec![false; n];
        let mut queue = VecDeque::new();
        for i in 0..n {
            if self.is_accepting(i) {
                keep[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(t) = queue.pop_front() {
            for p in 0..n {
                if !keep[p] && self.edges(p).iter().any(|&(_, _, t2)| t2 == t && keep[t2]) {
                    keep[p] = true;
                    queue.push_back(p);
                }
            }
        }
        if !keep[self.start] {
            return DFA::empty_language();
        }
        let mut new_id = vec![usize::MAX; n];
        let mut states = Vec::new();
        for (i, s) in self.states.iter().enumerate() {
            if keep[i] {
                new_id[i] = states.len();
                let mut st = State::new(states.len());
                st.set_token_kind(s.state.token_kind);
                states.push(DfaState::from_edges(st, Vec::new()));
            }
        }
        for (i, s) in self.states.iter().enumerate() {
            if !keep[i] {
                continue;
            }
            let id = new_id[i];
            for &(lo, hi, t) in &s.edges {
                if keep[t] {
                    states[id]
                        .edges
                        .push((lo, hi, new_id[t]));
                }
            }
        }
        // 重排序合并（继承 smart constructor 的规范化不变量）
        let states: Vec<DfaState> = states
            .into_iter()
            .enumerate()
            .map(|(i, mut s)| {
                let rebuilt = DfaState::from_edges(s.state.clone(), std::mem::take(&mut s.edges));
                let mut st = State::new(i);
                st.set_token_kind(rebuilt.state.token_kind);
                DfaState::from_edges(st, rebuilt.edges)
            })
            .collect();
        let out = DFA {
            states,
            start: new_id[self.start],
        };
        #[cfg(debug_assertions)]
        out.assert_all_reachable();
        out
    }

    /// 仅供测试：手造 DFA（豁免可达性检查；正常路径只有 from_nfa/minimize）
    #[cfg(test)]
    pub(crate) fn raw(states: Vec<DfaState>, start: StateId) -> DFA {
        DFA { states, start }
    }
}

/// 最小化共用件：着色初始分割（按 `Option<TokenKind>`，DEAD 归 None 块）
pub(super) struct Block {
    pub(super) color: Option<TokenKind>,
    pub(super) members: Vec<usize>,
}

/// total 化目标函数：DEAD 槽自环，缺口落 DEAD 槽
pub(super) fn total_target(dfa: &DFA, dead_i: usize, q: usize, c: char) -> usize {
    if q == dead_i {
        dead_i
    } else {
        dfa.next_state(q, c).unwrap_or(dead_i)
    }
}

/// 着色初始分割：`Option<TokenKind>` 各一块（颜色是 lexer 的可观察行为，
/// 语言盲的 accepting 布尔分割会静默切错 token——ax/ay 反例测试守卫）
pub(super) fn initial_blocks(dfa: &DFA, dead_i: usize) -> (Vec<Block>, Vec<usize>) {
    let total = dead_i + 1;
    let mut blocks: Vec<Block> = Vec::new();
    let mut block_of = vec![0usize; total];
    for i in 0..total {
        let color = if i == dead_i { None } else { dfa.token_kind(i) };
        let b = blocks
            .iter()
            .position(|blk| blk.color == color)
            .unwrap_or_else(|| {
                blocks.push(Block {
                    color,
                    members: Vec::new(),
                });
                blocks.len() - 1
            });
        block_of[i] = b;
        blocks[b].members.push(i);
    }
    (blocks, block_of)
}

/// 由分块构建最小化 DFA：删除 sink 块（含 DEAD）及其入边；
/// start 落入 sink（⇔ 原语言为空）⇒ canonical 1 态非接受无边 DFA；
/// 幸存块按 min-member 重编号（粗稳定划分唯一 ⇒ 位级可复现）。
pub(super) fn build(dfa: &DFA, blocks: &[Block], block_of: &[usize], sink: usize) -> DFA {
    if block_of[dfa.start()] == sink {
        return DFA::empty_language();
    }
    let mut survivors: Vec<usize> = (0..blocks.len())
        .filter(|&b| b != sink && !blocks[b].members.is_empty())
        .collect();
    survivors.sort_by_key(|&b| *blocks[b].members.iter().min().unwrap());
    let mut new_id = vec![usize::MAX; blocks.len()];
    for (i, &b) in survivors.iter().enumerate() {
        new_id[b] = i;
    }
    let mut states = Vec::new();
    for (i, &b) in survivors.iter().enumerate() {
        let mut st = State::new(i);
        st.set_token_kind(dfa.token_kind(*blocks[b].members.iter().min().unwrap()));
        let mut e = Vec::new();
        for &p in &blocks[b].members {
            if p >= dfa.state_count() {
                continue; // DEAD 槽
            }
            for &(lo, hi, t) in dfa.edges(p) {
                let tb = block_of[t];
                if tb == sink {
                    continue; // 入边删除：续读永不接受
                }
                e.push((lo, hi, new_id[tb]));
            }
        }
        states.push(DfaState::from_edges(st, e));
    }
    let out = DFA {
        states,
        start: new_id[block_of[dfa.start()]],
    };
    #[cfg(debug_assertions)]
    out.assert_all_reachable();
    out
}

#[cfg(test)]
pub(crate) mod test_support {
    //! 测试基建：product 构造的着色等价检查（穷尽验证，替代采样）

    use std::collections::{BTreeSet, VecDeque};

    use ast::token::TokenKind;

    use crate::dfa::{DFA, DEAD};
    use crate::nfa::NFA;
    use crate::regex::parse::parse;
    use crate::regex::Translate;
    use crate::state::StateId;
    use crate::transition::next_char;

    pub(crate) fn nfa_from(pattern: &str) -> NFA {
        let ast = parse(pattern).unwrap();
        let hir = Translate::new().translate(&ast);
        NFA::from_hir(&hir, TokenKind::Identifier)
    }

    /// 断言两个 DFA 着色等价：每个可达对颜色 `Option<TokenKind>` 全等，
    /// 转移/DEAD 配对（双侧 DEAD 一致跳过）。DEAD 为虚拟吸收态。
    pub(crate) fn assert_colored_equivalent(a: &DFA, b: &DFA) {
        // 探测点 = 两侧切点集 {lo} ∪ {next_char(hi)} 的并集
        let mut cuts = BTreeSet::new();
        for dfa in [a, b] {
            for id in 0..dfa.state_count() {
                for &(lo, hi, _) in dfa.edges(id) {
                    cuts.insert(lo);
                    if let Some(succ) = next_char(hi) {
                        cuts.insert(succ);
                    }
                }
            }
        }
        let probes: Vec<char> = cuts.into_iter().collect();

        let color = |dfa: &DFA, s: StateId| -> Option<TokenKind> {
            if s == DEAD {
                None
            } else {
                dfa.token_kind(s)
            }
        };
        let step = |dfa: &DFA, s: StateId, c: char| -> StateId {
            if s == DEAD {
                DEAD
            } else {
                dfa.next_state(s, c).unwrap_or(DEAD)
            }
        };

        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        seen.insert((a.start(), b.start()));
        queue.push_back((a.start(), b.start()));

        while let Some((p, q)) = queue.pop_front() {
            assert_eq!(color(a, p), color(b, q), "着色不等价：状态对 (S{p}, S{q})");
            for &c in &probes {
                let p2 = step(a, p, c);
                let q2 = step(b, q, c);
                if (p2, q2) != (DEAD, DEAD) {
                    // 经其他探测路径重复到达同一对属正常，seen 去重即可
                    if seen.insert((p2, q2)) {
                        queue.push_back((p2, q2));
                    }
                }
            }
        }
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
