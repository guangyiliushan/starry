mod cfg;
mod ll1_parser;
mod parser;

pub mod analysis;
pub use analysis::{
    FirstSet, FirstSetCalculator, FollowSet, FollowSetCalculator,
    LeftRecursionAnalyzer, LeftRecursionDetector, LeftRecursionEliminator,
    LeftRecursionInfo, LeftRecursionType, NullableSet,
    ParsingConflict, ParsingTable, ParsingTableBuilder, TableEntry,
};

pub use cfg::{ContextFreeGrammar, NonTerminalId, Production, Symbol, TerminalId};
pub use ll1_parser::{LL1ParseError, LL1Parser, LL1ParserBuilder};
pub use parser::{ParseError, ParseTreeNode, Parser, ParserBuilder};
pub use starry_ast::{Token, TokenKind, TokenStream, TokenStreamBuilder};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_parsing_pipeline() {
        let grammar_str = r#"
            Expr -> Term + Expr | Term
            Term -> num
        "#;
        
        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        
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
    fn test_simple_expression() {
        let grammar_str = "Expr -> num";
        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        
        let mut builder = TokenStreamBuilder::new();
        builder.add_integer(42, "42".to_string());
        let stream = builder.build();

        let mut parser = Parser::new(cfg, stream);
        let result = parser.parse();
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_epsilon_production() {
        let grammar_str = r#"
            S -> a A | ε
            A -> b
        "#;
        
        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
        
        let stream = TokenStreamBuilder::new().build();
        
        let mut parser = Parser::new(cfg, stream);
        let result = parser.parse();
        
        assert!(result.is_ok());
    }
}
