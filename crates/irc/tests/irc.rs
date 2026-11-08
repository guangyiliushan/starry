//! irc 集成测试：parser → sem → irc 全链，钉住 TAC 文本形态。
//! sem 诊断门禁与 irc M1 边界（IrcUnsupportedLowering）各一例。

use ast::syntax::node::DiagCode;
use irc::tac::TacOutput;

fn dump(source: &str) -> (String, Vec<DiagCode>) {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let sem_codes: Vec<DiagCode> = sem_out.diagnostics.iter().map(|d| d.code).collect();
    let tac: TacOutput = irc::lower(&mut sem_out, source);
    let irc_codes: Vec<DiagCode> = tac.diags.iter().map(|d| d.code).collect();
    let mut codes = sem_codes;
    codes.extend(irc_codes);
    let text = tac.dump(&sem_out.tys, &sem_out.defs, &sem_out.interner);
    (text, codes)
}

fn assert_tac(source: &str, expected: &str) {
    let (text, codes) = dump(source);
    assert_eq!(codes, Vec::new(), "unexpected diagnostics");
    assert_eq!(text, expected);
}

#[test]
fn arith_and_return() {
    assert_tac(
        "fun f(a: int): int {\n    return a + 1\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = add i64 %t0, 1\n    ret %t1 : i64\n",
    );
}

#[test]
fn var_slot_assignment_and_load() {
    assert_tac(
        "fun f(): int {\n    var i = 0\n    i = i + 1\n    return i\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    %t1 = load i64 %t0[0]\n    %t2 = add i64 %t1, 1\n    store %t0[0] = %t2 : i64\n    %t3 = load i64 %t0[0]\n    ret %t3 : i64\n",
    );
}

#[test]
fn while_loop_with_break_continue() {
    assert_tac(
        "fun f(n: int): int {\n    var i = 0\n    while i < n {\n        i = i + 1\n        if i == 2 {\n            continue\n        }\n        if i > 5 {\n            break\n        }\n    }\n    return i\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i64\n    jmp b1\n  b1:\n    %t2 = load i64 %t1[0]\n    %t3 = lt i64 %t2, %t0\n    br %t3 -> b2, b3\n  b2:\n    %t4 = load i64 %t1[0]\n    %t5 = add i64 %t4, 1\n    store %t1[0] = %t5 : i64\n    %t6 = load i64 %t1[0]\n    %t7 = eq i64 %t6, 2\n    br %t7 -> b4, b5\n  b3:\n    %t10 = load i64 %t1[0]\n    ret %t10 : i64\n  b4:\n    jmp b1\n  b5:\n    jmp b6\n  b6:\n    %t8 = load i64 %t1[0]\n    %t9 = gt i64 %t8, 5\n    br %t9 -> b7, b8\n  b7:\n    jmp b3\n  b8:\n    jmp b9\n  b9:\n    jmp b1\n",
    );
}

#[test]
fn short_circuit_and_materializes_bool() {
    assert_tac(
        "fun f(a: int, b: int): bool {\n    return a < b && b < 10\n}",
        "fun f(%t0: i64, %t1: i64) -> i1:\n  b0:\n    %t3 = lt i64 %t0, %t1\n    br %t3 -> b1, b3\n  b1:\n    %t4 = lt i64 %t1, 10\n    br %t4 -> b2, b3\n  b2:\n    %t2 = copy true : i1\n    jmp b4\n  b3:\n    %t2 = copy false : i1\n    jmp b4\n  b4:\n    ret %t2 : i1\n",
    );
}

#[test]
fn cast_as_inserts_conv() {
    assert_tac(
        "fun f(a: int): f64 {\n    return a as f64\n}",
        "fun f(%t0: i64) -> double:\n  b0:\n    %t1 = conv %t0 : i64 -> double\n    ret %t1 : double\n",
    );
}

#[test]
fn call_with_default_arg() {
    assert_tac(
        "fun g(a: int, b: int = 7): int {\n    return a + b\n}\nfun f(): int {\n    return g(1)\n}",
        "fun g(%t0: i64, %t1: i64) -> i64:\n  b0:\n    %t2 = add i64 %t0, %t1\n    ret %t2 : i64\nfun f() -> i64:\n  b0:\n    param 1 : i64\n    param 7 : i64\n    %t0 = call g : i64\n    ret %t0 : i64\n",
    );
}

#[test]
fn if_expression_uses_result_slot() {
    assert_tac(
        "fun f(x: int): int {\n    val y = if x > 0 { x } else { 0 }\n    return y\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = gt i64 %t0, 0\n    br %t1 -> b1, b2\n  b1:\n    store %t2[0] = %t0 : i64\n    jmp b3\n  b2:\n    store %t2[0] = 0 : i64\n    jmp b3\n  b3:\n    %t3 = load i64 %t2[0]\n    %t4 = copy %t3 : i64\n    ret %t4 : i64\n",
    );
}

