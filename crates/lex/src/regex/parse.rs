//! 正则表达式解析器模块
//!
//! 该模块提供正则表达式字符串到 AST 的解析功能。
//!
//! # 设计特点
//!
//! - **递归下降解析**：清晰直观，易于理解和调试
//! - **错误恢复**：支持错误恢复，继续解析剩余内容
//! - **嵌套深度限制**：防止深层嵌套导致的栈溢出
//! - **友好的错误信息**：提供详细的错误位置和原因
//!
//! # 核心类型
//!
//! - [`Parser`] - 解析器
//! - [`ParseError`] - 解析错误类型
//! - [`ParseResult`] - 解析结果（包含 AST、警告、错误）
//!
//! # 示例
//!
//! ```
//! use lex::regex::{Parser, parse};
//!
//! // 简单解析
//! let ast = parse("a|b").unwrap();
//!
//! // 使用 Parser 进行高级控制
//! let parser = Parser::new("[a-zA-Z][a-zA-Z0-9_]*");
//! let result = parser.parse_with_recovery();
//! ```

use std::fmt;
use crate::regex::ast::{Ast, Flags};
use crate::transition::{CharClass, PredefinedClass};

// ==================== 解析器 ====================

/// 正则表达式解析器
///
/// 使用递归下降算法解析正则表达式字符串。
///
/// # 设计特点
///
/// - 支持错误恢复，提供详细的错误信息
/// - 支持嵌套深度限制，防止栈溢出
/// - 使用迭代遍历而非递归，避免栈溢出
///
/// # 示例
///
/// ```
/// # use lex::regex::Parser;
///
/// let mut parser = Parser::new("a|b");
/// let ast = parser.parse().unwrap();
///
/// println!("AST: {:?}", ast);
/// ```
pub struct Parser {
    input: Vec<char>,
    pos: usize,
    nest_limit: usize,
    current_depth: usize,
}

