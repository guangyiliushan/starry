use crate::analysis::{ParsingTable, ParsingTableBuilder};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use crate::parser::ParseTreeNode;
use starry_ast::{TokenKind, TokenStream};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum StackSymbol {
    NonTerminal(NonTerminalId),
    Terminal(TerminalId),
    EndMarker,
}

#[derive(Debug)]
pub struct LL1ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub expected: Vec<String>,
    pub found: Option<String>,
}

impl LL1ParseError {
    pub fn new(message: String, line: usize, column: usize) -> Self {
        LL1ParseError {
            message,
            line,
            column,
            expected: Vec::new(),
            found: None,
        }
    }

    pub fn with_expected(mut self, expected: Vec<String>) -> Self {
        self.expected = expected;
        self
    }

    pub fn with_found(mut self, found: Option<String>) -> Self {
        self.found = found;
        self
    }
}

pub struct LL1Parser {
    cfg: ContextFreeGrammar,
    table: ParsingTable,
    stream: TokenStream,
    stack: VecDeque<StackSymbol>,
    trace: bool,
}

impl LL1Parser {
    pub fn new(cfg: ContextFreeGrammar, table: ParsingTable, stream: TokenStream) -> Self {
        let mut parser = LL1Parser {
            cfg,
            table,
            stream,
            stack: VecDeque::new(),
            trace: false,
        };
        parser.initialize_stack();
        parser
    }

    pub fn from_grammar(cfg: ContextFreeGrammar, stream: TokenStream) -> Result<Self, String> {
        let table = ParsingTableBuilder::build_from_grammar(&cfg)
            .map_err(|conflicts| {
                format!(
                    "Grammar is not LL(1): {} conflict(s) detected",
                    conflicts.len()
                )
            })?;
        Ok(Self::new(cfg, table, stream))
    }

    fn initialize_stack(&mut self) {
        self.stack.clear();
        self.stack.push_front(StackSymbol::EndMarker);
        self.stack.push_front(StackSymbol::NonTerminal(self.cfg.start_symbol));
    }

    pub fn enable_trace(&mut self, enable: bool) {
        self.trace = enable;
    }

    pub fn parse(&mut self) -> Result<ParseTreeNode, LL1ParseError> {
        self.initialize_stack();

        if self.trace {
            println!("=== LL(1) Parsing Trace ===");
            self.print_state("Initial");
        }

        loop {
            let top = self.stack.front().cloned();
            let current_token = self.stream.peek();

            match top {
                Some(StackSymbol::EndMarker) => {
                    match current_token {
                        None => {
                            if self.trace {
                                println!("\nParsing successful!");
                            }
                            return self.build_parse_tree();
                        }
                        Some(token) => {
                            if matches!(token.kind, TokenKind::Eof) {
                                if self.trace {
                                    println!("\nParsing successful!");
                                }
                                return self.build_parse_tree();
                            }
                            return Err(LL1ParseError::new(
                                "Unexpected input after parsing completed".to_string(),
                                token.line,
                                token.column,
                            )
                            .with_found(Some(format!("{:?}", token.kind))));
                        }
                    }
                }

                Some(StackSymbol::Terminal(expected_term)) => {
                    if let Some(token) = current_token {
                        if self.matches_terminal(expected_term, &token.kind) {
                            if self.trace {
                                println!(
                                    "Match terminal: {}",
                                    self.cfg.get_terminal_name(expected_term)
                                );
                            }
                            self.stack.pop_front();
                            self.stream.consume();

                            if self.trace {
                                self.print_state("After match");
                            }
                        } else {
                            return Err(LL1ParseError::new(
                                format!(
                                    "Expected '{}', found '{}'",
                                    self.cfg.get_terminal_name(expected_term),
                                    token
                                ),
                                token.line,
                                token.column,
                            )
                            .with_expected(vec![self.cfg.get_terminal_name(expected_term).to_string()])
                            .with_found(Some(format!("{:?}", token.kind))));
                        }
                    } else {
                        return Err(LL1ParseError::new(
                            format!(
                                "Unexpected end of input, expected '{}'",
                                self.cfg.get_terminal_name(expected_term)
                            ),
                            0,
                            0,
                        )
                        .with_expected(vec![self.cfg.get_terminal_name(expected_term).to_string()]));
                    }
                }

                Some(StackSymbol::NonTerminal(nt)) => {
                    let term_id = self.get_current_terminal_id()?;

                    if let Some(entry) = self.table.get(nt, term_id) {
                        if self.trace {
                            println!(
                                "Apply production: {} -> {}",
                                self.cfg.get_non_terminal_name(entry.production.lhs),
                                self.format_rhs(&entry.production.rhs)
                            );
                        }

                        self.stack.pop_front();

                        for symbol in entry.production.rhs.iter().rev() {
                            match symbol {
                                Symbol::NonTerminal(nt_id) => {
                                    self.stack.push_front(StackSymbol::NonTerminal(*nt_id));
                                }
                                Symbol::Terminal(term_id) => {
                                    self.stack.push_front(StackSymbol::Terminal(*term_id));
                                }
                                Symbol::Epsilon => {}
                            }
                        }

                        if self.trace {
                            self.print_state("After production");
                        }
                    } else {
                        let token = self.stream.peek();
                        let expected = self.get_expected_terminals(nt);

                        return Err(LL1ParseError::new(
                            format!(
                                "No production for {} at current input",
                                self.cfg.get_non_terminal_name(nt)
                            ),
                            token.map_or(0, |t| t.line),
                            token.map_or(0, |t| t.column),
                        )
                        .with_expected(expected)
                        .with_found(token.map(|t| format!("{:?}", t.kind))));
                    }
                }

                None => {
                    return Err(LL1ParseError::new(
                        "Stack underflow - this should not happen".to_string(),
                        0,
                        0,
                    ));
                }
            }
        }
    }

