//! opt 集成测试：parser → sem → irc → opt 全链，钉住优化后 TAC 文本。
//! 每 pass 一正一负（契约：fold/copy/dce/cse 钉板 + 副作用不可删）。

use irc::interp::{InterpError, Value};
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
    opt::optimize(&mut tac, &sem_out.tys);
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
fn dce_keeps_temp_used_in_successor_block() {
    // 回归：val temp 跨块引用（b1 用 b0 的 t1/t2），块内活性扫描曾误删定义
    assert_opt(
        "fun f(a: int): int {\n    val x = a + 1\n    if a > 0 {\n        return x\n    }\n    return 0\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = add i64 %t0, 1\n    %t2 = copy %t1 : i64\n    %t3 = gt i64 %t0, 0\n    br %t3 -> b1, b2\n  b1:\n    ret %t2 : i64\n  b2:\n    jmp b3\n  b3:\n    ret 0 : i64\n",
    );
}

#[test]
fn dce_removes_temp_unused_across_all_blocks() {
    // 全函数无使用 → 跨块也该删（与上一例互补）
    assert_opt(
        "fun f(a: int): int {\n    val x = a + 1\n    if a > 0 {\n        return a\n    }\n    return 0\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t3 = gt i64 %t0, 0\n    br %t3 -> b1, b2\n  b1:\n    ret %t0 : i64\n  b2:\n    jmp b3\n  b3:\n    ret 0 : i64\n",
    );
}

#[test]
fn licm_hoists_loop_invariant_to_preheader() {
    assert_opt(
        "fun f(n: int): int {\n    var i = 0\n    var s: int = 0\n    while i < n {\n        s = s + n * 2\n        i = i + 1\n    }\n    return s\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i32\n    store %t2[0] = 0 : i64\n    %t7 = mul i64 %t0, 2\n    jmp b1\n  b1:\n    %t3 = load i32 %t1[0]\n    %t4 = conv %t3 : i32 -> i64\n    %t5 = lt i64 %t4, %t0\n    br %t5 -> b2, b3\n  b2:\n    %t6 = load i64 %t2[0]\n    %t8 = add i64 %t6, %t7\n    store %t2[0] = %t8 : i64\n    %t9 = load i32 %t1[0]\n    %t10 = add i32 %t9, 1\n    store %t1[0] = %t10 : i32\n    jmp b1\n  b3:\n    %t11 = load i64 %t2[0]\n    ret %t11 : i64\n",
    );
}

#[test]
fn licm_keeps_trap_family_in_loop() {
    // Div/Rem/Shl/Shr 零次执行循环里外提会发明陷阱语义，v1 保守排除
    assert_opt(
        "fun f(n: int): int {\n    var i = 0\n    var s: int = 0\n    while i < n {\n        s = s + n / 2\n        i = i + 1\n    }\n    return s\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i32\n    store %t2[0] = 0 : i64\n    jmp b1\n  b1:\n    %t3 = load i32 %t1[0]\n    %t4 = conv %t3 : i32 -> i64\n    %t5 = lt i64 %t4, %t0\n    br %t5 -> b2, b3\n  b2:\n    %t6 = load i64 %t2[0]\n    %t7 = div i64 %t0, 2\n    %t8 = add i64 %t6, %t7\n    store %t2[0] = %t8 : i64\n    %t9 = load i32 %t1[0]\n    %t10 = add i32 %t9, 1\n    store %t1[0] = %t10 : i32\n    jmp b1\n  b3:\n    %t11 = load i64 %t2[0]\n    ret %t11 : i64\n",
    );
}

