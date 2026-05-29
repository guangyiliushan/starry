mod cfg;
mod ll1;
mod lr;
mod parser;

pub mod analysis;
pub use analysis::{
    FirstSet, FirstSetCalculator, FollowSet, FollowSetCalculator,
    LeftRecursionAnalyzer, LeftRecursionDetector, LeftRecursionEliminator,
    LeftRecursionInfo, LeftRecursionType, NullableSet,
};

pub use cfg::{ContextFreeGrammar, NonTerminalId, Production, Symbol, TerminalId};
pub use ll1::{
    LL1ParseError, LL1Parser, LL1ParserBuilder, LL1ValidationReport, LL1Validator,
    ParsingConflict, ParsingTable, ParsingTableBuilder, TableEntry,
    ParseStack, StackSymbol, TokenMapper, ParseTreeBuilder, TreeNode,
    DerivationRecorder, DerivationStep,
};
pub use lr::{
    Action, ActionTable, AugmentedGrammar, Conflict, ConflictDetector, ConflictKind,
    ConflictReport, CoreSet, CoreSetDetector, CoreSetMerger, GotoEntry, GotoResult,
    GotoTable, ItemSetId, LALR1Validator, LR0Closure, LR0Goto, LR0Item, LR0ItemSet,
    LR0Table, LR0TableBuilder, LR0Validator, LR1Closure, LR1Goto, LR1Item, LR1ItemSet, LR1Validator,
    LR1Table, LR1TableBuilder, LALR1Table, LALR1TableBuilder,
    LRError, LRErrorKind, LRGrammarType, LRFirstCalculator, LRFollowCalculator,
    LRParser, LRParserBuilder, LRParserConfig, LRStack, LRTable, LRValidationError,
    LRValidationReport, LRValidator, LookaheadCalculator, LookaheadSet, ParseResult,
    ParseStep, ParseTrace, ProductionId, SLR1Validator, SLRTable, SLRTableBuilder,
    StackAction, StackSymbol as LRStackSymbol, TokenMapper as LRTokenMapper,
    Item, ItemKind, ItemTrait, format_item,
    create_initial_lr0_item_set, create_initial_lr1_item_set,
    compute_closure_lookaheads, AllLR0Items, AllLR1Items,
    LR0ItemSetCollection, LR1ItemSetCollection, GrammarType,
};
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
