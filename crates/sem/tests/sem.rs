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

// ---- 审查修复回归：循环内 break/continue、异构数组 ----

#[test]
fn break_continue_inside_unlabeled_loops_are_clean() {
    assert_clean("fun main() {\n    loop {\n        break\n    }\n}");
    assert_clean("fun main() {\n    while true {\n        continue\n    }\n}");
    assert_clean("fun f(a: [int; 2]) {\n    for v in a {\n        break\n    }\n}");
    assert_clean("fun main() {\n    'a: loop {\n        break 'a\n    }\n}");
}

#[test]
fn heterogeneous_array_reports_without_panic() {
    // join 失败仍须定型剩余元素（coverage 纪律；debug 下 verify 曾因此 panic）
    assert_has(
        "fun main() {\n    val a = [1, \"s\", 2]\n}",
        DiagCode::SemTypeMismatch,
    );
}

// ---- Or 模式覆盖 ----

#[test]
fn or_pattern_full_coverage_is_clean() {
    let src = "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    return when e {\n        A | B -> 1\n    }\n}";
    assert_clean(src);
    let src2 = "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    return when e {\n        E.A | E.B -> 1\n    }\n}";
    assert_clean(src2);
}

#[test]
fn or_pattern_partial_coverage_reports() {
    let src = "enum E {\n    A,\n    B,\n    C\n}\nfun f(e: E) : int {\n    return when e {\n        A | B -> 1\n    }\n}";
    assert_has(src, DiagCode::SemNonExhaustiveWhen);
}

// ---- 值构造 ----

#[test]
fn enum_variant_value_constructs() {
    let src = "enum E {\n    A,\n    B\n}\nfun f() : E {\n    return E.A\n}";
    assert_clean(src);
    let src2 =
        "enum Opt {\n    Some(int),\n    None\n}\nfun f() : Opt {\n    return Opt.Some(1)\n}";
    assert_clean(src2);
}

#[test]
fn struct_constructor_call_is_clean() {
    let src =
        "struct P {\n    x: int,\n    y: int\n}\nfun f() : P {\n    return P(x = 1, y = 2)\n}";
    assert_clean(src);
    let src2 = "struct P {\n    x: int\n}\nfun f() : P {\n    return P(1)\n}";
    assert_clean(src2);
}

#[test]
fn constructor_unknown_field_reports() {
    let src = "struct P {\n    x: int\n}\nfun f() : P {\n    return P(z = 1)\n}";
    assert_has(src, DiagCode::SemArityMismatch);
}

#[test]
fn variant_constructor_arity_reports() {
    let src =
        "enum Opt {\n    Some(int),\n    None\n}\nfun f() : Opt {\n    return Opt.Some(1, 2)\n}";
    assert_has(src, DiagCode::SemArityMismatch);
}

#[test]
fn function_value_call_is_clean() {
    let src = "fun apply(v: int, f: (int) -> int) : int {\n    return f(v)\n}";
    assert_clean(src);
}

#[test]
fn function_value_arity_reports() {
    let src = "fun apply(f: (int) -> int) : int {\n    return f(1, 2)\n}";
    assert_has(src, DiagCode::SemArityMismatch);
}

// ---- return / unit 门槛 ----

#[test]
fn bare_return_in_non_unit_fun_reports() {
    assert_has("fun f() : int {\n    return\n}", DiagCode::SemTypeMismatch);
}

#[test]
fn bare_return_in_unit_fun_is_clean() {
    assert_clean("fun f() {\n    return\n}");
}

#[test]
fn implicit_unit_fun_tail_is_not_compared() {
    // 未声明返回型按 unit 推断：尾表达式求值但不核查（惯用形状
    // `fun main() { helper() }` 不得假阳性）；显式 `: unit` 仍核查
    assert_clean("fun main() {\n    helper()\n}\nfun helper() { }");
    assert_clean("fun f() {\n    1\n}");
    assert_has("fun f() : unit {\n    1\n}", DiagCode::SemTypeMismatch);
    assert_clean("fun f() {\n}");
}

// ---- Own 参数移动 ----

#[test]
fn own_param_second_use_reports() {
    let src = "fun f(x: Own<int>) : int {\n    val a = x\n    val b = x\n    return 0\n}";
    assert_has(src, DiagCode::SemUseAfterMove);
}

// ---- 推断序与 Unknown 哨兵 ----

#[test]
fn forward_binding_use_gets_diagnostic() {
    // `a` 依赖后声明的 `b`：单遍推断推不出 a 的类型——
    // Unknown 不投毒，钉「无法推定」+ 使用点照常诊断
    let src = "val a = b\nval b = 1\nfun f() : int {\n    return a\n}";
    assert_has(src, DiagCode::SemTypeMismatch);
}

#[test]
fn duplicate_parameter_reports() {
    assert_has(
        "fun f(a: int, a: int) : int {\n    return a\n}",
        DiagCode::SemDuplicateDef,
    );
}

#[test]
fn generic_fun_reports_single_diagnostic() {
    let parsed = parser::parse_source("fun id<T>(v: T) : T {\n    return v\n}").unwrap();
    let out = analyze(parsed);
    assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(out.diagnostics[0].code, DiagCode::SemUnsupportedConstruct);
}

// ---- 序比较限定 ----

#[test]
fn string_ordering_reports() {
    assert_has(
        "fun f() {\n    val b = \"a\" < \"b\"\n}",
        DiagCode::SemTypeMismatch,
    );
    assert_clean("fun f(a: int, b: int) {\n    val c = a < b\n}");
}

// ---- 循环/条件绑定不可变 ----

#[test]
fn for_loop_variable_is_immutable() {
    assert_has(
        "fun f(a: [int; 2]) {\n    for x in a {\n        x = 1\n    }\n}",
        DiagCode::SemAssignToVal,
    );
}

// ---- span 断言 ----

#[test]
fn diagnostic_span_is_pinned() {
    use ast::token::Span;
    let src = "fun main() {\n    val x = 1\n    x = 2\n}";
    let parsed = parser::parse_source(src).unwrap();
    let out = analyze(parsed);
    // 诊断定位在赋值目标标识符本身（x），非整条语句
    let start = src.find("x = 2").unwrap() as u32;
    assert!(
        out.diagnostics.iter().any(|d| {
            d.code == DiagCode::SemAssignToVal
                && d.span
                    == Span {
                        start,
                        end: start + 1,
                    }
        }),
        "span drift: {:?}",
        out.diagnostics
    );
}

#[test]
fn undefined_name_span_is_pinned() {
    use ast::token::Span;
    let src = "fun main() {\n    nosuch()\n}";
    let parsed = parser::parse_source(src).unwrap();
    let out = analyze(parsed);
    let start = src.find("nosuch").unwrap() as u32;
    assert!(
        out.diagnostics.iter().any(|d| {
            d.code == DiagCode::SemUndefinedName
                && d.span
                    == Span {
                        start,
                        end: start + 6,
                    }
        }),
        "span drift: {:?}",
        out.diagnostics
    );
}

// ---- 健壮性 ----

#[test]
fn malformed_sources_do_not_panic() {
    for src in [
        "fun f( {",
        "val = ",
        "fun f() {\n    if {\n}",
        "enum {\n}",
        "fun f() : {\n}",
    ] {
        if let Ok(parsed) = parser::parse_source(src) {
            let out = analyze(parsed);
            let _ = out.diagnostics.len();
        }
    }
}