#[test]
fn hex_and_radix_literals() {
    assert_tac(
        "fun f(): int {\n    return 0x1F\n}",
        "fun f() -> i64:\n  b0:\n    ret 31 : i64\n",
    );
    assert_tac(
        "fun f(): int {\n    return 0b101\n}",
        "fun f() -> i64:\n  b0:\n    ret 5 : i64\n",
    );
}

#[test]
fn overflowing_integer_literal_reports() {
    let (_, codes) =
        dump("fun f(): int {\n    return 999999999999999999999999999999999999999999\n}");
    assert!(codes.contains(&DiagCode::IntegerOutOfRange), "{codes:?}");
}

#[test]
fn unicode_escapes_decode() {
    assert_tac(
        "fun f(): int {\n    val c = '\u{41}'\n    return 0\n}",
        "fun f() -> i64:\n  b0:\n    %t0 = copy 65 : i32\n    ret 0 : i64\n",
    );
    assert_tac(
        "fun f(): string {\n    return \"A\u{42}C\"\n}",
        "fun f() -> str:\n  b0:\n    ret \"ABC\" : str\n",
    );
}

#[test]
fn invalid_unicode_escape_reports() {
    let (_, codes) = dump("fun f(): string {\n    return \"\\u{ZZ}\"\n}");
    assert!(codes.contains(&DiagCode::InvalidEscape), "{codes:?}");
}

#[test]
fn struct_ctor_and_field_read() {
    assert_tac(
        "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    val p = P(1, 2)\n    return p.x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    %t1 = load i64 %t0[0]\n    ret %t1 : i64\n",
    );
}

#[test]
fn enum_variant_value_and_ctor() {
    assert_tac(
        "enum E {\n    A\n    B\n}\nenum Opt {\n    None\n    Some(int)\n}\nfun f(): int {\n    val a = E.A\n    val o = Opt.Some(7)\n    return 0\n}",
        // 不可变聚合绑定 = 别名构造基址（零拷贝）；不再发聚合型 Copy
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i8\n    store %t1[0] = 1 : i8\n    store %t1[1] = 7 : i64\n    ret 0 : i64\n",
    );
}

#[test]
fn array_literal_and_dynamic_index() {
    assert_tac(
        "fun f(i: int): int {\n    val a = [1, 2, 3]\n    return a[i]\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 1 : i64\n    store %t1[1] = 2 : i64\n    store %t1[2] = 3 : i64\n    %t2 = load i64 %t1[%t0]\n    ret %t2 : i64\n",
    );
}

#[test]
fn array_repeat_fill_loop() {
    assert_tac(
        "fun f(): int {\n    val a = [9; 3]\n    return a[0]\n}",
        "fun f() -> i64:\n  b0:\n    %t0 = slot [3]\n    store %t1[0] = 0 : i32\n    jmp b1\n  b1:\n    %t2 = load i32 %t1[0]\n    %t3 = lt i32 %t2, 3\n    br %t3 -> b2, b3\n  b2:\n    %t4 = load i32 %t1[0]\n    store %t0[%t4] = 9 : i64\n    %t5 = add i32 %t4, 1\n    store %t1[0] = %t5 : i32\n    jmp b1\n  b3:\n    %t6 = load i64 %t0[0]\n    ret %t6 : i64\n",
    );
}

#[test]
fn tuple_literal() {
    assert_tac(
        "fun f(): int {\n    val t = (1, true)\n    return 0\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = true : i1\n    ret 0 : i64\n",
    );
}

#[test]
fn nullable_null_and_unwrap() {
    assert_tac(
        "fun f(): int {\n    val x = 1 as? int\n    return x!!\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i8\n    store %t0[1] = 1 : i64\n    %t1 = load i8 %t0[0]\n    br %t1 -> b1, b2\n  b1:\n    %t2 = load i64 %t0[1]\n    ret %t2 : i64\n  b2:\n    unreachable\n",
    );
}

#[test]
fn aggregate_var_assignment_copies() {
    assert_tac(
        "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    var p = P(1, 2)\n    p = P(3, 4)\n    return p.x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[1] = %t3 : i64\n    store %t4[0] = 3 : i64\n    store %t4[1] = 4 : i64\n    %t5 = load i64 %t4[0]\n    store %t1[0] = %t5 : i64\n    %t6 = load i64 %t4[1]\n    store %t1[1] = %t6 : i64\n    %t8 = load i64 %t1[0]\n    store %t7[0] = %t8 : i64\n    %t9 = load i64 %t1[1]\n    store %t7[1] = %t9 : i64\n    %t10 = load i64 %t7[0]\n    ret %t10 : i64\n",
    );
}

#[test]
fn when_statement_enum_subject() {
    assert_tac(
        "enum E {\n    A\n    B\n}\nfun f(e: E): int {\n    when e {\n        E.A -> 1\n        E.B -> 2\n    }\n}",
        "fun f(%t0: E) -> i64:\n  b0:\n    %t2 = load i8 %t0[0]\n    %t3 = eq i8 %t2, 0\n    br %t3 -> b2, b3\n  b1:\n    %t6 = load i64 %t1[0]\n    ret %t6 : i64\n  b2:\n    store %t1[0] = 1 : i64\n    jmp b1\n  b3:\n    %t4 = load i8 %t0[0]\n    %t5 = eq i8 %t4, 1\n    br %t5 -> b4, b5\n  b4:\n    store %t1[0] = 2 : i64\n    jmp b1\n  b5:\n    unreachable\n",
    );
}

