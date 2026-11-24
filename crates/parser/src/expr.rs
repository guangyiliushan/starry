//! Pratt expression parser。

use crate::cursor::TokenCursor;
use ast::interner::Interner;
use ast::syntax::expr::{
    ActorConstruction, Block, CallArg, CastKind, ExprKind, ExprStmt, ExprWrap, LambdaParams,
    SpawnExpr, SpawnKind, SpawnScheduler, SuperSuffix, TokenTree, WhenBody, WhenBranch,
    WhenPattern,
};
use ast::syntax::node::{DiagCode, DiagCollector, ErrorNode, NodeIdGen, Span};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::token::{HardKeyword, LiteralKind, OperatorKind, PunctuationKind, Token, TokenKind};

use crate::pat::parse_pattern;
use crate::ty::join_span;
use crate::{MAX_PARSE_DEPTH, ParseSession, ParserError};

pub(crate) fn parse_expr(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    parse_bp(session, 0)
}

fn parse_bp(session: &mut ParseSession<'_>, min_bp: u8) -> Result<ExprWrap, ParserError> {
    match session.try_with_depth(|session| parse_bp_inner(session, min_bp))? {
        Some(expr) => Ok(expr),
        None => depth_error(session),
    }
}

fn parse_bp_inner(session: &mut ParseSession<'_>, min_bp: u8) -> Result<ExprWrap, ParserError> {
    let mut lhs = parse_unary(session)?;
    loop {
        if session.cursor.check(&TokenKind::Nl) && newline_continues_expr(session) {
            session.cursor.skip_nl();
        }
        // `is` / `!is` and `in` / `!in` are symmetric operator pairs (Kotlin:
        // `isOperator : 'is' | NOT_IS`, `inOperator : 'in' | NOT_IN`).
        let negated_is = session
            .cursor
            .check(&TokenKind::Operator(OperatorKind::Bang))
            && session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Keyword(HardKeyword::Is));
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::Is)) || negated_is {
            if negated_is {
                session.cursor.advance();
            }
            session.cursor.advance();
            // is 操作数后必有语句块（if/while 的 then-body 强制）：
            // `{` 归块不归结构体模式，禁止结构体尾（parser-notes 有记录）
            let pattern = crate::pat::parse_is_operand_pattern(session)?;
            let span = join_span(lhs.span, pattern.span);
            let id = session.next_node()?;
            lhs = ExprWrap::new(
                id,
                span,
                ExprKind::Is {
                    lhs: Box::new(lhs),
                    pattern: Box::new(pattern),
                    negated: negated_is,
                },
            );
            continue;
        }
        let negated_in = session
            .cursor
            .check(&TokenKind::Operator(OperatorKind::Bang))
            && session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Keyword(HardKeyword::In));
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::In)) || negated_in {
            if negated_in {
                session.cursor.advance();
            }
            session.cursor.advance();
            // EBNF：右操作数是 RangeExpr——以 range 绑定幂（17）为下限解析，
            // 保持左结合链不被右操作数吞吃
            let rhs = parse_bp(session, 17)?;
            let span = join_span(lhs.span, rhs.span);
            let id = session.next_node()?;
            lhs = ExprWrap::new(
                id,
                span,
                ExprKind::In {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    negated: negated_in,
                },
            );
            continue;
        }
        let Some(operator) = infix_operator(session) else {
            break;
        };
        let Some((left_bp, right_bp)) = binding_power(&operator) else {
            break;
        };
        if left_bp < min_bp {
            break;
        }
        session.cursor.advance();
        session.cursor.skip_nl();
        let rhs = parse_bp(session, right_bp)?;
        let span = join_span(lhs.span, rhs.span);
        let id = session.next_node()?;
        lhs = ExprWrap {
            id,
            span,
            kind: ExprKind::Binary {
                op: operator,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        };
    }
    Ok(lhs)
}

fn infix_operator(session: &ParseSession<'_>) -> Option<OperatorKind> {
    match session.cursor.peek().kind {
        TokenKind::Operator(op) if binding_power(&op).is_some() => Some(op),
        _ => None,
    }
}

fn binding_power(op: &OperatorKind) -> Option<(u8, u8)> {
    match op {
        OperatorKind::QuestionQuestion => Some((1, 2)),
        OperatorKind::Or => Some((3, 4)),
        OperatorKind::And => Some((5, 6)),
        OperatorKind::BitOr => Some((7, 8)),
        OperatorKind::BitXor => Some((9, 10)),
        OperatorKind::BitAnd => Some((11, 12)),
        OperatorKind::EqEq | OperatorKind::BangEq => Some((13, 14)),
        OperatorKind::Lt | OperatorKind::LtEq | OperatorKind::Gt | OperatorKind::GtEq => {
            Some((15, 16))
        }
        OperatorKind::DotDot | OperatorKind::DotDotEq => Some((17, 18)),
        OperatorKind::Shl | OperatorKind::Shr => Some((19, 20)),
        OperatorKind::Plus | OperatorKind::Minus => Some((21, 22)),
        // M1 wrapping 算术：与受检对应算子同优先级
        OperatorKind::PlusPercent | OperatorKind::MinusPercent => Some((21, 22)),
        OperatorKind::Star | OperatorKind::Slash | OperatorKind::Percent => Some((23, 24)),
        OperatorKind::StarPercent => Some((23, 24)),
        _ => None,
    }
}

