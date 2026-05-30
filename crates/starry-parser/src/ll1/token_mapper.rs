use crate::cfg::{ContextFreeGrammar, TerminalId};
use crate::ll1::error::LL1ParseError;
use crate::ll1::parsing_table::ParsingTable;
use starry_ast::{Token, TokenKind, TokenStream};

pub struct TokenMapper<'a> {
    cfg: &'a ContextFreeGrammar,
    table: &'a ParsingTable,
}

impl<'a> TokenMapper<'a> {
    pub fn new(cfg: &'a ContextFreeGrammar, table: &'a ParsingTable) -> Self {
        TokenMapper { cfg, table }
    }

    pub fn get_terminal_id(&self, token: &Token) -> Result<TerminalId, LL1ParseError> {
        self.terminal_id_from_token(&token.kind)
            .ok_or_else(|| {
                LL1ParseError::unknown_terminal(
                    format!("{:?}", token.kind),
                    token.line,
                    token.column,
                )
            })
    }

    pub fn get_current_terminal_id(&self, stream: &TokenStream) -> Result<TerminalId, LL1ParseError> {
        match stream.peek() {
            Some(token) => self.get_terminal_id(token),
            None => Ok(self.table.end_marker()),
        }
    }

    pub fn terminal_id_from_token(&self, kind: &TokenKind) -> Option<TerminalId> {
        crate::token_mapper::terminal_id_from_token(
            kind,
            |name| self.cfg.terminal_map.get(name).copied(),
            self.table.end_marker(),
        )
    }

    pub fn matches_terminal(&self, expected: TerminalId, kind: &TokenKind) -> bool {
        crate::token_mapper::matches_terminal(
            expected,
            kind,
            self.cfg.get_terminal_name(expected),
            self.table.end_marker(),
        )
    }

    pub fn get_terminal_name(&self, id: TerminalId) -> &str {
        self.cfg.get_terminal_name(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;
    use crate::ll1::parsing_table::ParsingTableBuilder;

    fn create_test_grammar() -> ContextFreeGrammar {
        ContextFreeGrammar::parse(r#"
            E -> num
        "#).unwrap()
    }

    #[test]
    fn test_map_integer_token() {
        let cfg = create_test_grammar();
        let table = ParsingTableBuilder::build_from_grammar(&cfg).unwrap();
        let mapper = TokenMapper::new(&cfg, &table);
        let kind = TokenKind::Integer(42);
        let result = mapper.terminal_id_from_token(&kind);
        assert!(result.is_some());
    }
}
