use super::ast::{Production, RegularGrammar};

impl RegularGrammar {
    pub fn print(&self) {
        println!("Regular Grammar:");
        println!("  Start: {}", self.non_terminals[self.start_symbol.0]);
        println!("  Productions:");
        for (non_terminal, productions) in &self.productions {
            let nt_name = &self.non_terminals[non_terminal.0];
            print!("    {} -> ", nt_name);
            
            for (i, production) in productions.iter().enumerate() {
                if i > 0 {
                    print!(" | ");
                }
                
                match production {
                    Production::Epsilon => print!("ε"),
                    Production::Terminal(ch) => print!("{}", ch),
                    Production::TerminalNonTerminal(ch, nt) => {
                        if *ch != '\0' {
                            print!("{}{}", ch, self.non_terminals[nt.0]);
                        } else {
                            print!("{}", self.non_terminals[nt.0]);
                        }
                    }
                }
            }
            println!();
        }
    }
}
