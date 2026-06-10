use starry_ast::AstNode;

use crate::analyzer::{IrGenerator, NameResolutionEvaluator, RuleDrivenEvaluator, TypeCheckEvaluator, WarmupEvaluator};
use crate::rules::table::RuleTable;
use crate::attribute::context::{AttrContext, Pass, WarmupStats};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, IrModule, PassOutput, PassEvaluator};
use crate::attribute::result::AttrResult;
use crate::attribute::store::{AstWalker, AttrStore, NodeAttrs, NodeId};
use crate::dependency::DependencyGraph;
use crate::error::SemanticError;

/// 语义分析管道结果
///
/// 包含四遍语义分析流程的全部产出。
#[derive(Debug)]
pub struct PipelineResult {
    /// 所有遍次中收集到的语义错误
    pub errors: Vec<SemanticError>,
    /// 预热遍产出
    pub warmup_stats: WarmupStats,
    /// IR 生成遍产出
    pub ir_module: IrModule,
    /// 各遍次收集的属性
    pub attr_store: AttrStore,
}

/// 语义分析管道
///
/// 按序执行四遍语义分析流程：
/// 1. **预热遍**：统计节点/作用域/符号数量，为后续遍历预分配内存
/// 2. **名称解析遍**：注册所有声明的名字（含占位符），构建符号表骨架
/// 3. **类型检查遍**：解析类型、执行类型检查与常量折叠
/// 4. **IR 生成遍**：生成三地址码（TAC）/四元式/三元式，构建基本块
#[derive(Debug)]
pub struct Pipeline {
    errors: Vec<SemanticError>,
    warmup_stats: Option<WarmupStats>,
    ir_module: Option<IrModule>,
    attr_store: AttrStore,
    name_resolution_rules: Option<RuleTable>,
    type_check_rules: Option<RuleTable>,
    ir_gen_rules: Option<RuleTable>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warmup_stats: None,
            ir_module: None,
            attr_store: AttrStore::new(),
            name_resolution_rules: None,
            type_check_rules: None,
            ir_gen_rules: None,
        }
    }

    pub fn with_name_resolution_rules(mut self, rules: RuleTable) -> Self {
        self.name_resolution_rules = Some(rules);
        self
    }

    pub fn with_type_check_rules(mut self, rules: RuleTable) -> Self {
        self.type_check_rules = Some(rules);
        self
    }

    pub fn with_ir_gen_rules(mut self, rules: RuleTable) -> Self {
        self.ir_gen_rules = Some(rules);
        self
    }

    /// 执行完整四遍流程
    pub fn run(&mut self, root: &AstNode) -> PipelineResult {
        let warmup_stats = self.run_warmup(root);

        let mut env = Env::with_capacity(warmup_stats.clone());
        let ctx = AttrContext::root(env.current_scope()).with_warmup_stats(warmup_stats.clone());

        self.run_name_resolution(root, &mut env, &ctx);
        self.run_dependency_analysis(&env);
        self.run_type_check(root, &mut env, &ctx);
        let ir_module = self.run_ir_generation(root, &mut env, &ctx);

        PipelineResult {
            errors: self.errors.clone(),
            warmup_stats,
            ir_module,
            attr_store: self.attr_store.clone(),
        }
    }

    /// Pass 0: 预热遍
    pub fn run_warmup(&mut self, root: &AstNode) -> WarmupStats {
        let mut env = Env::new();
        let ctx = AttrContext::root(env.current_scope());
        let mut evaluator = WarmupEvaluator::new();
        
        // 收集预热属性
        let mut walker = AstWalker::new();
        let pass_attrs = &mut self.attr_store.warmup;
        
        walker.walk(root, |_node, node_id, depth| {
            let mut attrs = NodeAttrs::new();
            attrs.depth = depth;
            attrs.ty = Some(crate::ty::Ty::Void);
            pass_attrs.set(node_id, attrs);
        });
        
        self.attr_store.node_count = walker.node_count();
        
        if let PassOutput::Warmup(stats) = evaluator.run(root, &mut env, &ctx) {
            self.warmup_stats = Some(stats.clone());
            self.attr_store.warmup_stats = Some(stats.clone());
            stats
        } else {
            let stats = WarmupStats {
                node_count: 0,
                max_scope_depth: 0,
                estimated_symbol_count: 0,
            };
            self.warmup_stats = Some(stats.clone());
            self.attr_store.warmup_stats = Some(stats.clone());
            stats
        }
    }

    /// Pass 1: 名称解析遍
    pub fn run_name_resolution(
        &mut self,
        root: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
    ) -> &[SemanticError] {
        if let Some(rules) = self.name_resolution_rules.clone() {
            let mut evaluator = RuleDrivenEvaluator::new(rules, Pass::NameResolution);
            evaluator.run(root, env, ctx);
            self.errors.extend(evaluator.into_errors());
        } else {
            let mut evaluator = NameResolutionEvaluator::new();
            evaluator.run(root, env, ctx);
            self.errors.extend(evaluator.errors().iter().cloned());
        }

        let mut walker = AstWalker::new();
        let pass_attrs = &mut self.attr_store.name_resolution;

        walker.walk(root, |node, node_id, depth| {
            let mut attrs = NodeAttrs::new();
            attrs.depth = depth;
            attrs.scope_id = Some(env.current_scope());

            if let starry_ast::AstNode::Ident(ident) = node {
                if let Some(symbol) = env.resolve(&ident.name) {
                    attrs.symbol_name = Some(ident.name.clone());
                    attrs.ty = Some(symbol.ty.clone());
                }
            }

            pass_attrs.set(node_id, attrs);
        });

        &self.errors
    }

    /// 依赖分析（基于名称解析遍产出的符号表）
    pub fn run_dependency_analysis(&mut self, env: &Env) {
        let deps = env.collect_dependencies();
        if !deps.is_empty() {
            let graph = DependencyGraph::from_dependencies(&deps);
            if let Err(cycle) = graph.topological_sort() {
                self.errors.push(SemanticError::CyclicDependency {
                    cycle,
                    span: starry_ast::Span {
                        start: starry_ast::Position { line: 0, col: 0 },
                        end: starry_ast::Position { line: 0, col: 0 },
                    },
                });
            }
        }
    }

    /// Pass 2: 类型检查遍
    pub fn run_type_check(
        &mut self,
        root: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
    ) -> &[SemanticError] {
        if let Some(rules) = self.type_check_rules.clone() {
            let mut evaluator = RuleDrivenEvaluator::new(rules, Pass::TypeChecking);
            let _ = self.run_rule_driven_type_check(root, env, ctx, &mut evaluator, 0, &mut 0);
            self.errors.extend(evaluator.into_errors());
        } else {
            let mut evaluator = TypeCheckEvaluator::new();
            let _ = self.run_type_check_recursive(root, env, ctx, &mut evaluator, 0, &mut 0);
            self.errors.extend(evaluator.errors().iter().cloned());
        }

        &self.errors
    }

    fn run_rule_driven_type_check(
        &mut self,
        node: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
        evaluator: &mut RuleDrivenEvaluator,
        depth: usize,
        node_id_counter: &mut usize,
    ) -> AttrResult {
        let node_id = NodeId(*node_id_counter);
        *node_id_counter += 1;

        let child_ctx = evaluator.enter(node, ctx, env);

        let mut child_results = Vec::new();
        for child in node.children() {
            let result = self.run_rule_driven_type_check(child, env, &child_ctx, evaluator, depth + 1, node_id_counter);
            child_results.push(result);
        }

        let result = evaluator.leave(node, ctx, &child_results, env);

        let mut attrs = NodeAttrs::from_attr_result(&result);
        attrs.depth = depth;
        self.attr_store.type_check.set(node_id, attrs);

        result
    }

    fn run_type_check_recursive(
        &mut self,
        node: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
        evaluator: &mut TypeCheckEvaluator,
        depth: usize,
        node_id_counter: &mut usize,
    ) -> AttrResult {
        let node_id = NodeId(*node_id_counter);
        *node_id_counter += 1;

        let child_ctx = evaluator.enter(node, ctx, env);

        let mut child_results = Vec::new();
        for child in node.children() {
            let result = self.run_type_check_recursive(child, env, &child_ctx, evaluator, depth + 1, node_id_counter);
            child_results.push(result);
        }

        let result = evaluator.leave(node, ctx, &child_results, env);

        let mut attrs = NodeAttrs::from_attr_result(&result);
        attrs.depth = depth;
        self.attr_store.type_check.set(node_id, attrs);

        result
    }

    /// Pass 3: IR 生成遍
    pub fn run_ir_generation(
        &mut self,
        root: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
    ) -> IrModule {
        if let Some(rules) = self.ir_gen_rules.clone() {
            let mut evaluator = RuleDrivenEvaluator::new(rules, Pass::IrGeneration);
            let result = self.run_rule_driven_ir_gen(root, env, ctx, &mut evaluator, 0, &mut 0);
            let ir_module = IrModule::new(result.instructions);
            self.ir_module = Some(ir_module.clone());
            ir_module
        } else {
            let mut evaluator = IrGenerator::new();
            let _ = self.run_ir_gen_recursive(root, env, ctx, &mut evaluator, 0, &mut 0);

            if let PassOutput::IrGeneration(ir_module) = evaluator.run(root, env, ctx) {
                self.ir_module = Some(ir_module.clone());
                return ir_module;
            }
            IrModule::empty()
        }
    }

    fn run_rule_driven_ir_gen(
        &mut self,
        node: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
        evaluator: &mut RuleDrivenEvaluator,
        depth: usize,
        node_id_counter: &mut usize,
    ) -> AttrResult {
        let node_id = NodeId(*node_id_counter);
        *node_id_counter += 1;

        let child_ctx = evaluator.enter(node, ctx, env);

        let mut child_results = Vec::new();
        for child in node.children() {
            let result = self.run_rule_driven_ir_gen(child, env, &child_ctx, evaluator, depth + 1, node_id_counter);
            child_results.push(result);
        }

        let result = evaluator.leave(node, ctx, &child_results, env);

        let mut attrs = NodeAttrs::from_attr_result(&result);
        attrs.depth = depth;
        self.attr_store.ir_generation.set(node_id, attrs);

        result
    }

    fn run_ir_gen_recursive(
        &mut self,
        node: &AstNode,
        env: &mut Env,
        ctx: &AttrContext,
        evaluator: &mut IrGenerator,
        depth: usize,
        node_id_counter: &mut usize,
    ) -> AttrResult {
        let node_id = NodeId(*node_id_counter);
        *node_id_counter += 1;

        let child_ctx = evaluator.enter(node, ctx, env);

        let mut child_results = Vec::new();
        for child in node.children() {
            let result = self.run_ir_gen_recursive(child, env, &child_ctx, evaluator, depth + 1, node_id_counter);
            child_results.push(result);
        }

        let result = evaluator.leave(node, ctx, &child_results, env);

        let mut attrs = NodeAttrs::from_attr_result(&result);
        attrs.depth = depth;
        self.attr_store.ir_generation.set(node_id, attrs);

        result
    }

    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    pub fn warmup_stats(&self) -> Option<&WarmupStats> {
        self.warmup_stats.as_ref()
    }

    pub fn ir_module(&self) -> Option<&IrModule> {
        self.ir_module.as_ref()
    }

    pub fn attr_store(&self) -> &AttrStore {
        &self.attr_store
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷函数：对 AST 执行完整四遍语义分析
pub fn analyze(root: &AstNode) -> PipelineResult {
    let mut pipeline = Pipeline::new();
    pipeline.run(root)
}
