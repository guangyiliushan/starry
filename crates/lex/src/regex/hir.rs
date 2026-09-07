//! 高级中间表示（HIR）模块
//!
//! 该模块定义了正则表达式的高级中间表示，经过 AST 的翻译和优化。
//!
//! # 设计特点
//!
//! - **规范化**：HIR 是规范化形式，便于分析和优化
//! - **扁平化**：序列和选择都使用 `Vec` 存储，避免深度嵌套
//! - **优化就绪**：已经过初步优化，适合进行进一步的分析和编译
//!
//! # 核心类型
//!
//! - [`Hir`] - 高级中间表示枚举
//!
//! # 示例
//!
//! ```
//! use lex::regex::{Parser, Translate};
//!
//! let mut parser = Parser::new("a|b");
//! let ast = parser.parse().unwrap();
//!
//! let mut translator = Translate::new();
//! let hir = translator.translate(&ast);
//!
//! println!("HIR: {:?}", hir);
//! ```

use std::fmt;
use crate::transition::CharClass;

// ==================== HIR 枚举 ====================

/// 高级中间表示（HIR）
///
/// HIR 是经过 AST 翻译和优化后的规范化形式，适合用于 NFA 构建和进一步优化。
///
/// # 与 AST 的区别
///
/// - **规范化**：HIR 经过规范化，如展开重复、简化序列等
/// - **扁平化**：序列和选择都使用 `Vec`，避免深度嵌套
/// - **优化就绪**：已经过初步优化，便于进一步分析和编译
///
/// # 设计特点
///
/// - **扁平化序列**：`Sequence(Vec<Hir>)` 保持扁平结构
/// - **扁平化选择**：`Choice(Vec<Hir>)` 保持扁平结构
/// - **直接对应 Thompson 构造**：每个变体都有明确的 NFA 构造方法
///
/// # 示例
///
/// ```
/// # use lex::regex::Hir;
///
/// // 字面量
/// let a = Hir::Literal('a');
///
/// // 序列：abc
/// let abc = Hir::Sequence(vec![
///     Hir::Literal('a'),
///     Hir::Literal('b'),
///     Hir::Literal('c'),
/// ]);
///
/// // 重复：a*
/// let star = Hir::ZeroOrMore(Box::new(Hir::Literal('a')));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hir {
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
    /// 使用 `Vec` 而非嵌套结构，保持扁平化。
    ///
    /// # 示例
    ///
    /// `abc` → `Sequence([a, b, c])`
    Sequence(Vec<Hir>),

    /// 选择：匹配其中之一
    ///
    /// 使用 `Vec` 而非嵌套结构，保持扁平化。
    ///
    /// # 示例
    ///
    /// `a|b|c` → `Choice([a, b, c])`
    Choice(Vec<Hir>),

    /// 零次或多次重复（`*`）
    ///
    /// # 示例
    ///
    /// `a*` → `ZeroOrMore(a)`
    ZeroOrMore(Box<Hir>),

    /// 一次或多次重复（`+`）
    ///
    /// # 示例
    ///
    /// `a+` → `OneOrMore(a)`
    OneOrMore(Box<Hir>),

    /// 零次或一次（`?`）
    ///
    /// # 示例
    ///
    /// `a?` → `ZeroOrOne(a)`
    ZeroOrOne(Box<Hir>),

    /// 重复指定次数（`{n}`, `{n,m}`, `{n,}`)
    ///
    /// 注意：HIR 中的重复是规范化的，如 `{n}` 会被展开为重复 n 次。
    ///
    /// # 示例
    ///
    /// `a{3}` → `Sequence([a, a, a])`
    /// `a{2,5}` → 展开为多个选择（复杂情况）
    Repeat {
        /// 要重复的表达式
        expr: Box<Hir>,
        /// 最小重复次数
        min: u32,
        /// 最大重复次数（None 表示无上限）
        max: Option<u32>,
    },
}