#[test]
fn licm_keeps_variant_operand_in_loop() {
    assert_opt(
        "fun f(n: int): int {\n    var i = 0\n    var s = 0\n    while i < n {\n        s = s + i * 2\n        i = i + 1\n    }\n    return s\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    store %t1[0] = 0 : i32\n    store %t2[0] = 0 : i32\n    jmp b1\n  b1:\n    %t3 = load i32 %t1[0]\n    %t4 = conv %t3 : i32 -> i64\n    %t5 = lt i64 %t4, %t0\n    br %t5 -> b2, b3\n  b2:\n    %t6 = load i32 %t2[0]\n    %t7 = load i32 %t1[0]\n    %t8 = mul i32 %t7, 2\n    %t9 = add i32 %t6, %t8\n    store %t2[0] = %t9 : i32\n    %t10 = load i32 %t1[0]\n    %t11 = add i32 %t10, 1\n    store %t1[0] = %t11 : i32\n    jmp b1\n  b3:\n    %t12 = load i32 %t2[0]\n    %t13 = conv %t12 : i32 -> i64\n    ret %t13 : i64\n",
    );
}

#[test]
fn licm_never_treats_phi_materialized_temp_as_invariant() {
    // 回归：&& 物化的 result temp 两分支各写一次（多定义），
    // LICM 不得把它当循环不变外提；真不变量（a < n）仍外提
    assert_opt(
        "fun f(n: int, a: int): int {\n    var i = 0\n    var s = 0\n    while i < n {\n        val b = a < n && i < n\n        if b {\n            s = s + 1\n        }\n        i = i + 1\n    }\n    return s\n}",
        "fun f(%t0: i64, %t1: i64) -> i64:\n  b0:\n    store %t2[0] = 0 : i32\n    store %t3[0] = 0 : i32\n    %t8 = lt i64 %t1, %t0\n    jmp b1\n  b1:\n    %t4 = load i32 %t2[0]\n    %t5 = conv %t4 : i32 -> i64\n    %t6 = lt i64 %t5, %t0\n    br %t6 -> b2, b3\n  b2:\n    br %t8 -> b4, b6\n  b3:\n    %t17 = load i32 %t3[0]\n    %t18 = conv %t17 : i32 -> i64\n    ret %t18 : i64\n  b4:\n    %t9 = load i32 %t2[0]\n    %t10 = conv %t9 : i32 -> i64\n    %t11 = lt i64 %t10, %t0\n    br %t11 -> b5, b6\n  b5:\n    %t7 = copy true : i1\n    jmp b7\n  b6:\n    %t7 = copy false : i1\n    jmp b7\n  b7:\n    br %t7 -> b8, b9\n  b8:\n    %t13 = load i32 %t3[0]\n    %t14 = add i32 %t13, 1\n    store %t3[0] = %t14 : i32\n    jmp b10\n  b9:\n    jmp b10\n  b10:\n    %t15 = load i32 %t2[0]\n    %t16 = add i32 %t15, 1\n    store %t2[0] = %t16 : i32\n    jmp b1\n",
    );
}

#[test]
fn div_by_zero_not_folded() {
    assert_opt(
        "fun f(a: int): int {\n    return a / 0\n}",
        "fun f(%t0: i64) -> i64:\n  b0:\n    %t1 = div i64 %t0, 0\n    ret %t1 : i64\n",
    );
}

// ── 折叠位宽（2026-10-31 修正案）─────────────────────────

#[test]
fn fold_wraps_add_at_operand_width() {
    // i32 域回绕成 -294967296，不是 i128 的 4000000000（后者是超位宽字面量）
    assert_opt(
        "fun f(): int {\n    return 2000000000 + 2000000000\n}",
        "fun f() -> i64:\n  b0:\n    %t1 = conv -294967296 : i32 -> i64\n    ret %t1 : i64\n",
    );
}

#[test]
fn fold_div_sees_wrapped_operand() {
    // 回绕后再除：-294967296 / 2 = -147483648。旧的 i128 折叠给出
    // 2000000000，与未优化代码（LLVM `div i32`）不同值——静默错编
    assert_opt(
        "fun f(): int {\n    return (2000000000 + 2000000000) / 2\n}",
        "fun f() -> i64:\n  b0:\n    %t2 = conv -147483648 : i32 -> i64\n    ret %t2 : i64\n",
    );
}

