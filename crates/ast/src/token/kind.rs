//! Token 类型定义

use std::fmt;

use super::{KeywordKind, LiteralKind, OperatorKind};

/// 标点符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PunctuationKind {
    /// (
    LParen,
    /// )
    RParen,
    /// {
    LBrace,
    /// }
    RBrace,
    /// [
    LBracket,
    /// ]
    RBracket,
    /// ,
    Comma,
    /// .
    Dot,
    /// :
    Colon,
    /// ::
    ColonColon,
    /// ;
    Semicolon,
    /// #
    Hash,
    /// @
    At,
    /// _
    Underscore,
}

impl fmt::Display for PunctuationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PunctuationKind {
    /// 获取标点符号的文本表示
    pub fn as_str(&self) -> &'static str {
        match self {
            PunctuationKind::LParen => "(",
            PunctuationKind::RParen => ")",
            PunctuationKind::LBrace => "{",
            PunctuationKind::RBrace => "}",
            PunctuationKind::LBracket => "[",
            PunctuationKind::RBracket => "]",
            PunctuationKind::Comma => ",",
            PunctuationKind::Dot => ".",
            PunctuationKind::Colon => ":",
            PunctuationKind::ColonColon => "::",
            PunctuationKind::Semicolon => ";",
            PunctuationKind::Hash => "#",
            PunctuationKind::At => "@",
            PunctuationKind::Underscore => "_",
        }
    }
}

/// Token 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ==================== 字面量 ====================
    /// 字面量
    Literal(LiteralKind),

    // ==================== 标识符 ====================
    /// 标识符
    Identifier,

    // ==================== 关键字 ====================
    /// 关键字
    Keyword(KeywordKind),

    // ==================== 运算符 ====================
    /// 运算符
    Operator(OperatorKind),

    // ==================== 标点符号 ====================
    /// 标点符号
    Punctuation(PunctuationKind),

    // ==================== 特殊 Token ====================
    /// 换行符（语句分隔，由驱动层合成）
    Nl,
    /// 文件结束
    Eof,
    /// 未知/无效 Token
    Unknown,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Literal(kind) => write!(f, "{} literal", kind),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Keyword(kind) => write!(f, "keyword '{}'", kind),
            TokenKind::Operator(kind) => write!(f, "operator '{}'", kind),
            TokenKind::Punctuation(kind) => write!(f, "punctuation '{}'", kind),
            TokenKind::Eof => write!(f, "end of file"),
            TokenKind::Nl => write!(f, "newline"),
            TokenKind::Unknown => write!(f, "unknown"),
        }
    }
}

impl TokenKind {
    /// 检查是否为字面量
    pub fn is_literal(&self) -> bool {
        matches!(self, TokenKind::Literal(_))
    }

    /// 检查是否为标识符
    pub fn is_identifier(&self) -> bool {
        matches!(self, TokenKind::Identifier)
    }

    /// 检查是否为关键字
    pub fn is_keyword(&self) -> bool {
        matches!(self, TokenKind::Keyword(_))
    }

    /// 检查是否为运算符
    pub fn is_operator(&self) -> bool {
        matches!(self, TokenKind::Operator(_))
    }

    /// 检查是否为标点符号
    pub fn is_punctuation(&self) -> bool {
        matches!(self, TokenKind::Punctuation(_))
    }

    /// 检查是否为 EOF
    pub fn is_eof(&self) -> bool {
        matches!(self, TokenKind::Eof)
    }

    /// 获取关键字类型（如果是关键字）
    pub fn as_keyword(&self) -> Option<KeywordKind> {
        match self {
            TokenKind::Keyword(kind) => Some(*kind),
            _ => None,
        }
    }

    /// 获取运算符类型（如果是运算符）
    pub fn as_operator(&self) -> Option<OperatorKind> {
        match self {
            TokenKind::Operator(kind) => Some(*kind),
            _ => None,
        }
    }

    /// 获取字面量类型（如果是字面量）
    pub fn as_literal(&self) -> Option<LiteralKind> {
        match self {
            TokenKind::Literal(kind) => Some(*kind),
            _ => None,
        }
    }

    /// 获取标点符号类型（如果是标点符号）
    pub fn as_punctuation(&self) -> Option<PunctuationKind> {
        match self {
            TokenKind::Punctuation(kind) => Some(*kind),
            _ => None,
        }
    }
}

// ==================== 便捷构造方法 ====================

impl TokenKind {
    /// 创建整数字面量 Token
    pub const fn integer() -> Self {
        TokenKind::Literal(LiteralKind::Integer)
    }

    /// 创建浮点数字面量 Token
    pub const fn float() -> Self {
        TokenKind::Literal(LiteralKind::Float)
    }

    /// 创建字符串字面量 Token
    pub const fn string() -> Self {
        TokenKind::Literal(LiteralKind::String)
    }

    /// 创建字符字面量 Token
    pub const fn char_() -> Self {
        TokenKind::Literal(LiteralKind::Char)
    }

    /// 创建布尔字面量 Token
    pub const fn bool_() -> Self {
        TokenKind::Literal(LiteralKind::Bool)
    }

    /// 创建空值字面量 Token
    pub const fn null() -> Self {
        TokenKind::Literal(LiteralKind::Null)
    }

    /// 创建 EOF Token
    pub const fn eof() -> Self {
        TokenKind::Eof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_kind_display() {
        assert_eq!(TokenKind::Identifier.to_string(), "identifier");
        assert_eq!(TokenKind::Eof.to_string(), "end of file");
    }

    #[test]
    fn test_token_kind_checks() {
        assert!(TokenKind::Identifier.is_identifier());
        assert!(!TokenKind::Identifier.is_keyword());

        assert!(TokenKind::Keyword(KeywordKind::If).is_keyword());
        assert!(!TokenKind::Keyword(KeywordKind::If).is_operator());
    }

    #[test]
    fn test_convenience_constructors() {
        assert_eq!(TokenKind::integer(), TokenKind::Literal(LiteralKind::Integer));
        assert_eq!(TokenKind::float(), TokenKind::Literal(LiteralKind::Float));
        assert_eq!(TokenKind::eof(), TokenKind::Eof);
    }
}
