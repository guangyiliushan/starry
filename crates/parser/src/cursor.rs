//! TokenCursor：带 checkpoint/backtrack 的 token 流游标

use ast::token::{Span, Token, TokenKind};

/// 游标检查点（用于投机解析回退）
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pos: usize,
}

pub struct TokenCursor {
    tokens: Vec<Token>,
    pos: usize,
}

impl TokenCursor {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint { pos: self.pos }
    }

    pub fn rollback(&mut self, cp: &Checkpoint) {
        self.pos = cp.pos;
    }

    pub fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| self.tokens.last().expect("eof"))
    }

    pub fn peek_ahead(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    pub fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or_else(|| self.tokens.last().expect("eof").clone());
        if self.pos < self.tokens.len() { self.pos += 1; }
        tok
    }

    pub fn bump_if(&mut self, kind: &TokenKind) -> bool {
        if self.peek().kind == *kind { self.advance(); true } else { false }
    }

    pub fn check(&self, kind: &TokenKind) -> bool {
        self.peek().kind == *kind
    }

    pub fn is_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    pub fn skip_nl(&mut self) {
        while self.check(&TokenKind::Nl) { self.advance(); }
    }
}
