//! NFA 转移边模块
//!
//! 将纯条件的 `Transition` 与目标 `StateId` 绑定，
//! 使 `Transition` 可在 NFA 和 DFA 中复用。
//!
//! # 设计理念
//!
//! `Transition` 只负责描述**匹配什么**（条件），
//! `Edge` 则关心**转移到哪里**（目标）。
//!
//! 这种分离使 `Transition` 可以在解析、NFA 构建和 DFA 匹配中复用，
//! 而 `Edge` 是 NFA 专用的转移表示。

use std::fmt;

use crate::state::StateId;
use crate::transition::Transition;

/// NFA 转移边：匹配条件 + 目标状态
///
/// 将 `Transition`（纯匹配条件）与 `StateId`（目标）绑定在一起。
///
/// # 示例
///
/// ```
/// # use lex::nfa::Edge;
/// # use lex::Transition;
/// let edge = Edge::new(Transition::char('a'), 1);
/// assert_eq!(edge.target, 1);
/// assert!(edge.trans.is_char());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// 匹配条件（不携带目标，可在 NFA/DFA 中复用）
    pub trans: Transition,
    /// 目标状态 ID
    pub target: StateId,
}

impl Edge {
    /// 创建新的转移边
    ///
    /// # 参数
    ///
    /// - `trans` - 匹配条件
    /// - `target` - 目标状态 ID
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::nfa::Edge;
    /// # use lex::Transition;
    /// let edge = Edge::new(Transition::epsilon(), 2);
    /// assert!(edge.is_epsilon());
    /// assert_eq!(edge.target, 2);
    /// ```
    pub fn new(trans: Transition, target: StateId) -> Self {
        Self { trans, target }
    }

    /// 创建 epsilon 转移边
    ///
    /// # 参数
    ///
    /// - `target` - 目标状态 ID
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::nfa::Edge;
    /// let edge = Edge::epsilon(3);
    /// assert!(edge.is_epsilon());
    /// ```
    pub fn epsilon(target: StateId) -> Self {
        Self {
            trans: Transition::epsilon(),
            target,
        }
    }

    /// 创建字符转移边
    ///
    /// # 参数
    ///
    /// - `c` - 匹配的字符
    /// - `target` - 目标状态 ID
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::nfa::Edge;
    /// let edge = Edge::char('a', 1);
    /// assert!(edge.matches('a'));
    /// ```
    pub fn char(c: char, target: StateId) -> Self {
        Self {
            trans: Transition::char(c),
            target,
        }
    }

    /// 检查是否为 epsilon 转移
    pub fn is_epsilon(&self) -> bool {
        self.trans.is_epsilon()
    }

    /// 检查是否消耗输入
    pub fn consumes_input(&self) -> bool {
        self.trans.consumes_input()
    }

    /// 检查字符是否匹配该转移
    ///
    /// 委托给 `Transition::matches()`。
    pub fn matches(&self, c: char) -> bool {
        self.trans.matches(c)
    }

    /// 检查字节是否匹配该转移
    pub fn matches_byte(&self, byte: u8) -> bool {
        self.trans.matches_byte(byte)
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} --> S{}", self.trans.description(), self.target)
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_new() {
        let edge = Edge::new(Transition::char('a'), 1);
        assert_eq!(edge.target, 1);
        assert!(edge.trans.is_char());
    }

    #[test]
    fn test_edge_epsilon() {
        let edge = Edge::epsilon(2);
        assert!(edge.is_epsilon());
        assert_eq!(edge.target, 2);
    }

    #[test]
    fn test_edge_matches() {
        let edge = Edge::char('a', 1);
        assert!(edge.matches('a'));
        assert!(!edge.matches('b'));
    }

    #[test]
    fn test_edge_display() {
        let edge = Edge::char('a', 1);
        assert_eq!(edge.to_string(), "'a' --> S1");
    }
}