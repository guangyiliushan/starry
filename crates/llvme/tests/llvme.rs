//! llvme 集成测试：parser → sem → irc → llvme 全链，.ll 金样 +
//! 可选 LLVM 验收层（优先 llvm-as，缺失时回落 clang，没有则跳过）。

fn emit(source: &str) -> (String, Vec<ast::syntax::node::DiagCode>) {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    let mut codes: Vec<ast::syntax::node::DiagCode> =
        sem_out.diagnostics.iter().map(|d| d.code).collect();
    let tac = irc::lower(&mut sem_out, source);
    codes.extend(tac.diags.iter().map(|d| d.code));
    let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &mut sem_out.interner);
    (text, codes)
}

fn llvm_tool_path(name: &str) -> Option<std::path::PathBuf> {
    if std::process::Command::new(name)
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Some(std::path::PathBuf::from(name));
    }
    for dir in [
        "C:\\Program Files\\LLVM\\bin",
        "C:\\Program Files (x86)\\LLVM\\bin",
    ] {
        let candidate = std::path::Path::new(dir).join(format!("{name}.exe"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn llvm_as_ok(text: &str) -> bool {
    let tool = llvm_tool_path("llvm-as")
        .map(|path| (path, false))
        .or_else(|| llvm_tool_path("clang").map(|path| (path, true)));
    let Some((program, is_clang)) = tool else {
        eprintln!(
            "SKIP: LLVM validator requires llvm-as or clang on PATH (checked C:\\Program Files\\LLVM\\bin)"
        );
        return true;
    };
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = std::process::Command::new(program);
    if is_clang {
        command.args(["-x", "ir", "-c", "-", "-o"]);
    } else {
        command.args(["-", "-o"]);
    }
    let mut child = command
        .arg(null_device)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("validator discovered but failed to spawn");
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
        // M1 后数组字面量默认 i64——本测的 sext 意图用 as i32 钉元素宽度
        "fun f(i: i32): int {\n    val a = [10 as i32, 20 as i32, 30 as i32]\n    val j = i\n    return a[j] as int\n}",
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
        // M1：42 未约束 → i64
        text.contains("declare void @starry_print_i64(i64)")
            && text.contains("call void @starry_print_i64"),
        "{text}"
    );
}

#[test]
fn string_plus_concat_emits_valid_ir() {
    let (text, codes) = emit("fun f(): string {\n    return \"a\" + \"b\"\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("declare i8* @starry_str_concat(i8*, i8*)")
            && text.contains("call i8* @starry_str_concat"),
        "{text}"
    );
}

#[test]
fn top_level_globals_emit_storage_init_and_valid_ir() {
    let (text, codes) =
        emit("val G = 40\nvar H: int\nfun main() {\n    H = G + 2\n    print(H)\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains(" = global i64 0")
            && text.contains("@__starry_init_global.")
            && text.contains("store i64")
            && text.contains("call void @starry_print_i64"),
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
    var t = (1, 2)
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
        // M1：break 字面量默认 i64
        text.contains("store i64 1") && text.contains("store i64 2"),
        "{text}"
    );
}

#[test]
fn single_slot_aggregate_var_field_assignment_emits_valid_ir() {
    // 回归：单字段 struct 的 var 字段赋值。槽数 1 的聚合也是「基址 + 槽」，
    // 曾把基址当 i64 值存（`store i64 %t0.slot`，ptr 当 i64）。
    let (text, codes) = emit(
        "struct P {\n    x: int\n}\nfun f(): int {\n    var p = P(1)\n    p.x = 50\n    return p.x\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        !text.contains("store i64 %t0.slot"),
        "aggregate base stored as an i64 value:\n{text}"
    );
}

#[test]
fn float_constants_use_llvm_bit_pattern() {
    // 回归：浮点常量曾发 C99 十六进制浮点（0x1.8p0），LLVM 要的是位模式
    // 十六进制（0x3FF8000000000000）。
    let (text, codes) = emit("fun f(): f64 {\n    return 3.25\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(text.contains("0x400A000000000000"), "{text}");
}

#[test]
fn i128_aggregates_use_paired_i64_slots() {
    // S3/P3-2：聚合内 i128 = lo/hi 两个 i64 槽（i128-layout-spec）；
    // 标量 i128 仍是 alloca i128；任何聚合都不再出现 [k x i128]
    let (text, codes) = emit("fun f(x: i128) : i128 {\n    return x + 1\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(text.contains("%t0.slot = alloca i128"), "{text}");

    let (wide, codes) =
        emit("struct W { a: i128, b: int }\nfun g(w: W) : int {\n    return w.b\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&wide) {
        panic!("LLVM validator rejected:\n{wide}");
    }
    // a=i128 占槽 0,1（lo/hi）→ b 在槽 2；形参聚合 = ptr 槽 + [3 x i64] 块
    assert!(wide.contains("alloca ptr"), "{wide}");
    assert!(
        wide.contains("getelementptr [3 x i64], ptr %u0, i64 0, i32 2"),
        "{wide}"
    );
    assert!(!wide.contains("x i128"), "{wide}");

    let (narrow, codes) =
        emit("struct N { a: int, b: int }\nfun h(n: N) : int {\n    return n.b\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&narrow) {
        panic!("LLVM validator rejected:\n{narrow}");
    }
    assert!(narrow.contains("getelementptr [2 x i64]"), "{narrow}");
    assert!(!narrow.contains("x i128"), "{narrow}");
}
#[test]
fn extern_pointer_params_emit_ptr_signature() {
    // D-007 slice：裸指针类型在 extern 声明的 LLVM 签名是 ptr（不透明指针）
    // declare 在调用点发射：程序必须真的调用 extern
    let (text, codes) = emit(
        "extern \"c\" fun c_log(p: *const int, q: *mut int) : int\nfun f() : int {\n    return unsafe { c_log(null, null) }\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("declare i64 @c_log(ptr, ptr)"), // : int = i64
        "pointer extern signature missing:\n{text}"
    );
}

#[test]
fn checked_arith_emits_overflow_intrinsic_and_panic_decl() {
    // M1（Q5）：整型 + - * 发 llvm.*.with.overflow 受检序列 + panic 声明；
    // 窄位宽（i8 形参 × i8 形参）按 i8 内联
    let (text, codes) = emit("fun f(x: i8, y: i8): int {\n    return x * y\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("@llvm.smul.with.overflow.i8") && text.contains("starry_overflow_panic"),
        "{text}"
    );

    // 整型除法：守卫在 sdiv 之前（除零/INT_MIN÷-1 不落到硬件异常）
    let (text, codes) = emit("fun f(x: int): int {\n    return x / 1\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("sdiv") && text.contains("starry_overflow_panic"),
        "{text}"
    );

    // wrapping 算子：裸指令，无受检序列
    let (text, codes) = emit("fun f(x: int, y: int): int {\n    return x +% y\n}");
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("add i64") && !text.contains("with.overflow"),
        "{text}"
    );
}

#[test]
fn dyn_class_vtable_layout_snapshot() {
    // Phase 2a ABI: three-pointer header, declaration-ordered methods, and
    // data/vtable fat-pointer slots.
    let (text, codes) = emit(
        "trait T {\n    fun first(): int\n}\nclass C(val v: int)\nimpl T for C {\n    fun first(): int {\n        return self.v\n    }\n}\nfun main(): int {\n    val d: dyn T = C(7)\n    return d.first()\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("@vtable.T.16.C.19 = private constant [4 x ptr] [ptr null, ptr inttoptr (i64 24 to ptr), ptr inttoptr (i64 8 to ptr), ptr @first.")
            && text.contains("getelementptr i8, ptr %u23, i64 24")
            && text.contains("alloca [2 x i64]"),
        "{text}"
    );
}

#[test]
fn class_typeinfo_and_vtable_snapshot() {
    // Phase 3a: every class gets a stable TypeInfo header record and a
    // declaration-ordered class virtual table. Virtual dispatch reads the
    // TypeInfo from the payload-base header and calls through slot zero.
    let (text, codes) = emit(
        "open class Base(val x: int) {\n    open fun who(): int {\n        return self.x\n    }\n}\nfun main(): int {\n    val b = Base(7)\n    print(b.who())\n    return 0\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("@classvtable.Base.16 = private constant [1 x ptr] [ptr @who.")
            && text.contains("@typeinfo.Base.16 = private constant { ptr, ptr, i64, ptr } { ptr null, ptr @classvtable.Base.16, i64 16, ptr null }")
            && text.contains("@starry_alloc_object(i64 32, ptr @typeinfo.Base.16)")
            && text.contains("call i64 %"),
        "{text}"
    );
}
#[test]
fn class_object_emits_alloc_retain_release() {
    // M4 slice 1：class 对象 = alloc + 复制 retain + 块出口隐式 release；
    // 字段经 24B header 偏移（offsetptr/GEP i8）
    let (text, codes) = emit(
        "class P(val x: int, var y: int)\nfun main() : int {\n    val a = P(1, 2)\n    val b = a\n    b.y = 9\n    return a.x + b.y\n}",
    );
    assert_eq!(codes, Vec::new(), "{codes:?}");
    if !llvm_as_ok(&text) {
        panic!("LLVM validator rejected:\n{text}");
    }
    assert!(
        text.contains("starry_alloc_object")
            && text.contains("starry_retain")
            && text.contains("starry_release")
            && text.contains("getelementptr i8"),
        "{text}"
    );
    // 头 24B：第一个字段（槽 0）偏移 24
    assert!(text.contains("getelementptr i8, ptr %u"), "{text}");
}
