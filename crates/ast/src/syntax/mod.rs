//! 四空间 AST：Decl / Expr / Type / Pattern + 共享骨架（node.rs）
//! 与共享辅件（modifier.rs）

use crate::interner::Interner;

pub mod decl;
pub mod expr;
pub mod modifier;
pub mod node;
pub mod pat;
pub mod stmt;
pub mod ty;

pub use decl::DeclNode;
pub use expr::ExprWrap;
pub use expr::{Block, ExprStmt};
pub use node::{
    AstNode, DiagCode, DiagCollector, DiagId, DiagSeverity, Diagnostic, ErrorNode, MissingNode,
    SourceFile,
};
pub use pat::PatWrap;
pub use stmt::StmtWrap;
pub use ty::TypeWrap;

/// 解析结果（ast + interner + diagnostics + node_count 同船旅行；
/// interner 与 AST 同生共死——Symbol resolve 唯一入口）
pub struct ParseOutput {
    pub ast: SourceFile,
    pub interner: Interner,
    pub diagnostics: Vec<Diagnostic>,
    pub node_count: u32,
}
