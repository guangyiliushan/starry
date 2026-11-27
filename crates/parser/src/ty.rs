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
    let name = if session.cursor.check(&TokenKind::Identifier) {
        let token = session.cursor.advance();
        ast::syntax::ty::TypeParamName::Valid(session.interner.intern(session.token_text(&token)))
    } else {
        let token = session.cursor.advance();
        let id = session.next_node()?;
        let diag = session.emit_unique(DiagCode::ExpectedToken, token.span, || {
            format!("expected type parameter name, found {}", token.kind)
        });
        ast::syntax::ty::TypeParamName::Error(ast::syntax::node::ErrorNode {
            id,
            span: token.span,
            err: diag,
        })
    };
    let bounds = if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        parse_bounds(session)?
    } else {
        Vec::new()
    };
    let default = if session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Eq))
    {
        Some(Box::new(parse_type(session)?))
    } else {
        None
    };
    let end = session.cursor.previous_span();
    Ok(ast::syntax::ty::TypeParam {
        id: session.next_node()?,
        span: join_span(start, end),
        variance,
        name,
        bounds,
        default,
    })
}

/// `Bound := Type { "&" Type }` — an intersection bound list, shared by
/// `TypeParameter` inline bounds and `WhereConstraint`.
pub(crate) fn parse_bounds(session: &mut ParseSession<'_>) -> Result<Vec<TypeWrap>, ParserError> {
    let mut bounds = vec![parse_type(session)?];
    while session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::BitAnd))
    {
        bounds.push(parse_type(session)?);
    }
    Ok(bounds)
}

fn parse_variance(_session: &mut ParseSession<'_>) -> Option<ast::syntax::ty::Variance> {
    // D-014: variance is deferred (Optional). in stays membership-only;
    // future variance markers will use +/- (see keyword-capability-plan).
    None
}

pub(crate) fn parse_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    parse_type_with(session, true)
}

/// A type with no `->` at its own top level: `FunctionType := UnionType ["->" T]`
/// and the `( … ) -> …` form are both disallowed there. Used for a `when` branch's
/// top-level binding annotation, where `a: int -> 1` would otherwise read the
/// PointerType := "*" ( "const" | "mut" ) PrimaryType（D-007 / M2）。
/// bare *T 诊断（ExpectedToken：期待 const/mut），恢复为 const 继续。
/// 可空指针写作 ?*const T（前缀）；`*const T?` 的 ? 归内层（parse_named_type
/// 的原子后缀环消费），即 *const T? = *const (?T)。
/// A type with no `->` at its own top level: `FunctionType := UnionType ["->" T]`
/// and the `( … ) -> …` form are both disallowed there. Used for a `when` branch's
/// top-level binding annotation, where `a: int -> 1` would otherwise read the
/// branch arrow as a function-type arrow. Nested types still accept `->`.
pub(crate) fn parse_no_arrow_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    parse_type_with(session, false)
}