#[test]
fn when_expression_with_else() {
    assert_tac(
        "fun f(x: int): int {\n    val y = when x {\n        0 -> 10\n        else -> 20\n    }\n    return y\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t2 = conv %t0 : i64 -> i32\n    %t3 = eq i32 %t2, 0\n    br %t3 -> b2, b3\n  b1:\n    %t4 = load i64 %t1[0]\n    %t5 = copy %t4 : i64\n    ret %t5 : i64\n  b2:\n    store %t1[0] = 10 : i64\n    jmp b1\n  b3:\n    jmp b4\n  b4:\n    store %t1[0] = 20 : i64\n    jmp b1\n  b5:\n    unreachable\n",
    );
}

#[test]
fn for_over_array() {
    assert_tac(
        "fun f(a: [int; 3]): int {\n    var s: int = 0\n    for i in a {\n        s = s + i\n    }\n    return s\n}",
        "fun f(%t0: [i64; 3]) -> i64:\n  b0:\n    store %t1[0] = 0 : i64\n    store %t2[0] = 0 : i32\n    jmp b1\n  b1:\n    %t3 = load i32 %t2[0]\n    %t4 = lt i32 %t3, 3\n    br %t4 -> b2, b4\n  b2:\n    %t6 = load i32 %t2[0]\n    %t5 = load i64 %t0[%t6]\n    %t7 = load i64 %t1[0]\n    %t8 = add i64 %t7, %t5\n    store %t1[0] = %t8 : i64\n    jmp b3\n  b3:\n    %t9 = load i32 %t2[0]\n    %t10 = add i32 %t9, 1\n    store %t2[0] = %t10 : i32\n    jmp b1\n  b4:\n    %t11 = load i64 %t1[0]\n    ret %t11 : i64\n",
    );
}

#[test]
fn for_over_inclusive_range() {
    assert_tac(
        "fun f(): int {\n    var s: int = 0\n    for i in 1..=3 {\n        s = s + i\n    }\n    return s\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    store %t1[0] = 1 : i64\n    jmp b1\n  b1:\n    %t2 = load i64 %t1[0]\n    %t3 = le i64 %t2, 3\n    br %t3 -> b2, b4\n  b2:\n    %t4 = load i64 %t1[0]\n    %t5 = load i64 %t0[0]\n    %t6 = add i64 %t5, %t4\n    store %t0[0] = %t6 : i64\n    jmp b3\n  b3:\n    %t7 = load i64 %t1[0]\n    %t8 = add i64 %t7, 1\n    store %t1[0] = %t8 : i64\n    jmp b1\n  b4:\n    %t9 = load i64 %t0[0]\n    ret %t9 : i64\n",
    );
}

#[test]
fn in_with_exclusive_range() {
    assert_tac(
        "fun f(x: i32): bool {\n    return x in 1..5\n}",
        "fun f(%t0: i32) -> i1:\n  b0:\n    %t1 = ge i32 %t0, 1\n    %t2 = lt i32 %t0, 5\n    br %t1 -> b1, b3\n  b1:\n    br %t2 -> b2, b3\n  b2:\n    %t3 = copy true : i1\n    jmp b4\n  b3:\n    %t3 = copy false : i1\n    jmp b4\n  b4:\n    ret %t3 : i1\n",
    );
}

#[test]
fn in_with_array_linear_scan() {
    assert_tac(
        "fun f(a: [int; 3], x: int): bool {\n    return x in a\n}",
        "fun f(%t0: [i64; 3], %t1: i64) -> i1:\n  b0:\n    store %t2[0] = 0 : i32\n    %t3 = copy false : i1\n    jmp b1\n  b1:\n    %t4 = load i32 %t2[0]\n    %t5 = lt i32 %t4, 3\n    br %t5 -> b2, b5\n  b2:\n    %t6 = load i64 %t0[%t4]\n    %t7 = eq i64 %t1, %t6\n    br %t7 -> b3, b4\n  b3:\n    %t3 = copy true : i1\n    jmp b5\n  b4:\n    %t8 = add i32 %t4, 1\n    store %t2[0] = %t8 : i32\n    jmp b1\n  b5:\n    ret %t3 : i1\n",
    );
}

#[test]
fn is_pattern_binds_in_then_branch() {
    assert_tac(
        "enum Opt {\n    None\n    Some(int)\n}\nfun f(o: Opt): int {\n    if o is Opt.Some(v) {\n        return v\n    }\n    return 0\n}",
        "fun f(%t0: Opt) -> i64:\n  b0:\n    %t1 = load i8 %t0[0]\n    %t2 = eq i8 %t1, 1\n    %t3 = load i64 %t0[1]\n    br %t2 -> b1, b2\n  b1:\n    ret %t3 : i64\n  b2:\n    jmp b3\n  b3:\n    ret 0 : i64\n",
    );
}

