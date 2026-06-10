use crate::cfg::{ContextFreeGrammar, Production, Symbol};
use starry_ast::{AstBuilder, AstNode, BinaryExpr, BinaryOp, Ident, Literal, LiteralValue};
use starry_ast::{ParenExpr, Span, Token, TokenKind, UnaryExpr, UnaryOp};

pub fn is_epsilon_node(node: &AstNode) -> bool {
    matches!(node, AstNode::Ident(i) if i.name == "<ε>")
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
    _productions: Vec<Production>,
    _terminal_names: Vec<String>,
    _non_terminal_names: Vec<String>,
}

impl GrammarAstBuilder {
    pub fn new(cfg: &ContextFreeGrammar) -> Self {
        GrammarAstBuilder {
            _productions: cfg.productions.clone(),
            _terminal_names: cfg.terminals.clone(),
            _non_terminal_names: cfg.non_terminals.clone(),
        }
    }

    fn is_terminal_named(&self, symbol: &Symbol, name: &str) -> bool {
        match symbol {
            Symbol::Terminal(id) => self._terminal_names.get(id.0).map(|n| n == name).unwrap_or(false),
            _ => false,
        }
    }

    fn get_lhs_name(&self, prod: &Production) -> String {
        self._non_terminal_names.get(prod.lhs.0).cloned().unwrap_or_default()
    }
}

impl GrammarAstBuilder {
    pub fn reduce_by_production(&self, prod: &Production, children: Vec<AstNode>) -> AstNode {
        let span = Span::merge_opt(&children.iter().map(|c| c.span()).collect::<Vec<_>>());
        let rhs = &prod.rhs;
        let is_tail = is_tail_production(prod);
        let rhs_len = rhs.len();

        if prod.is_epsilon() {
            return AstNode::Ident(Ident {
                span: Span::default(),
                name: "<ε>".to_string(),
            });
        }

        if rhs_len == 1 {
            match &rhs[0] {
                Symbol::NonTerminal(_) | Symbol::Terminal(_) => {
                    return children.into_iter().next().unwrap();
                }
                Symbol::Epsilon => {
                    return AstNode::Ident(Ident {
                        span: Span::default(),
                        name: "<ε>".to_string(),
                    });
                }
            }
        }

        if is_tail {
            let accumulator = &children[0];
            let rhs_children = &children[1..];

            if rhs_len == 3
                && matches!(&rhs[0], Symbol::Terminal(_))
                && matches!(&rhs[1], Symbol::NonTerminal(_))
                && matches!(&rhs[2], Symbol::NonTerminal(_))
            {
                if let AstNode::Ident(op_ident) = &rhs_children[0] {
                    if let Some(op) = BinaryOp::from_str(&op_ident.name) {
                        let right = if rhs_children.len() >= 3 && !is_epsilon_node(&rhs_children[2]) {
                            rhs_children[2].clone()
                        } else {
                            rhs_children[1].clone()
                        };
                        return AstNode::Binary(BinaryExpr {
                            span,
                            left: Box::new(accumulator.clone()),
                            op,
                            right: Box::new(right),
                        });
                    }
                }
            }

            return pass_through(children, span, &self.get_lhs_name(prod));
        }

        if rhs_len == 3
            && matches!(&rhs[0], Symbol::NonTerminal(_))
            && matches!(&rhs[1], Symbol::Terminal(_))
            && matches!(&rhs[2], Symbol::NonTerminal(_))
        {
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
        }

        if rhs_len == 3
            && self.is_terminal_named(&rhs[0], "(")
            && matches!(&rhs[1], Symbol::NonTerminal(_))
            && self.is_terminal_named(&rhs[2], ")")
        {
            return AstNode::Paren(ParenExpr {
                span,
                expr: Box::new(children[1].clone()),
            });
        }

        if rhs_len == 2
            && matches!(&rhs[0], Symbol::Terminal(_))
            && matches!(&rhs[1], Symbol::NonTerminal(_))
        {
            if let AstNode::Ident(op_ident) = &children[0] {
                if let Some(op) = UnaryOp::from_str(&op_ident.name) {
                    return AstNode::Unary(UnaryExpr {
                        span,
                        op,
                        expr: Box::new(children[1].clone()),
                    });
                }
            }
        }

        pass_through(children, span, &self.get_lhs_name(prod))
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
        let prod = &self._productions[prod_id];
        self.reduce_by_production(prod, children)
    }
}

fn pass_through(children: Vec<AstNode>, span: Span, lhs_name: &str) -> AstNode {
    children
        .into_iter()
        .find(|c| !is_epsilon_node(c))
        .unwrap_or_else(|| {
            AstNode::Ident(Ident {
                span,
                name: format!("<{}>", lhs_name),
            })
        })
}
