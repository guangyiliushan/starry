use starry_ast::{AstNode, BinaryOp, LiteralValue, UnaryOp};

use crate::attribute::context::{AttrContext, Pass};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, IrModule, PassEvaluator, PassOutput, evaluate};
use crate::attribute::result::AttrResult;
use crate::error::SemanticError;
use crate::ir::operand::Operand;
use crate::ir::tac::TacInstr;
use crate::ir::{LabelManager, TempManager};
use crate::rules::action::{
    Action, AttrField, BinValOp, BuiltinFn, ErrorTemplate, FoldOp, InstrTemplate, UnValOp,
    ValueExpr,
};
use crate::rules::table::RuleTable;
use crate::ty::Ty;

#[derive(Debug)]
pub struct RuleDrivenEvaluator {
    rule_table: RuleTable,
    pass: Pass,
    errors: Vec<SemanticError>,
    temp_mgr: TempManager,
    _label_mgr: LabelManager,
}

impl RuleDrivenEvaluator {
    pub fn new(rule_table: RuleTable, pass: Pass) -> Self {
        Self {
            rule_table,
            pass,
            errors: Vec::new(),
            temp_mgr: TempManager::new(),
            _label_mgr: LabelManager::new(),
        }
    }

    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<SemanticError> {
        self.errors
    }

    fn report(&mut self, error: SemanticError) {
        self.errors.push(error);
    }

    fn new_temp(&mut self) -> Operand {
        Operand::Temp(self.temp_mgr.new_temp())
    }

    fn literal_ty(value: &LiteralValue) -> Ty {
        match value {
            LiteralValue::Integer(_) => Ty::Int,
            LiteralValue::Float(_) => Ty::Float,
            LiteralValue::Bool(_) => Ty::Bool,
            LiteralValue::String(_) => Ty::String,
        }
    }

    fn literal_to_operand(value: &LiteralValue) -> Operand {
        match value {
            LiteralValue::Integer(v) => Operand::IntConst(*v),
            LiteralValue::Float(v) => Operand::FloatConst(*v),
            LiteralValue::Bool(v) => Operand::BoolConst(*v),
            LiteralValue::String(_) => Operand::StrConst(0),
        }
    }

    fn fold_binary(op: &BinaryOp, left: &AttrResult, right: &AttrResult) -> Option<LiteralValue> {
        if !left.is_const || !right.is_const {
            return None;
        }
        let l = left.const_value.as_ref()?;
        let r = right.const_value.as_ref()?;
        match (op, l, r) {
            (BinaryOp::Add, LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                Some(LiteralValue::Integer(a.wrapping_add(*b)))
            }
            (BinaryOp::Sub, LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                Some(LiteralValue::Integer(a.wrapping_sub(*b)))
            }
            (BinaryOp::Mul, LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                Some(LiteralValue::Integer(a.wrapping_mul(*b)))
            }
            (BinaryOp::Div, LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                if *b == 0 { None } else { Some(LiteralValue::Integer(a.wrapping_div(*b))) }
            }
            (BinaryOp::Add, LiteralValue::Float(a), LiteralValue::Float(b)) => {
                Some(LiteralValue::Float(a + b))
            }
            (BinaryOp::Sub, LiteralValue::Float(a), LiteralValue::Float(b)) => {
                Some(LiteralValue::Float(a - b))
            }
            (BinaryOp::Mul, LiteralValue::Float(a), LiteralValue::Float(b)) => {
                Some(LiteralValue::Float(a * b))
            }
            (BinaryOp::Div, LiteralValue::Float(a), LiteralValue::Float(b)) => {
                Some(LiteralValue::Float(a / b))
            }
            _ => None,
        }
    }

    fn fold_unary(op: &UnaryOp, operand: &AttrResult) -> Option<LiteralValue> {
        if !operand.is_const { return None; }
        let v = operand.const_value.as_ref()?;
        match (op, v) {
            (UnaryOp::Neg, LiteralValue::Integer(n)) => Some(LiteralValue::Integer(-*n)),
            (UnaryOp::Neg, LiteralValue::Float(n)) => Some(LiteralValue::Float(-*n)),
            (UnaryOp::Not, LiteralValue::Bool(b)) => Some(LiteralValue::Bool(!*b)),
            _ => None,
        }
    }

