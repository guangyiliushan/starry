use crate::attribute::env::ScopeId;
use crate::ty::Ty;

/// 遍次标识
///
/// 对应多遍语义分析流程中的四个阶段。
/// 每个遍次有独立的语义职责，通过 `AttrContext::pass` 字段
/// 让求值器感知当前所处的遍次，从而执行不同的语义动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// 预热遍：纯计数 DFS，统计节点/作用域/符号数量
    Warmup,
    /// 名称解析遍：L属性，自顶向下，注册所有声明的名字（含占位符）
    NameResolution,
    /// 类型推断与检查遍：混合策略，解析类型、检查语义、常量折叠
    TypeChecking,
    /// 中间代码生成遍：S属性，自底向上，生成 TAC
    IrGeneration,
}

/// 预热统计信息
///
/// 由预热遍（Pass::Warmup）产出，供后续遍历预分配内存使用。
/// 这是工程优化手段，避免频繁的动态内存分配。
#[derive(Debug, Clone)]
pub struct WarmupStats {
    /// AST 节点总数
    pub node_count: usize,
    /// 最大作用域嵌套深度
    pub max_scope_depth: usize,
    /// 估计的符号总数（基于 Ident 节点数）
    pub estimated_symbol_count: usize,
}

/// 继承属性上下文（L属性）
///
/// 对应龙书 §5.2.2 L属性文法：继承属性仅依赖于父节点和左兄弟节点的属性。
/// 在 AST 遍历的**前序阶段**（进入节点时），通过函数参数自顶向下传递。
///
/// `AttrContext` 是一个纯值类型，每次进入子节点时通过 `with_xxx()` 建造者方法
/// 创建新的副本。这种不可变传递风格完美契合虎书的函数式环境思想：
/// 父节点的上下文不会被修改，子节点获得自己的上下文视图。
#[derive(Debug, Clone)]
pub struct AttrContext {
    /// 当前作用域 ID（指向 `Env` 中某个 `Scope`）
    pub scope: ScopeId,

    /// 期望的返回值类型（用于 return 语句的类型检查）
    pub expected_return: Option<Ty>,

    /// 是否处于循环内部（用于 break/continue 合法性检查）
    pub in_loop: bool,

    /// 当前所在的函数作用域 ID（用于 return 语句定位）
    pub current_fn: Option<ScopeId>,

    /// 当前所处的遍次
    pub pass: Pass,

    /// 预热遍产出的统计信息（后续遍历可参考）
    pub warmup_stats: Option<WarmupStats>,
}

impl AttrContext {
    /// 创建根上下文（全局作用域）
    pub fn root(scope: ScopeId) -> Self {
        Self {
            scope,
            expected_return: None,
            in_loop: false,
            current_fn: None,
            pass: Pass::Warmup,
            warmup_stats: None,
        }
    }

    /// 返回一个 scope 被替换为新值的上下文副本
    pub fn with_scope(self, scope: ScopeId) -> Self {
        Self { scope, ..self }
    }

    /// 返回一个 in_loop 被替换为新值的上下文副本
    pub fn with_in_loop(self, in_loop: bool) -> Self {
        Self { in_loop, ..self }
    }

    /// 返回一个 expected_return 被替换为新值的上下文副本
    pub fn with_expected_return(self, expected_return: Option<Ty>) -> Self {
        Self {
            expected_return,
            ..self
        }
    }

    /// 返回一个 current_fn 被替换为新值的上下文副本
    pub fn with_current_fn(self, current_fn: Option<ScopeId>) -> Self {
        Self { current_fn, ..self }
    }

    /// 返回一个 pass 被替换为新值的上下文副本
    pub fn with_pass(self, pass: Pass) -> Self {
        Self { pass, ..self }
    }

    /// 返回一个 warmup_stats 被替换为新值的上下文副本
    pub fn with_warmup_stats(self, stats: WarmupStats) -> Self {
        Self {
            warmup_stats: Some(stats),
            ..self
        }
    }
}