/// 换行续行判定：下一有效 token 是二元专属运算符或 `.` 时，换行不终止表达式。
fn newline_continues_expr(session: &ParseSession<'_>) -> bool {
    let mut offset = 0usize;
    loop {
        match session.cursor.peek_ahead(offset) {
            Some(token) if token.kind == TokenKind::Nl => offset += 1,
            Some(token) => {
                return match &token.kind {
                    TokenKind::Operator(op) => {
                        binding_power(op).is_some()
                            && !matches!(
                                op,
                                OperatorKind::Bang
                                    | OperatorKind::Minus
                                    | OperatorKind::Star
                                    | OperatorKind::BitAnd
                                    | OperatorKind::BitNot
                            )
                    }
                    TokenKind::Punctuation(PunctuationKind::Dot) => true,
                    _ => false,
                };
            }
            None => return false,
        }
    }
}

fn parse_unary(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    match session.try_with_depth(parse_unary_inner)? {
        Some(expr) => Ok(expr),
        None => depth_error(session),
    }
}

fn parse_unary_inner(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let token = *session.cursor.peek();
    let unary = match token.kind {
        TokenKind::Operator(OperatorKind::Bang) => Some(OperatorKind::Bang),
        TokenKind::Operator(OperatorKind::Minus) => Some(OperatorKind::Minus),
        TokenKind::Operator(OperatorKind::BitNot) => Some(OperatorKind::BitNot),
        // M2：&x / &mut x 取址（T1）；*p 解引。不复用 Unary——借用/解引
        // 有独立语义（sem 借用检查 / irc 地址与指针读写）
        TokenKind::Operator(OperatorKind::BitAnd) => {
            session.cursor.advance();
            let mutable = matches!(
                session.cursor.peek().kind,
                TokenKind::Keyword(HardKeyword::Mut)
            );
            if mutable {
                session.cursor.advance();
            }
            let operand = parse_unary(session)?;
            let span = join_span(token.span, operand.span);
            let id = session.next_node()?;
            return Ok(ExprWrap {
                id,
                span,
                kind: ExprKind::AddrOf {
                    mutable,
                    expr: Box::new(operand),
                },
            });
        }
        TokenKind::Operator(OperatorKind::Star) => {
            session.cursor.advance();
            let operand = parse_unary(session)?;
            let span = join_span(token.span, operand.span);
            let id = session.next_node()?;
            return Ok(ExprWrap {
                id,
                span,
                kind: ExprKind::Deref {
                    expr: Box::new(operand),
                },
            });
        }
        TokenKind::Keyword(HardKeyword::Await) => {
            session.cursor.advance();
            let operand = parse_unary(session)?;
            let span = join_span(token.span, operand.span);
            let id = session.next_node()?;
            return Ok(ExprWrap {
                id,
                span,
                kind: ExprKind::Await {
                    expr: Box::new(operand),
                },
            });
        }
        _ => None,
    };
    if let Some(op) = unary {
        session.cursor.advance();
        let operand = parse_unary(session)?;
        let span = join_span(token.span, operand.span);
        let id = session.next_node()?;
        return Ok(ExprWrap {
            id,
            span,
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
        });
    }
    parse_postfix(session)
}

fn parse_postfix(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let expr = parse_primary(session)?;
    // `f<T>(...)` and `f < T` share the `<` prefix. Try the call suffix
    // speculatively so a failed type parse cannot leak ghost diagnostics.
    finish_postfix(session, expr)
}

fn finish_postfix(
    session: &mut ParseSession<'_>,
    mut expr: ExprWrap,
) -> Result<ExprWrap, ParserError> {
    loop {
        if session.cursor.check(&TokenKind::Nl) && newline_continues_expr(session) {
            session.cursor.skip_nl();
        }
        if session.cursor.check(&TokenKind::Operator(OperatorKind::Lt)) {
            let checkpoint = session.checkpoint();
            let type_args = match crate::ty::parse_type_args(session) {
                Ok(type_args) => type_args,
                Err(error) => {
                    session.rollback(&checkpoint);
                    return Err(error);
                }
            };
            if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LParen))
            {
                let (args, close) = parse_call_args(session)?;
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span: join_span(expr.span, close),
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        type_args,
                        args,
                    },
                };
                continue;
            }
            session.rollback(&checkpoint);
        }
        match session.cursor.peek().kind {
            TokenKind::Punctuation(PunctuationKind::Dot) => {
                session.cursor.advance();
                let name = parse_identifier(session);
                let span = join_span(expr.span, name.1);
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::Member {
                        base: Box::new(expr),
                        name: name.0,
                        safe: false,
                    },
                };
            }
            TokenKind::Operator(OperatorKind::QuestionDot) => {
                session.cursor.advance();
                let name = parse_identifier(session);
                let span = join_span(expr.span, name.1);
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::Member {
                        base: Box::new(expr),
                        name: name.0,
                        safe: true,
                    },
                };
            }
            TokenKind::Operator(OperatorKind::BangBang) => {
                session.cursor.advance();
                let span = expr.span;
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::ForceUnwrap {
                        expr: Box::new(expr),
                    },
                };
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => {
                session.cursor.advance();
                let index = parse_expr(session)?;
                let const_index = match &index.kind {
                    ExprKind::Literal(LiteralKind::Integer) => {
                        integer_literal_index(session, index.span)
                    }
                    _ => None,
                };
                let close = session.cursor.peek().span;
                if !session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::RBracket))
                {
                    expected(session, "']'", close);
                }
                let span = join_span(expr.span, close);
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                        const_index,
                    },
                };
            }
            TokenKind::Punctuation(PunctuationKind::LParen) => {
                let (args, end) = parse_call_args(session)?;
                let span = join_span(expr.span, end);
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        type_args: Vec::new(),
                        args,
                    },
                };
            }
            TokenKind::Keyword(HardKeyword::As)
            | TokenKind::Operator(OperatorKind::AsQuestion)
            | TokenKind::Operator(OperatorKind::AsBang) => {
                let cast_kind = match session.cursor.advance().kind {
                    TokenKind::Keyword(HardKeyword::As) => CastKind::As,
                    TokenKind::Operator(OperatorKind::AsQuestion) => CastKind::AsQuestion,
                    _ => CastKind::AsBang,
                };
                let ty = crate::ty::parse_type(session)?;
                let span = join_span(expr.span, ty.span);
                let id = session.next_node()?;
                expr = ExprWrap {
                    id,
                    span,
                    kind: ExprKind::Cast {
                        expr: Box::new(expr),
                        ty: Box::new(ty),
                        kind: cast_kind,
                    },
                };
            }
            _ => break,
        }
    }
    Ok(expr)
}