impl Hir {
    /// 创建字面量 HIR
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let a = Hir::literal('a');
    /// assert_eq!(a, Hir::Literal('a'));
    /// ```
    pub fn literal(c: char) -> Self {
        Hir::Literal(c)
    }

    /// 创建序列 HIR
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let seq = Hir::sequence(vec![
    ///     Hir::literal('a'),
    ///     Hir::literal('b'),
    /// ]);
    /// assert_eq!(seq, Hir::Sequence(vec![Hir::literal('a'), Hir::literal('b')]));
    /// ```
    pub fn sequence(elements: Vec<Hir>) -> Self {
        // 自动扁平化嵌套的序列
        let mut flattened = Vec::new();
        for elem in elements {
            if let Hir::Sequence(inner) = elem {
                flattened.extend(inner);
            } else {
                flattened.push(elem);
            }
        }

        if flattened.is_empty() {
            Hir::Empty
        } else if flattened.len() == 1 {
            flattened.into_iter().next().unwrap()
        } else {
            Hir::Sequence(flattened)
        }
    }

    /// 创建选择 HIR
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let choice = Hir::choice(vec![
    ///     Hir::literal('a'),
    ///     Hir::literal('b'),
    /// ]);
    /// assert_eq!(choice, Hir::Choice(vec![Hir::literal('a'), Hir::literal('b')]));
    /// ```
    pub fn choice(choices: Vec<Hir>) -> Self {
        // 自动扁平化嵌套的选择
        let mut flattened = Vec::new();
        for choice in choices {
            if let Hir::Choice(inner) = choice {
                flattened.extend(inner);
            } else {
                flattened.push(choice);
            }
        }

        if flattened.is_empty() {
            Hir::Empty
        } else if flattened.len() == 1 {
            flattened.into_iter().next().unwrap()
        } else {
            Hir::Choice(flattened)
        }
    }

    /// 创建零次或多次重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let star = Hir::zero_or_more(Hir::literal('a'));
    /// assert_eq!(star, Hir::ZeroOrMore(Box::new(Hir::literal('a'))));
    /// ```
    pub fn zero_or_more(expr: Hir) -> Self {
        Hir::ZeroOrMore(Box::new(expr))
    }

    /// 创建一次或多次重复
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let plus = Hir::one_or_more(Hir::literal('a'));
    /// assert_eq!(plus, Hir::OneOrMore(Box::new(Hir::literal('a'))));
    /// ```
    pub fn one_or_more(expr: Hir) -> Self {
        Hir::OneOrMore(Box::new(expr))
    }

    /// 创建零次或一次
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let optional = Hir::zero_or_one(Hir::literal('a'));
    /// assert_eq!(optional, Hir::ZeroOrOne(Box::new(Hir::literal('a'))));
    /// ```
    pub fn zero_or_one(expr: Hir) -> Self {
        Hir::ZeroOrOne(Box::new(expr))
    }

    /// 检查是否为空
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// assert!(Hir::Empty.is_empty());
    /// assert!(!Hir::literal('a').is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        matches!(self, Hir::Empty)
    }

    /// 获取所有子节点（用于迭代遍历）
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let seq = Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]);
    /// let children = seq.children();
    /// assert_eq!(children.len(), 2);
    /// ```
    pub fn children(&self) -> Vec<&Hir> {
        match self {
            Hir::Empty => vec![],
            Hir::Literal(_) => vec![],
            Hir::Class(_) => vec![],
            Hir::Sequence(seq) => seq.iter().collect(),
            Hir::Choice(choices) => choices.iter().collect(),
            Hir::ZeroOrMore(expr) => vec![expr.as_ref()],
            Hir::OneOrMore(expr) => vec![expr.as_ref()],
            Hir::ZeroOrOne(expr) => vec![expr.as_ref()],
            Hir::Repeat { expr, .. } => vec![expr.as_ref()],
        }
    }

    /// 获取子节点的可变引用（用于优化）
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Hir;
    /// let mut seq = Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]);
    /// for child in seq.children_mut() {
    ///     println!("{:?}", child);
    /// }
    /// ```
    pub fn children_mut(&mut self) -> Vec<&mut Hir> {
        match self {
            Hir::Empty => vec![],
            Hir::Literal(_) => vec![],
            Hir::Class(_) => vec![],
            Hir::Sequence(seq) => seq.iter_mut().collect(),
            Hir::Choice(choices) => choices.iter_mut().collect(),
            Hir::ZeroOrMore(expr) => vec![expr.as_mut()],
            Hir::OneOrMore(expr) => vec![expr.as_mut()],
            Hir::ZeroOrOne(expr) => vec![expr.as_mut()],
            Hir::Repeat { expr, .. } => vec![expr.as_mut()],
        }
    }
}

impl fmt::Display for Hir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hir::Empty => write!(f, "∅"),
            Hir::Literal(c) => write!(f, "{}", c),
            Hir::Class(class) => write!(f, "{}", class),
            Hir::Sequence(seq) => {
                write!(f, "(")?;
                for (i, elem) in seq.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Hir::Choice(choices) => {
                write!(f, "(")?;
                for (i, choice) in choices.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", choice)?;
                }
                write!(f, ")")
            }
            Hir::ZeroOrMore(expr) => write!(f, "({})*", expr),
            Hir::OneOrMore(expr) => write!(f, "({})+", expr),
            Hir::ZeroOrOne(expr) => write!(f, "({})?", expr),
            Hir::Repeat { expr, min, max } => {
                write!(f, "({})", expr)?;
                match max {
                    Some(m) if min == m => write!(f, "{{{}}}", min),
                    Some(m) => write!(f, "{{{}, {}}}", min, m),
                    None => write!(f, "{{{},}}", min),
                }
            }
        }
    }
}

