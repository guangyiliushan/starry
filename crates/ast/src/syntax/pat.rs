//! 模式空间（与冻结 EBNF Pattern 产生式一一对应）

use super::node::{ErrorNode, MissingNode, NodeId, Path, Span, Symbol};
use super::ty::TypeWrap;
use crate::token::LiteralKind;

#[derive(Debug, Clone)]
pub struct PatWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: PatKind,
}

impl PatWrap {
    pub const fn new(id: NodeId, span: Span, kind: PatKind) -> Self {
        Self { id, span, kind }
    }
}

#[derive(Debug, Clone)]
pub enum PatKind {
    Or(Vec<PatWrap>),
    Binding {
        mutable: bool,
        name: Symbol,
        ty: Option<Box<TypeWrap>>,
    },
    Literal(LiteralKind),
    Bool(bool),
    Null,
    Wildcard,
    Tuple {
        elements: Vec<PatWrap>,
        rest: Option<usize>,
    },
    /// `rest` is the slot index at which `..` sits among `elements`: `[a, .., b]`
    /// is `{ elements: [a, b], rest: Some(1) }`, `[.., b]` is `{ [b], Some(0) }`,
    /// `[a, ..]` is `{ [a], Some(1) }`, `[..]` is `{ [], Some(0) }`. `None` means
    /// the pattern has no rest element. The grammar allows at most one.
    Array {
        elements: Vec<PatWrap>,
        rest: Option<usize>,
    },
    Constructor {
        path: Path,
        args: Vec<PatWrap>,
    },
    Struct {
        path: Path,
        fields: Vec<(Symbol, PatWrap)>,
    },
    Range {
        lo: Box<PatWrap>,
        hi: Box<PatWrap>,
        inclusive: bool,
    },
    Error(ErrorNode),
    Missing(MissingNode),
}