fn parse_primary(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let token = *session.cursor.peek();
    let span = token.span;
    let id = session.next_node()?;
    match token.kind {
        TokenKind::Literal(LiteralKind::String) => {
            session.cursor.advance();
            parse_string_literal(session, token, id, span)
        }
        TokenKind::Literal(kind) => {
            session.cursor.advance();
            Ok(ExprWrap::new(id, span, ExprKind::Literal(kind)))
        }
        TokenKind::Identifier => {
            if session.token_text(&token) == "spawn" {
                return parse_spawn(session, span);
            }
            if session.token_text(&token) == "comptime" {
                session.cursor.advance();
                let block = parse_block(session)?;
                let end = block_tail_span(&block);
                return Ok(ExprWrap::new(
                    id,
                    join_span(span, end),
                    ExprKind::Comptime {
                        block: Box::new(block),
                    },
                ));
            }
            if session.token_text(&token) == "move"
                && session.cursor.peek_ahead(1).is_some_and(|next| {
                    next.kind == TokenKind::Punctuation(PunctuationKind::LBrace)
                })
            {
                session.cursor.advance();
                return parse_lambda(session, span);
            }
            if session.token_text(&token) == "proceed" {
                session.cursor.advance();
                return Ok(ExprWrap::new(id, span, ExprKind::Proceed));
            }
            if session.token_text(&token) == "concurrent"
                && session.cursor.peek_ahead(1).is_some_and(|next| {
                    next.kind == TokenKind::Punctuation(PunctuationKind::LBrace)
                })
            {
                session.cursor.advance();
                let block = parse_block(session)?;
                let end = block_tail_span(&block);
                return Ok(ExprWrap::new(
                    id,
                    join_span(span, end),
                    ExprKind::Concurrent {
                        block: Box::new(block),
                    },
                ));
            }
            let symbol = session.interner.intern(session.token_text(&token));
            session.cursor.advance();
            let mut expr = ExprWrap::new(id, span, ExprKind::Ident(symbol));
            if session
                .cursor
                .check(&TokenKind::Operator(OperatorKind::Bang))
                && session.cursor.peek_ahead(1).is_some_and(|next| {
                    next.kind == TokenKind::Punctuation(PunctuationKind::LParen)
                })
            {
                session.cursor.advance();
                let tokens = parse_macro_tokens(session)?;
                expr.kind = ExprKind::MacroCall {
                    name: ast::syntax::node::Path::single(symbol, span),
                    tokens,
                };
            }
            Ok(expr)
        }
        TokenKind::Keyword(HardKeyword::True) => {
            session.cursor.advance();
            Ok(ExprWrap::new(id, span, ExprKind::Bool(true)))
        }
        TokenKind::Keyword(HardKeyword::False) => {
            session.cursor.advance();
            Ok(ExprWrap::new(id, span, ExprKind::Bool(false)))
        }
        TokenKind::Keyword(HardKeyword::Null) => {
            session.cursor.advance();
            Ok(ExprWrap::new(id, span, ExprKind::Null))
        }
        TokenKind::Keyword(HardKeyword::Self_) => {
            session.cursor.advance();
            let symbol = session.interner.intern("self");
            Ok(ExprWrap::new(id, span, ExprKind::Ident(symbol)))
        }
        // EBNF Primary := … | "Self" [ CallSuffix ] — the optional call suffix is
        // picked up by the postfix loop, so `Self` is just a primary here.
        TokenKind::Keyword(HardKeyword::SelfType) => {
            session.cursor.advance();
            let symbol = session.interner.intern("Self");
            Ok(ExprWrap::new(id, span, ExprKind::Ident(symbol)))
        }
        TokenKind::Keyword(HardKeyword::Super) => {
            session.cursor.advance();
            parse_super(session, id, span)
        }
        TokenKind::Keyword(HardKeyword::If) => parse_if(session),
        TokenKind::Keyword(HardKeyword::When) => parse_when(session),
        TokenKind::Keyword(HardKeyword::Loop) => {
            session.cursor.advance();
            let body = parse_block(session)?;
            let end = block_tail_span(&body);
            Ok(ExprWrap::new(
                id,
                join_span(span, end),
                ExprKind::Loop {
                    label: None,
                    body: Box::new(body),
                },
            ))
        }
        TokenKind::Keyword(HardKeyword::Async) => {
            // EBNF AsyncExpr := "async" Block：`{` 归块不归 lambda
            session.cursor.advance();
            let block = parse_block(session)?;
            let end = block_tail_span(&block);
            Ok(ExprWrap::new(
                id,
                join_span(span, end),
                ExprKind::Async {
                    block: Box::new(block),
                },
            ))
        }
        TokenKind::Punctuation(PunctuationKind::LParen) => parse_paren_expr(session),
        TokenKind::Punctuation(PunctuationKind::LBracket) => parse_array(session),
        TokenKind::Punctuation(PunctuationKind::LBrace) => parse_lambda(session, span),
        TokenKind::Keyword(HardKeyword::Unsafe) => {
            session.cursor.advance();
            let block = parse_block(session)?;
            let end = block_tail_span(&block);
            Ok(ExprWrap::new(
                id,
                join_span(span, end),
                ExprKind::Unsafe {
                    block: Box::new(block),
                },
            ))
        }
        _ => {
            session.cursor.advance();
            let diag = session.emit_unique(DiagCode::UnexpectedToken, span, || {
                format!("unexpected {}", token.kind)
            });
            Ok(ExprWrap::new(
                id,
                span,
                ExprKind::Error(ErrorNode {
                    id,
                    span,
                    err: diag,
                }),
            ))
        }
    }
}

