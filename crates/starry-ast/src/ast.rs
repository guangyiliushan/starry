use crate::{Span, Token, TokenKind};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Root(Root),
    ExprStmt(ExprStmt),
    Block(Block),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Literal(Literal),
    Ident(Ident),
    Paren(ParenExpr),
}

impl AstNode {
    pub fn span(&self) -> Span {
        match self {
            AstNode::Root(n) => n.span,
            AstNode::ExprStmt(n) => n.span,
            AstNode::Block(n) => n.span,
            AstNode::Binary(n) => n.span,
            AstNode::Unary(n) => n.span,
            AstNode::Literal(n) => n.span,
            AstNode::Ident(n) => n.span,
            AstNode::Paren(n) => n.span,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            AstNode::Root(_) => "Root",
            AstNode::ExprStmt(_) => "ExprStmt",
            AstNode::Block(_) => "Block",
            AstNode::Binary(_) => "BinaryExpr",
            AstNode::Unary(_) => "UnaryExpr",
            AstNode::Literal(_) => "Literal",
            AstNode::Ident(_) => "Ident",
            AstNode::Paren(_) => "Paren",
        }
    }

    pub fn children(&self) -> Vec<&AstNode> {
        match self {
            AstNode::Root(n) => n.items.iter().collect(),
            AstNode::ExprStmt(n) => vec![n.expr.as_ref()],
            AstNode::Block(n) => n.stmts.iter().collect(),
            AstNode::Binary(n) => vec![n.left.as_ref(), n.right.as_ref()],
            AstNode::Unary(n) => vec![n.expr.as_ref()],
            AstNode::Paren(n) => vec![n.expr.as_ref()],
            AstNode::Literal(_) | AstNode::Ident(_) => vec![],
        }
    }

    pub fn summary(&self) -> String {
        match self {
            AstNode::Root(_) => "Root".to_string(),
            AstNode::ExprStmt(_) => "ExprStmt".to_string(),
            AstNode::Block(n) => format!("Block [{} stmts]", n.stmts.len()),
            AstNode::Binary(n) => format!("BinaryExpr {}", n.op),
            AstNode::Unary(n) => format!("UnaryExpr {}", n.op),
            AstNode::Literal(n) => format!("Literal {}", n.value),
            AstNode::Ident(n) => format!("Ident {}", n.name),
            AstNode::Paren(_) => "ParenExpr".to_string(),
        }
    }

    pub fn print(&self) {
        println!("{}", self.kind_name());
        print_tree_children(&self.children(), "");
    }
}

fn print_tree_children(children: &[&AstNode], prefix: &str) {
    let len = children.len();
    for (i, child) in children.iter().enumerate() {
        let is_last = i == len - 1;
        let branch = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        println!("{}{}{}", prefix, branch, child.summary());

        let grand_children = child.children();
        if !grand_children.is_empty() {
            let new_prefix = format!("{}{}", prefix, child_prefix);
            print_tree_children(&grand_children, &new_prefix);
        }
    }
}

impl fmt::Display for AstNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AstNode::Root(n) => write!(f, "{}", n),
            AstNode::ExprStmt(n) => write!(f, "{}", n),
            AstNode::Block(n) => write!(f, "{}", n),
            AstNode::Binary(n) => write!(f, "{}", n),
            AstNode::Unary(n) => write!(f, "{}", n),
            AstNode::Literal(n) => write!(f, "{}", n),
            AstNode::Ident(n) => write!(f, "{}", n),
            AstNode::Paren(n) => write!(f, "{}", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub span: Span,
    pub items: Vec<AstNode>,
}

impl fmt::Display for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in &self.items {
            writeln!(f, "{}", item)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub span: Span,
    pub expr: Box<AstNode>,
}

impl fmt::Display for ExprStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{};", self.expr)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub span: Span,
    pub stmts: Vec<AstNode>,
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{{")?;
        for stmt in &self.stmts {
            writeln!(f, "  {}", stmt)?;
        }
        write!(f, "}}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub span: Span,
    pub left: Box<AstNode>,
    pub op: BinaryOp,
    pub right: Box<AstNode>,
}

impl fmt::Display for BinaryExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} {} {})", self.left, self.op, self.right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
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
    Custom(&'static str),
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Mod => write!(f, "%"),
            BinaryOp::Eq => write!(f, "=="),
            BinaryOp::Ne => write!(f, "!="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Le => write!(f, "<="),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Ge => write!(f, ">="),
            BinaryOp::And => write!(f, "&&"),
            BinaryOp::Or => write!(f, "||"),
            BinaryOp::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl BinaryOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "+" => Some(BinaryOp::Add),
            "-" => Some(BinaryOp::Sub),
            "*" => Some(BinaryOp::Mul),
            "/" => Some(BinaryOp::Div),
            "%" => Some(BinaryOp::Mod),
            "==" => Some(BinaryOp::Eq),
            "!=" => Some(BinaryOp::Ne),
            "<" => Some(BinaryOp::Lt),
            "<=" => Some(BinaryOp::Le),
            ">" => Some(BinaryOp::Gt),
            ">=" => Some(BinaryOp::Ge),
            "&&" => Some(BinaryOp::And),
            "||" => Some(BinaryOp::Or),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub span: Span,
    pub op: UnaryOp,
    pub expr: Box<AstNode>,
}

impl fmt::Display for UnaryExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}{})", self.op, self.expr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
    Pos,
    Custom(&'static str),
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
            UnaryOp::Pos => write!(f, "+"),
            UnaryOp::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl UnaryOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "-" => Some(UnaryOp::Neg),
            "!" => Some(UnaryOp::Not),
            "+" => Some(UnaryOp::Pos),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Literal {
    pub span: Span,
    pub value: LiteralValue,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Integer(v) => write!(f, "{}", v),
            LiteralValue::Float(v) => write!(f, "{}", v),
            LiteralValue::String(v) => write!(f, "\"{}\"", v),
            LiteralValue::Bool(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub span: Span,
    pub name: String,
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParenExpr {
    pub span: Span,
    pub expr: Box<AstNode>,
}

impl fmt::Display for ParenExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.expr)
    }
}

