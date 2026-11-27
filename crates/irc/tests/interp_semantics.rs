//! 语义级回归（TAC 解释器执行，断言「跑出来的结果」）：对应审查报告的
//! 六个 P0 形态——快照测试抓不到的语义错编，在这里全部变成结果断言。

use irc::interp::{InterpError, Value, run_named};
use irc::tac::TacOutput;

fn build(source: &str) -> (TacOutput, sem::SemanticOutput, String) {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let codes: Vec<ast::syntax::node::DiagCode> =
        sem_out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, Vec::new(), "{codes:?}");
    let tac = irc::lower(&mut sem_out, source);
    assert!(tac.diags.is_empty(), "{:?}", tac.diags);
    (tac, sem_out, source.to_string())
}

fn run(source: &str, name: &str, args: &[Value]) -> Value {
    let (tac, sem_out, _) = build(source);
    run_named(
        &tac,
        &sem_out.tys,
        &sem_out.defs,
        &sem_out.interner,
        name,
        args,
    )
    .expect("interp")
}

/// 同 `run`，但保留 Result（trap 用例断言用）
fn run_res(source: &str, name: &str, args: &[Value]) -> Result<Value, InterpError> {
    let (tac, sem_out, _) = build(source);
    run_named(
        &tac,
        &sem_out.tys,
        &sem_out.defs,
        &sem_out.interner,
        name,
        args,
    )
}

// P0-1：?? 曾恒取右值
#[test]
fn elvis_picks_some_when_nonnull() {
    let src = "fun f(o: int?): int {\n    return o ?? 7\n}";
    assert_eq!(run(src, "f", &[Value::I(1)]), Value::I(1));
    assert_eq!(run(src, "f", &[Value::Null]), Value::I(7));
}

// P0-2：if is 模式绑定曾弹穿外层作用域（return x 解析不到）
#[test]
fn if_is_keeps_outer_scope() {
    let src = "fun f(x: int): int {\n    if x is 5 {\n    }\n    return x\n}";
    assert_eq!(run(src, "f", &[Value::I(5)]), Value::I(5));
}

// P0-3：for 的 continue 曾跳过自增死循环（解释器步数上限会抓到）
#[test]
fn for_continue_advances_loop() {
    let src = "fun f(): int {\n    var s: int = 0\n    for i in 1..=3 {\n        if i == 2 {\n            continue\n        }\n        s += i\n    }\n    return s\n}";
    assert_eq!(run(src, "f", &[]), Value::I(4));
}

// P0-4：copy_slots 槽类型越界（嵌套 struct 快照拷贝）
#[test]
fn nested_struct_snapshot_copy() {
    let src = "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    var p = P(1, 2)\n    var q = p\n    q.x = 9\n    return p.x * 10 + q.x\n}";
    assert_eq!(run(src, "f", &[]), Value::I(19));
}

// P0-5：嵌套聚合成员读取曾产垃圾
#[test]
fn nested_aggregate_member_read() {
    let src = "struct Inner {\n    y: int\n}\nstruct Outer {\n    inner: Inner\n}\nfun f(): int {\n    val o = Outer(Inner(3))\n    return o.inner.y\n}";
    assert_eq!(run(src, "f", &[]), Value::I(3));
}

// P0-6：聚合结果型的 if/when 表达式
#[test]
fn aggregate_if_and_when_values() {
    let src_if = "struct P {\n    x: int\n}\nfun f(c: bool): int {\n    val p = if c {\n        P(1)\n    } else {\n        P(2)\n    }\n    return p.x\n}";
    assert_eq!(run(src_if, "f", &[Value::B(true)]), Value::I(1));
    assert_eq!(run(src_if, "f", &[Value::B(false)]), Value::I(2));
    let src_when = "struct P {\n    x: int\n}\nfun g(x: int): int {\n    val p = when x {\n        0 -> P(10)\n        else -> P(20)\n    }\n    return p.x\n}";
    assert_eq!(run(src_when, "g", &[Value::I(0)]), Value::I(10));
    assert_eq!(run(src_when, "g", &[Value::I(5)]), Value::I(20));
}

