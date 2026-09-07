//! # Starry 词法分析管道
//!
//! ```text
//! regex 字符串
//!   | parse      regex::parse               -> Ast
//!   | translate  regex::Translate           -> Hir   规范化 + (?i) + 重复展开
//!   | optimize   NFA::from_hir 内部          -> Hir   不动点等价改写（唯一优化入口）
//!   | subset     dfa::DFA::from_nfa         -> DFA   子集法（原子字母表分区）
//!   | minimize   dfa::minimize              -> DFA   Hopcroft / Moore / Brzozowski
//!   | grammar    grammar::RegularGrammar    -> 文法  与 NFA 等价互转（右线性）
//!   | scan       lexer::Lexer::tokenize     -> Token 最长匹配 + 规则优先级
//! ```
//!
//! 未实现（按教科书路线推进）：parser、语义分析、中间代码与优化
//! pass、LLVM IR 生成。

pub mod dfa;
pub mod display;
pub mod grammar;
pub mod lexer;
pub mod nfa;
pub mod regex;
pub mod state;
pub mod transition;

pub use dfa::{DfaError, DFA};
pub use display::{write_escaped_char, write_escaped_str};
pub use grammar::{NonTerminalId, Production, RegularGrammar, SymbolId};
pub use lexer::{Lexer, LexerError};
pub use nfa::{NFA, Edge, NFAState, Builder, Fragment};
pub use regex::{Ast, Hir, Parser, ParseError, Flags};
pub use state::{State, StateGenerator, StateId, StateSet};
pub use transition::{CharClass, PredefinedClass, Transition};

// 接受态标签类型来自 ast crate；lexer 落地前经此单点转出
pub use ast::token::{LiteralKind, TokenKind};
