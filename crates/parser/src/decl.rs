//! Declaration parser for the frozen v0.4 grammar.

use ast::syntax::decl::*;
use ast::syntax::modifier::ModifierList;
use ast::syntax::node::Path;
use ast::syntax::node::{DiagCode, ErrorNode, Span, Symbol};
use ast::token::{HardKeyword, LiteralKind, OperatorKind, PunctuationKind, TokenKind};

use crate::expr::{parse_block, parse_expr, parse_identifier};
use crate::pat::parse_pattern;
use crate::ty::{join_span, parse_type, parse_type_params};
use crate::{MAX_PARSE_DEPTH, ParseSession, ParserError};

macro_rules! complete_decl {
    ($session:expr, $start:expr, $kind:expr) => {{
        let kind = $kind;
        let end = $session.cursor.previous_span();
        Ok(DeclNode {
            id: $session.next_node()?,
            span: join_span($start, end),
            kind,
        })
    }};
}

pub(crate) fn parse_modifier_list(
    session: &mut ParseSession<'_>,
) -> Result<ModifierList, ParserError> {
    let mut modifiers = ModifierList::new();
    while let TokenKind::Identifier = session.cursor.peek().kind {
        let Some(kind) =
            ast::token::lookup_modifier_word(session.token_text(session.cursor.peek()))
        else {
            break;
        };
        let token = session.cursor.advance();
        modifiers.push(kind, token.span);
    }
    Ok(modifiers)
}

pub(crate) fn parse_parameter_list(
    session: &mut ParseSession<'_>,
) -> Result<Vec<Param>, ParserError> {
    session.cursor.advance();
    let mut params = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        params.push(parse_parameter(session)?);
        if !session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
        {
            break;
        }
    }
    let close = session.cursor.peek().span;
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::RParen))
    {
        session.emit_unique(DiagCode::ExpectedToken, close, || {
            "expected ')'".to_string()
        });
    }
    Ok(params)
}

fn parse_parameter(session: &mut ParseSession<'_>) -> Result<Param, ParserError> {
    let start = session.cursor.peek().span;
    let modifier = match session.cursor.peek().kind {
        TokenKind::Keyword(HardKeyword::Mut) => {
            session.cursor.advance();
            ParamModifier::Mutable
        }
        TokenKind::Keyword(HardKeyword::Val) => {
            session.cursor.advance();
            ParamModifier::Val
        }
        TokenKind::Keyword(HardKeyword::Var) => {
            session.cursor.advance();
            ParamModifier::Var
        }
        _ => ParamModifier::None,
    };
    let name = parse_identifier(session).0;
    let ty = if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        Some(parse_type(session)?)
    } else {
        None
    };
    let default = if session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Eq))
    {
        Some(Box::new(parse_expr(session)?))
    } else {
        None
    };
    let _end = default
        .as_ref()
        .map(|expr| expr.span)
        .or_else(|| ty.as_ref().map(|ty| ty.span))
        .unwrap_or(start);
    Ok(Param {
        modifier,
        name,
        ty,
        default,
    })
}

pub(crate) fn parse_fun_signature(
    session: &mut ParseSession<'_>,
) -> Result<FunSignature, ParserError> {
    let modifiers = parse_modifier_list(session)?;
    expect_keyword(session, HardKeyword::Fun, "fun");
    let name = parse_identifier(session).0;
    let type_params = parse_type_params(session)?;
    let params = parse_parameter_list(session)?;
    let ret = if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        Some(parse_type(session)?)
    } else {
        None
    };
    Ok(FunSignature {
        modifiers,
        name,
        type_params,
        params,
        ret,
    })
}

pub(crate) fn parse_fun_decl(session: &mut ParseSession<'_>) -> Result<FunDecl, ParserError> {
    let signature = parse_fun_signature(session)?;
    let where_clause = parse_where_clause(session)?;
    let body = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
    {
        Some(FunctionBody::Block(parse_block(session)?))
    } else if session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Eq))
    {
        session.cursor.skip_nl();
        Some(FunctionBody::Expr(parse_expr(session)?))
    } else {
        let span = session.cursor.peek().span;
        session.emit_unique(DiagCode::ExpectedToken, span, || {
            "expected function body".to_string()
        });
        None
    };
    Ok(FunDecl {
        modifiers: signature.modifiers,
        name: signature.name,
        type_params: signature.type_params,
        params: signature.params,
        ret: signature.ret,
        where_clause,
        body,
    })
}

