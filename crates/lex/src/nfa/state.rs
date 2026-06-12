//! NFA 状态定义模块
//!
//! 定义 NFA 中的单个状态，包含 epsilon 转移和非 epsilon 转移的分离存储。

use crate::state::StateId;
use super::edge::Edge;

/// NFA 状态
///
/// # 字段说明
///
/// - `id` - 状态唯一标识符
/// - `epsilons` - epsilon 转移的目标（单独存储，加速闭包计算）
/// - `edges` - 非 epsilon 转移边（携带匹配条件和目标状态）
///
/// # 设计理念
///
/// 采用**方案 B 变体**：将 epsilon 转移与非 epsilon 转移分离存储。
/// - 闭包计算时只需访问 `epsilons`，O(1) 直接遍历
/// - 字符匹配时遍历 `edges` 列表
///
/// # 示例
///
/// ```
/// # use lex::nfa::NFAState;
/// let state = NFAState::new(0);
/// assert_eq!(state.id, 0);
/// assert!(state.epsilons.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct NFAState {
    /// 状态 ID（与 states 数组索引一致）
    pub id: StateId,
    /// Epsilon 转移的目标状态列表
    pub epsilons: Vec<StateId>,
    /// 非 epsilon 转移边列表
    pub edges: Vec<Edge>,
}

impl NFAState {
    /// 创建新的空状态
    ///
    /// # 参数
    ///
    /// - `id` - 状态 ID
    pub fn new(id: StateId) -> Self {
        Self {
            id,
            epsilons: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// 检查是否有 epsilon 转移
    pub fn has_epsilon(&self) -> bool {
        !self.epsilons.is_empty()
    }

    /// 检查是否有非 epsilon 转移
    pub fn has_edges(&self) -> bool {
        !self.edges.is_empty()
    }

    /// 获取所有 epsilon 转移的目标状态
    pub fn epsilon_targets(&self) -> &[StateId] {
        &self.epsilons
    }

    /// 获取所有非 epsilon 转移的 Edge
    pub fn all_edges(&self) -> &[Edge] {
        &self.edges
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nfa_state_new() {
        let state = NFAState::new(0);
        assert_eq!(state.id, 0);
        assert!(state.epsilons.is_empty());
        assert!(state.edges.is_empty());
    }
}