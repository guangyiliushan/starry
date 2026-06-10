//! 状态管理模块
//!
//! 该模块提供词法分析自动机（NFA/DFA）的状态管理功能：
//! - [`StateId`] - 状态标识符类型别名
//! - [`State`] - 自动机状态定义
//! - [`StateSet`] - 状态集合类型别名
//! - [`StateGenerator`] - 状态 ID 生成器

use std::collections::BTreeSet;
use std::fmt;

use ast::token::TokenKind;

// ==================== 状态标识符 ====================

/// 状态标识符
///
/// 使用简单的 `usize` 作为状态标识符，提供以下优势：
/// - 零成本抽象，无额外内存开销
/// - 可以直接作为数组索引
/// - 简单直观，易于调试
///
/// 未来如果需要区分不同来源的状态（如组合多个 NFA），
/// 可以轻松替换为结构体类型，而不影响使用方式。
pub type StateId = usize;

// ==================== 状态集合 ====================

/// 状态集合
///
/// 使用 `BTreeSet<StateId>` 作为状态集合，提供以下优势：
/// - 自动排序，保证遍历顺序的确定性
/// - 可以直接比较两个集合是否相等
/// - 可以直接作为 HashMap 的键（用于 DFA 状态去重）
/// - 支持高效的集合操作（并、交、差）
/// - 调试友好，输出有序
///
/// 对于词法分析器的状态集合（通常几十到几百个状态），
/// BTreeSet 的性能足够好。如果未来需要优化，可以：
/// - 替换为 `HashSet<StateId>`（更快但无序）
/// - 替换为 `BitSet`（更节省内存，但需要预知最大状态数）
/// - 定义 `StateSet` trait 来抽象集合行为
pub type StateSet = BTreeSet<StateId>;

// ==================== 状态定义 ====================

/// 自动机状态
///
/// 表示 NFA 或 DFA 中的一个状态，包含状态 ID 和关联的 Token 类型。
///
/// # 字段说明
///
/// - `id` - 状态的唯一标识符
/// - `token_kind` - 如果是接受状态，存储对应的 Token 类型；否则为 None
///
/// # 设计考量
///
/// 采用简单直接的结构体设计，而不是复杂的状态管理系统：
/// - 信息集中，访问 Token 类型只需一次查找
/// - 直观易懂，符合自动机的语义模型
/// - 内存占用可接受（大部分状态是接受状态，TokenKind 是 Copy 类型）
///
/// # 未来扩展点
///
/// 如果需要添加更多状态属性，可以轻松扩展：
///
/// ```ignore
/// pub struct State {
///     pub id: StateId,
///     pub token_kind: Option<TokenKind>,
///     // 未来可能的扩展：
///     // pub fallback_state: Option<StateId>,  // 回退状态（用于错误恢复）
///     // pub name: Option<String>,              // 调试用的状态名称
///     // pub is_dead: bool,                     // 死状态标记（用于 DFA 最小化）
/// }
/// ```
///
/// # 性能优化可能性
///
/// 如果未来遇到性能瓶颈，可以在不改变使用方式的前提下进行内部优化：
/// - 使用位掩码打包 `is_accepting` 和 `token_kind`
/// - 将状态信息存储在分离的数组中（SoA 布局）
/// - 使用 `#[repr(C)]` 或 `#[repr(packed)]` 控制内存布局
///
/// 这些优化对外部消费者是完全透明的，依然可以通过 `state.is_accepting()` 等方法访问信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// 状态 ID
    pub id: StateId,
    /// Token 类型（None 表示非接受状态）
    pub token_kind: Option<TokenKind>,
}

impl State {
    /// 创建新的非接受状态
    ///
    /// # 参数
    ///
    /// - `id` - 状态 ID
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let state = State::new(0);
    /// assert_eq!(state.id, 0);
    /// assert!(!state.is_accepting());
    /// ```
    pub fn new(id: StateId) -> Self {
        Self {
            id,
            token_kind: None,
        }
    }

    /// 创建接受状态
    ///
    /// # 参数
    ///
    /// - `id` - 状态 ID
    /// - `token_kind` - 对应的 Token 类型
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let state = State::accepting(1, TokenKind::Identifier);
    /// assert_eq!(state.id, 1);
    /// assert!(state.is_accepting());
    /// assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));
    /// ```
    pub fn accepting(id: StateId, token_kind: TokenKind) -> Self {
        Self {
            id,
            token_kind: Some(token_kind),
        }
    }

