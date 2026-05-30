use crate::cfg::ContextFreeGrammar;
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::error::{LRError, LRValidationReport, LRValidationError};
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::parser::{LRParser, LRParserConfig};
use crate::lr::table::SLRTable;
use crate::lr::SLRTableBuilder;

pub struct LRParserBuilder {
    grammar: Option<ContextFreeGrammar>,
    config: LRParserConfig,
    validate: bool,
}

impl LRParserBuilder {
    pub fn new() -> Self {
        LRParserBuilder {
            grammar: None,
            config: LRParserConfig::default(),
            validate: true,
        }
    }

    pub fn grammar(mut self, grammar: ContextFreeGrammar) -> Self {
        self.grammar = Some(grammar);
        self
    }

    pub fn grammar_type(mut self, grammar_type: LRGrammarType) -> Self {
        self.config.grammar_type = grammar_type;
        self
    }

    pub fn error_recovery(mut self, enabled: bool) -> Self {
        self.config.error_recovery = enabled;
        self
    }

    pub fn max_errors(mut self, max: usize) -> Self {
        self.config.max_errors = max;
        self
    }

    pub fn trace(mut self, enabled: bool) -> Self {
        self.config.trace = enabled;
        self
    }

    pub fn validate(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    pub fn build(self) -> Result<LRParser, LRError> {
        let grammar = self.grammar.ok_or_else(|| {
            LRError::invalid_grammar("No grammar provided")
        })?;

        if self.validate {
            let report = LRValidator::validate(&grammar);
            if !report.is_valid {
                return Err(LRError::invalid_grammar(format!(
                    "Grammar validation failed: {:?}",
                    report.errors
                )));
            }
        }

        LRParser::with_config(grammar, self.config)
    }

    pub fn build_unchecked(self) -> Result<LRParser, LRError> {
        let grammar = self.grammar.ok_or_else(|| {
            LRError::invalid_grammar("No grammar provided")
        })?;

        LRParser::with_config(grammar, self.config)
    }
}

impl Default for LRParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LRValidator;

impl LRValidator {
    pub fn validate(cfg: &ContextFreeGrammar) -> LRValidationReport {
        let mut report = LRValidationReport::new();

        let augmented = AugmentedGrammar::new(cfg.clone());
        let table = SLRTableBuilder::build(&augmented);

        if table.has_conflicts() {
            for conflict in &table.conflicts {
                match &conflict.kind {
                    crate::lr::ConflictKind::ShiftReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ShiftReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                    crate::lr::ConflictKind::ReduceReduce { terminal, .. } => {
                        report.add_error(LRValidationError::ReduceReduceConflict {
                            state: conflict.state,
                            terminal: *terminal,
                        });
                    }
                }
            }
            report.grammar_type = LRGrammarType::NotLR;
        } else {
            report.grammar_type = Self::determine_grammar_type(&table);
        }

        report
    }

    fn determine_grammar_type(table: &SLRTable) -> LRGrammarType {
        let mut has_shift_reduce = false;
        let has_reduce_reduce = false;

        for state in 0..table.state_count {
            for term in 0..=table.terminal_count {
                let action = table.action.get(
                    crate::lr::ItemSetId(state),
                    crate::cfg::TerminalId(term),
                );

                if action.is_shift() {
                    for term2 in 0..=table.terminal_count {
                        let action2 = table.action.get(
                            crate::lr::ItemSetId(state),
                            crate::cfg::TerminalId(term2),
                        );
                        if action2.is_reduce() {
                            has_shift_reduce = true;
                        }
                    }
                }
            }
        }

        if has_reduce_reduce {
            LRGrammarType::NotLR
        } else if has_shift_reduce {
            LRGrammarType::SLR1
        } else {
            LRGrammarType::LR0
        }
    }

    pub fn is_lr0(cfg: &ContextFreeGrammar) -> bool {
        let report = Self::validate(cfg);
        report.grammar_type.is_lr0()
    }

    pub fn is_slr1(cfg: &ContextFreeGrammar) -> bool {
        let report = Self::validate(cfg);
        report.grammar_type.is_slr1() || report.grammar_type.is_lr0()
    }

    pub fn is_lalr1(cfg: &ContextFreeGrammar) -> bool {
        let report = Self::validate(cfg);
        report.grammar_type.is_lalr1() || report.grammar_type.is_slr1() || report.grammar_type.is_lr0()
    }

    pub fn is_lr1(cfg: &ContextFreeGrammar) -> bool {
        let report = Self::validate(cfg);
        report.grammar_type.is_deterministic()
    }

    pub fn get_grammar_type(cfg: &ContextFreeGrammar) -> LRGrammarType {
        let report = Self::validate(cfg);
        report.grammar_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let parser = LRParserBuilder::new()
            .grammar(grammar)
            .build();

        assert!(parser.is_ok());
    }

    #[test]
    fn test_builder_with_config() {
        let grammar = parse_grammar("E -> num");
        let parser = LRParserBuilder::new()
            .grammar(grammar)
            .trace(true)
            .error_recovery(true)
            .max_errors(5)
            .build();

        assert!(parser.is_ok());
        let parser = parser.unwrap();
        assert!(parser.config().trace);
        assert!(parser.config().error_recovery);
        assert_eq!(parser.config().max_errors, 5);
    }

    #[test]
    fn test_validator_simple() {
        let grammar = parse_grammar("E -> num");
        let report = LRValidator::validate(&grammar);

        assert!(report.is_valid);
    }

    #[test]
    fn test_is_slr1() {
        let grammar = parse_grammar("E -> num");
        assert!(LRValidator::is_slr1(&grammar));
    }

    #[test]
    fn test_get_grammar_type() {
        let grammar = parse_grammar("E -> num");
        let grammar_type = LRValidator::get_grammar_type(&grammar);

        assert!(grammar_type.is_deterministic());
    }
}