fn parse_paren_expr(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let open = session.cursor.advance();
    if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        let id = session.next_node()?;
        return Ok(ExprWrap::new(
            id,
            join_span(open.span, Span::new(open.span.end, open.span.end)),
            ExprKind::Tuple(Vec::new()),
        ));
    }
    let first = parse_expr(session)?;
    if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
    {
        let mut items = vec![first];
        while !session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RParen))
        {
            items.push(parse_expr(session)?);
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
        let id = session.next_node()?;
        return Ok(ExprWrap::new(
            id,
            join_span(open.span, close),
            ExprKind::Tuple(items),
        ));
    }
    let close = session.cursor.peek().span;
    session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen));
    let id = session.next_node()?;
    Ok(ExprWrap::new(id, join_span(open.span, close), first.kind))
}

fn parse_array(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let open = session.cursor.advance();
    if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RBracket))
    {
        let close = session.cursor.advance();
        let id = session.next_node()?;
        return Ok(ExprWrap::new(
            id,
            join_span(open.span, close.span),
            ExprKind::Array(Vec::new()),
        ));
    }
    let first = parse_expr(session)?;
    if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon))
    {
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
                expected(session, "array length", count_token.span);
                0
            }
        };
        let close = session.cursor.peek().span;
        if !session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::RBracket))
        {
            expected(session, "']'", close);
        }
        let id = session.next_node()?;
        return Ok(ExprWrap::new(
            id,
            join_span(open.span, close),
            ExprKind::ArrayRepeat {
                value: Box::new(first),
                count,
            },
        ));
    }
    let mut items = vec![first];
    let comma_separated = session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma));
    while comma_separated
        && !session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBracket))
    {
        items.push(parse_expr(session)?);
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
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(open.span, close),
        ExprKind::Array(items),
    ))
}

fn parse_lambda(session: &mut ParseSession<'_>, open: Span) -> Result<ExprWrap, ParserError> {
    let mut scan_pos = 1usize;
    let mut depth = 1usize;
    let mut has_arrow = false;
    while let Some(token) = session.cursor.peek_ahead(scan_pos) {
        match token.kind {
            TokenKind::Punctuation(PunctuationKind::LParen)
            | TokenKind::Punctuation(PunctuationKind::LBracket) => depth += 1,
            TokenKind::Punctuation(PunctuationKind::RParen)
            | TokenKind::Punctuation(PunctuationKind::RBracket) => {
                depth = depth.saturating_sub(1);
            }
            TokenKind::Punctuation(PunctuationKind::RBrace) => break,
            TokenKind::Operator(OperatorKind::Arrow) if depth == 1 => {
                has_arrow = true;
                break;
            }
            _ => {}
        }
        scan_pos += 1;
    }

    session.cursor.advance(); // {
    let params = if !has_arrow {
        LambdaParams::It
    } else if session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::Arrow))
    {
        LambdaParams::Explicit(Vec::new())
    } else {
        let mut patterns = vec![parse_pattern(session)?];
        while session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            patterns.push(parse_pattern(session)?);
        }
        LambdaParams::Explicit(patterns)
    };
    let has_arrow = session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Arrow));
    let block = if has_arrow {
        // S2/P2-3b-3（矩阵 #6 字面嵌套）：`->` 后的 `{` 消歧——前瞻扫描
        // 配对 `}` 之前是否出现深度 1 的 `->`（括号/花括号/方括号深度
        // 感知）：是 = 嵌套 lambda 表达式（含显式参数注解），否 = 块。
        // 此前一律 parse_block，`{ (b: int) -> … }` 内层注解参数误归块
        //（UnexpectedToken ':'）。
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
            && body_brace_is_lambda(session)
        {
            let brace = session.cursor.peek().span;
            // 嵌套 lambda 以单尾表达式块形式作为 body（与 parse_if 的
            // else-if 包装同式，line 924 既有先例）
            let nested = parse_lambda(session, brace)?;
            // 外层 lambda 的闭合 '}' 由本分支消费（parse_block_after_open
            // 的对应职责转移至此）
            session.cursor.skip_nl();
            let close = session.cursor.peek().span;
            if !session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::RBrace))
            {
                expected(session, "'}'", close);
            }
            Block {
                stmts: Vec::new(),
                tail: Some(Box::new(nested)),
            }
        } else if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
        {
            eprintln!("[dbg] helper returned false -> parse_block");
            parse_block(session)?
        } else {
            parse_block_after_open(session)?.0
        }
    } else {
        parse_block_after_open(session)?.0
    };
    let end = block_tail_span(&block);
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(open, end),
        ExprKind::Lambda {
            params,
            body: Box::new(block),
        },
    ))
}

