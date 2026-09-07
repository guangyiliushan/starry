//! 子集法（龙书 §3.6.4）
//!
//! # 字母表原子分区
//!
//! 切点集 = 每条 NFA 边语义区间（[`Transition::intervals`]）的
//! `{lo} ∪ {next_char(hi)}`，外加终末切点 `char::MAX`；相邻切点构成原子
//! `[c_k, c_{k+1})`，探针取首字符 `c_k`——原子内任意 NFA 边真值恒定。
//!
//! 只用 `{lo, hi}` 作切点是错的：原子会跨过区间的闭端（反例：边 `(a,z)`
//! 与 `(a,b),(d,z)`，切点 `{a,b,d,z}` 产生的原子 `[b,c]` 内 `(a,b)` 真值
//! 不恒定），子集法会接受非法转移。
//!
//! 注意：若 [`Transition`](crate::transition::Transition) 引入否定标志
//! 形态，语义区间须先物化为补集，不能直接使用内部表示
//! （`[^a]` 测试守卫此前提）。
//!
//! # 无显式死状态
//!
//! **move 结果为空集 ⇒ 不登记状态、不发边。** 区间之间的缺口即隐式死态，
//! 由 [`DFA::next_state`](crate::dfa::DFA) 返回 `None` 表达。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ast::token::TokenKind;

use crate::dfa::{DfaError, DfaState, DFA, MAX_DFA_STATES};
use crate::nfa::NFA;
use crate::state::{State, StateGenerator, StateId};
use crate::transition::{next_char, prev_char};

pub(super) fn construct(nfa: &NFA) -> Result<DFA, DfaError> {
    let atoms = alphabet_atoms(nfa);

    let mut generator = StateGenerator::new();
    let mut colors: Vec<Option<TokenKind>> = Vec::new();
    let mut edges: Vec<Vec<(char, char, StateId)>> = Vec::new();
    let mut set_ids: BTreeMap<BTreeSet<StateId>, StateId> = BTreeMap::new();
    let mut queue: VecDeque<(StateId, BTreeSet<StateId>)> = VecDeque::new();

    let start = intern(
        nfa,
        &mut generator,
        &mut colors,
        &mut edges,
        &mut set_ids,
        &mut queue,
        nfa.epsilon_closure_of(nfa.start_state()),
    )?;

    while let Some((current_id, current)) = queue.pop_front() {
        for &(lo, hi) in &atoms {
            // move：探针 = 原子首字符（原子内真值恒定）
            let mut moved = BTreeSet::new();
            for &q in &current {
                for edge in &nfa.states()[q].edges {
                    if edge.trans.matches(lo) {
                        moved.insert(edge.target);
                    }
                }
            }
            // 空集不登记状态、不发边（无显式死状态）
            if moved.is_empty() {
                continue;
            }
            let closure = nfa.epsilon_closure(&moved);
            let target = match set_ids.get(&closure) {
                Some(&id) => id,
                None => intern(
                    nfa,
                    &mut generator,
                    &mut colors,
                    &mut edges,
                    &mut set_ids,
                    &mut queue,
                    closure,
                )?,
            };
            edges[current_id].push((lo, hi, target));
        }
    }

    let states: Vec<DfaState> = colors
        .into_iter()
        .zip(edges)
        .enumerate()
        .map(|(i, (color, e))| {
            let mut st = State::new(i);
            st.set_token_kind(color);
            DfaState::from_edges(st, e)
        })
        .collect();

    let dfa = DFA { states, start };
    #[cfg(debug_assertions)]
    dfa.assert_all_reachable();
    Ok(dfa)
}

/// 登记一个 NFA 状态集合为 DFA 状态（去重 + 上限检查）
///
/// 接受颜色取集合内**最小 NFA 状态 id** 的接受态——即规则添加顺序的
/// 首规则优先（龙书 §3.9.6 规则优先级）。
fn intern(
    nfa: &NFA,
    generator: &mut StateGenerator,
    colors: &mut Vec<Option<TokenKind>>,
    edges: &mut Vec<Vec<(char, char, StateId)>>,
    set_ids: &mut BTreeMap<BTreeSet<StateId>, StateId>,
    queue: &mut VecDeque<(StateId, BTreeSet<StateId>)>,
    set: BTreeSet<StateId>,
) -> Result<StateId, DfaError> {
    if let Some(&id) = set_ids.get(&set) {
        return Ok(id);
    }
    if colors.len() >= MAX_DFA_STATES {
        return Err(DfaError::StateLimitExceeded {
            limit: MAX_DFA_STATES,
        });
    }
    let id = generator.next();
    let color = set.iter().find_map(|&s| nfa.token_kind(s));
    colors.push(color);
    edges.push(Vec::new());
    set_ids.insert(set.clone(), id);
    queue.push_back((id, set));
    Ok(id)
}

/// 字母表原子分区：切点集 `{lo} ∪ {next_char(hi)}` ∪ {char::MAX}
fn alphabet_atoms(nfa: &NFA) -> Vec<(char, char)> {
    let mut cuts: BTreeSet<char> = BTreeSet::new();
    for state in nfa.states() {
        for edge in &state.edges {
            for (lo, hi) in edge.trans.intervals() {
                cuts.insert(lo);
                if let Some(succ) = next_char(hi) {
                    cuts.insert(succ);
                }
            }
        }
    }
    // 终末切点：尾原子覆盖到 Unicode 末尾（hi = char::MAX 的区间无后继切点）
    cuts.insert(char::MAX);
    let cuts: Vec<char> = cuts.into_iter().collect();
    let mut atoms: Vec<(char, char)> = cuts
        .windows(2)
        .map(|w| (w[0], prev_char(w[1]).expect("非最大切点必有前驱")))
        .collect();
    if let Some(&last) = cuts.last() {
        atoms.push((last, char::MAX));
    }
    atoms
}

