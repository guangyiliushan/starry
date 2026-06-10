//! 状态转移模块
//!
//! 该模块提供词法分析自动机（NFA/DFA）的状态转移功能：
//! - [`Transition`] - 状态转移类型定义
//! - [`PredefinedClass`] - 预定义字符类
//! - [`CharClass`] - 自定义字符类
//!
//! # 设计理念
//!
//! 本模块采用"语义化优先，性能其次"的设计原则：
//!
//! ## NFA 阶段
//! - 使用枚举类型的 `Transition`，表达力强，易于理解和操作
//! - 支持复杂的转移类型：Epsilon、单字符、字符范围、预定义类等
//! - 适合构建、合并、优化 NFA 的图操作
//!
//! ## DFA 阶段
//! - 通过 `matches_byte` 方法将语义化的转移转换为字符匹配
//! - 为未来引入字节等价类（Byte Classes）预留接口
//! - 可以构建密集的跳转表实现高性能匹配
//!
//! # 未来优化
//!
//! 当 DFA 构建完成后，可以引入以下优化：
//! - **字节等价类**：将行为相同的字节归为一类，减少跳转表大小
//! - **稀疏跳转表**：为热点路径使用数组表，为冷门路径使用更节省空间的结构
//! - **位掩码缓存**：对于小规模的字符类，使用位掩码加速匹配

use std::ops::RangeInclusive;
use std::fmt;

// ==================== 预定义字符类 ====================

/// 预定义字符类
///
/// 对应正则表达式中的预定义字符类：
/// - `\d` - 数字字符
/// - `\w` - 单词字符（字母、数字、下划线）
/// - `\s` - 空白字符（空格、制表符、换行等）
///
/// # 注意
///
/// 当前实现仅支持 ASCII 范围。未来可以扩展到 Unicode。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredefinedClass {
    /// 数字字符 `[0-9]`
    Digit,
    /// 单词字符 `[a-zA-Z0-9_]`
    Word,
    /// 空白字符（空格、制表符、换行、回车、换页）
    Whitespace,
}

impl PredefinedClass {
    /// 检查字符是否属于该预定义类
    ///
    /// # 参数
    ///
    /// - `c` - 要检查的字符
    ///
    /// # 返回
    ///
    /// 如果字符属于该类，返回 true
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::PredefinedClass;
    /// assert!(PredefinedClass::Digit.contains('5'));
    /// assert!(!PredefinedClass::Digit.contains('a'));
    /// assert!(PredefinedClass::Word.contains('_'));
    /// assert!(PredefinedClass::Whitespace.contains(' '));
    /// ```
    pub fn contains(self, c: char) -> bool {
        // 目前仅支持 ASCII，未来可以扩展到 Unicode
        if !c.is_ascii() {
            return false;
        }

        let byte = c as u8;
        match self {
            PredefinedClass::Digit => byte.is_ascii_digit(),
            PredefinedClass::Word => byte.is_ascii_alphanumeric() || byte == b'_',
            PredefinedClass::Whitespace => byte.is_ascii_whitespace(),
        }
    }

    /// 获取该字符类的字符范围列表
    ///
    /// 将预定义类展开为具体的字符范围，用于构建字符类或优化。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::PredefinedClass;
    /// let digit_ranges = PredefinedClass::Digit.to_ranges();
    /// assert_eq!(digit_ranges, vec!['0'..='9']);
    /// ```
    pub fn to_ranges(self) -> Vec<RangeInclusive<char>> {
        match self {
            PredefinedClass::Digit => vec!['0'..='9'],
            PredefinedClass::Word => vec![
                '0'..='9',
                'A'..='Z',
                'a'..='z',
                '_'..='_',
            ],
            PredefinedClass::Whitespace => vec![
                ' '..=' ',        // 空格
                '\t'..='\t',      // 制表符
                '\n'..='\n',      // 换行符
                '\r'..='\r',      // 回车符
                '\x0C'..='\x0C',  // 换页符
            ],
        }
    }

    /// 获取字符类的名称（用于调试和错误信息）
    pub fn as_str(self) -> &'static str {
        match self {
            PredefinedClass::Digit => "\\d",
            PredefinedClass::Word => "\\w",
            PredefinedClass::Whitespace => "\\s",
        }
    }
}

impl fmt::Display for PredefinedClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ==================== 自定义字符类 ====================

