mod dfa;
pub mod grammar;
mod lexer;
mod minimize;
mod nfa;
mod regex;
mod subset;
mod thompson;

pub use dfa::{Dfa, DfaState, DfaStateId};
pub use grammar::{GrammarBuilder, NonTerminalId, Production, RegularGrammar};
pub use lexer::{Lexer, Token};
pub use nfa::{Nfa, NfaState, NfaStateId, Symbol};
pub use regex::Regex;

pub use thompson::regex_to_nfa;
pub use subset::nfa_to_dfa;
pub use minimize::{hopcroft, moore, brzozowski};

pub fn build_lexer(rules: Vec<(&str, &str)>) -> Result<Lexer, String> {
    let mut nfas = Vec::new();

    for (regex_str, token_name) in rules {
        let regex = Regex::parse(regex_str)?;
        let nfa = thompson::ThompsonBuilder::build_single(&regex, token_name);
        nfas.push(nfa);
    }

    let combined_nfa = thompson::ThompsonBuilder::union(nfas);

    let dfa = subset::SubsetConstruction::build(&combined_nfa);

    let min_dfa = minimize::HopcroftMinimizer::minimize(&dfa);

    Ok(Lexer::new(min_dfa, String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_lexer() {
        let rules = vec![
            ("a", "A"),
            ("b", "B"),
        ];

        let result = build_lexer(rules);
        assert!(result.is_ok());

        let mut lexer = result.unwrap();
        lexer.set_source("a b a b".to_string());

        let tokens = lexer.tokenize();
        for (i, token) in tokens.iter().enumerate() {
            eprintln!("Token {}: {:?} -> {:?}", i, token.name, token.lexeme);
        }
        assert!(tokens.len() >= 2);
    }

    #[test]
    fn test_complex_regex() {
        let rules = vec![
            ("a*b", "A_STAR_B"),
            ("cc*", "C_PLUS"),
        ];

        let result = build_lexer(rules);
        assert!(result.is_ok());

        let mut lexer = result.unwrap();
        lexer.set_source("aaab ccc".to_string());

        let tokens = lexer.tokenize();
        assert!(tokens.len() >= 2);
    }
}
