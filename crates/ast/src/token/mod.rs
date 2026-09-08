//! Token 模块 - 词法分析相关类型定义
//!
//! 该模块提供了编译器词法分析所需的核心类型：
//! - [`Token`] - 表示源代码中的一个词法单元
//! - [`TokenKind`] - Token 的类型枚举
//! - [`KeywordKind`] - 关键字类型
//! - [`OperatorKind`] - 运算符类型
//! - [`LiteralKind`] - 字面量类型
//! - [`PunctuationKind`] - 标点符号类型

mod error;
mod kind;
pub mod keyword_gen;
mod literal;
mod operator;

pub use error::{TokenError, TokenResult};
pub use kind::{PunctuationKind, TokenKind};
pub use keyword_gen::{
    lookup_hard_keyword as lookup_keyword, lookup_modifier_word, lookup_slot_word,
    HardKeyword as KeywordKind, HardKeyword, ModifierWord, SlotWord, MODIFIER_WORD_COUNT,
    HARD_KEYWORD_COUNT, SLOT_WORD_COUNT,
};
pub use literal::LiteralKind;
pub use operator::OperatorKind;

use std::fmt;

impl fmt::Display for HardKeyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}



/// 源码字节区间（`end` exclusive；`end - start` = lexeme 字节数）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 起始字节偏移（含）
    pub start: u32,
    /// 结束字节偏移（不含）
    pub end: u32,
}

impl Span {
    /// 创建字节区间
    ///
    /// # Panics (debug)
    ///
    /// `start > end` 时断言失败。
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// 词法单元
///
/// 表示源代码中的一个词法单元，包含类型、位置和词素信息
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// Token 类型
    pub kind: TokenKind,
    /// 词素（Token 的文本内容）
    pub lexeme: Box<str>,
    /// 位置信息
    pub span: Span,
}

impl Token {
    /// 创建新的 Token
    pub fn new(kind: TokenKind, lexeme: impl Into<Box<str>>, span: Span) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            span,
        }
    }

    /// 创建标识符 Token
    pub fn identifier(name: impl Into<Box<str>>, span: Span) -> Self {
        Self::new(TokenKind::Identifier, name, span)
    }

    /// 创建关键字 Token
    pub fn keyword(kind: KeywordKind, span: Span) -> Self {
        Self::new(TokenKind::Keyword(kind), kind.as_str(), span)
    }

    /// 创建字面量 Token
    pub fn literal(kind: LiteralKind, value: impl Into<Box<str>>, span: Span) -> Self {
        Self::new(TokenKind::Literal(kind), value, span)
    }

    /// 创建运算符 Token
    pub fn operator(kind: OperatorKind, span: Span) -> Self {
        Self::new(TokenKind::Operator(kind), kind.as_str(), span)
    }

    /// 创建标点符号 Token
    pub fn punctuation(kind: PunctuationKind, span: Span) -> Self {
        Self::new(TokenKind::Punctuation(kind), kind.as_str(), span)
    }

    /// 创建 EOF Token
    pub fn eof(span: Span) -> Self {
        Self::new(TokenKind::Eof, "", span)
    }

    /// 检查是否为指定类型的 Token
    pub fn is(&self, kind: TokenKind) -> bool {
        self.kind == kind
    }

    /// 检查是否为标识符
    pub fn is_identifier(&self) -> bool {
        self.kind.is_identifier()
    }

    /// 检查是否为关键字
    pub fn is_keyword(&self) -> bool {
        self.kind.is_keyword()
    }

    /// 检查是否为运算符
    pub fn is_operator(&self) -> bool {
        self.kind.is_operator()
    }

    /// 检查是否为字面量
    pub fn is_literal(&self) -> bool {
        self.kind.is_literal()
    }

    /// 检查是否为 EOF
    pub fn is_eof(&self) -> bool {
        self.kind.is_eof()
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token {{ kind: {}, lexeme: \"{}\", span: {} }}",
            self.kind, self.lexeme, self.span
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span() {
        let span = Span::new(10, 15);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 15);
        assert_eq!(span.to_string(), "10..15");
        // end exclusive：end - start = 字节数
        assert_eq!(span.end - span.start, 5);
    }

    #[test]
    fn test_token_creation() {
        let span = Span::new(1, 1);

        // 标识符
        let token = Token::identifier("foo", span);
        assert!(token.is_identifier());
        assert_eq!(&*token.lexeme, "foo");

        // 关键字
        let token = Token::keyword(KeywordKind::If, span);
        assert!(token.is_keyword());
        assert_eq!(&*token.lexeme, "if");

        // EOF
        let token = Token::eof(span);
        assert!(token.is_eof());
    }

    #[test]
    fn test_token_display() {
        let span = Span::new(0, 1);
        let token = Token::identifier("x", span);
        assert_eq!(
            token.to_string(),
            "Token { kind: identifier, lexeme: \"x\", span: 0..1 }"
        );
    }
}
