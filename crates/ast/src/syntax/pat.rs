//! 模式空间

use super::node::{NodeId, Span, Symbol};
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
        name: Symbol,
        ty: Option<Box<super::ty::TypeWrap>>,
    },
    Literal(LiteralKind),
    Is(Box<super::ty::TypeWrap>),
    Wildcard,
    Tuple(Vec<PatWrap>),
    Array(Vec<PatWrap>),
    Constructor {
        path: super::node::Path,
        args: Vec<PatWrap>,
    },
    Struct {
        path: super::node::Path,
        fields: Vec<(Symbol, PatWrap)>,
    },
    Range {
        lo: Box<super::pat::PatWrap>,
        hi: Box<super::pat::PatWrap>,
    },
}
