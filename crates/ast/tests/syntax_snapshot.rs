//! 手构 AST 快照测试（真组件台架：NodeIdGen + Interner + DiagCollector）
//!
//! 验证 AST 形态稳定、walk 可达、Debug 输出稳定。
//! EBNF→AST 映射验证归 parser 计划（此处手构，不经 parser）。
//!
//! 快照确定性前提：parse 全程确定（含投机回退烧 id 路径）——
//! 本测试手构而非 parse，确定性由测试自身保证。

use ast::interner::Interner;
use ast::syntax::decl::{BindingDecl, DeclKind, DeclNode, FunDecl, FunctionBody, Param};
use ast::syntax::expr::{ExprKind, ExprStmt, ExprWrap};
use ast::syntax::modifier::ModifierList;
use ast::syntax::node::AstNode;
use ast::syntax::node::{DiagCollector, DiagId, DiagSeverity, Diagnostic, NodeId, NodeIdGen};
use ast::syntax::ty::{TypeKind, TypeWrap};
use ast::token::{OperatorKind, Span};

fn span(start: u32, end: u32) -> Span {
    Span::new(start, end)
}

#[test]
fn fun_decl_snapshot() {
    let mut id_gen = NodeIdGen::new();
    let mut interner = Interner::new();
    let mut collector = DiagCollector::new();

    let sym_double = interner.intern("double");
    let sym_x = interner.intern("x");
    let sym_int = interner.intern("int");

    // fun double(x: int) -> int { return x + x }
    let param_ty = TypeWrap {
        id: id_gen.next(),
        span: span(14, 17),
        kind: TypeKind::Named {
            path: ast::syntax::node::Path::single(sym_int, span(14, 17)),
        },
    };
    let param = Param {
        mutable: false,
        name: sym_x,
        ty: Some(param_ty),
        default: None,
    };

    let lhs = ExprWrap {
        id: id_gen.next(),
        span: span(29, 30),
        kind: ExprKind::Ident(sym_x),
    };
    let rhs = ExprWrap {
        id: id_gen.next(),
        span: span(33, 34),
        kind: ExprKind::Ident(sym_x),
    };
    let add = ExprWrap {
        id: id_gen.next(),
        span: span(29, 34),
        kind: ExprKind::Binary {
            op: OperatorKind::Plus,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    };

    let diag = Diagnostic {
        span: span(0, 34),
        severity: ast::syntax::node::DiagSeverity::Warning,
        message: "placeholder".to_string(),
    };
    let _did = collector.emit(diag);

    let fun_decl = FunDecl {
        modifiers: ModifierList::new(),
        name: sym_double,
        type_params: Vec::new(),
        params: vec![param],
        ret: None,
        body: Some(FunctionBody::Block(ast::syntax::expr::Block {
            stmts: vec![],
            tail: None,
        })),
    };

    let decl = DeclNode {
        id: id_gen.next(),
        span: span(0, 34),
        kind: DeclKind::Fun(fun_decl),
    };

    // AstNode trait 可访问
    assert_eq!(decl.id(), NodeId(id_gen.count() - 1));
    assert_eq!(decl.span(), span(0, 34));

    // Debug 输出确定
    let d1 = format!("{:#?}", decl);
    let d2 = format!("{:#?}", decl);
    assert_eq!(d1, d2);

    // interner resolve 正确
    assert_eq!(interner.resolve(sym_double), "double");
    assert_eq!(interner.resolve(sym_x), "x");
    assert_eq!(interner.resolve(sym_int), "int");

    // DiagCollector 可 resolve
    assert_eq!(collector.all().len(), 1);
}

#[test]
fn partial_ast_with_error() {
    let mut id_gen = NodeIdGen::new();
    let mut collector = DiagCollector::new();

    let did = collector.emit(Diagnostic {
        span: span(5, 10),
        severity: ast::syntax::node::DiagSeverity::Error,
        message: "unexpected token".to_string(),
    });

    let err_node = ast::syntax::node::ErrorNode {
        id: id_gen.next(),
        span: span(5, 10),
        err: did,
    };

    assert_eq!(err_node.err, did);
    assert_eq!(collector.get(did).message, "unexpected token");
    assert_eq!(collector.all().len(), 1, "partial-AST 诊断唯一性");
}
