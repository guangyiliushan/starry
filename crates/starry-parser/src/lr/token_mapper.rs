use crate::cfg::TerminalId;
use crate::lr::error::LRError;
use starry_ast::{Token, TokenKind};
use std::collections::HashMap;

pub struct TokenMapper {
    terminal_map: HashMap<String, TerminalId>,
    terminal_names: Vec<String>,
    end_marker: TerminalId,
}

impl TokenMapper {
    pub fn new(
        terminal_map: HashMap<String, TerminalId>,
        terminal_names: Vec<String>,
        end_marker: TerminalId,
    ) -> Self {
        TokenMapper {
            terminal_map,
            terminal_names,
            end_marker,
        }
    }

    pub fn end_marker(&self) -> TerminalId {
        self.end_marker
    }

    pub fn tokenize(&self, tokens: &[Token]) -> Result<Vec<TerminalId>, LRError> {
        let mut result = Vec::with_capacity(tokens.len());

        for (i, token) in tokens.iter().enumerate() {
            let terminal_id = self.get_terminal_id(token).map_err(|mut e| {
                e.position = Some(i);
                e
            })?;
            if terminal_id != self.end_marker {
                result.push(terminal_id);
            }
        }

        Ok(result)
    }

    pub fn get_terminal_id(&self, token: &Token) -> Result<TerminalId, LRError> {
        self.terminal_id_from_token(&token.kind)
            .ok_or_else(|| {
                LRError::invalid_grammar(format!(
                    "Unknown token: {:?} at line {} col {}",
                    token.kind, token.line, token.column
                ))
            })
    }

    pub fn terminal_id_from_token(&self, kind: &TokenKind) -> Option<TerminalId> {
        crate::token_mapper::terminal_id_from_token(
            kind,
            |name| self.terminal_map.get(name).copied(),
            self.end_marker,
        )
    }

    pub fn matches_terminal(&self, expected: TerminalId, kind: &TokenKind) -> bool {
        crate::token_mapper::matches_terminal(
            expected,
            kind,
            self.get_terminal_name(expected),
            self.end_marker,
        )
    }

    pub fn get_terminal_name(&self, id: TerminalId) -> &str {
        &self.terminal_names[id.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;

    fn create_mapper() -> TokenMapper {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        TokenMapper::new(cfg.terminal_map.clone(), cfg.terminals.clone(), end_marker)
    }

    #[test]
    fn test_map_integer_token() {
        let mapper = create_mapper();
        let kind = TokenKind::Integer(42);
        let result = mapper.terminal_id_from_token(&kind);
        assert!(result.is_some());
    }

    #[test]
    fn test_map_identifier_token() {
        let cfg = ContextFreeGrammar::parse("E -> id").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let mapper = TokenMapper::new(cfg.terminal_map.clone(), cfg.terminals.clone(), end_marker);
        let kind = TokenKind::Identifier("id".to_string());
        let result = mapper.terminal_id_from_token(&kind);
        assert!(result.is_some());
    }

    #[test]
    fn test_map_eof() {
        let mapper = create_mapper();
        let result = mapper.terminal_id_from_token(&TokenKind::Eof);
        assert_eq!(result, Some(mapper.end_marker));
    }

    #[test]
    fn test_matches_terminal() {
        let mapper = create_mapper();
        let num_id = mapper.terminal_id_from_token(&TokenKind::Integer(42)).unwrap();
        assert!(mapper.matches_terminal(num_id, &TokenKind::Integer(99)));
        assert!(mapper.matches_terminal(num_id, &TokenKind::Float(3.14)));
    }

    #[test]
    fn test_get_terminal_name() {
        let mapper = create_mapper();
        let num_id = mapper.terminal_id_from_token(&TokenKind::Integer(42)).unwrap();
        let name = mapper.get_terminal_name(num_id);
        assert_eq!(name, "num");
    }

    #[test]
    fn test_tokenize_simple() {
        let mapper = create_mapper();
        let tokens = vec![
            Token::new(TokenKind::Integer(42), "42".to_string(), 0, 0),
        ];
        let result = mapper.tokenize(&tokens).unwrap();
        assert_eq!(result.len(), 1);
    }
}