/// 自定义字符类
///
/// 使用字符范围列表表示字符类，支持复杂的字符集合操作。
///
/// # 特性
///
/// - 支持字符范围（如 `a-z`, `0-9`）
/// - 支持离散字符（如 `+-*/`）
/// - 自动规范化（合并重叠和相邻的范围）
/// - 支持集合运算（并、交、补）
///
/// # 性能考虑
///
/// - 范围列表有序存储，可以使用二分查找
/// - 对于小规模集合（ASCII），性能足够
/// - 未来可以添加位掩码缓存优化
///
/// # 示例
///
/// ```
/// # use lex::transition::CharClass;
/// let digit = CharClass::range('0', '9');
/// let alpha = CharClass::range('a', 'z');
/// let alnum = digit.union(&alpha);
///
/// assert!(alnum.contains('5'));
/// assert!(alnum.contains('z'));
/// assert!(!alnum.contains('_'));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharClass {
    /// 规范化的字符范围列表（不重叠、已排序）
    ranges: Vec<RangeInclusive<char>>,
}

impl Default for CharClass {
    fn default() -> Self {
        Self::new()
    }
}

impl CharClass {
    /// 创建空的字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let empty = CharClass::new();
    /// assert!(!empty.contains('a'));
    /// ```
    pub fn new() -> Self {
        Self {
            ranges: Vec::new(),
        }
    }

    /// 从单个字符创建字符类
    ///
    /// # 参数
    ///
    /// - `c` - 字符
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let single = CharClass::single('a');
    /// assert!(single.contains('a'));
    /// assert!(!single.contains('b'));
    /// ```
    pub fn single(c: char) -> Self {
        Self {
            ranges: vec![c..=c],
        }
    }

    /// 从字符范围创建字符类
    ///
    /// # 参数
    ///
    /// - `start` - 起始字符
    /// - `end` - 结束字符
    ///
    /// # Panics
    ///
    /// 如果 `start > end`，会 panic
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let digits = CharClass::range('0', '9');
    /// assert!(digits.contains('5'));
    /// assert!(!digits.contains('a'));
    /// ```
    pub fn range(start: char, end: char) -> Self {
        assert!(start <= end, "字符范围起点不能大于终点");
        Self {
            ranges: vec![start..=end],
        }
    }

    /// 从多个字符创建字符类
    ///
    /// # 参数
    ///
    /// - `chars` - 字符切片
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let operators = CharClass::chars(&['+', '-', '*', '/']);
    /// assert!(operators.contains('+'));
    /// assert!(operators.contains('/'));
    /// ```
    pub fn chars(chars: &[char]) -> Self {
        let mut class = Self::new();
        for &c in chars {
            class = class.union(&Self::single(c));
        }
        class
    }

    /// 从预定义类创建字符类
    ///
    /// # 参数
    ///
    /// - `class` - 预定义字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{CharClass, PredefinedClass};
    /// let digit = CharClass::predefined(PredefinedClass::Digit);
    /// assert!(digit.contains('5'));
    /// ```
    pub fn predefined(class: PredefinedClass) -> Self {
        Self {
            ranges: class.to_ranges(),
        }
    }

