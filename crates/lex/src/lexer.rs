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
use ast::token::TokenKind;
use ast::token::lookup_keyword;

use crate::dfa::{DFA, DfaError};
use crate::nfa::NFA;
use crate::regex::Translate;
use crate::regex::parse::ParseError;
use crate::regex::parse::parse;
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

    /// 构建冻结的 Starry 全量 lexer。
    pub fn starry() -> Result<Lexer, LexerError> {
        Self::build(Self::starry_rules())
    }

    /// Starry v0.3 全套词法规则表（操作符 + 标点 + 字面量）
    ///
    /// 冲突组由 DFA 最长匹配自动择优（机械推导 + 人工示例双验证）。
    /// 关键字不在规则表内——ident 规则 + lookup_keyword 后分类。
    pub fn starry_rules() -> Vec<(&'static str, TokenKind)> {
        use ast::token::{LiteralKind, OperatorKind, PunctuationKind};
        vec![
            // 标识符与数字
            ("[a-zA-Z_][a-zA-Z0-9_]*", TokenKind::Identifier),
            (
                "0[xX][0-9a-fA-F][_0-9a-fA-F]*|0[oO][0-7][_0-7]*|0[bB][01][_01]*|[0-9][_0-9]*",
                TokenKind::Literal(LiteralKind::Integer),
            ),
            (
                "[0-9][_0-9]*\\.[0-9][_0-9]*([eE][+-]?[0-9]+)?|[0-9]+[eE][+-]?[0-9]+",
                TokenKind::Literal(LiteralKind::Float),
            ),
            // 字符串（含插值 raw 文本，转义解码归 parser 子扫描）
            (
                "\"(\\\\[^\\r\\n]|[^\"\\\\\\r\\n])*\"",
                TokenKind::Literal(LiteralKind::String),
            ),
            // 字符
            (
                "'(\\\\u\\{[0-9a-fA-F]{1,6}\\}|\\\\[^\\r\\n]|[^'\\\\\\r\\n])'",
                TokenKind::Literal(LiteralKind::Char),
            ),
            // 标签（Rust 式 'ident；'a' 仍为字符字面量，最长匹配自动择优）
            ("'[a-zA-Z_][a-zA-Z0-9_]*", TokenKind::Label),
            // 复合赋值（必须在单字符 = 之前）
            ("\\+=", TokenKind::Operator(OperatorKind::PlusEq)),
            ("-=", TokenKind::Operator(OperatorKind::MinusEq)),
            ("\\*=", TokenKind::Operator(OperatorKind::StarEq)),
            ("/=", TokenKind::Operator(OperatorKind::SlashEq)),
            ("%=", TokenKind::Operator(OperatorKind::PercentEq)),
            ("&=", TokenKind::Operator(OperatorKind::BitAndEq)),
            ("\\|=", TokenKind::Operator(OperatorKind::BitOrEq)),
            ("\\^=", TokenKind::Operator(OperatorKind::BitXorEq)),
            ("<<=", TokenKind::Operator(OperatorKind::ShlEq)),
            (">>=", TokenKind::Operator(OperatorKind::ShrEq)),
            // 逻辑
            ("&&", TokenKind::Operator(OperatorKind::And)),
            ("\\|\\|", TokenKind::Operator(OperatorKind::Or)),
            // 比较复合（在单字符 < > ! 之前）
            ("==", TokenKind::Operator(OperatorKind::EqEq)),
            ("!=", TokenKind::Operator(OperatorKind::BangEq)),
            ("<=", TokenKind::Operator(OperatorKind::LtEq)),
            (">=", TokenKind::Operator(OperatorKind::GtEq)),
            // 移位（在 < > 之前）
            ("<<", TokenKind::Operator(OperatorKind::Shl)),
            (">>", TokenKind::Operator(OperatorKind::Shr)),
            // 范围（在 . 之前）
            ("\\.\\.", TokenKind::Operator(OperatorKind::DotDot)),
            ("\\.\\.=", TokenKind::Operator(OperatorKind::DotDotEq)),
            // 箭头（在 - 之前）
            ("->", TokenKind::Operator(OperatorKind::Arrow)),
            ("=>", TokenKind::Operator(OperatorKind::FatArrow)),
            // as 系列与 ?. / !!（在 ? ! 之前）
            ("as\\?", TokenKind::Operator(OperatorKind::AsQuestion)),
            ("as!", TokenKind::Operator(OperatorKind::AsBang)),
            ("\\?\\.", TokenKind::Operator(OperatorKind::QuestionDot)),
            ("!!", TokenKind::Operator(OperatorKind::BangBang)),
            (
                "\\?\\?",
                TokenKind::Operator(OperatorKind::QuestionQuestion),
            ),
            // 单字符操作符
            ("\\+", TokenKind::Operator(OperatorKind::Plus)),
            ("-", TokenKind::Operator(OperatorKind::Minus)),
            ("\\*", TokenKind::Operator(OperatorKind::Star)),
            ("/", TokenKind::Operator(OperatorKind::Slash)),
            ("%", TokenKind::Operator(OperatorKind::Percent)),
            ("<", TokenKind::Operator(OperatorKind::Lt)),
            (">", TokenKind::Operator(OperatorKind::Gt)),
            ("&", TokenKind::Operator(OperatorKind::BitAnd)),
            ("\\|", TokenKind::Operator(OperatorKind::BitOr)),
            ("\\^", TokenKind::Operator(OperatorKind::BitXor)),
            ("!", TokenKind::Operator(OperatorKind::Bang)),
            ("~", TokenKind::Operator(OperatorKind::BitNot)),
            ("\\?", TokenKind::Operator(OperatorKind::Question)),
            ("=", TokenKind::Operator(OperatorKind::Eq)),
            // 标点
            ("\\(", TokenKind::Punctuation(PunctuationKind::LParen)),
            ("\\)", TokenKind::Punctuation(PunctuationKind::RParen)),
            ("\\{", TokenKind::Punctuation(PunctuationKind::LBrace)),
            ("\\}", TokenKind::Punctuation(PunctuationKind::RBrace)),
            ("\\[", TokenKind::Punctuation(PunctuationKind::LBracket)),
            ("\\]", TokenKind::Punctuation(PunctuationKind::RBracket)),
            (",", TokenKind::Punctuation(PunctuationKind::Comma)),
            ("\\.", TokenKind::Punctuation(PunctuationKind::Dot)),
            (":", TokenKind::Punctuation(PunctuationKind::Colon)),
            (";", TokenKind::Punctuation(PunctuationKind::Semicolon)),
            ("#", TokenKind::Punctuation(PunctuationKind::Hash)),
            ("@", TokenKind::Punctuation(PunctuationKind::At)),
        ]
    }
    pub fn dfa(&self) -> &DFA {
        &self.dfa
    }

    /// tokenize：NL 合成 + 注释跳过 + 括号深度抑制 + 最长匹配
    /// + 关键字后分类 + 未知字符恢复
    ///
    /// - 连续行结束符（\n / \r\n）合并为括号深度 0 时的一个
    ///   [`TokenKind::Nl`] token（语句分隔），括号内吞掉（深度抑制）；
    /// - 空白集 {' ', '\t', '\r'} 先于匹配跳过（NL 归 NL 处理器）；
    /// - 行注释 // 跳到行尾（不吃 NL）；块注释 /* */ 嵌套计数；
    /// - 未命中字符按**字符**推进（`len_utf8`，多字节安全）；
    /// - 未知产出 [`TokenKind::Unknown`] 单字符 token（错误恢复）。
    pub fn tokenize(&self, source: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut i = 0usize;
        let mut paren_depth = 0usize;

        while i < source.len() {
            let rest = &source[i..];
            let first = match rest.chars().next() {
                Some(c) => c,
                None => break,
            };

            // 连续 NL 合成一个 token；CRLF 是一个 terminator。
            if first == '\n' || rest.starts_with("\r\n") {
                let start = i;
                let bytes = source.as_bytes();
                while i < bytes.len() {
                    if bytes[i] == b'\n' {
                        i += 1;
                    } else if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                        i += 2;
                    } else {
                        break;
                    }
                }
                debug_assert!(i <= u32::MAX as usize);
                if paren_depth == 0 {
                    tokens.push(Token::new(TokenKind::Nl, Span::new(start as u32, i as u32)));
                }
                continue;
            }

            // 空白（不含 NL——上方归 NL 处理器）
            if matches!(first, ' ' | '\t' | '\r') {
                i += first.len_utf8();
                continue;
            }

            // 行注释 //：跳到行尾（不吃 NL——NL 由下方处理器接管）
            if rest.starts_with("//") {
                if let Some(nl) = rest.find('\n') {
                    i += nl;
                } else {
                    i = source.len();
                }
                continue;
            }

            // 块注释 /* */：嵌套计数
            if rest.starts_with("/*") {
                let mut depth = 1usize;
                let mut pos = 2usize;
                let bytes = rest.as_bytes();
                while pos + 1 < bytes.len() && depth > 0 {
                    if bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
                        depth += 1;
                        pos += 2;
                    } else if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                        depth -= 1;
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                }
                i += pos;
                continue;
            }

            // 只有 `(`/`[` 打开 suppress 上下文；`}` 始终是语句边界。
            match first {
                '(' | '[' => paren_depth += 1,
                ')' | ']' if paren_depth > 0 => paren_depth = paren_depth.saturating_sub(1),
                _ => {}
            }

            // DFA 最长匹配
            match self.dfa.longest_match(rest) {
                Some((len, mut kind)) if len > 0 => {
                    let end = i + len;
                    debug_assert!(end <= u32::MAX as usize);
                    // 关键字后分类：ident lexeme 查表
                    if kind == TokenKind::Identifier
                        && let Some(kw) = lookup_keyword(&source[i..end])
                    {
                        kind = TokenKind::Keyword(kw);
                    }
                    tokens.push(Token::new(kind, Span::new(i as u32, end as u32)));
                    i = end;
                }
                _ => {
                    // 未知字符：按字符推进（i 必为字符边界）
                    let end = i + first.len_utf8();
                    debug_assert!(end <= u32::MAX as usize);
                    tokens.push(Token::new(
                        TokenKind::Unknown,
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
    use ast::token::KeywordKind;
    use ast::token::LiteralKind;

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
        assert_eq!(tokens[0].text("🦀if"), "🦀");
        assert_eq!((tokens[0].span.start, tokens[0].span.end), (0, 4));
        assert_eq!(tokens[1].kind, TokenKind::Keyword(KeywordKind::If));
        assert_eq!((tokens[1].span.start, tokens[1].span.end), (4, 6));
    }

    #[test]
    fn tokenize_octal_and_binary_without_splitting() {
        let lexer = Lexer::starry().unwrap();
        let tokens = lexer.tokenize("0o777 0b1010");
        assert_eq!(tokens.len(), 2, "{tokens:?}");
        assert_eq!(tokens[0].kind, TokenKind::Literal(LiteralKind::Integer));
        assert_eq!(tokens[0].text("0o777 0b1010"), "0o777");
        assert_eq!(tokens[1].kind, TokenKind::Literal(LiteralKind::Integer));
        assert_eq!(tokens[1].text("0o777 0b1010"), "0b1010");
    }

    #[test]
    fn consecutive_terminators_merge_into_one_nl() {
        let lexer = Lexer::starry().unwrap();
        for (source, end) in [("a\n\nb", 3), ("a\r\n\r\nb", 5), ("a\n\r\nb", 4)] {
            let tokens = lexer.tokenize(source);
            assert_eq!(tokens.len(), 3, "{source}: {tokens:?}");
            assert_eq!(tokens[1].kind, TokenKind::Nl, "{source}");
            assert_eq!(tokens[1].span, ast::token::Span::new(1, end), "{source}");
        }
    }

    #[test]
    fn lambda_braces_do_not_close_parenthesis_newline_suppression() {
        let lexer = Lexer::starry().unwrap();
        let tokens = lexer.tokenize("({\n1\n})");
        assert!(
            !tokens.iter().any(|token| token.kind == TokenKind::Nl),
            "{tokens:?}"
        );
    }
}
