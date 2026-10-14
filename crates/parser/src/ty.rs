//! Type parser：冻结 EBNF 的 Type 空间。

use ast::syntax::node::{DiagCode, Span};
use ast::syntax::ty::{TypeArg, TypeKind, TypeWrap};
use ast::token::{LiteralKind, OperatorKind, PunctuationKind, TokenKind};

use crate::expr::parse_path;
use crate::{ParseSession, ParserError};

pub(crate) fn parse_type_params(
    session: &mut ParseSession<'_>,
) -> Result<Vec<ast::syntax::ty::TypeParam>, ParserError> {
    if !session.cursor.check(&TokenKind::Operator(OperatorKind::Lt)) {
        return Ok(Vec::new());
    }
    session.cursor.advance();
    let mut params = Vec::new();
    while !at_type_args_close(session) {
        if session.cursor.is_eof() {
            missing_close(session);
            break;
        }
        params.push(parse_type_param(session)?);
        if !session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            break;
        }
        session.cursor.advance();
    }
    consume_type_args_close(session);
    Ok(params)
}

fn parse_type_param(
    session: &mut ParseSession<'_>,
) -> Result<ast::syntax::ty::TypeParam, ParserError> {
    let start = session.cursor.peek().span;
    let variance = parse_variance(session);
    let name_token = session.cursor.advance();
    let name = session.interner.intern(session.token_text(&name_token));
    let bound = if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        Some(Box::new(parse_type(session)?))
    } else {
        None
    };
    let default = if session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Eq))
    {
        Some(Box::new(parse_type(session)?))
    } else {
        None
    };
    let end = session
        .cursor
        .peek_ahead(0)
        .map(|token| token.span)
        .unwrap_or_default();
    Ok(ast::syntax::ty::TypeParam {
        id: session.next_node()?,
        span: join_span(start, end),
        variance,
        name,
        bound,
        default,
    })
}

fn parse_variance(session: &mut ParseSession<'_>) -> Option<ast::syntax::ty::Variance> {
    if session
        .cursor
        .check(&TokenKind::Keyword(ast::token::HardKeyword::In))
    {
        session.cursor.advance();
        if let TokenKind::Identifier = session.cursor.peek().kind
            && session.token_text(session.cursor.peek()) == "out"
        {
            session.cursor.advance();
            return Some(ast::syntax::ty::Variance::InOut);
        }
        return Some(ast::syntax::ty::Variance::In);
    }
    if let TokenKind::Identifier = session.cursor.peek().kind
        && session.token_text(session.cursor.peek()) == "out"
    {
        session.cursor.advance();
        return Some(ast::syntax::ty::Variance::Out);
    }
    None
}

pub(crate) fn parse_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    match session.try_with_depth(parse_type_inner)? {
        Some(ty) => Ok(ty),
        None => missing_type(session, session.cursor.peek().span),
    }
}

fn parse_type_inner(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let mut function = parse_union_type(session)?;
    while session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
    {
        let ret = parse_type(session)?;
        let span = join_span(function.span, ret.span);
        let id = session.next_node()?;
        function = TypeWrap {
            id,
            span,
            kind: TypeKind::Fn {
                params: vec![function],
                ret: Box::new(ret),
            },
        };
    }
    Ok(function)
}

pub(crate) fn parse_union_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let first = parse_atomic_type(session)?;
    if !session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::BitOr))
    {
        return Ok(first);
    }

    let start = first.span;
    let mut types = vec![first];
    while session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::BitOr))
    {
        types.push(parse_atomic_type(session)?);
    }
    let end = types.last().map(|ty| ty.span).unwrap_or_default();
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(start, end),
        kind: TypeKind::Union(types),
    })
}

fn parse_atomic_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let token = *session.cursor.peek();
    if token.kind == TokenKind::Eof {
        return missing_type(session, token.span);
    }
    match token.kind {
        TokenKind::Punctuation(PunctuationKind::LParen) => parse_paren_or_fn_type(session),
        TokenKind::Punctuation(PunctuationKind::LBracket) => parse_array_type(session),
        TokenKind::Identifier => parse_named_type(session),
        TokenKind::Keyword(ast::token::HardKeyword::Self_) => {
            let token = session.cursor.advance();
            let name = session.interner.intern("Self");
            parse_named_suffix(
                session,
                ast::syntax::node::Path::single(name, token.span),
                token.span,
            )
        }
        TokenKind::Keyword(ast::token::HardKeyword::Dyn) => {
            session.cursor.advance();
            let bound = parse_type(session)?;
            let span = join_span(token.span, bound.span);
            let id = session.next_node()?;
            Ok(TypeWrap {
                id,
                span,
                kind: TypeKind::Dyn(Box::new(bound)),
            })
        }
        _ => missing_type(session, token.span),
    }
}

fn parse_named_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let start = session.cursor.peek().span;
    let path = parse_path(session)?;
    parse_named_suffix(session, path, start)
}

