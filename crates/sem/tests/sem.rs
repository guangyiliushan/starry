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
fn print_builtin_accepts_scalars_and_rejects_aggregates() {
    assert_clean("fun main() {\n    print(42)\n}");
    assert_clean("fun main() {\n    print(1.5)\n}");
    assert_clean("fun main() {\n    print(true)\n}");
    assert_clean("fun main() {\n    print(\"hello\")\n}");
    assert_has("fun main() {\n    print()\n}", DiagCode::SemArityMismatch);
    assert_has(
        "fun main() {\n    print([1, 2])\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn string_plus_concat_is_clean() {
    assert_clean("fun f(): string {\n    return \"a\" + \"b\"\n}");
    assert_has(
        "fun f(): string {\n    return \"a\" + 1\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn top_level_scalar_globals_are_clean() {
    assert_clean("val G = 40\nvar H: int\nfun main() {\n    H = G + 2\n    print(H)\n}");
}

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
fn generic_fun_decl_without_instantiation_is_clean() {
    // M7：泛型 fun 声明本身不再拒收；原件不检查（无 bounds 可依），
    // 检查发生在实例化克隆上——未实例化 = 无诊断（限制见 semantic-notes）
    let parsed = parser::parse_source("fun id<T>(v: T) : T {\n    return v\n}").unwrap();
    let out = analyze(parsed);
    assert_eq!(out.diagnostics.len(), 0, "{:?}", out.diagnostics);
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
fn class_declaration_with_method_is_clean() {
    // M4 slice 2b: class + method decl accepted (static dispatch)
    let src = "class C {\n    fun m() : int {\n        return 1\n    }\n}";
    assert_clean(src);
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
fn rest_pattern_destructuring_is_clean_and_checked() {
    assert_clean(
        "fun f(a: [int; 3]) {
    val [x, .., y] = a
}",
    );
    assert_clean(
        "fun f(t: (int, int, int)) : int {
    return when t {
        (a, .., c) -> a + c
        else -> 0
    }
}",
    );
    assert_has(
        "fun f(a: [int; 2]) {
    val [x, .., y, z] = a
}",
        DiagCode::SemArityMismatch,
    );
}

#[test]
fn loop_expression_typed_from_break_values() {
    assert_clean(
        "fun f() : int {
    val x = loop {
        break 7
    }
    return x
}",
    );
    assert_clean(
        "fun f(c: bool) : int {
    val x = loop {
        if c {
            break 1
        } else {
            break 2
        }
    }
    return x
}",
    );
    assert_has(
        "fun f(c: bool) : int {
    val x = loop {
        if c {
            break
        } else {
            break 1
        }
    }
    return x
}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn tuple_subscript_requires_constant_in_bounds_index() {
    assert_clean(
        "fun f() : int {
    val t = (1, 2)
    return t[1]
}",
    );
    assert_clean(
        "fun f() : int {
    var t = (1, 2)
    t[1] = 9
    return t[1]
}",
    );
    assert_has(
        "fun f() : int {
    val t = (1, 2)
    return t[2]
}",
        DiagCode::SemTypeMismatch,
    );
    assert_has(
        "fun f(i: int) : int {
    val t = (1, 2)
    return t[i]
}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn definite_assignment_ignores_returning_branch() {
    assert_clean(
        "fun f(c: bool): int {\n    var x: int\n    if c {\n        return 1\n    } else {\n        x = 2\n    }\n    return x\n}",
    );
}

#[test]
fn function_can_reference_later_initialized_global() {
    assert_clean(
        "fun make(): int {\n    return B\n}\nval B: int = 2\nfun main() {\n    print(make())\n}",
    );
}

#[test]
fn uninitialized_local_read_reports() {
    assert_has(
        "fun f() : int {
    var x: int
    return x
}",
        DiagCode::SemUninitializedBinding,
    );
}

#[test]
fn definite_assignment_paths_are_checked() {
    assert_clean(
        "fun f(c: bool) : int {
    var x: int
    if c {
        x = 1
    } else {
        x = 2
    }
    return x
}",
    );
    assert_has(
        "fun f(c: bool) : int {
    var x: int
    if c {
        x = 1
    }
    return x
}",
        DiagCode::SemUninitializedBinding,
    );
    assert_clean(
        "fun f() : int {
    var x: int
    x = 1
    return x
}",
    );
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

// ---- 第七轮回归：穷尽性、聚合格、别名归一、诊断文案 ----

#[test]
fn when_irrefutable_patterns_are_exhaustive() {
    assert_clean("fun f(x: int) : int {\n    return when x {\n        _ -> 0\n    }\n}");
    assert_clean(
        "fun f(p: P) : int {\n    return when p {\n        P { x, y } -> x\n    }\n}\nstruct P {\n    x: int,\n    y: int\n}",
    );
    assert_clean(
        "fun f(t: (int, int)) : int {\n    return when t {\n        (a, b) -> a + b\n    }\n}",
    );
    assert_clean("fun f(a: [int; 2]) : int {\n    return when a {\n        [x, y] -> x\n    }\n}");
    // 枚举主语上的通配同样穷尽
    assert_clean(
        "enum E {\n    A,\n    B\n}\nfun f(e: E) : int {\n    return when e {\n        _ -> 0\n    }\n}",
    );
    // 可驳模式仍需 else
    assert_has(
        "fun f(x: int) : int {\n    return when x {\n        1 -> 1\n    }\n}",
        DiagCode::SemMissingElse,
    );
}

