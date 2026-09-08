//! 类型空间

use super::node::{NodeId, Span};

#[derive(Debug, Clone)]
pub struct TypeWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeKind,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Named(String),
    Nullable(Box<TypeWrap>),
    Fn {
        params: Vec<TypeWrap>,
        ret: Box<TypeWrap>,
    },
    Tuple(Vec<TypeWrap>),
}

/// 类型变异性（in 硬关键字 / «out» 槽位词——记法不对称是词汇冻结后果）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// `in`（硬关键字 token）
    In,
    /// «out»（槽位词 Identifier）
    Out,
}