// 缺点5：opt/llvme 的 API 级诊断门禁
#[test]
fn downstream_gates_on_diags() {
    let src = "fun f(): int {
    return 1
}";
    let parsed = parser::parse_source(src).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let mut tac = irc::lower(&mut sem_out, src);
    assert!(tac.diags.is_empty(), "{:?}", tac.diags);
    tac.diags.push(ast::syntax::node::Diagnostic::error(
        ast::syntax::node::DiagCode::IrcUnsupportedLowering,
        ast::token::Span { start: 0, end: 0 },
        "synthetic diagnostic for gate test",
    ));
    let before = format!("{:?}", tac.funs);
    opt::optimize(&mut tac, &sem_out.tys);
    assert_eq!(
        format!("{:?}", tac.funs),
        before,
        "optimize 须在有诊断时不动 TAC"
    );
    let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &mut sem_out.interner);
    assert_eq!(text, "", "llvme 须在有诊断时不发射");
}

#[test]
fn string_plus_concatenates_values() {
    let src = "fun f(): string {\n    return \"a\" + \"b\"\n}";
    assert_eq!(run(src, "f", &[]), Value::S("ab".to_string()));
}

// top-level globals persist across function entry and assignment
#[test]
fn top_level_globals_persist_and_update() {
    let src = "val G = 40\nvar H: int\nfun f(): int {\n    H = G + 2\n    return H\n}";
    assert_eq!(run(src, "f", &[]), Value::I(42));
}

// mutable destructuring writes through the original slots
#[test]
fn mutable_destructuring_assignment_updates_value() {
    let src = "fun f(): int {
    var (x, y) = (1, 2)
    x = 3
    return x + y
}";
    assert_eq!(run(src, "f", &[]), Value::I(5));
}

// loop expression: value exits through a shared result slot
#[test]
fn loop_expression_returns_break_value() {
    let src = "fun f(c: bool): int {
    val x = loop {
        if c {
            break 1
        } else {
            break 2
        }
    }
    return x
}";
    assert_eq!(run(src, "f", &[Value::B(true)]), Value::I(1));
    assert_eq!(run(src, "f", &[Value::B(false)]), Value::I(2));
}

// ── 位宽模型（2026-10-31）────────────────────────────────
// 整型按 `ty` 位宽回绕/规约，与 llvme 发出的 N 位 LLVM 指令一致。
// 旧扁平模型（一律按 i64）会算出真实代码给不出的值，也就抓不到
// 折叠位宽那类错编。

#[test]
fn int_arith_wraps_at_type_width() {
    // M1 后未约束字面量默认 i64：宽度意图用 as i32 钉住（i64 加法再截断）
    let src = "fun f(): i32 {\n    return (2000000000 + 2000000000) as i32\n}";
    assert_eq!(run(src, "f", &[]), Value::I(-294_967_296));
}

#[test]
fn int_div_sees_wrapped_operand() {
    let src = "fun f(): i32 {\n    return ((2000000000 + 2000000000) as i32) / (2 as i32)\n}";
    assert_eq!(run(src, "f", &[]), Value::I(-147_483_648));
}

#[test]
fn int_compare_sees_wrapped_operand() {
    let src = "fun f(): bool {\n    return ((2000000000 + 2000000000) as i32) < 0\n}";
    assert_eq!(run(src, "f", &[]), Value::B(true));
}

#[test]
fn arg_narrowed_to_param_width_then_sext_back() {
    // 200 进 i8 形参规约成 -56，as i32 符号扩展仍是 -56
    let src = "fun f(a: i8): i32 {\n    return a as i32\n}";
    assert_eq!(run(src, "f", &[Value::I(200)]), Value::I(-56));
}

#[test]
fn conv_narrows_to_target_width() {
    let src = "fun f(a: i64): i8 {\n    return a as i8\n}";
    assert_eq!(run(src, "f", &[Value::I(300)]), Value::I(44));
}

