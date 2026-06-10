use crate::ast::*;
use crate::Span;

#[derive(Debug, Clone, PartialEq)]
enum SExprToken {
    LParen,
    RParen,
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    Op(String),
}

struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Tokenizer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn tokenize(&mut self) -> Result<Vec<SExprToken>, String> {
        let mut tokens = Vec::new();
        loop {
            match self.next_token()? {
                Some(t) => tokens.push(t),
                None => break,
            }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Option<SExprToken>, String> {
        self.skip_whitespace();
        if self.pos >= self.chars.len() {
            return Ok(None);
        }

        let ch = self.chars[self.pos];
        match ch {
            '(' => {
                self.pos += 1;
                Ok(Some(SExprToken::LParen))
            }
            ')' => {
                self.pos += 1;
                Ok(Some(SExprToken::RParen))
            }
            '"' => self.read_string(),
            '-' if self.is_next_digit() => self.read_number_with_sign(),
            '+' if self.is_next_digit() => self.read_number_with_sign(),
            '0'..='9' => self.read_number(),
            't' | 'f' => self.read_bool_or_ident(),
            c if c.is_alphabetic() || c == '_' => self.read_identifier(),
            _ => self.read_operator(),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn is_next_digit(&self) -> bool {
        self.pos + 1 < self.chars.len() && self.chars[self.pos + 1].is_ascii_digit()
    }

    fn read_string(&mut self) -> Result<Option<SExprToken>, String> {
        self.pos += 1;
        let mut s = String::new();
        while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
            if self.chars[self.pos] == '\\' && self.pos + 1 < self.chars.len() {
                self.pos += 1;
                match self.chars[self.pos] {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    c => s.push(c),
                }
            } else {
                s.push(self.chars[self.pos]);
            }
            self.pos += 1;
        }
        if self.pos >= self.chars.len() {
            return Err("unterminated string".to_string());
        }
        self.pos += 1;
        Ok(Some(SExprToken::Str(s)))
    }

    fn read_number_with_sign(&mut self) -> Result<Option<SExprToken>, String> {
        let sign = if self.chars[self.pos] == '-' { -1i64 } else { 1i64 };
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let has_dot = self.pos < self.chars.len() && self.chars[self.pos] == '.';
        if has_dot {
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            let num_str: String = self.chars[start..self.pos].iter().collect();
            let val: f64 = num_str.parse().map_err(|e| format!("bad float: {}", e))?;
            Ok(Some(SExprToken::Float(sign as f64 * val)))
        } else {
            let num_str: String = self.chars[start..self.pos].iter().collect();
            let val: i64 = num_str.parse().map_err(|e| format!("bad integer: {}", e))?;
            Ok(Some(SExprToken::Integer(sign * val)))
        }
    }

    fn read_number(&mut self) -> Result<Option<SExprToken>, String> {
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let has_dot = self.pos < self.chars.len() && self.chars[self.pos] == '.';
        if has_dot {
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            let num_str: String = self.chars[start..self.pos].iter().collect();
            let val: f64 = num_str.parse().map_err(|e| format!("bad float: {}", e))?;
            Ok(Some(SExprToken::Float(val)))
        } else {
            let num_str: String = self.chars[start..self.pos].iter().collect();
            let val: i64 = num_str.parse().map_err(|e| format!("bad integer: {}", e))?;
            Ok(Some(SExprToken::Integer(val)))
        }
    }

    fn read_bool_or_ident(&mut self) -> Result<Option<SExprToken>, String> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        match word.as_str() {
            "true" => Ok(Some(SExprToken::Bool(true))),
            "false" => Ok(Some(SExprToken::Bool(false))),
            _ => Ok(Some(SExprToken::Ident(word))),
        }
    }

    fn read_identifier(&mut self) -> Result<Option<SExprToken>, String> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        Ok(Some(SExprToken::Ident(word)))
    }

    fn read_operator(&mut self) -> Result<Option<SExprToken>, String> {
        let start = self.pos;
        while self.pos < self.chars.len() && is_op_char(self.chars[self.pos]) {
            self.pos += 1;
        }
        let op: String = self.chars[start..self.pos].iter().collect();
        Ok(Some(SExprToken::Op(op)))
    }
}

