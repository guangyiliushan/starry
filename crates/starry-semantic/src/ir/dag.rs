use std::collections::HashMap;

use super::basic_block::BasicBlock;
use super::operand::Operand;
use super::quadruple::TacOp;
use super::tac::TacInstr;

/// DAG 节点（用于基本块内公共子表达式消除）
///
/// 对应龙书 §8.5 中 DAG 的定义：基本块内的每条指令对应一个 DAG 节点，
/// 相同运算的节点只创建一次（公共子表达式合并），变量名附加到节点上。
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: usize,
    pub op: TacOp,
    pub children: Vec<usize>,
    pub identifiers: Vec<String>,
}

impl DagNode {
    pub fn new(id: usize, op: TacOp) -> Self {
        Self {
            id,
            op,
            children: Vec::new(),
            identifiers: Vec::new(),
        }
    }
}

/// 基本块 DAG
///
/// 从基本块的 TAC 指令序列构建，用于发现公共子表达式，
/// 重建优化后的 TAC 指令序列。
#[derive(Debug, Clone)]
pub struct Dag {
    nodes: Vec<DagNode>,
}

impl Dag {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// 从基本块构建 DAG
    pub fn build_from_block(block: &BasicBlock) -> Self {
        let mut dag = Self::new();
        let mut name_to_node: HashMap<String, usize> = HashMap::new();

        for instr in &block.instructions {
            dag.process_instruction(instr, &mut name_to_node);
        }

        dag
    }

    fn process_instruction(
        &mut self,
        instr: &TacInstr,
        name_to_node: &mut HashMap<String, usize>,
    ) {
        match instr {
            TacInstr::Binary {
                op,
                result,
                left,
                right,
            } => {
                let left_id = self.resolve_or_create_leaf(left, name_to_node);
                let right_id = self.resolve_or_create_leaf(right, name_to_node);

                if let (Some(lid), Some(rid)) = (left_id, right_id) {
                    let tac_op = Self::binary_op_to_tac(op);
                    if let Some(existing) = self.find_node(tac_op, &[lid, rid]) {
                        self.attach_result(existing, result, name_to_node);
                    } else {
                        let node_id = self.add_node(tac_op, &[lid, rid]);
                        self.attach_result(node_id, result, name_to_node);
                    }
                }
            }
            TacInstr::Assign { target, source } => {
                let source_id = self.resolve_or_create_leaf(source, name_to_node);
                if let Some(sid) = source_id {
                    self.attach_target(sid, target, name_to_node);
                }
            }
            TacInstr::Unary {
                op,
                result,
                operand,
            } => {
                let operand_id = self.resolve_or_create_leaf(operand, name_to_node);
                if let Some(oid) = operand_id {
                    let tac_op = Self::unary_op_to_tac(op);
                    if let Some(existing) = self.find_node(tac_op, &[oid]) {
                        self.attach_result(existing, result, name_to_node);
                    } else {
                        let node_id = self.add_node(tac_op, &[oid]);
                        self.attach_result(node_id, result, name_to_node);
                    }
                }
            }
            _ => {}
        }
    }

    fn resolve_or_create_leaf(
        &mut self,
        operand: &Operand,
        name_to_node: &HashMap<String, usize>,
    ) -> Option<usize> {
        match operand {
            Operand::Name(name) => name_to_node.get(name).copied(),
            Operand::Temp(tid) => {
                let temp_name = super::temp::TempManager::temp_name(*tid);
                name_to_node.get(&temp_name).copied()
            }
            Operand::IntConst(_)
            | Operand::FloatConst(_)
            | Operand::BoolConst(_)
            | Operand::StrConst(_) => {
                let node_id = self.add_leaf();
                Some(node_id)
            }
            Operand::Label(_) => None,
        }
    }

    fn attach_result(
        &mut self,
        node_id: usize,
        result: &Operand,
        name_to_node: &mut HashMap<String, usize>,
    ) {
        match result {
            Operand::Temp(tid) => {
                let temp_name = super::temp::TempManager::temp_name(*tid);
                self.nodes[node_id].identifiers.push(temp_name.clone());
                name_to_node.insert(temp_name, node_id);
            }
            Operand::Name(name) => {
                self.nodes[node_id].identifiers.push(name.clone());
                name_to_node.insert(name.clone(), node_id);
            }
            _ => {}
        }
    }

    fn attach_target(
        &mut self,
        node_id: usize,
        target: &Operand,
        name_to_node: &mut HashMap<String, usize>,
    ) {
        match target {
            Operand::Name(name) => {
                self.nodes[node_id].identifiers.push(name.clone());
                name_to_node.insert(name.clone(), node_id);
            }
            Operand::Temp(tid) => {
                let temp_name = super::temp::TempManager::temp_name(*tid);
                self.nodes[node_id].identifiers.push(temp_name.clone());
                name_to_node.insert(temp_name, node_id);
            }
            _ => {}
        }
    }

    fn add_leaf(&mut self) -> usize {
        let id = self.nodes.len();
        self.nodes.push(DagNode::new(id, TacOp::Nop));
        id
    }

    fn add_node(&mut self, op: TacOp, children: &[usize]) -> usize {
        let id = self.nodes.len();
        let mut node = DagNode::new(id, op);
        node.children = children.to_vec();
        self.nodes.push(node);
        id
    }

    fn find_node(&self, op: TacOp, children: &[usize]) -> Option<usize> {
        for node in &self.nodes {
            if node.op == op && node.children.len() == children.len() {
                let matches = node
                    .children
                    .iter()
                    .zip(children.iter())
                    .all(|(a, b)| a == b);
                if matches {
                    return Some(node.id);
                }
            }
        }
        None
    }

    fn binary_op_to_tac(op: &starry_ast::BinaryOp) -> TacOp {
        use starry_ast::BinaryOp::*;
        match op {
            Add => TacOp::Add,
            Sub => TacOp::Sub,
            Mul => TacOp::Mul,
            Div => TacOp::Div,
            Mod => TacOp::Mod,
            Eq => TacOp::Eq,
            Ne => TacOp::Ne,
            Lt => TacOp::Lt,
            Le => TacOp::Le,
            Gt => TacOp::Gt,
            Ge => TacOp::Ge,
            And => TacOp::And,
            Or => TacOp::Or,
            Custom(_) => TacOp::Nop,
        }
    }

    fn unary_op_to_tac(op: &starry_ast::UnaryOp) -> TacOp {
        use starry_ast::UnaryOp::*;
        match op {
            Neg => TacOp::Neg,
            Not => TacOp::Not,
            Pos => TacOp::Pos,
            Custom(_) => TacOp::Nop,
        }
    }

    pub fn nodes(&self) -> &[DagNode] {
        &self.nodes
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}
