use std::collections::HashMap;

use super::ast::{NonTerminalId, Production, RegularGrammar};

impl RegularGrammar {
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut grammar = RegularGrammar::new();
        let mut symbol_map: HashMap<String, NonTerminalId> = HashMap::new();
        let mut lines_vec: Vec<(&str, &str)> = Vec::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split("->").collect();
            if parts.len() != 2 {
                return Err(format!("Invalid production: {}", line));
            }

            let lhs = parts[0].trim();
            let rhs = parts[1].trim();

            if !symbol_map.contains_key(lhs) {
                let id = grammar.add_non_terminal(lhs);
                symbol_map.insert(lhs.to_string(), id);
            }

            lines_vec.push((lhs, rhs));
        }

        if let Some(first_lhs) = lines_vec.first().map(|(lhs, _)| *lhs) {
            if let Some(&id) = symbol_map.get(first_lhs) {
                grammar.start_symbol = id;
            }
        }

        for (lhs, rhs) in lines_vec {
            let non_terminal_id = symbol_map[lhs];
            let productions = Self::parse_rhs_with_grammar(rhs, &mut grammar, &mut symbol_map)?;
            for prod in productions {
                grammar.add_production(non_terminal_id, prod);
            }
        }

        Ok(grammar)
    }

    fn parse_rhs_with_grammar(
        rhs: &str,
        grammar: &mut RegularGrammar,
        symbol_map: &mut HashMap<String, NonTerminalId>,
    ) -> Result<Vec<Production>, String> {
        let mut productions = Vec::new();
        
        let alternatives: Vec<&str> = rhs.split('|').collect();
        
        for alt in alternatives {
            let alt = alt.trim();
            
            if alt == "ε" || alt == "epsilon" || alt.is_empty() {
                productions.push(Production::Epsilon);
                continue;
            }

            let chars: Vec<char> = alt.chars().collect();
            
            if chars.len() == 1 {
                let ch = chars[0];
                if ch.is_uppercase() {
                    let id = Self::get_or_create_non_terminal(&ch.to_string(), grammar, symbol_map);
                    productions.push(Production::TerminalNonTerminal('\0', id));
                } else {
                    productions.push(Production::Terminal(ch));
                }
            } else if chars.len() == 2 {
                let terminal = chars[0];
                let non_terminal = chars[1];
                
                let id = Self::get_or_create_non_terminal(&non_terminal.to_string(), grammar, symbol_map);
                productions.push(Production::TerminalNonTerminal(terminal, id));
            } else {
                return Err(format!("Invalid production right-hand side: {}", alt));
            }
        }

        Ok(productions)
    }

    fn get_or_create_non_terminal(
        name: &str,
        grammar: &mut RegularGrammar,
        symbol_map: &mut HashMap<String, NonTerminalId>,
    ) -> NonTerminalId {
        if let Some(&id) = symbol_map.get(name) {
            id
        } else {
            let id = grammar.add_non_terminal(name);
            symbol_map.insert(name.to_string(), id);
            id
        }
    }
}