/// S2/P2-3b-3：lambda body 位 `{` 的嵌套 lambda 前瞻——从当前 `{` 起
/// 扫描（深度自 1 起）：配对 `}` 前出现深度 1 的 `->` = 嵌套 lambda
///（`{ (b: int) -> … }` 显式参数注解形式）；扫到配对 `}` 仍无 = 块
///（`{ stmts }` / `{ it + 1 }` It 形式——It 无箭头，块路径保持既有行为）。
/// 扫描上限 = MAX_PARSE_DEPTH × 8（防病态长体失控）；越限按块处理。
fn body_brace_is_lambda(session: &ParseSession<'_>) -> bool {
    // cursor 停在 '{'（未消费）：peek_ahead(1) = '{' 后第一个 token，
    // 与 parse_lambda 自身扫描的起点约定一致
    let mut depth = 1usize;
    let mut pos = 1usize;
    let cap = MAX_PARSE_DEPTH * 8;
    while pos < cap {
        let Some(token) = session.cursor.peek_ahead(pos) else {
            return false;
        };
        match token.kind {
            TokenKind::Punctuation(PunctuationKind::LBrace)
            | TokenKind::Punctuation(PunctuationKind::LParen)
            | TokenKind::Punctuation(PunctuationKind::LBracket) => depth += 1,
            TokenKind::Punctuation(PunctuationKind::RBrace) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return false;
                }
            }
            TokenKind::Punctuation(PunctuationKind::RParen)
            | TokenKind::Punctuation(PunctuationKind::RBracket) => depth = depth.saturating_sub(1),
            TokenKind::Operator(OperatorKind::Arrow) if depth == 1 => return true,
            _ => {}
        }
        pos += 1;
    }
    false
}

pub(crate) fn parse_block(session: &mut ParseSession<'_>) -> Result<Block, ParserError> {
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::LBrace))
    {
        let span = session.cursor.peek().span;
        expected(session, "'{'", span);
    }
    Ok(parse_block_after_open(session)?.0)
}

/// 解析 `{` 之后的块体；同时返回右花括号 span（parse_if 等需要精确的
/// 块结束位置——空块的 tail/语句 span 推导不出 `}` 位置）。
fn parse_block_after_open(session: &mut ParseSession<'_>) -> Result<(Block, Span), ParserError> {
    let mut stmts = Vec::new();
    let mut tail = None;
    session.cursor.skip_nl();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
    {
        if session.cursor.is_eof() {
            expected(session, "'}'", session.cursor.peek().span);
            break;
        }
        if session.cursor.check(&TokenKind::Nl) {
            session.cursor.advance();
            continue;
        }
        match crate::stmt::parse_statement(session)? {
            crate::stmt::ParsedStatement::Statement(statement) => {
                // EBNF Block := "{" { Statement } [ NL ] [ Expr ] "}"：块尾（直达
                // `}`）的 Loop 语句按 LoopExpr 语义归一为尾表达式——带值 break
                // 即块的值（`fun f() : int { 'outer: loop { break 'outer 42 } }`）。
                // 节点 id 保持：label→flow 解析（LabelFrame.node）不受影响。
                // 后随语句时保持语句形态（带值 break 的值丢弃，Rust 同纪律）。
                if let StmtKind::Loop { label, body } = statement.kind {
                    session.cursor.skip_nl();
                    if session
                        .cursor
                        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                    {
                        tail = Some(Box::new(ExprWrap::new(
                            statement.id,
                            statement.span,
                            ExprKind::Loop { label, body },
                        )));
                        continue;
                    }
                    stmts.push(StmtWrap::new(
                        statement.id,
                        statement.span,
                        StmtKind::Loop { label, body },
                    ));
                    session.cursor.skip_nl();
                    continue;
                }
                stmts.push(statement);
            }
            crate::stmt::ParsedStatement::Tail(expr) => {
                // EBNF Block := "{" { Statement } [Expr] "}"：裸表达式仅当
                // 其后直达块尾才是尾表达式，否则降级为表达式语句保持源序。
                session.cursor.skip_nl();
                if session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                {
                    tail = Some(Box::new(expr));
                } else {
                    stmts.push(StmtWrap::new(
                        session.next_node()?,
                        expr.span,
                        StmtKind::Expr(ExprStmt { expr }),
                    ));
                }
            }
        }
        session.cursor.skip_nl();
    }
    let close = session.cursor.advance();
    if close.kind != TokenKind::Punctuation(PunctuationKind::RBrace) {
        expected(session, "'}'", close.span);
    }
    Ok((Block { stmts, tail }, close.span))
}

fn parse_if(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let start = session.cursor.advance().span;
    let cond = parse_refutable_condition(session)?;
    let then_block = parse_block(session)?;
    let then_close = session.cursor.previous_span();
    let mut else_block = None;
    let mut end = then_close;
    if session.cursor.check(&TokenKind::Keyword(HardKeyword::Else)) {
        session.cursor.advance();
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::If)) {
            let nested = parse_if(session)?;
            end = nested.span;
            else_block = Some(Box::new(Block {
                stmts: Vec::new(),
                tail: Some(Box::new(nested)),
            }));
        } else {
            let block = parse_block(session)?;
            let tail_span = block_tail_span(&block);
            // 空 else 块：block_tail_span 退化为默认 span，回退右花括号位置
            end = if tail_span.end == 0 {
                session.cursor.previous_span()
            } else {
                tail_span
            };
            else_block = Some(Box::new(block));
        }
    }
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(start, end),
        ExprKind::If {
            cond: Box::new(cond),
            then_block: Box::new(then_block),
            else_block,
        },
    ))
}

pub(crate) fn parse_refutable_condition(
    session: &mut ParseSession<'_>,
) -> Result<ast::syntax::expr::Condition, ParserError> {
    if session.cursor.check(&TokenKind::Keyword(HardKeyword::Val)) {
        session.cursor.advance();
        let pattern = parse_pattern(session)?;
        let close = session.cursor.peek().span;
        if !session
            .cursor
            .bump_if(&TokenKind::Operator(OperatorKind::Eq))
        {
            expected(session, "'='", close);
        }
        let expr = parse_expr(session)?;
        return Ok(ast::syntax::expr::Condition::Val {
            pattern: Box::new(pattern),
            expr: Box::new(expr),
        });
    }
    Ok(ast::syntax::expr::Condition::Expr(Box::new(parse_expr(
        session,
    )?)))
}

