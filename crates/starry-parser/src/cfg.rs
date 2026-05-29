use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NonTerminalId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    NonTerminal(NonTerminalId),
    Terminal(TerminalId),
    Epsilon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub lhs: NonTerminalId,
    pub rhs: Vec<Symbol>,
}

impl Production {
    pub fn new(lhs: NonTerminalId, rhs: Vec<Symbol>) -> Self {
        Production { lhs, rhs }
    }

    pub fn epsilon(lhs: NonTerminalId) -> Self {
        Production {
            lhs,
            rhs: vec![Symbol::Epsilon],
        }
    }

    pub fn is_epsilon(&self) -> bool {
        self.rhs.len() == 1 && matches!(self.rhs[0], Symbol::Epsilon)
    }
}

#[derive(Debug, Clone)]
pub struct ContextFreeGrammar {
    pub non_terminals: Vec<String>,
    pub terminals: Vec<String>,
    pub start_symbol: NonTerminalId,
    pub productions: Vec<Production>,
    pub terminal_map: HashMap<String, TerminalId>,
    pub non_terminal_map: HashMap<String, NonTerminalId>,
}

impl ContextFreeGrammar {
    pub fn new() -> Self {
        ContextFreeGrammar {
            non_terminals: Vec::new(),
            terminals: Vec::new(),
            start_symbol: NonTerminalId(0),
            productions: Vec::new(),
            terminal_map: HashMap::new(),
            non_terminal_map: HashMap::new(),
        }
    }

    pub fn add_non_terminal(&mut self, name: &str) -> NonTerminalId {
        if let Some(&id) = self.non_terminal_map.get(name) {
            id
        } else {
            let id = NonTerminalId(self.non_terminals.len());
            self.non_terminals.push(name.to_string());
            self.non_terminal_map.insert(name.to_string(), id);
            id
        }
    }

    pub fn add_terminal(&mut self, name: &str) -> TerminalId {
        if let Some(&id) = self.terminal_map.get(name) {
            id
        } else {
            let id = TerminalId(self.terminals.len());
            self.terminals.push(name.to_string());
            self.terminal_map.insert(name.to_string(), id);
            id
        }
    }

    pub fn set_start(&mut self, id: NonTerminalId) {
        self.start_symbol = id;
    }

    pub fn add_production(&mut self, production: Production) {
        self.productions.push(production);
    }

    pub fn add_production_from_str(&mut self, lhs: &str, rhs: Vec<&str>) {
        let lhs_id = self.add_non_terminal(lhs);
        
        let rhs_symbols: Vec<Symbol> = if rhs.len() == 1 && rhs[0] == "ε" {
            vec![Symbol::Epsilon]
        } else {
            rhs.iter()
                .map(|sym| {
                    if sym.chars().next().map_or(false, |c| c.is_uppercase()) {
                        Symbol::NonTerminal(self.add_non_terminal(sym))
                    } else {
                        Symbol::Terminal(self.add_terminal(sym))
                    }
                })
                .collect()
        };

        self.add_production(Production::new(lhs_id, rhs_symbols));
    }

    pub fn get_non_terminal_name(&self, id: NonTerminalId) -> &str {
        &self.non_terminals[id.0]
    }

    pub fn get_terminal_name(&self, id: TerminalId) -> &str {
        if id.0 < self.terminals.len() {
            &self.terminals[id.0]
        } else {
            "$"
        }
    }

