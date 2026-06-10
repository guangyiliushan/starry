use crate::ast_builder::{GrammarAstBuilder, is_tail_production};
use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol, TerminalId};
use crate::ll1::error::LL1ParseError;
use crate::ll1::parsing_table::{ParsingTable, ParsingTableBuilder};
use crate::ll1::token_mapper::TokenMapper;
use crate::token_mapper as shared_mapper;
use super::LL1ParserTrait;
use starry_ast::{AstBuilder, AstNode, TokenKind, TokenStream};
use std::collections::HashSet;

pub struct RecursiveDescentParser {
    cfg: ContextFreeGrammar,
    table: ParsingTable,
    stream: TokenStream,
    ast_builder: GrammarAstBuilder,
    tail_non_terminals: HashSet<NonTerminalId>,
    trace: bool,
}

impl RecursiveDescentParser {
    pub fn new(cfg: ContextFreeGrammar, table: ParsingTable, stream: TokenStream) -> Self {
        let tail_non_terminals: HashSet<NonTerminalId> = cfg
            .productions
            .iter()
            .filter(|p| is_tail_production(p))
            .map(|p| p.lhs)
            .collect();
        let ast_builder = GrammarAstBuilder::new(&cfg);

        RecursiveDescentParser {
            cfg,
            table,
            stream,
            ast_builder,
            tail_non_terminals,
            trace: false,
        }
    }

    pub fn enable_trace(&mut self, enable: bool) {
        self.trace = enable;
    }

    fn is_tail_non_terminal(&self, nt: NonTerminalId) -> bool {
        self.tail_non_terminals.contains(&nt)
    }

    fn parse_non_terminal(&mut self, nt: NonTerminalId) -> Result<AstNode, LL1ParseError> {
        self.parse_non_terminal_inner(nt, None)
    }

    fn parse_non_terminal_inner(
        &mut self,
        nt: NonTerminalId,
        inherited: Option<AstNode>,
    ) -> Result<AstNode, LL1ParseError> {
        let (prod_index, rhs) = {
            let mapper = TokenMapper::new(&self.cfg, &self.table);
            let term_id = mapper.get_current_terminal_id(&self.stream)?;
            let entry = self.table.get(nt, term_id).ok_or_else(|| {
                let token = self.stream.peek();
                let expected = self.get_expected_terminals(nt);
                LL1ParseError::no_production(
                    self.cfg.get_non_terminal_name(nt).to_string(),
                    token.map_or(0, |t| t.line),
                    token.map_or(0, |t| t.column),
                    expected,
                    token.map(|t| format!("{:?}", t.kind)),
                )
            })?;
            (entry.production_index, entry.production.rhs.clone())
        };

        if self.trace {
            self.print_production(nt, &rhs);
        }

        if rhs.is_empty() || (rhs.len() == 1 && matches!(rhs[0], Symbol::Epsilon)) {
            return Ok(inherited.unwrap_or_else(|| {
                self.ast_builder.reduce(prod_index, vec![])
            }));
        }

        let production = &self.cfg.productions[prod_index];

        if is_tail_production(production) {
            return self.parse_tail_rhs(prod_index, &rhs, inherited);
        }

        let n = rhs.len();
        let calls_tail = n > 0
            && matches!(rhs.last(), Some(Symbol::NonTerminal(last_nt)) if self.is_tail_non_terminal(*last_nt));

        if calls_tail {
            let mut children = Vec::new();
            for symbol in &rhs[..n - 1] {
                match symbol {
                    Symbol::NonTerminal(n) => {
                        children.push(self.parse_non_terminal_inner(*n, None)?);
                    }
                    Symbol::Terminal(term_id) => {
                        children.push(self.match_terminal(*term_id)?);
                    }
                    Symbol::Epsilon => {}
                }
            }

            let inherited = if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                self.ast_builder.reduce(prod_index, children)
            };

            let last_nt = match rhs.last().unwrap() {
                Symbol::NonTerminal(nt) => *nt,
                _ => unreachable!(),
            };
            return self.parse_non_terminal_inner(last_nt, Some(inherited));
        }

        let mut children = Vec::with_capacity(rhs.len());
        for symbol in &rhs {
            match symbol {
                Symbol::NonTerminal(n) => {
                    children.push(self.parse_non_terminal_inner(*n, None)?);
                }
                Symbol::Terminal(term_id) => {
                    children.push(self.match_terminal(*term_id)?);
                }
                Symbol::Epsilon => {}
            }
        }