    fn get_child_ty(child_results: &[AttrResult], index: usize) -> Ty {
        child_results
            .get(index)
            .map(|c| c.ty.clone())
            .unwrap_or(Ty::Error)
    }

    fn get_child_bool(child_results: &[AttrResult], index: usize, field: AttrField) -> Option<bool> {
        let child = child_results.get(index)?;
        match field {
            AttrField::IsConst => Some(child.is_const),
            AttrField::IsLvalue => Some(child.is_lvalue),
            _ => None,
        }
    }

    fn eval_bool(
        &self,
        expr: &ValueExpr,
        node: &AstNode,
        child_results: &[AttrResult],
        result: &AttrResult,
    ) -> bool {
        match expr {
            ValueExpr::Call { func: BuiltinFn::IsNumeric, args } => {
                let ty = self.eval_ty(&args[0], node, child_results, result);
                ty.is_numeric()
            }
            ValueExpr::Call { func: BuiltinFn::IsCompatible, args } => {
                let left_ty = self.eval_ty(&args[0], node, child_results, result);
                let right_ty = self.eval_ty(&args[1], node, child_results, result);
                left_ty.is_compatible(&right_ty)
            }
            ValueExpr::BinaryVal { op: BinValOp::And, left, right } => {
                self.eval_bool(left, node, child_results, result)
                    && self.eval_bool(right, node, child_results, result)
            }
            ValueExpr::BinaryVal { op: BinValOp::Or, left, right } => {
                self.eval_bool(left, node, child_results, result)
                    || self.eval_bool(right, node, child_results, result)
            }
            ValueExpr::UnaryVal { op: UnValOp::Not, operand } => {
                !self.eval_bool(operand, node, child_results, result)
            }
            ValueExpr::ConstBool(b) => *b,
            _ => false,
        }
    }

    fn eval_ty(
        &self,
        expr: &ValueExpr,
        node: &AstNode,
        child_results: &[AttrResult],
        result: &AttrResult,
    ) -> Ty {
        match expr {
            ValueExpr::ChildAttr { index, field: AttrField::Ty } => {
                Self::get_child_ty(child_results, *index)
            }
            ValueExpr::CurrentAttr(AttrField::Ty) => result.ty.clone(),
            ValueExpr::LiteralTy(v) => Self::literal_ty(v),
            ValueExpr::ConstTy(ty) => ty.clone(),
            ValueExpr::Call { func: BuiltinFn::Promote, args } => {
                let left_ty = self.eval_ty(&args[0], node, child_results, result);
                let right_ty = self.eval_ty(&args[1], node, child_results, result);
                Ty::promote(&left_ty, &right_ty)
            }
            ValueExpr::Call { func: BuiltinFn::LastChild, .. } => {
                child_results.last().map(|c| c.ty.clone()).unwrap_or(Ty::Void)
            }
            ValueExpr::Call { func: BuiltinFn::FirstChild, .. } => {
                child_results.first().map(|c| c.ty.clone()).unwrap_or(Ty::Void)
            }
            _ => Ty::Error,
        }
    }

    fn eval_bool_val(
        &self,
        expr: &ValueExpr,
        child_results: &[AttrResult],
    ) -> bool {
        match expr {
            ValueExpr::ChildAttr { index, field } => {
                Self::get_child_bool(child_results, *index, *field).unwrap_or(false)
            }
            ValueExpr::ConstBool(b) => *b,
            ValueExpr::BinaryVal { op: BinValOp::And, left, right } => {
                self.eval_bool_val(left, child_results) && self.eval_bool_val(right, child_results)
            }
            _ => false,
        }
    }

