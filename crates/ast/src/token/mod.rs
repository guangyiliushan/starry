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
mod keyword;
mod literal;
mod operator;

pub use error::{TokenError, TokenResult};
pub use kind::{PunctuationKind, TokenKind};
pub use keyword::{is_keyword, lookup_keyword, KeywordKind};
pub use literal::LiteralKind;
pub use operator::OperatorKind;

use compact_str::CompactString;
use std::fmt;

/// 源代码位置信息
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 行号（从 1 开始）
    pub line: u32,
    /// 列号（从 1 开始）
    pub column: u32,
}

impl Span {
    /// 创建新的位置信息
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// 创建起始位置（第 1 行第 1 列）
    pub fn start() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::start()
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
    pub lexeme: CompactString,
    /// 位置信息
    pub span: Span,
}

impl Token {
    /// 创建新的 Token
    pub fn new(kind: TokenKind, lexeme: impl Into<CompactString>, span: Span) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            span,
        }
    }

    /// 创建标识符 Token
    pub fn identifier(name: impl Into<CompactString>, span: Span) -> Self {
        Self::new(TokenKind::Identifier, name, span)
    }

    /// 创建关键字 Token
    pub fn keyword(kind: KeywordKind, span: Span) -> Self {
        Self::new(TokenKind::Keyword(kind), kind.as_str(), span)
    }

    /// 创建字面量 Token
    pub fn literal(kind: LiteralKind, value: impl Into<CompactString>, span: Span) -> Self {
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

    /// 获取行号
    pub fn line(&self) -> u32 {
        self.span.line
    }

    /// 获取列号
    pub fn column(&self) -> u32 {
        self.span.column
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
        let span = Span::new(10, 5);
        assert_eq!(span.line, 10);
        assert_eq!(span.column, 5);
        assert_eq!(span.to_string(), "10:5");
    }

    #[test]
    fn test_token_creation() {
        let span = Span::new(1, 1);

        // 标识符
        let token = Token::identifier("foo", span);
        assert!(token.is_identifier());
        assert_eq!(token.lexeme, "foo");

        // 关键字
        let token = Token::keyword(KeywordKind::If, span);
        assert!(token.is_keyword());
        assert_eq!(token.lexeme, "if");

        // EOF
        let token = Token::eof(span);
        assert!(token.is_eof());
    }

    #[test]
    fn test_token_display() {
        let span = Span::new(1, 1);
        let token = Token::identifier("x", span);
        assert_eq!(
            token.to_string(),
            "Token { kind: identifier, lexeme: \"x\", span: 1:1 }"
        );
    }
}