#[test]
fn array_and_range_elementwise_assignable() {
    assert_clean("fun f() {\n    val a: [int; 3] = [1, 2, 3]\n}");
    assert_clean("fun f() {\n    val a: [f64; 2] = [1, 2]\n}");
    assert_clean("fun g(a: [int; 3]) {\n}\nfun f() {\n    g([1, 2, 3])\n}");
    assert_clean("fun f(x: int) : bool {\n    return x in 1..5\n}");
    // 收窄方向仍拒绝
    assert_has(
        "fun f() {\n    val a: [int; 2] = [1.5, 2.5]\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn alias_normalizes_to_target_def() {
    let src = "enum E {\n    A,\n    B\n}\ntype F = E\nfun f() : E {\n    return F.A\n}";
    assert_clean(src);
    let src2 = "enum Opt {\n    Some(int),\n    None\n}\ntype O = Opt\nfun f() : Opt {\n    return O.Some(1)\n}";
    assert_clean(src2);
    let src3 =
        "enum E {\n    A,\n    B\n}\ntype F = E\ntype G = F\nfun f() : E {\n    return G.A\n}";
    assert_clean(src3);
    let src4 = "enum E {\n    A,\n    B\n}\ntype F = E\nfun f(e: E) : int {\n    return when e {\n        F.A -> 1\n        F.B -> 2\n    }\n}";
    assert_clean(src4);
    // 类型对象同型聚合仍拒绝
    let src5 = "struct P {\n    x: int\n}\ntype Q = P\nfun f() {\n    val a = [P, Q]\n}";
    assert_has(src5, DiagCode::SemTypeMismatch);
}

#[test]
fn method_call_reports_method_not_unknown_field() {
    let src = "struct P {\n    x: int\n}\nfun f(p: P) {\n    p.m()\n}";
    assert_eq!(codes(src), vec![DiagCode::SemUnsupportedConstruct]);
}

#[test]
fn self_reports_dedicated_diagnostic() {
    let src = "fun f() : int {\n    return self\n}";
    assert_eq!(codes(src), vec![DiagCode::SemSelfOutsideMember]);
}

#[test]
fn class_use_site_ctor_is_clean() {
    // M4 slice 1: class ctor call accepted
    let src = "class C {\n}\nfun main() {\n    C()\n}";
    assert_clean(src);
}

// ---- 2026-11 审查修复回归 ----

#[test]
fn default_argument_types_are_checked_and_bound() {
    assert_clean("fun f(x: int = 1) : int = x\nfun main() { print(f()) }");
    assert_clean("fun g() : int = 7\nfun f(x: int = g()) : int = x\nfun main() { print(f()) }");
    assert_has(
        "fun f(x: int = \"oops\") : int = x",
        DiagCode::SemTypeMismatch,
    );
    // 默认值引用未定义名：resolve 期进入默认表达式后必须报错
    assert_has(
        "fun f(x: int = nosuch) : int = x",
        DiagCode::SemUndefinedName,
    );
    // 默认值不能引用形参（v1 无调用点绑定语义）
    assert_has(
        "fun f(y: int = 1, x: int = y) : int = x",
        DiagCode::SemUndefinedName,
    );
}

#[test]
fn if_val_constructor_and_for_tuple_bindings() {
    assert_clean(
        "enum Opt { None, Some(int) }\n\
         fun unwrap(o: Opt) : int {\n\
             if val Some(v) = o { return v }\n\
             return 0\n\
         }\n",
    );
    assert_clean(
        "fun main() {\n\
             val ps: [(int, int); 2] = [(1, 2), (3, 4)]\n\
             for (a, b) in ps { print(a + b) }\n\
         }",
    );
}

#[test]
fn immutable_aggregate_members_are_not_assignable() {
    assert_has(
        "struct P { x: int }\n\
         fun main() {\n\
             val p = P(1)\n\
             p.x = 2\n\
         }",
        DiagCode::SemAssignToVal,
    );
    assert_clean(
        "struct P { x: int }\n\
         fun main() {\n\
             var p = P(1)\n\
             p.x = 2\n\
         }",
    );
}

#[test]
fn recursive_types_are_rejected() {
    assert_has("struct N { next: N }", DiagCode::SemTypeCycle);
    assert_has(
        "struct A { b: B }\nstruct B { a: A }",
        DiagCode::SemTypeCycle,
    );
    // 非递归的相互可见结构必须是干净的
    assert_clean("struct A { x: int }\nstruct B { a: A }");
}

#[test]
fn large_integer_literals_widen_instead_of_wrapping() {
    // 字面量按值取最小可容纳类型：i32 → i64 → i128
    assert_clean("fun main() { print(3000000000) }");
}

#[test]
fn extern_c_i128_scalar_abi_reports() {
    // i128 标量跨 C ABI 在本目标上与 IR i128 传参不一致：extern 声明点拒收
    assert_has(
        "extern \"c\" fun big(x: i128) : i128",
        DiagCode::SemUnsupportedConstruct,
    );
    assert_has(
        "extern \"c\" fun wide() : i128",
        DiagCode::SemUnsupportedConstruct,
    );
    // 别名经 lower 归一为 Int(I128)，同样命中
    assert_has(
        "type Big = i128\nextern \"c\" fun f(x: Big) : int",
        DiagCode::SemUnsupportedConstruct,
    );
    // i64 形参与含 i128 成员的聚合（按指针传递）不受影响
    assert_clean("extern \"c\" fun ok(x: i64) : i64");
    assert_clean("struct W {\n    v: i128\n}\nextern \"c\" fun agg(w: W) : int");
}

#[test]
fn deep_type_chain_does_not_overflow() {
    // 回归：递归类型检测原为递归 DFS，数万级字段链在 2 MiB 测试线程
    // 栈上爆栈；显式栈迭代应稳过（无环 → 零诊断）
    let n = 20_000;
    let mut src = String::new();
    for i in 0..n {
        if i + 1 < n {
            src.push_str(&format!("struct S{i} {{\n    next: S{}\n}}\n", i + 1));
        } else {
            src.push_str(&format!("struct S{i} {{\n    next: int\n}}\n"));
        }
    }
    let parsed = parser::parse_source(&src).expect("machine parse");
    let out = analyze(parsed);
    assert_eq!(out.diagnostics.len(), 0, "{:?}", out.diagnostics.first());
}
#[test]
fn block_tail_loop_statement_yields_its_break_value() {
    // 审查 P0-4 run-pass 抓出的 miscompile：`loop { break 7 }` 曾是
    // LoopStatement（块无尾式），带值 break 被静默丢弃、函数落到
    // unreachable。现块尾 Loop 归一为 LoopExpr（parser-notes），值即
    // 块的值；裸 break 的 loop 在值位置按 unit 处理（Rust 同纪律）。
    assert_clean(
        "fun pick() : int {
    loop {
        break 7
    }
}",
    );
    assert_clean(
        "fun find() : int {
    'outer: loop {
        break 'outer 42
    }
}",
    );
    assert_clean(
        "fun main() {
    loop {
        break
    }
}",
    );
    // 混用（部分带值）仍是类型错误
    assert_has(
        "fun f(c: bool) : int {
    loop {
        if c {
            break
        }
        break 1
    }
}",
        DiagCode::SemTypeMismatch,
    );
}
#[test]
fn integer_literal_typing_boundary_table() {
    // M1 数字语义：未约束整数字面量默认 i64；约束上下文（标注/形参/
    // 返回）按目标宽度定型，值域按目标宽度诊断（IntegerOutOfRange）。
    let i32_max = i32::MAX as i64;
    let i64_max = i64::MAX;
    // 未约束 → i64（= int）：边界值干净
    assert_clean(&format!("fun f() : int {{ return {i32_max} }}"));
    assert_clean(&format!("fun f() : int {{ return {i64_max} }}"));
    // 超 i64 → i128（需 i128 返回型才接得住）
    assert_clean(&format!(
        "fun f() : i128 {{ return {} }}",
        i64_max as i128 + 1
    ));
    // 超 i128 拒收
    assert_has(
        "fun f() : int {\n    return 170141183460469231731687303715884105728\n}",
        DiagCode::IntegerOutOfRange,
    );
    // 上下文定型：标注/形参/返回把字面量钉到目标宽度
    assert_clean("fun f() : int {\n    val x: i32 = 1\n    return x\n}");
    assert_clean("fun g(x: i32) : int {\n    return x\n}\nfun f() : int {\n    return g(1)\n}");
    assert_clean("fun f() : i32 {\n    return 5\n}");
    // 值域按目标宽度诊断
    assert_has(
        "fun f() : int {\n    val x: i32 = 3000000000\n    return x\n}",
        DiagCode::IntegerOutOfRange,
    );
    assert_has(
        "fun f() : i32 {\n    return 2147483648\n}",
        DiagCode::IntegerOutOfRange,
    );
    // radix/下划线同样按值定型
    assert_clean("fun f() : int {\n    return 0x1_FFFF_FFFF\n}"); // 2^33-1 默认 i64
    assert_clean("fun f() {\n    val x: i32 = 0b1111111111111111111111111111111\n}"); // 31 个 1，上下文 i32 容纳
    assert_has(
        "fun f() {\n    val x: i32 = 0b1_1111_1111_1111_1111_1111_1111_1111_1111\n}",
        DiagCode::IntegerOutOfRange, // 2^32-1 超 i32 目标宽度
    );
}