pub trait AstBuilder {
    fn make_leaf(&self, token: &Token) -> AstNode;

    fn reduce(&self, prod_id: usize, children: Vec<AstNode>) -> AstNode;
}

pub struct DefaultAstBuilder;

impl AstBuilder for DefaultAstBuilder {
    fn make_leaf(&self, token: &Token) -> AstNode {
        let span = Span::from_token(token);
        match &token.kind {
            TokenKind::Integer(v) => AstNode::Literal(Literal {
                span,
                value: LiteralValue::Integer(*v),
            }),
            TokenKind::Float(v) => AstNode::Literal(Literal {
                span,
                value: LiteralValue::Float(*v),
            }),
            TokenKind::StringLiteral(s) => AstNode::Literal(Literal {
                span,
                value: LiteralValue::String(s.clone()),
            }),
            TokenKind::Identifier(name) => AstNode::Ident(Ident {
                span,
                name: name.clone(),
            }),
            TokenKind::Keyword(kw) => AstNode::Ident(Ident {
                span,
                name: kw.clone(),
            }),
            _ => AstNode::Ident(Ident {
                span,
                name: token.lexeme.clone(),
            }),
        }
    }

    fn reduce(&self, prod_id: usize, children: Vec<AstNode>) -> AstNode {
        let span = Span::merge_opt(&children.iter().map(|c| c.span()).collect::<Vec<_>>());

        match prod_id {
            0 => AstNode::Root(Root {
                span,
                items: children.into_iter().filter(|c| !matches!(c, AstNode::Ident(_))).collect(),
            }),
            _ => {
                if let Some(first) = children.first() {
                    first.clone()
                } else {
                    AstNode::Ident(Ident {
                        span: Span::default(),
                        name: format!("<prod_{}>", prod_id),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_node() {
        let lit = Literal {
            span: Span::default(),
            value: LiteralValue::Integer(42),
        };
        assert_eq!(format!("{}", lit), "42");
    }

    #[test]
    fn test_binary_expr() {
        let left = AstNode::Literal(Literal {
            span: Span::default(),
            value: LiteralValue::Integer(1),
        });
        let right = AstNode::Literal(Literal {
            span: Span::default(),
            value: LiteralValue::Integer(2),
        });
        let bin = BinaryExpr {
            span: Span::default(),
            left: Box::new(left),
            op: BinaryOp::Add,
            right: Box::new(right),
        };
        assert_eq!(format!("{}", bin), "(1 + 2)");
    }

    #[test]
    fn test_binary_op_from_str() {
        assert_eq!(BinaryOp::from_str("+"), Some(BinaryOp::Add));
        assert_eq!(BinaryOp::from_str("*"), Some(BinaryOp::Mul));
        assert_eq!(BinaryOp::from_str("??"), None);
    }

    #[test]
    fn test_node_span() {
        let lit = AstNode::Literal(Literal {
            span: Span::from_positions(1, 1, 1, 3),
            value: LiteralValue::Integer(100),
        });
        assert_eq!(lit.span().start.line, 1);
        assert_eq!(lit.span().end.col, 3);
    }

    #[test]
    fn test_kind_name() {
        let lit = AstNode::Literal(Literal {
            span: Span::default(),
            value: LiteralValue::Integer(1),
        });
        assert_eq!(lit.kind_name(), "Literal");
    }

    #[test]
    fn test_default_builder_make_leaf() {
        let builder = DefaultAstBuilder;
        let token = Token::new(TokenKind::Integer(42), "42".to_string(), 1, 1);
        let node = builder.make_leaf(&token);
        assert!(matches!(node, AstNode::Literal(_)));
        if let AstNode::Literal(lit) = node {
            assert_eq!(lit.value, LiteralValue::Integer(42));
        }
    }
}
