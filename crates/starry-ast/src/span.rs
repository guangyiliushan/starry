use crate::Token;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Span { start, end }
    }

    pub fn from_positions(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Self {
        Span {
            start: Position::new(start_line, start_col),
            end: Position::new(end_line, end_col),
        }
    }

    pub fn from_token(token: &Token) -> Self {
        Span {
            start: Position::new(token.line, token.column),
            end: Position::new(token.line, token.column + token.lexeme.len()),
        }
    }

    pub fn merge(first: &Span, last: &Span) -> Self {
        Span {
            start: first.start,
            end: last.end,
        }
    }

    pub fn merge_opt(spans: &[Span]) -> Self {
        if spans.is_empty() {
            return Span::default();
        }
        Span {
            start: spans[0].start,
            end: spans[spans.len() - 1].end,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start.line == self.end.line {
            write!(
                f,
                "{}:{}-{}",
                self.start.line, self.start.col, self.end.col
            )
        } else {
            write!(
                f,
                "{}:{}-{}:{}",
                self.start.line, self.start.col, self.end.line, self.end.col
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    pub fn new(line: usize, col: usize) -> Self {
        Position { line, col }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