    fn make_error(&self, template: &ErrorTemplate, node: &AstNode, child_results: &[AttrResult]) -> SemanticError {
        let span = node.span().clone();
        match template {
            ErrorTemplate::InvalidBinaryOp => {
                let op_str = if let AstNode::Binary(expr) = node {
                    format!("{:?}", expr.op)
                } else {
                    "?".to_string()
                };
                let left = child_results.get(0).map(|c| c.ty.clone()).unwrap_or(Ty::Error);
                let right = child_results.get(1).map(|c| c.ty.clone()).unwrap_or(Ty::Error);
                SemanticError::InvalidBinaryOp { op: op_str, left_ty: left, right_ty: right, span }
            }
            ErrorTemplate::InvalidUnaryOp => {
                let op_str = if let AstNode::Unary(expr) = node {
                    match expr.op {
                        UnaryOp::Neg => "-".to_string(),
                        UnaryOp::Not => "!".to_string(),
                        UnaryOp::Pos => "+".to_string(),
                        UnaryOp::Custom(s) => s.to_string(),
                    }
                } else { "?".to_string() };
                let operand_ty = child_results.get(0).map(|c| c.ty.clone()).unwrap_or(Ty::Error);
                SemanticError::InvalidUnaryOp { op: op_str, operand_ty, span }
            }
            ErrorTemplate::UndefinedName => {
                let name = if let AstNode::Ident(ident) = node { ident.name.clone() } else { "?".to_string() };
                SemanticError::UndefinedName { name, span }
            }
            ErrorTemplate::UnresolvedType => {
                let name = if let AstNode::Ident(ident) = node { ident.name.clone() } else { "?".to_string() };
                SemanticError::UnresolvedType { name, span }
            }
            ErrorTemplate::Redeclaration => {
                let name = if let AstNode::Ident(ident) = node { ident.name.clone() } else { "?".to_string() };
                SemanticError::Redeclaration { name, span }
            }
            ErrorTemplate::TypeMismatch => SemanticError::General { message: "type mismatch".to_string(), span },
            ErrorTemplate::General(msg) => SemanticError::General { message: msg.clone(), span },
        }
    }

    fn execute_actions(
        &mut self,
        actions: &[Action],
        node: &AstNode,
        child_results: &[AttrResult],
        env: &mut Env,
        result: &mut AttrResult,
    ) -> bool {
        for action in actions {
            if !self.execute_action(action, node, child_results, env, result) {
                return false;
            }
        }
        true
    }

