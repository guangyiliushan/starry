/// 声明式规则构建宏
///
/// 用法:
/// ```
/// build_rules! {
///     rule "binary_arith" {
///         pattern: BinaryOp(Add, Sub, Mul, Div, Mod);
///         pre: [];
///         post: [
///             ...
///         ];
///     }
/// }
/// ```
#[macro_export]
macro_rules! build_rules {
    ($($rule_name:literal { $($field:ident : $value:tt);* $(;)? })*) => {
        {
            let mut table = $crate::rules::RuleTable::new();
            $(
                let rule = build_rules!(@rule $rule_name { $($field : $value);* });
                table.register(rule);
            )*
            table
        }
    };

    (@rule $name:literal { $($field:ident : $value:tt);* }) => {
        {
            let mut rule = $crate::rules::SemanticRule::new(
                $name,
                build_rules!(@pattern $($field : $value);*)
            );
            $(
                build_rules!(@apply rule, $field, $value);
            )*
            rule
        }
    };

    (@apply $rule:ident, pattern, $value:tt) => {};

    (@apply $rule:ident, pre, [ $($action:expr),* $(,)? ]) => {
        $rule.pre_actions = vec![$($action),*];
    };

    (@apply $rule:ident, post, [ $($action:expr),* $(,)? ]) => {
        $rule.post_actions = vec![$($action),*];
    };

    (@pattern pattern: $p:ident ($($op:ident),* $(,)?)) => {
        $crate::rules::NodePattern::$p { ops: vec![$(starry_ast::BinaryOp::$op),*] }
    };

    (@pattern pattern: $p:ident) => {
        $crate::rules::NodePattern::$p
    };

    (@pattern pattern: Exact($kind:ident)) => {
        $crate::rules::NodePattern::Exact($crate::rules::AstNodeKind::$kind)
    };
}
