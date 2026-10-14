//! Statement parser。

use ast::syntax::decl::DeclKind;
use ast::syntax::expr::{Block, ExprKind, ExprWrap};
use ast::syntax::node::{DiagCode, Span, Symbol};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::token::{HardKeyword, PunctuationKind, TokenKind};

use crate::decl::expect_keyword;
use crate::expr::{parse_block, parse_expr, parse_refutable_condition};
use crate::pat::parse_pattern;
use crate::ty::join_span;
use crate::{ParseSession, ParserError};

pub(crate) enum ParsedStatement {
    Statement(StmtWrap),
    Tail(ExprWrap),
}

pub(crate) fn parse_statement(
    session: &mut ParseSession<'_>,
) -> Result<ParsedStatement, ParserError> {
    let start = session.cursor.peek().span;
    let label = parse_label(session)?;
    match session.cursor.peek().kind {
        TokenKind::Keyword(HardKeyword::Val) | TokenKind::Keyword(HardKeyword::Var) => {
            let declaration = crate::decl::parse_binding_decl(session)?;
            let DeclKind::Binding(binding) = declaration.kind else {
                unreachable!("binding parser returns Binding");
            };
            Ok(ParsedStatement::Statement(StmtWrap::new(
                declaration.id,
                declaration.span,
                StmtKind::Binding(Box::new(binding)),
            )))
        }
        TokenKind::Keyword(HardKeyword::For) => {
            session.cursor.advance();
            let pattern = parse_pattern(session)?;
            expect_keyword(session, HardKeyword::In, "in");
            let iterable = parse_expr(session)?;
            let body = parse_block(session)?;
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, block_end(&body)),
                StmtKind::For {
                    label,
                    pattern: Box::new(pattern),
                    iterable: Box::new(iterable),
                    body: Box::new(body),
                },
            )))
        }
        TokenKind::Keyword(HardKeyword::While) => {
            session.cursor.advance();
            let cond = parse_refutable_condition(session)?;
            let body = parse_block(session)?;
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, block_end(&body)),
                StmtKind::While {
                    label,
                    cond: Box::new(cond),
                    body: Box::new(body),
                },
            )))
        }
        TokenKind::Keyword(HardKeyword::Loop) => {
            session.cursor.advance();
            let body = parse_block(session)?;
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, block_end(&body)),
                StmtKind::Loop {
                    label,
                    body: Box::new(body),
                },
            )))
        }
        TokenKind::Keyword(HardKeyword::Break) => {
            session.cursor.advance();
            let label = parse_optional_label(session);
            let value = if statement_has_expression(session) {
                Some(Box::new(parse_expr(session)?))
            } else {
                None
            };
            let end = value.as_ref().map(|expr| expr.span).unwrap_or(start);
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, end),
                StmtKind::Break { label, value },
            )))
        }
        TokenKind::Keyword(HardKeyword::Continue) => {
            session.cursor.advance();
            let label = parse_optional_label(session);
            let end = label
                .map(|_| session.cursor.previous_span())
                .unwrap_or(start);
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, end),
                StmtKind::Continue { label },
            )))
        }
        TokenKind::Keyword(HardKeyword::Return) => {
            session.cursor.advance();
            let value = if statement_has_expression(session) {
                Some(Box::new(parse_expr(session)?))
            } else {
                None
            };
            let end = value.as_ref().map(|expr| expr.span).unwrap_or(start);
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, end),
                StmtKind::Return { value },
            )))
        }
        TokenKind::Keyword(HardKeyword::Defer) => {
            session.cursor.advance();
            let body = parse_block(session)?;
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, block_end(&body)),
                StmtKind::Defer {
                    body: Box::new(body),
                },
            )))
        }
        TokenKind::Identifier if session.token_text(session.cursor.peek()) == "grow" => {
            session.cursor.advance();
            let expr = parse_expr(session)?;
            if !session.cursor.check(&TokenKind::Identifier)
                || session.token_text(session.cursor.peek()) != "to"
            {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::ExpectedToken, span, || {
                    "expected 'to'".to_string()
                });
            } else {
                session.cursor.advance();
            }
            let bits_token = session.cursor.advance();
            let bits_expr = ExprWrap::new(
                session.next_node()?,
                bits_token.span,
                ExprKind::Literal(ast::token::LiteralKind::Integer),
            );
            if !session.cursor.check(&TokenKind::Identifier)
                || session.token_text(session.cursor.peek()) != "bits"
            {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::ExpectedToken, span, || {
                    "expected 'bits'".to_string()
                });
            } else {
                session.cursor.advance();
            }
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, bits_expr.span),
                StmtKind::Grow {
                    expr: Box::new(expr),
                    bits: Some(Box::new(bits_expr)),
                },
            )))
        }
        TokenKind::Identifier if session.token_text(session.cursor.peek()) == "comptime" => {
            session.cursor.advance();
            let block = parse_block(session)?;
            Ok(ParsedStatement::Statement(StmtWrap::new(
                session.next_node()?,
                join_span(start, block_end(&block)),
                StmtKind::Comptime {
                    block: Box::new(block),
                },
            )))
        }
        _ => {
            if label.is_some() {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::ExpectedToken, span, || {
                    "expected labeled loop".to_string()
                });
            }
            let expr = parse_expr(session)?;
            let assignment = match session.cursor.peek().kind {
                TokenKind::Operator(op) if is_assignment(op) => Some(op),
                _ => None,
            };
            if let Some(op) = assignment {
                session.cursor.advance();
                let value = parse_expr(session)?;
                let span = join_span(expr.span, value.span);
                return Ok(ParsedStatement::Statement(StmtWrap::new(
                    session.next_node()?,
                    span,
                    StmtKind::Assignment {
                        target: Box::new(expr),
                        op,
                        value: Box::new(value),
                    },
                )));
            }
            Ok(ParsedStatement::Tail(expr))
        }
    }
}

