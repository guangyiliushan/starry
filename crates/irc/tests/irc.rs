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
        "fun f() -> i64:\n  b0:\n    store %t0[0] = 0 : i32\n    %t1 = load i32 %t0[0]\n    %t2 = add i32 %t1, 1\n    store %t0[0] = %t2 : i32\n    %t3 = load i32 %t0[0]\n    %t4 = conv %t3 : i32 -> i64\n    ret %t4 : i64\n",
    );
}

#[test]
fn while_loop_with_break_continue() {
    assert_tac(
        "fun f(n: int): int {\n    var i = 0\n    while i < n {\n        i = i + 1\n        if i == 2 {\n            continue\n        }\n        if i > 5 {\n            break\n        }\n    }\n    return i\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i32\n    jmp b1\n  b1:\n    %t2 = load i32 %t1[0]\n    %t3 = conv %t2 : i32 -> i64\n    %t4 = lt i64 %t3, %t0\n    br %t4 -> b2, b3\n  b2:\n    %t5 = load i32 %t1[0]\n    %t6 = add i32 %t5, 1\n    store %t1[0] = %t6 : i32\n    %t7 = load i32 %t1[0]\n    %t8 = eq i32 %t7, 2\n    br %t8 -> b4, b5\n  b3:\n    %t11 = load i32 %t1[0]\n    %t12 = conv %t11 : i32 -> i64\n    ret %t12 : i64\n  b4:\n    jmp b1\n  b5:\n    jmp b6\n  b6:\n    %t9 = load i32 %t1[0]\n    %t10 = gt i32 %t9, 5\n    br %t10 -> b7, b8\n  b7:\n    jmp b3\n  b8:\n    jmp b9\n  b9:\n    jmp b1\n",
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
fn sem_error_gates_lowering() {
    let (text, codes) = dump("fun f(): int {\n    return nosuch\n}");
    assert!(codes.contains(&DiagCode::SemUndefinedName));
    assert_eq!(text, "");
}

#[test]
fn unsupported_construct_reports_and_stops() {
    let (_, codes) = dump(
        "enum E {\n    A\n    B\n}\nfun f(e: E): int {\n    when e {\n        E.A -> 1\n        E.B -> 2\n    }\n}",
    );
    assert!(codes.contains(&DiagCode::IrcUnsupportedLowering));
}
