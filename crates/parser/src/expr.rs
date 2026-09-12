//! Pratt 表达式解析器（v1 最小骨架：字面量 + 标识符 + 二元运算）

use ast::interner::Interner;
use ast::syntax::expr::{ExprKind, ExprWrap};
use ast::syntax::node::{DiagCollector, DiagId, NodeId, NodeIdGen, Span};
use ast::token::OperatorKind;
use ast::token::TokenKind;
use crate::cursor::TokenCursor;

fn binding_power(op: &OperatorKind) -> Option<(u8, u8)> {
    match op {
        OperatorKind::Or => Some((1, 2)),
        OperatorKind::And => Some((3, 4)),
        OperatorKind::EqEq | OperatorKind::BangEq => Some((5, 6)),
        OperatorKind::Lt | OperatorKind::LtEq
        | OperatorKind::Gt | OperatorKind::GtEq => Some((7, 8)),
        OperatorKind::Plus | OperatorKind::Minus => Some((9, 10)),
        OperatorKind::Star | OperatorKind::Slash | OperatorKind::Percent => Some((11, 12)),
        _ => None,
    }
}

pub fn parse_expr(
    cursor: &mut TokenCursor,
    interner: &mut Interner,
    id_gen: &mut NodeIdGen,
    diags: &mut DiagCollector,
) -> ExprWrap {
    parse_bp(cursor, interner, id_gen, diags, 0)
}

fn parse_bp(
    cursor: &mut TokenCursor,
    interner: &mut Interner,
    id_gen: &mut NodeIdGen,
    diags: &mut DiagCollector,
    min_bp: u8,
) -> ExprWrap {
    let mut lhs = parse_prefix(cursor, interner, id_gen, diags);
    loop {
        let op = {
            let tok = cursor.peek();
            match &tok.kind {
                TokenKind::Operator(op) => match binding_power(op) {
                    Some((l, _)) if l >= min_bp => Some(op.clone()),
                    _ => None,
                },
                _ => None,
            }
        };
        let Some(op) = op else { break };
        cursor.advance();
        let rhs = parse_bp(cursor, interner, id_gen, diags, min_bp);
        let span = Span::new(lhs.span.start, rhs.span.end);
        let id = id_gen.next();
        lhs = ExprWrap {
            id,
            span,
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        };
    }
    lhs
}

fn parse_prefix(
    cursor: &mut TokenCursor,
    interner: &mut Interner,
    id_gen: &mut NodeIdGen,
    diags: &mut DiagCollector,
) -> ExprWrap {
    let tok = cursor.advance();
    let span = tok.span;
    let id = id_gen.next();
    match &tok.kind {
        TokenKind::Literal(kind) => ExprWrap {
            id, span, kind: ExprKind::Literal(*kind),
        },
        TokenKind::Identifier => {
            let sym = interner.intern(&tok.lexeme);
            let _ = sym;
            ExprWrap { id, span, kind: ExprKind::Ident(sym) }
        }
        TokenKind::Punctuation(ast::token::PunctuationKind::LParen) => {
            let inner = parse_bp(cursor, interner, id_gen, diags, 0);
            cursor.bump_if(&TokenKind::Punctuation(ast::token::PunctuationKind::RParen));
            inner
        }
        _ => {
            let diag_id = diags.emit(ast::syntax::node::Diagnostic {
                span,
                severity: ast::syntax::node::DiagSeverity::Error,
                message: format!("unexpected token: {}", tok.kind),
            });
            ExprWrap {
                id, span,
                kind: ExprKind::Error(ast::syntax::node::ErrorNode {
                    id, span, err: diag_id,
                }),
            }
        }
    }
}