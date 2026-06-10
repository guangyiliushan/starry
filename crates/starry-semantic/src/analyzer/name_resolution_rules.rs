use crate::rules::action::{Action, ValueExpr};
use crate::rules::rule::{NodePattern, SemanticRule};
use crate::rules::table::RuleTable;

pub fn build_name_resolution_rules() -> RuleTable {
    let mut table = RuleTable::new();

    table.register(
        SemanticRule::new("nr_ident", NodePattern::AnyIdent).with_pre_actions(vec![
            Action::DefinePlaceholder {
                name_expr: ValueExpr::NodeName,
            },
        ]),
    );

    table.register(
        SemanticRule::new("nr_block", NodePattern::AnyBlock)
            .with_pre_actions(vec![Action::PushScope])
            .with_post_actions(vec![Action::PopScope]),
    );

    table
}
