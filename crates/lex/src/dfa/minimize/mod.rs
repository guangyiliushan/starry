//! 最小化模块：着色最小化（颜色是 lexer 的可观察行为）
//!
//! - [`hopcroft`]：经典机制（state→block 成员索引、按 splitter 计数分裂、
//!   smaller-half 入队 + 等长 min id tie-break），O(|Σ|·n·log n)，
//!   |Σ| = 原子类数。
//! - [`moore`]：不动点轮次细化，O(|Σ|·n²)。
//! - [`brzozowski`]：两次反转 + 两次确定化。中间确定化最坏指数膨胀
//!   （reverse 语言的状态复杂度可指数），且 kind 盲——仅限语言识别
//!   实验；生产链路用 [`DFA::minimize`](crate::dfa::DFA::minimize)
//!   （默认 hopcroft）。
//!
//! hopcroft/moore 输出统一过 sink-prune（删除 sink 块及其入边；空语言
//! 降级为 canonical 1 态）；可复现性：粗稳定划分唯一（与 worklist 顺序
//! 无关），输出块按 min-member 重编号 ⇒ 位级可复现。

pub mod brzozowski;
pub mod hopcroft;
pub mod moore;

use super::{build, initial_blocks, total_target, Block};

pub use brzozowski::brzozowski;
pub use hopcroft::hopcroft;
pub use moore::moore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::test_support::{assert_colored_equivalent, nfa_from};
    use crate::dfa::DFA;
    use crate::regex::parse::parse;
    use crate::regex::Translate;
    use ast::token::{KeywordKind, TokenKind};

    fn dfa_from(pattern: &str) -> DFA {
        DFA::from_nfa(&nfa_from(pattern)).unwrap()
    }

    fn build(rules: &[(&str, TokenKind)]) -> DFA {
        let hirs: Vec<_> = rules
            .iter()
            .map(|(p, k)| {
                let ast = parse(p).unwrap();
                let hir = Translate::new().translate(&ast);
                (hir, *k)
            })
            .collect();
        DFA::from_nfa(&crate::nfa::NFA::from_hir_multi(hirs)).unwrap()
    }

    #[test]
    fn test_colored_minimal_ax_ay() {
        // ax/ay 反例：语言盲最小化会把 ay 并进 K1 静默切错 token；
        // 着色最小化后 "ay" 仍产出 K2
        let dfa = build(&[
            ("ax", TokenKind::Identifier),
            ("ay", TokenKind::Keyword(KeywordKind::If)),
        ]);
        let min = dfa.minimize();
        assert_eq!(
            min.longest_match("ay"),
            Some((2, TokenKind::Keyword(KeywordKind::If)))
        );
        assert_eq!(min.longest_match("ax"), Some((2, TokenKind::Identifier)));
    }

    #[test]
    fn test_dead_end_shrink_raw() {
        // raw：P—a→Q—b→R(acc) 附加死枝 P—c→X（X 无出边非接受）
        // prune 删除 X 及入边 P—c；P/Q/R 保留；语言不变
        // （管道输入 ab 因"空集不登记"永不产死端——平凡假绿，不作覆盖来源）
        use crate::dfa::DfaState;
        use crate::state::State;

        let st = |i: usize| State::new(i);
        let p = DfaState::from_edges(st(0), vec![('a', 'a', 1), ('c', 'c', 3)]);
        let q = DfaState::from_edges(st(1), vec![('b', 'b', 2)]);
        let r = DfaState::from_edges(State::accepting(2, TokenKind::Identifier), Vec::new());
        let x = DfaState::from_edges(st(3), Vec::new());
        let raw = DFA::raw(vec![p, q, r, x], 0);

        let min = raw.minimize();
        assert_eq!(min.state_count(), 3);
        assert_eq!(min.edges(0), &[('a', 'a', 1)]);
        assert_eq!(min.longest_match("ab"), Some((2, TokenKind::Identifier)));
        assert_eq!(min.match_prefix("ac"), None);
        assert_colored_equivalent(&raw, &min);
    }

    #[test]
    fn test_empty_language_real_path() {
        // 真实路径：补集为空 → 1 态非接受无边（降级规则生效）
        let dfa = dfa_from("[^\u{0}-\u{10FFFF}]");
        let min = dfa.minimize();
        assert_eq!(min.state_count(), 1);
        assert!(min.edges(0).is_empty());
        assert!(!min.is_accepting(0));
        assert_eq!(min.match_prefix("a"), None);
    }

    #[test]
    fn test_golden_minimized_states() {
        // 黄金样例：最小化 4 态
        assert_eq!(dfa_from("(a|b)*abb").minimize().state_count(), 4);
    }

    #[test]
    fn test_three_algorithms_homogeneous() {
        // 同质 kind：hopcroft↔moore 状态数相等 + 着色 product；
        // brzozowski 无色占位（kind 盲）→ 仅长度对拍
        // （brz 输出可能非状态最小：双反转经非最小中间 DFA 保留等价对）
        let dfa = dfa_from("(a|b)*abb");
        let h = hopcroft(&dfa);
        let m = moore(&dfa);
        let b = brzozowski(&dfa).unwrap();

        assert_eq!(h.state_count(), 4);
        assert_eq!(h.state_count(), m.state_count());
        assert_colored_equivalent(&h, &m);
        for input in ["", "a", "b", "ab", "abb", "aabb", "bba"] {
            assert_eq!(
                h.match_prefix(input),
                b.match_prefix(input),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn test_three_algorithms_heterogeneous() {
        // 异质 kind：hopcroft↔moore 着色 product；brzozowski 长度对拍
        // + 状态数 ≤ hopcroft（颜色区分的代价，ax/ay 的定量呈现）
        let dfa = build(&[
            ("ax", TokenKind::Identifier),
            ("ay", TokenKind::Keyword(KeywordKind::If)),
        ]);
        let h = hopcroft(&dfa);
        let m = moore(&dfa);
        let b = brzozowski(&dfa).unwrap();

        assert_colored_equivalent(&h, &m);
        assert!(b.state_count() <= h.state_count());
        for input in ["", "a", "x", "ax", "ay", "y"] {
            assert_eq!(
                h.match_prefix(input),
                b.match_prefix(input),
                "input {input:?}"
            );
        }
    }
}