    /// 检查字符是否属于该字符类
    ///
    /// 使用二分查找，时间复杂度为 O(log n)。
    ///
    /// # 参数
    ///
    /// - `c` - 要检查的字符
    ///
    /// # 返回
    ///
    /// 如果字符属于该类，返回 true
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let digits = CharClass::range('0', '9');
    /// assert!(digits.contains('5'));
    /// assert!(!digits.contains('a'));
    /// ```
    pub fn contains(&self, c: char) -> bool {
        // 使用二分查找
        self.ranges
            .binary_search_by(|range| {
                if c < *range.start() {
                    std::cmp::Ordering::Greater
                } else if c > *range.end() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    /// 检查是否为空字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// assert!(CharClass::new().is_empty());
    /// assert!(!CharClass::single('a').is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// 获取字符类中的字符数量
    ///
    /// 注意：对于 Unicode 字符，这个值可能很大。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let digits = CharClass::range('0', '9');
    /// assert_eq!(digits.len(), 10);
    /// ```
    pub fn len(&self) -> usize {
        self.ranges
            .iter()
            .map(|range| (*range.end() as u32 - *range.start() as u32 + 1) as usize)
            .sum()
    }

    /// 计算并集
    ///
    /// # 参数
    ///
    /// - `other` - 另一个字符类
    ///
    /// # 返回
    ///
    /// 包含两个字符类中所有字符的新字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let digits = CharClass::range('0', '9');
    /// let letters = CharClass::range('a', 'z');
    /// let alnum = digits.union(&letters);
    /// assert!(alnum.contains('5'));
    /// assert!(alnum.contains('z'));
    /// ```
    pub fn union(&self, other: &Self) -> Self {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < self.ranges.len() || j < other.ranges.len() {
            let range1 = self.ranges.get(i).cloned();
            let range2 = other.ranges.get(j).cloned();

            match (range1, range2) {
                (Some(r1), Some(r2)) => {
                    // 选择起始位置较小的范围
                    if *r1.start() < *r2.start() {
                        i = Self::merge_range(&mut result, r1, i + 1);
                    } else {
                        j = Self::merge_range(&mut result, r2, j + 1);
                    }
                }
                (Some(r1), None) => {
                    i = Self::merge_range(&mut result, r1, i + 1);
                }
                (None, Some(r2)) => {
                    j = Self::merge_range(&mut result, r2, j + 1);
                }
                (None, None) => break,
            }
        }

        Self { ranges: result }
    }

    /// 辅助方法：合并一个范围到结果中
    ///
    /// 返回下一个要处理的索引（如果与下一个范围重叠）
    fn merge_range(
        result: &mut Vec<RangeInclusive<char>>,
        range: RangeInclusive<char>,
        next_idx: usize,
    ) -> usize {
        if let Some(last) = result.last_mut() {
            // 检查是否与最后一个范围重叠或相邻
            if *last.end() >= *range.start() || (*last.end() as u32 + 1 == *range.start() as u32) {
                // 合并范围
                let new_end = *range.end().max(last.end());
                *last = *last.start()..=new_end;
                return next_idx;
            }
        }

        // 添加新范围
        result.push(range);
        next_idx
    }

    /// 计算交集
    ///
    /// # 参数
    ///
    /// - `other` - 另一个字符类
    ///
    /// # 返回
    ///
    /// 包含两个字符类共同字符的新字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let range1 = CharClass::range('a', 'g');
    /// let range2 = CharClass::range('e', 'k');
    /// let intersection = range1.intersect(&range2);
    /// assert!(!intersection.contains('d'));
    /// assert!(intersection.contains('e'));
    /// assert!(intersection.contains('g'));
    /// assert!(!intersection.contains('h'));
    /// ```
    pub fn intersect(&self, other: &Self) -> Self {
        let mut result = Vec::new();

        for r1 in &self.ranges {
            for r2 in &other.ranges {
                let start = *r1.start().max(r2.start());
                let end = *r1.end().min(r2.end());

                if start <= end {
                    result.push(start..=end);
                }
            }
        }

        Self { ranges: result }
    }

    /// 计算补集（相对于 Unicode）
    ///
    /// 注意：对于完整的 Unicode，这个操作会产生大量范围。
    /// 实际使用中，通常只在有限的字符范围内使用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let non_digit = CharClass::range('0', '9').complement();
    /// assert!(!non_digit.contains('5'));
    /// assert!(non_digit.contains('a'));
    /// ```
    pub fn complement(&self) -> Self {
        let mut result = Vec::new();
        let mut last_end = '\0';

        for range in &self.ranges {
            if *range.start() > last_end {
                result.push(last_end..=Self::prev_char(*range.start()));
            }
            last_end = Self::next_char(*range.end());
        }

        // 添加最后一个范围到 Unicode 结束
        if last_end <= char::MAX {
            result.push(last_end..=char::MAX);
        }

        Self { ranges: result }
    }

    /// 获取下一个字符（处理 Unicode 边界）
    fn next_char(c: char) -> char {
        if c == char::MAX {
            c
        } else {
            char::from_u32(c as u32 + 1).unwrap_or(char::MAX)
        }
    }

    /// 获取前一个字符（处理 Unicode 边界）
    fn prev_char(c: char) -> char {
        if c == '\0' {
            c
        } else {
            char::from_u32(c as u32 - 1).unwrap_or('\0')
        }
    }

    /// 迭代字符类中的所有范围
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::CharClass;
    /// let digit = CharClass::range('0', '9');
    /// for range in digit.iter_ranges() {
    ///     println!("{:?}", range);
    /// }
    /// ```
    pub fn iter_ranges(&self) -> impl Iterator<Item = &RangeInclusive<char>> {
        self.ranges.iter()
    }

    /// 获取内部范围列表的引用
    ///
    /// 注意：返回的范围列表是规范化的（不重叠、已排序）
    pub fn ranges(&self) -> &[RangeInclusive<char>] {
        &self.ranges
    }
}

impl fmt::Display for CharClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ranges.is_empty() {
            write!(f, "[]")
        } else if self.ranges.len() == 1 && self.ranges[0].start() == self.ranges[0].end() {
            write!(f, "[{}]", self.ranges[0].start())
        } else {
            write!(f, "[")?;
            for (i, range) in self.ranges.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                if range.start() == range.end() {
                    write!(f, "{}", range.start())?;
                } else {
                    write!(f, "{}-{}", range.start(), range.end())?;
                }
            }
            write!(f, "]")
        }
    }
}

