//! 规则 DSL 解析器
//!
//! 将声明式规则文本解析为 `RuleTable`，供 `RuleDrivenEvaluator` 使用。
//!
//! # DSL 格式
//!
//! ```text
//! # 注释行
//! @type_check                    # section marker
//! rule_name BinaryOp(Add,Sub)    # 规则头：名称 + 模式
//!   check: !is_numeric(@0) => InvalidBinaryOp
//!   ty: promote(@0, @1)
//!   is_lvalue: false
//!   fold: Binary(Add)
//! ```
//!
//! # 语法元素
//!
//! - `@section`：标记规则类别（`@type_check` / `@ir_gen` / `@name_resolution`）
//! - `name pattern`：规则定义行
//! - 缩进行：动作定义
//! - `@0` / `@1`：引用子节点属性
//! - `@0.ty` / `@0.is_const`：引用子节点的具体字段

use starry_ast::{BinaryOp, UnaryOp};

use crate::rules::action::{Action, AttrField, BinValOp, BuiltinFn, ErrorTemplate, FoldOp, UnValOp, ValueExpr};
use crate::rules::rule::{AstNodeKind, NodePattern, SemanticRule};
use crate::rules::table::RuleTable;
use crate::ty::Ty;

/// 规则所属的语义分析阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSection {
    TypeCheck,
    IrGen,
    NameResolution,
}

/// 解析后的规则
#[derive(Debug, Clone)]
pub struct ParsedRule {
    pub section: RuleSection,
    pub rule: SemanticRule,
}

/// 解析规则文本，返回规则列表
///
/// # Example
///
/// ```
/// let text = r#"
/// @type_check
/// binary_arith BinaryOp(Add,Sub,Mul,Div,Mod)
///   ty: promote(@0, @1)
/// "#;
/// let rules = parse_rules(text);
/// ```
pub fn parse_rules(input: &str) -> Vec<ParsedRule> {
    let mut rules = Vec::new();
    let mut current_section = RuleSection::TypeCheck;
    let mut current_name = String::new();
    let mut current_pattern: Option<NodePattern> = None;
    let mut current_pre: Vec<Action> = Vec::new();
    let mut current_post: Vec<Action> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Section markers
        if let Some(section) = parse_section_marker(trimmed) {
            flush_rule(
                &mut current_name,
                &mut current_pattern,
                &mut current_pre,
                &mut current_post,
                &current_section,
                &mut rules,
            );
            current_section = section;
            continue;
        }

        // Indented lines = actions
        if line.starts_with("  ") || line.starts_with('\t') {
            if let Some(action) = parse_action(trimmed) {
                if trimmed.starts_with("pre:") {
                    current_pre.push(action);
                } else {
                    current_post.push(action);
                }
            }
            continue;
        }

        // New rule header
        flush_rule(
            &mut current_name,
            &mut current_pattern,
            &mut current_pre,
            &mut current_post,
            &current_section,
            &mut rules,
        );

        if let Some((name, pattern)) = parse_rule_header(trimmed) {
            current_name = name;
            current_pattern = Some(pattern);
        }
    }

    // Flush final rule
    flush_rule(
        &mut current_name,
        &mut current_pattern,
        &mut current_pre,
        &mut current_post,
        &current_section,
        &mut rules,
    );

    rules
}

/// 将解析后的规则按 section 分组为三个 RuleTable
pub fn build_rule_tables(rules: &[ParsedRule]) -> (RuleTable, RuleTable, RuleTable) {
    let mut nr_table = RuleTable::new();
    let mut tc_table = RuleTable::new();
    let mut ir_table = RuleTable::new();

    for pr in rules {
        match pr.section {
            RuleSection::NameResolution => nr_table.register(pr.rule.clone()),
            RuleSection::TypeCheck => tc_table.register(pr.rule.clone()),
            RuleSection::IrGen => ir_table.register(pr.rule.clone()),
        }
    }

    (nr_table, tc_table, ir_table)
}

fn flush_rule(
    name: &mut String,
    pattern: &mut Option<NodePattern>,
    pre: &mut Vec<Action>,
    post: &mut Vec<Action>,
    section: &RuleSection,
    rules: &mut Vec<ParsedRule>,
) {
    if let Some(pat) = pattern.take() {
        if !name.is_empty() {
            rules.push(ParsedRule {
                section: *section,
                rule: SemanticRule {
                    name: std::mem::take(name),
                    pattern: pat,
                    pre_actions: std::mem::take(pre),
                    post_actions: std::mem::take(post),
                },
            });
        }
    }
    name.clear();
    pre.clear();
    post.clear();
}

