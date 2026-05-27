use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol};
use starry_ast::{Token, TokenKind, TokenStream};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseTreeNode {
    NonTerminal {
        name: String,
        children: Vec<ParseTreeNode>,
    },
    Terminal {
        token: Token,
    },
    Epsilon,
}

impl ParseTreeNode {
    pub fn non_terminal(name: String, children: Vec<ParseTreeNode>) -> Self {
        ParseTreeNode::NonTerminal { name, children }
    }

    pub fn terminal(token: Token) -> Self {
        ParseTreeNode::Terminal { token }
    }

    pub fn epsilon() -> Self {
        ParseTreeNode::Epsilon
    }

    pub fn print(&self, indent: usize) {
        let spaces = "  ".repeat(indent);
        match self {
            ParseTreeNode::NonTerminal { name, children } => {
                println!("{}{}", spaces, name);
                for child in children {
                    child.print(indent + 1);
                }
            }
            ParseTreeNode::Terminal { token } => {
                println!("{}{}", spaces, token);
            }
            ParseTreeNode::Epsilon => {
                println!("{}ε", spaces);
            }
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    pub fn new(message: String, line: usize, column: usize) -> Self {
        ParseError {
            message,
            line,
            column,
        }
    }
}

pub struct Parser {
    cfg: ContextFreeGrammar,
    stream: TokenStream,
}

impl Parser {
    pub fn new(cfg: ContextFreeGrammar, stream: TokenStream) -> Self {
        Parser { cfg, stream }
    }

    pub fn parse(&mut self) -> Result<ParseTreeNode, ParseError> {
        let start_symbol = self.cfg.start_symbol;
        self.parse_non_terminal(start_symbol)
    }

    fn parse_non_terminal(&mut self, non_terminal: NonTerminalId) -> Result<ParseTreeNode, ParseError> {
        let name = self.cfg.get_non_terminal_name(non_terminal).to_string();
        let productions: Vec<Vec<Symbol>> = self.cfg.get_productions_for(non_terminal)
            .into_iter()
            .map(|p| p.rhs.clone())
            .collect();

        for rhs_symbols in productions {
            let saved_position = self.stream.position();
            
            match self.try_production(rhs_symbols) {
                Ok(children) => {
                    return Ok(ParseTreeNode::non_terminal(name, children));
                }
                Err(_) => {
                    self.stream.set_position(saved_position);
                }
            }
        }

        let token = self.stream.peek();
        Err(ParseError::new(
            format!("Failed to parse non-terminal: {}", name),
            token.map_or(0, |t| t.line),
            token.map_or(0, |t| t.column),
        ))
    }

    fn try_production(&mut self, symbols: Vec<Symbol>) -> Result<Vec<ParseTreeNode>, ParseError> {
        let mut children = Vec::new();

        for symbol in symbols {
            match symbol {
                Symbol::NonTerminal(nt_id) => {
                    let node = self.parse_non_terminal(nt_id)?;
                    children.push(node);
                }
                Symbol::Terminal(term_id) => {
                    let expected = self.cfg.get_terminal_name(term_id).to_string();
                    let token = self.match_terminal(&expected)?;
                    children.push(ParseTreeNode::terminal(token));
                }
                Symbol::Epsilon => {
                    children.push(ParseTreeNode::epsilon());
                }
            }
        }

        Ok(children)
    }

    fn match_terminal(&mut self, expected: &str) -> Result<Token, ParseError> {
        let token = self.stream.peek().ok_or_else(|| {
            ParseError::new("Unexpected end of input".to_string(), 0, 0)
        })?;

        let matches = match &token.kind {
            TokenKind::Identifier(name) => name == expected,
            TokenKind::Keyword(kw) => kw == expected,
            TokenKind::Operator(op) => op == expected,
            TokenKind::Delimiter(del) => del == expected,
            TokenKind::Integer(_) => {
                expected.eq_ignore_ascii_case("NUM") || expected == "INTEGER"
            }
            TokenKind::Float(_) => {
                expected.eq_ignore_ascii_case("NUM") || expected == "FLOAT"
            }
            TokenKind::StringLiteral(_) => expected == "STRING",
            _ => false,
        };

        if matches {
            Ok(self.stream.consume().unwrap())
        } else {
            Err(ParseError::new(
                format!("Expected '{}', found '{}'", expected, token),
                token.line,
                token.column,
            ))
        }
    }
}

pub struct ParserBuilder {
    cfg: Option<ContextFreeGrammar>,
}

impl ParserBuilder {
    pub fn new() -> Self {
        ParserBuilder { cfg: None }
    }

    pub fn with_grammar(mut self, cfg: ContextFreeGrammar) -> Self {
        self.cfg = Some(cfg);
        self
    }

    pub fn with_grammar_from_str(mut self, input: &str) -> Result<Self, String> {
        self.cfg = Some(ContextFreeGrammar::parse(input)?);
        Ok(self)
    }

    pub fn build(self, stream: TokenStream) -> Result<Parser, String> {
        let cfg = self.cfg.ok_or("No grammar provided")?;
        Ok(Parser::new(cfg, stream))
    }
}

impl Default for ParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starry_ast::TokenStreamBuilder;

    fn create_simple_grammar() -> ContextFreeGrammar {
        let input = r#"
            Expr -> Term + Expr | Term
            Term -> num
        "#;
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_parser_simple() {
        let cfg = create_simple_grammar();
        
        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let mut parser = Parser::new(cfg, stream);
        let result = parser.parse();
        
        assert!(result.is_ok());
        let tree = result.unwrap();
        tree.print(0);
    }

    #[test]
    fn test_parser_addition() {
        let cfg = create_simple_grammar();
        
        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        let stream = builder.build();

        let mut parser = Parser::new(cfg, stream);
        let result = parser.parse();
        
        assert!(result.is_ok());
        let tree = result.unwrap();
        tree.print(0);
    }

    #[test]
    fn test_parser_builder() {
        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let parser = ParserBuilder::new()
            .with_grammar_from_str("Expr -> num")
            .unwrap()
            .build(stream)
            .unwrap();

        assert!(parser.cfg.non_terminals.len() > 0);
    }
}
