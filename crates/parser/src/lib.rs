//! Starry parser：递归下降 + Pratt 表达式解析。
//!
//! 语法错误是 `ParseOutput.diagnostics`；`ParserError` 只表示 lexer 构建、
//! source 超出 span 表达能力或 NodeId 耗尽等机器/resource 失败。

mod cursor;
mod decl;
mod expr;
mod pat;
mod stmt;
mod ty;

use ast::interner::Interner;
use ast::syntax::node::{DiagCode, DiagCollector, DiagId, NodeId, NodeIdGen, SourceFile};
use ast::token::{Span, Token, TokenKind};
use lex::lexer::Lexer;

use crate::cursor::TokenCursor;

pub use ast::syntax::ParseOutput;

#[derive(Debug)]
pub enum ParserError {
    Lexer(lex::LexerError),
    SourceTooLarge { len: usize },
    NodeIdExhausted,
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lexer(error) => write!(f, "lexer build failed: {error}"),
            Self::SourceTooLarge { len } => {
                write!(f, "source length {len} exceeds u32 byte spans")
            }
            Self::NodeIdExhausted => write!(f, "AST NodeId space exhausted"),
        }
    }
}

impl std::error::Error for ParserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lexer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<lex::LexerError> for ParserError {
    fn from(value: lex::LexerError) -> Self {
        Self::Lexer(value)
    }
}

#[derive(Debug)]
pub struct Parser {
    lexer: Lexer,
}

impl Parser {
    pub fn new() -> Result<Self, ParserError> {
        Ok(Self {
            lexer: Lexer::starry()?,
        })
    }

    pub fn parse(&self, source: &str) -> Result<ParseOutput, ParserError> {
        let source_len = u32::try_from(source.len())
            .map_err(|_| ParserError::SourceTooLarge { len: source.len() })?;
        let tokens = self.lexer.tokenize(source);
        let mut session = ParseSession::new(source, tokens, source_len, &self.lexer);
        let ast = session.parse_compilation_unit()?;
        Ok(ParseOutput {
            ast,
            interner: session.interner,
            diagnostics: session.diags.all().to_vec(),
            node_count: session.id_gen.count(),
        })
    }
}

pub fn parse_source(source: &str) -> Result<ParseOutput, ParserError> {
    Parser::new()?.parse(source)
}

pub(crate) struct ParseSession<'src> {
    pub(crate) source: &'src str,
    pub(crate) source_len: u32,
    pub(crate) cursor: TokenCursor,
    pub(crate) interner: Interner,
    pub(crate) id_gen: NodeIdGen,
    pub(crate) diags: DiagCollector,
    pub(crate) depth: usize,
    pub(crate) lexer: &'src Lexer,
}

impl<'src> ParseSession<'src> {
    fn new(source: &'src str, tokens: Vec<Token>, source_len: u32, lexer: &'src Lexer) -> Self {
        Self {
            source,
            source_len,
            cursor: TokenCursor::new(tokens, source_len),
            interner: Interner::new(),
            id_gen: NodeIdGen::new(),
            diags: DiagCollector::new(),
            depth: 0,
            lexer,
        }
    }

    /// 对源码切片做词法分析，并把 token span 平移回全文坐标。
    pub(crate) fn tokenize_range(&self, start: usize, end: usize) -> Vec<Token> {
        let inner = &self.source[start..end];
        self.lexer
            .tokenize(inner)
            .into_iter()
            .map(|mut token| {
                token.span = Span::new(
                    token.span.start + start as u32,
                    token.span.end + start as u32,
                );
                token
            })
            .collect()
    }

    fn parse_compilation_unit(&mut self) -> Result<SourceFile, ParserError> {
        let id = self.next_node()?;
        self.cursor.skip_nl();
        let package = self.parse_named_prefix(ast::token::HardKeyword::Package)?;
        let mut items = Vec::new();
        let mut seen_top_level = false;
        while !self.cursor.is_eof() {
            if self.cursor.check(&TokenKind::Nl) {
                self.cursor.advance();
                continue;
            }
            let item = match self.cursor.peek().kind {
                TokenKind::Keyword(ast::token::HardKeyword::Import) => {
                    let import_span = self.cursor.peek().span;
                    if seen_top_level {
                        self.emit_unique(DiagCode::UnexpectedToken, import_span, || {
                            "imports must precede top-level declarations".to_string()
                        });
                    }
                    let (import, start) = decl::parse_import(self)?;
                    let end = self.cursor.previous_span();
                    Ok(ast::syntax::decl::DeclNode {
                        id: self.next_node()?,
                        span: ast::token::Span::new(start.start, end.end),
                        kind: ast::syntax::decl::DeclKind::Import(import),
                    })
                }
                TokenKind::Keyword(ast::token::HardKeyword::Val)
                | TokenKind::Keyword(ast::token::HardKeyword::Var) => {
                    seen_top_level = true;
                    decl::parse_binding_decl(self)
                }
                _ => {
                    seen_top_level = true;
                    decl::parse_top_level(self)
                }
            }?;
            items.push(item);
            self.finish_top_level();
        }
        Ok(SourceFile {
            id,
            span: Span::new(0, self.source_len),
            package,
            items,
        })
    }

    fn parse_named_prefix(
        &mut self,
        keyword: ast::token::HardKeyword,
    ) -> Result<Option<ast::syntax::node::Path>, ParserError> {
        if !self.cursor.check(&TokenKind::Keyword(keyword)) {
            return Ok(None);
        }
        self.cursor.advance();
        let path = crate::expr::parse_path(self)?;
        self.cursor.skip_nl();
        Ok(Some(path))
    }

    fn finish_top_level(&mut self) {
        self.cursor.skip_nl();
    }

    pub(crate) fn next_node(&mut self) -> Result<NodeId, ParserError> {
        self.id_gen
            .try_allocate()
            .ok_or(ParserError::NodeIdExhausted)
    }

    pub(crate) fn emit_unique(
        &mut self,
        code: DiagCode,
        span: Span,
        message: impl FnOnce() -> String,
    ) -> DiagId {
        if let Some(id) = self.diags.find_unique(span, code) {
            return id;
        }
        let message = message();
        self.diags
            .emit(ast::syntax::node::Diagnostic::error(code, span, message))
    }

    pub(crate) fn checkpoint(&self) -> (crate::cursor::Checkpoint, usize) {
        (self.cursor.checkpoint(), self.diags.checkpoint())
    }

    pub(crate) fn rollback(&mut self, checkpoint: &(crate::cursor::Checkpoint, usize)) {
        self.cursor.rollback(&checkpoint.0);
        self.diags.rollback(checkpoint.1);
    }

    pub(crate) fn token_text(&self, token: &Token) -> &'src str {
        token.text(self.source)
    }

    /// Shared recursion budget. `None` means the caller should emit a
    /// depth-limit diagnostic and recover; `Err` remains a resource failure.
    pub(crate) fn try_with_depth<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, ParserError>,
    ) -> Result<Option<T>, ParserError> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Ok(None);
        }
        self.depth += 1;
        let result = parse(self).map(Some);
        self.depth -= 1;
        result
    }
}

pub(crate) const MAX_PARSE_DEPTH: usize = 50;