fn parse_section_marker(s: &str) -> Option<RuleSection> {
    match s {
        "@type_check" => Some(RuleSection::TypeCheck),
        "@ir_gen" => Some(RuleSection::IrGen),
        "@name_resolution" => Some(RuleSection::NameResolution),
        _ => None,
    }
}

fn parse_rule_header(line: &str) -> Option<(String, NodePattern)> {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let name = parts[0].to_string();
    let pattern = parse_pattern(parts[1])?;
    Some((name, pattern))
}

fn parse_pattern(s: &str) -> Option<NodePattern> {
    // Wildcard patterns
    if s == "AnyBlock" {
        return Some(NodePattern::AnyBlock);
    }
    if s == "AnyIdent" {
        return Some(NodePattern::AnyIdent);
    }
    if s == "AnyLiteral" {
        return Some(NodePattern::AnyLiteral);
    }

    // BinaryOp(Add,Sub,...)
    if let Some(inner) = s.strip_prefix("BinaryOp(").and_then(|s| s.strip_suffix(')')) {
        let ops: Vec<BinaryOp> = inner.split(',').filter_map(|op| parse_binary_op(op.trim())).collect();
        if !ops.is_empty() {
            return Some(NodePattern::BinaryOp { ops });
        }
    }

    // UnaryOp(Neg,Not,...)
    if let Some(inner) = s.strip_prefix("UnaryOp(").and_then(|s| s.strip_suffix(')')) {
        let ops: Vec<UnaryOp> = inner.split(',').filter_map(|op| parse_unary_op(op.trim())).collect();
        if !ops.is_empty() {
            return Some(NodePattern::UnaryOp { ops });
        }
    }

    // Exact node kind
    let kind = match s {
        "Root" => AstNodeKind::Root,
        "Block" => AstNodeKind::Block,
        "ExprStmt" => AstNodeKind::ExprStmt,
        "Binary" => AstNodeKind::Binary,
        "Unary" => AstNodeKind::Unary,
        "Literal" => AstNodeKind::Literal,
        "Ident" => AstNodeKind::Ident,
        "Paren" => AstNodeKind::Paren,
        _ => return None,
    };
    Some(NodePattern::Exact(kind))
}

fn parse_binary_op(s: &str) -> Option<BinaryOp> {
    match s {
        "Add" => Some(BinaryOp::Add),
        "Sub" => Some(BinaryOp::Sub),
        "Mul" => Some(BinaryOp::Mul),
        "Div" => Some(BinaryOp::Div),
        "Mod" => Some(BinaryOp::Mod),
        "Eq" => Some(BinaryOp::Eq),
        "Ne" => Some(BinaryOp::Ne),
        "Lt" => Some(BinaryOp::Lt),
        "Le" => Some(BinaryOp::Le),
        "Gt" => Some(BinaryOp::Gt),
        "Ge" => Some(BinaryOp::Ge),
        "And" => Some(BinaryOp::And),
        "Or" => Some(BinaryOp::Or),
        _ => None,
    }
}

fn parse_unary_op(s: &str) -> Option<UnaryOp> {
    match s {
        "Neg" => Some(UnaryOp::Neg),
        "Not" => Some(UnaryOp::Not),
        "Pos" => Some(UnaryOp::Pos),
        _ => None,
    }
}

// ============================================================================
// Action Parsing
// ============================================================================

fn parse_action(s: &str) -> Option<Action> {
    let s = s.trim();

    // Handle pre: prefix
    if let Some(inner) = s.strip_prefix("pre:") {
        return parse_single_action(inner.trim());
    }

    parse_single_action(s)
}

