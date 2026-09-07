//! 正则表达式模块
//!
//! 该模块提供正则表达式的解析、编译和优化功能：
//! - [`ast`] - 抽象语法树（AST）定义
//! - [`hir`] - 高级中间表示（HIR）定义
//! - [`parse`] - 正则表达式解析器
//! - [`translate`] - AST 到 HIR 的翻译器
//! - [`optimize`] - HIR 优化器
//! - [`visitor`] - 迭代遍历器
//!
//! # 架构设计
//!
//! 本模块采用多级表示架构，类似于 Rust 官方 `regex` 库：
//!
//! 1. **AST（抽象语法树）**：忠实保留源码结构，便于错误报告
//! 2. **HIR（高级中间表示）**：经过化简和规范化，便于分析和编译
//!
//! 这种架构的优势：
//! - 更好的错误报告（AST 保留源码信息）
//! - 强大的功能支持（可以在 AST 层处理控制标记）
//! - 更易优化（HIR 的规范形式让分析和优化更容易）
//!
//! # 示例
//!
//! ```
//! use lex::regex::parse;
//!
//! // 解析正则表达式
//! let ast = parse("[a-zA-Z][a-zA-Z0-9_]*").unwrap();
//! println!("AST: {:?}", ast);
//! ```

pub mod ast;
pub mod hir;
pub mod parse;
pub mod translate;
pub mod optimize;
pub mod visitor;

pub use ast::{Ast, Flags};
pub use hir::Hir;
pub use parse::{parse, Parser, ParseError};
pub use translate::{Translate, Translator};
pub use optimize::Optimizer;
