//! lexer 驱动：token 流生成
//!
//! - 规则 = 正则字符串 + [`TokenKind`]；ε-匹配规则在 build 期拒绝
//!   （`Some(0)` 驱动的 tokenize 死循环防线，flex 同款）；
//! - 关键字不进 NFA：ident 规则的 lexeme 经 `keyword::lookup_keyword`
//!   **后分类**（教科书做法，避免 DFA 膨胀与查找逻辑重复）；
//! - 空白集 `{' ', '\t', '\r', '\n'}` **先于匹配尝试**跳过（规则永远
//!   无法认领空白）；
//! - 未知字符按字符推进（`len_utf8`），产出 [`TokenKind::Unknown`]
//!   单字符 token（错误恢复，不 panic）。

use std::fmt;

use ast::token::Token;
use ast::token::lookup_keyword;
use ast::token::TokenKind;

use crate::dfa::{DfaError, DFA};
use crate::nfa::NFA;
use crate::regex::parse::parse;
use crate::regex::parse::ParseError;
use crate::regex::Translate;
use ast::token::Span;

/// lexer 构建错误：正则解析、DFA 上限、ε-规则三种座位
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexerError {
    /// 规则正则解析失败
    Parse(ParseError),
    /// 子集构造触达状态上限
    Dfa(DfaError),
    /// 规则匹配空串（ε ∈ L(rule)）——会卡死 tokenize
    RuleMatchesEmpty { pattern: Box<str> },
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexerError::Parse(e) => write!(f, "规则正则解析失败: {e}"),
            LexerError::Dfa(e) => write!(f, "DFA 构建失败: {e}"),
            LexerError::RuleMatchesEmpty { pattern } => write!(
                f,
                "规则 {pattern:?} 匹配空串（ε ∈ L），会卡死 tokenize，请改写"
            ),
        }
    }
}

impl std::error::Error for LexerError {}

impl From<ParseError> for LexerError {
    fn from(e: ParseError) -> Self {
        LexerError::Parse(e)
    }
}

impl From<DfaError> for LexerError {
    fn from(e: DfaError) -> Self {
        LexerError::Dfa(e)
    }
}

/// lexer：多规则最长匹配 token 流生成器
///
/// 规则优先级 = 注册顺序（子集法的 min-id 规则）；最长匹配优先于
/// 规则顺序（"ab" 命中 ab 规则而非 a 规则）；同长才看规则序。
#[derive(Debug, Clone)]
pub struct Lexer {
    dfa: DFA,
}

impl Lexer {
    /// 由（正则字符串，TokenKind）规则表构建 lexer
    ///
    /// 每条规则先单独检查 ε-匹配（build 期拒绝，错误携带规则原文），
    /// 再合并、子集法、着色最小化。
    pub fn build(rules: Vec<(&str, TokenKind)>) -> Result<Lexer, LexerError> {
        let mut hirs = Vec::new();
        for (pattern, kind) in &rules {
            let ast = parse(pattern)?;
            let hir = Translate::new().translate(&ast);
            let nfa = NFA::from_hir(&hir, *kind);
            if nfa.matches_epsilon() {
                return Err(LexerError::RuleMatchesEmpty {
                    pattern: (*pattern).into(),
                });
            }
            hirs.push((hir, *kind));
        }
        let nfa = NFA::from_hir_multi(hirs);
        let dfa = DFA::from_nfa(&nfa)?.minimize();
        Ok(Lexer { dfa })
    }

    pub fn dfa(&self) -> &DFA {
        &self.dfa
    }