fn parse_type_with(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<TypeWrap, ParserError> {
    match session.try_with_depth(|session| parse_type_inner(session, arrow_types))? {
        Some(ty) => Ok(ty),
        None => missing_type(session, session.cursor.peek().span),
    }
}

fn parse_type_inner(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<TypeWrap, ParserError> {
    let mut function = parse_union_type_with(session, arrow_types)?;
    while arrow_types
        && session
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

fn parse_union_type_with(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<TypeWrap, ParserError> {
    let first = parse_atomic_type(session, arrow_types)?;
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
        types.push(parse_atomic_type(session, arrow_types)?);
    }
    let end = types.last().map(|ty| ty.span).unwrap_or_default();
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(start, end),
        kind: TypeKind::Union(types),
    })
}

fn parse_atomic_type(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<TypeWrap, ParserError> {
    let token = *session.cursor.peek();
    if token.kind == TokenKind::Eof {
        return missing_type(session, token.span);
    }
    match token.kind {
        TokenKind::Punctuation(PunctuationKind::LParen) => {
            parse_paren_or_fn_type(session, arrow_types)
        }
        TokenKind::Punctuation(PunctuationKind::LBracket) => parse_array_type(session),
        TokenKind::Identifier => parse_named_type(session),
        // EBNF PointerType := "*" ( "const" | "mut" ) PrimaryType（D-007 / M2：
        // bare *T 诊断，不再静默当 const）
        TokenKind::Operator(OperatorKind::Star) => parse_pointer_type(session),
        // 前缀可空构造子 ?T（D-007 / M2）：复合类型的可空只用前缀（?[T; N]）
        TokenKind::Operator(OperatorKind::Question) => parse_prefix_nullable(session),
        // 借用 &T / &mut T（D-007 / M2）
        TokenKind::Operator(OperatorKind::BitAnd) => parse_ref_type(session),
        TokenKind::Keyword(ast::token::HardKeyword::SelfType) => {
            let token = session.cursor.advance();
            // EBNF: PrimaryType := … | "Self" | … — no type arguments.
            if session.cursor.check(&TokenKind::Operator(OperatorKind::Lt)) {
                let args_span = session.cursor.peek().span;
                session.emit_unique(DiagCode::UnexpectedToken, args_span, || {
                    "Self takes no type arguments".to_string()
                });
                let _ = parse_type_args(session)?;
            }
            let id = session.next_node()?;
            Ok(TypeWrap {
                id,
                span: token.span,
                kind: TypeKind::SelfType,
            })
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
fn parse_pointer_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let star = session.cursor.advance();
    // const 是 modifier/slot word（词法 IDENT），mut 是硬关键字——分开判
    let mutable = match session.cursor.peek().kind {
        TokenKind::Identifier if session.token_text(session.cursor.peek()) == "const" => {
            session.cursor.advance();
            false
        }
        TokenKind::Keyword(ast::token::HardKeyword::Mut) => {
            session.cursor.advance();
            true
        }
        _ => {
            session.emit_unique(DiagCode::ExpectedToken, star.span, || {
                "expected `const` or `mut` after `*` (bare `*T` is not allowed)".to_string()
            });
            false
        }
    };
    let inner = match session.try_with_depth(|s| parse_atomic_type(s, true))? {
        Some(ty) => ty,
        None => return missing_type(session, session.cursor.peek().span),
    };
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(star.span, inner.span),
        kind: TypeKind::Pointer {
            mutable,
            inner: Box::new(inner),
        },
    })
}

/// 前缀可空构造子 `?T`（D-007 / M2）：与 *const/& 同族的前缀链。
/// 复合类型的可空只走前缀（?[T; N]、?(T, U)），原子后缀 T? 是糖（见
/// parse_named_suffix 的 desugar）。
fn parse_prefix_nullable(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let q = session.cursor.advance();
    let inner = match session.try_with_depth(|s| parse_atomic_type(s, true))? {
        Some(ty) => ty,
        None => return missing_type(session, session.cursor.peek().span),
    };
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(q.span, inner.span),
        kind: TypeKind::Nullable(Box::new(inner)),
    })
}

/// 借用 `&T` / `&mut T`（D-007 / M2）：mut 是硬关键字；正交前缀可组合
///（?&T、&?T、&&T 按前缀链自然嵌套）。
fn parse_ref_type(session: &mut ParseSession<'_>) -> Result<TypeWrap, ParserError> {
    let amp = session.cursor.advance();
    let mutable = matches!(
        session.cursor.peek().kind,
        TokenKind::Keyword(ast::token::HardKeyword::Mut)
    );
    if mutable {
        session.cursor.advance();
    }
    let inner = match session.try_with_depth(|s| parse_atomic_type(s, true))? {
        Some(ty) => ty,
        None => return missing_type(session, session.cursor.peek().span),
    };
    let id = session.next_node()?;
    Ok(TypeWrap {
        id,
        span: join_span(amp.span, inner.span),
        kind: TypeKind::Ref {
            mutable,
            inner: Box::new(inner),
        },
    })
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
            .unwrap_or_else(|_| {
                session.emit_unique(DiagCode::IntegerOutOfRange, count_token.span, || {
                    "array length exceeds u32".to_string()
                });
                0
            }),
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
    // D-007 / M2：复合类型可空只走前缀 [T; N]? 拒绝并提示 ?[T; N]
    if session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::Question))
    {
        let q = session.cursor.advance();
        session.emit_unique(DiagCode::UnexpectedToken, q.span, || {
            "nullable of array types uses the prefix form `?[T; N]`".to_string()
        });
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

fn parse_paren_or_fn_type(
    session: &mut ParseSession<'_>,
    arrow_types: bool,
) -> Result<TypeWrap, ParserError> {
    let open = session.cursor.advance();
    if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        if arrow_types
            && session
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
        // EBNF: UnitType := "(" ")" — a unit type, not a missing one.
        return Ok(TypeWrap {
            id: session.next_node()?,
            span: join_span(open.span, Span::new(open.span.end, open.span.end)),
            kind: TypeKind::Unit,
        });
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
    let takes_arrow = arrow_types
        && session
            .cursor
            .bump_if(&TokenKind::Operator(OperatorKind::Arrow));
    if !takes_arrow {
        if params.len() == 1 {
            return Ok(params.pop().expect("checked len"));
        }
        // D-007 / M2：(T, U)? 拒绝并提示 ?(T, U)
        if session
            .cursor
            .check(&TokenKind::Operator(OperatorKind::Question))
        {
            let q = session.cursor.advance();
            session.emit_unique(DiagCode::UnexpectedToken, q.span, || {
                "nullable of tuple types uses the prefix form `?(T, U)`".to_string()
            });
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
