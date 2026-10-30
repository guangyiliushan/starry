//! opt 集成测试：parser → sem → irc → opt 全链，钉住优化后 TAC 文本。
//! 每 pass 一正一负（契约：fold/copy/dce/cse 钉板 + 副作用不可删）。

use irc::tac::TacOutput;

fn dump(source: &str) -> String {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    assert!(
        sem_out.diagnostics.is_empty(),
        "unexpected sem diags: {:?}",
        sem_out
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
    );
    let mut tac: TacOutput = irc::lower(&mut sem_out, source);
    assert!(
        tac.diags.is_empty(),
        "unexpected irc diags: {:?}",
        tac.diags.iter().map(|d| d.code).collect::<Vec<_>>()
    );
    opt::optimize(&mut tac);
    tac.dump(&sem_out.tys, &sem_out.defs, &sem_out.interner)
}

fn assert_opt(source: &str, expected: &str) {
    assert_eq!(dump(source), expected);
}

#[test]
fn fold_arith_to_const() {
    assert_opt(
        "fun f(): int {\n    return 1 + 2\n}",
        "fun f() -> i64:\n  b0:\n    %t1 = conv 3 : i32 -> i64\n    ret %t1 : i64\n",
    );
}

#[test]
fn fold_br_cond_to_jmp() {
    assert_opt(
        "fun f(): int {\n    if 1 < 2 {\n        return 1\n    }\n    return 0\n}",
        "fun f() -> i64:\n  b0:\n    jmp b1\n  b1:\n    ret 1 : i64\n  b2:\n    jmp b3\n  b3:\n    ret 0 : i64\n",
    );
}

#[test]
fn copy_prop_through_temp() {
    assert_opt(
        "fun f(a: int): int {\n    val x = a\n    return x\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    ret %t0 : i64\n",
    );
}

#[test]
fn dce_removes_unused_pure() {
    assert_opt(
        "fun f(a: int): int {\n    val x = a + 1\n    return a\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    ret %t0 : i64\n",
    );
}

#[test]
fn call_side_effect_survives_dce() {
    assert_opt(
        "fun g(): int {\n    return 1\n}\nfun f(a: int): int {\n    val x = g()\n    return a\n}",
        "fun g() -> i64:\n  b0:\n    ret 1 : i64\nfun f(%t0: i64) -> i64:\n  b0:\n    %t1 = call g : i64\n    ret %t0 : i64\n",
    );
}

#[test]
fn cse_reuses_common_subexpr() {
    assert_opt(
        "fun f(a: int): int {\n    val x = a + 1\n    val y = a + 1\n    return x + y\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = add i64 %t0, 1\n    %t5 = add i64 %t1, %t1\n    ret %t5 : i64\n",
    );
}

#[test]
fn commutative_operands_normalized() {
    // 交换律归一命中：1 + a 与 a + 1 是同一表达式
    assert_opt(
        "fun f(a: int): int {\n    val x = 1 + a\n    val y = a + 1\n    return x + y\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = add i64 1, %t0\n    %t5 = add i64 %t1, %t1\n    ret %t5 : i64\n",
    );
}

#[test]
fn div_by_zero_not_folded() {
    assert_opt(
        "fun f(a: int): int {\n    return a / 0\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = div i64 %t0, 0\n    ret %t1 : i64\n",
    );
}
