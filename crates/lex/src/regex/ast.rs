//! 抽象语法树（AST）模块
//!
//! 该模块定义了正则表达式的抽象语法树， faithfully 保留源码结构。
//!
//! # 设计特点
//!
//! - **扁平化序列**：使用 `Sequence(Vec<Ast>)` 而非嵌套的 `Concat`，避免深度嵌套
//! - **类型安全**：每个变体语义清晰，便于模式匹配
//! - **可扩展**：预留扩展点，支持未来添加新特性
//! - **易于优化**：扁平化结构便于分析和优化
//!
//! # 核心类型
//!
//! - [`Ast`] - 正则表达式 AST 枚举
//! - [`Repeat`] - 重复构造（如 `{n,m}`）
//! - [`Flags`] - 控制标记（如 `(?i)`）

use std::fmt;
use crate::transition::{CharClass, PredefinedClass};

// ==================== AST 枚举 ====================

/// 正则表达式抽象语法树
///
/// 采用扁平化设计，将嵌套的连接和选择展开为列表，避免深度递归。
///
/// # 设计原则
///
/// - **扁平化序列**：`Sequence(Vec<Ast>)` 代替 `Concat(Box<Ast>, Box<Ast>)`
/// - **扁平化选择**：`Choice(Vec<Ast>)` 代替 `Alt(Box<Ast>, Box<Ast>)`
/// - **保留二元重复**：保持重复操作的经典形式
///
/// # 示例
///
/// ```
/// # use lex::regex::Ast;
///
/// // 字面量
/// let a = Ast::Literal('a');
///
/// // 序列：abc
/// let abc = Ast::Sequence(vec![
///     Ast::Literal('a'),
///     Ast::Literal('b'),
///     Ast::Literal('c'),
/// ]);
///
/// // 选择：a|b|c
/// let choice = Ast::Choice(vec![
///     Ast::Literal('a'),
///     Ast::Literal('b'),
///     Ast::Literal('c'),
/// ]);
///
/// // 重复：a*
/// let star = Ast::ZeroOrMore(Box::new(Ast::Literal('a')));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    /// 空字符串（匹配零次）
    ///
    /// # 示例
    ///
    /// `a|` → `Choice([Literal('a'), Empty])`
    Empty,

    /// 单个字面字符
    ///
    /// # 示例
    ///
    /// `a` → `Literal('a')`
    Literal(char),

    /// 字符类（如 `[a-z]`, `\d`）
    ///
    /// # 示例
    ///
    /// `[a-z]` → `Class(CharClass::range('a', 'z'))`
    /// `\d` → `Class(PredefinedClass::Digit)`
    Class(CharClass),

    /// 序列：按顺序匹配
    ///
    /// 使用 `Vec` 而非嵌套的 `Concat`，避免深度递归。
    ///
    /// # 示例
    ///
    /// `abc` → `Sequence([a, b, c])`
    Sequence(Vec<Ast>),

    /// 选择：匹配其中之一
    ///
    /// 使用 `Vec` 而非嵌套的 `Alt`，避免深度递归。
    ///
    /// # 示例
    ///
    /// `a|b|c` → `Choice([a, b, c])`
    Choice(Vec<Ast>),

    /// 零次或多次重复（`*`）
    ///
    /// # 示例
    ///
    /// `a*` → `ZeroOrMore(a)`
    ZeroOrMore(Box<Ast>),

    /// 一次或多次重复（`+`）
    ///
    /// # 示例
    ///
    /// `a+` → `OneOrMore(a)`
    OneOrMore(Box<Ast>),

    /// 零次或一次（`?`）
    ///
    /// # 示例
    ///
    /// `a?` → `ZeroOrOne(a)`
    ZeroOrOne(Box<Ast>),

    /// 重复指定次数（`{n}`, `{n,m}`, `{n,}`)
    ///
    /// # 示例
    ///
    /// `a{3}` → `Repeat { min: 3, max: Some(3) }`
    /// `a{2,5}` → `Repeat { min: 2, max: Some(5) }`
    /// `a{3,}` → `Repeat { min: 3, max: None }`
    Repeat {
        /// 要重复的表达式
        expr: Box<Ast>,
        /// 最小重复次数
        min: u32,
        /// 最大重复次数（None 表示无上限）
        max: Option<u32>,
    },

    /// 捕获组（用于控制优先级）
    ///
    /// 即使不需要捕获，组也用于控制优先级。
    ///
    /// # 示例
    ///
    /// `(a|b)` → `Group(Choice([a, b]))`
    Group(Box<Ast>),

    /// 控制标记（如 `(?i)` 忽略大小写）
    ///
    /// # 示例
    ///
    /// `(?i)abc` → `Flags(CASE_INSENSITIVE)`, `Sequence([a, b, c])`
    Flags(Flags),
}

