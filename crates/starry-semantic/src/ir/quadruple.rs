use starry_ast::{BinaryOp, UnaryOp};

use super::operand::Operand;
use super::tac::TacInstr;

/// 四元式运算符
///
/// 从 TacInstr 中提取的统一运算符表示，四元式使用此枚举
/// 而非直接使用 AST 的 BinaryOp/UnaryOp。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TacOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Neg,
    Pos,
    Assign,
    Jmp,
    Jz,
    Jnz,
    Call,
    Ret,
    Label,
    Index,
    Nop,
}

impl std::fmt::Display for TacOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TacOp::Add => write!(f, "+"),
            TacOp::Sub => write!(f, "-"),
            TacOp::Mul => write!(f, "*"),
            TacOp::Div => write!(f, "/"),
            TacOp::Mod => write!(f, "%"),
            TacOp::Eq => write!(f, "=="),
            TacOp::Ne => write!(f, "!="),
            TacOp::Lt => write!(f, "<"),
            TacOp::Le => write!(f, "<="),
            TacOp::Gt => write!(f, ">"),
            TacOp::Ge => write!(f, ">="),
            TacOp::And => write!(f, "&&"),
            TacOp::Or => write!(f, "||"),
            TacOp::Not => write!(f, "!"),
            TacOp::Neg => write!(f, "neg"),
            TacOp::Pos => write!(f, "pos"),
            TacOp::Assign => write!(f, "="),
            TacOp::Jmp => write!(f, "jmp"),
            TacOp::Jz => write!(f, "jz"),
            TacOp::Jnz => write!(f, "jnz"),
            TacOp::Call => write!(f, "call"),
            TacOp::Ret => write!(f, "ret"),
            TacOp::Label => write!(f, "label"),
            TacOp::Index => write!(f, "[]"),
            TacOp::Nop => write!(f, "nop"),
        }
    }
}

/// 四元式: (op, arg1, arg2, result)
///
/// 对应编译原理教材中四元式的标准定义。
/// 与三地址码相比，四元式将运算符统一为 TacOp 枚举，
/// 便于后续的优化和代码生成阶段统一处理。
#[derive(Debug, Clone, PartialEq)]
pub struct Quadruple {
    pub op: TacOp,
    pub arg1: Option<Operand>,
    pub arg2: Option<Operand>,
    pub result: Option<Operand>,
}

impl Quadruple {
    pub fn new(
        op: TacOp,
        arg1: Option<Operand>,
        arg2: Option<Operand>,
        result: Option<Operand>,
    ) -> Self {
        Self {
            op,
            arg1,
            arg2,
            result,
        }
    }
}

impl std::fmt::Display for Quadruple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a1 = self
            .arg1
            .as_ref()
            .map(|o| o.to_string())
            .unwrap_or_else(|| "—".to_string());
        let a2 = self
            .arg2
            .as_ref()
            .map(|o| o.to_string())
            .unwrap_or_else(|| "—".to_string());
        let r = self
            .result
            .as_ref()
            .map(|o| o.to_string())
            .unwrap_or_else(|| "—".to_string());
        write!(f, "({}, {}, {}, {})", self.op, a1, a2, r)
    }
}

fn binary_op_to_tac(op: &BinaryOp) -> TacOp {
    match op {
        BinaryOp::Add => TacOp::Add,
        BinaryOp::Sub => TacOp::Sub,
        BinaryOp::Mul => TacOp::Mul,
        BinaryOp::Div => TacOp::Div,
        BinaryOp::Mod => TacOp::Mod,
        BinaryOp::Eq => TacOp::Eq,
        BinaryOp::Ne => TacOp::Ne,
        BinaryOp::Lt => TacOp::Lt,
        BinaryOp::Le => TacOp::Le,
        BinaryOp::Gt => TacOp::Gt,
        BinaryOp::Ge => TacOp::Ge,
        BinaryOp::And => TacOp::And,
        BinaryOp::Or => TacOp::Or,
        BinaryOp::Custom(_) => TacOp::Nop,
    }
}

