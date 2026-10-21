//! 类型空间

use super::node::{ErrorNode, MissingNode, NodeId, Path, Span, Symbol};

#[derive(Debug, Clone)]
pub struct TypeWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeKind,
}

#[derive(Debug, Clone)]
pub struct TypeArg {
    pub id: NodeId,
    pub span: Span,
    /// `IDENT "=" Type` associated binding；普通 type arg 为 `None`。
    pub associated: Option<Symbol>,
    pub ty: Box<TypeWrap>,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub id: NodeId,
    pub span: Span,
    pub variance: Option<Variance>,
    pub name: TypeParamName,
    /// `T : A & B` — intersection bounds, same shape as `WhereConstraint::bounds`.
    pub bounds: Vec<TypeWrap>,
    pub default: Option<Box<TypeWrap>>,
}

/// Type parameter names carry recovery without inventing a semantic symbol.
#[derive(Debug, Clone)]
pub enum TypeParamName {
    Valid(Symbol),
    Error(ErrorNode),
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Union(Vec<TypeWrap>),
    Named {
        path: Path,
        type_args: Vec<TypeArg>,
    },
    Dyn(Box<TypeWrap>),
    SelfType,
    Nullable(Box<TypeWrap>),
    Fn {
        params: Vec<TypeWrap>,
        ret: Box<TypeWrap>,
    },
    Array {
        element: Box<TypeWrap>,
        len: u32,
    },
    Tuple(Vec<TypeWrap>),
    Unit,
    Error(ErrorNode),
    Missing(MissingNode),
}

/// 类型变异性（`in` 硬关键字 / `out` slot word）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    In,
    Out,
}