    /// tokenize：空白跳过 + 最长匹配 + 关键字后分类
    ///
    /// 空白先于匹配尝试跳过（规则永远无法认领空白）；未命中字符按
    /// **字符**推进（`len_utf8`，多字节安全），产出
    /// [`TokenKind::Unknown`] 单字符 token。
    pub fn tokenize(&self, source: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut i = 0usize;
        while i < source.len() {
            let rest = &source[i..];
            let first = match rest.chars().next() {
                Some(c) => c,
                None => break,
            };
            // 空白先于匹配尝试
            if matches!(first, ' ' | '\t' | '\r' | '\n') {
                i += first.len_utf8();
                continue;
            }
            match self.dfa.longest_match(rest) {
                Some((len, mut kind)) if len > 0 => {
                    let end = i + len;
                    debug_assert!(end <= u32::MAX as usize);
                    // 关键字后分类：ident lexeme 查表
                    if kind == TokenKind::Identifier {
                        if let Some(kw) = lookup_keyword(&source[i..end]) {
                            kind = TokenKind::Keyword(kw);
                        }
                    }
                    tokens.push(Token::new(
                        kind,
                        &source[i..end],
                        Span::new(i as u32, end as u32),
                    ));
                    i = end;
                }
                _ => {
                    // 未知字符：按字符推进（i 必为字符边界）
                    let end = i + first.len_utf8();
                    debug_assert!(end <= u32::MAX as usize);
                    tokens.push(Token::new(
                        TokenKind::Unknown,
                        &source[i..end],
                        Span::new(i as u32, end as u32),
                    ));
                    i = end;
                }
            }
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::token::LiteralKind;
    use ast::token::KeywordKind;

    #[test]
    fn test_build_rejects_epsilon_rules() {
        for pattern in ["a*", "a?"] {
            let ast = parse(pattern).unwrap();
            let hir = Translate::new().translate(&ast);
            let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
            eprintln!(
                "{pattern}: states={} eps={}",
                nfa.state_count(),
                nfa.matches_epsilon()
            );
            for i in 0..nfa.state_count() {
                for edge in &nfa.states()[i].edges {
                    eprintln!("  S{i} --{:?}--> S{}", edge.trans, edge.target);
                }
            }
        }
        // 匹配 ε（build → Err）
        for pattern in ["a*", "a?", "(a|)", "a{0,5}"] {
            let err = Lexer::build(vec![(pattern, TokenKind::Identifier)]).unwrap_err();
            assert_eq!(
                err,
                LexerError::RuleMatchesEmpty {
                    pattern: pattern.into()
                },
                "{pattern}"
            );
        }
        // 负对照：含可选段但不匹配 ε → Ok（防"含可选段 ⇒ 拒绝"误实现）
        assert!(Lexer::build(vec![("a?b", TokenKind::Identifier)]).is_ok());
    }

    #[test]
    fn test_tokenize_ident_number_keyword() {
        let lexer = Lexer::build(vec![
            ("[a-zA-Z_][a-zA-Z0-9_]*", TokenKind::Identifier),
            ("[0-9]+", TokenKind::Literal(LiteralKind::Integer)),
        ])
        .unwrap();

        // 关键字后分类：if → Keyword(If)；iffy → Identifier（最长匹配 + 查表）
        let tokens = lexer.tokenize("if iffy x1 42");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Keyword(KeywordKind::If));
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::Identifier);
        assert_eq!(tokens[3].kind, TokenKind::Literal(LiteralKind::Integer));
        // exclusive span：end - start = lexeme 字节数
        assert_eq!((tokens[0].span.start, tokens[0].span.end), (0, 2));
        assert_eq!((tokens[1].span.start, tokens[1].span.end), (3, 7));
        assert_eq!((tokens[2].span.start, tokens[2].span.end), (8, 10));
        assert_eq!((tokens[3].span.start, tokens[3].span.end), (11, 13));
    }

    #[test]
    fn test_unknown_char_advances_by_char() {
        // 多字节未知字符：按字符推进，不 panic、不切碎
        let lexer = Lexer::build(vec![("if", TokenKind::Keyword(KeywordKind::If))]).unwrap();
        let tokens = lexer.tokenize("🦀if");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Unknown);
        assert_eq!(&*tokens[0].lexeme, "🦀");
        assert_eq!((tokens[0].span.start, tokens[0].span.end), (0, 4));
        assert_eq!(tokens[1].kind, TokenKind::Keyword(KeywordKind::If));
        assert_eq!((tokens[1].span.start, tokens[1].span.end), (4, 6));
    }
}
