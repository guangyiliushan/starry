use crate::rules::action::{
    Action, AttrField, BinValOp, BuiltinFn, ErrorTemplate, FoldOp, UnValOp, ValueExpr,
};
use crate::rules::rule::{AstNodeKind, NodePattern, SemanticRule};
use crate::rules::table::RuleTable;
use crate::ty::Ty;
use starry_ast::BinaryOp;
use starry_ast::UnaryOp;

pub fn build_type_check_rules() -> RuleTable {
    let mut table = RuleTable::new();

    table.register(
        SemanticRule::new(
            "binary_arith",
            NodePattern::BinaryOp {
                ops: vec![
                    BinaryOp::Add,
                    BinaryOp::Sub,
                    BinaryOp::Mul,
                    BinaryOp::Div,
                    BinaryOp::Mod,
                ],
            },
        )
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::BinaryVal {
                    op: BinValOp::Or,
                    left: Box::new(ValueExpr::UnaryVal {
                        op: UnValOp::Not,
                        operand: Box::new(ValueExpr::Call {
                            func: BuiltinFn::IsNumeric,
                            args: vec![ValueExpr::ChildAttr {
                                index: 0,
                                field: AttrField::Ty,
                            }],
                        }),
                    }),
                    right: Box::new(ValueExpr::UnaryVal {
                        op: UnValOp::Not,
                        operand: Box::new(ValueExpr::Call {
                            func: BuiltinFn::IsNumeric,
                            args: vec![ValueExpr::ChildAttr {
                                index: 1,
                                field: AttrField::Ty,
                            }],
                        }),
                    }),
                },
                error: ErrorTemplate::InvalidBinaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::Call {
                    func: BuiltinFn::Promote,
                    args: vec![
                        ValueExpr::ChildAttr {
                            index: 0,
                            field: AttrField::Ty,
                        },
                        ValueExpr::ChildAttr {
                            index: 1,
                            field: AttrField::Ty,
                        },
                    ],
                },
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
            Action::SetSynthesized {
                field: AttrField::IsConst,
                value: ValueExpr::BinaryVal {
                    op: BinValOp::And,
                    left: Box::new(ValueExpr::ChildAttr {
                        index: 0,
                        field: AttrField::IsConst,
                    }),
                    right: Box::new(ValueExpr::ChildAttr {
                        index: 1,
                        field: AttrField::IsConst,
                    }),
                },
            },
            Action::FoldIfConst {
                target_field: AttrField::ConstValue,
                op_fold: FoldOp::NodeBinary,
            },
        ]),
    );

    table.register(
        SemanticRule::new(
            "binary_compare",
            NodePattern::BinaryOp {
                ops: vec![
                    BinaryOp::Eq,
                    BinaryOp::Ne,
                    BinaryOp::Lt,
                    BinaryOp::Le,
                    BinaryOp::Gt,
                    BinaryOp::Ge,
                ],
            },
        )
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::Call {
                    func: BuiltinFn::IsCompatible,
                    args: vec![
                        ValueExpr::ChildAttr {
                            index: 0,
                            field: AttrField::Ty,
                        },
                        ValueExpr::ChildAttr {
                            index: 1,
                            field: AttrField::Ty,
                        },
                    ],
                },
                error: ErrorTemplate::InvalidBinaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::ConstTy(Ty::Bool),
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
        ]),
    );

    table.register(
        SemanticRule::new(
            "binary_logical",
            NodePattern::BinaryOp {
                ops: vec![BinaryOp::And, BinaryOp::Or],
            },
        )
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::BinaryVal {
                    op: BinValOp::And,
                    left: Box::new(ValueExpr::Call {
                        func: BuiltinFn::IsNumeric,
                        args: vec![ValueExpr::ChildAttr {
                            index: 0,
                            field: AttrField::Ty,
                        }],
                    }),
                    right: Box::new(ValueExpr::Call {
                        func: BuiltinFn::IsNumeric,
                        args: vec![ValueExpr::ChildAttr {
                            index: 1,
                            field: AttrField::Ty,
                        }],
                    }),
                },
                error: ErrorTemplate::InvalidBinaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::ConstTy(Ty::Bool),
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
        ]),
    );

    table.register(
        SemanticRule::new("unary_neg", NodePattern::UnaryOp {
            ops: vec![UnaryOp::Neg],
        })
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::Call {
                    func: BuiltinFn::IsNumeric,
                    args: vec![ValueExpr::ChildAttr {
                        index: 0,
                        field: AttrField::Ty,
                    }],
                },
                error: ErrorTemplate::InvalidUnaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::ChildAttr {
                    index: 0,
                    field: AttrField::Ty,
                },
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
            Action::SetSynthesized {
                field: AttrField::IsConst,
                value: ValueExpr::ChildAttr {
                    index: 0,
                    field: AttrField::IsConst,
                },
            },
            Action::FoldIfConst {
                target_field: AttrField::ConstValue,
                op_fold: FoldOp::Unary { op: UnaryOp::Neg },
            },
        ]),
    );

    table.register(
        SemanticRule::new("unary_not", NodePattern::UnaryOp {
            ops: vec![UnaryOp::Not],
        })
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::Call {
                    func: BuiltinFn::IsNumeric,
                    args: vec![ValueExpr::ChildAttr {
                        index: 0,
                        field: AttrField::Ty,
                    }],
                },
                error: ErrorTemplate::InvalidUnaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::ConstTy(Ty::Bool),
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
            Action::SetSynthesized {
                field: AttrField::IsConst,
                value: ValueExpr::ChildAttr {
                    index: 0,
                    field: AttrField::IsConst,
                },
            },
            Action::FoldIfConst {
                target_field: AttrField::ConstValue,
                op_fold: FoldOp::Unary { op: UnaryOp::Not },
            },
        ]),
    );

    table.register(
        SemanticRule::new("unary_pos", NodePattern::UnaryOp {
            ops: vec![UnaryOp::Pos],
        })
        .with_post_actions(vec![
            Action::Check {
                condition: ValueExpr::Call {
                    func: BuiltinFn::IsNumeric,
                    args: vec![ValueExpr::ChildAttr {
                        index: 0,
                        field: AttrField::Ty,
                    }],
                },
                error: ErrorTemplate::InvalidUnaryOp,
            },
            Action::SetSynthesized {
                field: AttrField::Ty,
                value: ValueExpr::ChildAttr {
                    index: 0,
                    field: AttrField::Ty,
                },
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
        ]),
    );

    table.register(
        SemanticRule::new("literal", NodePattern::AnyLiteral).with_post_actions(vec![
            Action::SetSynthesized {
                field: AttrField::IsConst,
                value: ValueExpr::ConstBool(true),
            },
            Action::SetSynthesized {
                field: AttrField::IsLvalue,
                value: ValueExpr::ConstBool(false),
            },
        ]),
    );

    table.register(
        SemanticRule::new("ident", NodePattern::AnyIdent).with_post_actions(vec![
            Action::LookupSymbol {
                name_expr: ValueExpr::NodeName,
            },
        ]),
    );

    table.register(
        SemanticRule::new("block", NodePattern::AnyBlock)
            .with_pre_actions(vec![Action::PushScope])
            .with_post_actions(vec![Action::PopScope, Action::SequenceLast]),
    );

    table.register(
        SemanticRule::new("expr_stmt", NodePattern::Exact(AstNodeKind::ExprStmt))
            .with_post_actions(vec![Action::SequenceFirst]),
    );

    table.register(
        SemanticRule::new("root", NodePattern::Exact(AstNodeKind::Root))
            .with_post_actions(vec![Action::SequenceLast]),
    );

    table.register(
        SemanticRule::new("paren", NodePattern::Exact(AstNodeKind::Paren))
            .with_post_actions(vec![Action::PassThrough { child_index: 0 }]),
    );

    table
}
