use super::operand::LabelId;
use super::tac::TacInstr;

/// 基本块：连续的 TAC 指令序列，只有一个入口和一个出口
///
/// 对应龙书 §8.4 中基本块的定义：
/// - 第一条指令是唯一入口（或紧跟跳转目标的标签）
/// - 最后一条指令是唯一出口（跳转或条件跳转）
/// - 中间没有跳转进入或离开的指令
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: Option<LabelId>,
    pub instructions: Vec<TacInstr>,
}

impl BasicBlock {
    pub fn new(label: Option<LabelId>) -> Self {
        Self {
            label,
            instructions: Vec::new(),
        }
    }

    /// 将线性 TAC 指令序列切分为基本块列表
    ///
    /// 切分规则（龙书算法 8.4）：
    /// 1. 第一条指令是一个基本块的首指令
    /// 2. 跳转目标（标签）是一个基本块的首指令
    /// 3. 紧跟在跳转指令后的指令是一个基本块的首指令
    pub fn split(instructions: &[TacInstr]) -> Vec<Self> {
        if instructions.is_empty() {
            return Vec::new();
        }

        let mut leaders = vec![false; instructions.len()];
        leaders[0] = true;

        let mut label_positions = std::collections::HashMap::new();
        for (i, instr) in instructions.iter().enumerate() {
            if let TacInstr::Label { id } = instr {
                label_positions.insert(*id, i);
            }
        }

        for (i, instr) in instructions.iter().enumerate() {
            match instr {
                TacInstr::Jump { label } | TacInstr::CondJump { label, .. } => {
                    if let Some(&target) = label_positions.get(label) {
                        leaders[target] = true;
                    }
                    if i + 1 < instructions.len() {
                        leaders[i + 1] = true;
                    }
                }
                TacInstr::Return { .. } => {
                    if i + 1 < instructions.len() {
                        leaders[i + 1] = true;
                    }
                }
                _ => {}
            }
        }

        let mut blocks = Vec::new();
        let mut current_start = 0;

        for i in 1..instructions.len() {
            if leaders[i] {
                let block_instrs = instructions[current_start..i].to_vec();
                let label = extract_label(&block_instrs);
                blocks.push(BasicBlock {
                    label,
                    instructions: block_instrs,
                });
                current_start = i;
            }
        }

        if current_start < instructions.len() {
            let block_instrs = instructions[current_start..].to_vec();
            let label = extract_label(&block_instrs);
            blocks.push(BasicBlock {
                label,
                instructions: block_instrs,
            });
        }

        blocks
    }

    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.instructions.len()
    }
}

fn extract_label(instrs: &[TacInstr]) -> Option<LabelId> {
    if let Some(TacInstr::Label { id }) = instrs.first() {
        Some(*id)
    } else {
        None
    }
}
