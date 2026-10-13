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
pub mod keyword;
mod kind;
mod literal;
mod operator;

pub use error::{TokenError, TokenResult};
pub use keyword::{
    HARD_KEYWORD_COUNT, HardKeyword as KeywordKind, HardKeyword, MODIFIER_WORD_COUNT, ModifierWord,
    SLOT_WORD_COUNT, SlotWord, lookup_hard_keyword as lookup_keyword, lookup_modifier_word,
    lookup_slot_word,
};
pub use kind::{PunctuationKind, TokenKind};
pub use literal::LiteralKind;
pub use operator::OperatorKind;

use std::fmt;

impl fmt::Display for HardKeyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 源码字节区间（`end` exclusive；`end - start` = lexeme 字节数）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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
    pub const fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        let start = self.start as usize;
        let end = self.end as usize;
        debug_assert!(end <= source.len());
        debug_assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
        source.get(start..end).unwrap_or_default()
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// 词法单元
///
/// 表示源代码中的一个词法单元；文本由 source + span 取得，Token 保持 Copy。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    /// Token 类型
    pub kind: TokenKind,
    /// 位置信息
    pub span: Span,
}

impl Token {
    /// 创建新的 Token
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Token 对应的源码文本；越界是 lexer bug，release 走空切片避免 panic。
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        self.span.slice(source)
    }

    /// 创建 EOF Token
    pub fn eof(span: Span) -> Self {
        Self::eof_with_kind(span)
    }

    const fn eof_with_kind(span: Span) -> Self {
        Self::new(TokenKind::Eof, span)
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
        write!(f, "Token {{ kind: {}, span: {} }}", self.kind, self.span)
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

        let token = Token::new(TokenKind::Identifier, span);
        assert!(token.is_identifier());

        let token = Token::new(TokenKind::Keyword(KeywordKind::If), span);
        assert!(token.is_keyword());

        // EOF
        let token = Token::eof(span);
        assert!(token.is_eof());
    }

    #[test]
    fn test_token_display() {
        let span = Span::new(0, 1);
        let token = Token::new(TokenKind::Identifier, span);
        assert_eq!(token.to_string(), "Token { kind: identifier, span: 0..1 }");
        assert_eq!(token.text("x"), "x");
    }
}
