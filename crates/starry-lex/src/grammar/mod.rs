mod ast;
mod builder;
mod converter;
mod display;
mod parser;

pub use ast::{NonTerminalId, Production, RegularGrammar};
pub use builder::GrammarBuilder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_grammar() {
        let input = r#"
            S -> aA | ε
            A -> aA | b
        "#;
        
        let grammar = RegularGrammar::parse(input).unwrap();
        assert_eq!(grammar.non_terminals.len(), 2);
        assert_eq!(grammar.start_symbol, NonTerminalId(0));
    }

    #[test]
    fn test_grammar_to_nfa() {
        let input = r#"
            S -> aA | ε
            A -> b
        "#;
        
        let grammar = RegularGrammar::parse(input).unwrap();
        let nfa = grammar.to_nfa();
        
        assert!(nfa.states.len() >= 2);
    }

    #[test]
    fn test_simple_grammar() {
        let mut grammar = RegularGrammar::new();
        
        let s = grammar.add_non_terminal("S");
        let a = grammar.add_non_terminal("A");
        
        grammar.set_start(s);
        
        grammar.add_production(s, Production::Terminal('a'));
        grammar.add_production(s, Production::TerminalNonTerminal('a', a));
        grammar.add_production(a, Production::Terminal('b'));
        
        assert_eq!(grammar.non_terminals.len(), 2);
        assert_eq!(grammar.productions.len(), 2);
    }

    #[test]
    fn test_arden_simple() {
        let mut grammar = RegularGrammar::new();
        
        let s = grammar.add_non_terminal("S");
        grammar.set_start(s);
        
        grammar.add_production(s, Production::Terminal('a'));
        grammar.add_production(s, Production::TerminalNonTerminal('a', s));
        
        let regex = grammar.to_regex().unwrap();
        println!("Simple Arden result: {:?}", regex);
    }

    #[test]
    fn test_arden_two_nonterminals() {
        let mut grammar = RegularGrammar::new();
        
        let s = grammar.add_non_terminal("S");
        let a = grammar.add_non_terminal("A");
        
        grammar.set_start(s);
        
        grammar.add_production(s, Production::TerminalNonTerminal('a', s));
        grammar.add_production(s, Production::TerminalNonTerminal('b', a));
        grammar.add_production(s, Production::Epsilon);
        
        grammar.add_production(a, Production::TerminalNonTerminal('a', a));
        grammar.add_production(a, Production::TerminalNonTerminal('b', s));
        grammar.add_production(a, Production::Terminal('b'));
        
        let regex = grammar.to_regex().unwrap();
        println!("Two nonterminals Arden result: {:?}", regex);
    }

    #[test]
    fn test_regex_to_grammar_via_nfa() {
        let regex = crate::Regex::parse("a(b|c)*").unwrap();
        let grammar = GrammarBuilder::from_regex(&regex);
        
        println!("Grammar from regex a(b|c)*:");
        grammar.print();
        
        assert!(!grammar.non_terminals.is_empty());
        assert!(!grammar.productions.is_empty());
    }

    #[test]
    fn test_dfa_to_grammar() {
        use crate::dfa::Dfa;
        
        let mut dfa = Dfa::new();
        let s0 = dfa.add_state();
        let s1 = dfa.add_state();
        
        dfa.start = s0;
        dfa.mark_accept(s1, "test_token".to_string());
        
        dfa.add_transition(s0, '0', s1);
        dfa.add_transition(s0, '1', s0);
        dfa.add_transition(s1, '0', s1);
        dfa.add_transition(s1, '1', s0);
        
        let grammar = GrammarBuilder::from_dfa(&dfa);
        
        println!("Grammar from DFA:");
        grammar.print();
        
        assert_eq!(grammar.non_terminals.len(), 2);
        assert!(grammar.productions.contains_key(&NonTerminalId(1)));
    }

    #[test]
    fn test_roundtrip_regex_grammar_regex() {
        let original_regex = crate::Regex::parse("ab").unwrap();
        let grammar = GrammarBuilder::from_regex(&original_regex);
        
        println!("Original regex: {:?}", original_regex);
        println!("Grammar:");
        grammar.print();
        
        let back_to_regex = grammar.to_regex().unwrap();
        println!("Back to regex: {:?}", back_to_regex);
    }
}