fn parse_where_clause(session: &mut ParseSession<'_>) -> Result<Vec<WhereConstraint>, ParserError> {
    if !session
        .cursor
        .bump_if(&TokenKind::Keyword(HardKeyword::Where))
    {
        return Ok(Vec::new());
    }
    let mut constraints = vec![parse_where_constraint(session)?];
    while session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
    {
        constraints.push(parse_where_constraint(session)?);
    }
    Ok(constraints)
}

fn parse_where_constraint(session: &mut ParseSession<'_>) -> Result<WhereConstraint, ParserError> {
    let ty = parse_type(session)?;
    expect_punctuation(session, PunctuationKind::Colon, ':');
    let bounds = crate::ty::parse_bounds(session)?;
    Ok(WhereConstraint { ty, bounds })
}

pub(crate) fn parse_binding_decl(session: &mut ParseSession<'_>) -> Result<DeclNode, ParserError> {
    let start = session.cursor.peek().span;
    let modifiers = parse_modifier_list(session)?;
    let keyword = session.cursor.advance();
    let immutable = keyword.kind == TokenKind::Keyword(HardKeyword::Val);
    parse_binding_body(session, start, modifiers, immutable)
}

/// Binding body after the `val` / `var` keyword has been consumed (or, for `let`
/// recovery, replaced by a diagnostic). Shared so `let x = 1` recovers into one
/// binding statement instead of silently splitting into two statements.
pub(crate) fn parse_binding_body(
    session: &mut ParseSession<'_>,
    start: Span,
    modifiers: ModifierList,
    immutable: bool,
) -> Result<DeclNode, ParserError> {
    let id = session.next_node()?;
    let pattern = parse_pattern(session)?;
    let pattern_ty = match &pattern.kind {
        ast::syntax::pat::PatKind::Binding { ty, .. } => ty.as_deref().cloned(),
        _ => None,
    };
    let ty = if pattern_ty.is_some() {
        pattern_ty
    } else if session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        Some(parse_type(session)?)
    } else {
        None
    };
    let init = if session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::Eq))
    {
        session.cursor.skip_nl();
        Some(Box::new(parse_expr(session)?))
    } else {
        None
    };
    let end = init
        .as_ref()
        .map(|expr| expr.span)
        .or_else(|| ty.as_ref().map(|ty| ty.span))
        .unwrap_or(pattern.span);
    Ok(DeclNode {
        id,
        span: join_span(start, end),
        kind: DeclKind::Binding(BindingDecl {
            modifiers,
            immutable,
            pattern,
            ty,
            init,
        }),
    })
}

