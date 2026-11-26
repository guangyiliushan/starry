#![cfg(target_os = "linux")]
//! P0 Task 8：生成的 Starry 程序在 Linux 上的 ASan/UBSan 门禁。
//!
//! 范围：只 sanitize 生成的原生程序（clang -fsanitize=address,undefined），
//! 不 sanitize Rust 编译器本身（stable Rust 不用 nightly sanitizer）。
//! Windows freestanding runtime（HeapAlloc 路径）不支持 ASan——整个测试
//! 文件 cfg 到 Linux；工具链缺失时显式 SKIP，不允许假绿。

use std::process::Command;

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// 源码 → ASan/UBSan 构建 → 运行 → (stdout, stderr, exit code)。
/// None = 无工具链，调用方跳过（显式 SKIP 行）。
fn build_sanitized(name: &str, source: &str) -> Option<(String, String, i32)> {
    if !clang_available() {
        eprintln!("SKIP: sanitizer harness requires clang on PATH");
        return None;
    }
    let dir =
        std::env::temp_dir().join(format!("starry_sanitizer_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join(format!("{name}.starry"));
    std::fs::write(&src, source).unwrap();
    let exe = dir.join(name);
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args([
            "build",
            src.to_str().unwrap(),
            exe.to_str().unwrap(),
            "--",
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "sanitized build failed\nstderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe)
        .env("ASAN_OPTIONS", "detect_leaks=1:halt_on_error=1")
        .env("UBSAN_OPTIONS", "halt_on_error=1")
        .output()
        .unwrap();
    let code = run.status.code().unwrap_or(-1);
    let out = String::from_utf8_lossy(&run.stdout).into_owned();
    let err = String::from_utf8_lossy(&run.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    Some((out, err, code))
}

fn assert_sanitizer_clean(name: &str, source: &str, expected_stdout: &str) {
    let Some((out, err, code)) = build_sanitized(name, source) else {
        return;
    };
    assert_eq!(code, 0, "{name}: exit {code}\nstdout: {out}\nstderr: {err}");
    assert_eq!(out, expected_stdout, "{name}: stdout");
    assert!(
        !err.contains("AddressSanitizer"),
        "{name}: ASan report\nstderr: {err}"
    );
    assert!(
        !err.contains("runtime error:"),
        "{name}: UBSan report\nstderr: {err}"
    );
    assert!(
        !err.contains("LeakSanitizer"),
        "{name}: leak report\nstderr: {err}"
    );
}

#[test]
fn sanitizer_class_arc_corpus() {
    // class 构造/复制/weak 读取的 ARC 平衡（deinit 恰一次，无泄漏）
    assert_sanitizer_clean(
        "san_class_arc",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    val a = C(1)\n    val b = a\n    print(b.x)\n    return 0\n}\n",
        "1\n1\n",
    );
}

#[test]
fn sanitizer_aggregate_corpus() {
    // 纯值聚合/数组/字符串的分配-释放路径
    assert_sanitizer_clean(
        "san_aggregate",
        "struct P {\n    x: int,\n    y: int\n}\nfun main(): int {\n    val a = [P(1, 2), P(3, 4)]\n    print(a[1].y)\n    val s = \"a\" + \"b\"\n    print(s)\n    return 0\n}\n",
        "4\nab\n",
    );
}

#[test]
fn sanitizer_dyn_class_corpus() {
    // Phase 2a: non-null dyn, nullable dyn call, null propagation, and
    // shared-payload reassignment release exactly once.
    assert_sanitizer_clean(
        "san_dyn_class",
        "trait T {\n    fun num(): int\n}\nclass C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nimpl T for C {\n    fun num(): int {\n        return self.x + 5\n    }\n}\nfun main(): int {\n    val d: dyn T = C(2)\n    val n: ?dyn T = d\n    print(n?.num()!!)\n    var r: dyn T = C(3)\n    r = d\n    print(r.num())\n    return 0\n}\n",
        "7\n3\n7\n2\n",
    );
}

#[test]
fn sanitizer_class_inheritance_corpus() {
    // Inherited payload prefix, ctor/deinit chaining, and shared derived->base
    // references must stay allocation balanced.
    assert_sanitizer_clean(
        "san_class_inheritance",
        "open class Base(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nopen class Derived(val y: int): Base(y * 10) {\n    deinit {\n        print(self.y)\n    }\n}\nfun main(): int {\n    val b: Base = Derived(3)\n    val d: Base = Derived(4)\n    print(99)\n    return 0\n}\n",
        "99\n4\n40\n3\n30\n",
    );
}

#[test]
fn sanitizer_abstract_override_corpus() {
    // Abstract declarations allocate no runtime representation; the concrete
    // override owns the vtable entry used through the base receiver.
    assert_sanitizer_clean(
        "san_abstract_override",
        "abstract class Shape(val name: int) {\n    abstract fun area(): int;\n}\nopen class Square(val side: int): Shape(side) {\n    override fun area(): int {\n        return self.side * self.side\n    }\n}\nfun main(): int {\n    val shape: Shape = Square(4)\n    print(shape.area())\n    return 0\n}\n",
        "16\n",
    );
}

#[test]
fn sanitizer_struct_arc_corpus() {
    // struct fields recursively retain/release class payloads.
    assert_sanitizer_clean(
        "san_struct_arc",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nstruct S(val c: C)\nfun main(): int {\n    val s = S(C(1))\n    print(s.c.x)\n    val b = s\n    print(b.c.x)\n    return 0\n}\n",
        "1\n1\n1\n",
    );
}

#[test]
fn sanitizer_enum_payload_corpus() {
    // Tag-aware traversal releases only the active ARC payload.
    assert_sanitizer_clean(
        "san_enum_payload",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nenum E {\n    Has(C)\n    Empty\n}\nfun main(): int {\n    val e = E.Has(C(1))\n    val f = e\n    print(9)\n    return 0\n}\n",
        "9\n1\n",
    );
}

#[test]
fn sanitizer_nullable_abi_corpus() {
    // Nullable class and i128 pointer pointees use multi-slot read/write ABI.
    assert_sanitizer_clean(
        "san_nullable_abi",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun class_part(): int {\n    var x: C? = C(1)\n    val p = &x\n    val y = *p\n    print(y!!.x)\n    val q = &mut x\n    *q = C(2)\n    print(x!!.x)\n    return 0\n}\nfun i128_part(): int {\n    var x: i128? = 123456789012345678\n    val p = &x\n    val y = *p\n    print(y!!)\n    val q = &mut x\n    *q = 123456789012345679\n    print(x!!)\n    *q = null\n    if (x != null) {\n        print(1)\n    } else {\n        print(9)\n    }\n    return 0\n}\nfun main(): int {\n    class_part()\n    i128_part()\n    return 0\n}\n",
        "1\n2\n2\n123456789012345678\n123456789012345679\n9\n",
    );
}

#[test]
fn sanitizer_aggregate_closure_corpus() {
    // struct/tuple/array capture environments recursively release ARC leaves.
    assert_sanitizer_clean(
        "san_aggregate_closure",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nstruct S(val c: C)\nfun call(f: () -> int): int {\n    return f()\n}\nfun main(): int {\n    val c = C(1)\n    val s = S(c)\n    val fs = { () -> s.c.x }\n    print(call(fs))\n    val t = (C(2), 20)\n    val ft = move { () -> t[0].x }\n    print(call(ft))\n    val a = [C(3)]\n    val fa = { () -> a[0].x }\n    print(call(fa))\n    return 0\n}\n",
        "1\n2\n3\n3\n2\n1\n",
    );
}

#[test]
fn sanitizer_class_reassignment_corpus() {
    // Plain and nullable class var reassignment release the old payload and
    // retain/transfer the new one exactly once.
    assert_sanitizer_clean(
        "san_class_reassign",
        "class P(val x: int) {\n}\nfun main(): int {\n    var b = P(1)\n    if (true) {\n        val a = P(2)\n        b = a\n    }\n    b = P(3)\n    b = b\n    return 0\n}\n",
        "",
    );
    assert_sanitizer_clean(
        "san_nullable_class_reassign",
        "class P(val x: int) {\n}\nfun main(): int {\n    var b: P? = null\n    b = P(1)\n    b = null\n    return 0\n}\n",
        "",
    );
}

#[test]
fn sanitizer_fn_reassignment_corpus() {
    // Fn var reassignment releases the old box and transfers the captured
    // class value; the final binding release triggers deinit once.
    assert_sanitizer_clean(
        "san_fn_reassign",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    val k = 10\n    var f: () -> int = { () -> k }\n    print(f())\n    val c = C(7)\n    f = { () -> c.x }\n    print(f())\n    return 0\n}\n",
        "10\n7\n7\n",
    );
}

#[test]
fn sanitizer_closure_corpus() {
    // 闭包 box ARC：捕获 class 的 env 释放平衡
    assert_sanitizer_clean(
        "san_closure",
        "class C(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    val c = C(7)\n    val f: () -> int = { () -> c.x }\n    print(f())\n    return 0\n}\n",
        "7\n7\n",
    );
}
