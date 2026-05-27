use starry_lex::{GrammarBuilder, Regex, RegularGrammar, brzozowski, hopcroft, moore, nfa_to_dfa, regex_to_nfa};

fn main() {
    let regex_str = "a(b|a)*";
    let regex = Regex::parse(regex_str).unwrap();
    println!("Input: {}", regex_str);
    println!("Parsed: {:?}\n", regex);

    let nfa = regex_to_nfa(&regex, "TEST_TOKEN");
    nfa.print();
    println!();

    let dfa = nfa_to_dfa(&nfa);
    dfa.print();
    println!();

    let minimized_hopcroft = hopcroft(&dfa);
    println!("(Hopcroft):");
    minimized_hopcroft.print();

    let minimized_moore = moore(&dfa);
    println!("(Moore):");
    minimized_moore.print();

    let minimized_brzozowski = brzozowski(&dfa);
    println!("(Brzozowski):");
    minimized_brzozowski.print();
    println!();

    let grammar_from_dfa = GrammarBuilder::from_dfa(&minimized_hopcroft);
    grammar_from_dfa.print();
    println!();

    let regex_from_grammar = grammar_from_dfa.to_regex().unwrap();
    println!("{:?}\n", regex_from_grammar);

    let grammar = GrammarBuilder::from_regex(&regex);
    grammar.print();
    println!();

    let grammar_nfa = grammar.to_nfa();
    grammar_nfa.print();
    println!();

    let grammar_dfa = nfa_to_dfa(&grammar_nfa);
    grammar_dfa.print();
    println!();

    let grammar_min_dfa = hopcroft(&grammar_dfa);
    grammar_min_dfa.print();
    println!();

    let grammar_str = r#"
        S -> aA
        A -> aA | bA | ε
    "#;
    println!("{}", grammar_str);
    
    let parsed_grammar = RegularGrammar::parse(grammar_str).unwrap();
    parsed_grammar.print();
    println!();
    
    let parsed_nfa = parsed_grammar.to_nfa();
    parsed_nfa.print();
    println!();
    
    let parsed_dfa = nfa_to_dfa(&parsed_nfa);
    parsed_dfa.print();
    println!();
    
    let parsed_min_dfa = hopcroft(&parsed_dfa);
    parsed_min_dfa.print();
    println!();
    
    let parsed_regex = parsed_grammar.to_regex().unwrap();
    println!("{:?}\n", parsed_regex);
}