pub(crate) fn parse_import(
    session: &mut ParseSession<'_>,
) -> Result<(ImportDecl, Span), ParserError> {
    let start = session.cursor.advance().span;
    let token = *session.cursor.peek();
    if session
        .cursor
        .check(&TokenKind::Literal(LiteralKind::String))
        && session
            .cursor
            .peek_ahead(1)
            .is_some_and(|next| next.kind == TokenKind::Literal(LiteralKind::String))
    {
        let language = match token.kind {
            TokenKind::Literal(LiteralKind::String) => {
                let decoded = crate::expr::decode_string_token(session, &token);
                match decoded.as_str() {
                    "cpp" => NativeLanguage::Cpp,
                    "c" => NativeLanguage::C,
                    _ => {
                        session.emit_unique(DiagCode::ExpectedToken, token.span, || {
                            "expected native language \"c\" or \"cpp\"".to_string()
                        });
                        NativeLanguage::C
                    }
                }
            }
            _ => NativeLanguage::C,
        };
        session.cursor.advance();
        let path_token = *session.cursor.peek();
        let decoded_path = crate::expr::decode_string_token(session, &path_token);
        let path = session.interner.intern(&decoded_path);
        session.cursor.advance();
        let alias = parse_alias(session)?;
        let mut members = Vec::new();
        if session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::LBrace))
        {
            while !session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                && !session.cursor.is_eof()
            {
                session.cursor.skip_nl();
                if session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                    || session.cursor.is_eof()
                {
                    break;
                }
                members.push(parse_fun_signature(session)?);
                // EBNF separates members with `,`; `;` and newline are accepted
                // as redundant terminators, matching every other `FunSignature` slot.
                session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon));
                session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma));
                session.cursor.skip_nl();
            }
            expect_punctuation(session, PunctuationKind::RBrace, '}');
        }
        return Ok((
            ImportDecl {
                kind: ImportKind::Native {
                    language,
                    path,
                    alias,
                    members,
                },
            },
            start,
        ));
    }

    let path = parse_import_path(session)?;
    let suffix = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::Dot))
        && session
            .cursor
            .peek_ahead(1)
            .is_some_and(|token| token.kind == TokenKind::Operator(OperatorKind::Star))
    {
        session.cursor.advance();
        session.cursor.advance();
        ImportSuffix::Star
    } else if session.cursor.check(&TokenKind::Keyword(HardKeyword::As)) {
        session.cursor.advance();
        ImportSuffix::Alias(parse_identifier(session).0)
    } else {
        ImportSuffix::None
    };
    Ok((
        ImportDecl {
            kind: ImportKind::Qualified { path, suffix },
        },
        start,
    ))
}

fn parse_alias(session: &mut ParseSession<'_>) -> Result<Option<Symbol>, ParserError> {
    if session.cursor.bump_if(&TokenKind::Keyword(HardKeyword::As)) {
        Ok(Some(parse_identifier(session).0))
    } else {
        Ok(None)
    }
}

fn parse_import_path(session: &mut ParseSession<'_>) -> Result<Path, ParserError> {
    let first = parse_identifier(session);
    let mut segments = vec![ast::syntax::node::PathSegment {
        name: first.0,
        span: first.1,
    }];
    loop {
        let wildcard_after_dot = session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::Dot))
            && session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Operator(OperatorKind::Star));
        if wildcard_after_dot {
            break;
        }
        if matches!(
            session.cursor.peek().kind,
            TokenKind::Punctuation(PunctuationKind::Dot)
        ) {
            session.cursor.advance();
            let next = parse_identifier(session);
            segments.push(ast::syntax::node::PathSegment {
                name: next.0,
                span: next.1,
            });
            continue;
        }
        break;
    }
    Ok(Path { segments })
}

pub(crate) fn parse_top_level(session: &mut ParseSession<'_>) -> Result<DeclNode, ParserError> {
    let Some(declaration) = session.try_with_depth(|session| parse_top_level_inner(session))?
    else {
        let token = session.cursor.advance();
        let span = token.span;
        let id = session.next_node()?;
        let diag = session.emit_unique(DiagCode::DepthLimitExceeded, span, || {
            format!("declaration nesting exceeds {MAX_PARSE_DEPTH}")
        });
        return Ok(DeclNode {
            id,
            span,
            kind: DeclKind::Error(ErrorNode {
                id,
                span,
                err: diag,
            }),
        });
    };
    Ok(declaration)
}

