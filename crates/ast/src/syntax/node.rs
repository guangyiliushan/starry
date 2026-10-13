//! 共享骨架：NodeId / NodeIdGen / Span / Path / Ident
//!
//! 每个节点携带 `id: NodeId` 和 `span: Span` 头字段；
//! 语义数据零内嵌（走 `Vec<Option<T>>` 稠密侧表，容量按
//! `NodeIdGen` 最终计数，空洞合法，索引 panic = 受控失败）。

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

/// 共享路径（点分或 :: 分隔）
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
    pub module: Option<Path>,
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
    DepthLimitExceeded,
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

/// 诊断收集器（append-only，emit → DiagId）
#[derive(Debug, Default)]
pub struct DiagCollector {
    diags: Vec<Diagnostic>,
}

impl DiagCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, diag: Diagnostic) -> DiagId {
        let id = DiagId(self.diags.len() as u32);
        self.diags.push(diag);
        id
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
