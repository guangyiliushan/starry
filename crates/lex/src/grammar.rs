//! 正则文法（右线性文法，Chomsky Type-3）
//!
//! 与 NFA 的等价互转：非终结符 = NFA 状态（一一对应），产生式 =
//! 字符区间边（[`SymbolId`] intern 字母表），接受信息按非终结符存放
//! 在 `accept_kinds`。
//!
//! # ε-消除
//!
//! [`RegularGrammar::from_nfa`] 对输入 NFA 先做状态一一对应的 ε-消除
//! （Thompson NFA 是 ε-heavy 的，右线性文法装不下 ε 边）：
//!
//! - 边：`q—c→r ⇔ ∃q′ ∈ ε*(q)，q′—c→r`（闭包折进源点）；
//! - 接受：`q 接受 ⇔ ε*(q) ∩ F ≠ ∅`，kind 取闭包内最小 id 接受态
//!   （与子集法的 min-id 规则对称）。
//!
//! 状态 id 在消除后保持不变，`accept_kinds` 直接按下标映射。

use std::collections::BTreeMap;
use std::fmt;

use ast::token::TokenKind;

use crate::display::write_escaped_char;
use crate::nfa::NFA;
use crate::transition::Transition;

/// 非终结符标识（与 NFA 状态一一对应）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonTerminalId(pub usize);

/// 字母表符号（字符区间）标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub usize);

/// 右线性产生式（类型强制正则性：只有 ε 与 终结符区间·非终结符 两形式）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Production {
    /// A → ε（A 为接受态，颜色见 [`RegularGrammar`] 的 accept_kinds）
    Epsilon,
    /// A → t B（终结符区间 t，转移到非终结符 B）
    TerminalNonTerminal(SymbolId, NonTerminalId),
}

/// 正则文法
///
/// 不变量：`productions` 排序去重（canonical）；`alphabet` 按 intern
/// 顺序保存（构造序确定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegularGrammar {
    alphabet: Vec<(char, char)>,
    names: Vec<String>,
    start: NonTerminalId,
    accept_kinds: Vec<Option<TokenKind>>,
    productions: Vec<(NonTerminalId, Production)>,
}

impl RegularGrammar {
    /// 从 NFA 构建正则文法（先做状态一一对应的 ε-消除）
    pub fn from_nfa(nfa: &NFA) -> Self {
        let n = nfa.state_count();

        let mut symbols: BTreeMap<(char, char), SymbolId> = BTreeMap::new();
        let mut alphabet: Vec<(char, char)> = Vec::new();
        let mut productions: Vec<(NonTerminalId, Production)> = Vec::new();
        let mut accept_kinds = vec![None; n];

        for q in 0..n {
            let closure = nfa.epsilon_closure_of(q);
            for &q2 in &closure {
                for edge in &nfa.states()[q2].edges {
                    for interval in edge.trans.intervals() {
                        let sym = *symbols.entry(interval).or_insert_with(|| {
                            alphabet.push(interval);
                            SymbolId(alphabet.len() - 1)
                        });
                        productions.push((
                            NonTerminalId(q),
                            Production::TerminalNonTerminal(sym, NonTerminalId(edge.target)),
                        ));
                    }
                }
            }
            // 接受：闭包含接受态 → 最小 id 接受态的 kind（min-id 对称）
            let acc = closure.iter().filter(|&&s| nfa.is_accepting(s)).min();
            if let Some(&s) = acc {
                accept_kinds[q] = nfa.token_kind(s);
            }
        }

        productions.sort_unstable();
        productions.dedup();

        let mut names = Vec::with_capacity(n);
        for i in 0..n {
            names.push(format!("A{i}"));
        }

        Self {
            alphabet,
            names,
            start: NonTerminalId(nfa.start_state()),
            accept_kinds,
            productions,
        }
    }

    /// 转回 NFA：非终结符 → 状态，产生式 → 区间边，
    /// 接受态 = `accept_kinds[i].is_some()` 的状态
    pub fn to_nfa(&self) -> NFA {
        let mut builder = crate::nfa::Builder::new();
        for _ in &self.names {
            builder.add_state();
        }
        for (nt, prod) in &self.productions {
            if let Production::TerminalNonTerminal(sym, b) = prod {
                let (lo, hi) = self.alphabet[sym.0];
                builder.add_edge(nt.0, Transition::range(lo, hi), b.0);
            }
        }
        NFA::new(
            self.start.0,
            builder.into_states(),
            self.accept_kinds.clone(),
        )
    }

    pub fn start(&self) -> NonTerminalId {
        self.start
    }