fn parse_top_level_inner(session: &mut ParseSession<'_>) -> Result<DeclNode, ParserError> {
    let start = session.cursor.peek().span;
    if session
        .cursor
        .check(&TokenKind::Keyword(HardKeyword::Extern))
    {
        return parse_extern(session, start);
    }
    // M5 slice 3：修饰词解析前捕获真实检查点（内联 session.checkpoint()
    // 只捕获当前位置——曾致回卷 no-op、顶层修饰词静默丢失）
    let cp = session.checkpoint();
    let modifiers = parse_modifier_list(session)?;
    match session.cursor.peek().kind {
        TokenKind::Keyword(HardKeyword::Fun) => complete_decl!(session, start, {
            // M5 slice 3：parse_fun_signature 自读修饰词——回卷到检查点
            // 重解析（与 val/var 臂同模式），否则 438 行的修饰词被丢弃、
            // FunDecl.modifiers 恒空（private/internal 可见性失语的根因）
            session.rollback(&cp);
            let fun = parse_fun_decl(session)?;
            DeclKind::Fun(fun)
        }),
        TokenKind::Keyword(HardKeyword::Val) | TokenKind::Keyword(HardKeyword::Var) => {
            // parse_binding_decl 重新自读修饰词与 val/var（回卷到真实检查点）
            session.rollback(&cp);
            parse_binding_decl(session)
        }
        TokenKind::Keyword(HardKeyword::Class) => complete_decl!(session, start, {
            let _keyword = session.cursor.advance().span;
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            let primary_ctor = parse_primary_constructor(session)?;
            let supertypes = parse_supertypes(session)?;
            let body = if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
            {
                parse_class_body(session)?
            } else {
                ClassBody {
                    members: Vec::new(),
                }
            };
            DeclKind::Class(ClassDecl {
                modifiers,
                name,
                type_params,
                primary_ctor,
                supertypes,
                members: body.members,
            })
        }),
        TokenKind::Keyword(HardKeyword::Struct) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            // M3 主构造：struct Point(val x: int, var y: int)；body 兼容
            //（两者可共存）；头参数无 val/var = 仅构造参数。
            let primary_ctor = if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LParen))
            {
                // parse_parameter_list 自吃左右括号
                parse_parameter_list(session)?
            } else {
                Vec::new()
            };
            let fields = if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
            {
                expect_punctuation(session, PunctuationKind::LBrace, '{');
                let f = parse_struct_fields(session)?;
                expect_punctuation(session, PunctuationKind::RBrace, '}');
                f
            } else {
                Vec::new()
            };
            if primary_ctor.is_empty() && fields.is_empty() {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::ExpectedToken, span, || {
                    "expected a primary constructor or a field body".to_string()
                });
            }
            DeclKind::Struct(StructDecl {
                modifiers,
                name,
                type_params,
                primary_ctor,
                fields,
            })
        }),
        TokenKind::Keyword(HardKeyword::Enum) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            expect_punctuation(session, PunctuationKind::LBrace, '{');
            let variants = parse_enum_variants(session)?;
            expect_punctuation(session, PunctuationKind::RBrace, '}');
            DeclKind::Enum(EnumDecl {
                modifiers,
                name,
                type_params,
                variants,
            })
        }),
        TokenKind::Keyword(HardKeyword::Union) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            expect_punctuation(session, PunctuationKind::LBrace, '{');
            let fields = parse_struct_fields(session)?;
            expect_punctuation(session, PunctuationKind::RBrace, '}');
            DeclKind::Union(UnionDecl {
                modifiers,
                name,
                type_params,
                fields,
            })
        }),
        TokenKind::Keyword(HardKeyword::Trait) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            let supertypes = parse_supertypes(session)?;
            expect_punctuation(session, PunctuationKind::LBrace, '{');
            let members = parse_trait_members(session)?;
            expect_punctuation(session, PunctuationKind::RBrace, '}');
            DeclKind::Trait(TraitDecl {
                modifiers,
                name,
                type_params,
                supertypes,
                members,
            })
        }),
        TokenKind::Keyword(HardKeyword::Alias) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            // EBNF: TypeAliasDecl := Modifiers \x22alias\x22 IDENT TypeParams? "=" Type
            if !session
                .cursor
                .bump_if(&TokenKind::Operator(OperatorKind::Eq))
            {
                let span = session.cursor.peek().span;
                session.emit_unique(DiagCode::ExpectedToken, span, || {
                    "expected '=' in type alias".to_string()
                });
            }
            let ty = parse_type(session)?;
            DeclKind::TypeAlias(TypeAliasDecl {
                modifiers,
                name,
                type_params,
                ty,
            })
        }),
        TokenKind::Identifier => match session.token_text(session.cursor.peek()) {
            "impl" => complete_decl!(session, start, {
                session.cursor.advance();
                // EBNF: ImplDecl := «impl» TypeParams? Type "for" Type "{" { ClassMember } "}"
                let type_params = parse_type_params(session)?;
                let type_ref = parse_type(session)?;
                expect_keyword(session, HardKeyword::For, "for");
                let trait_ref = parse_type(session)?;
                expect_punctuation(session, PunctuationKind::LBrace, '{');
                let members = parse_class_members(session)?;
                expect_punctuation(session, PunctuationKind::RBrace, '}');
                DeclKind::Impl(ImplDecl {
                    type_params,
                    type_ref,
                    trait_ref,
                    members,
                })
            }),
            _ => {
                let checkpoint = session.checkpoint();
                session.rollback(&checkpoint);
                recover_top_level(session)
            }
        },
        _ => {
            let checkpoint = session.checkpoint();
            session.rollback(&checkpoint);
            recover_top_level(session)
        }
    }
}