fn is_op_char(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '!'
            | '<'
            | '>'
            | '&'
            | '|'
            | '^'
            | '~'
            | '@'
            | '#'
            | '$'
            | '?'
            | ':'
            | '.'
    )
}

struct Parser {
    tokens: Vec<SExprToken>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<SExprToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn parse(&mut self) -> Result<AstNode, String> {
        self.parse_expr()
    }

    fn parse_expr(&mut self) -> Result<AstNode, String> {
        if self.pos >= self.tokens.len() {
            return Err("unexpected end of input".to_string());
        }

        match &self.tokens[self.pos] {
            SExprToken::LParen => self.parse_paren_expr(),
            SExprToken::Integer(v) => {
                let val = *v;
                self.pos += 1;
                Ok(AstNode::Literal(Literal {
                    span: Span::default(),
                    value: LiteralValue::Integer(val),
                }))
            }
            SExprToken::Float(v) => {
                let val = *v;
                self.pos += 1;
                Ok(AstNode::Literal(Literal {
                    span: Span::default(),
                    value: LiteralValue::Float(val),
                }))
            }
            SExprToken::Str(s) => {
                let val = s.clone();
                self.pos += 1;
                Ok(AstNode::Literal(Literal {
                    span: Span::default(),
                    value: LiteralValue::String(val),
                }))
            }
            SExprToken::Bool(b) => {
                let val = *b;
                self.pos += 1;
                Ok(AstNode::Literal(Literal {
                    span: Span::default(),
                    value: LiteralValue::Bool(val),
                }))
            }
            SExprToken::Ident(name) => {
                let name = name.clone();
                self.pos += 1;
                Ok(AstNode::Ident(Ident {
                    span: Span::default(),
                    name,
                }))
            }
            SExprToken::Op(op) => {
                let op = op.clone();
                self.pos += 1;
                let expr = self.parse_expr()?;
                Ok(AstNode::Unary(UnaryExpr {
                    span: Span::default(),
                    op: UnaryOp::from_str(&op).unwrap_or(UnaryOp::Custom(Box::leak(op.into_boxed_str()))),
                    expr: Box::new(expr),
                }))
            }
            SExprToken::RParen => Err("unexpected ')'".to_string()),
        }
    }

    fn parse_paren_expr(&mut self) -> Result<AstNode, String> {
        self.pos += 1;

        if self.pos >= self.tokens.len() {
            return Err("unexpected end of input after '('".to_string());
        }

        let first = self.parse_expr()?;

        if self.pos >= self.tokens.len() {
            return Err("unexpected end of input, expected ')'".to_string());
        }

        if matches!(self.tokens[self.pos], SExprToken::RParen) {
            self.pos += 1;
            return Ok(AstNode::Paren(ParenExpr {
                span: Span::default(),
                expr: Box::new(first),
            }));
        }

        let op_token = self.tokens[self.pos].clone();
        let op = match op_token {
            SExprToken::Op(s) => s,
            SExprToken::Ident(s) => s,
            _ => return Err(format!("expected operator, found {:?}", op_token)),
        };
        self.pos += 1;

        let op_binary = BinaryOp::from_str(&op);
        let op_unary = UnaryOp::from_str(&op);

        let second = self.parse_expr()?;

        if self.pos >= self.tokens.len() {
            return Err("unexpected end of input, expected ')'".to_string());
        }

        if !matches!(self.tokens[self.pos], SExprToken::RParen) {
            return Err(format!("expected ')', found {:?}", self.tokens[self.pos]));
        }
        self.pos += 1;

        if let Some(bin_op) = op_binary {
            Ok(AstNode::Binary(BinaryExpr {
                span: Span::default(),
                left: Box::new(first),
                op: bin_op,
                right: Box::new(second),
            }))
        } else if let Some(un_op) = op_unary {
            Ok(AstNode::Unary(UnaryExpr {
                span: Span::default(),
                op: un_op,
                expr: Box::new(second),
            }))
        } else {
            Ok(AstNode::Binary(BinaryExpr {
                span: Span::default(),
                left: Box::new(first),
                op: BinaryOp::Custom(Box::leak(op.into_boxed_str())),
                right: Box::new(second),
            }))
        }
    }
}

