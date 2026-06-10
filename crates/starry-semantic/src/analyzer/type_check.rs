use starry_ast::{AstNode, BinaryOp, LiteralValue, UnaryOp};

use crate::attribute::context::{AttrContext, Pass};
use crate::attribute::env::Env;
use crate::attribute::eval::{AttrEvaluator, PassEvaluator, PassOutput, evaluate};
use crate::attribute::result::AttrResult;
use crate::error::SemanticError;
use crate::ty::Ty;

/// 类型检查遍求值器
///
/// 与旧版 Analyzer 逻辑基本相同，但增强：
/// - 遇到占位符时尝试将其解析为具体类型
/// - 发现 Unresolved 类型时报 UnresolvedType 错误
/// - 类型推断：为 auto/var 声明生成类型变量
#[derive(Debug)]
pub struct TypeCheckEvaluator {
    errors: Vec<SemanticError>,
}

impl TypeCheckEvaluator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    fn report(&mut self, error: SemanticError) {
        self.errors.push(error);
    }

    fn literal_ty(value: &LiteralValue) -> Ty {
        match value {
            LiteralValue::Integer(_) => Ty::Int,
            LiteralValue::Float(_) => Ty::Float,
            LiteralValue::Bool(_) => Ty::Bool,
            LiteralValue::String(_) => Ty::String,
        }
    }

    fn fold_binary(
        op: &BinaryOp,
        left: &AttrResult,
        right: &AttrResult,
    ) -> Option<LiteralValue> {
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
                if *b == 0 {
                    None
                } else {
                    Some(LiteralValue::Integer(a.wrapping_div(*b)))
                }
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
        if !operand.is_const {
            return None;
        }
        let v = operand.const_value.as_ref()?;
        match (op, v) {
            (UnaryOp::Neg, LiteralValue::Integer(n)) => Some(LiteralValue::Integer(-*n)),
            (UnaryOp::Neg, LiteralValue::Float(n)) => Some(LiteralValue::Float(-*n)),
            (UnaryOp::Not, LiteralValue::Bool(b)) => Some(LiteralValue::Bool(!*b)),
            _ => None,
        }
    }
}

impl Default for TypeCheckEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl AttrEvaluator for TypeCheckEvaluator {
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
            AstNode::Root(_) => child_results
                .last()
                .cloned()
                .unwrap_or_else(|| AttrResult::computed(Ty::Void)),

            AstNode::ExprStmt(_) => child_results
                .first()
                .cloned()
                .unwrap_or_else(|| AttrResult::computed(Ty::Void)),

            AstNode::Block(_) => {
                env.pop_scope();
                child_results
                    .last()
                    .cloned()
                    .unwrap_or_else(|| AttrResult::computed(Ty::Void))
            }

            AstNode::Binary(expr) => {
                let error_result = AttrResult::error();
                let left = child_results.get(0).unwrap_or(&error_result);
                let right = child_results.get(1).unwrap_or(&error_result);

                if left.is_error() || right.is_error() {
                    return AttrResult::error();
                }

                let result_ty = match expr.op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => {
                        if !left.ty.is_numeric() || !right.ty.is_numeric() {
                            self.report(SemanticError::InvalidBinaryOp {
                                op: format!("{:?}", expr.op),
                                left_ty: left.ty.clone(),
                                right_ty: right.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            Ty::promote(&left.ty, &right.ty)
                        }
                    }
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => {
                        if !left.ty.is_compatible(&right.ty) {
                            self.report(SemanticError::InvalidBinaryOp {
                                op: format!("{:?}", expr.op),
                                left_ty: left.ty.clone(),
                                right_ty: right.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            Ty::Bool
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if left.ty != Ty::Bool || right.ty != Ty::Bool {
                            self.report(SemanticError::InvalidBinaryOp {
                                op: format!("{:?}", expr.op),
                                left_ty: left.ty.clone(),
                                right_ty: right.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            Ty::Bool
                        }
                    }
                    BinaryOp::Custom(ref s) => {
                        self.report(SemanticError::General {
                            message: format!("unsupported custom operator: {}", s),
                            span: expr.span.clone(),
                        });
                        Ty::Error
                    }
                };

                if result_ty == Ty::Error {
                    return AttrResult::error();
                }

                let const_value = Self::fold_binary(&expr.op, left, right);
                let is_const = const_value.is_some();

                AttrResult {
                    ty: result_ty,
                    is_const,
                    const_value,
                    is_lvalue: false,
                    instructions: Vec::new(),
                    place: None,
                }
            }

            AstNode::Unary(expr) => {
                let error_result = AttrResult::error();
                let operand = child_results.get(0).unwrap_or(&error_result);

                if operand.is_error() {
                    return AttrResult::error();
                }

                let result_ty = match expr.op {
                    UnaryOp::Neg => {
                        if !operand.ty.is_numeric() {
                            self.report(SemanticError::InvalidUnaryOp {
                                op: "-".to_string(),
                                operand_ty: operand.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            operand.ty.clone()
                        }
                    }
                    UnaryOp::Not => {
                        if operand.ty != Ty::Bool {
                            self.report(SemanticError::InvalidUnaryOp {
                                op: "!".to_string(),
                                operand_ty: operand.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            Ty::Bool
                        }
                    }
                    UnaryOp::Pos => {
                        if !operand.ty.is_numeric() {
                            self.report(SemanticError::InvalidUnaryOp {
                                op: "+".to_string(),
                                operand_ty: operand.ty.clone(),
                                span: expr.span.clone(),
                            });
                            Ty::Error
                        } else {
                            operand.ty.clone()
                        }
                    }
                    UnaryOp::Custom(ref s) => {
                        self.report(SemanticError::General {
                            message: format!("unsupported custom unary operator: {}", s),
                            span: expr.span.clone(),
                        });
                        Ty::Error
                    }
                };

                if result_ty == Ty::Error {
                    return AttrResult::error();
                }

                let const_value = Self::fold_unary(&expr.op, operand);
                let is_const = const_value.is_some();

                AttrResult {
                    ty: result_ty,
                    is_const,
                    const_value,
                    is_lvalue: false,
                    instructions: Vec::new(),
                    place: None,
                }
            }

            AstNode::Literal(expr) => {
                let ty = Self::literal_ty(&expr.value);
                AttrResult::literal(ty, expr.value.clone())
            }

            AstNode::Ident(expr) => {
                if let Some(symbol) = env.resolve(&expr.name) {
                    if symbol.is_placeholder() {
                        self.report(SemanticError::UnresolvedType {
                            name: expr.name.clone(),
                            span: expr.span.clone(),
                        });
                        return AttrResult::error();
                    }
                    AttrResult {
                        ty: symbol.ty.clone(),
                        is_const: symbol.is_const,
                        const_value: None,
                        is_lvalue: !symbol.is_const,
                        instructions: Vec::new(),
                        place: None,
                    }
                } else {
                    self.report(SemanticError::UndefinedName {
                        name: expr.name.clone(),
                        span: expr.span.clone(),
                    });
                    AttrResult::error()
                }
            }

            AstNode::Paren(_) => child_results
                .first()
                .cloned()
                .unwrap_or_else(AttrResult::error),
        }
    }
}

impl PassEvaluator for TypeCheckEvaluator {
    fn pass(&self) -> Pass {
        Pass::TypeChecking
    }

    fn run(&mut self, root: &AstNode, env: &mut Env, ctx: &AttrContext) -> PassOutput {
        let tc_ctx = ctx.clone().with_pass(Pass::TypeChecking);
        let _ = evaluate(self, root, &tc_ctx, env);
        PassOutput::TypeChecking
    }
}