/// Non-consuming check: does this member begin a `FunSignature` (`… fun NAME`)?
fn at_fun_signature(session: &ParseSession<'_>) -> bool {
    let mut offset = 0;
    while let Some(token) = session.cursor.peek_ahead(offset) {
        if token.kind == TokenKind::Eof {
            return false;
        }
        if token.kind == TokenKind::Keyword(HardKeyword::Fun) {
            return true;
        }
        if token.kind != TokenKind::Identifier
            || ast::token::lookup_modifier_word(session.token_text(token)).is_none()
        {
            return false;
        }
        offset += 1;
    }
    false
}

/// EBNF `FunSignature := Modifiers "fun" … [ ";" ]` — the terminator is
/// optional (newline also ends a signature); a body or `where` clause makes the
/// member a `FunDecl` instead.
fn signature_has_body(session: &mut ParseSession<'_>) -> bool {
    matches!(
        session.cursor.peek().kind,
        TokenKind::Operator(OperatorKind::Eq)
            | TokenKind::Punctuation(PunctuationKind::LBrace)
            | TokenKind::Keyword(HardKeyword::Where)
    )
}

fn parse_fun_signatures_until_rbrace(
    session: &mut ParseSession<'_>,
) -> Result<Vec<FunSignature>, ParserError> {
    let mut members = Vec::new();
    while !session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
        && !session.cursor.is_eof()
    {
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
        {
            break;
        }
        members.push(parse_fun_signature(session)?);
        session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon));
        session.cursor.skip_nl();
    }
    Ok(members)
}

fn parse_extern(session: &mut ParseSession<'_>, start: Span) -> Result<DeclNode, ParserError> {
    session.cursor.advance();
    let language_token = session.cursor.advance();
    let decoded_language = crate::expr::decode_string_token(session, &language_token);
    let language = session.interner.intern(&decoded_language);
    let body = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LBrace))
    {
        session.cursor.advance();
        let members = parse_fun_signatures_until_rbrace(session)?;
        expect_punctuation(session, PunctuationKind::RBrace, '}');
        ExternBody::Block { members }
    } else if session.cursor.check(&TokenKind::Identifier)
        && session.token_text(session.cursor.peek()) == "namespace"
    {
        session.cursor.advance();
        let name = parse_identifier(session).0;
        expect_punctuation(session, PunctuationKind::LBrace, '{');
        let members = parse_fun_signatures_until_rbrace(session)?;
        expect_punctuation(session, PunctuationKind::RBrace, '}');
        ExternBody::Namespace { name, members }
    } else {
        let signature = parse_fun_signature(session)?;
        session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon));
        ExternBody::Fun(signature)
    };
    let end = session.cursor.previous_span();
    Ok(DeclNode {
        id: session.next_node()?,
        span: join_span(start, end),
        kind: DeclKind::Extern(ExternDecl { language, body }),
    })
}

fn parse_primary_constructor(
    session: &mut ParseSession<'_>,
) -> Result<Option<PrimaryConstructor>, ParserError> {
    if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LParen))
    {
        Ok(Some(PrimaryConstructor {
            params: parse_parameter_list(session)?,
        }))
    } else {
        Ok(None)
    }
}

