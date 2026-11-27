//! 语句空间：block 成员的统一语法快照。

use super::decl::BindingDecl;
use super::expr::{Block, ExprStmt, ExprWrap};
use super::node::{ErrorNode, MissingNode, NodeId, Span, Symbol};
use super::pat::PatWrap;
use crate::token::OperatorKind;

#[derive(Debug, Clone)]
pub struct StmtWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: StmtKind,
}

impl StmtWrap {
    pub const fn new(id: NodeId, span: Span, kind: StmtKind) -> Self {
        Self { id, span, kind }
    }
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Expr(ExprStmt),
    Assignment {
        target: Box<ExprWrap>,
        op: OperatorKind,
        value: Box<ExprWrap>,
    },
    Binding(Box<BindingDecl>),
    For {
        label: Option<Symbol>,
        pattern: Box<PatWrap>,
        iterable: Box<ExprWrap>,
        body: Box<Block>,
    },
    While {
        label: Option<Symbol>,
        cond: Box<crate::syntax::expr::Condition>,
        body: Box<Block>,
    },
    Loop {
        label: Option<Symbol>,
        body: Box<Block>,
    },
    Break {
        label: Option<Symbol>,
        value: Option<Box<ExprWrap>>,
    },
    Continue {
        label: Option<Symbol>,
    },
    Return {
        value: Option<Box<ExprWrap>>,
    },
    Defer {
        body: Box<Block>,
    },
    Comptime {
        block: Box<Block>,
    },
    Grow {
        expr: Box<ExprWrap>,
        bits: Option<Box<ExprWrap>>,
    },
    Error(ErrorNode),
    Missing(MissingNode),
}