    pub fn get_productions_for(&self, non_terminal: NonTerminalId) -> Vec<&Production> {
        self.productions
            .iter()
            .filter(|p| p.lhs == non_terminal)
            .collect()
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let mut cfg = ContextFreeGrammar::new();
        let mut first_production = true;

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

            let lhs_id = cfg.add_non_terminal(lhs);
            
            if first_production {
                cfg.set_start(lhs_id);
                first_production = false;
            }

            let alternatives: Vec<&str> = rhs.split('|').collect();
            for alt in alternatives {
                let alt = alt.trim();
                
                if alt == "ε" || alt == "epsilon" {
                    cfg.add_production(Production::epsilon(lhs_id));
                } else {
                    let symbols: Vec<&str> = alt.split_whitespace().collect();
                    let rhs_symbols: Vec<Symbol> = symbols
                        .iter()
                        .map(|sym| {
                            if sym.chars().next().map_or(false, |c| c.is_uppercase()) {
                                Symbol::NonTerminal(cfg.add_non_terminal(sym))
                            } else {
                                Symbol::Terminal(cfg.add_terminal(sym))
                            }
                        })
                        .collect();
                    
                    cfg.add_production(Production::new(lhs_id, rhs_symbols));
                }
            }
        }

        Ok(cfg)
    }

    pub fn print(&self) {
        println!("Context-Free Grammar:");
        println!("  Start: {}", self.non_terminals[self.start_symbol.0]);
        println!("  Non-terminals: {:?}", self.non_terminals);
        println!("  Terminals: {:?}", self.terminals);
        println!("  Productions:");

        // 按非终结符分组，用 | 分隔多个产生式
        use std::collections::BTreeMap;
        let mut grouped: BTreeMap<usize, Vec<Vec<String>>> = BTreeMap::new();

        for production in &self.productions {
            let rhs_strings: Vec<String> = production.rhs.iter().map(|symbol| {
                match symbol {
                    Symbol::NonTerminal(id) => self.non_terminals[id.0].clone(),
                    Symbol::Terminal(id) => self.terminals[id.0].clone(),
                    Symbol::Epsilon => "ε".to_string(),
                }
            }).collect();

            grouped.entry(production.lhs.0).or_default().push(rhs_strings);
        }

        for (lhs_id, alternatives) in grouped {
            print!("    {} -> ", self.non_terminals[lhs_id]);
            for (i, rhs) in alternatives.iter().enumerate() {
                if i > 0 {
                    print!(" | ");
                }
                for (j, sym) in rhs.iter().enumerate() {
                    if j > 0 {
                        print!(" ");
                    }
                    print!("{}", sym);
                }
            }
            println!();
        }
    }
}

impl Default for ContextFreeGrammar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Symbol::NonTerminal(id) => write!(f, "N{}", id.0),
            Symbol::Terminal(id) => write!(f, "T{}", id.0),
            Symbol::Epsilon => write!(f, "ε"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_creation() {
        let mut cfg = ContextFreeGrammar::new();
        
        let expr = cfg.add_non_terminal("Expr");
        let term = cfg.add_non_terminal("Term");
        let _num = cfg.add_terminal("NUM");
        let plus = cfg.add_terminal("+");
        
        cfg.set_start(expr);
        
        cfg.add_production(Production::new(expr, vec![
            Symbol::NonTerminal(term),
        ]));
        
        cfg.add_production(Production::new(expr, vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(term),
        ]));
        
        assert_eq!(cfg.non_terminals.len(), 2);
        assert_eq!(cfg.terminals.len(), 2);
        assert_eq!(cfg.productions.len(), 2);
    }

    #[test]
    fn test_cfg_parse() {
        let input = r#"
            Expr -> Term | Expr + Term
            Term -> num
        "#;
        
        let cfg = ContextFreeGrammar::parse(input).unwrap();
        
        assert_eq!(cfg.non_terminals.len(), 2);
        assert_eq!(cfg.terminals.len(), 2);
        assert_eq!(cfg.productions.len(), 3);
    }

    #[test]
    fn test_cfg_epsilon() {
        let input = r#"
            S -> a A | ε
            A -> b
        "#;
        
        let cfg = ContextFreeGrammar::parse(input).unwrap();
        
        let epsilon_prod = cfg.productions.iter()
            .find(|p| p.is_epsilon())
            .unwrap();
        
        assert!(epsilon_prod.is_epsilon());
    }
}
