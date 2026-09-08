//! 模式空间

use super::node::{NodeId, Span};
use crate::token::LiteralKind;

#[derive(Debug, Clone)]
pub struct PatWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: PatKind,
}

#[derive(Debug, Clone)]
pub enum PatKind {
    Binding {
        mutable: bool,
        name: String,
        ty: Option<Box<super::ty::TypeWrap>>,
    },
    Literal(LiteralKind),
    Is(Box<super::ty::TypeWrap>),
    Wildcard,
    Tuple(Vec<PatWrap>),
}