    fn execute_action(
        &mut self,
        action: &Action,
        node: &AstNode,
        child_results: &[AttrResult],
        env: &mut Env,
        result: &mut AttrResult,
    ) -> bool {
        match action {
            Action::SetSynthesized { field, value } => {
                self.set_synthesized(field, value, node, child_results, result);
                true
            }

            Action::Check { condition, error } => {
                if !self.eval_bool(condition, node, child_results, result) {
                    let err = self.make_error(error, node, child_results);
                    self.report(err);
                    result.ty = Ty::Error;
                    return false;
                }
                true
            }

            Action::FoldIfConst { target_field: _, op_fold } => {
                if !result.is_const { return true; }
                let folded = match op_fold {
                    FoldOp::Binary { op } => {
                        let error_result = AttrResult::error();
                        let left = child_results.get(0).unwrap_or(&error_result);
                        let right = child_results.get(1).unwrap_or(&error_result);
                        Self::fold_binary(op, left, right)
                    }
                    FoldOp::Unary { op } => {
                        let error_result = AttrResult::error();
                        let operand = child_results.get(0).unwrap_or(&error_result);
                        Self::fold_unary(op, operand)
                    }
                    FoldOp::NodeBinary => {
                        if let AstNode::Binary(expr) = node {
                            let error_result = AttrResult::error();
                            let left = child_results.get(0).unwrap_or(&error_result);
                            let right = child_results.get(1).unwrap_or(&error_result);
                            Self::fold_binary(&expr.op, left, right)
                        } else { None }
                    }
                    FoldOp::NodeUnary => {
                        if let AstNode::Unary(expr) = node {
                            let error_result = AttrResult::error();
                            let operand = child_results.get(0).unwrap_or(&error_result);
                            Self::fold_unary(&expr.op, operand)
                        } else { None }
                    }
                };
                if let Some(val) = folded { result.const_value = Some(val); }
                true
            }

            Action::PushScope => { env.push_scope(); true }
            Action::PopScope => { env.pop_scope(); true }

            Action::DefinePlaceholder { .. } => {
                if let AstNode::Ident(ident) = node {
                    if env.resolve_local(&ident.name).is_none() {
                        if let Err(e) = env.define_placeholder(&ident.name) {
                            self.report(e);
                        }
                    }
                }
                true
            }

            Action::LookupSymbol { .. } => {
                if let AstNode::Ident(ident) = node {
                    if let Some(symbol) = env.resolve(&ident.name) {
                        if symbol.is_placeholder() {
                            self.report(SemanticError::UnresolvedType {
                                name: ident.name.clone(),
                                span: ident.span.clone(),
                            });
                            result.ty = Ty::Error;
                            return false;
                        }
                        result.ty = symbol.ty.clone();
                        result.is_const = symbol.is_const;
                        result.is_lvalue = !symbol.is_const;
                    } else {
                        self.report(SemanticError::UndefinedName {
                            name: ident.name.clone(),
                            span: ident.span.clone(),
                        });
                        result.ty = Ty::Error;
                        return false;
                    }
                }
                true
            }

            Action::NewTemp => {
                let temp = self.new_temp();
                result.place = Some(temp);
                true
            }

            Action::Emit { instr_template: InstrTemplate::BinaryAssign } => {
                if let AstNode::Binary(expr) = node {
                    let error_result = AttrResult::error();
                    let left = child_results.get(0).unwrap_or(&error_result);
                    let right = child_results.get(1).unwrap_or(&error_result);

                    if left.is_error() || right.is_error() {
                        result.ty = Ty::Error;
                        return false;
                    }

                    // 将子节点指令合并，常量操作数直接内联到 Binary 指令中
                    let mut instrs = Vec::new();
                    instrs.extend(left.instructions.clone());
                    instrs.extend(right.instructions.clone());

                    let left_place = left.place.clone().unwrap_or_else(|| Operand::IntConst(0));
                    let right_place = right.place.clone().unwrap_or_else(|| Operand::IntConst(0));

                    let res = self.new_temp();
                    instrs.push(TacInstr::Binary {
                        op: expr.op.clone(),
                        result: res.clone(),
                        left: left_place,
                        right: right_place,
                    });

                    let result_ty = match expr.op {
                        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => Ty::promote(&left.ty, &right.ty),
                        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Ty::Bool,
                        BinaryOp::And | BinaryOp::Or => Ty::Bool,
                        BinaryOp::Custom(_) => Ty::Error,
                    };

                    result.ty = result_ty;
                    result.instructions = instrs;
                    result.place = Some(res);
                }
                true
            }

            Action::Emit { instr_template: InstrTemplate::UnaryAssign } => {
                if let AstNode::Unary(expr) = node {
                    let error_result = AttrResult::error();
                    let operand = child_results.get(0).unwrap_or(&error_result);

                    if operand.is_error() {
                        result.ty = Ty::Error;
                        return false;
                    }

                    let mut instrs = Vec::new();
                    instrs.extend(operand.instructions.clone());

                    let operand_place = operand.place.clone().unwrap_or_else(|| Operand::IntConst(0));
                    let res = self.new_temp();
                    instrs.push(TacInstr::Unary {
                        op: expr.op.clone(),
                        result: res.clone(),
                        operand: operand_place,
                    });

                    let result_ty = match expr.op {
                        UnaryOp::Neg | UnaryOp::Pos => operand.ty.clone(),
                        UnaryOp::Not => Ty::Bool,
                        UnaryOp::Custom(_) => Ty::Error,
                    };

                    result.ty = result_ty;
                    result.instructions = instrs;
                    result.place = Some(res);
                }
                true
            }

            Action::Emit { instr_template: InstrTemplate::LoadConst } => {
                // 字面量不发射单独的 LoadConst 指令：常量值直接存入 place，
                // 父节点在 emit Binary/Unary 时直接将常量内联到运算指令中。
                if let AstNode::Literal(expr) = node {
                    let ty = Self::literal_ty(&expr.value);
                    let place = Self::literal_to_operand(&expr.value);
                    result.ty = ty;
                    result.is_const = true;
                    result.const_value = Some(expr.value.clone());
                    result.instructions = Vec::new();
                    result.place = Some(place);
                }
                true
            }

            Action::Emit { instr_template: InstrTemplate::CopyName } => {
                // 标识符引用不发射指令：变量名作为符号引用直接存入 place。
                if let AstNode::Ident(ident) = node {
                    if let Some(symbol) = env.resolve(&ident.name) {
                        let place = Operand::Name(ident.name.clone());
                        result.ty = symbol.ty.clone();
                        result.is_const = symbol.is_const;
                        result.is_lvalue = !symbol.is_const;
                        result.instructions = Vec::new();
                        result.place = Some(place);
                    }
                }
                true
            }

            Action::PassThrough { child_index } => {
                if let Some(child) = child_results.get(*child_index) {
                    *result = child.clone();
                }
                true
            }

            Action::ReturnError => { result.ty = Ty::Error; false }

            Action::SequenceLast => {
                if let Some(last) = child_results.last() {
                    *result = last.clone();
                } else {
                    *result = AttrResult::computed(Ty::Void);
                }
                true
            }

            Action::SequenceFirst => {
                if let Some(first) = child_results.first() {
                    *result = first.clone();
                } else {
                    *result = AttrResult::computed(Ty::Void);
                }
                true
            }

            Action::SetInherited { .. } => true,
        }
    }

