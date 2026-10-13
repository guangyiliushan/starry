//! Pattern parser：冻结 EBNF 的 Pattern 空间。

use ast::syntax::node::{DiagCode, ErrorNode, Span};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::token::{HardKeyword, OperatorKind, PunctuationKind, TokenKind};

use crate::ty::{join_span, parse_type};
use crate::{ParseSession, ParserError};

pub(crate) fn parse_pattern(session: &mut ParseSession<'_>) -> Result<PatWrap, ParserError> {
    let first = parse_primary_pattern(session)?;
    let inclusive = if session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::DotDot))
    {
        false
    } else if session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::DotDotEq))
    {
        true
    } else {
        if !session
            .cursor
            .check(&TokenKind::Operator(OperatorKind::BitOr))
        {
            return Ok(first);
        }
        let mut alternatives = vec![first];
        while session
            .cursor
            .bump_if(&TokenKind::Operator(OperatorKind::BitOr))
        {
            alternatives.push(parse_primary_pattern(session)?);
        }
        let span = join_span(
            alternatives.first().expect("non-empty").span,
            alternatives.last().expect("non-empty").span,
        );
        return Ok(PatWrap::new(
            session.next_node()?,
            span,
            PatKind::Or(alternatives),
        ));
    };
    session.cursor.advance();
    let hi = parse_primary_pattern(session)?;
    let span = join_span(first.span, hi.span);
    Ok(PatWrap::new(
        session.next_node()?,
        span,
        PatKind::Range {
            lo: Box::new(first),
            hi: Box::new(hi),
            inclusive,
        },
    ))
}

