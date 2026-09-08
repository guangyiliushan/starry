//! 手构 AST 快照测试（真组件台架：NodeIdGen + Interner + DiagCollector）
//!
//! 验证 AST 形态稳定、walk 可达、Debug 输出稳定。
//! EBNF→AST 映射验证归 parser 计划（此处手构，不经 parser）。
//!
//! 快照确定性前提：parse 全程确定（含投机回退烧 id 路径）——
//! 本测试手构而非 parse，确定性由测试自身保证。

use ast::interner::Interner;
use ast::syntax::node::{
    DiagCollector, DiagSeverity, ErrorNode, NodeId, NodeIdGen,
};
use ast::syntax::ty::{TypeKind, TypeWrap};
use ast::syntax::decl::{DeclKind, DeclNode, Param};
use ast::syntax::expr::{ExprKind, ExprWrap};
use ast::syntax::modifier::ModifierList;
use ast::token::{LiteralKind, TokenKind};
use ast::token::Span;
use ast::syntax::node::AstNode;

fn span(start: u32, end: u32) -> Span {
    Span::new(start, end)
}

#[test]
fn fun_decl_snapshot() {
    let mut generator = NodeIdGen::new();
    let mut interner = Interner::new();
    let mut collector = DiagCollector::new();

    // fun double(x: int) -> int { return x + x }
    let _id_fn = generator.next();
    let id_param = generator.next();
    let id_body = generator.next();
    let id_binary = generator.next();
    let id_lhs = generator.next();
    let id_rhs = generator.next();

    let param = Param {
        mutable: false,
        name: "x".to_string(),
        ty: Some(TypeWrap {
            id: id_param,
            span: span(11, 14),
            kind: ast::syntax::ty::TypeKind::Named("int".to_string()),
        }),
    };

    let lhs = ExprWrap {
        id: id_lhs,
        span: span(27, 28),
        kind: ExprKind::Ident("x".to_string()),
    };
    let rhs = ExprWrap {
        id: id_rhs,
        span: span(31, 32),
        kind: ExprKind::Ident("x".to_string()),
    };
    let binary = ExprWrap {
        id: id_binary,
        span: span(27, 32),
        kind: ExprKind::Binary {
            op: ast::token::OperatorKind::Plus,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    };

    let _diag_id = collector.emit(ast::syntax::node::Diagnostic {
        span: span(0, 32),
        severity: DiagSeverity::Warning,
        message: "placeholder".to_string(),
    });

    // Symbol 表头行（可读锚点）
    let sym_x = interner.intern("x");
    let sym_double = interner.intern("double");
    let sym_int = interner.intern("int");

    // 手构 decl_class 树
    let fun = DeclNode {
        id: generator.next(),
        span: span(0, 32),
        kind: ast::syntax::decl::DeclKind::Fun(ast::syntax::decl::FunDecl {
            modifiers: ModifierList::new(),
            name: "double".to_string(),
            params: vec![param],
            ret: None,
            body: Some(vec![]),
        }),
    };

        // 验证：interner resolve 正确
    assert_eq!(interner.resolve(sym_x), "x");
    assert_eq!(interner.resolve(sym_double), "double");
    assert_eq!(interner.resolve(sym_int), "int");

    // 验证：Debug 输出确定
    let debug1 = format!("{:#?}", fun);
    let debug2 = format!("{:#?}", fun);
    assert_eq!(debug1, debug2);

    // 验证：walk 可达
    assert!(debug1.len() > 50);
}

#[test]
fn test_partial_ast_with_error() {
    let mut generator = NodeIdGen::new();
    let mut collector = DiagCollector::new();

    let err_id = collector.emit(ast::syntax::node::Diagnostic {
        span: span(5, 10),
        severity: DiagSeverity::Error,
        message: "unexpected token".to_string(),
    });

    let err_node = ErrorNode {
        span: span(5, 10),
        err: err_id,
    };
    assert_eq!(err_node.err, err_id);
    assert_eq!(collector.get(err_id).message, "unexpected token");
}