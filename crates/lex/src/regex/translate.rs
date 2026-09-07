//! AST 到 HIR 的翻译模块
//!
//! 该模块将 AST（抽象语法树）翻译为 HIR（高级中间表示）。
//!
//! # 设计特点
//!
//! - **规范化**：展开重复、简化序列
//! - **控件标记处理**：`(?i)` 按 PCRE 作用域语义展开 ASCII 字面量
//! - **智能构造**：自动扁平化嵌套的序列和选择
//!
//! # 核心类型
//!
//! - [`Translator`] - AST 到 HIR 的翻译器
//!
//! # 示例
//!
//! ```
//! use lex::regex::Translate;
//!
//! let ast = lex::regex::parse("a|b").unwrap();
//! let mut translator = Translate::new();
//! let hir = translator.translate(&ast);
//! ```

use crate::regex::ast::{Ast, Flags};
use crate::regex::hir::Hir;
use crate::transition::CharClass;

// ==================== 翻译器 ====================

/// AST 到 HIR 的翻译器
///
/// 将 AST 翻译为规范化的 HIR，同时处理控制标记和展开重复。
///
/// # 设计特点
///
/// - **智能构造**：自动合并相邻的字面量序列
/// - **标记处理**：根据忽略大小写标记展开字面量
/// - **重复展开**：展开精确重复（如 `{3}` → `aaa`）
///
/// # 示例
///
/// ```
/// # use lex::regex::Translate;
/// # use lex::regex::parse;
///
/// let ast = parse("a|b").unwrap();
/// let mut translator = Translate::new();
/// let hir = translator.translate(&ast);
///
/// println!("HIR: {:?}", hir);
/// ```
pub struct Translator {
    /// 当前生效的控制标记
    flags: Flags,
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

impl Translator {
    /// 创建新的翻译器
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Translate;
    ///
    /// let translator = Translate::new();
    /// ```
    pub fn new() -> Self {
        Self {
            flags: Flags::new(),
        }
    }

    /// 翻译 AST 为 HIR
    ///
    /// # 参数
    ///
    /// - `ast` - 要翻译的 AST
    ///
    /// # 返回
    ///
    /// 翻译后的 HIR
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Translate;
    /// # use lex::regex::parse;
    /// # use lex::regex::Hir;
    ///
    /// let ast = parse("a|b").unwrap();
    /// let mut translator = Translate::new();
    /// let hir = translator.translate(&ast);
    ///
    /// assert!(matches!(hir, Hir::Choice(_)));
    /// ```
    pub fn translate(&mut self, ast: &Ast) -> Hir {
        match ast {
            Ast::Empty => Hir::Empty,
            Ast::Literal(c) => self.translate_literal(*c),
            Ast::Class(class) => Hir::Class(class.clone()),
            Ast::Sequence(seq) => self.translate_sequence(seq),
            Ast::Choice(choices) => self.translate_choice(choices),
            Ast::ZeroOrMore(expr) => {
                let hir = self.translate(expr);
                Hir::zero_or_more(hir)
            }
            Ast::OneOrMore(expr) => {
                let hir = self.translate(expr);
                Hir::one_or_more(hir)
            }
            Ast::ZeroOrOne(expr) => {
                let hir = self.translate(expr);
                Hir::zero_or_one(hir)
            }
            Ast::Repeat { expr, min, max } => self.translate_repeat(expr, *min, *max),
            Ast::Group(expr) => {
                // PCRE 语义：`(?i)` 作用到所在分组结束；出组恢复快照。
                // 分支内设置的标记跨 `|` 后续分支继承，直到分组结束。
                let saved = self.flags;
                let hir = self.translate(expr);
                self.flags = saved;
                hir
            }
            Ast::Flags(flags) => {
                // 设置后持续生效（顶层不恢复），由 Group 臂负责作用域恢复
                self.flags.case_insensitive = flags.case_insensitive;
                Hir::Empty
            }
        }
    }

    /// 翻译字面量，考虑控制标记
    ///
    /// 仅 ASCII 字母参与忽略大小写折叠（展开为大小写并集字符类，
    /// 单状态一次区间测试）；非 ASCII 字符不折叠。
    fn translate_literal(&self, c: char) -> Hir {
        if self.flags.case_insensitive && c.is_ascii_alphabetic() {
            let lower = c.to_ascii_lowercase();
            let upper = c.to_ascii_uppercase();
            Hir::Class(CharClass::from_ranges([(lower, lower), (upper, upper)]))
        } else {
            Hir::Literal(c)
        }
    }