// ==================== 状态转移 ====================

/// 状态转移
///
/// 表示 NFA 中的一个转移边，定义了从一个状态到另一个状态的条件。
///
/// # 转移类型
///
/// - `Epsilon` - 不消耗输入字符的转移（空转移）
/// - `Char` - 匹配特定字符的转移
/// - `Range` - 匹配字符范围内任意字符的转移
/// - `CharClass` - 匹配自定义字符类的转移
/// - `PredefinedClass` - 匹配预定义字符类的转移
///
/// # 设计理念
///
/// ## NFA 阶段
/// - 作为一等公民，支持图操作（构建、合并、优化）
/// - 语义清晰，便于实现 Thompson 构造等算法
/// - 灵活性高，可以轻松扩展新的转移类型
///
/// # DFA 阶段
/// - 通过 `matches_byte` 方法转换为字符匹配
/// - 可以被编译成密集的跳转表
/// - 支持字节等价类优化
///
/// # 未来扩展
///
/// 可以添加以下转移类型：
/// - `Lookahead` - 前瞻断言
/// - `Lookbehind` - 后顾断言
/// - `Anchor` - 锚点（行首、行尾等）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// 不消耗输入字符的转移
    ///
    /// # 示例
    ///
    /// 在正则 `a|b` 中，从起始状态到两个分支都使用 Epsilon 转移。
    Epsilon,

    /// 匹配特定字符的转移
    ///
    /// # 示例
    ///
    /// 在正则 `a` 中，从起始状态到接受状态的转移。
    Char(char),

    /// 匹配字符范围内的任意字符
    ///
    /// # 参数
    ///
    /// - `start` - 范围起点
    /// - `end` - 范围终点
    ///
    /// # 示例
    ///
    /// 在正则 `[a-z]` 中，从起始状态到接受状态的转移。
    Range(char, char),

    /// 匹配自定义字符类的转移
    ///
    /// # 示例
    ///
    /// 在正则 `[a-zA-Z0-9]` 中，从起始状态到接受状态的转移。
    CharClass(CharClass),

    /// 匹配预定义字符类的转移
    ///
    /// # 示例
    ///
    /// 在正则 `\d` 中，从起始状态到接受状态的转移。
    PredefinedClass(PredefinedClass),
}

impl Transition {
    /// 创建 Epsilon 转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// let epsilon = Transition::epsilon();
    /// assert!(epsilon.is_epsilon());
    /// ```
    pub fn epsilon() -> Self {
        Transition::Epsilon
    }

    /// 创建单字符转移
    ///
    /// # 参数
    ///
    /// - `c` - 要匹配的字符
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// let char_trans = Transition::char('a');
    /// assert!(char_trans.is_char());
    /// ```
    pub fn char(c: char) -> Self {
        Transition::Char(c)
    }

    /// 创建字符范围转移
    ///
    /// # 参数
    ///
    /// - `start` - 范围起点
    /// - `end` - 范围终点
    ///
    /// # Panics
    ///
    /// 如果 `start > end`，会 panic
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// let range = Transition::range('a', 'z');
    /// assert!(range.is_range());
    /// ```
    pub fn range(start: char, end: char) -> Self {
        assert!(start <= end, "字符范围起点不能大于终点");
        Transition::Range(start, end)
    }

