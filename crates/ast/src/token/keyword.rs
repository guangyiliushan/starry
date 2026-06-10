//! 关键字类型定义和查找

use std::fmt;
use phf::phf_map;

/// 关键字类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeywordKind {
    // ==================== 类型关键字 ====================
    /// int
    Int,
    /// i4
    I4,
    /// i8
    I8,
    /// i16
    I16,
    /// i32
    I32,
    /// i64
    I64,
    /// i128
    I128,
    /// uint
    Uint,
    /// u8
    U8,
    /// u16
    U16,
    /// u32
    U32,
    /// u64
    U64,
    /// u128
    U128,
    /// float
    Float,
    /// f4
    F4,
    /// f8
    F8,
    /// f8a
    F8a,
    /// f8b
    F8b,
    /// f8c
    F8c,
    /// f8d
    F8d,
    /// f8e
    F8e,
    /// f8f
    F8f,
    /// f16
    F16,
    /// f32
    F32,
    /// f64
    F64,
    /// f128
    F128,
    /// str
    Str,
    /// char
    Char,
    /// bool
    Bool,

    // ==================== 字面量关键字 ====================
    /// true
    True,
    /// false
    False,
    /// null
    Null,

    // ==================== 控制流关键字 ====================
    /// if
    If,
    /// else
    Else,
    /// when
    When,
    /// for
    For,
    /// while
    While,
    /// do
    Do,
    /// loop
    Loop,
    /// break
    Break,
    /// continue
    Continue,
    /// return
    Return,

    // ==================== 类型操作关键字 ====================
    /// as
    As,
    /// as? (词法层面识别)
    AsQuestion,
    /// is
    Is,
    /// !is (词法层面识别)
    NotIs,
    /// in
    In,
    /// !in (词法层面识别)
    NotIn,

    // ==================== 声明关键字 ====================
    /// class
    Class,
    /// interface
    Interface,
    /// struct
    Struct,
    /// enum
    Enum,
    /// union
    Union,
    /// trait
    Trait,
    /// fun
    Fun,
    /// val
    Val,
    /// var
    Var,
    /// type
    Type,
    /// typealias
    Typealias,

    // ==================== 访问修饰关键字 ====================
    /// public
    Public,
    /// private
    Private,
    /// protected
    Protected,
    /// internal
    Internal,
    /// abstract
    Abstract,
    /// final
    Final,
    /// open
    Open,
    /// override
    Override,

    // ==================== 特殊修饰关键字 ====================
    /// const
    Const,
    /// static
    Static,
    /// async
    Async,
    /// await
    Await,
    /// inline
    Inline,
    /// noinline
    Noinline,
    /// crossinline
    Crossinline,
    /// infix
    Infix,
    /// operator
    Operator,
    /// lateinit
    Lateinit,
    /// inner
    Inner,
    /// companion
    Companion,
    /// data
    Data,
    /// sealed
    Sealed,
    /// suspend
    Suspend,
    /// reified
    Reified,
    /// tailrec
    Tailrec,
    /// value
    Value,
    /// actual
    Actual,
    /// expect
    Expect,
    /// annotation
    Annotation,
    /// external
    External,

    // ==================== 类成员关键字 ====================
    /// constructor
    Constructor,
    /// init
    Init,
    /// get
    Get,
    /// set
    Set,
    /// field
    Field,
    /// property
    Property,

    // ==================== 异常处理关键字 ====================
    /// try
    Try,
    /// catch
    Catch,
    /// finally
    Finally,
    /// throw
    Throw,

    // ==================== 模块/导入关键字 ====================
    /// import
    Import,
    /// extern
    Extern,

    // ==================== 其他关键字 ====================
    /// self
    Self_,
    /// super
    Super,
    /// dyn
    Dyn,
    /// unsafe
    Unsafe,
    /// where
    Where,
    /// by
    By,
    /// delegate
    Delegate,
    /// this
    This,
    /// typeof
    Typeof,
    /// package
    Package,
    /// dynamic
    Dynamic,
    /// out
    Out,
    /// in (类型参数变型)
    InVariant,
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl KeywordKind {
    /// 获取关键字的文本表示
    pub fn as_str(&self) -> &'static str {
        match self {
            // 类型关键字
            KeywordKind::Int => "int",
            KeywordKind::I4 => "i4",
            KeywordKind::I8 => "i8",
            KeywordKind::I16 => "i16",
            KeywordKind::I32 => "i32",
            KeywordKind::I64 => "i64",
            KeywordKind::I128 => "i128",
            KeywordKind::Uint => "uint",
            KeywordKind::U8 => "u8",
            KeywordKind::U16 => "u16",
            KeywordKind::U32 => "u32",
            KeywordKind::U64 => "u64",
            KeywordKind::U128 => "u128",
            KeywordKind::Float => "float",
            KeywordKind::F4 => "f4",
            KeywordKind::F8 => "f8",
            KeywordKind::F8a => "f8a",
            KeywordKind::F8b => "f8b",
            KeywordKind::F8c => "f8c",
            KeywordKind::F8d => "f8d",
            KeywordKind::F8e => "f8e",
            KeywordKind::F8f => "f8f",
            KeywordKind::F16 => "f16",
            KeywordKind::F32 => "f32",
            KeywordKind::F64 => "f64",
            KeywordKind::F128 => "f128",
            KeywordKind::Str => "str",
            KeywordKind::Char => "char",
            KeywordKind::Bool => "bool",

            // 字面量关键字
            KeywordKind::True => "true",
            KeywordKind::False => "false",
            KeywordKind::Null => "null",

            // 控制流关键字
            KeywordKind::If => "if",
            KeywordKind::Else => "else",
            KeywordKind::When => "when",
            KeywordKind::For => "for",
            KeywordKind::While => "while",
            KeywordKind::Do => "do",
            KeywordKind::Loop => "loop",
            KeywordKind::Break => "break",
            KeywordKind::Continue => "continue",
            KeywordKind::Return => "return",

            // 类型操作关键字
            KeywordKind::As => "as",
            KeywordKind::AsQuestion => "as?",
            KeywordKind::Is => "is",
            KeywordKind::NotIs => "!is",
            KeywordKind::In => "in",
            KeywordKind::NotIn => "!in",

            // 声明关键字
            KeywordKind::Class => "class",
            KeywordKind::Interface => "interface",
            KeywordKind::Struct => "struct",
            KeywordKind::Enum => "enum",
            KeywordKind::Union => "union",
            KeywordKind::Trait => "trait",
            KeywordKind::Fun => "fun",
            KeywordKind::Val => "val",
            KeywordKind::Var => "var",
            KeywordKind::Type => "type",
            KeywordKind::Typealias => "typealias",

            // 访问修饰关键字
            KeywordKind::Public => "public",
            KeywordKind::Private => "private",
            KeywordKind::Protected => "protected",
            KeywordKind::Internal => "internal",
            KeywordKind::Abstract => "abstract",
            KeywordKind::Final => "final",
            KeywordKind::Open => "open",
            KeywordKind::Override => "override",

            // 特殊修饰关键字
            KeywordKind::Const => "const",
            KeywordKind::Static => "static",
            KeywordKind::Async => "async",
            KeywordKind::Await => "await",
            KeywordKind::Inline => "inline",
            KeywordKind::Noinline => "noinline",
            KeywordKind::Crossinline => "crossinline",
            KeywordKind::Infix => "infix",
            KeywordKind::Operator => "operator",
            KeywordKind::Lateinit => "lateinit",
            KeywordKind::Inner => "inner",
            KeywordKind::Companion => "companion",
            KeywordKind::Data => "data",
            KeywordKind::Sealed => "sealed",
            KeywordKind::Suspend => "suspend",
            KeywordKind::Reified => "reified",
            KeywordKind::Tailrec => "tailrec",
            KeywordKind::Value => "value",
            KeywordKind::Actual => "actual",
            KeywordKind::Expect => "expect",
            KeywordKind::Annotation => "annotation",
            KeywordKind::External => "external",

            // 类成员关键字
            KeywordKind::Constructor => "constructor",
            KeywordKind::Init => "init",
            KeywordKind::Get => "get",
            KeywordKind::Set => "set",
            KeywordKind::Field => "field",
            KeywordKind::Property => "property",

            // 异常处理关键字
            KeywordKind::Try => "try",
            KeywordKind::Catch => "catch",
            KeywordKind::Finally => "finally",
            KeywordKind::Throw => "throw",

            // 模块/导入关键字
            KeywordKind::Import => "import",
            KeywordKind::Extern => "extern",

            // 其他关键字
            KeywordKind::Self_ => "self",
            KeywordKind::Super => "super",
            KeywordKind::Dyn => "dyn",
            KeywordKind::Unsafe => "unsafe",
            KeywordKind::Where => "where",
            KeywordKind::By => "by",
            KeywordKind::Delegate => "delegate",
            KeywordKind::This => "this",
            KeywordKind::Typeof => "typeof",
            KeywordKind::Package => "package",
            KeywordKind::Dynamic => "dynamic",
            KeywordKind::Out => "out",
            KeywordKind::InVariant => "in",
        }
    }
}