    /// 翻译序列
    fn translate_sequence(&mut self, seq: &[Ast]) -> Hir {
        let mut elements = Vec::new();
        
        for sub in seq {
            let hir = self.translate(sub);
            
            // 扁平化：如果翻译结果也是序列，直接展开
            if let Hir::Sequence(inner) = hir {
                elements.extend(inner);
            } else {
                elements.push(hir);
            }
        }
        
        // 智能构造：使用 Hir::sequence 自动扁平化
        Hir::sequence(elements)
    }

    /// 翻译选择
    fn translate_choice(&mut self, choices: &[Ast]) -> Hir {
        let translated: Vec<Hir> = choices.iter()
            .map(|c| self.translate(c))
            .collect();
        
        // 智能构造：使用 Hir::choice 自动扁平化
        Hir::choice(translated)
    }

    /// 翻译重复
    fn translate_repeat(&mut self, expr: &Ast, min: u32, max: Option<u32>) -> Hir {
        let hir = self.translate(expr);
        
        // 优化：展开小次数的精确重复为序列
        if max == Some(min) && min <= 10 {
            let mut seq = Vec::with_capacity(min as usize);
            for _ in 0..min {
                seq.push(hir.clone());
            }
            return Hir::sequence(seq);
        }
        
        // 优化：展开 {0,1} 为 ?
        if min == 0 && max == Some(1) {
            return Hir::zero_or_one(hir);
        }
        
        // 优化：展开 {1,} 为 +
        if min == 1 && max.is_none() {
            return Hir::one_or_more(hir);
        }
        
        // 优化：展开 {0,} 为 *
        if min == 0 && max.is_none() {
            return Hir::zero_or_more(hir);
        }
        
        Hir::Repeat {
            expr: Box::new(hir),
            min,
            max,
        }
    }
}

// ==================== 重新导出 ====================

pub use Translator as Translate;

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regex::parse::parse;

    #[test]
    fn test_translate_literal() {
        let ast = parse("a").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::Literal('a'));
    }

    #[test]
    fn test_translate_sequence() {
        let ast = parse("ab").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
    }

    #[test]
    fn test_translate_choice() {
        let ast = parse("a|b").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::choice(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
    }

    #[test]
    fn test_translate_star() {
        let ast = parse("a*").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::zero_or_more(Hir::literal('a')));
    }

    #[test]
    fn test_translate_plus() {
        let ast = parse("a+").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::one_or_more(Hir::literal('a')));
    }

    #[test]
    fn test_translate_optional() {
        let ast = parse("a?").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::zero_or_one(Hir::literal('a')));
    }

    #[test]
    fn test_translate_repeat_exact() {
        let ast = parse("a{3}").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        // 应该展开为序列
        assert_eq!(hir, Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('a'),
            Hir::literal('a'),
        ]));
    }

    #[test]
    fn test_translate_repeat_range() {
        let ast = parse("a{2,5}").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert!(matches!(hir, Hir::Repeat { min: 2, max: Some(5), .. }));
    }

    #[test]
    fn test_translate_repeat_unbounded() {
        let ast = parse("a{3,}").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert!(matches!(hir, Hir::Repeat { min: 3, max: None, .. }));
    }

    #[test]
    fn test_translate_group() {
        let ast = parse("(a|b)").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert_eq!(hir, Hir::choice(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
    }

    #[test]
    fn test_translate_complex() {
        let ast = parse("[a-zA-Z][a-zA-Z0-9_]*").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        // 验证 HIR 结构
        assert!(matches!(hir, Hir::Sequence(_)));
    }

    #[test]
    fn test_translate_char_class() {
        let ast = parse("[a-z]").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert!(matches!(hir, Hir::Class(_)));
    }

    #[test]
    fn test_translate_escape() {
        let ast = parse("\\d").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        assert!(matches!(hir, Hir::Class(_)));
    }

    #[test]
    fn test_translate_nested() {
        let ast = parse("((a)(b))").unwrap();
        let mut translator = Translator::new();
        let hir = translator.translate(&ast);
        
        // 嵌套的组应该在翻译时展开
        assert_eq!(hir, Hir::sequence(vec![
            Hir::literal('a'),
            Hir::literal('b'),
        ]));
    }
}