fn parse_named_suffix(
    session: &mut ParseSession<'_>,
    path: ast::syntax::node::Path,
    start: Span,
) -> Result<TypeWrap, ParserError> {
    let type_args = if session.cursor.check(&TokenKind::Operator(OperatorKind::Lt)) {
        parse_type_args(session)?
    } else {
        Vec::new()
    };
    let mut ty = TypeWrap {
        id: session.next_node()?,
        span: path
            .segments
            .last()
            .map(|segment| segment.span)
            .unwrap_or_default(),
        kind: TypeKind::Named { path, type_args },
    };

    while session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::Question))
    {
        session.cursor.advance();
        let id = session.next_node()?;
        let end = ty.span.end;
        ty = TypeWrap {
            id,
            span: join_span(ty.span, Span::new(end, end)),
            kind: TypeKind::Nullable(Box::new(ty)),
        };
    }

    debug_assert!(ty.span.start >= start.start);
    Ok(ty)
}

pub(crate) fn parse_type_args(session: &mut ParseSession<'_>) -> Result<Vec<TypeArg>, ParserError> {
    session.cursor.advance(); // <
    let mut args = Vec::new();
    while !at_type_args_close(session) {
        if session.cursor.is_eof() {
            missing_close(session);
            break;
        }
        let start = session.cursor.peek().span;
        let associated = if let TokenKind::Identifier = session.cursor.peek().kind {
            let is_assoc = session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Operator(OperatorKind::Eq));
            if is_assoc {
                let symbol = session
                    .interner
                    .intern(session.token_text(session.cursor.peek()));
                session.cursor.advance();
                session.cursor.advance();
                Some(symbol)
            } else {
                None
            }
        } else {
            None
        };
        let ty = parse_type(session)?;
        let end = ty.span;
        args.push(TypeArg {
            id: session.next_node()?,
            span: join_span(start, end),
            associated,
            ty: Box::new(ty),
        });
        if !session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            break;
        }
        session.cursor.advance();
    }
    consume_type_args_close(session);
    Ok(args)
}

fn parse_array_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let open = session.cursor.advance();
    let element = parse_type(session)?;
    let semi = session.cursor.peek().span;
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon))
    {
        session.emit_unique(DiagCode::ExpectedToken, semi, || "expected ';'".to_string());
    }
    let count_token = session.cursor.advance();
    let count = match count_token.kind {
        TokenKind::Literal(LiteralKind::Integer) => session
            .token_text(&count_token)
            .replace('_', "")
            .parse::<u32>()
            .unwrap_or(u32::MAX),
        _ => {
            session.emit_unique(DiagCode::UnexpectedToken, count_token.span, || {
                "expected array length".to_string()
            });
            0
        }
    };
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RBracket))
    {
        missing_close(session);
    }
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(open.span, count_token.span),
        kind: TypeKind::Array {
            element: Box::new(element),
            len: count,
        },
    })
}

fn parse_paren_or_fn_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let open = session.cursor.advance();
    if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        if session
            .cursor
            .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
        {
            let ret = parse_type(session)?;
            let id = session.next_node()?;
            return Ok(TypeWrap {
                id,
                span: join_span(open.span, ret.span),
                kind: TypeKind::Fn {
                    params: Vec::new(),
                    ret: Box::new(ret),
                },
            });
        }
        return missing_type(session, open.span);
    }

    let mut params = vec![parse_type(session)?];
    while session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
    {
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RParen))
        {
            break;
        }
        params.push(parse_type(session)?);
    }
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        missing_close(session);
    }
    if !session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
    {
        if params.len() == 1 {
            return Ok(params.pop().expect("checked len"));
        }
        let end = params.last().map(|ty| ty.span).unwrap_or_default();
        let id = session.next_node()?;
        return Ok(TypeWrap {
            id,
            span: join_span(open.span, end),
            kind: TypeKind::Tuple(params),
        });
    }

    let ret = parse_type(session)?;
    let end = ret.span;
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(open.span, end),
        kind: TypeKind::Fn {
            params,
            ret: Box::new(ret),
        },
    })
}

fn at_type_args_close(session: &ParseSession<'_>) -> bool {
    matches!(
        session.cursor.peek().kind,
        TokenKind::Operator(OperatorKind::Gt)
    )
}

fn consume_type_args_close(session: &mut ParseSession<'_>) {
    if at_type_args_close(session) {
        session.cursor.advance();
    } else if matches!(
        session.cursor.peek().kind,
        TokenKind::Operator(OperatorKind::Shr)
            | TokenKind::Operator(OperatorKind::GtEq)
            | TokenKind::Operator(OperatorKind::ShrEq)
    ) {
        if session.cursor.split_type_close() {
            session.cursor.advance();
        }
    } else {
        missing_close(session);
    }
}

fn missing_close(session: &mut ParseSession<'_>) {
    let span = session.cursor.peek().span;
    session.emit_unique(DiagCode::ExpectedToken, span, || "expected '>'".to_string());
}

fn missing_type(session: &mut ParseSession<'_>, span: Span) -> Result<TypeWrap, ParserError> {
    let token = session.cursor.advance();
    let id = session.next_node()?;
    let diag = session.emit_unique(DiagCode::UnexpectedToken, token.span, || {
        format!("expected type, found {}", token.kind)
    });
    Ok(TypeWrap {
        id,
        span,
        kind: TypeKind::Error(ast::syntax::node::ErrorNode {
            id,
            span: token.span,
            err: diag,
        }),
    })
}

pub(crate) fn join_span(start: Span, end: Span) -> Span {
    Span::new(start.start.min(end.start), start.end.max(end.end))
}
