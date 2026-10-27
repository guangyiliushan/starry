//! llvme 集成测试：parser → sem → irc → llvme 全链，.ll 金样 +
//! 可选 llvm-as 验收层（机器有 LLVM 工具链则语法校验，没有则跳过）。

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
        Err(_) => return true, // 可选验收层：无工具链即跳过（契约）
    };
    use std::io::Write as _;
    let _ = child
        .stdin
        .as_mut()
        .expect("piped")
        .write_all(text.as_bytes());
    child.wait().map(|s| s.success()).unwrap_or(true)
}

fn assert_ll(source: &str, expected: &str) {
    let (text, codes) = emit(source);
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("llvm-as rejected:\n{text}");
    }
    assert_eq!(text, expected);
}

#[test]
fn arith_return_emits_define() {
    assert_ll(
        "fun f(): int {\n    return 0x1F\n}",
        "define i64 @f.15() {\n  %t0.slot = alloca i64\nb0:\n  ret i64 31\n}\n\n",
    );
}

#[test]
fn struct_slots_and_field_load() {
    let (text, codes) = emit(
        "struct P {\n    x: int\n    y: int\n}\nfun f(): int {\n    val p = P(1, 2)\n    return p.x\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("llvm-as rejected:\n{text}");
    }
    assert!(
        text.contains("%t0.slot = alloca [2 x i64]")
            && text.contains("store i64 1, ptr %u0")
            && text.contains("ret i64"),
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
        panic!("llvm-as rejected:\n{text}");
    }
    assert!(
        text.contains("define i64 @f.18(ptr %t0)")
            && text.contains("store ptr %t0, ptr %t0.slot")
            && text.contains("icmp eq i8")
            && text.contains("unreachable"),
        "{text}"
    );
}

#[test]
fn main_wrapper_emitted() {
    let (text, codes) = emit("fun main() {\n}\n");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("llvm-as rejected:\n{text}");
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
        panic!("llvm-as rejected:\n{text}");
    }
    assert!(
        text.contains("declare i8* @starry_str_conv.i64(i64)")
            && text.contains("declare i8* @starry_str_concat(i8*, i8*)")
            && text.contains("call i8* @starry_str_concat"),
        "{text}"
    );
}
