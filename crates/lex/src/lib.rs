//! # Starry 词法分析管道
//!
//! ```text
//! regex 字符串
//!   | parse      regex::parse           -> Ast   正则字符串 -> 语法树
//!   | translate  regex::Translate       -> Hir   规范化 + (?i) 作用域 + 重复展开
//!   | optimize   NFA::from_hir 内部      -> Hir   不动点等价改写（唯一优化入口）
//!   | subset      dfa::DFA::from_nfa     -> DFA   子集法（原子字母表分区）
//!   | compile    nfa::Builder::compile  -> NFA   Thompson 构造
//!   | simulate   NFA/DFA::match_prefix  -> Option<usize> 最长匹配（字节长度）
//! ```
//!
//! 未实现（按教科书路线推进）：DFA 最小化、lexer 驱动、parser、
//! 语义分析、中间代码与优化 pass、LLVM IR 生成。

pub mod dfa;
pub mod display;
pub mod grammar;
pub mod nfa;
pub mod regex;
pub mod state;
pub mod transition;

pub use dfa::{DfaError, DFA};
pub use display::{write_escaped_char, write_escaped_str};
pub use grammar::{NonTerminalId, Production, RegularGrammar, SymbolId};
pub use nfa::{NFA, Edge, NFAState, Builder, Fragment};
pub use regex::{Ast, Hir, Parser, ParseError, Flags};
pub use state::{State, StateGenerator, StateId, StateSet};
pub use transition::{CharClass, PredefinedClass, Transition};

// 接受态标签类型来自 ast crate；lexer 落地前经此单点转出
pub use ast::token::TokenKind;
