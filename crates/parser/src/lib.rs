//! Starry parser：递归下降 + Pratt 表达式解析（v1 骨架）

pub mod cursor;
pub mod expr;

use ast::interner::Interner;
use ast::syntax::node::{DiagCollector, Diagnostic, NodeIdGen, SourceFile};
use ast::token::Span;
use lex::lexer::Lexer;

pub struct ParseOutput {
    pub ast: SourceFile,
    pub interner: Interner,
    pub diagnostics: Vec<Diagnostic>,
    pub node_count: u32,
}

pub fn parse_source(source: &str) -> Result<ParseOutput, lex::LexerError> {
    let lexer = Lexer::build(vec![
        ("[a-zA-Z_][a-zA-Z0-9_]*", ast::token::TokenKind::Identifier),
        ("[0-9]+", ast::token::TokenKind::Literal(ast::token::LiteralKind::Integer)),
        ("\\+", ast::token::TokenKind::Operator(ast::token::OperatorKind::Plus)),
        ("-", ast::token::TokenKind::Operator(ast::token::OperatorKind::Minus)),
        ("\\*", ast::token::TokenKind::Operator(ast::token::OperatorKind::Star)),
    ])?;
    let tokens = lexer.tokenize(source);
    let mut interner = Interner::new();
    let mut id_gen = NodeIdGen::new();
    let mut diags = DiagCollector::new();
    let mut cursor = cursor::TokenCursor::new(tokens);
    cursor.skip_nl();

    let node_count = id_gen.count();
    let diagnostics = diags.all().to_vec();
    Ok(ParseOutput {
        ast: SourceFile { id: NodeIdGen::new().next(), span: Span::new(0, 0), items: Vec::new() },
        interner,
        diagnostics,
        node_count,
    })
}