    pub fn accept_kinds(&self) -> &[Option<TokenKind>] {
        &self.accept_kinds
    }

    pub fn productions(&self) -> &[(NonTerminalId, Production)] {
        &self.productions
    }

    pub fn alphabet(&self) -> &[(char, char)] {
        &self.alphabet
    }

    pub fn symbol_interval(&self, sym: SymbolId) -> (char, char) {
        self.alphabet[sym.0]
    }

    pub fn non_terminal_name(&self, nt: NonTerminalId) -> &str {
        &self.names[nt.0]
    }
}

impl fmt::Display for RegularGrammar {
    /// canonical 文法展示（产生式排序输出，区间经 escape_debug）
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (nt, prod) in &self.productions {
            write!(f, "{} -> ", self.names[nt.0])?;
            match prod {
                Production::Epsilon => write!(f, "ε")?,
                Production::TerminalNonTerminal(sym, b) => {
                    let (lo, hi) = self.alphabet[sym.0];
                    write!(f, "'")?;
                    write_escaped_char(f, lo)?;
                    if hi != lo {
                        write!(f, "-")?;
                        write_escaped_char(f, hi)?;
                    }
                    write!(f, "' ")?;
                    writeln!(f, "{}", self.names[b.0])?;
                }
            }
        }
        writeln!(f, "start: {}", self.names[self.start.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::test_support::{assert_colored_equivalent, nfa_from};
    use crate::dfa::DFA;
    use crate::regex::parse::parse;
    use crate::regex::Translate;
    use ast::token::{KeywordKind, TokenKind};

    fn dfa_of(nfa: &NFA) -> DFA {
        DFA::from_nfa(nfa).unwrap()
    }

    #[test]
    fn test_thompson_path_roundtrip() {
        // 全路径：parse → from_hir → N1 → 文法 → N2 → DFA2，
        // 与 DFA1 = from_nfa(N1) 做 product 着色等价对拍
        for pattern in ["(a|b)*abb", "[a-z]+", "(?i)abc", "a?b"] {
            let ast = parse(pattern).unwrap();
            let hir = Translate::new().translate(&ast);
            let n1 = NFA::from_hir(&hir, TokenKind::Identifier);
            let grammar = RegularGrammar::from_nfa(&n1);
            let n2 = grammar.to_nfa();
            assert_colored_equivalent(&dfa_of(&n1), &dfa_of(&n2));
        }
    }

    #[test]
    fn test_heterogeneous_kinds_survive_roundtrip() {
        // 异质 kind：ax→Identifier, ay→Keyword(If) 在文法往返后保留
        let ast1 = parse("ax").unwrap();
        let ast2 = parse("ay").unwrap();
        let hir1 = Translate::new().translate(&ast1);
        let hir2 = Translate::new().translate(&ast2);
        let nfa = NFA::from_hir_multi(vec![
            (hir1, TokenKind::Identifier),
            (hir2, TokenKind::Keyword(KeywordKind::If)),
        ]);
        let grammar = RegularGrammar::from_nfa(&nfa);
        let has_ident = grammar
            .accept_kinds()
            .iter()
            .any(|k| *k == Some(TokenKind::Identifier));
        let has_if = grammar
            .accept_kinds()
            .iter()
            .any(|k| *k == Some(TokenKind::Keyword(KeywordKind::If)));
        assert!(has_ident && has_if);

        let dfa = grammar.to_nfa();
        let dfa = DFA::from_nfa(&dfa).unwrap();
        assert_eq!(dfa.longest_match("ax"), Some((2, TokenKind::Identifier)));
        assert_eq!(
            dfa.longest_match("ay"),
            Some((2, TokenKind::Keyword(KeywordKind::If)))
        );
    }

    #[test]
    fn test_productions_canonical() {
        let nfa = nfa_from("(a|b)*abb");
        let g1 = RegularGrammar::from_nfa(&nfa);
        let g2 = RegularGrammar::from_nfa(&nfa);
        // 同输入 ⇒ 位级相同（canonical）
        assert_eq!(g1, g2);
        assert_eq!(g1.to_string(), g2.to_string());
    }

    #[test]
    fn test_epsilon_only_start_roundtrip() {
        // ε-语言：NFA.start 自身接受（kind），文法以 Epsilon 产生式表达
        let hir = crate::regex::Hir::Empty;
        let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
        let grammar = RegularGrammar::from_nfa(&nfa);
        assert!(grammar
            .accept_kinds()
            .iter()
            .any(|k| *k == Some(TokenKind::Identifier)));
        let back = grammar.to_nfa();
        assert!(back.is_accepting(back.start_state()));
    }
}
