use crate::rules::rule::{AstNodeKind, NodePattern};

pub fn pattern_to_string(pattern: &NodePattern) -> String {
    match pattern {
        NodePattern::Exact(kind) => {
            match kind {
                AstNodeKind::Root => "Root".to_string(),
                AstNodeKind::Block => "Block".to_string(),
                AstNodeKind::ExprStmt => "ExprStmt".to_string(),
                AstNodeKind::Binary => "Binary".to_string(),
                AstNodeKind::Unary => "Unary".to_string(),
                AstNodeKind::Literal => "Literal".to_string(),
                AstNodeKind::Ident => "Ident".to_string(),
                AstNodeKind::Paren => "Paren".to_string(),
            }
        }
        NodePattern::BinaryOp { ops } => {
            let op_names: Vec<String> = ops.iter().map(|op| format!("{:?}", op)).collect();
            format!("Binary({})", op_names.join(" | "))
        }
        NodePattern::UnaryOp { ops } => {
            let op_names: Vec<String> = ops.iter().map(|op| format!("{:?}", op)).collect();
            format!("Unary({})", op_names.join(" | "))
        }
        NodePattern::AnyLiteral => "AnyLiteral".to_string(),
        NodePattern::AnyIdent => "AnyIdent".to_string(),
        NodePattern::AnyBlock => "AnyBlock".to_string(),
    }
}

impl std::fmt::Display for NodePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", pattern_to_string(self))
    }
}