use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub enum TokenKind {
    Identifier(String),
    Integer(i64),
    Float(f64),
    StringLiteral(String),
    Keyword(String),
    Operator(String),
    Delimiter(String),
    Eof,
    Error(String),
}

impl PartialEq for TokenKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TokenKind::Identifier(a), TokenKind::Identifier(b)) => a == b,
            (TokenKind::Integer(a), TokenKind::Integer(b)) => a == b,
            (TokenKind::Float(a), TokenKind::Float(b)) => a.to_bits() == b.to_bits(),
            (TokenKind::StringLiteral(a), TokenKind::StringLiteral(b)) => a == b,
            (TokenKind::Keyword(a), TokenKind::Keyword(b)) => a == b,
            (TokenKind::Operator(a), TokenKind::Operator(b)) => a == b,
            (TokenKind::Delimiter(a), TokenKind::Delimiter(b)) => a == b,
            (TokenKind::Eof, TokenKind::Eof) => true,
            (TokenKind::Error(a), TokenKind::Error(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for TokenKind {}

impl Hash for TokenKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            TokenKind::Identifier(s) => s.hash(state),
            TokenKind::Integer(i) => i.hash(state),
            TokenKind::Float(f) => f.to_bits().hash(state),
            TokenKind::StringLiteral(s) => s.hash(state),
            TokenKind::Keyword(s) => s.hash(state),
            TokenKind::Operator(s) => s.hash(state),
            TokenKind::Delimiter(s) => s.hash(state),
            TokenKind::Eof => {}
            TokenKind::Error(s) => s.hash(state),
        }
    }
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, line: usize, column: usize) -> Self {
        Token {
            kind,
            lexeme,
            line,
            column,
        }
    }

    pub fn eof(line: usize, column: usize) -> Self {
        Token::new(TokenKind::Eof, String::new(), line, column)
    }

    pub fn identifier(name: String, line: usize, column: usize) -> Self {
        Token::new(TokenKind::Identifier(name.clone()), name, line, column)
    }

    pub fn integer(value: i64, lexeme: String, line: usize, column: usize) -> Self {
        Token::new(TokenKind::Integer(value), lexeme, line, column)
    }

    pub fn keyword(keyword: String, line: usize, column: usize) -> Self {
        Token::new(TokenKind::Keyword(keyword.clone()), keyword, line, column)
    }

    pub fn operator(op: String, line: usize, column: usize) -> Self {
        Token::new(TokenKind::Operator(op.clone()), op, line, column)
    }

    pub fn delimiter(del: String, line: usize, column: usize) -> Self {
        Token::new(TokenKind::Delimiter(del.clone()), del, line, column)
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::Eof)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TokenKind::Identifier(name) => write!(f, "IDENT({})", name),
            TokenKind::Integer(value) => write!(f, "INT({})", value),
            TokenKind::Float(value) => write!(f, "FLOAT({})", value),
            TokenKind::StringLiteral(s) => write!(f, "STRING(\"{}\")", s),
            TokenKind::Keyword(kw) => write!(f, "KW({})", kw),
            TokenKind::Operator(op) => write!(f, "OP({})", op),
            TokenKind::Delimiter(del) => write!(f, "DEL({})", del),
            TokenKind::Eof => write!(f, "EOF"),
            TokenKind::Error(msg) => write!(f, "ERROR({})", msg),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenStream {
    tokens: Vec<Token>,
    position: usize,
}

impl TokenStream {
    pub fn new(tokens: Vec<Token>) -> Self {
        TokenStream {
            tokens,
            position: 0,
        }
    }