    /// 检查是否为接受状态
    ///
    /// 接受状态表示自动机在此状态可以成功匹配，并产生对应的 Token。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let normal = State::new(0);
    /// assert!(!normal.is_accepting());
    ///
    /// let accepting = State::accepting(1, TokenKind::Identifier);
    /// assert!(accepting.is_accepting());
    /// ```
    pub fn is_accepting(&self) -> bool {
        self.token_kind.is_some()
    }

    /// 获取 Token 类型
    ///
    /// 如果是接受状态，返回对应的 Token 类型；否则返回 None。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let state = State::accepting(0, TokenKind::Integer);
    /// assert_eq!(state.token_kind(), Some(&TokenKind::Integer));
    /// ```
    pub fn token_kind(&self) -> Option<&TokenKind> {
        self.token_kind.as_ref()
    }

    /// 设置 Token 类型
    ///
    /// 将状态转换为接受状态，或更新接受状态的 Token 类型。
    ///
    /// # 参数
    ///
    /// - `token_kind` - 要设置的 Token 类型（None 表示非接受状态）
    pub fn set_token_kind(&mut self, token_kind: Option<TokenKind>) {
        self.token_kind = token_kind;
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(kind) = &self.token_kind {
            write!(f, "S{}(accepting: {})", self.id, kind)
        } else {
            write!(f, "S{}", self.id)
        }
    }
}

// ==================== 状态生成器 ====================

/// 状态 ID 生成器
///
/// 用于自动生成唯一的状态 ID。采用简单的递增计数器设计。
///
/// # 设计考量
///
/// 使用全局计数器而不是 ID 池的原因：
/// - 编译器是短期运行的，不需要 ID 重用
/// - 简单高效，保证单调递增
/// - 避免了 ID 池的额外内存开销
///
/// # 多线程考虑
///
/// 当前实现不是线程安全的。如果未来需要并发构建自动机，
/// 可以：
/// - 使用 `AtomicUsize` 替换 `usize`
/// - 每个线程使用独立的生成器，在组合时添加偏移量
/// - 使用 `Arc<Mutex<StateGenerator>>` 加锁
pub struct StateGenerator {
    /// 下一个可用的状态 ID
    next_id: StateId,
}

impl Default for StateGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl StateGenerator {
    /// 创建新的状态生成器，从 0 开始
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut gen = StateGenerator::new();
    /// assert_eq!(gen.next(), 0);
    /// assert_eq!(gen.next(), 1);
    /// ```
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    /// 创建新的状态生成器，从指定 ID 开始
    ///
    /// 这个方法在组合多个 NFA 时很有用，可以确保不同来源的 NFA 使用不同的 ID 范围。
    ///
    /// # 参数
    ///
    /// - `start` - 起始状态 ID
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut gen = StateGenerator::with_start(100);
    /// assert_eq!(gen.next(), 100);
    /// assert_eq!(gen.next(), 101);
    /// ```
    pub fn with_start(start: StateId) -> Self {
        Self { next_id: start }
    }

    /// 生成下一个状态 ID
    ///
    /// # 返回
    ///
    /// 新的状态 ID
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut gen = StateGenerator::new();
    /// let id1 = gen.next();
    /// let id2 = gen.next();
    /// assert!(id1 < id2);
    /// ```
    pub fn next(&mut self) -> StateId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 查看下一个将要生成的状态 ID（不生成）
    ///
    /// # 返回
    ///
    /// 下一个状态 ID
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let gen = StateGenerator::new();
    /// assert_eq!(gen.peek(), 0);
    /// assert_eq!(gen.peek(), 0); // 不改变状态
    /// ```
    pub fn peek(&self) -> StateId {
        self.next_id
    }

    /// 批量生成多个状态 ID
    ///
    /// # 参数
    ///
    /// - `count` - 要生成的状态数量
    ///
    /// # 返回
    ///
    /// 状态 ID 的向量
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut gen = StateGenerator::new();
    /// let ids = gen.next_batch(3);
    /// assert_eq!(ids, vec![0, 1, 2]);
    /// ```
    pub fn next_batch(&mut self, count: usize) -> Vec<StateId> {
        let start = self.next_id;
        self.next_id += count;
        (start..self.next_id).collect()
    }

