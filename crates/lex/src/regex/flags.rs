//! 控制标记模块
//!
//! 该模块提供正则表达式的控制标记功能，如忽略大小写、多行模式等。
//!
//! # 核心类型
//!
//! - [`FlagState`] - 标记状态管理
//!
//! # 示例
//!
//! ```
//! use lex::regex::flags::FlagState;
//! use lex::regex::Flags;
//!
//! let mut state = FlagState::default();
//! let flags = Flags::case_insensitive();
//! state.apply(&flags);
//!
//! assert!(state.is_case_insensitive());
//! ```

use crate::regex::ast::Flags;

// ==================== 标记状态 ====================

/// 标记状态管理器
///
/// 跟踪当前生效的控制标记，支持标记的应用和恢复。
///
/// # 设计特点
///
/// - **栈式管理**：支持嵌套的标记作用域
/// - **组合标记**：支持多个标记的组合
/// - **状态查询**：提供便捷的状态查询方法
pub struct FlagState {
    /// 标记栈，用于支持嵌套的标记作用域
    stack: Vec<Flags>,
    /// 当前生效的标记
    current: Flags,
}

impl Default for FlagState {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            current: Flags::new(),
        }
    }
}

impl FlagState {
    /// 创建新的标记状态
    pub fn new() -> Self {
        Self::default()
    }

    /// 应用新的标记
    ///
    /// 将当前标记压入栈中，然后应用新标记。
    ///
    /// # 参数
    ///
    /// - `flags` - 要应用的标记
    pub fn apply(&mut self, flags: &Flags) {
        self.stack.push(self.current);
        self.current = self.current.merge(flags);
    }

    /// 恢复之前的标记状态
    ///
    /// 从栈中弹出之前的标记状态。
    pub fn pop(&mut self) {
        if let Some(flags) = self.stack.pop() {
            self.current = flags;
        }
    }

    /// 检查是否忽略大小写
    pub fn is_case_insensitive(&self) -> bool {
        self.current.is_case_insensitive()
    }

    /// 检查是否为多行模式
    pub fn is_multiline(&self) -> bool {
        self.current.is_multiline()
    }

    /// 检查点号是否匹配换行符
    pub fn is_dot_matches_newline(&self) -> bool {
        self.current.is_dot_matches_newline()
    }

    /// 获取当前标记
    pub fn current(&self) -> &Flags {
        &self.current
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_state_default() {
        let state = FlagState::new();
        assert!(!state.is_case_insensitive());
        assert!(!state.is_multiline());
    }

    #[test]
    fn test_flag_state_apply() {
        let mut state = FlagState::new();
        
        state.apply(&Flags::case_insensitive());
        assert!(state.is_case_insensitive());
        
        state.pop();
        assert!(!state.is_case_insensitive());
    }

    #[test]
    fn test_flag_state_nested() {
        let mut state = FlagState::new();
        
        state.apply(&Flags::case_insensitive());
        assert!(state.is_case_insensitive());
        
        state.apply(&Flags::multiline());
        assert!(state.is_case_insensitive());
        assert!(state.is_multiline());
        
        state.pop();
        assert!(state.is_case_insensitive());
        assert!(!state.is_multiline());
        
        state.pop();
        assert!(!state.is_case_insensitive());
        assert!(!state.is_multiline());
    }
}