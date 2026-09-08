//! 四空间 AST：Decl / Expr / Type / Pattern + 共享骨架（node.rs）
//! 与共享辅件（modifier.rs）

pub mod decl;
pub mod expr;
pub mod modifier;
pub mod node;
pub mod pat;
pub mod ty;

pub use node::{
    AstNode, DiagCollector, DiagId, DiagSeverity, Diagnostic, ErrorNode,
};
pub use decl::DeclNode;
pub use expr::ExprWrap;
pub use ty::TypeWrap;

/// 解析结果（ast + interner + diagnostics + node_count 同船旅行）
pub struct ParseOutput {
    pub interner: Interner,
    pub diagnostics: Vec<Diagnostic>,
    pub node_count: u32,
}

use crate::interner::Interner;
use node::SourceFile;
