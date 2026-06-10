pub mod attribute;
pub mod dependency;
pub mod error;
pub mod ir;
pub mod pipeline;
pub mod rules;
pub mod symbol;
pub mod ty;

pub mod analyzer;

pub use attribute::{AttrContext, AttrEvaluator, AttrResult, AttrStore, AstWalker, Env, IrModule, NodeAttrs, NodeId, Pass, PassAttrs, Scope, ScopeId, WarmupStats, evaluate};
pub use dependency::DependencyGraph;
pub use error::SemanticError;
pub use ir::{
    BasicBlock, Dag, DagNode, LabelId, LabelManager, Operand, Quadruple, TacInstr, TacOp,
    TempId, TempManager, Triple, TripleRef, cse_and_to_triples, cse_quadruples, quadruples_to_triples,
};
pub use pipeline::{Pipeline, PipelineResult, analyze};
pub use rules::{pattern_to_string, parse_rules, build_rule_tables, Action, AttrField, NodePattern, ParsedRule, RuleSection, RuleTable, SemanticRule, ValueExpr};
pub use analyzer::RuleDrivenEvaluator;
pub use symbol::{Symbol, SymbolKind, SymbolStatus};
pub use symbol::Symbol as SymbolEntry;
pub use ty::{Ty, TypeVarId};