//! 表达式空间（Pratt 解析产物，糖全保留）

use super::node::{ErrorNode, MissingNode, NodeId, Span, Symbol};
use super::ty::TypeWrap;
use crate::token::{LiteralKind, OperatorKind};

#[derive(Debug, Clone)]
pub struct ExprWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: ExprKind,
}

/// 语句：表达式 + 可选分号（Block 的成员）
#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: ExprWrap,
}

/// 块：语句序列 + 可选尾表达式（块尾取值）
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<ExprStmt>,
    pub tail: Option<Box<ExprWrap>>,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(LiteralKind),
    Ident(Symbol),
    Path(super::node::Path),
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
        name: Symbol,
    },
    Lambda {
        params: Vec<Symbol>,
        body: Box<ExprWrap>,
    },
    StringInterp {
        parts: Vec<ExprWrap>,
    },
    When {
        subject: Option<Box<ExprWrap>>,
        branches: Vec<WhenBranch>,
    },
    If {
        cond: Box<ExprWrap>,
        then_block: Box<Block>,
        else_block: Option<Box<ExprWrap>>,
    },
    Range {
        lo: Box<ExprWrap>,
        hi: Option<Box<ExprWrap>>,
        inclusive: bool,
    },
    Tuple(Vec<ExprWrap>),
    Array(Vec<ExprWrap>),
    Error(ErrorNode),
    Missing(MissingNode),
}

/// when 分支
#[derive(Debug, Clone)]
pub struct WhenBranch {
    pub pattern: ExprWrap,
    pub body: WhenBody,
}

#[derive(Debug, Clone)]
pub enum WhenBody {
    Expr(ExprWrap),
    Block(Block),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    As,
    AsQuestion,
    AsBang,
}
