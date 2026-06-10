use starry_ast::TokenStreamBuilder;
use starry_parser::{
    AugmentedGrammar, ContextFreeGrammar, FirstSetCalculator, FollowSetCalculator,
    LALR1TableBuilder, LR0TableBuilder, LR1TableBuilder, LRParserBuilder, SLRTableBuilder,
    LRGrammarType, AllLR0Items, LR0ItemSetCollection, LR1ItemSetCollection, LookaheadCalculator,
    AllLR1Items,
};

fn main() {
    let grammar_str = r#"
        E -> E + T | T
        T -> T * F | F
        F -> ( E ) | num
    "#;

    let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
    cfg.print();
    println!();

    let augmented = AugmentedGrammar::new(cfg.clone());
    let augmented_grammar = augmented.grammar();
    augmented.print();
    println!();

    let items0 = AllLR0Items::build(augmented_grammar);
    items0.print(augmented_grammar);
    println!();

    let lr0_items = LR0ItemSetCollection::build(augmented_grammar);
    lr0_items.print(augmented_grammar);
    println!();

    let lr0_table = LR0TableBuilder::build(&augmented);
    lr0_table.print(augmented.grammar());
    if lr0_table.is_lr0() {
        println!("Is LR(0)");
    }
    println!();

    let first_sets = FirstSetCalculator::compute(augmented_grammar);
    first_sets.print(&cfg);
    println!();

    let nullable = FirstSetCalculator::compute_nullable_set(augmented_grammar);
    let follow_sets = FollowSetCalculator::compute(augmented_grammar, &first_sets, &nullable);
    follow_sets.print(&cfg);
    println!();

    let slr_table = SLRTableBuilder::build(&augmented);
    slr_table.print(augmented.grammar());
    if slr_table.is_slr1() {
        println!("Is SLR(1)");
    }
    println!();

    let lookahead_calculator = LookaheadCalculator::new(augmented_grammar);
    let items1 = AllLR1Items::build(augmented_grammar, &lookahead_calculator);
    items1.print(augmented_grammar);
    println!();

    let lr1_items = LR1ItemSetCollection::build(augmented_grammar, &lookahead_calculator);
    lr1_items.print(augmented_grammar);
    println!();

    let lr1_table = LR1TableBuilder::build(&augmented);
    lr1_table.print(augmented.grammar());
    if lr1_table.is_lr1() {
        println!("Is LR(1)");
    }
    println!();

    let lalr1_table = LALR1TableBuilder::build(&augmented);
    lalr1_table.print(augmented.grammar());
    if lalr1_table.is_lalr1() {
        println!("Is LALR(1)");
    }
    println!();

    let mut builder = TokenStreamBuilder::new();
    builder.add_integer(1, "1".to_string());
    builder.add_operator("+".to_string());
    builder.add_integer(2, "2".to_string());
    builder.add_operator("*".to_string());
    builder.add_integer(3, "3".to_string());
    let stream = builder.build();

    match LRParserBuilder::new()
        .grammar(cfg.clone())
        .grammar_type(LRGrammarType::LALR1)
        .trace(true)
        .build()
    {
        Ok(parser) => match parser.parse(stream.tokens()) {
            Ok(r) => {
                println!("{}", r.trace);
                println!();
                if r.success {
                    println!("Parse successful!");
                    if let Some(ast) = r.ast {
                        println!("Display: {}", ast);
                        println!("\nAST Tree:");
                        ast.print();
                    }
                } else {
                    println!("Parse failed");
                }
            }
            Err(e) => println!("Parse error: {}", e),
        },
        Err(e) => println!("Parser build error: {}", e),
    }
}