fn parse_single_action(s: &str) -> Option<Action> {
    // Simple actions
    match s {
        "PushScope" => return Some(Action::PushScope),
        "PopScope" => return Some(Action::PopScope),
        "SequenceLast" => return Some(Action::SequenceLast),
        "SequenceFirst" => return Some(Action::SequenceFirst),
        "NewTemp" => return Some(Action::NewTemp),
        "ReturnError" => return Some(Action::ReturnError),
        _ => {}
    }

    // PassThrough(n)
    if let Some(inner) = s.strip_prefix("PassThrough(").and_then(|s| s.strip_suffix(')')) {
        if let Ok(idx) = inner.trim().parse() {
            return Some(Action::PassThrough { child_index: idx });
        }
    }

    // DefinePlaceholder / LookupSymbol
    if s.starts_with("DefinePlaceholder") {
        return Some(Action::DefinePlaceholder { name_expr: ValueExpr::NodeName });
    }
    if s.starts_with("LookupSymbol") {
        return Some(Action::LookupSymbol { name_expr: ValueExpr::NodeName });
    }

    // Emit actions for IR generation
    if let Some(inner) = s.strip_prefix("Emit(").and_then(|s| s.strip_suffix(')')) {
        let template = match inner.trim() {
            "BinaryAssign" => crate::rules::action::InstrTemplate::BinaryAssign,
            "UnaryAssign" => crate::rules::action::InstrTemplate::UnaryAssign,
            "LoadConst" => crate::rules::action::InstrTemplate::LoadConst,
            "CopyName" => crate::rules::action::InstrTemplate::CopyName,
            _ => return None,
        };
        return Some(Action::Emit { instr_template: template });
    }

    // check: condition => ErrorTemplate
    if let Some(inner) = s.strip_prefix("check:") {
        let parts: Vec<&str> = inner.split("=>").collect();
        if parts.len() == 2 {
            let cond = parse_bool_expr(parts[0].trim());
            let err = parse_error_template(parts[1].trim())?;
            return Some(Action::Check { condition: cond, error: err });
        }
    }

    // ty: value
    if let Some(inner) = s.strip_prefix("ty:") {
        let val = parse_value_expr(inner.trim())?;
        return Some(Action::SetSynthesized { field: AttrField::Ty, value: val });
    }

    // is_lvalue: bool
    if let Some(inner) = s.strip_prefix("is_lvalue:") {
        let val = parse_bool_expr(inner.trim());
        return Some(Action::SetSynthesized { field: AttrField::IsLvalue, value: val });
    }

    // is_const: bool
    if let Some(inner) = s.strip_prefix("is_const:") {
        let val = parse_bool_expr(inner.trim());
        return Some(Action::SetSynthesized { field: AttrField::IsConst, value: val });
    }

    // fold: Binary(op) or Unary(op) or Binary(@op) or Unary(@op)
    if let Some(inner) = s.strip_prefix("fold:") {
        if let Some(op_str) = inner.trim().strip_prefix("Binary(").and_then(|s| s.strip_suffix(')')) {
            if op_str.trim() == "@op" {
                return Some(Action::FoldIfConst {
                    target_field: AttrField::ConstValue,
                    op_fold: FoldOp::NodeBinary,
                });
            }
            if let Some(op) = parse_binary_op(op_str.trim()) {
                return Some(Action::FoldIfConst {
                    target_field: AttrField::ConstValue,
                    op_fold: FoldOp::Binary { op },
                });
            }
        }
        if let Some(op_str) = inner.trim().strip_prefix("Unary(").and_then(|s| s.strip_suffix(')')) {
            if op_str.trim() == "@op" {
                return Some(Action::FoldIfConst {
                    target_field: AttrField::ConstValue,
                    op_fold: FoldOp::NodeUnary,
                });
            }
            if let Some(op) = parse_unary_op(op_str.trim()) {
                return Some(Action::FoldIfConst {
                    target_field: AttrField::ConstValue,
                    op_fold: FoldOp::Unary { op },
                });
            }
        }
    }

    None
}

// ============================================================================
// Value Expression Parsing
// ============================================================================

fn parse_value_expr(s: &str) -> Option<ValueExpr> {
    let s = s.trim();

    // promote(@0, @1)
    if let Some(inner) = s.strip_prefix("promote(").and_then(|s| s.strip_suffix(')')) {
        let args: Vec<&str> = inner.split(',').collect();
        if args.len() == 2 {
            return Some(ValueExpr::Call {
                func: BuiltinFn::Promote,
                args: vec![
                    parse_value_expr(args[0].trim())?,
                    parse_value_expr(args[1].trim())?,
                ],
            });
        }
    }

    // is_numeric(@0)
    if let Some(inner) = s.strip_prefix("is_numeric(").and_then(|s| s.strip_suffix(')')) {
        return Some(ValueExpr::Call {
            func: BuiltinFn::IsNumeric,
            args: vec![parse_value_expr(inner.trim())?],
        });
    }

    // is_compatible(@0, @1)
    if let Some(inner) = s.strip_prefix("is_compatible(").and_then(|s| s.strip_suffix(')')) {
        let args: Vec<&str> = inner.split(',').collect();
        if args.len() == 2 {
            return Some(ValueExpr::Call {
                func: BuiltinFn::IsCompatible,
                args: vec![
                    parse_value_expr(args[0].trim())?,
                    parse_value_expr(args[1].trim())?,
                ],
            });
        }
    }

    // @0.ty, @1.is_const, @2.is_lvalue
    if let Some(rest) = s.strip_prefix('@') {
        if let Some(dot_pos) = rest.find('.') {
            let idx: usize = rest[..dot_pos].parse().ok()?;
            let field = match &rest[dot_pos + 1..] {
                "ty" => AttrField::Ty,
                "is_const" => AttrField::IsConst,
                "is_lvalue" => AttrField::IsLvalue,
                _ => return None,
            };
            return Some(ValueExpr::ChildAttr { index: idx, field });
        } else {
            // @0 shorthand for @0.ty
            let idx: usize = rest.parse().ok()?;
            return Some(ValueExpr::ChildAttr { index: idx, field: AttrField::Ty });
        }
    }

    // Type literals
    match s {
        "Int" => return Some(ValueExpr::ConstTy(Ty::Int)),
        "Float" => return Some(ValueExpr::ConstTy(Ty::Float)),
        "Bool" | "bool" => return Some(ValueExpr::ConstTy(Ty::Bool)),
        "String" => return Some(ValueExpr::ConstTy(Ty::String)),
        "Void" => return Some(ValueExpr::ConstTy(Ty::Void)),
        _ => {}
    }

    // Boolean literals
    match s {
        "true" => return Some(ValueExpr::ConstBool(true)),
        "false" => return Some(ValueExpr::ConstBool(false)),
        _ => {}
    }

    // Node expressions
    if s == "NodeName" {
        return Some(ValueExpr::NodeName);
    }
    if s == "NodeOp" {
        return Some(ValueExpr::NodeOp);
    }
    if s == "NodeSpan" {
        return Some(ValueExpr::NodeSpan);
    }
    if s == "NodeValue" {
        return Some(ValueExpr::NodeValue);
    }

    None
}

