use crate::cfg::TerminalId;
use starry_ast::TokenKind;

pub fn terminal_id_from_token<F>(
    kind: &TokenKind,
    resolve: F,
    end_marker: TerminalId,
) -> Option<TerminalId>
where
    F: Fn(&str) -> Option<TerminalId>,
{
    match kind {
        TokenKind::Identifier(name) => resolve(name),
        TokenKind::Keyword(kw) => resolve(kw),
        TokenKind::Operator(op) => resolve(op),
        TokenKind::Delimiter(del) => resolve(del),
        TokenKind::Integer(_) | TokenKind::Float(_) => {
            resolve("NUM").or_else(|| resolve("num"))
        }
        TokenKind::StringLiteral(_) => resolve("STRING"),
        TokenKind::Eof => Some(end_marker),
        _ => None,
    }
}

pub fn matches_terminal(
    expected: TerminalId,
    kind: &TokenKind,
    expected_name: &str,
    end_marker: TerminalId,
) -> bool {
    match kind {
        TokenKind::Identifier(name)
        | TokenKind::Keyword(name)
        | TokenKind::Operator(name)
        | TokenKind::Delimiter(name) => name == expected_name,
        TokenKind::Integer(_) | TokenKind::Float(_) => {
            expected_name.eq_ignore_ascii_case("NUM") || expected_name == "num"
        }
        TokenKind::StringLiteral(_) => expected_name == "STRING",
        TokenKind::Eof => expected == end_marker,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;
    use std::collections::HashMap;

    fn make_resolve(map: &HashMap<String, TerminalId>) -> impl Fn(&str) -> Option<TerminalId> + '_ {
        |name: &str| map.get(name).copied()
    }

    #[test]
    fn test_map_integer_token() {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let resolve = make_resolve(&cfg.terminal_map);
        let result = terminal_id_from_token(&TokenKind::Integer(42), resolve, end_marker);
        assert!(result.is_some());
    }

    #[test]
    fn test_map_identifier_token() {
        let cfg = ContextFreeGrammar::parse("E -> id").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let resolve = make_resolve(&cfg.terminal_map);
        let result = terminal_id_from_token(&TokenKind::Identifier("id".to_string()), resolve, end_marker);
        assert!(result.is_some());
    }

    #[test]
    fn test_map_eof() {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let resolve = make_resolve(&cfg.terminal_map);
        let result = terminal_id_from_token(&TokenKind::Eof, resolve, end_marker);
        assert_eq!(result, Some(end_marker));
    }

    #[test]
    fn test_matches_integer() {
        let cfg = ContextFreeGrammar::parse("E -> num").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let resolve = make_resolve(&cfg.terminal_map);
        let num_id = terminal_id_from_token(&TokenKind::Integer(42), &resolve, end_marker).unwrap();
        assert!(matches_terminal(num_id, &TokenKind::Integer(99), "num", end_marker));
        assert!(matches_terminal(num_id, &TokenKind::Float(3.14), "num", end_marker));
    }

    #[test]
    fn test_matches_terminal_name() {
        let cfg = ContextFreeGrammar::parse("E -> id").unwrap();
        let end_marker = TerminalId(cfg.terminals.len());
        let resolve = make_resolve(&cfg.terminal_map);
        let id = terminal_id_from_token(&TokenKind::Identifier("id".to_string()), &resolve, end_marker).unwrap();
        assert!(matches_terminal(id, &TokenKind::Identifier("id".to_string()), "id", end_marker));
        assert!(!matches_terminal(id, &TokenKind::Identifier("x".to_string()), "id", end_marker));
    }
}
