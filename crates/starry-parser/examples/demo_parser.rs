use starry_parser::{
    ContextFreeGrammar, FirstSetCalculator, FollowSetCalculator, LeftRecursionAnalyzer,
    LL1Parser, ParsingTableBuilder,
};
use starry_ast::TokenStreamBuilder;

fn main() {
    let grammar_str = r#"
        Expr -> Expr + Term | Term
        Term -> Term * Factor | Factor
        Factor -> num
    "#;

    let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
    cfg.print();
    println!();

    let analyzer = LeftRecursionAnalyzer::new();

    let recursions = analyzer.detect(&cfg);
    println!("Left Recursion: {:?}", recursions);
    println!();

    let new_cfg = analyzer.eliminate(&cfg).unwrap();
    new_cfg.print();
    println!();

    let nullable = FirstSetCalculator::compute_nullable_set(&new_cfg);
    nullable.print(&new_cfg);
    println!();

    let first_sets = FirstSetCalculator::compute(&new_cfg);
    first_sets.print(&new_cfg);
    println!();

    let follow_sets = FollowSetCalculator::compute(&new_cfg, &first_sets, &nullable);
    follow_sets.print(&new_cfg);
    println!();

    let parsing_table = ParsingTableBuilder::build_from_grammar(&new_cfg).unwrap();
    parsing_table.print(&new_cfg);
    println!();

    let mut builder = TokenStreamBuilder::new();
    builder.add_integer(1, "1".to_string());
    builder.add_operator("+".to_string());
    builder.add_integer(2, "2".to_string());
    builder.add_operator("*".to_string());
    builder.add_integer(3, "3".to_string());
    let stream = builder.build();

    let mut parser = LL1Parser::new(new_cfg, parsing_table, stream);
    parser.enable_trace(true);

    match parser.parse() {
        Ok(tree) => {
            println!("\nParse successful!");
            tree.print(0);
        }
        Err(e) => {
            println!("\nParse error: {}", e.message);
            println!("  Line: {}, Column: {}", e.line, e.column);
            if !e.expected.is_empty() {
                println!("  Expected: {:?}", e.expected);
            }
            if let Some(found) = e.found {
                println!("  Found: {}", found);
            }
        }
    }
}