fn parse_supertypes(session: &mut ParseSession<'_>) -> Result<Vec<SuperItem>, ParserError> {
    if !session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
    {
        return Ok(Vec::new());
    }
    let mut supertypes = vec![parse_super_item(session)?];
    while session
        .cursor
        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
    {
        supertypes.push(parse_super_item(session)?);
    }
    Ok(supertypes)
}

fn parse_super_item(session: &mut ParseSession<'_>) -> Result<SuperItem, ParserError> {
    let ty = parse_type(session)?;
    let args = if session
        .cursor
        .check(&TokenKind::Punctuation(PunctuationKind::LParen))
    {
        Some(crate::expr::parse_call_args(session)?.0)
    } else {
        None
    };
    let by = if session.cursor.check(&TokenKind::Identifier)
        && session.token_text(session.cursor.peek()) == "by"
    {
        session.cursor.advance();
        Some(Box::new(parse_expr(session)?))
    } else {
        None
    };
    Ok(SuperItem { ty, args, by })
}

fn parse_class_body(session: &mut ParseSession<'_>) -> Result<ClassBody, ParserError> {
    expect_punctuation(session, PunctuationKind::LBrace, '{');
    let members = parse_class_members(session)?;
    expect_punctuation(session, PunctuationKind::RBrace, '}');
    Ok(ClassBody { members })
}

fn parse_class_members(session: &mut ParseSession<'_>) -> Result<Vec<ClassMember>, ParserError> {
    let mut members = Vec::new();
    loop {
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
            || session.cursor.is_eof()
        {
            break;
        }
        let constructor_checkpoint = session.checkpoint();
        let constructor_modifiers = parse_modifier_list(session)?;
        // M4 slice 2b：deinit 块（init 同款的文本槽位词——不做硬关键字，
        // 与 init 先例一致；R4 的 "deinit 拼写" 裁定由此兑现）
        if session.cursor.check(&TokenKind::Identifier)
            && ast::token::lookup_slot_word(session.token_text(session.cursor.peek()))
                == Some(ast::token::SlotWord::Deinit)
        {
            session.cursor.advance();
            members.push(ClassMember::Deinit(parse_block(session)?));
            continue;
        }
        if session.cursor.check(&TokenKind::Identifier)
            && session.token_text(session.cursor.peek()) == "init"
        {
            session.cursor.advance();
            if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LParen))
            {
                let params = parse_parameter_list(session)?;
                let body = parse_block(session)?;
                members.push(ClassMember::Constructor(Box::new(ConstructorDecl {
                    modifiers: constructor_modifiers,
                    params,
                    body,
                })));
            } else {
                members.push(ClassMember::Init(parse_block(session)?));
            }
            continue;
        }
        session.rollback(&constructor_checkpoint);
        if matches!(session.cursor.peek().kind, TokenKind::Identifier)
            && matches!(session.token_text(session.cursor.peek()), "get" | "set")
        {
            let getter = if session.token_text(session.cursor.peek()) == "get" {
                session.cursor.advance();
                Some(parse_block(session)?)
            } else {
                None
            };
            let setter = if session.cursor.check(&TokenKind::Identifier)
                && session.token_text(session.cursor.peek()) == "set"
            {
                session.cursor.advance();
                Some(parse_block(session)?)
            } else {
                None
            };
            members.push(ClassMember::Accessor(Box::new(AccessorDecl {
                getter,
                setter,
            })));
            continue;
        }
        members.push(ClassMember::Declaration(Box::new(parse_top_level(
            session,
        )?)));
    }
    Ok(members)
}