    pub fn from_tokens(tokens: Vec<Token>) -> Self {
        Self::new(tokens)
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    pub fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.position).map(|t| &t.kind)
    }

    pub fn advance(&mut self) -> Option<&Token> {
        if self.position < self.tokens.len() {
            self.position += 1;
            self.tokens.get(self.position - 1)
        } else {
            None
        }
    }

    pub fn consume(&mut self) -> Option<Token> {
        if self.position < self.tokens.len() {
            let token = self.tokens[self.position].clone();
            self.position += 1;
            Some(token)
        } else {
            None
        }
    }

    pub fn expect(&mut self, expected: &TokenKind) -> Result<Token, String> {
        match self.peek() {
            Some(token) if &token.kind == expected => {
                Ok(self.consume().unwrap())
            }
            Some(token) => Err(format!(
                "Expected {:?}, found {:?} at line {}, column {}",
                expected, token.kind, token.line, token.column
            )),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn expect_identifier(&mut self) -> Result<String, String> {
        match self.peek_kind() {
            Some(TokenKind::Identifier(name)) => {
                let name = name.clone();
                self.consume();
                Ok(name)
            }
            Some(token) => Err(format!("Expected identifier, found {:?}", token)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn expect_keyword(&mut self, keyword: &str) -> Result<Token, String> {
        match self.peek_kind() {
            Some(TokenKind::Identifier(kw)) | Some(TokenKind::Keyword(kw)) 
                if kw == keyword => {
                Ok(self.consume().unwrap())
            }
            Some(token) => Err(format!("Expected keyword '{}', found {:?}", keyword, token)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn expect_operator(&mut self, op: &str) -> Result<Token, String> {
        match self.peek_kind() {
            Some(TokenKind::Operator(o)) if o == op => {
                Ok(self.consume().unwrap())
            }
            Some(token) => Err(format!("Expected operator '{}', found {:?}", op, token)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn expect_delimiter(&mut self, del: &str) -> Result<Token, String> {
        match self.peek_kind() {
            Some(TokenKind::Delimiter(d)) if d == del => {
                Ok(self.consume().unwrap())
            }
            Some(token) => Err(format!("Expected delimiter '{}', found {:?}", del, token)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len() || self.peek().map_or(true, |t| t.is_eof())
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn set_position(&mut self, pos: usize) {
        self.position = pos.min(self.tokens.len());
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }
}

pub struct TokenStreamBuilder {
    tokens: Vec<Token>,
    current_line: usize,
    current_column: usize,
}

impl TokenStreamBuilder {
    pub fn new() -> Self {
        TokenStreamBuilder {
            tokens: Vec::new(),
            current_line: 1,
            current_column: 1,
        }
    }

    pub fn add_identifier(&mut self, name: String) -> &mut Self {
        let token = Token::identifier(name, self.current_line, self.current_column);
        self.tokens.push(token);
        self.current_column += 1;
        self
    }

    pub fn add_integer(&mut self, value: i64, lexeme: String) -> &mut Self {
        let token = Token::integer(value, lexeme, self.current_line, self.current_column);
        self.tokens.push(token);
        self.current_column += 1;
        self
    }

    pub fn add_keyword(&mut self, keyword: String) -> &mut Self {
        let token = Token::keyword(keyword, self.current_line, self.current_column);
        self.tokens.push(token);
        self.current_column += 1;
        self
    }

    pub fn add_operator(&mut self, op: String) -> &mut Self {
        let token = Token::operator(op, self.current_line, self.current_column);
        self.tokens.push(token);
        self.current_column += 1;
        self
    }

    pub fn add_delimiter(&mut self, del: String) -> &mut Self {
        let token = Token::delimiter(del, self.current_line, self.current_column);
        self.tokens.push(token);
        self.current_column += 1;
        self
    }

    pub fn new_line(&mut self) -> &mut Self {
        self.current_line += 1;
        self.current_column = 1;
        self
    }

    pub fn build(self) -> TokenStream {
        let mut tokens = self.tokens;
        tokens.push(Token::eof(self.current_line, self.current_column));
        TokenStream::new(tokens)
    }
}

impl Default for TokenStreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_creation() {
        let token = Token::identifier("x".to_string(), 1, 1);
        assert_eq!(token.lexeme, "x");
        assert!(matches!(token.kind, TokenKind::Identifier(_)));
    }

    #[test]
    fn test_token_stream_basic() {
        let mut builder = TokenStreamBuilder::new();
        builder.add_identifier("x".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(42, "42".to_string());
        let mut stream = builder.build();

        assert!(!stream.is_at_end());
        
        let token1 = stream.consume().unwrap();
        assert!(matches!(token1.kind, TokenKind::Identifier(_)));
        
        let token2 = stream.consume().unwrap();
        assert!(matches!(token2.kind, TokenKind::Operator(_)));
        
        let token3 = stream.consume().unwrap();
        assert!(matches!(token3.kind, TokenKind::Integer(_)));
        
        assert!(stream.is_at_end());
    }

    #[test]
    fn test_token_stream_expect() {
        let mut builder = TokenStreamBuilder::new();
        builder.add_keyword("if".to_string());
        builder.add_identifier("x".to_string());
        let mut stream = builder.build();

        let result = stream.expect_keyword("if");
        assert!(result.is_ok());
        
        let result = stream.expect_identifier();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "x");
    }
}
