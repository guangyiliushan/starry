use crate::cfg::{ContextFreeGrammar, TerminalId};
use crate::lr::action::Action;
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::error::LRError;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::stack::LRStack;
use crate::lr::table::{LRTable, LR0TableBuilder, SLRTableBuilder, LR1TableBuilder, LALR1TableBuilder};
use crate::lr::token_mapper::TokenMapper;
use crate::lr::ItemSetId;
use crate::ast_builder::{GrammarAstBuilder, is_tail_production};
use starry_ast::{AstBuilder, AstNode, Token};

#[derive(Debug, Clone)]
pub struct LRParserConfig {
    pub grammar_type: LRGrammarType,
    pub error_recovery: bool,
    pub max_errors: usize,
    pub trace: bool,
}

impl Default for LRParserConfig {
    fn default() -> Self {
        LRParserConfig {
            grammar_type: LRGrammarType::SLR1,
            error_recovery: false,
            max_errors: 10,
            trace: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseStep {
    pub step: usize,
    pub stack: Vec<String>,
    pub input: String,
    pub action: String,
}

impl std::fmt::Display for ParseStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:5} | {:20} | {:20} | {}", self.step, self.stack.join(" "), self.input, self.action)
    }
}

#[derive(Debug, Clone)]
pub struct ParseTrace {
    steps: Vec<ParseStep>,
}

impl ParseTrace {
    pub fn new() -> Self {
        ParseTrace { steps: Vec::new() }
    }

    pub fn add_step(&mut self, step: ParseStep) {
        self.steps.push(step);
    }

