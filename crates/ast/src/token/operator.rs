//! 运算符类型定义

use std::fmt;

/// 运算符类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    // 算术运算符
    /// +
    Plus,
    /// -
    Minus,
    /// *
    Star,
    /// /
    Slash,
    /// %
    Percent,

    // 比较运算符
    /// ==
    EqEq,
    /// !=
    BangEq,
    /// <
    Lt,
    /// <=
    LtEq,
    /// >
    Gt,
    /// >=
    GtEq,

    // 逻辑运算符
    /// &&
    And,
    /// ||
    Or,
    /// !
    Bang,

    // 位运算符
    /// &
    BitAnd,
    /// |
    BitOr,
    /// ^
    BitXor,
    /// <<
    Shl,
    /// >>
    Shr,

    // 赋值运算符
    /// =
    Eq,
    /// +=
    PlusEq,
    /// -=
    MinusEq,
    /// *=
    StarEq,
    /// /=
    SlashEq,
    /// %=
    PercentEq,
    /// &=
    BitAndEq,
    /// |=
    BitOrEq,
    /// ^=
    BitXorEq,
    /// <<=
    ShlEq,
    /// >>=
    ShrEq,

    // 范围运算符
    /// ..
    DotDot,
    /// ..=
    DotDotEq,

    // 其他运算符
    /// as?
    AsQuestion,
    /// as!
    AsBang,
    /// ?.
    QuestionDot,
    /// !!
    BangBang,
    /// ?
    Question,
    /// ??
    QuestionQuestion,
    /// ->
    Arrow,
    /// =>
    FatArrow,
}

impl fmt::Display for OperatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            OperatorKind::Plus => "+",
            OperatorKind::Minus => "-",
            OperatorKind::Star => "*",
            OperatorKind::Slash => "/",
            OperatorKind::Percent => "%",
            OperatorKind::EqEq => "==",
            OperatorKind::BangEq => "!=",
            OperatorKind::Lt => "<",
            OperatorKind::LtEq => "<=",
            OperatorKind::Gt => ">",
            OperatorKind::GtEq => ">=",
            OperatorKind::And => "&&",
            OperatorKind::Or => "||",
            OperatorKind::Bang => "!",
            OperatorKind::BitAnd => "&",
            OperatorKind::BitOr => "|",
            OperatorKind::BitXor => "^",
            OperatorKind::Shl => "<<",
            OperatorKind::Shr => ">>",
            OperatorKind::Eq => "=",
            OperatorKind::PlusEq => "+=",
            OperatorKind::MinusEq => "-=",
            OperatorKind::StarEq => "*=",
            OperatorKind::SlashEq => "/=",
            OperatorKind::PercentEq => "%=",
            OperatorKind::BitAndEq => "&=",
            OperatorKind::BitOrEq => "|=",
            OperatorKind::BitXorEq => "^=",
            OperatorKind::ShlEq => "<<=",
            OperatorKind::ShrEq => ">>=",
            OperatorKind::DotDot => "..",
            OperatorKind::DotDotEq => "..=",
            OperatorKind::Question => "?",
            OperatorKind::QuestionQuestion => "??",
            OperatorKind::Arrow => "->",
            OperatorKind::FatArrow => "=>",
            OperatorKind::AsQuestion => "as?",
            OperatorKind::AsBang => "as!",
            OperatorKind::QuestionDot => "?.",
            OperatorKind::BangBang => "!!",
        };
        write!(f, "{s}")
    }
}

impl OperatorKind {
    /// 获取运算符的文本表示
    pub fn as_str(&self) -> &'static str {
        match self {
            OperatorKind::Plus => "+",
            OperatorKind::Minus => "-",
            OperatorKind::Star => "*",
            OperatorKind::Slash => "/",
            OperatorKind::Percent => "%",
            OperatorKind::EqEq => "==",
            OperatorKind::BangEq => "!=",
            OperatorKind::Lt => "<",
            OperatorKind::LtEq => "<=",
            OperatorKind::Gt => ">",
            OperatorKind::GtEq => ">=",
            OperatorKind::And => "&&",
            OperatorKind::Or => "||",
            OperatorKind::Bang => "!",
            OperatorKind::BitAnd => "&",
            OperatorKind::BitOr => "|",
            OperatorKind::BitXor => "^",
            OperatorKind::Shl => "<<",
            OperatorKind::Shr => ">>",
            OperatorKind::Eq => "=",
            OperatorKind::PlusEq => "+=",
            OperatorKind::MinusEq => "-=",
            OperatorKind::StarEq => "*=",
            OperatorKind::SlashEq => "/=",
            OperatorKind::PercentEq => "%=",
            OperatorKind::BitAndEq => "&=",
            OperatorKind::BitOrEq => "|=",
            OperatorKind::BitXorEq => "^=",
            OperatorKind::ShlEq => "<<=",
            OperatorKind::ShrEq => ">>=",
            OperatorKind::DotDot => "..",
            OperatorKind::DotDotEq => "..=",
            OperatorKind::Question => "?",
            OperatorKind::QuestionQuestion => "??",
            OperatorKind::Arrow => "->",
            OperatorKind::FatArrow => "=>",
            OperatorKind::AsQuestion => "as?",
            OperatorKind::AsBang => "as!",
            OperatorKind::QuestionDot => "?.",
            OperatorKind::BangBang => "!!",
        }
    }
}
