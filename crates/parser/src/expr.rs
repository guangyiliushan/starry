//! Pratt expression parser。

use ast::syntax::expr::{
    Block, CastKind, ExprKind, ExprWrap, LambdaParams, SpawnExpr, SpawnKind, SpawnScheduler,
    SuperSuffix, TokenTree, WhenBody, WhenBranch, WhenPattern,
};
use ast::syntax::node::{DiagCode, ErrorNode, Span};
use ast::token::{HardKeyword, OperatorKind, PunctuationKind, TokenKind};

use crate::pat::parse_pattern;
use crate::ty::join_span;
use crate::{ParseSession, ParserError};

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
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::Is)) {
            session.cursor.advance();
            let pattern = parse_pattern(session)?;
            let span = join_span(lhs.span, pattern.span);
            let id = session.next_node()?;
            lhs = ExprWrap::new(
                id,
                span,
                ExprKind::Is {
                    lhs: Box::new(lhs),
                    pattern: Box::new(pattern),
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
            let pattern = parse_pattern(session)?;
            let span = join_span(lhs.span, pattern.span);
            let id = session.next_node()?;
            lhs = ExprWrap::new(
                id,
                span,
                ExprKind::In {
                    lhs: Box::new(lhs),
                    pattern: Box::new(pattern),
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
        OperatorKind::Star | OperatorKind::Slash | OperatorKind::Percent => Some((23, 24)),
        _ => None,
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
        TokenKind::Operator(OperatorKind::BitAnd) => Some(OperatorKind::BitAnd),
        TokenKind::Operator(OperatorKind::Star) => Some(OperatorKind::Star),
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
        if session.cursor.check(&TokenKind::Operator(OperatorKind::Lt)) {
            let checkpoint = session.checkpoint();
            let type_args = crate::ty::parse_type_args(session)?;
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
                    },
                };
            }
            TokenKind::Punctuation(PunctuationKind::LBracket) => {
                session.cursor.advance();
                let index = parse_expr(session)?;
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
        TokenKind::Keyword(HardKeyword::Super) => {
            session.cursor.advance();
            parse_super(session, id, span)
        }
        TokenKind::Keyword(HardKeyword::If) => parse_if(session),
        TokenKind::Keyword(HardKeyword::When) => parse_when(session),
        TokenKind::Keyword(HardKeyword::Async) => {
            session.cursor.advance();
            let expr = parse_expr(session)?;
            let end = expr.span;
            Ok(ExprWrap::new(
                id,
                join_span(span, end),
                ExprKind::Async {
                    expr: Box::new(expr),
                },
            ))
        }
        TokenKind::Punctuation(PunctuationKind::LParen) => parse_paren_expr(session),
        TokenKind::Punctuation(PunctuationKind::LBracket) => parse_array(session),
        TokenKind::Punctuation(PunctuationKind::LBrace) => parse_lambda(session, span),
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
    let mut items = Vec::new();
    while !session
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
            | TokenKind::Punctuation(PunctuationKind::RBracket) => depth -= 1,
            TokenKind::Punctuation(PunctuationKind::RBrace) => {
                if depth == 1 {
                    break;
                }
                depth -= 1;
            }
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
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
        {
            parse_block(session)?
        } else {
            parse_block_after_open(session)?
        }
    } else {
        parse_block_after_open(session)?
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

pub(crate) fn parse_block(session: &mut ParseSession<'_>) -> Result<Block, ParserError> {
    let _open = session.cursor.advance();
    parse_block_after_open(session)
}

fn parse_block_after_open(session: &mut ParseSession<'_>) -> Result<Block, ParserError> {
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
            crate::stmt::ParsedStatement::Statement(statement) => stmts.push(statement),
            crate::stmt::ParsedStatement::Tail(expr) => tail = Some(Box::new(expr)),
        }
        session.cursor.skip_nl();
    }
    let close = session.cursor.advance();
    if close.kind != TokenKind::Punctuation(PunctuationKind::RBrace) {
        expected(session, "'}'", close.span);
    }
    Ok(Block { stmts, tail })
}

fn parse_if(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let start = session.cursor.advance().span;
    let cond = parse_refutable_condition(session)?;
    let then_block = parse_block(session)?;
    let mut else_block = None;
    if session.cursor.check(&TokenKind::Keyword(HardKeyword::Else)) {
        session.cursor.advance();
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::If)) {
            let nested = parse_if(session)?;
            else_block = Some(Box::new(Block {
                stmts: Vec::new(),
                tail: Some(Box::new(nested)),
            }));
        } else {
            else_block = Some(Box::new(parse_block(session)?));
        }
    }
    let end = else_block
        .as_ref()
        .map(|block| block_tail_span(block))
        .unwrap_or_else(|| block_tail_span(&then_block));
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
        let pattern = if session.cursor.check(&TokenKind::Keyword(HardKeyword::Else)) {
            session.cursor.advance();
            WhenPattern::Else
        } else {
            WhenPattern::Expression(parse_expr(session)?)
        };
        session
            .cursor
            .bump_if(&TokenKind::Operator(OperatorKind::Arrow));
        let body = if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
        {
            WhenBody::Block(parse_block(session)?)
        } else {
            WhenBody::Expr(parse_expr(session)?)
        };
        branches.push(WhenBranch { pattern, body });
        if !session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            break;
        }
    }
    let close = session.cursor.advance();
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
    let body = parse_block(session)?;
    let end = block_tail_span(&body);
    let id = session.next_node()?;
    Ok(ExprWrap::new(
        id,
        join_span(start, end),
        ExprKind::Spawn(SpawnExpr {
            scheduler,
            kind,
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
        SuperSuffix::Call(Vec::new())
    };
    let end = session
        .cursor
        .peek_ahead(0)
        .map(|token| token.span)
        .unwrap_or_default();
    Ok(ExprWrap::new(
        id,
        join_span(start, end),
        ExprKind::Super { suffix },
    ))
}

pub(crate) fn parse_call_args(
    session: &mut ParseSession<'_>,
) -> Result<(Vec<ExprWrap>, Span), ParserError> {
    session.cursor.advance();
    let mut args = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        args.push(parse_expr(session)?);
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
    let mut tokens = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        if session.cursor.is_eof() {
            break;
        }
        tokens.push(TokenTree::Token(session.cursor.advance()));
    }
    session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen));
    Ok(tokens)
}

pub(crate) fn parse_path(
    session: &mut ParseSession<'_>,
) -> Result<ast::syntax::node::Path, ParserError> {
    let first = parse_identifier(session);
    let mut segments = vec![ast::syntax::node::PathSegment {
        name: first.0,
        span: first.1,
    }];
    while matches!(
        session.cursor.peek().kind,
        TokenKind::Punctuation(PunctuationKind::Dot)
            | TokenKind::Punctuation(PunctuationKind::ColonColon)
    ) {
        session.cursor.advance();
        let next = parse_identifier(session);
        segments.push(ast::syntax::node::PathSegment {
            name: next.0,
            span: next.1,
        });
    }
    Ok(ast::syntax::node::Path { segments })
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

fn expected(session: &mut ParseSession<'_>, expected: &str, span: Span) {
    session.emit_unique(DiagCode::ExpectedToken, span, || {
        format!("expected {expected}")
    });
}

fn depth_error(session: &mut ParseSession<'_>) -> Result<ExprWrap, ParserError> {
    let span = session.cursor.peek().span;
    let id = session.next_node()?;
    let diag = session.emit_unique(DiagCode::DepthLimitExceeded, span, || {
        "expression nesting exceeds 100".to_string()
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
