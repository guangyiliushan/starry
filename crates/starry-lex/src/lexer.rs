use crate::dfa::Dfa;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub name: String,
    pub lexeme: String,
}

pub struct Lexer {
    dfa: Dfa,
    source: String,
    position: usize,
}

impl Lexer {
    pub fn new(dfa: Dfa, source: String) -> Self {
        Self {
            dfa,
            source,
            position: 0,
        }
    }

    pub fn set_source(&mut self, source: String) {
        self.source = source;
        self.position = 0;
    }

    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace();

        if self.position >= self.source.len() {
            return None;
        }

        let mut current_state = self.dfa.start;
        let mut last_accepting: Option<(String, usize)> = None;
        let start_pos = self.position;

        let chars: Vec<char> = self.source.chars().collect();

        while self.position < chars.len() {
            let ch = chars[self.position];

            if let Some(dfa_state) = self.dfa.states.get(current_state.0) {
                if let Some(&next_state) = dfa_state.transitions.get(&ch) {
                    current_state = next_state;
                    self.position += 1;

                    if let Some(token_name) = self.dfa.accepts.get(&current_state) {
                        last_accepting = Some((token_name.clone(), self.position));
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        match last_accepting {
            Some((token_name, end_pos)) => {
                let lexeme: String = chars[start_pos..end_pos].iter().collect();
                Some(Token { name: token_name, lexeme })
            }
            None => {
                if self.position < chars.len() {
                    let unknown_char = chars[self.position];
                    self.position += 1;
                    Some(Token {
                        name: "UNKNOWN".to_string(),
                        lexeme: unknown_char.to_string(),
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens
    }

    fn skip_whitespace(&mut self) {
        let chars: Vec<char> = self.source.chars().collect();
        while self.position < chars.len() && chars[self.position].is_whitespace() {
            self.position += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::Dfa;

    fn create_simple_dfa() -> Dfa {
        let mut dfa = Dfa::new();

        let s0 = dfa.add_state();
        let s1 = dfa.add_state();
        let s2 = dfa.add_state();

        dfa.start = s0;
        dfa.mark_accept(s1, "ID".to_string());
        dfa.mark_accept(s2, "NUMBER".to_string());

        dfa.add_transition(s0, 'a', s1);
        dfa.add_transition(s1, 'a', s1);

        dfa.add_transition(s0, '1', s2);
        dfa.add_transition(s2, '1', s2);

        dfa
    }

    #[test]
    fn test_lexer_basic() {
        let dfa = create_simple_dfa();
        let mut lexer = Lexer::new(dfa, "aaa 111".to_string());

        let token1 = lexer.next_token();
        assert_eq!(
            token1,
            Some(Token {
                name: "ID".to_string(),
                lexeme: "aaa".to_string()
            })
        );

        let token2 = lexer.next_token();
        assert_eq!(
            token2,
            Some(Token {
                name: "NUMBER".to_string(),
                lexeme: "111".to_string()
            })
        );

        let token3 = lexer.next_token();
        assert_eq!(token3, None);
    }

    #[test]
    fn test_tokenize() {
        let dfa = create_simple_dfa();
        let mut lexer = Lexer::new(dfa, "aaa 111 a 1".to_string());

        let tokens = lexer.tokenize();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].name, "ID");
        assert_eq!(tokens[1].name, "NUMBER");
        assert_eq!(tokens[2].name, "ID");
        assert_eq!(tokens[3].name, "NUMBER");
    }
}