    fn get_current_terminal_id(&self) -> Result<TerminalId, LL1ParseError> {
        match self.stream.peek() {
            Some(token) => {
                self.terminal_id_from_token(&token.kind)
                    .ok_or_else(|| {
                        LL1ParseError::new(
                            format!("Unknown terminal: {:?}", token.kind),
                            token.line,
                            token.column,
                        )
                    })
            }
            None => Ok(self.table.end_marker()),
        }
    }

    fn terminal_id_from_token(&self, kind: &TokenKind) -> Option<TerminalId> {
        match kind {
            TokenKind::Identifier(name) => self.cfg.terminal_map.get(name).copied(),
            TokenKind::Keyword(kw) => self.cfg.terminal_map.get(kw).copied(),
            TokenKind::Operator(op) => self.cfg.terminal_map.get(op).copied(),
            TokenKind::Delimiter(del) => self.cfg.terminal_map.get(del).copied(),
            TokenKind::Integer(_) => self
                .cfg
                .terminal_map
                .get("NUM")
                .or_else(|| self.cfg.terminal_map.get("num"))
                .copied(),
            TokenKind::Float(_) => self
                .cfg
                .terminal_map
                .get("NUM")
                .or_else(|| self.cfg.terminal_map.get("num"))
                .copied(),
            TokenKind::StringLiteral(_) => self.cfg.terminal_map.get("STRING").copied(),
            TokenKind::Eof => Some(self.table.end_marker()),
            _ => None,
        }
    }

    fn matches_terminal(&self, expected: TerminalId, kind: &TokenKind) -> bool {
        let expected_name = self.cfg.get_terminal_name(expected);

        match kind {
            TokenKind::Identifier(name) => name == expected_name,
            TokenKind::Keyword(kw) => kw == expected_name,
            TokenKind::Operator(op) => op == expected_name,
            TokenKind::Delimiter(del) => del == expected_name,
            TokenKind::Integer(_) | TokenKind::Float(_) => {
                expected_name.eq_ignore_ascii_case("NUM") || expected_name == "num"
            }
            TokenKind::StringLiteral(_) => expected_name == "STRING",
            TokenKind::Eof => expected == self.table.end_marker(),
            _ => false,
        }
    }

    fn get_expected_terminals(&self, nt: NonTerminalId) -> Vec<String> {
        let mut expected = Vec::new();
        for (i, _) in self.cfg.terminals.iter().enumerate() {
            let term_id = TerminalId(i);
            if self.table.contains(nt, term_id) {
                expected.push(self.cfg.get_terminal_name(term_id).to_string());
            }
        }
        if self.table.contains(nt, self.table.end_marker()) {
            expected.push("$".to_string());
        }
        expected
    }