impl Ast {
    /// 创建字面量 AST
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let a = Ast::literal('a');
    /// assert_eq!(a, Ast::Literal('a'));
    /// ```
    pub fn literal(c: char) -> Self {
        Ast::Literal(c)
    }

    /// 创建序列 AST
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let seq = Ast::sequence(vec![
    ///     Ast::literal('a'),
    ///     Ast::literal('b'),
    /// ]);
    /// assert_eq!(seq, Ast::Sequence(vec![Ast::literal('a'), Ast::literal('b')]));
    /// ```
    pub fn sequence(elements: Vec<Ast>) -> Self {
        // 自动扁平化嵌套的序列
        let mut flattened = Vec::new();
        for elem in elements {
            if let Ast::Sequence(inner) = elem {
                flattened.extend(inner);
            } else {
                flattened.push(elem);
            }
        }

        if flattened.is_empty() {
            Ast::Empty
        } else if flattened.len() == 1 {
            flattened.into_iter().next().unwrap()
        } else {
            Ast::Sequence(flattened)
        }
    }

    /// 创建选择 AST
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let choice = Ast::choice(vec![
    ///     Ast::literal('a'),
    ///     Ast::literal('b'),
    /// ]);
    /// assert_eq!(choice, Ast::Choice(vec![Ast::literal('a'), Ast::literal('b')]));
    /// ```
    pub fn choice(choices: Vec<Ast>) -> Self {
        // 自动扁平化嵌套的选择
        let mut flattened = Vec::new();
        for choice in choices {
            if let Ast::Choice(inner) = choice {
                flattened.extend(inner);
            } else {
                flattened.push(choice);
            }
        }

        if flattened.is_empty() {
            Ast::Empty
        } else if flattened.len() == 1 {
            flattened.into_iter().next().unwrap()
        } else {
            Ast::Choice(flattened)
        }
    }

    /// 创建零次或多次重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let star = Ast::zero_or_more(Ast::literal('a'));
    /// assert_eq!(star, Ast::ZeroOrMore(Box::new(Ast::literal('a'))));
    /// ```
    pub fn zero_or_more(expr: Ast) -> Self {
        Ast::ZeroOrMore(Box::new(expr))
    }

    /// 创建一次或多次重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let plus = Ast::one_or_more(Ast::literal('a'));
    /// assert_eq!(plus, Ast::OneOrMore(Box::new(Ast::literal('a'))));
    /// ```
    pub fn one_or_more(expr: Ast) -> Self {
        Ast::OneOrMore(Box::new(expr))
    }

    /// 创建零次或一次
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let optional = Ast::zero_or_one(Ast::literal('a'));
    /// assert_eq!(optional, Ast::ZeroOrOne(Box::new(Ast::literal('a'))));
    /// ```
    pub fn zero_or_one(expr: Ast) -> Self {
        Ast::ZeroOrOne(Box::new(expr))
    }

    /// 创建精确重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let repeat = Ast::repeat_exact(Ast::literal('a'), 3);
    /// assert_eq!(repeat, Ast::Repeat { expr: Box::new(Ast::literal('a')), min: 3, max: Some(3) });
    /// ```
    pub fn repeat_exact(expr: Ast, count: u32) -> Self {
        Ast::Repeat {
            expr: Box::new(expr),
            min: count,
            max: Some(count),
        }
    }

    /// 创建范围重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let repeat = Ast::repeat_range(Ast::literal('a'), 2, Some(5));
    /// assert_eq!(repeat, Ast::Repeat { expr: Box::new(Ast::literal('a')), min: 2, max: Some(5) });
    /// ```
    pub fn repeat_range(expr: Ast, min: u32, max: Option<u32>) -> Self {
        Ast::Repeat {
            expr: Box::new(expr),
            min,
            max,
        }
    }

    /// 创建捕获组
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let group = Ast::group(Ast::literal('a'));
    /// assert_eq!(group, Ast::Group(Box::new(Ast::literal('a'))));
    /// ```
    pub fn group(expr: Ast) -> Self {
        Ast::Group(Box::new(expr))
    }

    /// 获取所有子节点（用于迭代遍历）
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// let seq = Ast::sequence(vec![Ast::literal('a'), Ast::literal('b')]);
    /// let children = seq.children();
    /// assert_eq!(children.len(), 2);
    /// ```
    pub fn children(&self) -> Vec<&Ast> {
        match self {
            Ast::Empty => vec![],
            Ast::Literal(_) => vec![],
            Ast::Class(_) => vec![],
            Ast::Sequence(seq) => seq.iter().collect(),
            Ast::Choice(choices) => choices.iter().collect(),
            Ast::ZeroOrMore(expr) => vec![expr.as_ref()],
            Ast::OneOrMore(expr) => vec![expr.as_ref()],
            Ast::ZeroOrOne(expr) => vec![expr.as_ref()],
            Ast::Repeat { expr, .. } => vec![expr.as_ref()],
            Ast::Group(expr) => vec![expr.as_ref()],
            Ast::Flags(_) => vec![],
        }
    }

    /// 检查是否为空
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Ast;
    /// assert!(Ast::Empty.is_empty());
    /// assert!(!Ast::literal('a').is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        matches!(self, Ast::Empty)
    }
}

impl fmt::Display for Ast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ast::Empty => write!(f, "∅"),
            Ast::Literal(c) => write!(f, "{}", c),
            Ast::Class(class) => write!(f, "{}", class),
            Ast::Sequence(seq) => {
                write!(f, "(")?;
                for (i, elem) in seq.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Ast::Choice(choices) => {
                write!(f, "(")?;
                for (i, choice) in choices.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", choice)?;
                }
                write!(f, ")")
            }
            Ast::ZeroOrMore(expr) => write!(f, "({})*", expr),
            Ast::OneOrMore(expr) => write!(f, "({})+", expr),
            Ast::ZeroOrOne(expr) => write!(f, "({})?", expr),
            Ast::Repeat { expr, min, max } => {
                write!(f, "({})", expr)?;
                match max {
                    Some(m) if min == m => write!(f, "{{{}}}", min),
                    Some(m) => write!(f, "{{{},{}}}", min, m),
                    None => write!(f, "{{{},}}", min),
                }
            }
            Ast::Group(expr) => write!(f, "({})", expr),
            Ast::Flags(flags) => write!(f, "({})", flags),
        }
    }
}

