//! 语义级回归（TAC 解释器执行，断言「跑出来的结果」）：对应审查报告的
//! 六个 P0 形态——快照测试抓不到的语义错编，在这里全部变成结果断言。

use irc::interp::{Value, run_named};
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
    let src = "fun f(): int {\n    val (x, y) = (1, 2)\n    return x\n}";
    let (mut tac, sem_out, _) = {
        let parsed = parser::parse_source(src).expect("machine parse");
        let mut sem_out = sem::analyze(parsed);
        let tac = irc::lower(&mut sem_out, src);
        (tac, sem_out, ())
    };
    assert!(!tac.diags.is_empty());
    let before = format!("{:?}", tac.funs);
    opt::optimize(&mut tac);
    assert_eq!(
        format!("{:?}", tac.funs),
        before,
        "optimize 须在有诊断时不动 TAC"
    );
    let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &sem_out.interner);
    assert_eq!(text, "", "llvme 须在有诊断时不发射");
}
