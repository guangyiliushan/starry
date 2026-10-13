//! TokenCursor：token 不可变流上的 checkpoint/backtrack 游标。

use std::collections::VecDeque;

use ast::token::{OperatorKind, Span, Token, TokenKind};

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pos: usize,
    pending_len: usize,
}

pub struct TokenCursor {
    tokens: Vec<Token>,
    pos: usize,
    pending: VecDeque<Token>,
}

impl TokenCursor {
    /// Lexer 输出不包含 EOF；cursor 负责追加 canonical EOF。
    pub fn new(mut tokens: Vec<Token>, source_len: u32) -> Self {
        debug_assert!(!tokens.iter().any(|token| token.kind == TokenKind::Eof));
        tokens.push(Token::new(
            TokenKind::Eof,
            Span::new(source_len, source_len),
        ));
        Self {
            tokens,
            pos: 0,
            pending: VecDeque::new(),
        }
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            pending_len: self.pending.len(),
        }
    }

    pub fn rollback(&mut self, checkpoint: &Checkpoint) {
        self.pos = checkpoint.pos;
        self.pending.truncate(checkpoint.pending_len);
    }

    pub fn peek(&self) -> &Token {
        if let Some(token) = self.pending.front() {
            token
        } else {
            &self.tokens[self.pos.min(self.tokens.len() - 1)]
        }
    }

    pub fn peek_ahead(&self, n: usize) -> Option<&Token> {
        if n < self.pending.len() {
            self.pending.get(n)
        } else {
            self.tokens.get(self.pos + n - self.pending.len())
        }
    }

    pub fn advance(&mut self) -> Token {
        if let Some(token) = self.pending.pop_front() {
            return token;
        }
        let token = self.tokens[self.pos.min(self.tokens.len() - 1)];
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    pub fn bump_if(&mut self, kind: &TokenKind) -> bool {
        if self.peek().kind == *kind {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn check(&self, kind: &TokenKind) -> bool {
        self.peek().kind == *kind
    }

    pub fn is_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    pub fn skip_nl(&mut self) {
        while self.check(&TokenKind::Nl) {
            self.advance();
        }
    }

    pub fn previous_span(&self) -> Span {
        if self.pending.is_empty() {
            self.tokens
                .get(self.pos.saturating_sub(1))
                .map(|token| token.span)
                .unwrap_or_default()
        } else {
            Span::default()
        }
    }

    /// 在类型上下文把 `>>` / `>>=` / `>=` 递归拆成原子 close/equality token。
    /// 只在 generic close 可能被 lexer 最长匹配吞掉的位置调用。
    pub fn split_type_close(&mut self) -> bool {
        if self.pending.front().is_some() || self.pos >= self.tokens.len() {
            return false;
        }
        let token = self.tokens[self.pos];
        let span = token.span;
        let mut parts = VecDeque::new();
        match token.kind {
            TokenKind::Operator(OperatorKind::Shr) => {
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Gt), span.start));
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Gt), span.start + 1));
            }
            TokenKind::Operator(OperatorKind::ShrEq) => {
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Gt), span.start));
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Gt), span.start + 1));
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Eq), span.start + 2));
            }
            TokenKind::Operator(OperatorKind::GtEq) => {
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Gt), span.start));
                parts.push_back(atom(TokenKind::Operator(OperatorKind::Eq), span.start + 1));
            }
            _ => return false,
        }
        debug_assert!(parts.back().is_some_and(|part| part.span.end == span.end));
        self.pos += 1;
        self.pending = parts;
        true
    }
}

fn atom(kind: TokenKind, start: u32) -> Token {
    Token::new(kind, Span::new(start, start + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(kind: TokenKind, start: u32, end: u32) -> Token {
        Token::new(kind, Span::new(start, end))
    }

    #[test]
    fn empty_input_has_stable_canonical_eof() {
        let mut cursor = TokenCursor::new(Vec::new(), 0);
        assert!(cursor.is_eof());
        assert_eq!(cursor.advance().kind, TokenKind::Eof);
        assert_eq!(cursor.advance().kind, TokenKind::Eof);
        assert_eq!(cursor.pos, 0);
    }

    #[test]
    fn checkpoint_restores_cursor_and_pending_length() {
        let mut cursor = TokenCursor::new(
            vec![
                token(TokenKind::Identifier, 0, 1),
                token(TokenKind::Nl, 1, 2),
            ],
            2,
        );
        cursor.advance();
        cursor.skip_nl();
        let checkpoint = cursor.checkpoint();
        cursor.advance();
        cursor.rollback(&checkpoint);
        assert!(cursor.is_eof());
    }

    #[test]
    fn split_shr_into_one_byte_atomic_tokens() {
        let mut cursor = TokenCursor::new(
            vec![token(TokenKind::Operator(OperatorKind::Shr), 10, 12)],
            12,
        );
        assert!(cursor.split_type_close());
        assert_eq!(cursor.peek().kind, TokenKind::Operator(OperatorKind::Gt));
        assert_eq!(cursor.peek().span, Span::new(10, 11));
        cursor.advance();
        assert_eq!(cursor.peek().kind, TokenKind::Operator(OperatorKind::Gt));
        assert_eq!(cursor.peek().span, Span::new(11, 12));
        cursor.advance();
        assert!(cursor.is_eof());
    }

    #[test]
    fn split_shr_eq_and_gt_eq_recursively() {
        let mut cursor = TokenCursor::new(
            vec![token(TokenKind::Operator(OperatorKind::ShrEq), 0, 3)],
            3,
        );
        assert!(cursor.split_type_close());
        assert_eq!(cursor.advance().span, Span::new(0, 1));
        assert_eq!(cursor.advance().span, Span::new(1, 2));
        assert_eq!(cursor.advance().kind, TokenKind::Operator(OperatorKind::Eq));
        assert!(cursor.is_eof());

        let mut cursor = TokenCursor::new(
            vec![token(TokenKind::Operator(OperatorKind::GtEq), 4, 6)],
            6,
        );
        assert!(cursor.split_type_close());
        assert_eq!(cursor.advance().kind, TokenKind::Operator(OperatorKind::Gt));
        assert_eq!(cursor.advance().kind, TokenKind::Operator(OperatorKind::Eq));
        assert!(cursor.is_eof());
    }
}