// ==================== Flags 定义 ====================

/// 控制标记
///
/// 用于修改正则表达式的匹配行为。
///
/// # 示例
///
/// ```
/// # use lex::regex::Flags;
/// let flags = Flags::CASE_INSENSITIVE;
/// assert!(flags.case_insensitive());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags {
    /// 忽略大小写
    pub case_insensitive: bool,
    /// 多行模式（^ 和 $ 匹配行首行尾）
    pub multiline: bool,
    /// 点号匹配换行符
    pub dot_matches_newline: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            case_insensitive: false,
            multiline: false,
            dot_matches_newline: false,
        }
    }
}

impl Flags {
    /// 创建默认标记
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建忽略大小写标记
    pub const fn case_insensitive() -> Self {
        Self {
            case_insensitive: true,
            multiline: false,
            dot_matches_newline: false,
        }
    }

    /// 创建多行模式标记
    pub const fn multiline() -> Self {
        Self {
            case_insensitive: false,
            multiline: true,
            dot_matches_newline: false,
        }
    }

    /// 创建点号匹配换行符标记
    pub const fn dot_matches_newline() -> Self {
        Self {
            case_insensitive: false,
            multiline: false,
            dot_matches_newline: true,
        }
    }

    /// 合并两个标记
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            case_insensitive: self.case_insensitive || other.case_insensitive,
            multiline: self.multiline || other.multiline,
            dot_matches_newline: self.dot_matches_newline || other.dot_matches_newline,
        }
    }

    /// 检查是否忽略大小写
    pub fn is_case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    /// 检查是否为多行模式
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// 检查点号是否匹配换行符
    pub fn is_dot_matches_newline(&self) -> bool {
        self.dot_matches_newline
    }
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.case_insensitive {
            flags.push("i");
        }
        if self.multiline {
            flags.push("m");
        }
        if self.dot_matches_newline {
            flags.push("s");
        }

        if flags.is_empty() {
            write!(f, "(? )")
        } else {
            write!(f, "(?{})", flags.join(""))
        }
    }
}

// ==================== 辅助函数 ====================

/// 创建字面量 AST（便捷函数）
///
/// # 示例
///
/// ```
/// # use lex::regex::ast;
/// let a = ast::literal('a');
/// ```
pub fn literal(c: char) -> Ast {
    Ast::literal(c)
}

/// 创建字符类 AST（便捷函数）
///
/// # 示例
///
/// ```
/// # use lex::regex::ast;
/// # use lex::transition::CharClass;
/// let digit = ast::char_class(CharClass::range('0', '9'));
/// ```
pub fn char_class(class: CharClass) -> Ast {
    Ast::Class(class)
}

