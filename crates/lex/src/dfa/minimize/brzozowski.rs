//! Brzozowski 算法：两次反转 + 两次确定化

use ast::token::TokenKind;

use crate::dfa::{DfaError, DFA};
use crate::nfa::{Builder, NFA};
use crate::transition::Transition;

/// Brzozowski 最小化：双反转 + 双确定化
///
/// - 输出为**语言最小** DFA（kind 盲）：接受态颜色统一为占位色
///   （输入单色则保持该色，多色统一 `TokenKind::Unknown`），原色不
///   保留——仅限语言识别实验，禁止作为 lexer 转导器使用；生产链路
///   用 [`DFA::minimize`](crate::dfa::DFA::minimize)（默认 hopcroft）。
/// - 中间确定化最坏指数膨胀（reverse 语言的状态复杂度可指数），
///   受状态数上限（crate::dfa::MAX_DFA_STATES）约束 → `Result` 签名
///   即结构事实。
/// - 细化算法（hopcroft/moore）结构上只缩不涨，保持 infallible——
///   签名不对称是诚实的。
pub fn brzozowski(dfa: &DFA) -> Result<DFA, DfaError> {
    let n1 = reverse(dfa);
    let d1 = DFA::from_nfa(&n1)?;
    let n2 = reverse(&d1);
    // 双反转的第二次确定化不做 sink 清理：输出可含可达死态
    // （黄金样例会多出 1 个死态），统一过 prune 后状态数才与
    // hopcroft/moore 可比
    Ok(DFA::from_nfa(&n2)?.prune_dead())
}

/// DFA 反转：边翻转，接受 = 原 start（kind 为占位色），新增 ε 起始态
/// ε 指向原全部接受态（原 start 非接受 ⇒ 反转不接受 ε，语言正确）。
pub(crate) fn reverse(dfa: &DFA) -> NFA {
    // 占位颜色：接受态颜色恰一种 → 保持；多种/无 → Unknown（kind 盲）
    let mut colors: Vec<TokenKind> = Vec::new();
    for i in 0..dfa.state_count() {
        if let Some(k) = dfa.token_kind(i) {
            if !colors.contains(&k) {
                colors.push(k);
            }
        }
    }
    let placeholder = if colors.len() == 1 { colors[0] } else { TokenKind::Unknown };

    let mut builder = Builder::new();
    for _ in 0..dfa.state_count() {
        builder.add_state();
    }
    let new_start = builder.add_state();
    for p in 0..dfa.state_count() {
        for &(lo, hi, t) in dfa.edges(p) {
            builder.add_edge(t, Transition::range(lo, hi), p);
        }
    }
    // 反转 NFA 的初始集 = 原 DFA 的全部接受态（经新起始态 ε 引入）
    for q in 0..dfa.state_count() {
        if dfa.is_accepting(q) {
            builder.add_epsilon(new_start, q);
        }
    }
    let mut accepting = vec![None; builder.state_count()];
    // 反转的 final 集 = {原 start}，无条件标记；ε 是否被接受由
    // 初始闭包是否包含它决定（s ∈ F ⟺ ε ∈ reverse(L)）
    accepting[dfa.start()] = Some(placeholder);
    NFA::new(new_start, builder.into_states(), accepting)
}