fn parse_label(session: &mut ParseSession<'_>) -> Result<Option<Symbol>, ParserError> {
    if let TokenKind::Label = session.cursor.peek().kind
        && session
            .cursor
            .peek_ahead(1)
            .is_some_and(|token| token.kind == TokenKind::Punctuation(PunctuationKind::Colon))
        && matches!(
            session.cursor.peek_ahead(2).map(|token| &token.kind),
            Some(TokenKind::Keyword(HardKeyword::For))
                | Some(TokenKind::Keyword(HardKeyword::While))
                | Some(TokenKind::Keyword(HardKeyword::Loop))
        )
    {
        let token = session.cursor.advance();
        session.cursor.advance();
        let text = session.token_text(&token);
        return Ok(Some(session.interner.intern(&text[1..])));
    }
    Ok(None)
}

fn parse_optional_label(session: &mut ParseSession<'_>) -> Option<Symbol> {
    if let TokenKind::Label = session.cursor.peek().kind {
        let token = session.cursor.advance();
        let text = session.token_text(&token);
        return Some(session.interner.intern(&text[1..]));
    }
    None
}

fn statement_has_expression(session: &ParseSession<'_>) -> bool {
    !matches!(
        session.cursor.peek().kind,
        TokenKind::Nl
            | TokenKind::Punctuation(PunctuationKind::Semicolon)
            | TokenKind::Punctuation(PunctuationKind::Comma)
            | TokenKind::Punctuation(PunctuationKind::RBrace)
            | TokenKind::Eof
    )
}

fn is_assignment(op: ast::token::OperatorKind) -> bool {
    use ast::token::OperatorKind::*;
    matches!(
        op,
        Eq | PlusEq
            | MinusEq
            | StarEq
            | SlashEq
            | PercentEq
            | BitAndEq
            | BitOrEq
            | BitXorEq
            | ShlEq
            | ShrEq
    )
}

fn block_end(block: &Block) -> Span {
    block
        .tail
        .as_ref()
        .map(|expr| expr.span)
        .or_else(|| block.stmts.last().map(|stmt| stmt.span))
        .unwrap_or_default()
}
