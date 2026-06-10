use crate::ty::Ty;

/// 符号状态
///
/// 支持多遍语义分析流程中的"占位符 → 已解析"状态转换。
/// 名称解析阶段注册占位符，类型检查阶段将其解析为具体类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolStatus {
    /// 占位符：仅注册了名字，类型尚未解析（名称解析阶段产出）
    Placeholder,
    /// 已解析：类型已完全确定（类型检查阶段产出）
    Resolved,
}

/// 符号种类
///
/// 区分不同种类的声明，便于在语义分析中施加不同的规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Constant,
    Function,
    Parameter,
}

/// 符号表条目
///
/// 对应虎书中 Symbol 的概念：每个标识符在声明时产生一个符号条目，
/// 存储其名称、类型、可变性等语义属性。
///
/// 符号条目被存储在 `attribute::env::Scope` 的哈希表中，
/// 通过 `Env::resolve` 沿作用域链查找。
#[derive(Debug, Clone)]
pub struct Symbol {
    /// 符号名称（标识符）
    pub name: String,
    /// 符号的静态类型
    pub ty: Ty,
    /// 是否为常量（不可重新赋值）
    pub is_const: bool,
    /// 是否可变（Rust 风格的 mut 语义）
    pub is_mutable: bool,
    /// 符号状态（占位符或已解析）
    pub status: SymbolStatus,
    /// 符号种类
    pub kind: SymbolKind,
    /// 依赖的其它符号名称（用于依赖分析与拓扑排序）
    pub dependencies: Vec<String>,
    /// 栈帧偏移量（IR 生成阶段填充）
    pub stack_offset: Option<usize>,
}

impl Symbol {
    /// 创建一个新的变量符号（已解析状态）
    pub fn var(name: impl Into<String>, ty: Ty) -> Self {
        Self {
            name: name.into(),
            ty,
            is_const: false,
            is_mutable: true,
            status: SymbolStatus::Resolved,
            kind: SymbolKind::Variable,
            dependencies: Vec::new(),
            stack_offset: None,
        }
    }

    /// 创建一个新的常量符号（已解析状态）
    pub fn constant(name: impl Into<String>, ty: Ty) -> Self {
        Self {
            name: name.into(),
            ty,
            is_const: true,
            is_mutable: false,
            status: SymbolStatus::Resolved,
            kind: SymbolKind::Constant,
            dependencies: Vec::new(),
            stack_offset: None,
        }
    }

    /// 创建错误降级符号（用于未定义变量等错误场景，避免级联报错）
    pub fn error(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: Ty::Error,
            is_const: false,
            is_mutable: false,
            status: SymbolStatus::Resolved,
            kind: SymbolKind::Variable,
            dependencies: Vec::new(),
            stack_offset: None,
        }
    }

    /// 创建占位符符号（名称解析阶段使用）
    ///
    /// 仅注册名字，类型标记为 `Ty::Unresolved`。
    /// 在类型检查阶段通过 `resolve()` 将其解析为具体类型。
    pub fn placeholder(name: impl Into<String>) -> Self {
        let name_str = name.into();
        let ty = Ty::Unresolved(name_str.clone());
        Self {
            name: name_str,
            ty,
            is_const: false,
            is_mutable: true,
            status: SymbolStatus::Placeholder,
            kind: SymbolKind::Variable,
            dependencies: Vec::new(),
            stack_offset: None,
        }
    }

    /// 判断是否为占位符符号
    pub fn is_placeholder(&self) -> bool {
        self.status == SymbolStatus::Placeholder
    }

    /// 将占位符解析为具体类型
    ///
    /// 将状态从 Placeholder 转为 Resolved，并更新类型。
    /// 若符号已处于 Resolved 状态，此方法无操作。
    pub fn resolve(&mut self, ty: Ty) {
        if self.status == SymbolStatus::Placeholder {
            self.ty = ty;
            self.status = SymbolStatus::Resolved;
        }
    }
}