#[test]
fn defer_runs_after_return_value_computed() {
    assert_tac(
        "fun f(): int {\n    var x = 0\n    defer {\n        x = 99\n    }\n    return x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    %t1 = load i64 %t0[0]\n    store %t0[0] = 99 : i64\n    ret %t1 : i64\n",
    );
}

#[test]
fn elvis_operator() {
    assert_tac(
        "fun f(o: int?): int {\n    return o ?? 7\n}",
        "fun f(%t0: i64?) -> i64:\n  b0:\n    %t1 = load i8 %t0[0]\n    br %t1 -> b1, b2\n  b1:\n    %t3 = load i64 %t0[1]\n    store %t2[0] = %t3 : i64\n    jmp b3\n  b2:\n    store %t2[0] = 7 : i64\n    jmp b3\n  b3:\n    %t4 = load i64 %t2[0]\n    ret %t4 : i64\n",
    );
}

#[test]
fn safe_call_chain_with_elvis() {
    assert_tac(
        "struct P {\n    x: int\n}\nfun f(p: P?): int {\n    return p?.x ?? 0\n}",
        "fun f(%t0: P?) -> i64:\n  b0:\n    %t1 = load i8 %t0[0]\n    br %t1 -> b1, b2\n  b1:\n    %t2 = load i64 %t0[1]\n    store %t3[0] = 1 : i8\n    store %t3[1] = %t2 : i64\n    jmp b3\n  b2:\n    store %t3[0] = 0 : i8\n    jmp b3\n  b3:\n    %t4 = load i8 %t3[0]\n    br %t4 -> b4, b5\n  b4:\n    %t6 = load i64 %t3[1]\n    store %t5[0] = %t6 : i64\n    jmp b6\n  b5:\n    store %t5[0] = 0 : i64\n    jmp b6\n  b6:\n    %t7 = load i64 %t5[0]\n    ret %t7 : i64\n",
    );
}

#[test]
fn for_loop_break_targets_loop() {
    assert_tac(
        "fun f(): int {\n    var s = 0\n    for i in 1..3 {\n        s = s + i\n        break\n    }\n    return s\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    store %t1[0] = 1 : i64\n    jmp b1\n  b1:\n    %t2 = load i64 %t1[0]\n    %t3 = lt i64 %t2, 3\n    br %t3 -> b2, b4\n  b2:\n    %t4 = load i64 %t1[0]\n    %t5 = load i64 %t0[0]\n    %t6 = add i64 %t5, %t4\n    store %t0[0] = %t6 : i64\n    jmp b4\n  b3:\n    %t7 = load i64 %t1[0]\n    %t8 = add i64 %t7, 1\n    store %t1[0] = %t8 : i64\n    jmp b1\n  b4:\n    %t9 = load i64 %t0[0]\n    ret %t9 : i64\n",
    );
}

#[test]
fn defer_at_return_runs_innermost_first() {
    assert_tac(
        "fun f(): int {\n    var a = 0\n    defer {\n        a = a + 10\n    }\n    if true {\n        defer {\n            a = a + 1\n        }\n        return a\n    }\n    return 0\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    br true -> b1, b2\n  b1:\n    %t1 = load i64 %t0[0]\n    %t2 = load i64 %t0[0]\n    %t3 = add i64 %t2, 1\n    store %t0[0] = %t3 : i64\n    %t4 = load i64 %t0[0]\n    %t5 = add i64 %t4, 10\n    store %t0[0] = %t5 : i64\n    ret %t1 : i64\n  b2:\n    jmp b3\n  b3:\n    ret 0 : i64\n",
    );
}

#[test]
fn defer_at_break_runs_loop_level_only() {
    assert_tac(
        "fun f(): int {\n    var a = 0\n    defer {\n        a = a + 10\n    }\n    while a < 1 {\n        defer {\n            a = a + 1\n        }\n        break\n    }\n    return a\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    jmp b1\n  b1:\n    %t1 = load i64 %t0[0]\n    %t2 = lt i64 %t1, 1\n    br %t2 -> b2, b3\n  b2:\n    %t3 = load i64 %t0[0]\n    %t4 = add i64 %t3, 1\n    store %t0[0] = %t4 : i64\n    jmp b3\n  b3:\n    %t5 = load i64 %t0[0]\n    %t6 = load i64 %t0[0]\n    %t7 = add i64 %t6, 10\n    store %t0[0] = %t7 : i64\n    ret %t5 : i64\n",
    );
}

#[test]
fn when_with_literal_subject() {
    assert_tac(
        "fun f(): int {\n    return when 0 {\n        0 -> 1\n        else -> 2\n    }\n}",
        "fun f() -> i64:\n  b0:\n    %t0 = copy 0 : i64\n    %t2 = conv %t0 : i64 -> i32\n    %t3 = eq i32 %t2, 0\n    br %t3 -> b2, b3\n  b1:\n    %t4 = load i64 %t1[0]\n    ret %t4 : i64\n  b2:\n    store %t1[0] = 1 : i64\n    jmp b1\n  b3:\n    jmp b4\n  b4:\n    store %t1[0] = 2 : i64\n    jmp b1\n  b5:\n    unreachable\n",
    );
}

