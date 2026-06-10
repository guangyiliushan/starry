use starry_ast::{AstNode, BinaryOp, UnaryOp};

use super::action::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AstNodeKind {
    Root,
    Block,
    ExprStmt,
    Binary,
    Unary,
    Literal,
    Ident,
    Paren,
}

impl AstNodeKind {
    pub fn from_node(node: &AstNode) -> Self {
        match node {
            AstNode::Root(_) => AstNodeKind::Root,
            AstNode::Block(_) => AstNodeKind::Block,
            AstNode::ExprStmt(_) => AstNodeKind::ExprStmt,
            AstNode::Binary(_) => AstNodeKind::Binary,
            AstNode::Unary(_) => AstNodeKind::Unary,
            AstNode::Literal(_) => AstNodeKind::Literal,
            AstNode::Ident(_) => AstNodeKind::Ident,
            AstNode::Paren(_) => AstNodeKind::Paren,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodePattern {
    Exact(AstNodeKind),
    BinaryOp {
        ops: Vec<BinaryOp>,
    },
    UnaryOp {
        ops: Vec<UnaryOp>,
    },
    AnyLiteral,
    AnyIdent,
    AnyBlock,
}

impl NodePattern {
    pub fn matches(&self, node: &AstNode) -> bool {
        match self {
            NodePattern::Exact(kind) => AstNodeKind::from_node(node) == *kind,
            NodePattern::BinaryOp { ops } => {
                if let AstNode::Binary(expr) = node {
                    ops.contains(&expr.op)
                } else {
                    false
                }
            }
            NodePattern::UnaryOp { ops } => {
                if let AstNode::Unary(expr) = node {
                    ops.contains(&expr.op)
                } else {
                    false
                }
            }
            NodePattern::AnyLiteral => matches!(node, AstNode::Literal(_)),
            NodePattern::AnyIdent => matches!(node, AstNode::Ident(_)),
            NodePattern::AnyBlock => matches!(node, AstNode::Block(_)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SemanticRule {
    pub name: String,
    pub pattern: NodePattern,
    pub pre_actions: Vec<Action>,
    pub post_actions: Vec<Action>,
}

impl SemanticRule {
    pub fn new(name: impl Into<String>, pattern: NodePattern) -> Self {
        Self {
            name: name.into(),
            pattern,
            pre_actions: Vec::new(),
            post_actions: Vec::new(),
        }
    }

    pub fn with_pre_actions(mut self, actions: Vec<Action>) -> Self {
        self.pre_actions = actions;
        self
    }

    pub fn with_post_actions(mut self, actions: Vec<Action>) -> Self {
        self.post_actions = actions;
        self
    }

    pub fn matches(&self, node: &AstNode) -> bool {
        self.pattern.matches(node)
    }
}
