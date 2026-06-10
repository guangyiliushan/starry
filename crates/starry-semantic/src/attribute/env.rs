use std::collections::HashMap;

use crate::attribute::context::WarmupStats;
use crate::error::SemanticError;
use crate::symbol::Symbol;

/// 作用域 ID（数值索引）
///
/// 使用 `usize` 索引而非引用，避免与 `AttrContext` 产生生命周期纠缠。
/// 所有 `Scope` 实例存储在 `Env::scopes` 向量中，通过 `ScopeId` 访问。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub usize);

/// 单个作用域（符号表的一层）
///
/// 对应虎书中的环境帧（environment frame）：每个 Block、函数体等
/// 作用域边界对应一个 `Scope`，通过 `parent` 字段链接到外层作用域。
#[derive(Debug, Clone)]
pub struct Scope {
    /// 父作用域 ID（形成不可变链式结构）
    pub parent: Option<ScopeId>,

    /// 当前作用域内的符号表
    pub symbols: HashMap<String, Symbol>,
}

impl Scope {
    /// 创建新的空作用域
    pub fn new(parent: Option<ScopeId>) -> Self {
        Self {
            parent,
            symbols: HashMap::new(),
        }
    }

    /// 创建具有预分配容量的空作用域
    pub fn with_capacity(parent: Option<ScopeId>, capacity: usize) -> Self {
        Self {
            parent,
            symbols: HashMap::with_capacity(capacity),
        }
    }
}

/// 环境（作用域表的拥有者）
///
/// 对应虎书中的符号表管理器：持有所有作用域的存储，
/// 并跟踪当前活动作用域。
#[derive(Debug)]
pub struct Env {
    /// 所有作用域的存储（按 ScopeId 索引）
    scopes: Vec<Scope>,

    /// 当前活动作用域的 ID
    current: ScopeId,

    /// 预热统计（用于预分配）
    warmup_stats: Option<WarmupStats>,
}

impl Env {
    /// 创建包含单个全局作用域的环境
    pub fn new() -> Self {
        let global = Scope::new(None);
        Self {
            current: ScopeId(0),
            scopes: vec![global],
            warmup_stats: None,
        }
    }

    /// 基于预热统计创建预分配容量的环境
    pub fn with_capacity(stats: WarmupStats) -> Self {
        let estimated_per_scope = if stats.max_scope_depth > 0 {
            stats.estimated_symbol_count / stats.max_scope_depth
        } else {
            stats.estimated_symbol_count
        };
        let global = Scope::with_capacity(None, estimated_per_scope.max(16));
        Self {
            current: ScopeId(0),
            scopes: vec![global],
            warmup_stats: Some(stats),
        }
    }

    /// 获取当前作用域 ID
    pub fn current_scope(&self) -> ScopeId {
        self.current
    }

    /// 进入新作用域：创建并切换为子作用域
    pub fn push_scope(&mut self) -> ScopeId {
        let parent = self.current;
        let estimated = self
            .warmup_stats
            .as_ref()
            .map(|s| s.estimated_symbol_count / (s.max_scope_depth.max(1)))
            .unwrap_or(8);
        let new_id = ScopeId(self.scopes.len());
        let new_scope = Scope::with_capacity(Some(parent), estimated);
        self.scopes.push(new_scope);
        self.current = new_id;
        new_id
    }

    /// 退出当前作用域：恢复到父作用域
    pub fn pop_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current.0].parent {
            self.current = parent;
        }
    }

    /// 在当前作用域中定义符号
    pub fn define(&mut self, name: &str, symbol: Symbol) -> Result<(), SemanticError> {
        let scope = &mut self.scopes[self.current.0];
        if scope.symbols.contains_key(name) {
            return Err(SemanticError::Redeclaration {
                name: name.to_string(),
                span: starry_ast::Span {
                    start: starry_ast::Position { line: 0, col: 0 },
                    end: starry_ast::Position { line: 0, col: 0 },
                },
            });
        }
        scope.symbols.insert(name.to_string(), symbol);
        Ok(())
    }

    /// 注册占位符符号（名称解析阶段使用）
    ///
    /// 仅在当前作用域中注册名字，类型标记为 `Ty::Unresolved`。
    /// 允许前向引用：即使符号尚未完整定义，名字也可被引用。
    pub fn define_placeholder(&mut self, name: &str) -> Result<(), SemanticError> {
        let symbol = Symbol::placeholder(name);
        self.define(name, symbol)
    }

    /// 沿作用域链查找符号（虎书的持久化查找）
    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        let mut current = Some(self.current);
        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0];
            if let Some(symbol) = scope.symbols.get(name) {
                return Some(symbol);
            }
            current = scope.parent;
        }
        None
    }

    /// 沿作用域链查找符号的可变引用
    pub fn resolve_symbol_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        let mut current = Some(self.current);
        while let Some(scope_id) = current {
            let parent = self.scopes[scope_id.0].parent;
            if self.scopes[scope_id.0].symbols.contains_key(name) {
                return self.scopes[scope_id.0].symbols.get_mut(name);
            }
            current = parent;
        }
        None
    }

    /// 仅在当前作用域中查找符号（用于重定义检查）
    pub fn resolve_local(&self, name: &str) -> Option<&Symbol> {
        self.scopes[self.current.0].symbols.get(name)
    }

    /// 遍历所有作用域的所有符号
    pub fn all_symbols(&self) -> Vec<(ScopeId, &Symbol)> {
        let mut result = Vec::new();
        for (idx, scope) in self.scopes.iter().enumerate() {
            for symbol in scope.symbols.values() {
                result.push((ScopeId(idx), symbol));
            }
        }
        result
    }

    /// 查找所有未解析的占位符符号
    pub fn unresolved_symbols(&self) -> Vec<(ScopeId, &Symbol)> {
        self.all_symbols()
            .into_iter()
            .filter(|(_, sym)| sym.is_placeholder())
            .collect()
    }

    /// 收集符号依赖关系图
    ///
    /// 返回每个符号及其依赖列表，用于依赖分析与拓扑排序。
    pub fn collect_dependencies(&self) -> Vec<(String, Vec<String>)> {
        self.all_symbols()
            .into_iter()
            .filter(|(_, sym)| !sym.dependencies.is_empty())
            .map(|(_, sym)| (sym.name.clone(), sym.dependencies.clone()))
            .collect()
    }

    /// 获取指定作用域的可变引用（内部使用）
    pub(crate) fn _scope_mut(&mut self, id: ScopeId) -> &mut Scope {
        &mut self.scopes[id.0]
    }

    /// 获取指定作用域的不可变引用（内部使用）
    pub(crate) fn _scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0]
    }
}

impl Clone for Env {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            current: self.current,
            warmup_stats: self.warmup_stats.clone(),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}
