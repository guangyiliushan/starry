use crate::cfg::{ContextFreeGrammar, Production, Symbol};
use starry_ast::{AstBuilder, AstNode, BinaryExpr, BinaryOp, Ident, Literal, LiteralValue};
use starry_ast::{ParenExpr, Span, Token, TokenKind, UnaryExpr, UnaryOp};

pub fn is_epsilon_node(node: &AstNode) -> bool {
    matches!(node, AstNode::Ident(i) if i.name == "<ε>")
}

fn is_delim(node: &AstNode, expected: &str) -> bool {
    matches!(node, AstNode::Ident(i) if i.name == expected)
}

pub fn is_tail_production(production: &Production) -> bool {
    if production.rhs.is_empty() {
        return false;
    }
    if production.rhs.len() == 1 && matches!(production.rhs[0], Symbol::Epsilon) {
        return false;
    }
    matches!(production.rhs.last(), Some(Symbol::NonTerminal(nt)) if *nt == production.lhs)
}

#[derive(Clone)]
pub struct GrammarAstBuilder {
    productions: Vec<Production>,
}

impl GrammarAstBuilder {
    pub fn new(cfg: &ContextFreeGrammar) -> Self {
        GrammarAstBuilder {
            productions: cfg.productions.clone(),
        }
    }
}

impl AstBuilder for GrammarAstBuilder {
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

        match children.len() {
            0 => {
                AstNode::Ident(Ident {
                    span: Span::default(),
                    name: "<ε>".to_string(),
                })
            }

            1 => {
                children.into_iter().next().unwrap()
            }

            2 => {
                let first = &children[0];
                let second = &children[1];

                if let AstNode::Ident(op_ident) = first {
                    if let Some(op) = UnaryOp::from_str(&op_ident.name) {
                        return AstNode::Unary(UnaryExpr {
                            span,
                            op,
                            expr: Box::new(second.clone()),
                        });
                    }
                }

                if is_epsilon_node(second) {
                    first.clone()
                } else {
                    second.clone()
                }
            }

            3 => {
                if let AstNode::Ident(op_ident) = &children[1] {
                    if let Some(op) = BinaryOp::from_str(&op_ident.name) {
                        return AstNode::Binary(BinaryExpr {
                            span,
                            left: Box::new(children[0].clone()),
                            op,
                            right: Box::new(children[2].clone()),
                        });
                    }
                }

                if is_delim(&children[0], "(") && is_delim(&children[2], ")") {
                    return AstNode::Paren(ParenExpr {
                        span,
                        expr: Box::new(children[1].clone()),
                    });
                }

                pass_through(children, span, prod_id)
            }

            4 => {
                if let AstNode::Ident(op_ident) = &children[1] {
                    if let Some(op) = BinaryOp::from_str(&op_ident.name) {
                        let right = if is_epsilon_node(&children[3]) {
                            children[2].clone()
                        } else {
                            children[3].clone()
                        };
                        return AstNode::Binary(BinaryExpr {
                            span,
                            left: Box::new(children[0].clone()),
                            op,
                            right: Box::new(right),
                        });
                    }
                }

                pass_through(children, span, prod_id)
            }

            _ => pass_through(children, span, prod_id),
        }
    }
}

fn pass_through(children: Vec<AstNode>, span: Span, prod_id: usize) -> AstNode {
    children
        .into_iter()
        .find(|c| !is_epsilon_node(c))
        .unwrap_or_else(|| {
            AstNode::Ident(Ident {
                span,
                name: format!("<prod_{}>", prod_id),
            })
        })
}