    pub fn steps(&self) -> &[ParseStep] {
        &self.steps
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for ParseTrace {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ParseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:5} | {:20} | {:20} | {}", "Step", "Stack", "Input", "Action")?;
        writeln!(f, "{}", "-".repeat(60))?;
        for step in &self.steps {
            write!(f, "{}", step)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub success: bool,
    pub ast: Option<AstNode>,
    pub trace: ParseTrace,
    pub errors: Vec<LRError>,
    pub derivations: Vec<String>,
}

impl ParseResult {
    pub fn new() -> Self {
        ParseResult {
            success: true,
            ast: None,
            trace: ParseTrace::new(),
            errors: Vec::new(),
            derivations: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: LRError) {
        self.success = false;
        self.errors.push(error);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl Default for ParseResult {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LRParser {
    table: LRTable,
    augmented: AugmentedGrammar,
    config: LRParserConfig,
    token_mapper: TokenMapper,
    ast_builder: GrammarAstBuilder,
}

impl LRParser {
    pub fn new(cfg: ContextFreeGrammar) -> Result<Self, LRError> {
        Self::with_config(cfg, LRParserConfig::default())
    }

    pub fn with_config(cfg: ContextFreeGrammar, config: LRParserConfig) -> Result<Self, LRError> {
        let ast_builder = GrammarAstBuilder::new(&cfg);
        let augmented = AugmentedGrammar::new(cfg);
        let end_marker = TerminalId(augmented.terminals().len());
        let token_mapper = TokenMapper::new(
            augmented.terminal_map().clone(),
            augmented.terminals().to_vec(),
            end_marker,
        );

        let table = match config.grammar_type {
            LRGrammarType::LR0 => {
                let t = LR0TableBuilder::build(&augmented);
                if t.has_conflicts() {
                    return Err(LRError::conflict(t.conflicts.clone()));
                }
                LRTable::LR0(t)
            }
            LRGrammarType::SLR1 => {
                let t = SLRTableBuilder::build(&augmented);
                if t.has_conflicts() {
                    return Err(LRError::conflict(t.conflicts.clone()));
                }
                LRTable::SLR1(t)
            }
            LRGrammarType::LR1 => {
                let t = LR1TableBuilder::build(&augmented);
                if t.has_conflicts() {
                    return Err(LRError::conflict(t.conflicts.clone()));
                }
                LRTable::LR1(t)
            }
            LRGrammarType::LALR1 => {
                let t = LALR1TableBuilder::build(&augmented);
                if t.has_conflicts() {
                    return Err(LRError::conflict(t.conflicts.clone()));
                }
                LRTable::LALR1(t)
            }
            _ => {
                return Err(LRError::invalid_grammar(format!(
                    "Unsupported grammar type: {}",
                    config.grammar_type.name()
                )));
            }
        };

        Ok(LRParser {
            table,
            augmented,
            config,
            token_mapper,
            ast_builder,
        })
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<ParseResult, LRError> {
        let mut result = ParseResult::new();
        let mut stack = LRStack::new(ItemSetId(0));
        let mut input: Vec<TerminalId> = self.token_mapper.tokenize(tokens)?;
        input.push(self.token_mapper.end_marker());

        let mut token_input: Vec<Token> = Vec::with_capacity(input.len());
        for token in tokens {
            if let Ok(tid) = self.token_mapper.get_terminal_id(token) {
                if tid != self.token_mapper.end_marker() {
                    token_input.push(token.clone());
                }
            }
        }

        let ast_builder = &self.ast_builder;
        let mut ast_stack: Vec<AstNode> = Vec::new();

        let mut position = 0;
        let mut step_count = 0;

        loop {
            let current_state = stack.top_state().ok_or_else(LRError::stack_underflow)?;
            let current_token = input.get(position).copied().unwrap_or(self.token_mapper.end_marker());

            if self.config.trace {
                let stack_str: Vec<String> = stack.iter().map(|s| s.to_string()).collect();
                let input_str: String = input[position..]
                    .iter()
                    .map(|t| format!("t{}", t.0))
                    .collect::<Vec<_>>()
                    .join(" ");
                
                result.trace.add_step(ParseStep {
                    step: step_count,
                    stack: stack_str,
                    input: input_str,
                    action: format!("State {}, Token {}", current_state.0, current_token.0),
                });
            }

            let action = self.table.action().get(current_state, current_token);

            match action {
                Action::Shift(next_state) => {
                    let node = ast_builder.make_leaf(&token_input[position]);
                    ast_stack.push(node);
                    stack.push_terminal(current_token);
                    stack.push_state(*next_state);
                    position += 1;
                    step_count += 1;
                }
                Action::Reduce(production_id) => {
                    let production = self.augmented.get_production(production_id.0)
                        .ok_or_else(|| LRError::invalid_grammar(format!(
                            "Invalid production index: {}", production_id.0
                        )))?;

                    let rhs_len = if production.is_epsilon() { 0 } else { production.rhs.len() };
                    let extra = if is_tail_production(production) && ast_stack.len() > rhs_len {
                        1
                    } else {
                        0
                    };
                    let sem_start = ast_stack.len().saturating_sub(rhs_len + extra);
                    let children: Vec<AstNode> = ast_stack.drain(sem_start..).collect();
                    let node = ast_builder.reduce(production_id.0, children);
                    ast_stack.push(node);

                    stack.pop_n(rhs_len * 2);

                    let top_state = stack.top_state().ok_or_else(LRError::stack_underflow)?;
                    
                    if let Some(next_state) = self.table.goto().get(top_state, production.lhs) {
                        stack.push_non_terminal(production.lhs);
                        stack.push_state(next_state);
                    } else {
                        result.add_error(LRError::invalid_state(top_state));
                        break;
                    }

                    if self.config.trace {
                        let lhs_name = self.augmented.get_non_terminal_name(production.lhs);
                        result.derivations.push(format!("{} -> ...", lhs_name));
                    }

                    step_count += 1;
                }
                Action::Accept => {
                    if self.config.trace {
                        result.trace.steps.last_mut().map(|s| {
                            s.action = "Accept".to_string();
                            s
                        });
                    }
                    result.ast = ast_stack.pop();
                    break;
                }
                Action::Error => {
                    let expected = self.get_expected_terminals(current_state);
                    let error = if current_token == self.token_mapper.end_marker() {
                        LRError::unexpected_end(expected, current_state)
                    } else {
                        LRError::unexpected_token(expected, current_token, current_state)
                    };
                    result.add_error(error.with_position(position));
                    
                    if !self.config.error_recovery || result.errors.len() >= self.config.max_errors {
                        break;
                    }
                    
                    position += 1;
                }
            }
        }

        Ok(result)
    }

    fn get_expected_terminals(&self, state: ItemSetId) -> Vec<TerminalId> {
        let mut expected = Vec::new();

        for term_idx in 0..=self.token_mapper.end_marker().0 {
            let terminal = TerminalId(term_idx);
            let action = self.table.action().get(state, terminal);
            if !action.is_error() {
                expected.push(terminal);
            }
        }

        expected
    }

    pub fn table(&self) -> &LRTable {
        &self.table
    }

    pub fn grammar(&self) -> &ContextFreeGrammar {
        self.augmented.grammar()
    }

    pub fn config(&self) -> &LRParserConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: LRParserConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr_parser_creation() {
        let grammar = parse_grammar("E -> num");
        let parser = LRParser::new(grammar);
        
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_simple() {
        let grammar = parse_grammar("E -> num");
        let parser = LRParser::new(grammar).unwrap();
        
        let mut tokens = Vec::new();
        tokens.push(Token {
            kind: starry_ast::TokenKind::Integer(42),
            lexeme: "42".to_string(),
            line: 0,
            column: 0,
        });

        let result = parser.parse(&tokens);
        assert!(result.is_ok());
        
        let parse_result = result.unwrap();
        assert!(parse_result.success);
    }

    #[test]
    fn test_parse_trace() {
        let grammar = parse_grammar("E -> num");
        let config = LRParserConfig {
            trace: true,
            ..Default::default()
        };
        let parser = LRParser::with_config(grammar, config).unwrap();
        
        let mut tokens = Vec::new();
        tokens.push(Token {
            kind: starry_ast::TokenKind::Integer(42),
            lexeme: "42".to_string(),
            line: 0,
            column: 0,
        });

        let result = parser.parse(&tokens).unwrap();
        assert!(!result.trace.is_empty());
    }
}