#[test]
fn fold_compares_wrapped_operands() {
    // 比较的是规约后的 N 位值：回绕成负数，故 < 0 为真
    assert_opt(
        "fun f(): bool {\n    return (2000000000 + 2000000000) < 0\n}",
        "fun f() -> i1:\n  b0:\n    ret true : i1\n",
    );
}

#[test]
fn shift_out_of_type_width_not_folded() {
    // 移位量判据是目标位宽（i32 → 0..32），不是 i128 的 0..128
    assert_opt(
        "fun f(): int {\n    return 1 << 40\n}",
        "fun f() -> i64:\n  b0:\n    %t0 = shl i32 1, 40\n    %t1 = conv %t0 : i32 -> i64\n    ret %t1 : i64\n",
    );
}

#[test]
fn fold_still_works_in_i64_domain() {
    // 位宽规约不得让 64 位域的正常折叠失效
    assert_opt(
        "fun f(): int {\n    val x: int = 3\n    return x + 1\n}",
        "fun f() -> i64:\n  b0:\n    ret 4 : i64\n",
    );
}

// ── 语义保持性（解释器对拍，2026-10-31）──────────────────
// 快照只看 TAC 文本形状，错编在里面是「合法文本」——折叠位宽那次
// 就是这么活下来的。这里优化前后各跑一遍解释器，结果必须逐值相等。

fn interp_result(
    source: &str,
    name: &str,
    args: &[Value],
    optimized: bool,
) -> Result<Value, InterpError> {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    assert!(sem_out.diagnostics.is_empty());
    let mut tac: TacOutput = irc::lower(&mut sem_out, source);
    assert!(tac.diags.is_empty());
    if optimized {
        opt::optimize(&mut tac, &sem_out.tys);
    }
    irc::interp::run_named(
        &tac,
        &sem_out.tys,
        &sem_out.defs,
        &sem_out.interner,
        name,
        args,
    )
}

fn assert_semantics_preserved(source: &str, name: &str, args: &[Value]) {
    let before = interp_result(source, name, args, false);
    let after = interp_result(source, name, args, true);
    assert_eq!(before, after, "优化改变了可观察行为: {source}");
}

#[test]
fn opt_preserves_semantics_fold_wrap() {
    // 回归钉板：折叠位宽错编（曾折成 2000000000，未优化算 -147483648）
    assert_semantics_preserved(
        "fun f(): i32 {\n    return (2000000000 + 2000000000) / 2\n}",
        "f",
        &[],
    );
}

#[test]
fn opt_preserves_semantics_fold_compare() {
    assert_semantics_preserved(
        "fun f(): bool {\n    return (2000000000 + 2000000000) < 0\n}",
        "f",
        &[],
    );
}

#[test]
fn opt_preserves_semantics_shift() {
    // 越界移位两侧同为 trap：不折叠、不发明值
    assert_semantics_preserved("fun f(): i32 {\n    return 1 << 40\n}", "f", &[]);
}

#[test]
fn opt_preserves_semantics_fold_chain() {
    // 折叠 + 复制传播 + CSE + DCE 一起动（死代码不得改变语义）
    assert_semantics_preserved(
        "fun f(): i32 {\n    val a = 3 * 4\n    val b = 3 * 4\n    val dead = 9 / 3\n    return (a + b) - 2\n}",
        "f",
        &[],
    );
}

#[test]
fn opt_preserves_semantics_licm_loop() {
    assert_semantics_preserved(
        "fun f(n: int): int {\n    var i = 0\n    var s: int = 0\n    while i < n {\n        s = s + n * 2\n        i = i + 1\n    }\n    return s\n}",
        "f",
        &[Value::I(5)],
    );
}

#[test]
fn opt_preserves_semantics_narrow_params() {
    // 窄位宽实参 + conv 链（sext/trunc）在优化下不得变值
    assert_semantics_preserved(
        "fun f(a: i8, b: i8): i32 {\n    return (a as i32) + (b as i32)\n}",
        "f",
        &[Value::I(200), Value::I(3)],
    );
}
