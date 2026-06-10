use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LRGrammarType {
    LR0,
    SLR1,
    LALR1,
    LR1,
    NotLR,
    Unknown,
}

impl fmt::Display for LRGrammarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LRGrammarType::LR0 => write!(f, "LR(0)"),
            LRGrammarType::SLR1 => write!(f, "SLR(1)"),
            LRGrammarType::LALR1 => write!(f, "LALR(1)"),
            LRGrammarType::LR1 => write!(f, "LR(1)"),
            LRGrammarType::NotLR => write!(f, "Not LR"),
            LRGrammarType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl LRGrammarType {
    pub fn is_deterministic(&self) -> bool {
        matches!(self, LRGrammarType::LR0 | LRGrammarType::SLR1 | LRGrammarType::LALR1 | LRGrammarType::LR1)
    }

    pub fn is_lr0(&self) -> bool {
        matches!(self, LRGrammarType::LR0)
    }

    pub fn is_slr1(&self) -> bool {
        matches!(self, LRGrammarType::SLR1)
    }

    pub fn is_lalr1(&self) -> bool {
        matches!(self, LRGrammarType::LALR1)
    }

    pub fn is_lr1(&self) -> bool {
        matches!(self, LRGrammarType::LR1)
    }

    pub fn name(&self) -> &'static str {
        match self {
            LRGrammarType::LR0 => "LR(0)",
            LRGrammarType::SLR1 => "SLR(1)",
            LRGrammarType::LALR1 => "LALR(1)",
            LRGrammarType::LR1 => "LR(1)",
            LRGrammarType::NotLR => "Not LR",
            LRGrammarType::Unknown => "Unknown",
        }
    }
}