fn parse_struct_fields(session: &mut ParseSession<'_>) -> Result<Vec<StructField>, ParserError> {
    let mut fields = Vec::new();
    loop {
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
            || session.cursor.is_eof()
        {
            break;
        }
        // M3：body 字段可带硬关键字 val/var（字段可变性三态）
        let mutable = if session.cursor.check(&TokenKind::Keyword(HardKeyword::Val)) {
            session.cursor.advance();
            Some(false)
        } else if session.cursor.check(&TokenKind::Keyword(HardKeyword::Var)) {
            session.cursor.advance();
            Some(true)
        } else {
            None
        };
        let modifiers = parse_modifier_list(session)?;
        let name = parse_identifier(session).0;
        expect_punctuation(session, PunctuationKind::Colon, ':');
        let ty = parse_type(session)?;
        fields.push(StructField {
            modifiers,
            mutable,
            name,
            ty,
        });
        session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma));
    }
    Ok(fields)
}

fn parse_enum_variants(session: &mut ParseSession<'_>) -> Result<Vec<EnumVariant>, ParserError> {
    let mut variants = Vec::new();
    loop {
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
            || session.cursor.is_eof()
        {
            break;
        }
        let name = parse_identifier(session).0;
        let args = if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::LParen))
        {
            parse_parameter_list(session)?
        } else {
            Vec::new()
        };
        variants.push(EnumVariant { name, args });
        session
            .cursor
            .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma));
        session.cursor.skip_nl();
    }
    Ok(variants)
}

fn parse_trait_members(session: &mut ParseSession<'_>) -> Result<Vec<TraitMember>, ParserError> {
    let mut members = Vec::new();
    loop {
        session.cursor.skip_nl();
        if session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
            || session.cursor.is_eof()
        {
            break;
        }
        if session.cursor.check(&TokenKind::Keyword(HardKeyword::Type))
            && session
                .cursor
                .peek_ahead(1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let bound = if session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::Colon))
            {
                Some(parse_type(session)?)
            } else {
                None
            };
            let default = if session
                .cursor
                .bump_if(&TokenKind::Operator(OperatorKind::Eq))
            {
                Some(parse_type(session)?)
            } else {
                None
            };
            // Redundant `;` is accepted on bodyless trait members, same as `FunSignature`.
            session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon));
            members.push(TraitMember::AssociatedType(AssociatedTypeDecl {
                name,
                bound,
                default,
            }));
            continue;
        }
        let checkpoint = session.checkpoint();
        if at_fun_signature(session)
            && let Ok(signature) = parse_fun_signature(session)
            && !signature_has_body(session)
        {
            session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon));
            members.push(TraitMember::Signature(signature));
            continue;
        }
        session.rollback(&checkpoint);
        if matches!(
            session.cursor.peek().kind,
            TokenKind::Keyword(HardKeyword::Fun)
        ) {
            members.push(TraitMember::Fun(Box::new(parse_fun_decl(session)?)));
        } else {
            members.push(TraitMember::Declaration(Box::new(parse_top_level(
                session,
            )?)));
        }
    }
    Ok(members)
}

pub(crate) fn expect_keyword(session: &mut ParseSession<'_>, expected: HardKeyword, display: &str) {
    if !session.cursor.check(&TokenKind::Keyword(expected)) {
        let span = session.cursor.peek().span;
        session.emit_unique(DiagCode::ExpectedToken, span, || {
            format!("expected {display}")
        });
    } else {
        session.cursor.advance();
    }
}

fn expect_punctuation(session: &mut ParseSession<'_>, expected: PunctuationKind, display: char) {
    if !session.cursor.check(&TokenKind::Punctuation(expected)) {
        let span = session.cursor.peek().span;
        session.emit_unique(DiagCode::ExpectedToken, span, || {
            format!("expected '{display}'")
        });
    } else {
        session.cursor.advance();
    }
}

fn recover_top_level(session: &mut ParseSession<'_>) -> Result<DeclNode, ParserError> {
    let token = session.cursor.advance();
    let id = session.next_node()?;
    let diag = session.emit_unique(DiagCode::UnexpectedToken, token.span, || {
        format!("unexpected {} at top level", token.kind)
    });
    Ok(DeclNode {
        id,
        span: token.span,
        kind: DeclKind::Error(ErrorNode {
            id,
            span: token.span,
            err: diag,
        }),
    })
}
