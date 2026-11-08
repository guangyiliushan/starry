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
    /// 借用引用（D-007 / M2）：&T 只读 / &mut T 独占可变（T1）。
    /// v1 只有类型层：取址/解引表达式同 slice 排期。
    Ref {
        mutable: bool,
        inner: Box<TypeWrap>,
    },
    /// 裸指针（D-007 / M2）：只允许 *const T / *mut T（bare *T 诊断，
    /// 不再静默当 const）。当前只有类型+FFI 传参语义。
    Pointer {
        mutable: bool,
        inner: Box<TypeWrap>,
    },
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
