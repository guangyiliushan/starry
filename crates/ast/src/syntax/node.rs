//! 共享骨架：NodeId / NodeIdGen / Span / Path / Ident
//!
//! 每个节点携带 `id: NodeId` 和 `span: Span` 头字段；
//! 语义数据零内嵌（走 `Vec<Option<T>>` 稠密侧表，容量按
//! `NodeIdGen` 最终计数，空洞合法，索引 panic = 受控失败）。

use std::collections::HashMap;

pub use crate::token::Span;

/// 节点唯一标识（全局计数 u32，跨 pass 稳定）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// 节点 ID 生成器（解析期分配；回绕 debug_assert）
#[derive(Debug, Default)]
pub struct NodeIdGen {
    next: u32,
}

impl NodeIdGen {
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    pub fn try_allocate(&mut self) -> Option<NodeId> {
        self.next.checked_add(1).map(|next| {
            let id = NodeId(self.next);
            self.next = next;
            id
        })
    }

    pub fn allocate(&mut self) -> NodeId {
        self.try_allocate().expect("NodeId space exhausted")
    }

    pub fn count(&self) -> u32 {
        self.next
    }
}

/// 所有 AST 节点的公共 trait（id/span 访问）
pub trait AstNode {
    fn id(&self) -> NodeId;
    fn span(&self) -> Span;
}

macro_rules! impl_ast_node {
    ($($ty:ty),* $(,)?) => {
        $(
            impl AstNode for $ty {
                fn id(&self) -> NodeId { self.id }
                fn span(&self) -> Span { self.span }
            }
        )*
    };
}

/// 共享路径段
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSegment {
    pub name: Symbol,
    pub span: Span,
}

/// 共享路径（点分）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<PathSegment>,
}

impl Path {
    pub fn single(name: Symbol, span: Span) -> Self {
        Self {
            segments: vec![PathSegment { name, span }],
        }
    }
}

// re-export for convenience
pub use crate::interner::Symbol;

/// 编译单元根节点
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: NodeId,
    pub span: Span,
    pub package: Option<Path>,
    pub items: Vec<crate::syntax::decl::DeclNode>,
}

/// Error 节点：携带 DiagId 回指诊断（抑制级联报错）
#[derive(Debug, Clone)]
pub struct ErrorNode {
    pub id: NodeId,
    pub span: Span,
    pub err: DiagId,
}

/// Missing 节点：插入点空 Span
#[derive(Debug, Clone)]
pub struct MissingNode {
    pub id: NodeId,
    pub span: Span,
}

/// 诊断 ID newtype
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagId(pub u32);

/// 诊断严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagSeverity {
    Error,
    Warning,
}

/// 稳定诊断类别；message 仅用于人类展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagCode {
    UnexpectedToken,
    ExpectedToken,
    InvalidAssignmentTarget,
    InvalidEscape,
    DepthLimitExceeded,
    UnclosedInterpolation,
    IntegerOutOfRange,
    /// `let` is not a Starry keyword; `let x = 1` must not silently split.
    LetKeyword,
    /// `..` appeared more than once in a tuple/array pattern.
    DuplicateRestPattern,
    // ---- semantic analysis（crates/sem 命名空间）----
    /// 未定义的名字
    SemUndefinedName,
    /// 未定义的类型
    SemUndefinedType,
    /// 重复定义
    SemDuplicateDef,
    /// 类型别名循环
    SemTypeCycle,
    /// 类型不匹配
    SemTypeMismatch,
    /// 调用目标不是函数
    SemNotCallable,
    /// 实参个数错误
    SemArityMismatch,
    /// 命名实参没有对应的参数名
    SemUnknownParam,
    /// 向不可变绑定赋值
    SemAssignToVal,
    /// 标签没有匹配的循环
    SemUnknownLabel,
    /// 循环外 break/continue
    SemBreakOutsideLoop,
    /// 函数外 return
    SemReturnOutsideFun,
    /// when 缺少 else（v1 保守穷尽早返回）
    SemMissingElse,
    /// when 未穷尽枚举变体
    SemNonExhaustiveWhen,
    /// Own 值移动后再次使用
    SemUseAfterMove,
    /// v1 暂不支持的构造
    SemUnsupportedConstruct,
    /// 宏展开是独立前置 pass，v1 未实现
    SemMacroExpansion,
    /// 参数缺少类型标注
    SemParamTypeRequired,
    /// 条件不是 Boolean
    SemConditionNotBool,
    /// 未知成员（字段/构造器）
    SemUnknownMember,
    // ---- intermediate code generation（crates/irc 命名空间）----
    /// sem 已接受但 irc 当前里程碑尚未 lower 的构造（M1 边界）
    IrcUnsupportedLowering,
    /// 枚举变体数超过槽位布局 tag 宽度（>255）
    IrcTooManyVariants,
}

/// 最小诊断
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub span: Span,
    pub severity: DiagSeverity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(code: DiagCode, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            severity: DiagSeverity::Error,
            message: message.into(),
        }
    }
}

/// 诊断收集器（append-only，emit → DiagId）。
///
/// `unique` maps each key to its first visible diagnostic. A speculative
/// emission may leave a stale id after rollback; visibility is checked against
/// `diags.len()`, so rollback is O(1) and never scans or rebuilds the map.
#[derive(Debug, Default)]
pub struct DiagCollector {
    diags: Vec<Diagnostic>,
    unique: HashMap<(Span, DiagCode), DiagId>,
}

impl DiagCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, diag: Diagnostic) -> DiagId {
        let id = DiagId(self.diags.len() as u32);
        let key = (diag.span, diag.code);
        if !self
            .unique
            .get(&key)
            .is_some_and(|existing| existing.0 < id.0)
        {
            self.unique.insert(key, id);
        }
        self.diags.push(diag);
        id
    }

    /// Map lookup for parser-side unique emission: O(1), not a diagnostic scan.
    pub fn find_unique(&self, span: Span, code: DiagCode) -> Option<DiagId> {
        self.unique
            .get(&(span, code))
            .copied()
            .filter(|id| id.0 < self.diags.len() as u32)
    }

    /// 投机解析 checkpoint；失败分支整体回滚，成功分支无需动作。
    pub fn checkpoint(&self) -> usize {
        self.diags.len()
    }

    pub fn rollback(&mut self, checkpoint: usize) {
        self.diags.truncate(checkpoint);
    }

    pub fn get(&self, id: DiagId) -> &Diagnostic {
        &self.diags[id.0 as usize]
    }

    pub fn all(&self) -> &[Diagnostic] {
        &self.diags
    }
}

impl_ast_node!(
    crate::syntax::decl::DeclNode,
    crate::syntax::expr::ExprWrap,
    crate::syntax::ty::TypeWrap,
    crate::syntax::ty::TypeArg,
    crate::syntax::ty::TypeParam,
    crate::syntax::pat::PatWrap,
    crate::syntax::stmt::StmtWrap,
    SourceFile,
    ErrorNode,
    MissingNode,
);
