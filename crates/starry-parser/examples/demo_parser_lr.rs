use starry_ast::TokenStreamBuilder;
use starry_parser::{
    AugmentedGrammar, ContextFreeGrammar, FirstSetCalculator, FollowSetCalculator,
    LALR1TableBuilder, LR0TableBuilder, LR1TableBuilder, LRParserBuilder, SLRTableBuilder, LRGrammarType, 
    AllLR0Items, LR0ItemSetCollection, LR1ItemSetCollection, LookaheadCalculator, AllLR1Items,
};

fn main() {
    let grammar_str = r#"
        E -> E + T | T
        T -> T * F | F
        F -> ( E ) | num
    "#;

    // 上下文无关文法
    let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();
    cfg.print();
    println!();
    // 构建拓广文法
    let augmented = AugmentedGrammar::new(cfg.clone());
    let augmented_grammar = augmented.grammar();
    augmented.print();
    println!();
    // 构建所有LR0项目
    let items0 = AllLR0Items::build(augmented_grammar);
    items0.print(augmented_grammar);
    println!();
    // 构建所有LR0目簇
    let lr0_items = LR0ItemSetCollection::build(augmented_grammar);
    lr0_items.print(augmented_grammar);
    println!();
    // 构建LR(0)分析表
    let lr0_table = LR0TableBuilder::build(&augmented);
    lr0_table.print(augmented.grammar());
    if !lr0_table.has_conflicts() {
        println!("Is LR(0)");
    }
    println!();
    // 构建所有FIRST集合
    let first_sets = FirstSetCalculator::compute(&augmented_grammar);
    first_sets.print(&cfg);
    println!();
    // 构建所有FOLLOW集合
    let nullable = FirstSetCalculator::compute_nullable_set(&augmented_grammar);
    let follow_sets = FollowSetCalculator::compute(&augmented_grammar, &first_sets, &nullable);
    follow_sets.print(&cfg);
    println!();

    // 构建SLR(1)分析表
    let slr_table = SLRTableBuilder::build(&augmented);
    slr_table.print(augmented.grammar());
    if !slr_table.has_conflicts() {
        println!("Is SLR(1)");
    }
    // 向前看符号
    let lookahead_calculator = LookaheadCalculator::new(&augmented_grammar);
    println!();
    // LR1项目
    let items1 = AllLR1Items::build(augmented_grammar, &lookahead_calculator);
    items1.print(augmented_grammar);
    println!();
    // 构建所有LR1项目簇
    let lr1_items = LR1ItemSetCollection::build(augmented_grammar, &lookahead_calculator);
    lr1_items.print(augmented_grammar);
    println!();
    // 构建LR(1)分析表
    let lr1_table = LR1TableBuilder::build(&augmented);
    lr1_table.print(augmented.grammar());
    if lr1_table.has_conflicts() {
        println!("Is LR(1)");
    }
    println!();

    // 构建LALR(1)分析表
    let lalr1_table = LALR1TableBuilder::build(&augmented);
    lalr1_table.print(augmented.grammar());
    if !lalr1_table.has_conflicts() {
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
                } else {
                    println!("Parse failed");
                }
            }
            Err(e) => println!("Parse error: {}", e),
        },
        Err(e) => println!("Parser build error: {}", e),
    }
}
