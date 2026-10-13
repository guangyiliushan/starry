//! 字面量类型定义

use std::fmt;

/// 字面量类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralKind {
    /// 整数字面量
    Integer,
    /// 浮点数字面量
    Float,
    /// 字符串字面量
    String,
    /// 字符字面量
    Char,
}

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralKind::Integer => write!(f, "integer"),
            LiteralKind::Float => write!(f, "float"),
            LiteralKind::String => write!(f, "string"),
            LiteralKind::Char => write!(f, "char"),
        }
    }
}