pub fn parse_ast_from_str(input: &str) -> Result<AstNode, String> {
    let mut tokenizer = Tokenizer::new(input);
    let tokens = tokenizer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

impl std::str::FromStr for AstNode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_ast_from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_integer() {
        let ast: AstNode = "42".parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::Integer(42), .. })));
    }

    #[test]
    fn test_parse_float() {
        let ast: AstNode = "3.14".parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::Float(_), .. })));
    }

    #[test]
    fn test_parse_string() {
        let ast: AstNode = r#""hello""#.parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::String(s), .. }) if s == "hello"));
    }

    #[test]
    fn test_parse_bool() {
        let ast: AstNode = "true".parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::Bool(true), .. })));

        let ast: AstNode = "false".parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::Bool(false), .. })));
    }

    #[test]
    fn test_parse_ident() {
        let ast: AstNode = "x".parse().unwrap();
        assert!(matches!(ast, AstNode::Ident(Ident { name, .. }) if name == "x"));
    }

    #[test]
    fn test_parse_binary_expr() {
        let ast: AstNode = "(1 + 2)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::Add, .. })));
    }

    #[test]
    fn test_parse_nested_binary() {
        let ast: AstNode = "(1 + (2 * 3))".parse().unwrap();
        match ast {
            AstNode::Binary(BinaryExpr { left, op: BinaryOp::Add, right, .. }) => {
                assert!(matches!(*left, AstNode::Literal(_)));
                assert!(matches!(*right, AstNode::Binary(_)));
            }
            _ => panic!("expected binary expr"),
        }
    }

    #[test]
    fn test_parse_unary_expr() {
        let ast: AstNode = "(-x)".parse().unwrap();
        match ast {
            AstNode::Paren(ParenExpr { expr, .. }) => {
                assert!(matches!(*expr, AstNode::Unary(UnaryExpr { op: UnaryOp::Neg, .. })));
            }
            _ => panic!("expected paren expr containing unary"),
        }
    }

    #[test]
    fn test_parse_paren_expr() {
        let ast: AstNode = "(7)".parse().unwrap();
        assert!(matches!(ast, AstNode::Paren(_)));
    }

    #[test]
    fn test_roundtrip_binary() {
        let original = "(1 + 2)";
        let ast: AstNode = original.parse().unwrap();
        let display = format!("{}", ast);
        assert_eq!(display, original);
    }

    #[test]
    fn test_roundtrip_nested() {
        let original = "(1 + (2 * 3))";
        let ast: AstNode = original.parse().unwrap();
        let display = format!("{}", ast);
        assert_eq!(display, original);
    }

    #[test]
    fn test_parse_comparison_ops() {
        let ast: AstNode = "(x == y)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::Eq, .. })));

        let ast: AstNode = "(a < b)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::Lt, .. })));

        let ast: AstNode = "(a <= b)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::Le, .. })));
    }

    #[test]
    fn test_parse_logical_ops() {
        let ast: AstNode = "(a && b)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::And, .. })));

        let ast: AstNode = "(a || b)".parse().unwrap();
        assert!(matches!(ast, AstNode::Binary(BinaryExpr { op: BinaryOp::Or, .. })));
    }

    #[test]
    fn test_parse_negative_number() {
        let ast: AstNode = "-42".parse().unwrap();
        assert!(matches!(ast, AstNode::Literal(Literal { value: LiteralValue::Integer(-42), .. })));
    }

    #[test]
    fn test_parse_custom_op() {
        let ast: AstNode = "(a <=> b)".parse().unwrap();
        match ast {
            AstNode::Binary(BinaryExpr { op: BinaryOp::Custom(s), .. }) => {
                assert_eq!(s, "<=>");
            }
            AstNode::Binary(BinaryExpr { left, right, .. }) => {
                let left_name = match *left {
                    AstNode::Ident(Ident { name, .. }) => name,
                    _ => String::new(),
                };
                let right_name = match *right {
                    AstNode::Ident(Ident { name, .. }) => name,
                    _ => String::new(),
                };
                panic!("expected custom op <=>, got left={}, right={}", left_name, right_name);
            }
            _ => panic!("expected binary expr with custom op"),
        }
    }
}
