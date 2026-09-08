//! 表达式空间（Pratt 解析产物，糖全保留）

use super::node::{ErrorNode, MissingNode, NodeId, Span};
use super::ty::TypeWrap;
use crate::token::{LiteralKind, OperatorKind};

#[derive(Debug, Clone)]
pub struct ExprWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(LiteralKind),
    Ident(String),
    Path(Vec<String>),
    Binary {
        op: OperatorKind,
        lhs: Box<ExprWrap>,
        rhs: Box<ExprWrap>,
    },
    Unary {
        op: OperatorKind,
        operand: Box<ExprWrap>,
    },
    Cast {
        expr: Box<ExprWrap>,
        ty: Box<super::ty::TypeWrap>,
        kind: CastKind,
    },
    Call {
        callee: Box<ExprWrap>,
        args: Vec<ExprWrap>,
    },
    Index {
        base: Box<ExprWrap>,
        index: Box<ExprWrap>,
    },
    Member {
        base: Box<ExprWrap>,
        name: String,
    },
    Lambda {
        params: Vec<String>,
        body: Box<ExprWrap>,
    },
    StringInterp {
        parts: Vec<ExprWrap>,
    },
    Error(ErrorNode),
    Missing(MissingNode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    As,
    AsQuestion,
    AsBang,
}
