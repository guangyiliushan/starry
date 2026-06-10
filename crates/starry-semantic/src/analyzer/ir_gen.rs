use starry_ast::{AstNode, BinaryOp, LiteralValue, UnaryOp};

use crate::attribute::context::{AttrContext, Pass};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, PassEvaluator, PassOutput, evaluate};
use crate::attribute::result::AttrResult;
use crate::ir::{LabelManager, TempManager};
use crate::ir::operand::Operand;
use crate::ir::tac::TacInstr;
use crate::attribute::eval::IrModule;
use crate::ty::Ty;

/// IR 生成遍求值器
///
/// S属性，自底向上遍历 AST，生成三地址码（TAC）。
/// 每个表达式节点在 `leave` 时分配临时变量并生成对应的 TAC 指令。
#[derive(Debug)]
pub struct IrGenerator {
    temp_mgr: TempManager,
    _label_mgr: LabelManager,
    instructions: Vec<TacInstr>,
}

impl IrGenerator {
    pub fn new() -> Self {
        Self {
            temp_mgr: TempManager::new(),
            _label_mgr: LabelManager::new(),
            instructions: Vec::new(),
        }
    }

    pub fn into_ir_module(self) -> IrModule {
        IrModule::new(self.instructions)
    }

    fn new_temp(&mut self) -> Operand {
        Operand::Temp(self.temp_mgr.new_temp())
    }

    fn literal_to_operand(&self, value: &LiteralValue) -> Operand {
        match value {
            LiteralValue::Integer(v) => Operand::IntConst(*v),
            LiteralValue::Float(v) => Operand::FloatConst(*v),
            LiteralValue::Bool(v) => Operand::BoolConst(*v),
            LiteralValue::String(_) => Operand::StrConst(0),
        }
    }
}

impl Default for IrGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl AttrEvaluator for IrGenerator {
    fn enter(&mut self, node: &AstNode, ctx: &AttrContext, env: &mut Env) -> AttrContext {
        match node {
            AstNode::Block(_) => {
                let new_scope = env.push_scope();
                ctx.clone().with_scope(new_scope)
            }
            _ => ctx.clone(),
        }
    }