    /// 创建字符类转移
    ///
    /// # 参数
    ///
    /// - `class` - 字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{Transition, CharClass};
    /// let digit = Transition::char_class(CharClass::range('0', '9'));
    /// assert!(digit.is_char_class());
    /// ```
    pub fn char_class(class: CharClass) -> Self {
        Transition::CharClass(class)
    }

    /// 创建预定义类转移
    ///
    /// # 参数
    ///
    /// - `class` - 预定义字符类
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{Transition, PredefinedClass};
    /// let digit = Transition::predefined_class(PredefinedClass::Digit);
    /// assert!(digit.is_predefined_class());
    /// ```
    pub fn predefined_class(class: PredefinedClass) -> Self {
        Transition::PredefinedClass(class)
    }

    /// 检查是否为 Epsilon 转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert!(Transition::epsilon().is_epsilon());
    /// assert!(!Transition::char('a').is_epsilon());
    /// ```
    pub fn is_epsilon(&self) -> bool {
        matches!(self, Transition::Epsilon)
    }

    /// 检查是否为字符转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert!(Transition::char('a').is_char());
    /// assert!(!Transition::epsilon().is_char());
    /// ```
    pub fn is_char(&self) -> bool {
        matches!(self, Transition::Char(_))
    }

    /// 检查是否为范围转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert!(Transition::range('a', 'z').is_range());
    /// assert!(!Transition::char('a').is_range());
    /// ```
    pub fn is_range(&self) -> bool {
        matches!(self, Transition::Range(_, _))
    }

    /// 检查是否为字符类转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{Transition, CharClass};
    /// assert!(Transition::char_class(CharClass::new()).is_char_class());
    /// assert!(!Transition::char('a').is_char_class());
    /// ```
    pub fn is_char_class(&self) -> bool {
        matches!(self, Transition::CharClass(_))
    }

    /// 检查是否为预定义类转移
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{Transition, PredefinedClass};
    /// assert!(Transition::predefined_class(PredefinedClass::Digit).is_predefined_class());
    /// assert!(!Transition::char('a').is_predefined_class());
    /// ```
    pub fn is_predefined_class(&self) -> bool {
        matches!(self, Transition::PredefinedClass(_))
    }

    /// 检查转移是否消耗输入字符
    ///
    /// Epsilon 转移不消耗字符，其他转移都消耗。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert!(!Transition::epsilon().consumes_input());
    /// assert!(Transition::char('a').consumes_input());
    /// ```
    pub fn consumes_input(&self) -> bool {
        !self.is_epsilon()
    }

    /// 检查字符是否匹配该转移
    ///
    /// 这是 DFA 构建阶段的核心方法，用于将语义化的转移转换为字符匹配。
    ///
    /// # 参数
    ///
    /// - `c` - 要匹配的字符
    ///
    /// # 返回
    ///
    /// 如果字符匹配该转移的条件，返回 true
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::{Transition, CharClass, PredefinedClass};
    /// assert!(Transition::char('a').matches('a'));
    /// assert!(!Transition::char('a').matches('b'));
    ///
    /// assert!(Transition::range('a', 'z').matches('m'));
    /// assert!(!Transition::range('a', 'z').matches('0'));
    ///
    /// assert!(Transition::predefined_class(PredefinedClass::Digit).matches('5'));
    ///
    /// let digit_class = CharClass::range('0', '9');
    /// assert!(Transition::char_class(digit_class).matches('7'));
    /// ```
    pub fn matches(&self, c: char) -> bool {
        match self {
            Transition::Epsilon => false,  // Epsilon 不匹配任何字符
            Transition::Char(ch) => *ch == c,
            Transition::Range(start, end) => c >= *start && c <= *end,
            Transition::CharClass(class) => class.contains(c),
            Transition::PredefinedClass(class) => class.contains(c),
        }
    }

    /// 检查字节是否匹配该转移
    ///
    /// 类似于 `matches`，但接受字节作为输入。
    /// 对于非 ASCII 字符，总是返回 false。
    ///
    /// # 参数
    ///
    /// - `byte` - 要匹配的字节
    ///
    /// # 返回
    ///
    /// 如果字节匹配该转移的条件，返回 true
    ///
    /// # 注意
    ///
    /// 当前仅支持 ASCII。未来可以扩展支持 UTF-8 编码。
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert!(Transition::char('a').matches_byte(b'a'));
    /// assert!(!Transition::char('a').matches_byte(b'b'));
    /// ```
    pub fn matches_byte(&self, byte: u8) -> bool {
        let c = byte as char;
        self.matches(c)
    }