// ==================== 辅助函数 ====================

/// 创建字面量 HIR（便捷函数）
///
/// # 示例
///
/// ```
/// # use lex::regex::hir;
/// let a = hir::literal('a');
/// ```
pub fn literal(c: char) -> Hir {
    Hir::literal(c)
}

/// 创建字符类 HIR（便捷函数）
///
/// # 示例
///
/// ```
/// # use lex::regex::hir;
/// # use lex::transition::CharClass;
/// let digit = hir::char_class(CharClass::range('0', '9'));
/// ```
pub fn char_class(class: CharClass) -> Hir {
    Hir::Class(class)
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hir_literal() {
        let a = Hir::literal('a');
        assert_eq!(a, Hir::Literal('a'));
        assert_eq!(a.to_string(), "a");
    }

    #[test]
    fn test_hir_sequence() {
        let seq = Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]);
        assert_eq!(seq, Hir::Sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
        assert_eq!(seq.to_string(), "(a b)");
    }

    #[test]
    fn test_hir_sequence_flatten() {
        // 测试嵌套序列的扁平化
        let seq = Hir::sequence(vec![
            Hir::sequence(vec![Hir::literal('a'), Hir::literal('b')]),
            Hir::literal('c'),
        ]);
        assert_eq!(seq, Hir::Sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
            Hir::literal('c'),
        ]));
    }

    #[test]
    fn test_hir_choice() {
        let choice = Hir::choice(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]);
        assert_eq!(choice, Hir::Choice(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
        assert_eq!(choice.to_string(), "(a | b)");
    }

    #[test]
    fn test_hir_zero_or_more() {
        let star = Hir::zero_or_more(Hir::literal('a'));
        assert_eq!(star, Hir::ZeroOrMore(Box::new(Hir::literal('a'))));
        assert_eq!(star.to_string(), "(a)*");
    }

    #[test]
    fn test_hir_is_empty() {
        assert!(Hir::Empty.is_empty());
        assert!(!Hir::literal('a').is_empty());
    }
}