    /// 重置生成器到指定状态
    ///
    /// # 警告
    ///
    /// 这个方法可能会导致 ID 重复，请谨慎使用。
    ///
    /// # 参数
    ///
    /// - `id` - 重置到的状态 ID
    pub fn reset(&mut self, id: StateId) {
        self.next_id = id;
    }
}

// ==================== 辅助函数 ====================

/// 创建包含单个状态的状态集合
///
/// # 参数
///
/// - `id` - 状态 ID
///
/// # 返回
///
/// 包含该状态的状态集合
///
/// # 示例
///
/// ```ignore
/// let set = state_set!(0);
/// assert!(set.contains(&0));
/// assert!(!set.contains(&1));
/// ```
#[macro_export]
macro_rules! state_set {
    ($($id:expr),+ $(,)?) => {{
        let mut set = $crate::state::StateSet::new();
        $(
            set.insert($id);
        )+
        set
    }};
}

/// 创建空的状态集合
///
/// # 返回
///
/// 空的状态集合
///
/// # 示例
///
/// ```ignore
/// let set = empty_state_set!();
/// assert!(set.is_empty());
/// ```
#[macro_export]
macro_rules! empty_state_set {
    () => {{
        $crate::state::StateSet::new()
    }};
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_new() {
        let state = State::new(0);
        assert_eq!(state.id, 0);
        assert!(!state.is_accepting());
        assert!(state.token_kind.is_none());
    }

    #[test]
    fn test_state_accepting() {
        let state = State::accepting(1, TokenKind::Identifier);
        assert_eq!(state.id, 1);
        assert!(state.is_accepting());
        assert_eq!(state.token_kind(), Some(&TokenKind::Identifier));
    }

    #[test]
    fn test_state_set_token_kind() {
        let mut state = State::new(0);
        assert!(!state.is_accepting());

        state.set_token_kind(Some(TokenKind::integer()));
        assert!(state.is_accepting());
        assert_eq!(state.token_kind(), Some(&TokenKind::integer()));

        state.set_token_kind(None);
        assert!(!state.is_accepting());
    }

    #[test]
    fn test_state_display() {
        let normal = State::new(0);
        assert_eq!(normal.to_string(), "S0");

        let accepting = State::accepting(1, TokenKind::Identifier);
        assert_eq!(accepting.to_string(), "S1(accepting: identifier)");
    }

    #[test]
    fn test_state_generator_new() {
        let mut generator = StateGenerator::new();
        assert_eq!(generator.peek(), 0);
        assert_eq!(generator.next(), 0);
        assert_eq!(generator.peek(), 1);
        assert_eq!(generator.next(), 1);
    }

    #[test]
    fn test_state_generator_with_start() {
        let mut generator = StateGenerator::with_start(100);
        assert_eq!(generator.peek(), 100);
        assert_eq!(generator.next(), 100);
        assert_eq!(generator.next(), 101);
    }

    #[test]
    fn test_state_generator_next_batch() {
        let mut generator = StateGenerator::new();
        let ids = generator.next_batch(3);
        assert_eq!(ids, vec![0, 1, 2]);
        assert_eq!(generator.peek(), 3);
    }

    #[test]
    fn test_state_generator_reset() {
        let mut generator = StateGenerator::new();
        assert_eq!(generator.next(), 0);
        assert_eq!(generator.next(), 1);

        generator.reset(10);
        assert_eq!(generator.next(), 10);
        assert_eq!(generator.next(), 11);
    }

    #[test]
    fn test_state_set_macro() {
        let set = state_set!(0, 2, 1);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&0));
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(!set.contains(&3));
    }

    #[test]
    fn test_empty_state_set_macro() {
        let set = empty_state_set!();
        assert!(set.is_empty());
    }

    #[test]
    fn test_state_set_operations() {
        let set1 = state_set!(0, 1, 2);
        let set2 = state_set!(2, 3, 4);

        // 并集
        let union: StateSet = set1.union(&set2).cloned().collect();
        assert_eq!(union.len(), 5);

        // 交集
        let intersection: StateSet = set1.intersection(&set2).cloned().collect();
        assert_eq!(intersection.len(), 1);
        assert!(intersection.contains(&2));

        // 差集
        let difference: StateSet = set1.difference(&set2).cloned().collect();
        assert_eq!(difference.len(), 2);
        assert!(difference.contains(&0));
        assert!(difference.contains(&1));
    }
}