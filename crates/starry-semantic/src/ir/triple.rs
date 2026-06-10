use super::operand::Operand;
use super::quadruple::{Quadruple, TacOp};

/// 三元式引用
///
/// 三元式中不使用临时变量名引用结果，而是通过指令位置（索引）引用。
/// 这节省了 result 字段的空间，但使得指令移动变得困难（索引会失效）。
#[derive(Debug, Clone, PartialEq)]
pub enum TripleRef {
    Operand(Operand),
    Index(usize),
}

impl std::fmt::Display for TripleRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TripleRef::Operand(op) => write!(f, "{}", op),
            TripleRef::Index(idx) => write!(f, "({})", idx),
        }
    }
}

/// 三元式: (op, arg1, arg2)
///
/// 对应编译原理教材中三元式的标准定义。
/// 与四元式不同，三元式不存储 result 字段，
/// 而是通过指令在序列中的位置（索引）来引用其结果。
#[derive(Debug, Clone, PartialEq)]
pub struct Triple {
    pub op: TacOp,
    pub arg1: Option<TripleRef>,
    pub arg2: Option<TripleRef>,
}

impl Triple {
    pub fn new(
        op: TacOp,
        arg1: Option<TripleRef>,
        arg2: Option<TripleRef>,
    ) -> Self {
        Self { op, arg1, arg2 }
    }
}

impl std::fmt::Display for Triple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a1 = self
            .arg1
            .as_ref()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "—".to_string());
        let a2 = self
            .arg2
            .as_ref()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "—".to_string());
        write!(f, "({}, {}, {})", self.op, a1, a2)
    }
}

/// 将四元式序列转换为三元式序列
///
/// 四元式中的 result 字段如果是临时变量（Operand::Temp），
/// 则在三元式中被替换为对应指令的索引引用（TripleRef::Index）。
pub fn quadruples_to_triples(quadruples: &[Quadruple]) -> Vec<Triple> {
    let mut temp_to_index = std::collections::HashMap::new();

    for (idx, quad) in quadruples.iter().enumerate() {
        if let Some(Operand::Temp(temp_id)) = &quad.result {
            temp_to_index.insert(*temp_id, idx);
        }
    }

    quadruples
        .iter()
        .map(|quad| {
            let arg1 = quad.arg1.as_ref().map(|op| convert_operand(op, &temp_to_index));
            let arg2 = quad.arg2.as_ref().map(|op| convert_operand(op, &temp_to_index));
            Triple::new(quad.op, arg1, arg2)
        })
        .collect()
}

fn convert_operand(
    operand: &Operand,
    temp_to_index: &std::collections::HashMap<usize, usize>,
) -> TripleRef {
    match operand {
        Operand::Temp(temp_id) => {
            if let Some(&idx) = temp_to_index.get(temp_id) {
                TripleRef::Index(idx)
            } else {
                TripleRef::Operand(operand.clone())
            }
        }
        _ => TripleRef::Operand(operand.clone()),
    }
}