    fn format_rhs(&self, rhs: &[Symbol]) -> String {
        if rhs.is_empty() || (rhs.len() == 1 && matches!(rhs[0], Symbol::Epsilon)) {
            return "ε".to_string();
        }

        rhs.iter()
            .map(|sym| match sym {
                Symbol::NonTerminal(nt) => self.cfg.get_non_terminal_name(*nt).to_string(),
                Symbol::Terminal(t) => self.cfg.get_terminal_name(*t).to_string(),
                Symbol::Epsilon => "ε".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn print_state(&self, label: &str) {
        print!("{}: Stack = [", label);
        let stack_str: Vec<String> = self
            .stack
            .iter()
            .rev()
            .map(|sym| match sym {
                StackSymbol::NonTerminal(nt) => {
                    format!("{}(NT)", self.cfg.get_non_terminal_name(*nt))
                }
                StackSymbol::Terminal(t) => {
                    format!("{}(T)", self.cfg.get_terminal_name(*t))
                }
                StackSymbol::EndMarker => "$".to_string(),
            })
            .collect();
        print!("{}]", stack_str.join(", "));

        if let Some(token) = self.stream.peek() {
            print!(", Input = {:?}", token.kind);
        } else {
            print!(", Input = $");
        }
        println!();
    }

    fn build_parse_tree(&self) -> Result<ParseTreeNode, LL1ParseError> {
        Ok(ParseTreeNode::epsilon())
    }
}

pub struct LL1ParserBuilder {
    cfg: Option<ContextFreeGrammar>,
    trace: bool,
}

impl LL1ParserBuilder {
    pub fn new() -> Self {
        LL1ParserBuilder {
            cfg: None,
            trace: false,
        }
    }

    pub fn with_grammar(mut self, cfg: ContextFreeGrammar) -> Self {
        self.cfg = Some(cfg);
        self
    }

    pub fn with_grammar_from_str(mut self, input: &str) -> Result<Self, String> {
        self.cfg = Some(ContextFreeGrammar::parse(input)?);
        Ok(self)
    }

    pub fn with_trace(mut self, enable: bool) -> Self {
        self.trace = enable;
        self
    }

    pub fn build(self, stream: TokenStream) -> Result<LL1Parser, String> {
        let cfg = self.cfg.ok_or("No grammar provided")?;
        let table = ParsingTableBuilder::build_from_grammar(&cfg).map_err(|conflicts| {
            format!(
                "Grammar is not LL(1): {} conflict(s) detected",
                conflicts.len()
            )
        })?;

        let mut parser = LL1Parser::new(cfg, table, stream);
        parser.enable_trace(self.trace);
        Ok(parser)
    }
}

impl Default for LL1ParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starry_ast::TokenStreamBuilder;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_simple_parse() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> num
        "#);

        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        let stream = builder.build();

        let mut parser = LL1Parser::from_grammar(grammar, stream).unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_parentheses() {
        let grammar = parse_grammar(r#"
            E -> T E'
            E' -> + T E' | ε
            T -> F T'
            T' -> * F T' | ε
            F -> ( E ) | num
        "#);

        let mut builder = TokenStreamBuilder::new();
        builder.add_delimiter("(".to_string());
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        builder.add_delimiter(")".to_string());
        let stream = builder.build();

        let mut parser = LL1Parser::from_grammar(grammar, stream).unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_epsilon() {
        let grammar = parse_grammar(r#"
            S -> a A | ε
            A -> b
        "#);

        let stream = TokenStreamBuilder::new().build();

        let mut parser = LL1Parser::from_grammar(grammar, stream).unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_error() {
        let grammar = parse_grammar(r#"
            S -> a A
            A -> b
        "#);

        let mut builder = TokenStreamBuilder::new();
        builder.add_identifier("x".to_string());
        let stream = builder.build();

        let mut parser = LL1Parser::from_grammar(grammar, stream).unwrap();
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parser_builder() {
        let grammar = parse_grammar("E -> num");

        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let parser = LL1ParserBuilder::new()
            .with_grammar(grammar)
            .build(stream);

        assert!(parser.is_ok());
    }
}
