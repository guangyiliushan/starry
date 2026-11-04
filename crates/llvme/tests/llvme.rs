//! llvme 集成测试：parser → sem → irc → llvme 全链，.ll 金样 +
//! 可选 LLVM 验收层（优先 llvm-as，缺失时回落 clang，没有则跳过）。

fn emit(source: &str) -> (String, Vec<ast::syntax::node::DiagCode>) {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let mut codes: Vec<ast::syntax::node::DiagCode> =
        sem_out.diagnostics.iter().map(|d| d.code).collect();
    let tac = irc::lower(&mut sem_out, source);
    codes.extend(tac.diags.iter().map(|d| d.code));
    let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &sem_out.interner);
    (text, codes)
}

fn llvm_as_ok(text: &str) -> bool {
    let mut child = match std::process::Command::new("llvm-as")
        .arg("-")
        .arg("-o")
        .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => match std::process::Command::new("clang")
            .args(["-x", "ir", "-c", "-", "-o"])
            .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return true, // 可选验收层：无工具链即跳过（契约）
        },
    };
    use std::io::Write as _;
    let _ = child
        .stdin
        .as_mut()
        .expect("piped")
        .write_all(text.as_bytes());
    child.wait().map(|s| s.success()).unwrap_or(false)
}

fn assert_ll(source: &str, expected: &str) {
    let (text, codes) = emit(source);
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert_eq!(text, expected);
}

#[test]
fn arith_return_emits_define() {
    assert_ll(
        "fun f(): int {\n    return 0x1F\n}",
        "define i64 @f.16() {\n  %t0.slot = alloca i64\n  br label %b0\nb0:\n  ret i64 31\n}\n\n",
    );
}

#[test]
fn struct_slots_and_field_load() {
    let (text, codes) = emit(
        "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    val p = P(1, 2)\n    return p.x\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("%t0.slot = alloca [2 x i64]")
            && text.contains("store i64 1, ptr %u0")
            && text.contains("ret i64"),
        "{text}"
    );
}

#[test]
fn scalar_copy_dynamic_index_and_conv_are_typed() {
    let (text, codes) = emit(
        "fun f(i: i32): int {\n    val a = [10, 20, 30]\n    val j = i\n    return a[j] as int\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("load i32, ptr %t0.slot")
            && text.contains("store i32")
            && text.contains("getelementptr [3 x i64]")
            && text.contains("sext i32"),
        "{text}"
    );
}

#[test]
fn enum_param_pointer_and_tag_compare() {
    let (text, codes) = emit(
        "enum E {\n    A\n    B\n}\nfun f(e: E): int {\n    when e {\n        E.A -> 1\n        E.B -> 2\n    }\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("define i64 @f.19(ptr %t0)")
            && text.contains("store ptr %t0, ptr %t0.slot")
            && text.contains("icmp eq i8")
            && text.contains("unreachable"),
        "{text}"
    );
}

#[test]
fn float_literals_emit_llvm_bit_patterns() {
    let (f64_text, codes) = emit("fun f(): f64 {\n    return 1.5\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&f64_text) {
        panic!("LLVM validator rejected:\n{f64_text}");
    }
    assert!(
        f64_text.contains("ret double 0x3FF8000000000000"),
        "{f64_text}"
    );

    // f32 常量经数组元素类型触发 const_val 的 f32 位模式路径。
    let (f32_text, codes) = emit(
        "fun f(): int {
    val a: [f32; 1] = [1.5 as f32]
    return 0
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&f32_text) {
        panic!(
            "LLVM validator rejected:
{f32_text}"
        );
    }
    assert!(
        f32_text.contains("store float 0x3FF8000000000000"),
        "{f32_text}"
    );
}

#[test]
fn aggregate_arguments_returns_and_current_var_slots_emit_valid_ir() {
    let cases = [
        "struct P {\n    x: int\n    y: int\n}\nfun id(p: P): P {\n    return p\n}\nfun f(): int {\n    var p = P(1, 2)\n    p.x = 9\n    return id(p).x\n}",
        "fun make(): [int; 3] {\n    return [1, 2, 3]\n}\nfun first(a: [int; 3]): int {\n    return a[0]\n}\nfun f(): int {\n    return first(make())\n}",
        "fun pair(): (int, int) {\n    return (1, 2)\n}\nfun fst(t: (int, int)): int {\n    return t[0]\n}\nfun f(): int {\n    return fst(pair())\n}",
        "enum E {\n    A\n    B\n}\nfun tag(e: E): int {\n    when e {\n        E.A -> 1\n        E.B -> 2\n    }\n}\nfun f(): int {\n    return tag(E.B)\n}",
    ];
    for source in cases {
        let (text, codes) = emit(source);
        assert_eq!(codes, Vec::new(), "{codes:?}\n{text}");
        if !llvm_as_ok(&text) {
            panic!("LLVM validator rejected:\n{text}");
        }
    }
}

#[test]
fn print_runtime_declares_and_calls_void() {
    let (text, codes) = emit(
        "fun main() {
    print(42)
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("declare void @starry_print_i32(i32)")
            && text.contains("call void @starry_print_i32"),
        "{text}"
    );
}

#[test]
fn main_wrapper_emitted() {
    let (text, codes) = emit("fun main() {\n}\n");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("define i32 @main()")
            && text.contains("call void @main")
            && text.contains("ret i32 0"),
        "{text}"
    );
}

#[test]
fn string_interp_runtime_declares() {
    let (text, codes) = emit("fun f(x: int): string {\n    return \"a${x}b\"\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("declare i8* @starry_str_conv_i64(i64)")
            && text.contains("declare i8* @starry_str_concat(i8*, i8*)")
            && text.contains("call i8* @starry_str_concat"),
        "{text}"
    );
}

#[test]
fn pattern_completion_and_aggregate_loop_emit_valid_ir() {
    let cases = [
        "fun f(a: [int; 4]): int {
    if a is [x, .., y] {
        return x + y
    }
    return 0
}",
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
        "fun f(): int {
    val (x, y) = (1, 2)
    return x + y
}",
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
        "struct P {
    x: int
    y: int
}
fun f(a: [P; 2], x: P): bool {
    return x in a
}",
        "struct P {
    x: int
    y: int
}
fun f(): int {
    var a = [P(1, 2), P(3, 4)]
    a[1] = P(9, 8)
    return a[1].x
}",
    ];
    for source in cases {
        let (text, codes) = emit(source);
        assert_eq!(
            codes,
            Vec::new(),
            "{codes:?}
{text}"
        );
        if !llvm_as_ok(&text) {
            panic!(
                "LLVM validator rejected:
{text}"
            );
        }
    }
}

#[test]
fn tuple_subscript_emits_valid_ir() {
    let (text, codes) = emit(
        "fun f(): int {
    val t = (1, 2)
    t[1] = 9
    return t[1]
}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(text.contains("getelementptr [2 x i64]"), "{text}");
}

#[test]
fn loop_expression_break_value_emits_valid_ir() {
    let (text, codes) = emit(
        "fun f(c: bool): int {
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
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("store i32 1") && text.contains("store i32 2"),
        "{text}"
    );
}
