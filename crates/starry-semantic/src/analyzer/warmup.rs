use starry_ast::AstNode;

use crate::attribute::context::{AttrContext, Pass, WarmupStats};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, PassEvaluator, PassOutput, evaluate};
use crate::attribute::result::AttrResult;
use crate::ty::Ty;

/// 预热遍求值器
///
/// 纯计数 DFS，统计节点数量、最大作用域深度、估计符号数量。
/// 产出 `WarmupStats`，供后续遍历预分配内存。
#[derive(Debug)]
pub struct WarmupEvaluator {
    node_count: usize,
    max_scope_depth: usize,
    current_depth: usize,
    estimated_symbol_count: usize,
}

impl WarmupEvaluator {
    pub fn new() -> Self {
        Self {
            node_count: 0,
            max_scope_depth: 0,
            current_depth: 0,
            estimated_symbol_count: 0,
        }
    }

    pub fn into_stats(self) -> WarmupStats {
        WarmupStats {
            node_count: self.node_count,
            max_scope_depth: self.max_scope_depth,
            estimated_symbol_count: self.estimated_symbol_count,
        }
    }
}

impl Default for WarmupEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl AttrEvaluator for WarmupEvaluator {
    fn enter(&mut self, node: &AstNode, ctx: &AttrContext, env: &mut Env) -> AttrContext {
        self.node_count += 1;

        match node {
            AstNode::Block(_) => {
                self.current_depth += 1;
                if self.current_depth > self.max_scope_depth {
                    self.max_scope_depth = self.current_depth;
                }
                let new_scope = env.push_scope();
                ctx.clone().with_scope(new_scope)
            }
            AstNode::Ident(_) => {
                self.estimated_symbol_count += 1;
                ctx.clone()
            }
            _ => ctx.clone(),
        }
    }

    fn leave(
        &mut self,
        node: &AstNode,
        _ctx: &AttrContext,
        _child_results: &[AttrResult],
        env: &mut Env,
    ) -> AttrResult {
        if matches!(node, AstNode::Block(_)) {
            self.current_depth = self.current_depth.saturating_sub(1);
            env.pop_scope();
        }
        AttrResult::computed(Ty::Void)
    }
}

impl PassEvaluator for WarmupEvaluator {
    fn pass(&self) -> Pass {
        Pass::Warmup
    }

    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput {
        let warmup_ctx = ctx.clone().with_pass(Pass::Warmup);
        let _ = evaluate(self, root, &warmup_ctx, env);
        PassOutput::Warmup(self.clone().into_stats())
    }
}

impl Clone for WarmupEvaluator {
    fn clone(&self) -> Self {
        Self {
            node_count: self.node_count,
            max_scope_depth: self.max_scope_depth,
            current_depth: self.current_depth,
            estimated_symbol_count: self.estimated_symbol_count,
        }
    }
}
