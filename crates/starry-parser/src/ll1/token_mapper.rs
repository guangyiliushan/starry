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
        match kind {
            TokenKind::Identifier(name) => self.cfg.terminal_map.get(name).copied(),
            TokenKind::Keyword(kw) => self.cfg.terminal_map.get(kw).copied(),
            TokenKind::Operator(op) => self.cfg.terminal_map.get(op).copied(),
            TokenKind::Delimiter(del) => self.cfg.terminal_map.get(del).copied(),
            TokenKind::Integer(_) => self
                .cfg
                .terminal_map
                .get("NUM")
                .or_else(|| self.cfg.terminal_map.get("num"))
                .copied(),
            TokenKind::Float(_) => self
                .cfg
                .terminal_map
                .get("NUM")
                .or_else(|| self.cfg.terminal_map.get("num"))
                .copied(),
            TokenKind::StringLiteral(_) => self.cfg.terminal_map.get("STRING").copied(),
            TokenKind::Eof => Some(self.table.end_marker()),
            _ => None,
        }
    }

    pub fn matches_terminal(&self, expected: TerminalId, kind: &TokenKind) -> bool {
        let expected_name = self.cfg.get_terminal_name(expected);

        match kind {
            TokenKind::Identifier(name) => name == expected_name,
            TokenKind::Keyword(kw) => kw == expected_name,
            TokenKind::Operator(op) => op == expected_name,
            TokenKind::Delimiter(del) => del == expected_name,
            TokenKind::Integer(_) | TokenKind::Float(_) => {
                expected_name.eq_ignore_ascii_case("NUM") || expected_name == "num"
            }
            TokenKind::StringLiteral(_) => expected_name == "STRING",
            TokenKind::Eof => expected == self.table.end_marker(),
            _ => false,
        }
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
