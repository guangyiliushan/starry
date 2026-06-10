use crate::ty::Ty;
use starry_ast::Span;

/// 语义分析阶段发现的错误
///
/// 对应龙书与虎书中语义动作可以产生的各类错误。
/// 每个错误携带源码位置（Span），便于后续生成诊断信息。
#[derive(Debug, Clone)]
pub enum SemanticError {
    /// 使用了未定义的标识符
    UndefinedName {
        name: String,
        span: Span,
    },
    /// 标识符在当前作用域中重复定义
    Redeclaration {
        name: String,
        span: Span,
    },
    /// 类型不匹配（如将字符串赋值给整型变量）
    TypeMismatch {
        expected: Ty,
        found: Ty,
        span: Span,
    },
    /// 二元运算符的操作数类型不兼容
    InvalidBinaryOp {
        op: String,
        left_ty: Ty,
        right_ty: Ty,
        span: Span,
    },
    /// 一元运算符的操作数类型不合法
    InvalidUnaryOp {
        op: String,
        operand_ty: Ty,
        span: Span,
    },
    /// 对非左值表达式进行赋值
    InvalidAssignment {
        span: Span,
    },
    /// 对常量进行重新赋值
    AssignToConstant {
        name: String,
        span: Span,
    },
    /// break / continue 出现在循环外部
    BreakOutsideLoop {
        span: Span,
    },
    /// return 语句的类型与函数声明不符
    ReturnTypeMismatch {
        expected: Ty,
        found: Ty,
        span: Span,
    },
    /// 循环依赖（依赖图中检测到环）
    CyclicDependency {
        cycle: Vec<String>,
        span: Span,
    },
    /// 占位符未被解析（类型检查阶段结束后仍有未解析的类型）
    UnresolvedType {
        name: String,
        span: Span,
    },
    /// 隐式类型转换失败
    ImplicitCastFailed {
        from: Ty,
        to: Ty,
        span: Span,
    },
    /// 其他通用语义错误
    General {
        message: String,
        span: Span,
    },
}

impl SemanticError {
    /// 获取错误对应的源码位置
    pub fn span(&self) -> &Span {
        match self {
            SemanticError::UndefinedName { span, .. } => span,
            SemanticError::Redeclaration { span, .. } => span,
            SemanticError::TypeMismatch { span, .. } => span,
            SemanticError::InvalidBinaryOp { span, .. } => span,
            SemanticError::InvalidUnaryOp { span, .. } => span,
            SemanticError::InvalidAssignment { span, .. } => span,
            SemanticError::AssignToConstant { span, .. } => span,
            SemanticError::BreakOutsideLoop { span, .. } => span,
            SemanticError::ReturnTypeMismatch { span, .. } => span,
            SemanticError::CyclicDependency { span, .. } => span,
            SemanticError::UnresolvedType { span, .. } => span,
            SemanticError::ImplicitCastFailed { span, .. } => span,
            SemanticError::General { span, .. } => span,
        }
    }

    /// 获取错误的简短描述（用于日志或测试断言）
    pub fn message(&self) -> String {
        match self {
            SemanticError::UndefinedName { name, .. } => {
                format!("undefined name: {}", name)
            }
            SemanticError::Redeclaration { name, .. } => {
                format!("redeclaration of: {}", name)
            }
            SemanticError::TypeMismatch { expected, found, .. } => {
                format!("type mismatch: expected {}, found {}", expected, found)
            }
            SemanticError::InvalidBinaryOp {
                op, left_ty, right_ty, ..
            } => {
                format!(
                    "invalid binary operator {} for types {} and {}",
                    op, left_ty, right_ty
                )
            }
            SemanticError::InvalidUnaryOp { op, operand_ty, .. } => {
                format!("invalid unary operator {} for type {}", op, operand_ty)
            }
            SemanticError::InvalidAssignment { .. } => {
                "invalid assignment target".to_string()
            }
            SemanticError::AssignToConstant { name, .. } => {
                format!("cannot assign to constant: {}", name)
            }
            SemanticError::BreakOutsideLoop { .. } => {
                "break outside of loop".to_string()
            }
            SemanticError::ReturnTypeMismatch { expected, found, .. } => {
                format!(
                    "return type mismatch: expected {}, found {}",
                    expected, found
                )
            }
            SemanticError::CyclicDependency { cycle, .. } => {
                format!("cyclic dependency: {}", cycle.join(" -> "))
            }
            SemanticError::UnresolvedType { name, .. } => {
                format!("unresolved type: {}", name)
            }
            SemanticError::ImplicitCastFailed { from, to, .. } => {
                format!("cannot implicitly cast from {} to {}", from, to)
            }
            SemanticError::General { message, .. } => message.clone(),
        }
    }
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for SemanticError {}