    fn leave(
        &mut self,
        node: &AstNode,
        _ctx: &AttrContext,
        child_results: &[AttrResult],
        env: &mut Env,
    ) -> AttrResult {
        match node {
            AstNode::Root(_) => {
                let mut all_instrs = Vec::new();
                for cr in child_results {
                    all_instrs.extend(cr.instructions.clone());
                }
                self.instructions.extend(all_instrs);
                AttrResult::computed(Ty::Void)
            }

            AstNode::ExprStmt(_) => {
                if let Some(result) = child_results.first() {
                    let mut r = result.clone();
                    self.instructions.extend(r.instructions.clone());
                    r.ty = Ty::Void;
                    r
                } else {
                    AttrResult::computed(Ty::Void)
                }
            }

            AstNode::Block(_) => {
                env.pop_scope();
                let mut all_instrs = Vec::new();
                let mut last_result = AttrResult::computed(Ty::Void);
                for cr in child_results {
                    all_instrs.extend(cr.instructions.clone());
                    last_result = cr.clone();
                }
                last_result.instructions = all_instrs;
                last_result
            }

            AstNode::Literal(expr) => {
                let ty = match &expr.value {
                    LiteralValue::Integer(_) => Ty::Int,
                    LiteralValue::Float(_) => Ty::Float,
                    LiteralValue::Bool(_) => Ty::Bool,
                    LiteralValue::String(_) => Ty::String,
                };
                // 字面量不发射单独的 LoadConst 指令：常量值直接存入 place，
                // 父节点在 emit 时直接将常量内联到运算指令中（如 t0 = 1 + 2）。
                let place = self.literal_to_operand(&expr.value);
                AttrResult {
                    ty,
                    is_const: true,
                    const_value: Some(expr.value.clone()),
                    is_lvalue: false,
                    instructions: Vec::new(),
                    place: Some(place),
                }
            }

            AstNode::Ident(expr) => {
                if let Some(symbol) = env.resolve(&expr.name) {
                    // 标识符引用不发射指令：变量名作为符号引用直接存入 place，
                    // 父节点在 emit 时直接使用变量名（如 t0 = x + 3）。
                    let place = Operand::Name(expr.name.clone());
                    AttrResult {
                        ty: symbol.ty.clone(),
                        is_const: symbol.is_const,
                        const_value: None,
                        is_lvalue: !symbol.is_const,
                        instructions: Vec::new(),
                        place: Some(place),
                    }
                } else {
                    AttrResult::error()
                }
            }

            AstNode::Binary(expr) => {
                let error_result = AttrResult::error();
                let left = child_results.get(0).unwrap_or(&error_result);
                let right = child_results.get(1).unwrap_or(&error_result);

                if left.is_error() || right.is_error() {
                    return AttrResult::error();
                }

                let mut instrs = Vec::new();
                instrs.extend(left.instructions.clone());
                instrs.extend(right.instructions.clone());

                // 常量直接内联：使用子节点的 place（常量值）直接作为操作数
                let left_place = left.place.clone().unwrap_or_else(|| Operand::IntConst(0));
                let right_place = right.place.clone().unwrap_or_else(|| Operand::IntConst(0));

                let result = self.new_temp();
                instrs.push(TacInstr::Binary {
                    op: expr.op.clone(),
                    result: result.clone(),
                    left: left_place,
                    right: right_place,
                });

                let result_ty = match expr.op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => Ty::promote(&left.ty, &right.ty),
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => Ty::Bool,
                    BinaryOp::And | BinaryOp::Or => Ty::Bool,
                    BinaryOp::Custom(_) => Ty::Error,
                };

                AttrResult {
                    ty: result_ty,
                    is_const: false,
                    const_value: None,
                    is_lvalue: false,
                    instructions: instrs,
                    place: Some(result),
                }
            }

            AstNode::Unary(expr) => {
                let error_result = AttrResult::error();
                let operand = child_results.get(0).unwrap_or(&error_result);

                if operand.is_error() {
                    return AttrResult::error();
                }

                let mut instrs = Vec::new();
                instrs.extend(operand.instructions.clone());

                let operand_place = operand.place.clone().unwrap_or_else(|| Operand::IntConst(0));
                let result = self.new_temp();
                instrs.push(TacInstr::Unary {
                    op: expr.op.clone(),
                    result: result.clone(),
                    operand: operand_place,
                });

                let result_ty = match expr.op {
                    UnaryOp::Neg | UnaryOp::Pos => operand.ty.clone(),
                    UnaryOp::Not => Ty::Bool,
                    UnaryOp::Custom(_) => Ty::Error,
                };

                AttrResult {
                    ty: result_ty,
                    is_const: false,
                    const_value: None,
                    is_lvalue: false,
                    instructions: instrs,
                    place: Some(result),
                }
            }

            AstNode::Paren(_) => {
                if let Some(result) = child_results.first() {
                    result.clone()
                } else {
                    AttrResult::error()
                }
            }
        }
    }
}

impl PassEvaluator for IrGenerator {
    fn pass(&self) -> Pass {
        Pass::IrGeneration
    }

    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput {
        let ir_ctx = ctx.clone().with_pass(Pass::IrGeneration);
        let result = evaluate(self, root, &ir_ctx, env);
        let ir_module = IrModule::new(result.instructions);
        PassOutput::IrGeneration(ir_module)
    }
}

impl Clone for IrGenerator {
    fn clone(&self) -> Self {
        Self {
            temp_mgr: TempManager::new(),
            _label_mgr: LabelManager::new(),
            instructions: self.instructions.clone(),
        }
    }
}
