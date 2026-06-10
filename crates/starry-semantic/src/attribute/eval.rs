use starry_ast::AstNode;

use crate::attribute::context::{AttrContext, Pass, WarmupStats};
use crate::attribute::env::Env;
use crate::attribute::result::AttrResult;
use crate::ir::basic_block::BasicBlock;
use crate::ir::quadruple::Quadruple;
use crate::ir::tac::TacInstr;

/// 属性求值器 trait
///
/// 对应龙书 SDT（语法制导翻译）中嵌入到产生式 first/last 位置的语义动作。
/// 实现者负责具体的语义逻辑（类型检查、符号表操作、常量折叠等），
/// 而遍历编排由 [`evaluate`] 函数统一处理。
///
/// # 设计原理
///
/// - `enter` 方法在**前序阶段**调用（进入节点时）：计算继承属性，返回新的上下文
///   供子节点使用。典型操作：创建新作用域、传递 expected_return 等。
///
/// - `leave` 方法在**后序阶段**调用（离开节点时）：所有子节点已遍历完毕，
///   子节点的综合属性通过 `child_results` 传入。典型操作：类型检查、
///   常量折叠、计算当前节点的类型等。
pub trait AttrEvaluator {
    /// 进入节点（前序 / first 位置）
    fn enter(&mut self, node: &AstNode, ctx: &AttrContext, env: &mut Env) -> AttrContext;

    /// 离开节点（后序 / last 位置）
    fn leave(
        &mut self,
        node: &AstNode,
        ctx: &AttrContext,
        child_results: &[AttrResult],
        env: &mut Env,
    ) -> AttrResult;
}

/// 遍次求值器 trait
///
/// 每个语义分析遍次实现此接口。`run` 方法内部调用 [`evaluate`]
/// 完成完整的 AST DFS 遍历，返回该遍次的产出。
pub trait PassEvaluator {
    /// 当前遍次的标识
    fn pass(&self) -> Pass;

    /// 执行当前遍次，返回遍次结果
    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput;
}

/// 遍次产出
///
/// 每个遍次的 `run` 方法返回此枚举，携带该遍次特有的结果数据。
#[derive(Debug)]
pub enum PassOutput {
    /// 预热遍产出
    Warmup(WarmupStats),
    /// 名称解析遍产出
    NameResolution,
    /// 类型检查遍产出
    TypeChecking,
    /// IR 生成遍产出
    IrGeneration(IrModule),
}

/// IR 模块（一个编译单元的完整中间表示）
///
/// 包含从 AST 生成的全部中间代码，提供多种表示形式：
/// - 三地址码（TAC）：线性的指令序列
/// - 四元式：统一操作符的指令序列
/// - 基本块：切分后的基本块列表
#[derive(Debug, Clone)]
pub struct IrModule {
    /// TAC 指令序列
    pub instructions: Vec<TacInstr>,
    /// 四元式序列
    pub quadruples: Vec<Quadruple>,
    /// 基本块列表
    pub basic_blocks: Vec<BasicBlock>,
}

impl IrModule {
    pub fn new(instructions: Vec<TacInstr>) -> Self {
        let quadruples = instructions.iter().cloned().map(Quadruple::from).collect();
        let basic_blocks = BasicBlock::split(&instructions);
        Self {
            instructions,
            quadruples,
            basic_blocks,
        }
    }

    pub fn empty() -> Self {
        Self {
            instructions: Vec::new(),
            quadruples: Vec::new(),
            basic_blocks: Vec::new(),
        }
    }
}

/// 深度优先遍历 AST，编排属性文法计算
///
/// 这是属性文法计算的**核心编排函数**，实现龙书所述的
/// "一入一出"深度优先遍历策略：
///
/// 1. **前序（进入节点）**：调用 `evaluator.enter()` → 计算继承属性 → 获得子上下文
/// 2. **递归子节点**：从左到右深度优先遍历所有子节点 → 收集 `AttrResult`
/// 3. **后序（离开节点）**：调用 `evaluator.leave()` → 计算综合属性 → 返回给父节点
pub fn evaluate<E: AttrEvaluator>(
    evaluator: &mut E,
    node: &AstNode,
    ctx: &AttrContext,
    env: &mut Env,
) -> AttrResult {
    let child_ctx = evaluator.enter(node, ctx, env);

    let mut child_results = Vec::new();
    for child in node.children() {
        let result = evaluate(evaluator, child, &child_ctx, env);
        child_results.push(result);
    }

    evaluator.leave(node, ctx, &child_results, env)
}