fn parse_when(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let start = session.cursor.advance().span;
    let subject = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
    {
        None
    } else {
        Some(Box::new(parse_expr(session)?))
    };
    let _open = session.cursor.advance();
    let mut branches = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
    {
        if session.cursor.is_eof() {
            break;
        }
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
            || session.cursor.is_eof()
        {
            break;
        }
        let (pattern, guard) = if session.cursor.check(&TokenKind::Keyword(HardKeyword::Else)) {
            session.cursor.advance();
            if !session
                .cursor
                .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
            {
                let span = session.cursor.peek().span;
                expected(session, "'->'", span);
            }
            (WhenPattern::Else, None)
        } else {
            let checkpoint = session.checkpoint();
            let pattern = crate::pat::parse_when_branch_pattern(session)?;
            let guard = if session.cursor.check(&TokenKind::Keyword(HardKeyword::If)) {
                session.cursor.advance();
                Some(Box::new(parse_expr(session)?))
            } else {
                None
            };
            if session
                .cursor
                .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
            {
                session.cursor.skip_nl();
                (WhenPattern::Pattern(pattern), guard)
            } else {
                // 不是「模式 [if 守卫] ->」：回滚后按表达式分支解析
                session.rollback(&checkpoint);
                let expr = parse_expr(session)?;
                if !session
                    .cursor
                    .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
                {
                    let span = session.cursor.peek().span;
                    expected(session, "'->'", span);
                }
                session.cursor.skip_nl();
                (WhenPattern::Expression(expr), None)
            }
        };
        let body = if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
        {
            session.cursor.skip_nl();
            WhenBody::Block(parse_block(session)?)
        } else if matches!(
            session.cursor.peek().kind,
            TokenKind::Keyword(HardKeyword::Break)
                | TokenKind::Keyword(HardKeyword::Continue)
                | TokenKind::Keyword(HardKeyword::Return)
        ) {
            match crate::stmt::parse_statement(session)? {
                crate::stmt::ParsedStatement::Statement(stmt) => WhenBody::Block(Block {
                    stmts: vec![stmt],
                    tail: None,
                }),
                crate::stmt::ParsedStatement::Tail(expr) => WhenBody::Expr(expr),
            }
        } else {
            WhenBody::Expr(parse_expr(session)?)
        };
        branches.push(WhenBranch {
            pattern,
            guard,
            body,
        });
        session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma));
        session.cursor.skip_nl();
    }
    let close = session.cursor.advance();
    if close.kind != TokenKind::Punctuation(PunctuationKind::RBrace) {
        expected(session, "'}'", close.span);
    }
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(start, close.span),
        ExprKind::When { subject, branches },
    ))
}

fn parse_spawn(session: &mut ParseSession<'_>, start: Span) -> Result<ExprWrap, ParserError> {
    session.cursor.advance();
    let mut scheduler = None;
    if let TokenKind::Identifier = session.cursor.peek().kind {
        match session.token_text(session.cursor.peek()) {
            "detached" => {
                session.cursor.advance();
                scheduler = Some(SpawnScheduler::Detached);
            }
            "concurrent" => {
                session.cursor.advance();
                scheduler = Some(SpawnScheduler::Concurrent);
            }
            _ => {}
        }
    }
    let mut kind = None;
    if let TokenKind::Identifier = session.cursor.peek().kind {
        kind = match session.token_text(session.cursor.peek()) {
            "thread" => {
                session.cursor.advance();
                Some(SpawnKind::Thread)
            }
            "fiber" => {
                session.cursor.advance();
                Some(SpawnKind::Fiber)
            }
            "process" => {
                session.cursor.advance();
                Some(SpawnKind::Process)
            }
            _ => None,
        };
    }
    let mut actor = None;
    if let TokenKind::Identifier = session.cursor.peek().kind
        && session.token_text(session.cursor.peek()) == "actor"
    {
        session.cursor.advance();
        let name = parse_path(session)?;
        let args = if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LParen))
        {
            parse_call_args(session)?.0
        } else {
            Vec::new()
        };
        actor = Some(ActorConstruction { name, args });
    }
    let body = parse_block(session)?;
    let end = block_tail_span(&body);
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(start, end),
        ExprKind::Spawn(SpawnExpr {
            scheduler,
            kind,
            actor,
            body: Box::new(body),
        }),
    ))
}

fn parse_super(
    session: &mut ParseSession<'_>,
    id: ast::syntax::node::NodeId,
    start: Span,
) -> Result<ExprWrap, ParserError> {
    let suffix = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LParen))
    {
        SuperSuffix::Call(parse_call_args(session)?.0)
    } else if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Dot))
    {
        SuperSuffix::Member(parse_identifier(session).0)
    } else {
        let span = session.cursor.peek().span;
        session.emit_unique(DiagCode::ExpectedToken, span, || {
            "expected '.' or '(' after 'super'".to_string()
        });
        SuperSuffix::Call(Vec::new())
    };
    let end = session.cursor.previous_span();
    Ok(ExprWrap::new(
        id,
        join_span(start, end),
        ExprKind::Super { suffix },
    ))
}

pub(crate) fn parse_call_args(
    session: &mut ParseSession<'_>,
) -> Result<(Vec<CallArg>, Span), ParserError> {
    session.cursor.advance();
    let mut args = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        if session.cursor.is_eof() {
            break;
        }
        let mut name = None;
        if session.cursor.check(&TokenKind::Identifier)
            && session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Operator(OperatorKind::Eq))
        {
            let name_token = session.cursor.advance();
            session.cursor.advance();
            name = Some(session.interner.intern(session.token_text(&name_token)));
        }
        let value = parse_expr(session)?;
        args.push(CallArg { name, value });
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
    Ok((args, close))
}

fn parse_macro_tokens(session: &mut ParseSession<'_>) -> Result<Vec<TokenTree>, ParserError> {
    session.cursor.advance(); // (
    match session.try_with_depth(|session| parse_macro_group(session, PunctuationKind::RParen))? {
        Some((tokens, _close)) => Ok(tokens),
        None => macro_depth_error(session).map(|(tokens, _close)| tokens),
    }
}

