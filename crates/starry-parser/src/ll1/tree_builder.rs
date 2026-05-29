use crate::cfg::{ContextFreeGrammar, NonTerminalId, Symbol};
use crate::ll1::error::LL1ParseError;
use crate::parser::ParseTreeNode;
use starry_ast::Token;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum TreeNode {
    NonTerminal {
        name: String,
        children: Vec<TreeNode>,
    },
    Terminal {
        token: Token,
    },
    Epsilon,
}

pub struct ParseTreeBuilder {
    node_stack: VecDeque<TreeNode>,
}

impl ParseTreeBuilder {
    pub fn new() -> Self {
        ParseTreeBuilder {
            node_stack: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.node_stack.clear();
    }

    pub fn push_terminal(&mut self, token: Token) {
        self.node_stack.push_front(TreeNode::Terminal { token });
    }

    pub fn push_epsilon(&mut self) {
        self.node_stack.push_front(TreeNode::Epsilon);
    }

    pub fn build_non_terminal(&mut self, name: String, child_count: usize) {
        let mut children = Vec::with_capacity(child_count);
        
        for _ in 0..child_count {
            if let Some(node) = self.node_stack.pop_front() {
                children.push(node);
            }
        }
        
        children.reverse();
        
        self.node_stack.push_front(TreeNode::NonTerminal { name, children });
    }

    pub fn build_from_production(
        &mut self,
        cfg: &ContextFreeGrammar,
        lhs: NonTerminalId,
        rhs: &[Symbol],
    ) {
        let name = cfg.get_non_terminal_name(lhs).to_string();
        let child_count = rhs.len();
        
        self.build_non_terminal(name, child_count);
    }

    pub fn get_result(&self) -> Option<&TreeNode> {
        self.node_stack.front()
    }

    pub fn take_result(&mut self) -> Option<TreeNode> {
        self.node_stack.pop_front()
    }

    pub fn to_parse_tree_node(&self) -> Result<ParseTreeNode, LL1ParseError> {
        match self.node_stack.front() {
            Some(node) => Ok(convert_to_parse_tree_node(node)),
            None => Ok(ParseTreeNode::epsilon()),
        }
    }
}

impl Default for ParseTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_to_parse_tree_node(node: &TreeNode) -> ParseTreeNode {
    match node {
        TreeNode::NonTerminal { name, children } => {
            ParseTreeNode::non_terminal(
                name.clone(),
                children.iter().map(convert_to_parse_tree_node).collect(),
            )
        }
        TreeNode::Terminal { token } => ParseTreeNode::terminal(token.clone()),
        TreeNode::Epsilon => ParseTreeNode::epsilon(),
    }
}

pub struct DerivationStep {
    pub production: String,
    pub input_position: usize,
}

pub struct DerivationRecorder {
    steps: Vec<DerivationStep>,
}

impl DerivationRecorder {
    pub fn new() -> Self {
        DerivationRecorder {
            steps: Vec::new(),
        }
    }

    pub fn record(&mut self, production: String, input_position: usize) {
        self.steps.push(DerivationStep {
            production,
            input_position,
        });
    }

    pub fn steps(&self) -> &[DerivationStep] {
        &self.steps
    }

    pub fn clear(&mut self) {
        self.steps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }
}

impl Default for DerivationRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_builder_basic() {
        let mut builder = ParseTreeBuilder::new();
        
        builder.push_epsilon();
        
        let result = builder.to_parse_tree_node();
        assert!(result.is_ok());
    }
}