/// 创建预定义字符类 AST（便捷函数）
///
/// # 示例
///
/// ```
/// # use lex::regex::ast;
/// # use lex::transition::PredefinedClass;
/// let digit = ast::predefined_class(PredefinedClass::Digit);
/// ```
pub fn predefined_class(class: PredefinedClass) -> Ast {
    Ast::Class(CharClass::predefined(class))
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal() {
        let a = Ast::literal('a');
        assert_eq!(a, Ast::Literal('a'));
        assert_eq!(a.to_string(), "a");
    }

    #[test]
    fn test_sequence() {
        let seq = Ast::sequence(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]);
        assert_eq!(seq, Ast::Sequence(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]));
        assert_eq!(seq.to_string(), "(a b c)");
    }

    #[test]
    fn test_sequence_flatten() {
        // 测试嵌套序列的扁平化
        let seq = Ast::sequence(vec![
            Ast::sequence(vec![Ast::literal('a'), Ast::literal('b')]),
            Ast::literal('c'),
        ]);
        assert_eq!(seq, Ast::Sequence(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]));
    }

    #[test]
    fn test_sequence_empty() {
        let seq = Ast::sequence(vec![]);
        assert_eq!(seq, Ast::Empty);
    }

    #[test]
    fn test_sequence_single() {
        let seq = Ast::sequence(vec![Ast::literal('a')]);
        assert_eq!(seq, Ast::literal('a'));
    }

    #[test]
    fn test_choice() {
        let choice = Ast::choice(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]);
        assert_eq!(choice, Ast::Choice(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]));
        assert_eq!(choice.to_string(), "(a | b | c)");
    }

    #[test]
    fn test_choice_flatten() {
        // 测试嵌套选择的扁平化
        let choice = Ast::choice(vec![
            Ast::choice(vec![Ast::literal('a'), Ast::literal('b')]),
            Ast::literal('c'),
        ]);
        assert_eq!(choice, Ast::Choice(vec![
            Ast::literal('a'),
            Ast::literal('b'),
            Ast::literal('c'),
        ]));
    }

    #[test]
    fn test_zero_or_more() {
        let star = Ast::zero_or_more(Ast::literal('a'));
        assert_eq!(star, Ast::ZeroOrMore(Box::new(Ast::literal('a'))));
        assert_eq!(star.to_string(), "(a)*");
    }

    #[test]
    fn test_one_or_more() {
        let plus = Ast::one_or_more(Ast::literal('a'));
        assert_eq!(plus, Ast::OneOrMore(Box::new(Ast::literal('a'))));
        assert_eq!(plus.to_string(), "(a)+");
    }

    #[test]
    fn test_zero_or_one() {
        let optional = Ast::zero_or_one(Ast::literal('a'));
        assert_eq!(optional, Ast::ZeroOrOne(Box::new(Ast::literal('a'))));
        assert_eq!(optional.to_string(), "(a)?");
    }

    #[test]
    fn test_repeat_exact() {
        let repeat = Ast::repeat_exact(Ast::literal('a'), 3);
        assert_eq!(repeat, Ast::Repeat {
            expr: Box::new(Ast::literal('a')),
            min: 3,
            max: Some(3),
        });
        assert_eq!(repeat.to_string(), "(a){3}");
    }

    #[test]
    fn test_repeat_range() {
        let repeat = Ast::repeat_range(Ast::literal('a'), 2, Some(5));
        assert_eq!(repeat, Ast::Repeat {
            expr: Box::new(Ast::literal('a')),
            min: 2,
            max: Some(5),
        });
        assert_eq!(repeat.to_string(), "(a){2,5}");
    }

    #[test]
    fn test_repeat_unbounded() {
        let repeat = Ast::repeat_range(Ast::literal('a'), 3, None);
        assert_eq!(repeat, Ast::Repeat {
            expr: Box::new(Ast::literal('a')),
            min: 3,
            max: None,
        });
        assert_eq!(repeat.to_string(), "(a){3,}");
    }

    #[test]
    fn test_group() {
        let group = Ast::group(Ast::literal('a'));
        assert_eq!(group, Ast::Group(Box::new(Ast::literal('a'))));
        assert_eq!(group.to_string(), "(a)");
    }

    #[test]
    fn test_children() {
        let seq = Ast::sequence(vec![
            Ast::literal('a'),
            Ast::literal('b'),
        ]);
        let children = seq.children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], &Ast::literal('a'));
        assert_eq!(children[1], &Ast::literal('b'));
    }

    #[test]
    fn test_is_empty() {
        assert!(Ast::Empty.is_empty());
        assert!(!Ast::literal('a').is_empty());
        assert!(!Ast::sequence(vec![Ast::literal('a')]).is_empty());
    }

    #[test]
    fn test_flags() {
        let flags = Flags::case_insensitive();
        assert!(flags.is_case_insensitive());
        assert!(!flags.is_multiline());
        assert_eq!(flags.to_string(), "(?i)");
    }

    #[test]
    fn test_flags_merge() {
        let flags1 = Flags::case_insensitive();
        let flags2 = Flags::multiline();
        let merged = flags1.merge(&flags2);
        assert!(merged.is_case_insensitive());
        assert!(merged.is_multiline());
        assert_eq!(merged.to_string(), "(?im)");
    }
}