#[test]
fn range_value_materializes_slots_with_inclusivity() {
    assert_tac(
        "fun f(x: i32): bool {\n    val r = 1..5\n    return x in r\n}\n",
        "fun f(%t0: i32) -> i1:\n  b0:\n    store %t1[0] = 1 : i64\n    store %t1[1] = 5 : i64\n    store %t1[2] = 0 : i8\n    %t2 = load i32 %t1[0]\n    %t3 = load i32 %t1[1]\n    %t4 = load i8 %t1[2]\n    %t5 = ge i32 %t0, %t2\n    %t7 = ne i8 %t4, 0\n    br %t7 -> b1, b2\n  b1:\n    %t8 = le i32 %t0, %t3\n    %t6 = copy %t8 : i1\n    jmp b3\n  b2:\n    %t9 = lt i32 %t0, %t3\n    %t6 = copy %t9 : i1\n    jmp b3\n  b3:\n    br %t5 -> b4, b6\n  b4:\n    br %t6 -> b5, b6\n  b5:\n    %t10 = copy true : i1\n    jmp b7\n  b6:\n    %t10 = copy false : i1\n    jmp b7\n  b7:\n    ret %t10 : i1\n",
    );
}

#[test]
fn for_iterable_hoisted_out_of_loop() {
    assert_tac(
        "fun f(): int {\n    var arr = [1, 2, 3]\n    var s: int = 0\n    for i in arr {\n        s = s + i\n    }\n    return s\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    store %t0[2] = 3 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[1] = %t3 : i64\n    %t4 = load i64 %t0[2]\n    store %t1[2] = %t4 : i64\n    store %t5[0] = 0 : i64\n    %t7 = load i64 %t1[0]\n    store %t6[0] = %t7 : i64\n    %t8 = load i64 %t1[1]\n    store %t6[1] = %t8 : i64\n    %t9 = load i64 %t1[2]\n    store %t6[2] = %t9 : i64\n    store %t10[0] = 0 : i32\n    jmp b1\n  b1:\n    %t11 = load i32 %t10[0]\n    %t12 = lt i32 %t11, 3\n    br %t12 -> b2, b4\n  b2:\n    %t14 = load i32 %t10[0]\n    %t13 = load i64 %t6[%t14]\n    %t15 = load i64 %t5[0]\n    %t16 = add i64 %t15, %t13\n    store %t5[0] = %t16 : i64\n    jmp b3\n  b3:\n    %t17 = load i32 %t10[0]\n    %t18 = add i32 %t17, 1\n    store %t10[0] = %t18 : i32\n    jmp b1\n  b4:\n    %t19 = load i64 %t5[0]\n    ret %t19 : i64\n",
    );
}

#[test]
fn string_interpolation_runtime_chain() {
    assert_tac(
        "fun f(x: int): string {\n    return \"a${x}b\"\n}",
        "fun f(%t0: i64) -> str:\n  b0:\n    param \"a\" : str\n    %t1 = call starry_str_conv : str\n    param %t0 : i64\n    %t2 = call starry_str_conv : str\n    param %t1 : str\n    param %t2 : str\n    %t3 = call starry_str_concat : str\n    param \"b\" : str\n    %t4 = call starry_str_conv : str\n    param %t3 : str\n    param %t4 : str\n    %t5 = call starry_str_concat : str\n    ret %t5 : str\n",
    );
}

#[test]
fn compound_assignment_desugars() {
    assert_tac(
        "fun f(): int {\n    var i: int = 0\n    i += 2\n    i *= 3\n    return i\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i64\n    %t1 = load i64 %t0[0]\n    %t2 = add i64 %t1, 2\n    store %t0[0] = %t2 : i64\n    %t3 = load i64 %t0[0]\n    %t4 = mul i64 %t3, 3\n    store %t0[0] = %t4 : i64\n    %t5 = load i64 %t0[0]\n    ret %t5 : i64\n",
    );
}

#[test]
fn member_field_assignment() {
    assert_tac(
        "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    var p = P(1, 2)\n    p.x = 9\n    return p.x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[1] = %t3 : i64\n    store %t1[0] = 9 : i64\n    %t5 = load i64 %t1[0]\n    store %t4[0] = %t5 : i64\n    %t6 = load i64 %t1[1]\n    store %t4[1] = %t6 : i64\n    %t7 = load i64 %t4[0]\n    ret %t7 : i64\n",
    );
}

#[test]
fn array_index_assignment() {
    assert_tac(
        "fun f(): int {\n    var a = [1, 2, 3]\n    a[1] = 9\n    return a[1]\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    store %t0[2] = 3 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[1] = %t3 : i64\n    %t4 = load i64 %t0[2]\n    store %t1[2] = %t4 : i64\n    store %t1[1] = 9 : i64\n    %t6 = load i64 %t1[0]\n    store %t5[0] = %t6 : i64\n    %t7 = load i64 %t1[1]\n    store %t5[1] = %t7 : i64\n    %t8 = load i64 %t1[2]\n    store %t5[2] = %t8 : i64\n    %t9 = load i64 %t5[1]\n    ret %t9 : i64\n",
    );
}