/// 关键字静态查找表
static KEYWORDS: phf::Map<&'static str, KeywordKind> = phf_map! {
    // 类型关键字
    "int" => KeywordKind::Int,
    "i4" => KeywordKind::I4,
    "i8" => KeywordKind::I8,
    "i16" => KeywordKind::I16,
    "i32" => KeywordKind::I32,
    "i64" => KeywordKind::I64,
    "i128" => KeywordKind::I128,
    "uint" => KeywordKind::Uint,
    "u8" => KeywordKind::U8,
    "u16" => KeywordKind::U16,
    "u32" => KeywordKind::U32,
    "u64" => KeywordKind::U64,
    "u128" => KeywordKind::U128,
    "float" => KeywordKind::Float,
    "f4" => KeywordKind::F4,
    "f8" => KeywordKind::F8,
    "f8a" => KeywordKind::F8a,
    "f8b" => KeywordKind::F8b,
    "f8c" => KeywordKind::F8c,
    "f8d" => KeywordKind::F8d,
    "f8e" => KeywordKind::F8e,
    "f8f" => KeywordKind::F8f,
    "f16" => KeywordKind::F16,
    "f32" => KeywordKind::F32,
    "f64" => KeywordKind::F64,
    "f128" => KeywordKind::F128,
    "str" => KeywordKind::Str,
    "char" => KeywordKind::Char,
    "bool" => KeywordKind::Bool,

    // 字面量关键字
    "true" => KeywordKind::True,
    "false" => KeywordKind::False,
    "null" => KeywordKind::Null,

    // 控制流关键字
    "if" => KeywordKind::If,
    "else" => KeywordKind::Else,
    "when" => KeywordKind::When,
    "for" => KeywordKind::For,
    "while" => KeywordKind::While,
    "do" => KeywordKind::Do,
    "loop" => KeywordKind::Loop,
    "break" => KeywordKind::Break,
    "continue" => KeywordKind::Continue,
    "return" => KeywordKind::Return,

    // 类型操作关键字
    "as" => KeywordKind::As,
    "is" => KeywordKind::Is,
    "in" => KeywordKind::In,

    // 声明关键字
    "class" => KeywordKind::Class,
    "interface" => KeywordKind::Interface,
    "struct" => KeywordKind::Struct,
    "enum" => KeywordKind::Enum,
    "union" => KeywordKind::Union,
    "trait" => KeywordKind::Trait,
    "fun" => KeywordKind::Fun,
    "val" => KeywordKind::Val,
    "var" => KeywordKind::Var,
    "type" => KeywordKind::Type,
    "typealias" => KeywordKind::Typealias,

    // 访问修饰关键字
    "public" => KeywordKind::Public,
    "private" => KeywordKind::Private,
    "protected" => KeywordKind::Protected,
    "internal" => KeywordKind::Internal,
    "abstract" => KeywordKind::Abstract,
    "final" => KeywordKind::Final,
    "open" => KeywordKind::Open,
    "override" => KeywordKind::Override,

    // 特殊修饰关键字
    "const" => KeywordKind::Const,
    "static" => KeywordKind::Static,
    "async" => KeywordKind::Async,
    "await" => KeywordKind::Await,
    "inline" => KeywordKind::Inline,
    "noinline" => KeywordKind::Noinline,
    "crossinline" => KeywordKind::Crossinline,
    "infix" => KeywordKind::Infix,
    "operator" => KeywordKind::Operator,
    "lateinit" => KeywordKind::Lateinit,
    "inner" => KeywordKind::Inner,
    "companion" => KeywordKind::Companion,
    "data" => KeywordKind::Data,
    "sealed" => KeywordKind::Sealed,
    "suspend" => KeywordKind::Suspend,
    "reified" => KeywordKind::Reified,
    "tailrec" => KeywordKind::Tailrec,
    "value" => KeywordKind::Value,
    "actual" => KeywordKind::Actual,
    "expect" => KeywordKind::Expect,
    "annotation" => KeywordKind::Annotation,
    "external" => KeywordKind::External,

    // 类成员关键字
    "constructor" => KeywordKind::Constructor,
    "init" => KeywordKind::Init,
    "get" => KeywordKind::Get,
    "set" => KeywordKind::Set,
    "field" => KeywordKind::Field,
    "property" => KeywordKind::Property,

    // 异常处理关键字
    "try" => KeywordKind::Try,
    "catch" => KeywordKind::Catch,
    "finally" => KeywordKind::Finally,
    "throw" => KeywordKind::Throw,

    // 模块/导入关键字
    "import" => KeywordKind::Import,
    "extern" => KeywordKind::Extern,

    // 其他关键字
    "self" => KeywordKind::Self_,
    "super" => KeywordKind::Super,
    "dyn" => KeywordKind::Dyn,
    "unsafe" => KeywordKind::Unsafe,
    "where" => KeywordKind::Where,
    "by" => KeywordKind::By,
    "delegate" => KeywordKind::Delegate,
    "this" => KeywordKind::This,
    "typeof" => KeywordKind::Typeof,
    "package" => KeywordKind::Package,
    "dynamic" => KeywordKind::Dynamic,
    "out" => KeywordKind::Out,
};

/// 查找关键字
///
/// # Arguments
/// * `ident` - 标识符字符串
///
/// # Returns
/// 如果是关键字，返回对应的 `KeywordKind`，否则返回 `None`
#[inline]
pub fn lookup_keyword(ident: &str) -> Option<KeywordKind> {
    KEYWORDS.get(ident).copied()
}

/// 检查字符串是否为关键字
#[inline]
pub fn is_keyword(ident: &str) -> bool {
    KEYWORDS.contains_key(ident)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_keyword() {
        assert_eq!(lookup_keyword("if"), Some(KeywordKind::If));
        assert_eq!(lookup_keyword("class"), Some(KeywordKind::Class));
        assert_eq!(lookup_keyword("not_a_keyword"), None);
    }

    #[test]
    fn test_is_keyword() {
        assert!(is_keyword("if"));
        assert!(is_keyword("class"));
        assert!(!is_keyword("not_a_keyword"));
    }

    #[test]
    fn test_keyword_display() {
        assert_eq!(KeywordKind::If.to_string(), "if");
        assert_eq!(KeywordKind::Class.to_string(), "class");
    }
}
