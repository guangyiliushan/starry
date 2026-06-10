//! Token 相关错误类型

use std::fmt;

use compact_str::CompactString;

/// Token 错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// 无效的字符
    InvalidCharacter {
        /// 字符
        ch: char,
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
    /// 未终止的字符串
    UnterminatedString {
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
    /// 未终止的字符字面量
    UnterminatedChar {
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
    /// 无效的数字字面量
    InvalidNumber {
        /// 错误信息
        message: CompactString,
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
    /// 无效的转义序列
    InvalidEscapeSequence {
        /// 转义序列
        sequence: CompactString,
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
    /// 未预期的 EOF
    UnexpectedEof {
        /// 行号
        line: u32,
        /// 列号
        column: u32,
    },
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenError::InvalidCharacter { ch, line, column } => {
                write!(f, "invalid character '{}' at {}:{}", ch, line, column)
            }
            TokenError::UnterminatedString { line, column } => {
                write!(f, "unterminated string at {}:{}", line, column)
            }
            TokenError::UnterminatedChar { line, column } => {
                write!(f, "unterminated char literal at {}:{}", line, column)
            }
            TokenError::InvalidNumber { message, line, column } => {
                write!(f, "invalid number literal: {} at {}:{}", message, line, column)
            }
            TokenError::InvalidEscapeSequence { sequence, line, column } => {
                write!(f, "invalid escape sequence '{}' at {}:{}", sequence, line, column)
            }
            TokenError::UnexpectedEof { line, column } => {
                write!(f, "unexpected end of file at {}:{}", line, column)
            }
        }
    }
}

impl std::error::Error for TokenError {}

impl TokenError {
    /// 获取错误位置的行号
    pub fn line(&self) -> u32 {
        match self {
            TokenError::InvalidCharacter { line, .. } => *line,
            TokenError::UnterminatedString { line, .. } => *line,
            TokenError::UnterminatedChar { line, .. } => *line,
            TokenError::InvalidNumber { line, .. } => *line,
            TokenError::InvalidEscapeSequence { line, .. } => *line,
            TokenError::UnexpectedEof { line, .. } => *line,
        }
    }

    /// 获取错误位置的列号
    pub fn column(&self) -> u32 {
        match self {
            TokenError::InvalidCharacter { column, .. } => *column,
            TokenError::UnterminatedString { column, .. } => *column,
            TokenError::UnterminatedChar { column, .. } => *column,
            TokenError::InvalidNumber { column, .. } => *column,
            TokenError::InvalidEscapeSequence { column, .. } => *column,
            TokenError::UnexpectedEof { column, .. } => *column,
        }
    }
}

/// Token 错误结果类型
pub type TokenResult<T> = Result<T, TokenError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_error_display() {
        let err = TokenError::InvalidCharacter {
            ch: '@',
            line: 1,
            column: 5,
        };
        assert_eq!(err.to_string(), "invalid character '@' at 1:5");
    }

    #[test]
    fn test_token_error_position() {
        let err = TokenError::UnterminatedString {
            line: 10,
            column: 20,
        };
        assert_eq!(err.line(), 10);
        assert_eq!(err.column(), 20);
    }
}