fn unary_op_to_tac(op: &UnaryOp) -> TacOp {
    match op {
        UnaryOp::Neg => TacOp::Neg,
        UnaryOp::Not => TacOp::Not,
        UnaryOp::Pos => TacOp::Pos,
        UnaryOp::Custom(_) => TacOp::Nop,
    }
}

impl From<TacInstr> for Quadruple {
    fn from(instr: TacInstr) -> Self {
        match instr {
            TacInstr::Binary {
                op,
                result,
                left,
                right,
            } => Quadruple::new(
                binary_op_to_tac(&op),
                Some(left),
                Some(right),
                Some(result),
            ),
            TacInstr::Unary {
                op,
                result,
                operand,
            } => Quadruple::new(unary_op_to_tac(&op), Some(operand), None, Some(result)),
            TacInstr::Assign { target, source } => {
                Quadruple::new(TacOp::Assign, Some(source), None, Some(target))
            }
            TacInstr::CondJump { condition, label } => Quadruple::new(
                TacOp::Jnz,
                Some(condition),
                Some(Operand::Label(label)),
                None,
            ),
            TacInstr::Jump { label } => {
                Quadruple::new(TacOp::Jmp, Some(Operand::Label(label)), None, None)
            }
            TacInstr::Label { id } => {
                Quadruple::new(TacOp::Label, Some(Operand::Label(id)), None, None)
            }
            TacInstr::Call {
                result,
                func,
                args,
            } => {
                let func_operand = Some(func);
                let args_operand = if args.is_empty() {
                    None
                } else {
                    Some(Operand::Name(format!(
                        "({})",
                        args.iter()
                            .map(|a| a.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )))
                };
                Quadruple::new(TacOp::Call, func_operand, args_operand, result)
            }
            TacInstr::Return { value } => {
                Quadruple::new(TacOp::Ret, value, None, None)
            }
            TacInstr::Nop => Quadruple::new(TacOp::Nop, None, None, None),
        }
    }
}

/// 公共子表达式消除（CSE）：扫描四元式序列，消除语义等价的重复计算。
///
/// 策略：按序扫描，将每一条四元式的 (op, arg1, arg2) 作为签名。
/// 若签名已出现过，则跳过此条并将结果映射到首次出现的结果；
/// 否则保留此条并记录签名。
///
/// 例如 `(+, 1, 2, t0)` 和 `(+, 1, 2, t1)` → 仅保留前者，t1 → t0。
pub fn cse_quadruples(quads: &[Quadruple]) -> Vec<Quadruple> {
    use std::collections::HashMap;

    let mut seen: HashMap<(TacOp, Option<Operand>, Option<Operand>), usize> = HashMap::new();
    let mut result_remap: HashMap<Operand, Operand> = HashMap::new();
    let mut new_quads: Vec<Quadruple> = Vec::new();

    for quad in quads {
        let key = (quad.op, quad.arg1.clone(), quad.arg2.clone());

        if let Some(&first_idx) = seen.get(&key) {
            // 重复：将当前 result 映射到首次出现的 result
            if let Some(result) = &quad.result {
                if let Some(first_result) = &new_quads[first_idx].result {
                    result_remap.insert(result.clone(), first_result.clone());
                }
            }
            continue;
        }

        // 映射参数中的引用
        let arg1 = quad.arg1.as_ref().map(|a| {
            result_remap.get(a).cloned().unwrap_or_else(|| a.clone())
        });
        let arg2 = quad.arg2.as_ref().map(|a| {
            result_remap.get(a).cloned().unwrap_or_else(|| a.clone())
        });

        let idx = new_quads.len();
        new_quads.push(Quadruple::new(quad.op, arg1, arg2, quad.result.clone()));
        seen.insert(key, idx);
    }

    new_quads
}

/// CSE + 转三元式：先对四元式执行 CSE，再将结果转为三元式。
///
/// 产物是优化后的三元式序列，等价子表达式仅保留首次出现。
pub fn cse_and_to_triples(quads: &[Quadruple]) -> Vec<crate::ir::triple::Triple> {
    let cse_quads = cse_quadruples(quads);
    crate::ir::triple::quadruples_to_triples(&cse_quads)
}
