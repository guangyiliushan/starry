use crate::rules::action::{Action, InstrTemplate};
use crate::rules::rule::{AstNodeKind, NodePattern, SemanticRule};
use crate::rules::table::RuleTable;
use starry_ast::BinaryOp;

pub fn build_ir_gen_rules() -> RuleTable {
    let mut table = RuleTable::new();

    table.register(
        SemanticRule::new("ir_binary", NodePattern::BinaryOp {
            ops: vec![
                BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Div, BinaryOp::Mod,
                BinaryOp::Eq, BinaryOp::Ne, BinaryOp::Lt, BinaryOp::Le, BinaryOp::Gt, BinaryOp::Ge,
                BinaryOp::And, BinaryOp::Or,
            ],
        })
        .with_post_actions(vec![Action::Emit { instr_template: InstrTemplate::BinaryAssign }]),
    );

    table.register(
        SemanticRule::new("ir_unary", NodePattern::Exact(AstNodeKind::Unary))
            .with_post_actions(vec![Action::Emit { instr_template: InstrTemplate::UnaryAssign }]),
    );

    table.register(
        SemanticRule::new("ir_literal", NodePattern::AnyLiteral)
            .with_post_actions(vec![Action::Emit { instr_template: InstrTemplate::LoadConst }]),
    );

    table.register(
        SemanticRule::new("ir_ident", NodePattern::AnyIdent)
            .with_post_actions(vec![Action::Emit { instr_template: InstrTemplate::CopyName }]),
    );

    table.register(
        SemanticRule::new("ir_block", NodePattern::AnyBlock)
            .with_pre_actions(vec![Action::PushScope])
            .with_post_actions(vec![Action::PopScope, Action::SequenceLast]),
    );

    table.register(
        SemanticRule::new("ir_root", NodePattern::Exact(AstNodeKind::Root))
            .with_post_actions(vec![Action::SequenceLast]),
    );

    table.register(
        SemanticRule::new("ir_expr_stmt", NodePattern::Exact(AstNodeKind::ExprStmt))
            .with_post_actions(vec![Action::SequenceFirst]),
    );

    table.register(
        SemanticRule::new("ir_paren", NodePattern::Exact(AstNodeKind::Paren))
            .with_post_actions(vec![Action::PassThrough { child_index: 0 }]),
    );

    table
}