        Ok(self.ast_builder.reduce(prod_index, children))
    }

    fn parse_tail_rhs(
        &mut self,
        prod_index: usize,
        rhs: &[Symbol],
        inherited: Option<AstNode>,
    ) -> Result<AstNode, LL1ParseError> {
        let acc = inherited.expect("tail production requires inherited attribute");
        let n = rhs.len();

        let mut children = vec![acc];
        for symbol in &rhs[..n - 1] {
            match symbol {
                Symbol::NonTerminal(nt) => {
                    children.push(self.parse_non_terminal_inner(*nt, None)?);
                }
                Symbol::Terminal(term_id) => {
                    children.push(self.match_terminal(*term_id)?);
                }
                Symbol::Epsilon => {}
            }
        }

        let new_inherited = self.ast_builder.reduce(prod_index, children);

        let last_nt = match rhs.last().unwrap() {
            Symbol::NonTerminal(nt) => *nt,
            _ => unreachable!("tail production must end with non-terminal"),
        };

        self.parse_non_terminal_inner(last_nt, Some(new_inherited))
    }

    fn match_terminal(&mut self, expected: TerminalId) -> Result<AstNode, LL1ParseError> {
        let (kind, line, col) = match self.stream.peek() {
            Some(t) => (t.kind.clone(), t.line, t.column),
            None => {
                return Err(LL1ParseError::unexpected_eof(
                    self.cfg.get_terminal_name(expected).to_string(),
                ));
            }
        };

        let expected_name = self.cfg.get_terminal_name(expected).to_string();
        let end_marker = self.table.end_marker();

        if shared_mapper::matches_terminal(expected, &kind, &expected_name, end_marker) {
            if self.trace {
                println!("  match {}", expected_name);
            }
            let consumed = self.stream.consume().unwrap();
            Ok(self.ast_builder.make_leaf(&consumed))
        } else {
            Err(LL1ParseError::terminal_mismatch(
                expected_name,
                format!("{:?}", kind),
                line,
                col,
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

    fn print_production(&self, nt: NonTerminalId, rhs: &[Symbol]) {
        print!("{}: ", self.cfg.get_non_terminal_name(nt));
        if rhs.is_empty() || (rhs.len() == 1 && matches!(rhs[0], Symbol::Epsilon)) {
            println!("ε");
        } else {
            let rhs_str: Vec<String> = rhs
                .iter()
                .map(|sym| match sym {
                    Symbol::NonTerminal(n) => self.cfg.get_non_terminal_name(*n).to_string(),
                    Symbol::Terminal(t) => self.cfg.get_terminal_name(*t).to_string(),
                    Symbol::Epsilon => "ε".to_string(),
                })
                .collect();
            println!("{}", rhs_str.join(" "));
        }
    }
}

impl LL1ParserTrait for RecursiveDescentParser {
    fn parse(&mut self) -> Result<AstNode, LL1ParseError> {
        if self.trace {
            println!("Recursive Descent LL(1) Trace:");
        }

        let start = self.cfg.start_symbol;
        let ast = self.parse_non_terminal(start)?;

        match self.stream.peek() {
            None => Ok(ast),
            Some(t) if matches!(t.kind, TokenKind::Eof) => Ok(ast),
            Some(t) => Err(LL1ParseError::unexpected_input(
                t.line,
                t.column,
                format!("{:?}", t.kind),
            )),
        }
    }
}

pub struct RecursiveDescentParserBuilder {
    cfg: Option<ContextFreeGrammar>,
    trace: bool,
}

impl RecursiveDescentParserBuilder {
    pub fn new() -> Self {
        RecursiveDescentParserBuilder {
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

    pub fn build(self, stream: TokenStream) -> Result<RecursiveDescentParser, String> {
        let cfg = self.cfg.ok_or("No grammar provided")?;
        let table = ParsingTableBuilder::build_from_grammar(&cfg).map_err(|conflicts| {
            format!(
                "Grammar is not LL(1): {} conflict(s) detected",
                conflicts.len()
            )
        })?;

        let mut parser = RecursiveDescentParser::new(cfg, table, stream);
        parser.enable_trace(self.trace);
        Ok(parser)
    }
}

impl Default for RecursiveDescentParserBuilder {
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
    fn test_simple_expression() {
        let grammar = parse_grammar(
            r#"
            E -> T E'
            E' -> + T E' | ε
            T -> num
        "#,
        );

        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        let stream = builder.build();

        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_epsilon_start() {
        let grammar = parse_grammar(
            r#"
            S -> a A | ε
            A -> b
        "#,
        );

        let stream = TokenStreamBuilder::new().build();
        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parentheses() {
        let grammar = parse_grammar(
            r#"
            E -> T E'
            E' -> + T E' | ε
            T -> F T'
            T' -> * F T' | ε
            F -> ( E ) | num
        "#,
        );

        let mut builder = TokenStreamBuilder::new();
        builder.add_delimiter("(".to_string());
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        builder.add_delimiter(")".to_string());
        let stream = builder.build();

        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_error() {
        let grammar = parse_grammar(
            r#"
            S -> a A
            A -> b
        "#,
        );

        let mut builder = TokenStreamBuilder::new();
        builder.add_identifier("x".to_string());
        let stream = builder.build();

        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_ast_output_is_displayable() {
        let grammar = parse_grammar("E -> num");

        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let ast = parser.parse().unwrap();
        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_ast_output_is_binary_expr() {
        let grammar = parse_grammar(
            r#"
            E -> T E'
            E' -> + T E' | ε
            T -> num
        "#,
        );

        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(1, "1".to_string());
        builder.add_operator("+".to_string());
        builder.add_integer(2, "2".to_string());
        let stream = builder.build();

        let mut parser = RecursiveDescentParserBuilder::new()
            .with_grammar(grammar)
            .build(stream)
            .unwrap();

        let ast = parser.parse().unwrap();
        let display = format!("{}", ast);
        assert!(!display.is_empty());
        assert!(matches!(ast, starry_ast::AstNode::Binary(_)), "expected Binary, got: {}", display);
    }
}