#[test]
fn compound_assignment_through_member() {
    assert_tac(
        "struct P {\n    x: int\n}\nfun f(): int {\n    var p = P(1)\n    p.x += 2\n    return p.x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t4 = load i64 %t1[0]\n    store %t3[0] = %t4 : i64\n    %t5 = load i64 %t3[0]\n    %t6 = add i64 %t5, 2\n    store %t1[0] = %t6 : i64\n    %t8 = load i64 %t1[0]\n    store %t7[0] = %t8 : i64\n    %t9 = load i64 %t7[0]\n    ret %t9 : i64\n",
    );
}

#[test]
fn compound_assignment_through_index() {
    assert_tac(
        "fun f(): int {\n    var a = [1, 2, 3]\n    a[1] += 5\n    return a[1]\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    store %t0[1] = 2 : i64\n    store %t0[2] = 3 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[1] = %t3 : i64\n    %t4 = load i64 %t0[2]\n    store %t1[2] = %t4 : i64\n    %t6 = load i64 %t1[0]\n    store %t5[0] = %t6 : i64\n    %t7 = load i64 %t1[1]\n    store %t5[1] = %t7 : i64\n    %t8 = load i64 %t1[2]\n    store %t5[2] = %t8 : i64\n    %t9 = load i64 %t5[1]\n    %t10 = add i64 %t9, 5\n    store %t1[1] = %t10 : i64\n    %t12 = load i64 %t1[0]\n    store %t11[0] = %t12 : i64\n    %t13 = load i64 %t1[1]\n    store %t11[1] = %t13 : i64\n    %t14 = load i64 %t1[2]\n    store %t11[2] = %t14 : i64\n    %t15 = load i64 %t11[1]\n    ret %t15 : i64\n",
    );
}

#[test]
fn nested_struct_ctor_slot_types_and_deep_read() {
    // 回归：copy_slots 槽类型按值内部下标求值（非绝对偏移）——
    // 偏移非 0 的聚合字段拷贝不再是 unit；嵌套成员读逐槽快照
    assert_tac(
        "struct Inner {\n    x: int\n    y: int\n}\nstruct Outer {\n    a: int\n    inner: Inner\n}\nfun f(): int {\n    val o = Outer(1, Inner(2, 3))\n    return o.inner.y\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 2 : i64\n    store %t0[1] = 3 : i64\n    store %t1[0] = 1 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[1] = %t2 : i64\n    %t3 = load i64 %t0[1]\n    store %t1[2] = %t3 : i64\n    %t5 = load i64 %t1[1]\n    store %t4[0] = %t5 : i64\n    %t6 = load i64 %t1[2]\n    store %t4[1] = %t6 : i64\n    %t7 = load i64 %t4[1]\n    ret %t7 : i64\n",
    );
}

#[test]
fn aggregate_if_expression_copies_slots() {
    // 回归：聚合结果型 if 走逐槽拷贝，汇合别名槽基址——
    // 结果槽不再把聚合基址塞进单槽（ty 塞聚合型）
    assert_tac(
        "struct P {\n    x: int\n    y: int\n}\nfun f(c: bool): P {\n    return if c { P(1, 2) } else { P(3, 4) }\n}",
        "fun f(%t0: i1) -> P:\n  b0:\n    br %t0 -> b1, b2\n  b1:\n    store %t2[0] = 1 : i64\n    store %t2[1] = 2 : i64\n    %t3 = load i64 %t2[0]\n    store %t1[0] = %t3 : i64\n    %t4 = load i64 %t2[1]\n    store %t1[1] = %t4 : i64\n    jmp b3\n  b2:\n    store %t5[0] = 3 : i64\n    store %t5[1] = 4 : i64\n    %t6 = load i64 %t5[0]\n    store %t1[0] = %t6 : i64\n    %t7 = load i64 %t5[1]\n    store %t1[1] = %t7 : i64\n    jmp b3\n  b3:\n    ret %t1 : P\n",
    );
}

#[test]
fn for_continue_targets_step_block() {
    // 回归：continue 直跳 step 块（先自增再判条件）——
    // 不再跳过自增死循环；自增也不再落进 break 后的孤儿块
    assert_tac(
        "fun f(n: int): int {\n    var s: int = 0\n    for i in 0..n {\n        if i == 1 {\n            continue\n        }\n        s = s + i\n    }\n    return s\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i64\n    store %t2[0] = 0 : i64\n    jmp b1\n  b1:\n    %t3 = load i64 %t2[0]\n    %t4 = lt i64 %t3, %t0\n    br %t4 -> b2, b4\n  b2:\n    %t5 = load i64 %t2[0]\n    %t6 = eq i64 %t5, 1\n    br %t6 -> b5, b6\n  b3:\n    %t9 = load i64 %t2[0]\n    %t10 = add i64 %t9, 1\n    store %t2[0] = %t10 : i64\n    jmp b1\n  b4:\n    %t11 = load i64 %t1[0]\n    ret %t11 : i64\n  b5:\n    jmp b3\n  b6:\n    jmp b7\n  b7:\n    %t7 = load i64 %t1[0]\n    %t8 = add i64 %t7, %t5\n    store %t1[0] = %t8 : i64\n    jmp b3\n",
    );
}

