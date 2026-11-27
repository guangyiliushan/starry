//! Pattern parser：冻结 EBNF 的 Pattern 空间。

use ast::syntax::node::{DiagCode, ErrorNode, Span};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::token::{HardKeyword, OperatorKind, PunctuationKind, TokenKind};

use crate::ty::{join_span, parse_no_arrow_type, parse_type};
use crate::{MAX_PARSE_DEPTH, ParseSession, ParserError};

pub(crate) fn parse_pattern(session: &mut ParseSession<'_>) -> Result<PatWrap, ParserError> {
    parse_pattern_with(session, true, true)
}

/// An `is` operand is always followed by a mandatory statement block, so `{`
/// belongs to the block, never to a struct pattern: struct tails are
/// suppressed (resolves the `if e is A { }` ambiguity; see parser-notes).
pub(crate) fn parse_is_operand_pattern(
    session: &mut ParseSession<'_>,
) -> Result<PatWrap, ParserError> {
    parse_pattern_with(session, true, false)
}

/// A `when` branch is `WhenPattern "->" body`, so a top-level binding annotation
/// such as `a: int -> 1` is ambiguous with `FunctionType := UnionType ["->" T]`
/// (`int -> 1` reads as a function type). In this position only, the annotation
/// is a no-arrow type; nested annotations stay parenthesisable as usual.
pub(crate) fn parse_when_branch_pattern(
    session: &mut ParseSession<'_>,
) -> Result<PatWrap, ParserError> {
    parse_pattern_with(session, false, true)
}

fn parse_pattern_with(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
    struct_tail: bool,
) -> Result<PatWrap, ParserError> {
    let first = parse_primary_pattern(session, arrow_types, struct_tail)?;
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
            alternatives.push(parse_primary_pattern(session, arrow_types, struct_tail)?);
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
    let hi = parse_primary_pattern(session, arrow_types, struct_tail)?;
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

fn parse_ctor_pattern_tail(
    session: &mut ParseSession<'_>,
    path: ast::syntax::node::Path,
) -> Result<(PatKind, Span), ParserError> {
    session.cursor.advance(); // (
    let mut args = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        args.push(parse_pattern_with(session, true, true)?);
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
    Ok((PatKind::Constructor { path, args }, close))
}

fn parse_struct_pattern_tail(
    session: &mut ParseSession<'_>,
    path: ast::syntax::node::Path,
) -> Result<(PatKind, Span), ParserError> {
    session.cursor.advance(); // {
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
            parse_pattern_with(session, true, true)?
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
    Ok((PatKind::Struct { path, fields }, close))
}

/// `Type` when `arrow_types`, otherwise a no-arrow type.
fn parse_pattern_annotation(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<ast::syntax::ty::TypeWrap, ParserError> {
    if arrow_types {
        parse_type(session)
    } else {
        parse_no_arrow_type(session)
    }
}

/// Shared element list for tuple / array patterns: elements plus an optional
/// `..` rest slot. The rest may appear once, in any position (Rust: "may only
/// appear once as one of the elements").
fn parse_pattern_elements(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
    struct_tail: bool,
    close: PunctuationKind,
) -> Result<(Vec<PatWrap>, Option<usize>, Span), ParserError> {
    let mut elements = Vec::new();
    let mut rest: Option<usize> = None;
    while !session.cursor.check(&TokenKind::Punctuation(close)) {
        if session
            .cursor
            .check(&TokenKind::Operator(OperatorKind::DotDot))
        {
            let token = session.cursor.advance();
            if rest.is_some() {
                session.emit_unique(DiagCode::DuplicateRestPattern, token.span, || {
                    "rest pattern `..` may appear only once".to_string()
                });
            } else {
                rest = Some(elements.len());
            }
        } else {
            elements.push(parse_pattern_with(session, arrow_types, struct_tail)?);
        }
        if !session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            break;
        }
    }
    let end = session.cursor.peek().span;
    Ok((elements, rest, end))
}

fn parse_primary_pattern(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
    struct_tail: bool,
) -> Result<PatWrap, ParserError> {
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
                    Some(Box::new(parse_pattern_annotation(session, arrow_types)?))
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
                let (elements, rest, close) =
                    parse_pattern_elements(session, true, struct_tail, PunctuationKind::RParen)?;
                session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen));
                Ok(PatWrap::new(
                    session.next_node()?,
                    join_span(open.span, close),
                    PatKind::Tuple { elements, rest },
                ))
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => {
                session.cursor.advance();
                let (elements, rest, close) =
                    parse_pattern_elements(session, true, struct_tail, PunctuationKind::RBracket)?;
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
                    let (kind, end) = match session.cursor.peek().kind {
                        TokenKind::Punctuation(PunctuationKind::Colon) => {
                            session.cursor.advance();
                            let ty = parse_pattern_annotation(session, arrow_types)?;
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
                        TokenKind::Punctuation(PunctuationKind::LBrace) if struct_tail => {
                            parse_struct_pattern_tail(
                                session,
                                ast::syntax::node::Path::single(name, token.span),
                            )?
                        }
                        TokenKind::Punctuation(PunctuationKind::LParen) => parse_ctor_pattern_tail(
                            session,
                            ast::syntax::node::Path::single(name, token.span),
                        )?,
                        TokenKind::Punctuation(PunctuationKind::Dot) => {
                            // QualifiedName 构造器/结构体模式：E.A / E.A(x) / S{ f: p }
                            let mut segments = vec![ast::syntax::node::PathSegment {
                                name,
                                span: token.span,
                            }];
                            let mut end = token.span;
                            while session
                                .cursor
                                .bump_if(&TokenKind::Punctuation(PunctuationKind::Dot))
                            {
                                let seg = session.cursor.advance();
                                if seg.kind != TokenKind::Identifier {
                                    let id = session.next_node()?;
                                    let diag = session.emit_unique(
                                        DiagCode::ExpectedToken,
                                        seg.span,
                                        || "expected identifier after '.'".to_string(),
                                    );
                                    return Ok(PatWrap::new(
                                        id,
                                        join_span(start, seg.span),
                                        PatKind::Error(ErrorNode {
                                            id,
                                            span: seg.span,
                                            err: diag,
                                        }),
                                    ));
                                }
                                let sname = session.interner.intern(session.token_text(&seg));
                                segments.push(ast::syntax::node::PathSegment {
                                    name: sname,
                                    span: seg.span,
                                });
                                end = seg.span;
                            }
                            let path = ast::syntax::node::Path { segments };
                            match session.cursor.peek().kind {
                                TokenKind::Punctuation(PunctuationKind::LParen) => {
                                    parse_ctor_pattern_tail(session, path)?
                                }
                                TokenKind::Punctuation(PunctuationKind::LBrace) if struct_tail => {
                                    parse_struct_pattern_tail(session, path)?
                                }
                                _ => (
                                    PatKind::Constructor {
                                        path,
                                        args: Vec::new(),
                                    },
                                    end,
                                ),
                            }
                        }
                        _ if !arrow_types => (
                            // when 分支顶层裸名：构造器候选（变体依主语类型在 sem 精化）
                            PatKind::Constructor {
                                path: ast::syntax::node::Path::single(name, token.span),
                                args: Vec::new(),
                            },
                            token.span,
                        ),
                        _ => (
                            PatKind::Binding {
                                mutable: false,
                                name,
                                ty: None,
                            },
                            token.span,
                        ),
                    };
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
                format!("pattern nesting exceeds {MAX_PARSE_DEPTH}")
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
