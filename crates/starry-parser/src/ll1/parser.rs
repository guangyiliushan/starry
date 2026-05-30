use super::parsing_table::{ParsingTable, ParsingTableBuilder};
use super::LL1ParserTrait;
use crate::ast_builder::{GrammarAstBuilder, is_tail_production};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use crate::ll1::error::LL1ParseError;
use crate::ll1::stack::{ParseStack, StackSymbol};
use crate::ll1::token_mapper::TokenMapper;
use starry_ast::{AstBuilder, AstNode, TokenKind, TokenStream};

pub struct LL1Parser {
    cfg: ContextFreeGrammar,
    table: ParsingTable,
    stream: TokenStream,
    stack: ParseStack,
    ast_builder: GrammarAstBuilder,
    ast_stack: Vec<AstNode>,
    frame_stack: Vec<usize>,
    trace: bool,
}

impl LL1Parser {
    pub fn new(cfg: ContextFreeGrammar, table: ParsingTable, stream: TokenStream) -> Self {
        let ast_builder = GrammarAstBuilder::new(&cfg);
        let mut parser = LL1Parser {
            cfg,
            table,
            stream,
            stack: ParseStack::new(),
            ast_builder,
            ast_stack: Vec::new(),
            frame_stack: Vec::new(),
            trace: false,
        };
        parser.initialize();
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

    fn initialize(&mut self) {
        self.stack.initialize(self.cfg.start_symbol);
        self.ast_stack.clear();
        self.frame_stack.clear();
    }

    pub fn enable_trace(&mut self, enable: bool) {
        self.trace = enable;
    }

    pub fn parse(&mut self) -> Result<AstNode, LL1ParseError> {
        self.initialize();

        if self.trace {
            println!("LL(1) Parsing Trace:");
            self.print_state("Initial");
        }

        loop {
            let top = self.stack.front_cloned();
            let current_token = self.stream.peek().cloned();

            match top {
                Some(StackSymbol::EndMarker) => {
                    if self.should_accept(current_token.as_ref())? {
                        if self.trace {
                            println!("\nParsing successful!");
                        }
                        return self.finalize_ast();
                    }
                }

                Some(StackSymbol::Terminal(expected_term)) => {
                    self.handle_terminal(expected_term, current_token.as_ref())?;
                }

                Some(StackSymbol::NonTerminal(nt)) => {
                    self.handle_non_terminal(nt)?;
                }

                Some(StackSymbol::ProductionEnd { non_terminal: _, production_index }) => {
                    self.handle_reduce(production_index);
                }

                None => {
                    return Err(LL1ParseError::stack_underflow());
                }
            }
        }
    }

    fn handle_reduce(&mut self, production_index: usize) {
        self.stack.pop_front();
        let frame_pos = self.frame_stack.pop().unwrap();

        let production = &self.cfg.productions[production_index];
        let sem_start = if is_tail_production(production) {
            frame_pos.saturating_sub(1)
        } else {
            frame_pos
        };

        let children: Vec<AstNode> = self.ast_stack.drain(sem_start..).collect();
        let node = self.ast_builder.reduce(production_index, children);
        self.ast_stack.push(node);

        if self.trace {
            self.print_state("After reduce");
        }
    }

    fn should_accept(&self, current_token: Option<&starry_ast::Token>) -> Result<bool, LL1ParseError> {
        match current_token {
            None => Ok(true),
            Some(token) => {
                if matches!(token.kind, TokenKind::Eof) {
                    Ok(true)
                } else {
                    Err(LL1ParseError::unexpected_input(
                        token.line,
                        token.column,
                        format!("{:?}", token.kind),
                    ))
                }
            }
        }
    }

    fn handle_terminal(&mut self, expected_term: TerminalId, current_token: Option<&starry_ast::Token>) -> Result<(), LL1ParseError> {
        if let Some(token) = current_token {
            let mapper = TokenMapper::new(&self.cfg, &self.table);

            if mapper.matches_terminal(expected_term, &token.kind) {
                if self.trace {
                    println!(
                        "Match terminal: {}",
                        self.cfg.get_terminal_name(expected_term)
                    );
                }
                self.stack.pop_front();

                let consumed_token = self.stream.consume().unwrap();
                let node = self.ast_builder.make_leaf(&consumed_token);
                self.ast_stack.push(node);

                if self.trace {
                    self.print_state("After match");
                }
                Ok(())
            } else {
                Err(LL1ParseError::terminal_mismatch(
                    self.cfg.get_terminal_name(expected_term).to_string(),
                    format!("{}", token),
                    token.line,
                    token.column,
                ))
            }
        } else {
            Err(LL1ParseError::unexpected_eof(
                self.cfg.get_terminal_name(expected_term).to_string(),
            ))
        }
    }

    fn handle_non_terminal(&mut self, nt: NonTerminalId) -> Result<(), LL1ParseError> {
        let mapper = TokenMapper::new(&self.cfg, &self.table);
        let term_id = mapper.get_current_terminal_id(&self.stream)?;

        if let Some(entry) = self.table.get(nt, term_id) {
            if self.trace {
                println!(
                    "Apply production: {} -> {}",
                    self.cfg.get_non_terminal_name(entry.production.lhs),
                    self.format_rhs(&entry.production.rhs)
                );
            }

            self.stack.pop_front();

            if entry.production.is_epsilon() {
                let node = self.ast_builder.reduce(entry.production_index, vec![]);
                self.ast_stack.push(node);
            } else {
                self.frame_stack.push(self.ast_stack.len());
                self.stack.push_front(StackSymbol::ProductionEnd {
                    non_terminal: nt,
                    production_index: entry.production_index,
                });
                for symbol in entry.production.rhs.iter().rev() {
                    match symbol {
                        Symbol::NonTerminal(n) => self.stack.push_front(StackSymbol::NonTerminal(*n)),
                        Symbol::Terminal(t) => self.stack.push_front(StackSymbol::Terminal(*t)),
                        Symbol::Epsilon => {}
                    }
                }
            }

            if self.trace {
                self.print_state("After production");
            }

            Ok(())
        } else {
            let token = self.stream.peek();
            let expected = self.get_expected_terminals(nt);

            Err(LL1ParseError::no_production(
                self.cfg.get_non_terminal_name(nt).to_string(),
                token.map_or(0, |t| t.line),
                token.map_or(0, |t| t.column),
                expected,
                token.map(|t| format!("{:?}", t.kind)),
            ))
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
            .iter_rev()
            .map(|sym| match sym {
                StackSymbol::NonTerminal(nt) => {
                    format!("{}(NT)", self.cfg.get_non_terminal_name(*nt))
                }
                StackSymbol::Terminal(t) => {
                    format!("{}(T)", self.cfg.get_terminal_name(*t))
                }
                StackSymbol::EndMarker => "$".to_string(),
                StackSymbol::ProductionEnd { non_terminal, .. } => {
                    format!("↓{}(NT)", self.cfg.get_non_terminal_name(*non_terminal))
                }
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

    fn finalize_ast(&self) -> Result<AstNode, LL1ParseError> {
        self.ast_stack.last().cloned().ok_or_else(LL1ParseError::stack_underflow)
    }

    pub fn get_cfg(&self) -> &ContextFreeGrammar {
        &self.cfg
    }

    pub fn get_table(&self) -> &ParsingTable {
        &self.table
    }
}

impl LL1ParserTrait for LL1Parser {
    fn parse(&mut self) -> Result<AstNode, LL1ParseError> {
        self.parse()
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
}