fn macro_depth_error(
    session: &mut ParseSession<'_>,
) -> Result<(Vec<TokenTree>, Option<Token>), ParserError> {
    let span = session.cursor.peek().span;
    session.emit_unique(DiagCode::DepthLimitExceeded, span, || {
        format!("macro token nesting exceeds {MAX_PARSE_DEPTH}")
    });
    Ok((Vec::new(), None))
}

fn parse_macro_group(
    session: &mut ParseSession<'_>,
    close_kind: PunctuationKind,
) -> Result<(Vec<TokenTree>, Option<Token>), ParserError> {
    let mut tokens = Vec::new();
    while !session.cursor.is_eof() {
        let kind = session.cursor.peek().kind;
        if kind == TokenKind::Punctuation(close_kind) {
            return Ok((tokens, Some(session.cursor.advance())));
        }

        let close = match kind {
            TokenKind::Punctuation(PunctuationKind::LParen) => PunctuationKind::RParen,
            TokenKind::Punctuation(PunctuationKind::LBracket) => PunctuationKind::RBracket,
            TokenKind::Punctuation(PunctuationKind::LBrace) => PunctuationKind::RBrace,
            _ => {
                tokens.push(TokenTree::Token(session.cursor.advance()));
                continue;
            }
        };

        let open = session.cursor.advance();
        match session.try_with_depth(|session| parse_macro_group(session, close))? {
            Some((group_tokens, group_close)) => tokens.push(TokenTree::Group {
                open,
                tokens: group_tokens,
                close: group_close,
            }),
            None => {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::DepthLimitExceeded, span, || {
                    format!("macro token nesting exceeds {MAX_PARSE_DEPTH}")
                });
                return Ok((tokens, None));
            }
        }
    }

    let span = session.cursor.peek().span;
    session.emit_unique(DiagCode::ExpectedToken, span, || {
        "unclosed macro token group".to_string()
    });
    Ok((tokens, None))
}

pub(crate) fn parse_path(
    session: &mut ParseSession<'_>,
) -> Result<ast::syntax::node::Path, ParserError> {
    let first = parse_identifier(session);
    let mut segments = vec![ast::syntax::node::PathSegment {
        name: first.0,
        span: first.1,
    }];
    // Only continue on `.` when an identifier actually follows; otherwise stop
    // and leave `.` for the caller (`Type.*` pointcuts, `a.` postfix, …).
    while session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::Dot))
        && session
            .cursor
            .peek_ahead(1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        session.cursor.advance();
        let next = parse_identifier(session);
        segments.push(ast::syntax::node::PathSegment {
            name: next.0,
            span: next.1,
        });
    }
    Ok(ast::syntax::node::Path { segments })
}

fn integer_literal_index(session: &ParseSession<'_>, span: Span) -> Option<usize> {
    let text = span.slice(session.source).replace('_', "");
    let (digits, radix) = match text.as_str() {
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0x") => (&s[2..], 16),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0o") => (&s[2..], 8),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0b") => (&s[2..], 2),
        s => (s, 10),
    };
    i128::from_str_radix(digits, radix)
        .ok()
        .and_then(|value| usize::try_from(value).ok())
}

pub(crate) fn parse_identifier(
    session: &mut ParseSession<'_>,
) -> (ast::syntax::node::Symbol, Span) {
    let token = session.cursor.advance();
    if token.kind != TokenKind::Identifier {
        let diag = session.emit_unique(DiagCode::ExpectedToken, token.span, || {
            format!("expected identifier, found {}", token.kind)
        });
        let _ = diag;
    }
    let symbol = session.interner.intern(session.token_text(&token));
    (symbol, token.span)
}

/// Decode a lexer's raw `String` token for named slots (native paths, FFI
/// languages). General expression strings keep raw spans for interpolation.
pub(crate) fn decode_string_token(session: &mut ParseSession<'_>, token: &Token) -> String {
    let raw = session.token_text(token);
    debug_assert!(raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"'));
    let mut decoded = String::with_capacity(raw.len().saturating_sub(2));
    let mut chars = raw[1..raw.len() - 1].chars();
    let invalid_escape = |session: &mut ParseSession<'_>, span: Span| {
        session.emit_unique(DiagCode::InvalidEscape, span, || {
            "invalid string escape".to_string()
        });
    };
    while let Some(char) = chars.next() {
        if char != '\\' {
            decoded.push(char);
            continue;
        }
        match chars.next() {
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('0') => decoded.push('\0'),
            Some('u') => {
                if chars.next() != Some('{') {
                    invalid_escape(session, token.span);
                    decoded.push('u');
                    continue;
                }
                let mut hex = String::new();
                let mut terminated = false;
                for hex_char in chars.by_ref() {
                    if hex_char == '}' {
                        terminated = true;
                        break;
                    }
                    if hex.len() == 6 || !hex_char.is_ascii_hexdigit() {
                        invalid_escape(session, token.span);
                        hex.clear();
                        break;
                    }
                    hex.push(hex_char);
                }
                let value = if terminated && !hex.is_empty() {
                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                } else {
                    None
                };
                match value {
                    Some(value) => decoded.push(value),
                    None => {
                        invalid_escape(session, token.span);
                        decoded.push(char::REPLACEMENT_CHARACTER);
                    }
                }
            }
            Some(escaped @ ('\\' | '"' | '\'' | '$' | '/')) => decoded.push(escaped),
            Some(escaped) => {
                invalid_escape(session, token.span);
                decoded.push(escaped);
            }
            None => {
                invalid_escape(session, token.span);
                decoded.push('\\');
            }
        }
    }
    decoded
}