#[test]
fn pointer_types_parse_and_type_in_annotations() {
    // D-007 / M2 类型语法层：前缀构造子链（? *const *mut & &mut）；
    // bare *T 诊断；复合后缀 ? 拒绝并提示前缀形态。
    // 引用/指针只做类型+FFI（取址/解引表达式在下一 slice）
    assert_clean("fun f(p: *const int) : int {\n    return 0\n}");
    assert_clean("fun f(p: *mut int) : int {\n    return 0\n}");
    // 可空指针 = 前缀 ?*const T（D-007：?*const T = ?( *const T)）
    assert_clean("fun f(p: ?*const int) : int {\n    return 0\n}");
    // 指向可空的指针：*const ?int 与 *const int? 同一 AST（? 归内层原子糖）
    assert_clean("fun f(p: *const ?int) : int {\n    return 0\n}");
    assert_clean("fun f(p: *const int?) : int {\n    return 0\n}");
    // 借用类型（T1）：&T / &mut T / ?&T / &?T
    assert_clean("fun f(p: &int) : int {\n    return 0\n}");
    assert_clean("fun f(p: &mut int) : int {\n    return 0\n}");
    assert_clean("fun f(p: ?&int) : int {\n    return 0\n}");
    // bare *T 诊断（只允许 *const / *mut）
    assert_has(
        "fun f(p: *int) : int {\n    return 0\n}",
        DiagCode::ExpectedToken,
    );
    // 复合类型后缀 ? 拒绝 + 前缀提示
    assert_has(
        "fun f(p: [int; 2]?) : int {\n    return 0\n}",
        DiagCode::UnexpectedToken,
    );
    assert_has(
        "fun f(p: (int, int)?) : int {\n    return 0\n}",
        DiagCode::UnexpectedToken,
    );
    // null 可提升入裸指针；引用非空（null 不可指派）
    assert_clean("fun f() : *const int {\n    return null\n}");
    assert_has(
        "fun f() : &int {\n    return null\n}",
        DiagCode::SemTypeMismatch,
    );
    // mutability 参与格：*const ≠ *mut、& ≠ &mut（v1 不做协变放宽）
    assert_has(
        "fun g(p: *mut int) : *const int {\n    return p\n}",
        DiagCode::SemTypeMismatch,
    );
    assert_has(
        "fun g(p: &mut int) : &int {\n    return p\n}",
        DiagCode::SemTypeMismatch,
    );
    assert_clean("extern \"c\" fun c_log(p: *const int, n: int) : int");
}
#[test]
fn borrow_and_deref_expressions_type_and_diagnose() {
    // M2 slice 2：&x / &mut x / *p 表达式 + T1 保守借用规则
    assert_clean("fun f() : int {\n    var x = 1\n    val p = &x\n    return *p\n}");
    assert_clean("fun f() : int {\n    var x = 1\n    val p = &mut x\n    *p = 2\n    return x\n}");
    // 借用的格：&mut 不可当 & 用
    assert_has(
        "fun g(p: &mut int) : int {\n    return 0\n}\nfun f() : int {\n    var x = 1\n    return g(&x)\n}",
        DiagCode::SemTypeMismatch,
    );
    // 借用目标限局部名：复合表达式目标 v1 拒绝
    assert_has(
        "fun f() : int {\n    var x = 1\n    val p = &(x + 1)\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // 顶层全局借用 = 逃逸，v1 拒绝
    assert_has(
        "val G: int = 1\nfun f() : int {\n    val p = &G\n    return *p\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // &mut 不可变绑定拒绝
    assert_has(
        "fun f() : int {\n    val x = 1\n    val p = &mut x\n    return 0\n}",
        DiagCode::SemAssignToVal,
    );
    // 解引非指针/引用
    assert_has(
        "fun f() : int {\n    val x = 1\n    return *x\n}",
        DiagCode::SemTypeMismatch,
    );
    // 经不可变引用/指针赋值拒绝
    assert_has(
        "fun f() : int {\n    var x = 1\n    val p = &x\n    *p = 2\n    return x\n}",
        DiagCode::SemAssignToVal,
    );
    // 非逃逸：struct 引用字段 / 返回引用，v1 拒绝
    assert_has(
        "struct S {\n    r: &int\n}\nfun f() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    assert_has(
        "fun f(x: &int) : &int {\n    return x\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}
#[test]
fn primary_constructor_and_field_mutability() {
    // M3：struct 主构造（val/var 参数声明字段，无修饰符参数仅构造期）
    // + 字段可变性三态（Some(true)/Some(false)/None=legacy）
    assert_clean(
        "struct Point(val x: int, var y: int)\nfun f() : int {\n    val p = Point(1, 2)\n    p.y = 9\n    return p.x + p.y\n}",
    );
    // val 字段恒不可写（哪怕 var 根绑定）
    assert_has(
        "struct Point(val x: int, var y: int)\nfun f() : int {\n    var p = Point(1, 2)\n    p.x = 5\n    return 0\n}",
        DiagCode::SemAssignToVal,
    );
    // var 字段可写——val 根绑定也拦不住（深度可变性，Swift 模型）
    assert_clean(
        "struct Point(val x: int, var y: int)\nfun f() : int {\n    val p = Point(1, 2)\n    p.y = 9\n    return p.y\n}",
    );
    // legacy 无修饰符字段：按根绑定（兼容旧语义）
    assert_clean(
        "struct P {\n    x: int\n}\nfun f() : int {\n    var p = P(1)\n    p.x = 50\n    return p.x\n}",
    );
    assert_has(
        "struct P {\n    x: int\n}\nfun f() : int {\n    val p = P(1)\n    p.x = 50\n    return 0\n}",
        DiagCode::SemAssignToVal,
    );
    // body 显式 var 字段：val 根绑定可写
    assert_clean(
        "struct P {\n    var x: int\n}\nfun f() : int {\n    val p = P(1)\n    p.x = 50\n    return p.x\n}",
    );
    // 命名/默认/仅构造参数
    assert_clean("struct R(val x: int = 5)\nfun f() : int {\n    return R().x\n}");
    assert_clean("struct C(val x: int, noisy: int)\nfun f() : int {\n    return C(1, 2).x\n}");
    // 头字段与 body 字段重名 → 重复定义
    assert_has(
        "struct P(val x: int) {\n    x: int\n}\nfun f() : int {\n    return 0\n}",
        DiagCode::SemDuplicateDef,
    );
    // 默认值类型不符
    assert_has(
        "struct R(val x: int = \"s\")\nfun f() : int {\n    return R().x\n}",
        DiagCode::SemTypeMismatch,
    );
}

// ---- M6：extern "c" 调用的 unsafe 边界 ----

#[test]
fn extern_calls_require_unsafe_context() {
    // FFI 永远 unchecked：unsafe 块外调用 extern 报错
    assert_has(
        "extern \"c\" fun c_log(p: *const int) : int\nfun f() : int {\n    return c_log(null)\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // unsafe 块内允许
    assert_clean(
        "extern \"c\" fun c_log(p: *const int) : int\nfun f() : int {\n    return unsafe { c_log(null) }\n}",
    );
    // unsafe 内的嵌套普通块继承许可（深度计数贯穿块嵌套）
    assert_clean(
        "extern \"c\" fun c_log(p: *const int) : int\nfun f() : int {\n    return unsafe {\n        val x = 1\n        c_log(null)\n    }\n}",
    );
    // 离开 unsafe 后深度归零，再调用仍报错
    assert_has(
        "extern \"c\" fun c_log(p: *const int) : int\nfun f() : int {\n    val a = unsafe { 1 }\n    return c_log(null)\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // 纯声明不调用 → 无诊断
    assert_clean("extern \"c\" fun c_log(p: *const int) : int\nfun f() : int {\n    return 0\n}");
}

// ---- M7：泛型单态化 ----

#[test]
fn generic_fun_instantiation_types_and_repeats() {
    // 同一泛型多个实例化：id<int> / id<f64> 各自克隆检查，互不串味
    assert_clean(
        "fun id<T>(v: T) : T {\n    return v\n}\nfun main() : int {\n    val a = id<int>(1)\n    val b = id<f64>(1.5)\n    return a\n}",
    );
    // 实例化实参类型不符：代入后形参是 int，string 拒
    assert_has(
        "fun id<T>(v: T) : T {\n    return v\n}\nfun main() : int {\n    return id<int>(\"s\")\n}",
        DiagCode::SemTypeMismatch,
    );
    // 体内错误在实例化点报告（原件不检查）
    assert_has(
        "fun bad<T>(v: T) : int {\n    return v + \"s\"\n}\nfun main() : int {\n    return bad<int>(1)\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn generic_fun_type_args_are_required_and_counted() {
    // M7 slice 2：缺类型实参改为调用点推断（id(1) 推断 T=int）
    assert_clean("fun id<T>(v: T) : T {\n    return v\n}\nfun main() : int {\n    return id(1)\n}");
    // 个数不符
    assert_has(
        "fun id<T>(v: T) : T {\n    return v\n}\nfun main() : int {\n    return id<int, int>(1)\n}",
        DiagCode::SemTypeArgMismatch,
    );
    // 非泛型函数携带类型实参
    assert_has(
        "fun f(v: int) : int {\n    return v\n}\nfun main() : int {\n    return f<int>(1)\n}",
        DiagCode::SemTypeArgMismatch,
    );
}

#[test]
fn generic_fun_recursion_via_explicit_args_is_bounded() {
    // 自递归 f<T>(x) 命中已注册实例 → 有界；同时覆盖嵌套调用链
    assert_clean(
        "fun f<T>(v: T) : T {\n    return f<T>(v)\n}\nfun main() : int {\n    return f<int>(1)\n}",
    );
}

#[test]
fn generic_struct_construction_fields_and_mutability() {
    assert_clean(
        "struct Box<T> {\n    v: T\n}\nfun main() : int {\n    val b = Box<int>(1)\n    return b.v\n}",
    );
    // 标注 + 构造一致（同一 spec def）
    assert_clean(
        "struct Box<T> {\n    v: T\n}\nfun main() : int {\n    val b: Box<int> = Box<int>(2)\n    return b.v\n}",
    );
    // 元素类型不同 = 不同类型：Box<int> 不能赋给 Box<f64>
    assert_has(
        "struct Box<T> {\n    v: T\n}\nfun main() : int {\n    val b: Box<f64> = Box<int>(2)\n    return 0\n}",
        DiagCode::SemTypeMismatch,
    );
    // 类型实参个数不符
    assert_has(
        "struct Box<T> {\n    v: T\n}\nfun main() : int {\n    val b = Box<int, int>(2)\n    return 0\n}",
        DiagCode::SemTypeArgMismatch,
    );
    // M3 可变性三态随 spec 字段走：显式 val 字段恒拒
    assert_has(
        "struct B<T>(val v: T)\nfun main() : int {\n    var b = B<int>(1)\n    b.v = 2\n    return 0\n}",
        DiagCode::SemAssignToVal,
    );
}

#[test]
fn generic_struct_nested_and_generic_fun_mixed() {
    // A<T> 的字段引用 B<T>：实例化 A<int> 须把 B<int> 也落成 spec def
    assert_clean(
        "struct B<T> {\n    v: T\n}\nstruct A<T> {\n    b: B<T>\n}\nfun main() : int {\n    val a = A<int>(B<int>(7))\n    return a.b.v\n}",
    );
    // 泛型 fun 形参引用泛型 struct：调用点代入后构造核查
    assert_clean(
        "struct Box<T> {\n    v: T\n}\nfun unwrap<T>(b: Box<T>) : T {\n    return b.v\n}\nfun main() : int {\n    return unwrap<int>(Box<int>(3))\n}",
    );
}

#[test]
fn generic_rejections_are_stable() {
    // 泛型 main：入口无法实例化
    assert_has(
        "fun main<T>() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // M9 9c：类型形参 bounds 已落地——声明放行；实例化违例拒收
    assert_clean(
        "trait Eq {\n    fun eq(o: Self): bool\n}\nfun f<T : Eq>(v: T) : int {\n    return 0\n}\nfun main() : int {\n    return 0\n}",
    );
    assert_has(
        "trait Eq {\n    fun eq(o: Self): bool\n}\nstruct Plain(val v: int)\nfun f<T : Eq>(v: T) : int {\n    return 0\n}\nfun main() : int {\n    val p = Plain(1)\n    return f(p)\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // M7 slice 2：泛型 enum 已落地（声明+实例化+变体构造推断）
    assert_clean(
        "enum E<T> {\n    A\n    B(T)\n}\nfun main() : int {\n    val a = E.A\n    val b = E.B(1)\n    return 0\n}",
    );
    // 泛型 struct 主构造默认值
    assert_has(
        "struct R<T>(val v: T = 1)\nfun main() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // 泛型 struct 模式（struct 模式只在 val 绑定位置可解析：`is` 后
    // 的 { 归 then 块——parser 消歧纪律；这里走 refine 的 Struct 臂拒收）
    assert_has(
        "struct Box<T> {\n    v: T\n}\nfun main() : int {\n    val b = Box<int>(1)\n    val Box { v } = b\n    return v\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

// ---- M1（Q5）：checked 默认溢出 + 显式 wrapping ----

#[test]
fn checked_const_overflow_and_wrapping_operators() {
    // D3：可证明的常量溢出钉独立码 SemConstOverflow（不复用 IntegerOutOfRange）
    assert_has(
        "fun f() : int {\n    return 9223372036854775807 + 1\n}",
        DiagCode::SemConstOverflow,
    );
    // 一元负参与常量链：-MAX - 2 越出 i64
    assert_has(
        "fun f() : int {\n    return -9223372036854775807 - 2\n}",
        DiagCode::SemConstOverflow,
    );
    // 乘法常量溢出
    assert_has(
        "fun f() : int {\n    return 9223372036854775807 * 2\n}",
        DiagCode::SemConstOverflow,
    );
    // D4：常量除零编译期诊断（独立码）
    assert_has(
        "fun f(x: int): int {\n    return x + 1 / 0\n}",
        DiagCode::SemDivisionByZero,
    );
    // D3：二元字面量上下文传播——i32 标注使常量加法在 i32 域核查
    assert_has(
        "fun f() : int {\n    val x: i32 = 2147483647 + 1\n    return x as int\n}",
        DiagCode::SemConstOverflow,
    );
    assert_clean("fun f() : int {\n    val x: i32 = 2147483646 + 1\n    return x as int\n}");
    // return 上下文传播：声明 i32 返回型
    assert_has(
        "fun f() : i32 {\n    return 2147483647 + 1\n}",
        DiagCode::SemConstOverflow,
    );
    // 界内常量干净；wrapping 算子不诊断
    assert_clean("fun f() : int {\n    return 9223372036854775806 + 1\n}");
    assert_clean("fun f() : int {\n    return 9223372036854775807 +% 1\n}");
    assert_clean("fun f(x: int) : int {\n    return x *% 3\n}");
    // wrapping 仅整型：字符串连接不吃 +%
    assert_has(
        "fun f() : int {\n    return \"a\" +% \"b\"\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn class_slice1_rejections_are_stable() {
    // reference equality (slice 2a: pointer compare)
    assert_clean(
        "class P(val x: int)\nfun main() : int {\n    val a = P(1)\n    val b = P(1)\n    if a == b {\n        return 1\n    }\n    return 0\n}",
    );
    // class vs non-class equality still rejected
    assert_has(
        "class P(val x: int)\nfun main() : int {\n    val a = P(1)\n    if a == 1 {\n        return 1\n    }\n    return 0\n}",
        DiagCode::SemTypeMismatch,
    );
    // M4 slice 4a: nullable class accepted (flag + payload slots)
    assert_clean("class P(val x: int)\nfun main() : int {\n    val a: P? = null\n    return 0\n}");
    // M4 slice 4c: class params/returns accepted (+0/+1)
    assert_clean("class P(val x: int)\nfun f(p: P) : int {\n    return p.x\n}");
    assert_clean("class P(val x: int)\nfun f() : P {\n    return P(1)\n}");
    // i128 class field (payload slot model limit)
    assert_has(
        "class P(val x: i128)\nfun main() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // aggregate class field
    assert_has(
        "struct S {\n    v: int\n}\nclass P(val s: S)\nfun main() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // primary-ctor default value
    assert_has(
        "class P(val x: int = 1)\nfun main() : int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn class_methods_and_deinit_slice2b() {
    // static dispatch + deinit + shared mutation
    assert_clean(
        "class Counter(var count: int) {\n    fun bump() : int {\n        self.count += 1\n        return self.count\n    }\n}\nfun main() : int {\n    val a = Counter(10)\n    val b = a\n    return b.bump() + a.bump()\n}",
    );
    // method arg type checking
    assert_has(
        "class C {\n    fun m(x: int) : int {\n        return x\n    }\n}\nfun main() : int {\n    val c = C()\n    return c.m(\"s\")\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn nullable_class_slice4a_semantics() {
    // lift/null/==/!!/?. accepted
    assert_clean(
        "class P(val x: int)\nfun main() : int {\n    val a = P(1)\n    val pa: P? = a\n    val pn: P? = null\n    if (pa != null) {\n        print(pa!!.x)\n    }\n    if (pn == null) {\n        print(0)\n    }\n    val v: int? = pa?.x\n    return v!!\n}",
    );
    // M4 slice 4c: ?P params/returns accepted (+0/+1)
    assert_clean(
        "class P(val x: int)\nfun f(p: P?) : int {\n    return 1\n}\nfun g() : P? {\n    return null\n}\nfun main() : int {\n    return 0\n}",
    );
}

#[test]
fn weak_var_slice3_semantics() {
    // weak var init accepted
    assert_clean(
        "class N(var v: int) {\n    deinit {\n        print(self.v)\n    }\n}\nfun main() {\n    val a = N(1)\n    weak var wa = a\n}",
    );
    // weak read rejected (upgrade semantics not scheduled)
    assert_has(
        "class N(var v: int) {\n}\nfun main() : int {\n    val a = N(1)\n    weak var wa = a\n    return wa.v\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // weak val rejected (must be var)
    assert_has(
        "class N(var v: int) {\n}\nfun main() {\n    val a = N(1)\n    weak val wa = a\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // weak of non-class rejected
    assert_has(
        "fun main() {\n    val a = 1\n    weak var wa = a\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn class_reassignment_slice4d_semantics() {
    // plain reassignment, cross-scope borrowed rhs, self-assign all accepted
    assert_clean(
        "class P(val x: int) {\n}\nfun main() : int {\n    var b = P(1)\n    if (true) {\n        val a = P(2)\n        b = a\n    }\n    b = P(3)\n    b = b\n    return b.x\n}",
    );
    // nullable var reassignment chains
    assert_clean(
        "class P(val x: int) {\n}\nfun main() : int {\n    var b: P? = null\n    b = P(1)\n    b = null\n    return 0\n}",
    );
}

#[test]
fn weak_upgrade_slice4b_semantics() {
    // weak read = upgrade to ?P: !!/?. /?? /binding all accepted
    assert_clean(
        "class N(var v: int) {\n}\nfun main() : int {\n    val a = N(1)\n    weak var w = a\n    val u = w\n    if (u != null) {\n        print(u!!.v)\n    }\n    print(w!!.v)\n    val x: int? = w?.v\n    print(x!!)\n    val fb = w ?? a\n    print(fb.v)\n    return 0\n}",
    );
    // ctor-initialized weak var accepted (releases at statement end)
    assert_clean("class N(var v: int) {\n}\nfun main() {\n    weak var w = N(2)\n}");
    // non-safe member through weak still rejected (use ?. or !!)
    assert_has(
        "class N(var v: int) {\n}\nfun main() : int {\n    val a = N(1)\n    weak var w = a\n    return w.v\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // weak reassignment still rejected
    assert_has(
        "class N(var v: int) {\n}\nfun main() {\n    val a = N(1)\n    val b = N(2)\n    weak var w = a\n    w = b\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn visibility_slice3_semantics() {
    // default public: same-package and cross-package import both fine
    assert_clean(
        "package util\nfun f(): int {\n    return 1\n}\npackage app\nimport util.f\nfun main(): int {\n    return f()\n}",
    );
    // internal: same-package use fine
    assert_clean(
        "package util\ninternal fun g(): int {\n    return 2\n}\nfun useit(): int {\n    return g()\n}\npackage app\nfun main(): int {\n    return 0\n}",
    );
    // internal: cross-package import rejected
    assert_has(
        "package util\ninternal fun g(): int {\n    return 2\n}\npackage app\nimport util.g\nfun main(): int {\n    return g()\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // private: same-segment use fine
    assert_clean(
        "package app\nprivate fun h(): int {\n    return 3\n}\nfun main(): int {\n    return h()\n}",
    );
    // private: cross-segment (same package) rejected
    assert_has(
        "package app\nprivate fun h(): int {\n    return 3\n}\npackage app\nfun other(): int {\n    return h()\n}",
        DiagCode::SemUndefinedName,
    );
    // protected: top-level rejected
    assert_has(
        "protected fun p(): int {\n    return 4\n}\nfun main(): int {\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn generic_inference_slice2_semantics() {
    // inferred + explicit + default-fallback all accepted
    assert_clean(
        "fun id<T>(x: T): T {\n    return x\n}\nfun def<T = int>(x: T): T {\n    return x\n}\nfun main(): int {\n    val a = id(42)\n    val b = id<int>(7)\n    val c = def(9)\n    return a + b + c\n}",
    );
    // struct ctor inference
    assert_clean(
        "struct Box<T>(val v: T)\nfun main(): int {\n    val b = Box(3)\n    return b.v\n}",
    );
    // uninferable without default (T not mentioned in params): rejected
    assert_has(
        "fun f<T>(x: int): int {\n    return x\n}\nfun main(): int {\n    return f(1)\n}",
        DiagCode::SemTypeArgMismatch,
    );
    // inferred instantiation mismatch: arg type vs inferred param
    assert_has(
        "fun id<T>(x: T): T {\n    return x\n}\nfun main(): int {\n    val s = id(\"a\")\n    return s\n}",
        DiagCode::SemTypeMismatch,
    );
}

#[test]
fn trait_impl_slice9a_semantics() {
    // trait + impl declarations accepted; Self in signatures
    assert_clean(
        "trait Show {\n    fun show(): string\n}\nstruct Point(val x: int)\nimpl Show for Point {\n    fun show(): string {\n        return \"p\"\n    }\n}\nfun main(): int {\n    return 0\n}",
    );
    // Self in trait signature unifies with the impl type
    assert_clean(
        "trait Eq {\n    fun equals(other: Self): bool\n}\nstruct P(val v: int)\nimpl Eq for P {\n    fun equals(other: P): bool {\n        return self.v == other.v\n    }\n}",
    );
    // signature mismatch rejected
    assert_has(
        "trait Show {\n    fun show(): string\n}\nstruct P(val v: int)\nimpl Show for P {\n    fun show(): int {\n        return 1\n    }\n}",
        DiagCode::SemTypeMismatch,
    );
    // missing trait method rejected
    assert_has(
        "trait Show {\n    fun show(): string\n}\nstruct P(val v: int)\nimpl Show for P {\n}",
        DiagCode::SemUnknownMember,
    );
    // extra impl method rejected
    assert_has(
        "trait Show {\n    fun show(): string\n}\nstruct P(val v: int)\nimpl Show for P {\n    fun show(): string {\n        return \"a\"\n    }\n    fun extra(): int {\n        return 1\n    }\n}",
        DiagCode::SemUnknownMember,
    );
    // duplicate impl rejected
    assert_has(
        "trait Show {\n    fun show(): string\n}\nstruct P(val v: int)\nimpl Show for P {\n    fun show(): string {\n        return \"a\"\n    }\n}\nimpl Show for P {\n    fun show(): string {\n        return \"b\"\n    }\n}",
        DiagCode::SemDuplicateDef,
    );
    // trait as a type annotation rejected (Q6: static dispatch only)
    assert_has(
        "trait Show {\n    fun show(): string\n}\nfun main(): int {\n    val x: Show = 1\n    return 0\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // generic trait / associated type / default impl rejected
    assert_has(
        "trait T<T> {\n    fun f(x: T): T\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}

#[test]
fn trait_dispatch_slice9b_semantics() {
    // static dispatch accepted (typed in 9a battery; here the call path)
    assert_clean(
        "trait Show {\n    fun show(): string\n}\nstruct P(val v: int)\nimpl Show for P {\n    fun show(): string {\n        return \"p\"\n    }\n}\nfun main(): int {\n    val p = P(1)\n    val s = p.show()\n    print(s)\n    return 0\n}",
    );
    // inherent (class) method wins over trait method (Q1 priority)
    assert_clean(
        "trait T {\n    fun who(): int\n}\nclass C(val v: int) {\n    fun who(): int {\n        return self.v\n    }\n}\nimpl T for C {\n    fun who(): int {\n        return 99\n    }\n}\nfun main(): int {\n    val c = C(7)\n    return c.who()\n}",
    );
    // ambiguous trait methods rejected
    assert_has(
        "trait A {\n    fun who(): int\n}\ntrait B {\n    fun who(): int\n}\nstruct P(val v: int)\nimpl A for P {\n    fun who(): int {\n        return 1\n    }\n}\nimpl B for P {\n    fun who(): int {\n        return 2\n    }\n}\nfun main(): int {\n    val p = P(0)\n    return p.who()\n}",
        DiagCode::SemUnsupportedConstruct,
    );
    // no impl: still the stable method-calls diagnostic
    assert_has(
        "struct P(val v: int)\nfun main(): int {\n    val p = P(0)\n    return p.nope()\n}",
        DiagCode::SemUnsupportedConstruct,
    );
}
