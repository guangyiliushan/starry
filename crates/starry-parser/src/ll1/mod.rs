mod builder;
mod error;
mod parser;
mod parsing_table;
mod stack;
mod token_mapper;
mod tree_builder;

pub use builder::{LL1ParserBuilder, LL1ValidationReport, LL1Validator};
pub use error::LL1ParseError;
pub use parser::LL1Parser;
pub use parsing_table::{ParsingConflict, ParsingTable, ParsingTableBuilder, TableEntry};
pub use stack::{ParseStack, StackSymbol};
pub use token_mapper::TokenMapper;
pub use tree_builder::{DerivationRecorder, DerivationStep, ParseTreeBuilder, TreeNode};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;
    use starry_ast::TokenStreamBuilder;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_full_parsing_pipeline() {
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

        let mut parser = LL1ParserBuilder::new()
            .with_grammar(grammar)
            .with_trace(false)
            .build(stream)
            .unwrap();

        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_ll1_validation() {
        let grammar = parse_grammar("E -> num");
        
        let report = LL1Validator::validate_and_report(&grammar).unwrap();
        assert!(report.is_valid());
    }

    #[test]
    fn test_builder_with_grammar_from_str() {
        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let parser = LL1ParserBuilder::new()
            .with_grammar_from_str("E -> num")
            .unwrap()
            .build(stream);

        assert!(parser.is_ok());
    }
}