fn parse_bool_expr(s: &str) -> ValueExpr {
    let s = s.trim();

    // Handle && (lower precedence than ||)
    if let Some(pos) = find_operator(s, "&&") {
        return ValueExpr::BinaryVal {
            op: BinValOp::And,
            left: Box::new(parse_bool_expr(&s[..pos])),
            right: Box::new(parse_bool_expr(&s[pos + 2..].trim())),
        };
    }

    // Handle ||
    if let Some(pos) = find_operator(s, "||") {
        return ValueExpr::BinaryVal {
            op: BinValOp::Or,
            left: Box::new(parse_bool_expr(&s[..pos])),
            right: Box::new(parse_bool_expr(&s[pos + 2..].trim())),
        };
    }

    // Handle ! (not)
    if let Some(rest) = s.strip_prefix('!') {
        return ValueExpr::UnaryVal {
            op: UnValOp::Not,
            operand: Box::new(parse_bool_expr(rest.trim())),
        };
    }

    // Try as value expression
    if let Some(ve) = parse_value_expr(s) {
        return ve;
    }

    // Fallback
    ValueExpr::ConstBool(false)
}

/// Find operator position, respecting parentheses
fn find_operator(s: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let chars: Vec<char> = s.chars().collect();
    let op_chars: Vec<char> = op.chars().collect();

    for i in 0..chars.len().saturating_sub(op_chars.len() - 1) {
        if chars[i] == '(' {
            depth += 1;
        } else if chars[i] == ')' {
            depth -= 1;
        } else if depth == 0 {
            let slice: String = chars[i..i + op_chars.len()].iter().collect();
            if slice == op {
                return Some(i);
            }
        }
    }
    None
}

fn parse_error_template(s: &str) -> Option<ErrorTemplate> {
    match s.trim() {
        "InvalidBinaryOp" => Some(ErrorTemplate::InvalidBinaryOp),
        "InvalidUnaryOp" => Some(ErrorTemplate::InvalidUnaryOp),
        "UndefinedName" => Some(ErrorTemplate::UndefinedName),
        "UnresolvedType" => Some(ErrorTemplate::UnresolvedType),
        "Redeclaration" => Some(ErrorTemplate::Redeclaration),
        "TypeMismatch" => Some(ErrorTemplate::TypeMismatch),
        s => Some(ErrorTemplate::General(s.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let text = r#"
@type_check
binary_arith BinaryOp(Add,Sub)
  ty: promote(@0, @1)
"#;
        let rules = parse_rules(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule.name, "binary_arith");
    }

    #[test]
    fn test_parse_multiple_sections() {
        let text = r#"
@name_resolution
nr_ident AnyIdent
  pre: DefinePlaceholder

@type_check
literal AnyLiteral
  is_const: true
"#;
        let rules = parse_rules(text);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].section, RuleSection::NameResolution);
        assert_eq!(rules[1].section, RuleSection::TypeCheck);
    }

    #[test]
    fn test_build_rule_tables() {
        let text = r#"
@name_resolution
nr_block AnyBlock
  pre: PushScope
  PopScope

@type_check
binary_arith BinaryOp(Add)
  ty: Int
"#;
        let rules = parse_rules(text);
        let (nr, tc, ir) = build_rule_tables(&rules);
        assert_eq!(nr.rule_count(), 1);
        assert_eq!(tc.rule_count(), 1);
        assert_eq!(ir.rule_count(), 0);
    }
}
