//! Declaration parser for the frozen v0.3 grammar.

use ast::syntax::decl::*;
use ast::syntax::expr::ExprWrap;
use ast::syntax::modifier::ModifierList;
use ast::syntax::node::Path;
use ast::syntax::node::{DiagCode, ErrorNode, Span, Symbol};
use ast::syntax::ty::TypeWrap;
use ast::token::{HardKeyword, LiteralKind, OperatorKind, PunctuationKind, TokenKind};

use crate::expr::{parse_block, parse_expr, parse_identifier, parse_path};
use crate::pat::parse_pattern;
use crate::ty::{join_span, parse_type, parse_type_params};
use crate::{ParseSession, ParserError};

macro_rules! complete_decl {
    ($session:expr, $start:expr, $kind:expr) => {{
        let kind = $kind;
        let end = $session
            .cursor
            .peek_ahead(0)
            .map(|token| token.span)
            .unwrap_or_default();
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
    let open = session.cursor.advance();
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
    let _ = open;
    Ok(params)
}

fn parse_parameter(session: &mut ParseSession<'_>) -> Result<Param, ParserError> {
    let start = session.cursor.peek().span;
    let mutable = session
        .cursor
        .bump_if(&TokenKind::Keyword(HardKeyword::Mut));
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
        mutable,
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
    let mut bounds = vec![parse_type(session)?];
    while session
        .cursor
        .bump_if(&TokenKind::Operator(OperatorKind::BitAnd))
    {
        bounds.push(parse_type(session)?);
    }
    Ok(WhereConstraint { ty, bounds })
}

pub(crate) fn parse_binding_decl(session: &mut ParseSession<'_>) -> Result<DeclNode, ParserError> {
    let start = session.cursor.peek().span;
    let modifiers = parse_modifier_list(session)?;
    let keyword = session.cursor.advance();
    let immutable = keyword.kind == TokenKind::Keyword(HardKeyword::Val);
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
            TokenKind::Literal(LiteralKind::String) => match session.token_text(&token) {
                "\"cpp\"" => NativeLanguage::Cpp,
                _ => NativeLanguage::C,
            },
            _ => NativeLanguage::C,
        };
        session.cursor.advance();
        let path = session
            .interner
            .intern(session.token_text(session.cursor.peek()));
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
            {
                session.cursor.skip_nl();
                if session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                {
                    break;
                }
                session.cursor.skip_nl();
                if session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                {
                    break;
                }
                members.push(parse_fun_signature(session)?);
                expect_punctuation(session, PunctuationKind::Semicolon, ';');
                if !session
                    .cursor
                    .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
                {
                    break;
                }
            }
            session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::RBrace));
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
    let start = session.cursor.peek().span;
    if session
        .cursor
        .check(&TokenKind::Keyword(HardKeyword::Extern))
    {
        return parse_extern(session, start);
    }
    let modifiers = parse_modifier_list(session)?;
    match session.cursor.peek().kind {
        TokenKind::Keyword(HardKeyword::Fun) => complete_decl!(session, start, {
            let fun = parse_fun_decl(session)?;
            DeclKind::Fun(fun)
        }),
        TokenKind::Keyword(HardKeyword::Val) | TokenKind::Keyword(HardKeyword::Var) => {
            // parse_binding_decl re-reads val/var after parsing modifiers above.
            session.cursor.rollback(&session.checkpoint().0);
            parse_binding_decl(session)
        }
        TokenKind::Keyword(HardKeyword::Class) => complete_decl!(session, start, {
            let _keyword = session.cursor.advance().span;
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            let primary_ctor = parse_primary_constructor(session)?;
            let supertypes = parse_supertypes(session)?;
            let body = parse_class_body(session)?;
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
            expect_punctuation(session, PunctuationKind::LBrace, '{');
            let fields = parse_struct_fields(session)?;
            expect_punctuation(session, PunctuationKind::RBrace, '}');
            DeclKind::Struct(StructDecl {
                modifiers,
                name,
                type_params,
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
        TokenKind::Keyword(HardKeyword::Type) => complete_decl!(session, start, {
            session.cursor.advance();
            let name = parse_identifier(session).0;
            let type_params = parse_type_params(session)?;
            let ty = parse_type(session)?;
            DeclKind::TypeAlias(TypeAliasDecl {
                modifiers,
                name,
                type_params,
                ty,
            })
        }),
        TokenKind::Identifier => match session.token_text(session.cursor.peek()) {
            "actor" => complete_decl!(session, start, {
                session.cursor.advance();
                let name = parse_identifier(session).0;
                let primary_ctor = parse_primary_constructor(session)?;
                let body = parse_class_body(session)?;
                DeclKind::Actor(ActorDecl {
                    name,
                    primary_ctor,
                    body,
                })
            }),
            "aspect" => complete_decl!(session, start, {
                session.cursor.advance();
                let name = parse_identifier(session).0;
                let type_params = parse_type_params(session)?;
                expect_punctuation(session, PunctuationKind::LBrace, '{');
                let mut members = Vec::new();
                while !session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                {
                    session.cursor.skip_nl();
                    if session
                        .cursor
                        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                    {
                        break;
                    }
                    members.push(parse_aspect_member(session)?);
                }
                expect_punctuation(session, PunctuationKind::RBrace, '}');
                DeclKind::Aspect(AspectDecl {
                    name,
                    type_params,
                    members,
                })
            }),
            "provide" => complete_decl!(session, start, {
                session.cursor.advance();
                let name = parse_identifier(session).0;
                expect_punctuation(session, PunctuationKind::LBrace, '{');
                let mut items = Vec::new();
                while !session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                {
                    session.cursor.skip_nl();
                    if session
                        .cursor
                        .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
                    {
                        break;
                    }
                    items.push(parse_provider_item(session)?);
                }
                expect_punctuation(session, PunctuationKind::RBrace, '}');
                DeclKind::Provide(ProvideDecl { name, items })
            }),
            "impl" => complete_decl!(session, start, {
                session.cursor.advance();
                let type_ref = parse_type(session)?;
                expect_keyword(session, HardKeyword::For, "for");
                let trait_ref = parse_type(session)?;
                expect_punctuation(session, PunctuationKind::LBrace, '{');
                let members = parse_class_members(session)?;
                expect_punctuation(session, PunctuationKind::RBrace, '}');
                DeclKind::Impl(ImplDecl {
                    type_ref,
                    trait_ref,
                    members,
                })
            }),
            "macro" => complete_decl!(session, start, {
                session.cursor.advance();
                let name = parse_identifier(session).0;
                expect_punctuation(session, PunctuationKind::LParen, '(');
                let mut params = Vec::new();
                while !session
                    .cursor
                    .check(&TokenKind::Punctuation(PunctuationKind::RParen))
                {
                    params.push(parse_identifier(session).0);
                    if !session
                        .cursor
                        .bump_if(&TokenKind::Punctuation(PunctuationKind::Comma))
                    {
                        break;
                    }
                }
                expect_punctuation(session, PunctuationKind::RParen, ')');
                let body = parse_block(session)?;
                DeclKind::Macro(MacroDecl { name, params, body })
            }),
            "comptime" => complete_decl!(session, start, {
                session.cursor.advance();
                let block = parse_block(session)?;
                DeclKind::Comptime(ComptimeDecl { block })
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

fn parse_extern(session: &mut ParseSession<'_>, start: Span) -> Result<DeclNode, ParserError> {
    session.cursor.advance();
    let language_token = session.cursor.advance();
    let language = session.interner.intern(session.token_text(&language_token));
    let body = if session.cursor.check(&TokenKind::Identifier)
        && session.token_text(session.cursor.peek()) == "namespace"
    {
        session.cursor.advance();
        let name = parse_identifier(session).0;
        expect_punctuation(session, PunctuationKind::LBrace, '{');
        let mut members = Vec::new();
        while !session
            .cursor
            .check(&TokenKind::Punctuation(PunctuationKind::RBrace))
        {
            members.push(parse_fun_signature(session)?);
            expect_punctuation(session, PunctuationKind::Semicolon, ';');
        }
        expect_punctuation(session, PunctuationKind::RBrace, '}');
        ExternBody::Namespace { name, members }
    } else {
        let signature = parse_fun_signature(session)?;
        expect_punctuation(session, PunctuationKind::Semicolon, ';');
        ExternBody::Fun(signature)
    };
    let end = session
        .cursor
        .peek_ahead(0)
        .map(|token| token.span)
        .unwrap_or_default();
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
        Some(
            crate::expr::parse_call_args(session)?
                .0
                .into_iter()
                .map(|arg| arg.value)
                .collect(),
        )
    } else {
        None
    };
    let by = if session.cursor.check(&TokenKind::Identifier)
        && session.token_text(session.cursor.peek()) == "by"
    {
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
        if session.cursor.check(&TokenKind::Identifier)
            && session.token_text(session.cursor.peek()) == "init"
        {
            session.cursor.advance();
            if session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::LParen))
            {
                let modifiers = parse_modifier_list(session)?;
                let params = parse_parameter_list(session)?;
                let body = parse_block(session)?;
                members.push(ClassMember::Constructor(Box::new(ConstructorDecl {
                    modifiers,
                    params,
                    body,
                })));
            } else {
                members.push(ClassMember::Init(parse_block(session)?));
            }
            continue;
        }
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
        let modifiers = parse_modifier_list(session)?;
        let name = parse_identifier(session).0;
        expect_punctuation(session, PunctuationKind::Colon, ':');
        let ty = parse_type(session)?;
        fields.push(StructField {
            modifiers,
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
            members.push(TraitMember::AssociatedType(AssociatedTypeDecl {
                name,
                bound,
                default,
            }));
            continue;
        }
        let checkpoint = session.checkpoint();
        if let Ok(signature) = parse_fun_signature(session)
            && session
                .cursor
                .bump_if(&TokenKind::Punctuation(PunctuationKind::Semicolon))
        {
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

fn parse_aspect_member(session: &mut ParseSession<'_>) -> Result<AspectMember, ParserError> {
    if let TokenKind::Identifier = session.cursor.peek().kind {
        let pointcut = match session.token_text(session.cursor.peek()) {
            "before" => Some(PointcutKind::Before),
            "after" => Some(PointcutKind::After),
            "around" => Some(PointcutKind::Around),
            _ => None,
        };
        if let Some(kind) = pointcut {
            session.cursor.advance();
            let target = parse_pointcut(session)?;
            let body = parse_block(session)?;
            let advice = AdviceDecl {
                kind: match kind {
                    PointcutKind::Before => AdviceKind::Before,
                    PointcutKind::After => AdviceKind::After,
                    PointcutKind::Around => AdviceKind::Around,
                },
                target,
                body,
            };
            return Ok(AspectMember::Advice(advice));
        }
    }
    Ok(AspectMember::Declaration(parse_top_level(session)?))
}

enum PointcutKind {
    Before,
    After,
    Around,
}

fn parse_pointcut(session: &mut ParseSession<'_>) -> Result<Pointcut, ParserError> {
    if let TokenKind::Identifier = session.cursor.peek().kind {
        let checkpoint = session.checkpoint();
        if let Ok(path) = parse_path(session) {
            if !session
                .cursor
                .check(&TokenKind::Punctuation(PunctuationKind::Dot))
            {
                return Ok(Pointcut::Name(path));
            }
            session.rollback(&checkpoint);
        } else {
            session.rollback(&checkpoint);
        }
    }
    let ty = parse_type(session)?;
    expect_punctuation(session, PunctuationKind::Dot, '.');
    let member = if session
        .cursor
        .check(&TokenKind::Operator(OperatorKind::Star))
    {
        session.cursor.advance();
        PointcutMember::Wildcard
    } else {
        PointcutMember::Name(parse_identifier(session).0)
    };
    Ok(Pointcut::Member { ty, member })
}

fn parse_provider_item(session: &mut ParseSession<'_>) -> Result<ProviderItem, ParserError> {
    if let TokenKind::Identifier = session.cursor.peek().kind {
        match session.token_text(session.cursor.peek()) {
            "singleton" | "scoped" | "transient" => {
                let lifetime = match session.token_text(session.cursor.peek()) {
                    "scoped" => ProviderLifetime::Scoped,
                    "transient" => ProviderLifetime::Transient,
                    _ => ProviderLifetime::Singleton,
                };
                session.cursor.advance();
                let ty = crate::ty::parse_union_type(session)?;
                if !session
                    .cursor
                    .bump_if(&TokenKind::Operator(OperatorKind::Arrow))
                {
                    let span = session.cursor.peek().span;
                    session.emit_unique(DiagCode::ExpectedToken, span, || {
                        "expected '->'".to_string()
                    });
                }
                let constructor = parse_expr(session)?;
                return Ok(ProviderItem::Decl(ProviderDecl {
                    lifetime,
                    ty,
                    constructor,
                }));
            }
            "bind" => {
                session.cursor.advance();
                let interface = parse_type(session)?;
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
                let implementation = parse_type(session)?;
                return Ok(ProviderItem::Binding(ProviderBinding {
                    interface,
                    implementation,
                }));
            }
            _ => {}
        }
    }
    let token = session.cursor.advance();
    let id = session.next_node()?;
    let diag = session.emit_unique(DiagCode::UnexpectedToken, token.span, || {
        format!("expected provider item, found {}", token.kind)
    });
    Ok(ProviderItem::Decl(ProviderDecl {
        lifetime: ProviderLifetime::Singleton,
        ty: TypeWrap {
            id,
            span: token.span,
            kind: ast::syntax::ty::TypeKind::Error(ErrorNode {
                id,
                span: token.span,
                err: diag,
            }),
        },
        constructor: ExprWrap::missing(id, token.span),
    }))
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
