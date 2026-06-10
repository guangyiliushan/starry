use super::parsing_table::ParsingTableBuilder;
use crate::cfg::ContextFreeGrammar;
use crate::ll1::parser::LL1Parser;
use starry_ast::TokenStream;

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

    pub fn validate_grammar(&self) -> Result<bool, String> {
        let cfg = self.cfg.as_ref().ok_or("No grammar provided")?;
        
        let conflicts = ParsingTableBuilder::check_grammar(cfg)?;
        
        Ok(conflicts.is_empty())
    }

    pub fn get_grammar(&self) -> Option<&ContextFreeGrammar> {
        self.cfg.as_ref()
    }
}

impl Default for LL1ParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LL1Validator;

impl LL1Validator {
    pub fn is_ll1(cfg: &ContextFreeGrammar) -> Result<bool, String> {
        let conflicts = ParsingTableBuilder::check_grammar(cfg)?;
        Ok(conflicts.is_empty())
    }

    pub fn get_conflicts(cfg: &ContextFreeGrammar) -> Result<Vec<String>, String> {
        let conflicts = ParsingTableBuilder::check_grammar(cfg)?;
        Ok(conflicts
            .iter()
            .map(|c| format!("{:?}", c))
            .collect())
    }

    pub fn validate_and_report(cfg: &ContextFreeGrammar) -> Result<LL1ValidationReport, String> {
        let conflicts = ParsingTableBuilder::check_grammar(cfg)?;
        
        let is_ll1 = conflicts.is_empty();
        let conflict_details: Vec<String> = conflicts
            .iter()
            .map(|c| format!("{:?}", c))
            .collect();

        Ok(LL1ValidationReport {
            is_ll1,
            conflicts: conflict_details,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LL1ValidationReport {
    pub is_ll1: bool,
    pub conflicts: Vec<String>,
}

impl LL1ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.is_ll1
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_builder() {
        let grammar = ContextFreeGrammar::parse("E -> num").unwrap();

        let mut builder = starry_ast::TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let parser = LL1ParserBuilder::new()
            .with_grammar(grammar)
            .build(stream);

        assert!(parser.is_ok());
    }

    #[test]
    fn test_ll1_validator() {
        let grammar = ContextFreeGrammar::parse("E -> num").unwrap();
        
        let is_ll1 = LL1Validator::is_ll1(&grammar);
        assert!(is_ll1.is_ok());
    }

    #[test]
    fn test_validation_report() {
        let grammar = ContextFreeGrammar::parse("E -> num").unwrap();
        
        let report = LL1Validator::validate_and_report(&grammar);
        assert!(report.is_ok());
        
        let report = report.unwrap();
        assert!(report.is_valid());
    }
}