#[test]
fn if_is_condition_keeps_outer_scope() {
    // 回归：is 条件先推作用域再 lowering，调用方配对弹出——
    // 外层 val 不再被弹掉（曾 IrcUnsupportedLowering）
    assert_tac(
        "enum Opt {\n    None\n    Some(int)\n}\nfun f(o: Opt): int {\n    val x = 5\n    if o is Opt.Some(v) {\n        return v\n    }\n    return x\n}",
        "fun f(%t0: Opt) -> i64:\n  b0:\n    %t1 = copy 5 : i64\n    %t2 = load i8 %t0[0]\n    %t3 = eq i8 %t2, 1\n    %t4 = load i64 %t0[1]\n    br %t3 -> b1, b2\n  b1:\n    ret %t4 : i64\n  b2:\n    jmp b3\n  b3:\n    ret %t1 : i64\n",
    );
}

#[test]
fn too_many_variants_emits_diagnostic() {
    // 回归：IrcTooManyVariants 曾只定义不发射（tag 槽 i8，>256 变体装不下）
    let variants: Vec<String> = (0..257).map(|i| format!("    V{i}")).collect();
    let src = format!(
        "enum E {{\n{}\n}}\nfun f(): int {{\n    val e = E.V256\n    return 0\n}}",
        variants.join("\n")
    );
    let (_, codes) = dump(&src);
    assert!(
        codes.contains(&DiagCode::IrcTooManyVariants),
        "missing IrcTooManyVariants: {codes:?}"
    );
}

#[test]
fn sem_error_gates_lowering() {
    let (text, codes) = dump("fun f(): int {\n    return nosuch\n}");
    assert!(codes.contains(&DiagCode::SemUndefinedName));
    assert_eq!(text, "");
}

#[test]
fn rest_pattern_maps_to_edges() {
    let (text, codes) = dump(
        "fun f(a: [int; 4]): int {
    if a is [x, .., y] {
        return x + y
    }
    return 0
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        text.contains("load i64 %t0[0]") && text.contains("load i64 %t0[3]"),
        "{text}"
    );
}