impl Parser {
    /// 创建新的解析器
    ///
    /// # 参数
    ///
    /// - `input` - 要解析的正则表达式字符串
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Parser;
    ///
    /// let parser = Parser::new("a|b");
    /// ```
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            nest_limit: 100,
            current_depth: 0,
        }
    }

    /// 设置嵌套深度限制
    ///
    /// # 参数
    ///
    /// - `limit` - 最大嵌套深度
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Parser;
    ///
    /// let mut parser = Parser::new("a|b");
    /// parser.set_nest_limit(200);
    /// ```
    pub fn set_nest_limit(&mut self, limit: usize) {
        self.nest_limit = limit;
    }

    /// 解析正则表达式（严格模式）
    ///
    /// # 返回
    ///
    /// 成功返回 AST，失败返回错误
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Parser;
    ///
    /// let parser = Parser::new("a|b");
    /// let ast = parser.parse().unwrap();
    /// ```
    pub fn parse(&mut self) -> Result<Ast, ParseError> {
        let result = self.parse_alt()?;

        // 确保解析完所有输入
        if !self.is_eof() {
            return Err(ParseError::UnexpectedChar {
                found: self.peek().unwrap(),
                expected: "end of input",
                position: self.pos,
            });
        }

        Ok(result)
    }

    /// 解析正则表达式（支持错误恢复）
    ///
    /// # 返回
    ///
    /// 返回包含 AST、警告和错误的解析结果
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::Parser;
    ///
    /// let mut parser = Parser::new("a|b");
    /// let result = parser.parse_with_recovery();
    /// ```
    pub fn parse_with_recovery(&mut self) -> ParseResult {
        let warnings = Vec::new();
        let mut errors = Vec::new();

        let ast = match self.parse_alt() {
            Ok(ast) => ast,
            Err(e) => {
                errors.push(e.clone());
                Ast::Empty
            }
        };

        ParseResult {
            ast,
            warnings,
            errors,
        }
    }

    // ==================== 解析方法 ====================

    /// 解析选择（alt）
    ///
    /// 语法：sequence ('|' sequence)*
    fn parse_alt(&mut self) -> Result<Ast, ParseError> {
        let mut choices = Vec::new();
        choices.push(self.parse_sequence()?);

        while self.peek() == Some('|') {
            self.bump();
            choices.push(self.parse_sequence()?);
        }

        if choices.len() == 1 {
            Ok(choices.into_iter().next().unwrap())
        } else {
            Ok(Ast::Choice(choices))
        }
    }

    /// 解析序列（sequence）
    ///
    /// 语法：repeat+
    fn parse_sequence(&mut self) -> Result<Ast, ParseError> {
        let mut elements = Vec::new();

        while !self.is_end_of_sequence() {
            elements.push(self.parse_repeat()?);
        }

        if elements.is_empty() {
            Ok(Ast::Empty)
        } else if elements.len() == 1 {
            Ok(elements.into_iter().next().unwrap())
        } else {
            Ok(Ast::sequence(elements))
        }
    }

    /// 检查是否到达序列结束
    fn is_end_of_sequence(&self) -> bool {
        match self.peek() {
            None | Some('|') | Some(')') => true,
            _ => false,
        }
    }

    /// 解析重复（repeat）
    ///
    /// 语法：atom ('*' | '+' | '?' | '{' number (',' number?)? '}')?
    fn parse_repeat(&mut self) -> Result<Ast, ParseError> {
        let atom = self.parse_atom()?;

        if !self.peek_one_of("*+?{") {
            return Ok(atom);
        }

        match self.peek().unwrap() {
            '*' => {
                self.bump();
                Ok(Ast::zero_or_more(atom))
            }
            '+' => {
                self.bump();
                Ok(Ast::one_or_more(atom))
            }
            '?' => {
                self.bump();
                Ok(Ast::zero_or_one(atom))
            }
            '{' => {
                self.bump();
                let (min, max) = self.parse_repeat_count()?;
                self.expect('}')?;

                let repeat = Ast::repeat_range(atom, min, max);

                // 校验 Repeat 语义
                if let Some(max_val) = max {
                    if min > max_val {
                        return Err(ParseError::InvalidRepeatRange {
                            min,
                            max: max_val,
                            message: "最小重复次数不能大于最大重复次数".to_string(),
                        });
                    }
                }

                // 避免过大的重复次数
                const MAX_REPEAT: u32 = 1000;
                if min > MAX_REPEAT || max.map_or(false, |m| m > MAX_REPEAT) {
                    return Err(ParseError::RepeatCountTooLarge {
                        limit: MAX_REPEAT,
                    });
                }

                Ok(repeat)
            }
            _ => unreachable!(),
        }
    }

    /// 解析重复次数
    ///
    /// 语法：number | number, | number, number
    fn parse_repeat_count(&mut self) -> Result<(u32, Option<u32>), ParseError> {
        let min = self.parse_number()?;

        match self.peek() {
            Some(',') => {
                self.bump();
                if self.peek() == Some('}') {
                    // {n,} - 无上限
                    Ok((min, None))
                } else {
                    // {n,m} - 有上限
                    let max = self.parse_number()?;
                    Ok((min, Some(max)))
                }
            }
            Some('}') => {
                // {n} - 精确次数
                Ok((min, Some(min)))
            }
            _ => Err(ParseError::Expected {
                expected: "',' or '}'".to_string(),
                found: format!("{:?}", self.peek()),
                position: self.pos,
            }),
        }
    }

    /// 解析数字
    fn parse_number(&mut self) -> Result<u32, ParseError> {
        let mut num = 0u32;
        let mut has_digit = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                has_digit = true;
                num = num * 10 + (c as u32 - '0' as u32);
                self.bump();
            } else {
                break;
            }
        }

        if !has_digit {
            return Err(ParseError::Expected {
                expected: "digit".to_string(),
                found: format!("{:?}", self.peek()),
                position: self.pos,
            });
        }

        Ok(num)
    }

    /// 解析原子（atom）
    ///
    /// 语法：literal | char_class | '(' regex ')' | '(?' flags ')' | '\' escaped_char
    fn parse_atom(&mut self) -> Result<Ast, ParseError> {
        match self.peek() {
            Some('(') => self.parse_group(),
            Some('[') => self.parse_char_class(),
            Some('\\') => self.parse_escape(),
            Some('|') => Ok(Ast::Empty),
            Some(')') => Err(ParseError::UnexpectedChar {
                found: ')',
                expected: "expression",
                position: self.pos,
            }),
            Some(c) => {
                self.bump();
                Ok(Ast::Literal(c))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// 解析分组（group）
    ///
    /// 语法：'(' regex ')' | '(?' flags ')'
    fn parse_group(&mut self) -> Result<Ast, ParseError> {
        self.expect('(')?;

        // 检查是否是控制标记 (?...)
        if self.peek() == Some('?') {
            self.bump();
            return self.parse_flags_group();
        }

        // 检查嵌套深度
        if self.current_depth >= self.nest_limit {
            return Err(ParseError::NestLimitExceeded {
                limit: self.nest_limit,
            });
        }

        self.current_depth += 1;
        let inner = self.parse_alt()?;
        self.current_depth -= 1;

        self.expect(')')?;

        Ok(Ast::group(inner))
    }

    /// 解析控制标记组
    ///
    /// 语法：'(?' flags ')'
    fn parse_flags_group(&mut self) -> Result<Ast, ParseError> {
        let mut flags = Flags::new();

        while let Some(c) = self.peek() {
            match c {
                'i' => {
                    flags.case_insensitive = true;
                    self.bump();
                }
                'm' => {
                    flags.multiline = true;
                    self.bump();
                }
                's' => {
                    flags.dot_matches_newline = true;
                    self.bump();
                }
                '-' => {
                    // 取消标记
                    self.bump();
                    if let Some(flag) = self.peek() {
                        match flag {
                            'i' => flags.case_insensitive = false,
                            'm' => flags.multiline = false,
                            's' => flags.dot_matches_newline = false,
                            _ => {
                                return Err(ParseError::InvalidFlag {
                                    flag,
                                    message: "未知或无效的控制标记".to_string(),
                                });
                            }
                        }
                        self.bump();
                    }
                }
                ')' => {
                    self.bump();
                    return Ok(Ast::Flags(flags));
                }
                _ => {
                    return Err(ParseError::InvalidFlag {
                        flag: c,
                        message: "未知或无效的控制标记".to_string(),
                    });
                }
            }
        }

        Err(ParseError::UnclosedGroup)
    }

    /// 解析字符类
    ///
    /// 语法：'[' ('^'? range+ ']'
    fn parse_char_class(&mut self) -> Result<Ast, ParseError> {
        self.expect('[')?;

        let negated = if self.peek() == Some('^') {
            self.bump();
            true
        } else {
            false
        };

        let mut class = CharClass::new();

        while let Some(c) = self.peek() {
            if c == ']' {
                break;
            }

            if c == '\\' {
                // 处理转义的字符类
                self.bump();
                match self.peek() {
                    Some('d') => {
                        self.bump();
                        class = class.union(&CharClass::predefined(PredefinedClass::Digit));
                    }
                    Some('w') => {
                        self.bump();
                        class = class.union(&CharClass::predefined(PredefinedClass::Word));
                    }
                    Some('s') => {
                        self.bump();
                        class = class.union(&CharClass::predefined(PredefinedClass::Whitespace));
                    }
                    Some(escaped) => {
                        self.bump();
                        class = class.union(&CharClass::single(escaped));
                    }
                    None => {
                        return Err(ParseError::UnexpectedEof);
                    }
                }
            } else {
                // 处理字符范围
                let start = c;
                self.bump();

                if self.peek() == Some('-') {
                    self.bump();
                    if let Some(end) = self.peek() {
                        if end != ']' {
                            self.bump();
                            if start <= end {
                                class = class.union(&CharClass::range(start, end));
                            } else {
                                return Err(ParseError::InvalidRange {
                                    start,
                                    end,
                                    message: "范围起点不能大于终点".to_string(),
                                });
                            }
                        }
                    } else {
                        return Err(ParseError::UnexpectedEof);
                    }
                } else {
                    class = class.union(&CharClass::single(start));
                }
            }
        }

        self.expect(']')?;

        if negated {
            class = class.complement();
        }

        Ok(Ast::Class(class))
    }

    /// 解析转义字符
    ///
    /// 语法：'\' char
    fn parse_escape(&mut self) -> Result<Ast, ParseError> {
        self.expect('\\')?;

        match self.peek() {
            Some(c) => {
                self.bump();

                // 处理特殊转义序列
                match c {
                    'd' => Ok(Ast::Class(CharClass::predefined(PredefinedClass::Digit))),
                    'w' => Ok(Ast::Class(CharClass::predefined(PredefinedClass::Word))),
                    's' => Ok(Ast::Class(CharClass::predefined(PredefinedClass::Whitespace))),
                    'n' => Ok(Ast::Literal('\n')),
                    'r' => Ok(Ast::Literal('\r')),
                    't' => Ok(Ast::Literal('\t')),
                    '\\' => Ok(Ast::Literal('\\')),
                    _ => Ok(Ast::Literal(c)),
                }
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    // ==================== 辅助方法 ====================

    /// 查看下一个字符
    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    /// 检查下一个字符是否在给定集合中
    fn peek_one_of(&self, chars: &str) -> bool {
        self.peek().map_or(false, |c| chars.contains(c))
    }

    /// 消耗一个字符
    fn bump(&mut self) {
        self.pos += 1;
    }

    /// 检查是否到达输入末尾
    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// 期望特定字符
    fn expect(&mut self, expected: char) -> Result<(), ParseError> {
        match self.peek() {
            Some(c) if c == expected => {
                self.bump();
                Ok(())
            }
            Some(c) => Err(ParseError::Expected {
                expected: expected.to_string(),
                found: c.to_string(),
                position: self.pos,
            }),
            None => Err(ParseError::Expected {
                expected: expected.to_string(),
                found: "end of input".to_string(),
                position: self.pos,
            }),
        }
    }
}

// ==================== 解析错误类型 ====================

/// 解析错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// 意外的文件结束
    UnexpectedEof,

    /// 意外的字符
    UnexpectedChar {
        found: char,
        expected: &'static str,
        position: usize,
    },

    /// 期望特定字符但未找到
    Expected {
        expected: String,
        found: String,
        position: usize,
    },

    /// 未闭合的分组
    UnclosedGroup,

    /// 无效的转义字符
    InvalidEscape {
        char: char,
        message: String,
    },

    /// 无效的字符范围
    InvalidRange {
        start: char,
        end: char,
        message: String,
    },

    /// 无效的重复次数
    InvalidRepeatRange {
        min: u32,
        max: u32,
        message: String,
    },

    /// 重复次数过大
    RepeatCountTooLarge {
        limit: u32,
    },

    /// 超过嵌套深度限制
    NestLimitExceeded {
        limit: usize,
    },

    /// 无效的控制标记
    InvalidFlag {
        flag: char,
        message: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof => {
                write!(f, "Unexpected end of file")
            }
            ParseError::UnexpectedChar { found, expected, position } => {
                write!(f, "Position {}: expected '{}', found '{}'", position + 1, expected, found)
            }
            ParseError::Expected { expected, found, position } => {
                write!(f, "Position {}: expected {}, found {}", position + 1, expected, found)
            }
            ParseError::UnclosedGroup => {
                write!(f, "Unclosed group")
            }
            ParseError::InvalidEscape { char, message } => {
                write!(f, "Invalid escape '\\{}': {}", char, message)
            }
            ParseError::InvalidRange { start, end, message } => {
                write!(f, "Invalid range '{}-{}': {}", start, end, message)
            }
            ParseError::InvalidRepeatRange { min, max, message } => {
                write!(f, "Invalid repeat count {{{},{}}}: {}", min, max, message)
            }
            ParseError::RepeatCountTooLarge { limit } => {
                write!(f, "Repeat count exceeds limit {}", limit)
            }
            ParseError::NestLimitExceeded { limit } => {
                write!(f, "Nesting depth limit {} exceeded", limit)
            }
            ParseError::InvalidFlag { flag, message } => {
                write!(f, "Invalid flag '(?{}': {}", flag, message)
            }
        }
    }
}

impl std::error::Error for ParseError {}

// ==================== 解析结果 ====================

/// 解析结果
///
/// 包含 AST、警告和错误。
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub ast: Ast,
    pub warnings: Vec<ParseWarning>,
    pub errors: Vec<ParseError>,
}

/// 解析警告
#[derive(Debug, Clone)]
pub enum ParseWarning {
    EmptyChoice,
    EmptySequence,
}

// ==================== 便捷函数 ====================

/// 解析正则表达式字符串（便捷函数）
///
/// # 参数
///
/// - `input` - 正则表达式字符串
///
/// # 返回
///
/// 成功返回 AST，失败返回错误
///
/// # 示例
///
/// ```
/// # use lex::regex::parse;
///
/// let ast = parse("a|b").unwrap();
/// println!("{:?}", ast);
/// ```
pub fn parse(input: &str) -> Result<Ast, ParseError> {
    Parser::new(input).parse()
}

/// 解析正则表达式字符串（支持错误恢复，便捷函数）
///
/// # 参数
///
/// - `input` - 正则表达式字符串
///
/// # 返回
///
/// 返回包含 AST、警告和错误的解析结果
///
/// # 示例
///
/// ```
/// # use lex::regex::parse_with_recovery;
///
/// let result = parse_with_recovery("a|b");
/// println!("{:?}", result.ast);
/// ```
pub fn parse_with_recovery(input: &str) -> ParseResult {
    Parser::new(input).parse_with_recovery()
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literal() {
        let mut parser = Parser::new("a");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::Literal('a'));
    }

    #[test]
    fn test_parse_sequence() {
        let mut parser = Parser::new("ab");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::sequence(vec![
            Ast::Literal('a'),
            Ast::Literal('b'),
        ]));
    }

    #[test]
    fn test_parse_choice() {
        let mut parser = Parser::new("a|b");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::Choice(vec![
            Ast::Literal('a'),
            Ast::Literal('b'),
        ]));
    }

    #[test]
    fn test_parse_star() {
        let mut parser = Parser::new("a*");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::zero_or_more(Ast::literal('a')));
    }

    #[test]
    fn test_parse_plus() {
        let mut parser = Parser::new("a+");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::one_or_more(Ast::literal('a')));
    }

    #[test]
    fn test_parse_optional() {
        let mut parser = Parser::new("a?");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::zero_or_one(Ast::literal('a')));
    }

    #[test]
    fn test_parse_repeat_exact() {
        let mut parser = Parser::new("a{3}");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::repeat_exact(Ast::literal('a'), 3));
    }

    #[test]
    fn test_parse_repeat_range() {
        let mut parser = Parser::new("a{2,5}");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::repeat_range(Ast::literal('a'), 2, Some(5)));
    }

    #[test]
    fn test_parse_repeat_unbounded() {
        let mut parser = Parser::new("a{3,}");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::repeat_range(Ast::literal('a'), 3, None));
    }

    #[test]
    fn test_parse_group() {
        let mut parser = Parser::new("(a|b)");
        let ast = parser.parse().unwrap();
        assert_eq!(ast, Ast::group(Ast::choice(vec![
            Ast::literal('a'),
            Ast::literal('b'),
        ])));
    }

    #[test]
    fn test_parse_char_class() {
        let mut parser = Parser::new("[a-z]");
        let ast = parser.parse().unwrap();
        if let Ast::Class(class) = ast {
            assert!(class.contains('a'));
            assert!(class.contains('z'));
            assert!(!class.contains('0'));
        } else {
            panic!("Expected CharClass");
        }
    }

    #[test]
    fn test_parse_negated_char_class() {
        let mut parser = Parser::new("[^a-z]");
        let ast = parser.parse().unwrap();
        if let Ast::Class(class) = ast {
            assert!(!class.contains('a'));
            assert!(!class.contains('z'));
            assert!(class.contains('0'));
        } else {
            panic!("Expected CharClass");
        }
    }

    #[test]
    fn test_parse_escape_digit() {
        let mut parser = Parser::new("\\d");
        let ast = parser.parse().unwrap();
        assert!(matches!(ast, Ast::Class(_)));
    }

    #[test]
    fn test_parse_escape_word() {
        let mut parser = Parser::new("\\w");
        let ast = parser.parse().unwrap();
        assert!(matches!(ast, Ast::Class(_)));
    }

    #[test]
    fn test_parse_escape_space() {
        let mut parser = Parser::new("\\s");
        let ast = parser.parse().unwrap();
        assert!(matches!(ast, Ast::Class(_)));
    }

    #[test]
    fn test_parse_complex() {
        let mut parser = Parser::new("[a-zA-Z][a-zA-Z0-9_]*");
        let ast = parser.parse().unwrap();

        // 验证 AST 结构
        assert!(matches!(ast, Ast::Sequence(_)));
    }

    #[test]
    fn test_parse_flags() {
        let mut parser = Parser::new("(?i)a");
        let ast = parser.parse().unwrap();

        // 验证标记
        if let Ast::Sequence(seq) = ast {
            if let Some(Ast::Flags(flags)) = seq.first() {
                assert!(flags.is_case_insensitive());
            } else {
                panic!("Expected Flags");
            }
        } else {
            panic!("Expected Sequence");
        }
    }

    #[test]
    fn test_parse_invalid_repeat_range() {
        let mut parser = Parser::new("a{5,3}");
        let result = parser.parse();

        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidRepeatRange { min, max, .. }) => {
                assert_eq!(min, 5);
                assert_eq!(max, 3);
            }
            _ => panic!("Expected InvalidRepeatRange error"),
        }
    }

    #[test]
    fn test_parse_unclosed_group() {
        let mut parser = Parser::new("(a|b");
        let result = parser.parse();

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_convenience() {
        let ast = parse("a|b").unwrap();
        assert_eq!(ast, Ast::Choice(vec![
            Ast::Literal('a'),
            Ast::Literal('b'),
        ]));
    }
}