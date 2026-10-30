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
    // 注：Own<T> 在 v1 无构造入口，标注初始化自带一条 mismatch；
    // 强断言钉住完整诊断序列（check 在前、moveck 在后）
    let src = "fun main() {\n    val x: Own<int> = 1\n    val y = x\n    val z = x\n}";
    assert_eq!(
        codes(src),
        vec![DiagCode::SemTypeMismatch, DiagCode::SemUseAfterMove]
    );
}

#[test]
fn own_single_use_is_not_a_move_error() {
    let src = "fun main() {\n    val x: Own<int> = 1\n    val y = x\n}";
    assert_eq!(codes(src), vec![DiagCode::SemTypeMismatch]);
}

#[test]
fn own_reassign_resets_moved_state() {
    // 初始化 + 赋值的 Own 标注均自带 mismatch；重置后不再报移动
    let src = "fun main() {\n    var x: Own<int> = 1\n    val y = x\n    x = 2\n    val z = x\n}";
    assert_eq!(
        codes(src),
        vec![DiagCode::SemTypeMismatch, DiagCode::SemTypeMismatch]
    );
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
    // 未知命名实参：SemUnknownParam 单条，不级联 missing argument
    let src = "struct P {\n    x: int\n}\nfun f() : P {\n    return P(z = 1)\n}";
    assert_has(src, DiagCode::SemUnknownParam);
    assert_eq!(codes(src).len(), 1);
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
    assert_eq!(codes(src), vec![DiagCode::SemUseAfterMove]);
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

// ---- 第三轮审查回归：可空提升格、while val 作用域、类型对象 ----

#[test]
fn nullable_lattice_accepts_value_into_nullable() {
    assert_clean("fun f() : int? {\n    return 1\n}");
    assert_clean("fun g(v: int?) {\n}\nfun main() {\n    g(1)\n}");
    assert_clean("fun f() {\n    val x: int? = 1\n}");
}

#[test]
fn nullable_join_produces_nullable() {
    // null ∨ int = int?：x 可接 ?? / !!，且定型确为可空
    let src = "fun f(c: bool) {\n    val x = if c { null } else { 1 }\n    val y = x ?? 0\n}";
    let parsed = parser::parse_source(src).unwrap();
    let out = analyze(parsed);
    assert_eq!(out.diagnostics.len(), 0, "{:?}", out.diagnostics);
    let mut interner = out.interner;
    let sym = interner.intern("x");
    let mut found = false;
    for i in 0..out.defs.len() as u32 {
        let def = out.defs.get(sem::DefId(i));
        if def.name == sym {
            assert!(matches!(out.tys.kind(def.sig.ty), sem::TyKind::Nullable(_)));
            found = true;
        }
    }
    assert!(found, "binding x not found");
    // Nullable(S) ∨ T = Nullable(S ∨ T)
    let src2 =
        "fun f(c: bool, m: int?) {\n    val x = if c { m } else { 1 }\n    val y = x ?? 0\n}";
    assert_clean(src2);
}

#[test]
fn while_val_binding_visible_in_body() {
    let src = "enum Opt {\n    Some(int),\n    None\n}\nfun f(o: Opt) {\n    while val Some(x) = o {\n        val y = x\n    }\n}";
    assert_clean(src);
}

#[test]
fn type_name_is_not_a_value() {
    let src = "struct P {\n    x: int\n}\nfun f() : P {\n    val p = P\n    return p\n}";
    assert_has(src, DiagCode::SemTypeMismatch);
    let src2 = "enum E {\n    A\n}\nfun f() : E {\n    val x = E\n    return x\n}";
    assert_has(src2, DiagCode::SemTypeMismatch);
}

#[test]
fn enum_type_name_call_reports() {
    let src = "enum E {\n    A(int)\n}\nfun main() {\n    E(1)\n}";
    assert_has(src, DiagCode::SemNotCallable);
}

#[test]
fn undefined_callee_reports_single_diagnostic() {
    let src = "fun main() {\n    nosuch()\n}";
    assert_eq!(codes(src), vec![DiagCode::SemUndefinedName]);
}

#[test]
fn variant_named_argument_constructs() {
    let src =
        "enum S {\n    Circle(radius: f64)\n}\nfun f() : S {\n    return S.Circle(radius = 2.0)\n}";
    assert_clean(src);
}

#[test]
fn top_level_destructuring_types_elements() {
    let src = "val (a, b) = (1, 2)\nfun f() : int {\n    return a\n}";
    assert_clean(src);
}

#[test]
fn bare_top_level_binding_reports() {
    assert_has("val x", DiagCode::SemUnsupportedConstruct);
    assert_clean("val x: int");
}

// ---- 第四轮审查回归：in/is、别名、class ----

#[test]
fn in_membership_element_in_collection() {
    assert_clean("fun f(a: [int; 3]) : bool {\n    return 1 in a\n}");
    // 字面量区间默认 i32：元素按同宽型参数验证（int=i64 对 i32 区间
    // 按固定型格合法拒绝，见语义笔记的提升格）
    assert_clean("fun f(x: i32) : bool {\n    return x in 1..5\n}");
    assert_clean("fun f(x: i32) : bool {\n    return x in 1..=5\n}");
    // 元素与集合类型不匹配
    assert_has(
        "fun f(a: [int; 3]) : bool {\n    return \"s\" in a\n}",
        DiagCode::SemTypeMismatch,
    );
    // 反方向（集合 in 元素）拒绝
    assert_has(
        "fun f(a: [int; 3]) : bool {\n    return a in 1\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn if_is_pattern_keeps_then_block() {
    // `if e is A { }`：{ 归 then 块，不被结构体模式吞吃
    let src = "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    if e is A {\n        return 1\n    }\n    return 0\n}";
    assert_clean(src);
}

#[test]
fn alias_name_is_not_a_value() {
    let src = "type M = int\nfun f() : M {\n    return M\n}";
    assert_has(src, DiagCode::SemTypeMismatch);
}

#[test]
fn alias_constructor_reaches_target() {
    let src = "struct P {\n    x: int\n}\ntype Q = P\nfun f() : Q {\n    return Q(1)\n}";
    assert_clean(src);
}

#[test]
fn class_declaration_reports_unsupported() {
    let src = "class C {\n    fun m() : int {\n        return 1\n    }\n}";
    assert_eq!(codes(src), vec![DiagCode::SemUnsupportedConstruct]);
}

#[test]
fn type_object_rejected_in_aggregates() {
    let src =
        "struct P {\n    x: int\n}\nstruct Q {\n    y: int\n}\nfun f() {\n    val a = [P, Q]\n}";
    assert_has(src, DiagCode::SemTypeMismatch);
}

// ---- 第六轮（后端连通后横切）：sem 接受面与 irc 能力对齐 ----

#[test]
fn or_pattern_binds_shared_variable() {
    // `A(x) | B(x)` 的各替代绑定同名同变量，不得报重名
    let src = "enum E {\n    A(int),\n    B(int)\n}\nfun f(e: E) : int {\n    return when e {\n        A(x) | B(x) -> x\n    }\n}";
    assert_clean(src);
}

#[test]
fn when_tuple_subject_destructures_elements() {
    let src = "fun f(t: (int, int)) : int {\n    return when t {\n        (a, b) -> a + b\n        else -> 0\n    }\n}";
    assert_clean(src);
}

#[test]
fn rest_pattern_reports_unsupported() {
    let src = "fun f(a: [int; 3]) {\n    val [x, .., y] = a\n}";
    assert_has(src, DiagCode::SemUnsupportedConstruct);
    let src2 = "fun f(t: (int, int, int)) : int {\n    return when t {\n        (a, .., c) -> a + c\n        else -> 0\n    }\n}";
    assert_has(src2, DiagCode::SemUnsupportedConstruct);
}

#[test]
fn grow_and_concurrent_report_unsupported() {
    assert_has(
        "fun f(x: int) {\n    grow x to 64 bits\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    assert_has(
        "fun main() {\n    concurrent {\n        val x = 1\n    }\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn async_and_await_report_unsupported() {
    assert_has(
        "fun main() {\n    val f = async {\n        1\n    }\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    assert_has(
        "fun main() {\n    val x = await y\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}
