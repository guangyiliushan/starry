use starry_ast::{BinaryOp, UnaryOp};

use super::operand::{LabelId, Operand};

/// 三地址码指令
///
/// 对应龙书 §8.3 中三地址指令的定义：每条指令最多包含两个操作数和一个结果。
/// 三地址码是 AST 到目标代码之间的关键中间表示，便于进行优化和代码生成。
#[derive(Debug, Clone, PartialEq)]
pub enum TacInstr {
    /// 二元运算: result = left op right
    Binary {
        op: BinaryOp,
        result: Operand,
        left: Operand,
        right: Operand,
    },
    /// 一元运算: result = op operand
    Unary {
        op: UnaryOp,
        result: Operand,
        operand: Operand,
    },
    /// 赋值: target = source
    Assign {
        target: Operand,
        source: Operand,
    },
    /// 条件跳转: if condition goto label
    CondJump {
        condition: Operand,
        label: LabelId,
    },
    /// 无条件跳转: goto label
    Jump {
        label: LabelId,
    },
    /// 标签定义
    Label {
        id: LabelId,
    },
    /// 函数调用: result = call func(args)
    Call {
        result: Option<Operand>,
        func: Operand,
        args: Vec<Operand>,
    },
    /// 返回: return value
    Return {
        value: Option<Operand>,
    },
    /// 空操作（占位）
    Nop,
}

impl std::fmt::Display for TacInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TacInstr::Binary {
                op,
                result,
                left,
                right,
            } => write!(f, "{} = {} {} {}", result, left, op, right),
            TacInstr::Unary {
                op,
                result,
                operand,
            } => write!(f, "{} = {}{}", result, op, operand),
            TacInstr::Assign { target, source } => write!(f, "{} = {}", target, source),
            TacInstr::CondJump { condition, label } => {
                write!(f, "if {} goto L{}", condition, label)
            }
            TacInstr::Jump { label } => write!(f, "goto L{}", label),
            TacInstr::Label { id } => write!(f, "L{}:", id),
            TacInstr::Call {
                result,
                func,
                args,
            } => {
                if let Some(r) = result {
                    write!(f, "{} = call {}(", r, func)?;
                } else {
                    write!(f, "call {}(", func)?;
                }
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            TacInstr::Return { value } => {
                if let Some(v) = value {
                    write!(f, "return {}", v)
                } else {
                    write!(f, "return")
                }
            }
            TacInstr::Nop => write!(f, "nop"),
        }
    }
}
