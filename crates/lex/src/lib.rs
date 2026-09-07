pub mod state;
pub mod transition;
pub mod regex;
pub mod nfa;

pub use state::{State, StateGenerator, StateId, StateSet};
pub use transition::{CharClass, PredefinedClass, Transition};
pub use regex::{Ast, Hir, Parser, ParseError, ParseResult, Flags};
pub use nfa::{NFA, Edge, NFAState, Builder, Fragment};

// Re-export TokenKind for convenience
pub use ast::token::TokenKind;