fn parse_primary_pattern(session: &mut ParseSession<'_>) -> Result<PatWrap, ParserError> {
    match session.try_with_depth(|session| {
        let start = session.cursor.peek().span;
        match session.cursor.peek().kind {
            TokenKind::Keyword(HardKeyword::Mut) => {
                session.cursor.advance();
                let name_token = session.cursor.advance();
                if name_token.kind != TokenKind::Identifier {
                    let id = session.next_node()?;
                    let diag =
                        session.emit_unique(DiagCode::ExpectedToken, name_token.span, || {
                            "expected identifier after mut".to_string()
                        });
                    return Ok(PatWrap::new(
                        id,
                        join_span(start, name_token.span),
                        PatKind::Error(ErrorNode {
                            id,
                            span: name_token.span,
                            err: diag,
                        }),
                    ));
                }
                let name = session.interner.intern(session.token_text(&name_token));
                let ty = if session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
                {
                    Some(Box::new(parse_type(session)?))
                } else {
                    None
                };
                let end = ty.as_ref().map(|ty| ty.span).unwrap_or(name_token.span);
                Ok(PatWrap::new(
                    session.next_node()?,
                    join_span(start, end),
                    PatKind::Binding {
                        mutable: true,
                        name,
                        ty,
                    },
                ))
            }
            TokenKind::Keyword(HardKeyword::True)
            | TokenKind::Keyword(HardKeyword::False)
            | TokenKind::Keyword(HardKeyword::Null) => {
                let token = session.cursor.advance();
                let kind = match token.kind {
                    TokenKind::Keyword(HardKeyword::True) => PatKind::Bool(true),
                    TokenKind::Keyword(HardKeyword::False) => PatKind::Bool(false),
                    _ => PatKind::Null,
                };
                Ok(PatWrap::new(session.next_node()?, token.span, kind))
            }
            TokenKind::Literal(kind) => {
                let token = session.cursor.advance();
                Ok(PatWrap::new(
                    session.next_node()?,
                    token.span,
                    PatKind::Literal(kind),
                ))
            }
            TokenKind::Punctuation(PunctuationKind::LParen) => {
                let open = session.cursor.advance();
                if session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
                {
                    let id = session.next_node()?;
                    return Ok(PatWrap::new(
                        id,
                        join_span(open.span, Span::new(open.span.end, open.span.end)),
                        PatKind::Tuple(Vec::new()),
                    ));
                }
                let mut patterns = vec![parse_pattern(session)?];
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
                    patterns.push(parse_pattern(session)?);
                }
                let close = session.cursor.peek().span;
                session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen));
                Ok(PatWrap::new(
                    session.next_node()?,
                    join_span(open.span, close),
                    PatKind::Tuple(patterns),
                ))
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => {
                session.cursor.advance();
                let mut elements = Vec::new();
                let mut rest = false;
                while !session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBracket))
                {
                    if session
                        .cursor
                        .check(&TokenKind::Operator(OperatorKind::DotDot))
                    {
                        session.cursor.advance();
                        rest = true;
                        break;
                    }
                    elements.push(parse_pattern(session)?);
                    if !session
                        .cursor
                        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
                    {
                        break;
                    }
                }
                let close = session.cursor.peek().span;
                session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::RBracket));
                Ok(PatWrap::new(
                    session.next_node()?,
                    join_span(start, close),
                    PatKind::Array { elements, rest },
                ))
            }
            _ => {
                let token = session.cursor.advance();
                if token.kind == TokenKind::Identifier && session.token_text(&token) == "_" {
                    return Ok(PatWrap::new(
                        session.next_node()?,
                        token.span,
                        PatKind::Wildcard,
                    ));
                }
                if token.kind == TokenKind::Identifier {
                    let name = session.interner.intern(session.token_text(&token));
                    let kind = match session.cursor.peek().kind {
                        TokenKind::Punctuation(PunctuationKind::Colon) => {
                            session.cursor.advance();
                            let ty = parse_type(session)?;
                            let end = ty.span;
                            (
                                PatKind::Binding {
                                    mutable: false,
                                    name,
                                    ty: Some(Box::new(ty)),
                                },
                                end,
                            )
                        }
                        TokenKind::Punctuation(PunctuationKind::LBrace) => {
                            session.cursor.advance();
                            let mut fields = Vec::new();
                            while !session
                                .cursor
                                .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                            {
                                let field_name_token = session.cursor.advance();
                                let field_name = session
                                    .interner
                                    .intern(session.token_text(&field_name_token));
                                let field = if session
                                    .cursor
                                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
                                {
                                    parse_pattern(session)?
                                } else {
                                    PatWrap::new(
                                        session.next_node()?,
                                        field_name_token.span,
                                        PatKind::Binding {
                                            mutable: false,
                                            name: field_name,
                                            ty: None,
                                        },
                                    )
                                };
                                fields.push((field_name, field));
                                if !session
                                    .cursor
                                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
                                {
                                    break;
                                }
                            }
                            let close = session.cursor.peek().span;
                            session
                                .cursor
                                .bump_if(&TokenKind::Punctuation(PunctuationKind::RBrace));
                            (
                                PatKind::Struct {
                                    path: ast::syntax::node::Path::single(name, token.span),
                                    fields,
                                },
                                close,
                            )
                        }
                        TokenKind::Punctuation(PunctuationKind::LParen) => {
                            session.cursor.advance();
                            let mut args = Vec::new();
                            while !session
                                .cursor
                                .check(&TokenKind::Punctuation(PunctuationKind::RParen))
                            {
                                args.push(parse_pattern(session)?);
                                if !session
                                    .cursor
                                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
                                {
                                    break;
                                }
                            }
                            let close = session.cursor.peek().span;
                            session
                                .cursor
                                .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen));
                            (
                                PatKind::Constructor {
                                    path: ast::syntax::node::Path::single(name, token.span),
                                    args,
                                },
                                close,
                            )
                        }
                        _ => (
                            PatKind::Binding {
                                mutable: false,
                                name,
                                ty: None,
                            },
                            token.span,
                        ),
                    };
                    let (kind, end) = kind;
                    return Ok(PatWrap::new(
                        session.next_node()?,
                        join_span(token.span, end),
                        kind,
                    ));
                }
                let id = session.next_node()?;
                let diag = session.emit_unique(DiagCode::UnexpectedToken, token.span, || {
                    format!("expected pattern, found {}", token.kind)
                });
                Ok(PatWrap::new(
                    id,
                    token.span,
                    PatKind::Error(ErrorNode {
                        id,
                        span: token.span,
                        err: diag,
                    }),
                ))
            }
        }
    })? {
        Some(pattern) => Ok(pattern),
        None => {
            let token = session.cursor.advance();
            let id = session.next_node()?;
            let diag = session.emit_unique(DiagCode::DepthLimitExceeded, token.span, || {
                "pattern nesting exceeds 100".to_string()
            });
            Ok(PatWrap::new(
                id,
                token.span,
                PatKind::Error(ErrorNode {
                    id,
                    span: token.span,
                    err: diag,
                }),
            ))
        }
    }
}