    /// 获取转移的描述字符串（用于调试）
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::transition::Transition;
    /// assert_eq!(Transition::epsilon().description(), "ε");
    /// assert_eq!(Transition::char('a').description(), "'a'");
    /// ```
    pub fn description(&self) -> String {
        match self {
            Transition::Epsilon => "ε".to_string(),
            Transition::Char(c) => format!("'{}'", c),
            Transition::Range(start, end) => format!("'{}'-'{}'", start, end),
            Transition::CharClass(class) => class.to_string(),
            Transition::PredefinedClass(class) => class.to_string(),
        }
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== PredefinedClass 测试 ====================

    #[test]
    fn test_predefined_class_digit() {
        assert!(PredefinedClass::Digit.contains('0'));
        assert!(PredefinedClass::Digit.contains('9'));
        assert!(!PredefinedClass::Digit.contains('a'));
        assert!(!PredefinedClass::Digit.contains(' '));
    }

    #[test]
    fn test_predefined_class_word() {
        assert!(PredefinedClass::Word.contains('a'));
        assert!(PredefinedClass::Word.contains('Z'));
        assert!(PredefinedClass::Word.contains('0'));
        assert!(PredefinedClass::Word.contains('9'));
        assert!(PredefinedClass::Word.contains('_'));
        assert!(!PredefinedClass::Word.contains(' '));
        assert!(!PredefinedClass::Word.contains('-'));
    }

    #[test]
    fn test_predefined_class_whitespace() {
        assert!(PredefinedClass::Whitespace.contains(' '));
        assert!(PredefinedClass::Whitespace.contains('\t'));
        assert!(PredefinedClass::Whitespace.contains('\n'));
        assert!(PredefinedClass::Whitespace.contains('\r'));
        assert!(!PredefinedClass::Whitespace.contains('a'));
        assert!(!PredefinedClass::Whitespace.contains('0'));
    }

    #[test]
    fn test_predefined_class_to_ranges() {
        let digit_ranges = PredefinedClass::Digit.to_ranges();
        assert_eq!(digit_ranges, vec!['0'..='9']);

        let word_ranges = PredefinedClass::Word.to_ranges();
        assert_eq!(
            word_ranges,
            vec!['0'..='9', 'A'..='Z', 'a'..='z', '_'..='_']
        );
    }

    #[test]
    fn test_predefined_class_display() {
        assert_eq!(PredefinedClass::Digit.to_string(), "\\d");
        assert_eq!(PredefinedClass::Word.to_string(), "\\w");
        assert_eq!(PredefinedClass::Whitespace.to_string(), "\\s");
    }

    // ==================== CharClass 测试 ====================

    #[test]
    fn test_char_class_new() {
        let empty = CharClass::new();
        assert!(empty.is_empty());
        assert!(!empty.contains('a'));
    }

    #[test]
    fn test_char_class_single() {
        let single = CharClass::single('a');
        assert!(!single.is_empty());
        assert!(single.contains('a'));
        assert!(!single.contains('b'));
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn test_char_class_range() {
        let range = CharClass::range('a', 'z');
        assert!(!range.is_empty());
        assert!(range.contains('a'));
        assert!(range.contains('m'));
        assert!(range.contains('z'));
        assert!(!range.contains('0'));
        assert_eq!(range.len(), 26);
    }

    #[test]
    fn test_char_class_chars() {
        let operators = CharClass::chars(&['+', '-', '*', '/']);
        assert!(operators.contains('+'));
        assert!(operators.contains('-'));
        assert!(operators.contains('*'));
        assert!(operators.contains('/'));
        assert!(!operators.contains('%'));
    }

    #[test]
    fn test_char_class_predefined() {
        let digit = CharClass::predefined(PredefinedClass::Digit);
        assert!(digit.contains('0'));
        assert!(digit.contains('9'));
        assert!(!digit.contains('a'));
    }

    #[test]
    fn test_char_class_union() {
        let digits = CharClass::range('0', '9');
        let letters = CharClass::range('a', 'z');
        let alnum = digits.union(&letters);

        assert!(alnum.contains('5'));
        assert!(alnum.contains('z'));
        assert!(!alnum.contains('_'));
    }

    #[test]
    fn test_char_class_union_overlap() {
        let range1 = CharClass::range('a', 'g');
        let range2 = CharClass::range('e', 'k');
        let union = range1.union(&range2);

        // 应该合并为一个范围
        assert_eq!(union.ranges(), &['a'..='k']);
        assert!(union.contains('a'));
        assert!(union.contains('g'));
        assert!(union.contains('k'));
        assert!(!union.contains('l'));
    }

    #[test]
    fn test_char_class_intersect() {
        let range1 = CharClass::range('a', 'g');
        let range2 = CharClass::range('e', 'k');
        let intersection = range1.intersect(&range2);

        assert!(!intersection.contains('d'));
        assert!(intersection.contains('e'));
        assert!(intersection.contains('g'));
        assert!(!intersection.contains('h'));
        assert_eq!(intersection.ranges(), &['e'..='g']);
    }

    #[test]
    fn test_char_class_intersect_empty() {
        let range1 = CharClass::range('a', 'c');
        let range2 = CharClass::range('d', 'f');
        let intersection = range1.intersect(&range2);

        assert!(intersection.is_empty());
    }

    #[test]
    fn test_char_class_complement() {
        let digit = CharClass::range('0', '9');
        let non_digit = digit.complement();

        assert!(!non_digit.contains('5'));
        assert!(non_digit.contains('a'));
        assert!(non_digit.contains('z'));
        assert!(non_digit.contains(' '));
    }

    #[test]
    fn test_char_class_display() {
        assert_eq!(CharClass::new().to_string(), "[]");
        assert_eq!(CharClass::single('a').to_string(), "[a]");
        assert_eq!(CharClass::range('a', 'z').to_string(), "[a-z]");
    }

    // ==================== Transition 测试 ====================

    #[test]
    fn test_transition_epsilon() {
        let epsilon = Transition::epsilon();
        assert!(epsilon.is_epsilon());
        assert!(!epsilon.is_char());
        assert!(!epsilon.consumes_input());
        assert!(!epsilon.matches('a'));
        assert!(!epsilon.matches_byte(b'a'));
        assert_eq!(epsilon.description(), "ε");
    }

    #[test]
    fn test_transition_char() {
        let char_trans = Transition::char('a');
        assert!(!char_trans.is_epsilon());
        assert!(char_trans.is_char());
        assert!(char_trans.consumes_input());
        assert!(char_trans.matches('a'));
        assert!(!char_trans.matches('b'));
        assert!(char_trans.matches_byte(b'a'));
        assert!(!char_trans.matches_byte(b'b'));
        assert_eq!(char_trans.description(), "'a'");
    }

    #[test]
    fn test_transition_range() {
        let range = Transition::range('a', 'z');
        assert!(!range.is_epsilon());
        assert!(range.is_range());
        assert!(range.consumes_input());
        assert!(range.matches('a'));
        assert!(range.matches('m'));
        assert!(range.matches('z'));
        assert!(!range.matches('0'));
        assert_eq!(range.description(), "'a'-'z'");
    }

    #[test]
    fn test_transition_char_class() {
        let digit_class = CharClass::range('0', '9');
        let char_class_trans = Transition::char_class(digit_class.clone());
        assert!(!char_class_trans.is_epsilon());
        assert!(char_class_trans.is_char_class());
        assert!(char_class_trans.consumes_input());
        assert!(char_class_trans.matches('0'));
        assert!(char_class_trans.matches('9'));
        assert!(!char_class_trans.matches('a'));
        assert_eq!(char_class_trans.description(), "[0-9]");
    }

    #[test]
    fn test_transition_predefined_class() {
        let digit_trans = Transition::predefined_class(PredefinedClass::Digit);
        assert!(!digit_trans.is_epsilon());
        assert!(digit_trans.is_predefined_class());
        assert!(digit_trans.consumes_input());
        assert!(digit_trans.matches('0'));
        assert!(digit_trans.matches('9'));
        assert!(!digit_trans.matches('a'));
        assert_eq!(digit_trans.description(), "\\d");
    }

    #[test]
    #[should_panic(expected = "字符范围起点不能大于终点")]
    fn test_transition_range_invalid() {
        let _ = Transition::range('z', 'a');
    }

    #[test]
    fn test_transition_display() {
        assert_eq!(Transition::epsilon().to_string(), "ε");
        assert_eq!(Transition::char('a').to_string(), "'a'");
        assert_eq!(Transition::range('0', '9').to_string(), "'0'-'9'");
    }
}