#[test]
fn or_pattern_with_shared_binding() {
    let (text, codes) = dump(
        "enum E {
    A(int)
    B(int)
}
fun f(e: E): int {
    if e is A(x) | B(x) {
        return x
    }
    return 0
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(text.contains("eq i8") && text.contains("ret"), "{text}");
}

#[test]
fn destructuring_binding_declaration() {
    let (text, codes) = dump(
        "fun f(): int {
    val (x, y) = (1, 2)
    return x + y
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        text.contains("store %t0[0] = 1") && text.contains("store %t0[1] = 2"),
        "{text}"
    );
}

#[test]
fn string_plus_lowers_to_runtime_concat() {
    let (text, codes) = dump("fun f(): string {\n    return \"a\" + \"b\"\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(text.contains("call starry_str_concat : str"), "{text}");
}

#[test]
fn top_level_scalar_globals_lower_to_storage_ops() {
    let (text, codes) =
        dump("val G = 40\nvar H: int\nfun f(): int {\n    H = G + 2\n    return H\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        text.contains("load_global") && text.contains("store_global"),
        "{text}"
    );
}

#[test]
fn print_builtin_lowers_to_runtime_call() {
    let (text, codes) = dump(
        "fun main() {
    print(42)
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        // M1：42 未约束 → i64（param 类型随字面量定型）
        text.contains("param 42 : i64") && text.contains("call starry_print : unit"),
        "{text}"
    );
}

#[test]
fn mutable_destructuring_binding_assignment() {
    let (text, codes) = dump(
        "fun f(): int {
    var (x, y) = (1, 2)
    x = 3
    return x + y
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    // M1：未约束字面量默认 i64
    assert!(
        text.contains("= 3 : i64") && text.contains("add i64"),
        "{text}"
    );
}

#[test]
fn aggregate_element_for_loop() {
    let (text, codes) = dump(
        "struct P {
    x: int
    y: int
}
fun f(a: [P; 2]): int {
    var s: int = 0
    for p in a {
        s = s + p.x
    }
    return s
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    // 聚合元素跨槽（es=2 → mul）；s: int 恒 i64（M1 不受影响）
    assert!(text.contains("mul") && text.contains("load i64"), "{text}");
}

#[test]
fn aggregate_element_in_scan() {
    let (text, codes) = dump(
        "struct P {
    x: int
    y: int
}
fun f(a: [P; 2], x: P): bool {
    return x in a
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(text.contains("mul") && text.contains("eq i64"), "{text}");
}

#[test]
fn aggregate_element_index_assignment() {
    let (text, codes) = dump(
        "struct P {
    x: int
    y: int
}
fun f(): int {
    var a = [P(1, 2), P(3, 4)]
    a[1] = P(9, 8)
    return a[1].x
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(text.contains("mul") && text.contains("store"), "{text}");
}

#[test]
fn single_slot_aggregate_var_field_assignment() {
    // 回归：单字段 struct 的 var 字段赋值。槽数 1 的聚合也是「基址 + 槽」，
    // Store 的 ty 必须是槽类型（i64）而不是聚合型 P——把基址当 i64 值存
    // 会让 llvme 发出 `store i64 %t0.slot`（ptr 当 i64，非法 IR）。
    assert_tac(
        "struct P { x: int }\nfun f(): int {\n    var p = P(1)\n    p.x = 50\n    return p.x\n}",
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 1 : i64\n    %t2 = load i64 %t0[0]\n    store %t1[0] = %t2 : i64\n    store %t1[0] = 50 : i64\n    %t4 = load i64 %t1[0]\n    store %t3[0] = %t4 : i64\n    %t5 = load i64 %t3[0]\n    ret %t5 : i64\n",
    );
}

#[test]
fn single_slot_aggregate_read_is_a_base() {
    // 回归：单槽聚合读出的必须是**基址**，不是标量值。折成标量会让成员
    // 读取把该值当基址用（`load i64 %t2[0]` 对一个 i64 做 GEP）。
    // 形参聚合是基址，字段读取就是一次槽类型 Load。
    assert_tac(
        "struct P { x: int }\nfun f(p: P): int {\n    return p.x\n}",
        "fun f(%t0: P) -> i64:\n  b0:\n    %t1 = load i64 %t0[0]\n    ret %t1 : i64\n",
    );
}

#[test]
fn cyclic_global_initializers_report_with_decl_spans() {
    // 环诊断定位到环内各全局的声明（Def.span），不再恒 0..0
    let src = "val a: int = b\nval b: int = a";
    let parsed = parser::parse_source(src).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let tac: TacOutput = irc::lower(&mut sem_out, src);
    // 回边目标（环的根成员）报一条即可拒绝整个环——与 sem 递归类型
    // 检测同一粒度；关键是 span 指向声明本身，不再恒 0..0
    assert_eq!(tac.diags.len(), 1, "{:?}", tac.diags);
    assert_eq!(
        tac.diags[0].span,
        ast::token::Span { start: 0, end: 14 },
        "{:?}",
        tac.diags
    );
}

#[test]
fn deep_global_chain_does_not_overflow() {
    // 回归：全局依赖排序原为递归 DFS，数万级初始化链在 2 MiB 测试
    // 线程栈上爆栈；显式栈迭代应稳过（链无环 → 零诊断）
    let n = 20_000;
    let mut src = String::new();
    for i in 0..n {
        if i + 1 < n {
            src.push_str(&format!("val g{i}: int = g{}\n", i + 1));
        } else {
            src.push_str(&format!("val g{i}: int = 0\n"));
        }
    }
    let parsed = parser::parse_source(&src).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    assert!(
        sem_out.diagnostics.is_empty(),
        "{:?}",
        sem_out.diagnostics.first()
    );
    let tac: TacOutput = irc::lower(&mut sem_out, &src);
    assert!(tac.diags.is_empty(), "{:?}", tac.diags.first());
}
#[test]
fn annotated_array_literal_stores_at_target_element_width() {
    // 审查 P0-1：字面量块按目标元素宽直写（消除 [n x i64] 块里的 store i32
    // 与二次 widen 双块）；窄写宽读是静默错编源
    let (text, codes) = dump("fun f() : int {\n    val a: [int; 2] = [7, 8]\n    return a[0]\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        text.contains("store %t0[0] = 7 : i64") && text.contains("store %t0[1] = 8 : i64"),
        "{text}"
    );
    assert!(!text.contains(": i32"), "{text}");
}
#[test]
fn aggregate_top_level_global_use_reports_stable_unsupported() {
    // 审查 P0-3：未 lower 的顶层聚合全局，使用点是稳定码 unsupported 诊断，
    // 不是「契约破缺」内部断言，也不级联
    let (_, codes) = dump("val G: [int; 2] = [7, 8]\nfun main() {\n    print(G[0])\n}");
    assert_eq!(
        codes,
        vec![
            DiagCode::IrcUnsupportedLowering,
            DiagCode::IrcUnsupportedLowering
        ]
    );
}
#[test]
fn borrow_and_deref_lower_to_pointer_instrs() {
    // M2 slice 2：&x → AddrOf；*p → LoadPtr；*p = v → StorePtr
    let (text, codes) = dump(
        "fun f() : int {
    var y = 7
    val p = &y
    val q = &mut y
    *q = 9
    return *p
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    assert!(
        text.contains("addrof") && text.contains("loadptr") && text.contains("storeptr"),
        "{text}"
    );
}