    fn set_synthesized(
        &mut self,
        field: &AttrField,
        value: &ValueExpr,
        node: &AstNode,
        child_results: &[AttrResult],
        result: &mut AttrResult,
    ) {
        match field {
            AttrField::Ty => {
                let ty = self.eval_ty(value, node, child_results, result);
                result.ty = ty;
            }
            AttrField::IsConst => {
                let b = self.eval_bool_val(value, child_results);
                result.is_const = b;
            }
            AttrField::IsLvalue => {
                let b = self.eval_bool_val(value, child_results);
                result.is_lvalue = b;
            }
            AttrField::ConstValue | AttrField::Place | AttrField::Instructions => {}
        }
    }
}

impl AttrEvaluator for RuleDrivenEvaluator {
    fn enter(&mut self, node: &AstNode, ctx: &AttrContext, env: &mut Env) -> AttrContext {
        let pre_actions: Vec<Action> = self.rule_table
            .find(node)
            .map(|r| r.pre_actions.clone())
            .unwrap_or_default();

        if !pre_actions.is_empty() {
            let mut result = AttrResult::computed(Ty::Void);
            self.execute_actions(&pre_actions, node, &[], env, &mut result);
        }

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
        let post_actions: Vec<Action> = self.rule_table
            .find(node)
            .map(|r| r.post_actions.clone())
            .unwrap_or_default();

        if !post_actions.is_empty() {
            let mut result = AttrResult::computed(Ty::Void);
            self.execute_actions(&post_actions, node, child_results, env, &mut result);
            if matches!(node, AstNode::Block(_)) {
                env.pop_scope();
            }
            result
        } else {
            self.leave_default(node, child_results, env)
        }
    }
}

impl RuleDrivenEvaluator {
    fn leave_default(
        &mut self,
        node: &AstNode,
        child_results: &[AttrResult],
        env: &mut Env,
    ) -> AttrResult {
        match node {
            AstNode::Root(_) => child_results.last().cloned().unwrap_or_else(|| AttrResult::computed(Ty::Void)),
            AstNode::ExprStmt(_) => child_results.first().cloned().unwrap_or_else(|| AttrResult::computed(Ty::Void)),
            AstNode::Block(_) => {
                env.pop_scope();
                child_results.last().cloned().unwrap_or_else(|| AttrResult::computed(Ty::Void))
            }
            AstNode::Paren(_) => child_results.first().cloned().unwrap_or_else(AttrResult::error),
            _ => AttrResult::computed(Ty::Void),
        }
    }
}

impl PassEvaluator for RuleDrivenEvaluator {
    fn pass(&self) -> Pass {
        self.pass
    }

    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput {
        let pass_ctx = ctx.clone().with_pass(self.pass);
        match self.pass {
            Pass::TypeChecking => {
                let _ = evaluate(self, root, &pass_ctx, env);
                PassOutput::TypeChecking
            }
            Pass::NameResolution => {
                let _ = evaluate(self, root, &pass_ctx, env);
                PassOutput::NameResolution
            }
            Pass::IrGeneration => {
                let result = evaluate(self, root, &pass_ctx, env);
                let ir_module = IrModule::new(result.instructions);
                PassOutput::IrGeneration(ir_module)
            }
            Pass::Warmup => PassOutput::Warmup(crate::attribute::context::WarmupStats {
                node_count: 0,
                max_scope_depth: 0,
                estimated_symbol_count: 0,
            }),
        }
    }
}