fn expected(session: &mut ParseSession<'_>, expected: &str, span: Span) {
    session.emit_unique(DiagCode::ExpectedToken, span, || {
        format!("expected {expected}")
    });
}

fn depth_error(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let span = session.cursor.peek().span;
    let id = session.next_node()?;
    let diag = session.emit_unique(DiagCode::DepthLimitExceeded, span, || {
        format!("expression nesting exceeds {MAX_PARSE_DEPTH}")
    });
    Ok(ExprWrap::new(
        id,
        span,
        ExprKind::Error(ErrorNode {
            id,
            span,
            err: diag,
        }),
    ))
}

fn block_tail_span(block: &Block) -> Span {
    block
        .tail
        .as_ref()
        .map(|expr| expr.span)
        .unwrap_or_else(|| block.stmts.last().map(|stmt| stmt.span).unwrap_or_default())
}

/// 字符串插值子扫描：`$IDENT` 与 `${Expr}` 解析为表达式部件，`\$` 保持字面。
/// 纯字面串走零开销快路径，仍是 `Literal(String)`。
fn parse_string_literal(
    session: &mut ParseSession<'_>,
    token: Token,
    id: ast::syntax::node::NodeId,
    span: Span,
) -> Result<ExprWrap, ParserError> {
    let text = session.token_text(&token);
    if !has_interpolation(text) {
        return Ok(ExprWrap::new(
            id,
            span,
            ExprKind::Literal(LiteralKind::String),
        ));
    }
    let open = span.start as usize + 1;
    let close = span.end as usize - 1;
    let source = session.source;
    let bytes = source.as_bytes();
    let mut parts = Vec::new();
    let mut chunk_start = open;
    let mut index = open;
    while index < close {
        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(close);
            }
            b'$' if index + 1 < close && bytes[index + 1] == b'{' => {
                if chunk_start < index {
                    parts.push(string_chunk(session, chunk_start, index)?);
                }
                let expr_start = index + 2;
                let Some(scan) = interpolation_close(session, expr_start, close) else {
                    session.emit_unique(
                        DiagCode::UnclosedInterpolation,
                        Span::new(index as u32, span.end),
                        || "unclosed ${ interpolation".to_string(),
                    );
                    break;
                };
                let tokens = session.tokenize_range(expr_start, scan);
                parts.push(parse_interpolation_expr(session, tokens)?);
                index = scan + 1;
                chunk_start = index;
            }
            b'$' if index + 1 < close && is_ident_start(bytes[index + 1]) => {
                if chunk_start < index {
                    parts.push(string_chunk(session, chunk_start, index)?);
                }
                let name_start = index + 1;
                let mut name_end = name_start;
                while name_end < close && is_ident_continue(bytes[name_end]) {
                    name_end += 1;
                }
                let symbol = session.interner.intern(&source[name_start..name_end]);
                let part_id = session.next_node()?;
                parts.push(ExprWrap::new(
                    part_id,
                    Span::new(index as u32, name_end as u32),
                    ExprKind::Ident(symbol),
                ));
                index = name_end;
                chunk_start = index;
            }
            _ => {
                index += 1;
            }
        }
    }
    if chunk_start < close {
        parts.push(string_chunk(session, chunk_start, close)?);
    }
    Ok(ExprWrap::new(id, span, ExprKind::StringInterp { parts }))
}

/// Find the matching `}` with the real lexer so braces in nested char/string
/// tokens and comments cannot terminate an interpolation early.
fn interpolation_close(session: &ParseSession<'_>, start: usize, end: usize) -> Option<usize> {
    let fragment = &session.source[start..end];
    let mut depth = 1usize;
    for token in session.lexer.tokenize(fragment) {
        match token.kind {
            TokenKind::Punctuation(PunctuationKind::LBrace) => depth += 1,
            TokenKind::Punctuation(PunctuationKind::RBrace) if depth == 1 => {
                return Some(start + token.span.start as usize);
            }
            TokenKind::Punctuation(PunctuationKind::RBrace) => depth -= 1,
            _ => {}
        }
    }
    None
}

/// 快路径判定：是否存在未转义的 `$` 插值标记。
fn has_interpolation(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'$' => return true,
            _ => index += 1,
        }
    }
    false
}

fn string_chunk(
    session: &mut ParseSession<'_>,
    start: usize,
    end: usize,
) -> Result<ExprWrap, ParserError> {
    let span = Span::new(start as u32, end as u32);
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        span,
        ExprKind::Literal(LiteralKind::String),
    ))
}

/// 在共享 Interner/NodeIdGen/诊断收集器的子会话中解析插值表达式，
/// 保证符号、Node ID 和诊断落回主会话。
fn parse_interpolation_expr(
    session: &mut ParseSession<'_>,
    tokens: Vec<Token>,
) -> Result<ExprWrap, ParserError> {
    let source = session.source;
    let source_len = session.source_len;
    let interner = std::mem::replace(&mut session.interner, Interner::new());
    let id_gen = std::mem::replace(&mut session.id_gen, NodeIdGen::new());
    let diags = std::mem::replace(&mut session.diags, DiagCollector::new());
    let mut child = ParseSession {
        source,
        source_len,
        cursor: TokenCursor::new(tokens, source_len),
        interner,
        id_gen,
        diags,
        depth: session.depth + 1,
        lexer: session.lexer,
        segments: Vec::new(),
        seg_idx: 0,
    };
    let result = parse_expr(&mut child);
    if !child.cursor.is_eof() {
        let span = child.cursor.peek().span;
        child.emit_unique(DiagCode::UnexpectedToken, span, || {
            "unexpected token in interpolation".to_string()
        });
    }
    session.interner = child.interner;
    session.id_gen = child.id_gen;
    session.diags = child.diags;
    result
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic() || byte.is_ascii_digit()
}