#[test]
fn float_conv_rounds_to_single_precision() {
    // f64 → f32 舍入：1 + 2^-24 在 f32 里是 1.0
    let src = "fun f(a: f64): f32 {\n    return a as f32\n}";
    assert_eq!(
        run(src, "f", &[Value::F(1.0 + 2f64.powi(-24))]),
        Value::F(1.0)
    );
}

#[test]
fn shift_out_of_type_width_traps() {
    // 移位量判据是操作数类型位宽；越界是 LLVM poison，解释器以 trap 表示。
    // M1 后未约束字面量默认 i64——宽度和越界意图用 as i32 钉住。
    let src = "fun f(): i32 {\n    return (1 as i32) << 40\n}";
    assert_eq!(
        run_res(src, "f", &[]),
        Err(InterpError::Trap("shift out of range"))
    );
}

#[test]
fn fptosi_out_of_range_traps() {
    // 1e30 落不进 i32：LLVM 是 poison，解释器不发明值
    let src = "fun f(a: f64): i32 {\n    return a as i32\n}";
    assert_eq!(
        run_res(src, "f", &[Value::F(1e30)]),
        Err(InterpError::Trap("fptosi out of range"))
    );
}
#[test]
fn tier0_scalar_conformance() {
    // P1-7 tier conformance 骨架：T0 每个标量类型种类经
    // parse → sem → irc → interp 往返保持值。新增标量类型时加行。
    let cases: &[(&str, &str, Value)] = &[
        ("i4", "7 as i4", Value::I(7)),
        ("i8", "-5 as i8", Value::I(-5)),
        ("i16", "10 as i16", Value::I(10)),
        ("i32", "2147483647", Value::I(2147483647)),
        ("i64", "9223372036854775807", Value::I(9223372036854775807)),
        ("int", "3000000000", Value::I(3000000000)),
        ("f32", "1.5 as f32", Value::F(1.5)),
        ("f64", "-0.25", Value::F(-0.25)),
        ("bool", "3 > 2", Value::B(true)),
        ("string", "\"ab\" + \"cd\"", Value::S("abcd".to_string())),
        ("char", "'x'", Value::I('x' as i64)),
    ];
    for (ty, expr, expected) in cases {
        let src = format!("fun probe() : {ty} = {expr}");
        let got = run(&src, "probe", &[]);
        assert_eq!(&got, expected, "tier0 conformance for {ty}: {src}");
    }
}

// M1（Q5）：受检算术溢出/除零 → interp trap；+% -% *% 回绕不 trap
#[test]
fn checked_arith_overflows_trap_and_wrapping_wraps() {
    // i64 形参 +1：MAX 溢出
    let src = "fun f(x: int): int {\n    return x + 1\n}";
    assert!(matches!(
        run_res(src, "f", &[Value::I(i64::MAX)]),
        Err(InterpError::Trap(_))
    ));
    // 窄位宽：i8 × i8 在 i8 域上溢出（两操作数皆窄 → join 不提升）
    let src8 = "fun f(x: i8, y: i8): int {\n    return x * y\n}";
    assert!(matches!(
        run_res(src8, "f", &[Value::I(100), Value::I(100)]),
        Err(InterpError::Trap(_))
    ));
    assert_eq!(run(src8, "f", &[Value::I(10), Value::I(10)]), Value::I(100));
    // 除零与 INT_MIN / -1
    let srcd = "fun f(x: int): int {\n    return x / 0\n}";
    assert!(matches!(
        run_res(srcd, "f", &[Value::I(1)]),
        Err(InterpError::Trap(_))
    ));
    // wrapping：i64 MAX +% 1 = MIN，不 trap
    let srcw = "fun f(x: int): int {\n    return x +% 1\n}";
    assert_eq!(run(srcw, "f", &[Value::I(i64::MAX)]), Value::I(i64::MIN));
}
