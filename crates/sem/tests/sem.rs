//! sem 集成测试：parser → analyze 全链路，按 DiagCode 断言
//! （parser 诊断在前、语义诊断在后的顺序不纳入断言）。

use ast::syntax::node::DiagCode;
use sem::analyze;

fn codes(source: &str) -> Vec<DiagCode> {
    let parsed = parser::parse_source(source).expect("machine parse");
    analyze(parsed).diagnostics.iter().map(|d| d.code).collect()
}

fn assert_clean(source: &str) {
    let got = codes(source);
    assert_eq!(got, Vec::new(), "unexpected diagnostics");
}

fn assert_has(source: &str, code: DiagCode) {
    let got = codes(source);
    assert!(got.contains(&code), "expected {code:?}, got {got:?}");
}

// ---- 名字解析 / 前向引用 ----

#[test]
fn forward_reference_is_clean() {
    assert_clean("fun main() {\n    helper()\n}\nfun helper() { }");
}

#[test]
fn undefined_name_reports() {
    assert_has("fun main() {\n    nosuch()\n}", DiagCode::SemUndefinedName);
}

#[test]
fn duplicate_definition_reports() {
    assert_has("fun f() { }\nfun f() { }", DiagCode::SemDuplicateDef);
}

// ---- 调用定型 ----

#[test]
fn positional_and_named_args_are_clean() {
    assert_clean("fun f(a: int, b: int) : int { return f(1, b = 2) }");
}

#[test]
fn default_parameter_fills_missing_slot() {
    assert_clean("fun f(a: int, b: int = 2) : int { return a }\nfun main() {\n    f(1)\n}");
}

#[test]
fn too_many_positionals_report() {
    assert_has(
        "fun f(a: int) : int { return a }\nfun main() {\n    f(1, 2)\n}",
        DiagCode::SemArityMismatch,
    );
}

#[test]
fn non_function_callee_reports() {
    assert_has(
        "val x = 1\nfun main() {\n    x(1)\n}",
        DiagCode::SemNotCallable,
    );
}

// ---- 字面量默认类型与提升 ----

#[test]
fn int_float_promotion_flows_to_binding() {
    let parsed = parser::parse_source("fun main() {\n    val x = 1 + 2.5\n}").unwrap();
    let out = analyze(parsed);
    assert_eq!(out.diagnostics.len(), 0, "{:?}", out.diagnostics);
    let mut interner = out.interner;
    let sym = interner.intern("x");
    for i in 0..out.defs.len() as u32 {
        let def = out.defs.get(sem::DefId(i));
        if def.name == sym {
            assert_eq!(
                out.tys.kind(def.sig.ty),
                &sem::TyKind::Float(sem::FloatKind::F64)
            );
            return;
        }
    }
    panic!("binding x not found");
}

// ---- 赋值 / 可变性 ----

#[test]
fn assign_to_val_binding_reports() {
    assert_has(
        "fun main() {\n    val x = 1\n    x = 2\n}",
        DiagCode::SemAssignToVal,
    );
}

#[test]
fn assign_to_var_binding_is_clean() {
    assert_clean("fun main() {\n    var x = 1\n    x = 2\n}");
}

// ---- 控制流 ----

#[test]
fn break_outside_loop_reports() {
    assert_has("fun main() {\n    break\n}", DiagCode::SemBreakOutsideLoop);
}

#[test]
fn unknown_label_reports() {
    assert_has(
        "fun main() {\n    'outer: loop { }\n    break 'nowhere\n}",
        DiagCode::SemUnknownLabel,
    );
}

#[test]
fn condition_must_be_bool() {
    assert_has(
        "fun main() {\n    if 1 { }\n}",
        DiagCode::SemConditionNotBool,
    );
}

// ---- when ----

#[test]
fn when_enum_partial_coverage_reports() {
    let src = "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    return when e {\n        E.A -> 1\n    }\n}";
    assert_has(src, DiagCode::SemNonExhaustiveWhen);
}

#[test]
fn when_enum_full_coverage_is_clean() {
    let src = "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    return when e {\n        E.A -> 1\n        E.B -> 2\n    }\n}";
    assert_clean(src);
}

#[test]
fn when_variant_binds_and_refinement_flows() {
    let src = "enum Opt {\n    Some(int),\n    None\n}\nfun f(o: Opt) : int {\n    return when o {\n        Opt.Some(x) -> x\n        Opt.None -> 0\n    }\n}";
    assert_clean(src);
}

#[test]
fn when_single_segment_variant_is_refined() {
    let src = "enum Opt {\n    Some(int),\n    None\n}\nfun f(o: Opt) : int {\n    return when o {\n        Some(x) -> x\n        None -> 0\n    }\n}";
    assert_clean(src);
}

#[test]
fn builtin_annotation_matches_literal_types() {
    let src = "fun f(b: bool, s: string) : int {\n    if b { }\n    val t = s\n    return 0\n}";
    assert_clean(src);
}
#[test]
fn when_int_subject_requires_else() {
    let src = "fun f(x: int) : int {\n    return when x {\n        1 -> 1\n    }\n}";
    assert_has(src, DiagCode::SemMissingElse);
}

#[test]
fn when_bool_double_coverage_is_clean() {
    let src = "fun f(b: bool) : int {\n    return when b {\n        true -> 1\n        false -> 2\n    }\n}";
    assert_clean(src);
}

#[test]
fn when_else_covers_any_subject() {
    let src = "fun f(x: int) : int {\n    return when x {\n        else -> 0\n    }\n}";
    assert_clean(src);
}

#[test]
fn when_predicate_branch_does_not_compare_with_subject() {
    let src = "fun f(x: int) : int {\n    return when x {\n        x > 1 -> 1\n        else -> 0\n    }\n}";
    assert_clean(src);
}

// ---- 结构体成员 ----

#[test]
fn struct_member_access_is_clean() {
    let src = "struct P {\n    x: int\n}\nfun f(p: P) : int {\n    return p.x\n}";
    assert_clean(src);
}

#[test]
fn unknown_struct_member_reports() {
    let src = "struct P {\n    x: int\n}\nfun f(p: P) : int {\n    return p.y\n}";
    assert_has(src, DiagCode::SemUnknownMember);
}

// ---- Own 移动 ----

#[test]
fn own_second_use_reports() {
    let src = "fun main() {\n    val x: Own<int> = 1\n    val y = x\n    val z = x\n}";
    assert_has(src, DiagCode::SemUseAfterMove);
}

#[test]
fn own_single_use_is_not_a_move_error() {
    let src = "fun main() {\n    val x: Own<int> = 1\n    val y = x\n}";
    assert!(!codes(src).contains(&DiagCode::SemUseAfterMove));
}

#[test]
fn own_reassign_resets_moved_state() {
    let src = "fun main() {\n    var x: Own<int> = 1\n    val y = x\n    x = 2\n    val z = x\n}";
    assert!(!codes(src).contains(&DiagCode::SemUseAfterMove));
}

// ---- 宏 / 健壮性 ----

#[test]
fn macro_call_reports_expansion_gap() {
    let src = "fun main() {\n    val v = m!(1)\n}";
    assert_has(src, DiagCode::SemMacroExpansion);
}

#[test]
fn malformed_source_does_not_panic() {
    // 机器层失败或诊断恢复两条路径都不得 panic
    if let Ok(parsed) = parser::parse_source("fun f( {") {
        let out = analyze(parsed);
        let _ = out.diagnostics.len();
    }
}
