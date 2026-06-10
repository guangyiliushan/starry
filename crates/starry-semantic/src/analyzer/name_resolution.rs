use starry_ast::AstNode;

use crate::attribute::context::{AttrContext, Pass};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, PassEvaluator, PassOutput, evaluate};
use crate::attribute::result::AttrResult;
use crate::error::SemanticError;
use crate::ty::Ty;

/// 名称解析遍求值器
///
/// L属性，自顶向下遍历 AST，注册所有声明的名字（含占位符）。
/// 此遍**只注册名字**，不做类型计算，支持前向引用。
#[derive(Debug)]
pub struct NameResolutionEvaluator {
    errors: Vec<SemanticError>,
}

impl NameResolutionEvaluator {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
        }
    }

    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    fn report(&mut self, error: SemanticError) {
        self.errors.push(error);
    }
}

impl Default for NameResolutionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl AttrEvaluator for NameResolutionEvaluator {
    fn enter(&mut self, node: &AstNode, ctx: &AttrContext, env: &mut Env) -> AttrContext {
        match node {
            AstNode::Block(_) => {
                let new_scope = env.push_scope();
                ctx.clone().with_scope(new_scope)
            }
            AstNode::Ident(ident) => {
                if env.resolve_local(&ident.name).is_none() {
                    if let Err(e) = env.define_placeholder(&ident.name) {
                        self.report(e);
                    }
                }
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
            env.pop_scope();
        }
        AttrResult::computed(Ty::Void)
    }
}

impl PassEvaluator for NameResolutionEvaluator {
    fn pass(&self) -> Pass {
        Pass::NameResolution
    }

    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput {
        let nr_ctx = ctx.clone().with_pass(Pass::NameResolution);
        let _ = evaluate(self, root, &nr_ctx, env);
        PassOutput::NameResolution
    }
}
