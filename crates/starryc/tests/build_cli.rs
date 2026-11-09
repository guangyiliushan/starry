//! End-to-end `starryc build` test: source -> native exe -> stdout.
//! Skips cleanly when clang is not on PATH (runtime validator policy).

use std::path::PathBuf;
use std::process::Command;

fn clang_available() -> bool {
    if Command::new("clang")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return true;
    }
    [
        r"C:\Program Files\LLVM\bin",
        r"C:\Program Files (x86)\LLVM\bin",
    ]
    .iter()
    .any(|dir| std::path::Path::new(dir).join("clang.exe").is_file())
}

/// 源码 → 原生可执行 → (stdout, exit code)。None = 无工具链，调用方跳过。
/// (stdout, stderr, exit code)。None = 无工具链，调用方跳过。
fn build_and_exec(name: &str, source: &str) -> Option<(String, String, i32)> {
    if !clang_available() {
        eprintln!("SKIP: starryc build e2e requires clang (PATH or C:\\Program Files\\LLVM\\bin)");
        return None;
    }
    let dir =
        std::env::temp_dir().join(format!("starry_build_test_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let source_path = dir.join(format!("{name}.starry"));
    std::fs::write(&source_path, source).unwrap();
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join(format!("{name}.exe"))
    } else {
        dir.join(name)
    };

    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args([
            "build",
            source_path.to_str().unwrap(),
            exe_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&exe_path).output().unwrap();
    let code = run.status.code().unwrap_or(-1);
    let out = String::from_utf8_lossy(&run.stdout).into_owned();
    let err = String::from_utf8_lossy(&run.stderr).into_owned();

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&exe_path);
    let _ = std::fs::remove_dir_all(&dir);
    Some((out, err, code))
}

/// 源码 → 原生可执行 → stdout。返回 None 表示无工具链，调用方跳过。
fn build_and_run(name: &str, source: &str) -> Option<String> {
    let (out, _err, code) = build_and_exec(name, source)?;
    assert_eq!(code, 0, "run failed with exit {code}");
    Some(out)
}

#[test]
fn build_print_program_and_run_it() {
    let Some(out) = build_and_run("hello", "fun main() {\n    print(\"hi ${41 + 1}\")\n}\n") else {
        return;
    };
    assert_eq!(out, "hi 42\n");
}

#[test]
fn float_formatting_keeps_the_decimal_point() {
    // 回归：runtime 的 fmt_double 曾从不输出小数点（3.25 → "325"、
    // 0.5 → "5e-1"），只在 LLVM IR 层校验看不见——必须跑起来断言 stdout。
    let Some(out) = build_and_run(
        "floats",
        "fun main() {\n\
             print(3.25)\n\
             print(-3.25)\n\
             print(0.5)\n\
             print(100.0)\n\
             print(1.0 / 3.0)\n\
             print(123456789.0)\n\
             print(-0.25)\n\
             print(25000000000.0)\n\
             print(0.0001)\n\
             print(0.00001)\n\
             print(1.0)\n\
             print(0.0)\n\
             print(9.87654321)\n\
             val f = 3.25\n\
             print(\"v=${f}\")\n\
         }\n",
    ) else {
        return;
    };
    assert_eq!(
        out,
        "3.25\n\
         -3.25\n\
         0.5\n\
         100\n\
         0.333333333333333\n\
         123456789\n\
         -0.25\n\
         25000000000\n\
         0.0001\n\
         1e-5\n\
         1\n\
         0\n\
         9.87654321\n\
         v=3.25\n"
    );
}

#[test]
fn single_slot_struct_var_field_assignment() {
    // 回归：单字段 struct 的 var 字段赋值。槽数 1 的聚合也曾被折成标量，
    // 于是把基址当 i64 存、成员读取又把该值当基址用。
    let Some(out) = build_and_run(
        "varagg",
        "struct P { x: int }\n\
         fun use(p: P) : int { return p.x }\n\
         fun main() {\n\
             var p = P(1)\n\
             p.x = 50\n\
             print(p.x)\n\
             print(use(p))\n\
             val q = P(7)\n\
             print(q.x)\n\
         }\n",
    ) else {
        return;
    };
    assert_eq!(out, "50\n50\n7\n");
}

#[test]
fn aggregate_returns_keep_their_value_across_calls() {
    // 回归：聚合返回值曾指向 callee 栈槽，第二次调用后第一次的副本被覆写。
    let source = "struct P { x: int, y: int }\n\
        fun make(n: int) : P = P(x = n, y = n + 1)\n\
        fun main() {\n\
            val p = make(1)\n\
            print(p.x)\n\
            print(p.y)\n\
            val q = make(100)\n\
            print(p.x)\n\
            print(q.x)\n\
            print(\"ab\" == \"a\" + \"b\")\n\
            print(\"ab\" != \"a\" + \"b\")\n\
            print(p == P(1, 2))\n\
            print(p == P(1, 3))\n\
        }\n";
    let Some(out) = build_and_run("aggret", source) else {
        return;
    };
    assert_eq!(out, "1\n2\n1\n100\ntrue\nfalse\ntrue\nfalse\n");
}

#[test]
fn globals_defaults_patterns_and_scalar_widths() {
    // 回归：全局依赖序、默认实参定型、if val 构造器绑定、for 元组模式、
    // 大整数字面量、i16/i128/char 的 runtime 入口。
    let source = "enum Opt { None, Some(int) }\n\
        val b: int = a + 3\n\
        val a: int = 2\n\
        fun helper() : int = 40\n\
        fun bumped(n: int = helper()) : int = n + 1\n\
        fun inside(x: int, lo: int, hi: int) : bool = x in lo..hi\n\
        fun unwrap(o: Opt) : int {\n\
            if val Some(v) = o { return v }\n\
            return -1\n\
        }\n\
        fun main() {\n\
            print(b)\n\
            print(bumped())\n\
            print(inside(3, 1, 5))\n\
            print(inside(7, 1, 5))\n\
            print(unwrap(Opt.Some(9)))\n\
            print(unwrap(Opt.None))\n\
            val pairs: [(int, int); 2] = [(1, 2), (3, 4)]\n\
            for (x, y) in pairs { print(x + y) }\n\
            print(3000000000)\n\
            print(10 as i16)\n\
            print(12 as i128)\n\
            print('a')\n\
            print('你')\n\
        }\n";
    let Some(out) = build_and_run("features", source) else {
        return;
    };
    assert_eq!(
        out,
        "5\n41\ntrue\nfalse\n9\n-1\n3\n7\n3000000000\n10\n12\na\n你\n"
    );
}

#[test]
fn materialized_ranges_keep_their_exclusivity() {
    let source = "fun main() {\n\
            val r = 1..5\n\
            print(5 in r)\n\
            print(4 in r)\n\
            val s = 1..=5\n\
            print(5 in s)\n\
            for x in r { print(x) }\n\
            for x in s { print(x) }\n\
        }\n";
    let Some(out) = build_and_run("ranges", source) else {
        return;
    };
    assert_eq!(out, "false\ntrue\ntrue\n1\n2\n3\n4\n1\n2\n3\n4\n5\n");
}

#[test]
fn main_return_value_is_the_process_exit_code() {
    let Some((out, _err, code)) =
        build_and_exec("exitcode", "fun main() : int {\n    return 42\n}\n")
    else {
        return;
    };
    assert_eq!(out, "");
    assert_eq!(code, 42);
}

#[test]
fn extern_c_function_links_against_kernel32() {
    if !cfg!(windows) {
        return;
    }
    let source = "extern \"c\" fun GetTickCount() : int\n\
        fun main() {\n\
            print(unsafe { GetTickCount() > 0 })\n\
        }\n";
    let Some(out) = build_and_run("extern", source) else {
        return;
    };
    assert_eq!(out, "true\n");
}

#[test]
fn nested_aggregate_equality_compares_every_field() {
    let source = "struct Inner { a: int, s: string }\n\
        struct Outer { inner: Inner, b: int }\n\
        enum E { A, B(Inner) }\n\
        fun main() {\n\
            val x = Outer(inner = Inner(a = 1, s = \"hi\"), b = 2)\n\
            print(x == Outer(inner = Inner(a = 1, s = \"hi\"), b = 2))\n\
            print(x == Outer(inner = Inner(a = 1, s = \"bye\"), b = 2))\n\
            print(E.B(Inner(1, \"z\")) == E.B(Inner(1, \"z\")))\n\
            print(E.B(Inner(1, \"z\")) == E.B(Inner(2, \"z\")))\n\
        }\n";
    let Some(out) = build_and_run("nestedeq", source) else {
        return;
    };
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

#[test]
fn string_globals_membership_and_folded_in() {
    // 回归：字符串全局初始化曾引用未 flush 的 @.str.N；数组 in 曾按指针
    // 比较字符串元素；常量折叠曾把比较结果按操作数类型写成 i32。
    let source = "val greeting: string = \"hi\"\n\
        fun main() {\n\
            print(greeting)\n\
            print(\"a\" in [\"a\", \"b\"])\n\
            print(\"z\" in [\"a\", \"b\"])\n\
            print(2 in 1..5)\n\
            print(2 in 1..=1)\n\
            val xs: [int; 2] = [1, 2]\n\
            print(xs == [1, 2])\n\
            print(xs == [1, 3])\n\
            val maybe: int? = null\n\
            print(maybe == null)\n\
        }\n";
    let Some(out) = build_and_run("strings", source) else {
        return;
    };
    assert_eq!(out, "hi\ntrue\nfalse\ntrue\nfalse\ntrue\nfalse\ntrue\n");
}

#[test]
fn build_without_main_reports_a_diagnostic() {
    if !clang_available() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("starry_no_main_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("no_main.starry");
    std::fs::write(&path, "fun f() : int { return 1 }\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["build", path.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("缺少入口函数"), "{err}");
}

#[test]
fn i128_aggregate_members_use_wide_slots() {
    let source = "struct W { a: i128, b: int }\n\
        fun make() : W = W(a = 11 as i128, b = 7)\n\
        fun main() {\n\
            val w = make()\n\
            print(w.a)\n\
            print(w.b)\n\
            print(w.a == (11 as i128))\n\
            var v = W(a = 1 as i128, b = 2)\n\
            v.a = 99 as i128\n\
            print(v.a)\n\
            print(v.b)\n\
            val arr: [i128; 2] = [5 as i128, 6 as i128]\n\
            print(arr[0])\n\
            print(arr[1])\n\
        }\n";
    let Some(out) = build_and_run("wide", source) else {
        return;
    };
    assert_eq!(out, "11\n7\ntrue\n99\n2\n5\n6\n");
}
#[test]
fn extern_c_function_links_against_local_c_object() {
    // 审查 P0-2：extern FFI 的 e2e 证明必须用本地 C 函数——kernel32 的
    // GetTickCount 是碰巧被 -lkernel32 带进来的，不构成链接能力证明。
    // 这里 helper.c 单独编译成目标文件，经 `build -- helper.obj` 链接。
    if !clang_available() {
        eprintln!("SKIP: extern FFI e2e requires clang");
        return;
    }
    let dir = std::env::temp_dir().join(format!("starry_ffi_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c_path = dir.join("helper.c");
    std::fs::write(
        &c_path,
        "long long c_add(long long a, long long b) { return a + b; }",
    )
    .unwrap();
    let obj_path = dir.join(if cfg!(windows) {
        "helper.obj"
    } else {
        "helper.o"
    });
    // 与 starryc 同一套工具链发现（PATH 回退 C:\Program Files\LLVM\bin）
    let clang_path = if Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        std::path::PathBuf::from("clang")
    } else {
        [
            r"C:\Program Files\LLVM\bin",
            r"C:\Program Files (x86)\LLVM\bin",
        ]
        .iter()
        .map(|d| std::path::Path::new(d).join("clang.exe"))
        .find(|p| p.is_file())
        .expect("clang available per clang_available()")
    };
    let cc = Command::new(&clang_path)
        .arg("-c")
        .arg(&c_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        cc.status.success(),
        "compile helper.c: {}",
        String::from_utf8_lossy(&cc.stderr)
    );

    let source_path = dir.join("ffi.starry");
    std::fs::write(
        &source_path,
        "extern \"c\" fun c_add(a: int, b: int) : int\nfun main() { print(unsafe { c_add(2, 3) }) }\n",
    )
    .unwrap();
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join("ffi.exe")
    } else {
        dir.join("ffi")
    };
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .arg("build")
        .arg(&source_path)
        .arg(&exe_path)
        .arg("--")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&exe_path).output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "5\n");

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn nullable_call_arguments_keep_their_null_flag() {
    // 审查回归钉：调用点 coerce 的 值→Nullable 臂曾把 flag 写进 payload
    // 槽（slot0 未初始化），且 Null 实参被当非空值构造——native 全靠
    // 栈零化碰巧正确。钉住 stdout：null 实参 → ?? 取默认，非空实参
    // → payload。
    let Some(out) = build_and_run(
        "nullable_args",
        "fun takes(n: int?) : int {\n    return n ?? 9\n}\nfun main() {\n    print(takes(null))\n    print(takes(4))\n}",
    ) else {
        return;
    };
    assert_eq!(out, "9\n4\n");
}

#[test]
fn extern_c_accepts_null_pointer_argument() {
    // P2-10 D-007 slice 的 e2e：裸指针 FFI 传参。C 侧收 void*，Starry 传
    // null（0 地址）——证明指针类型、null 提升、ABI 签名三层都对齐。
    if !clang_available() {
        eprintln!("SKIP: pointer FFI e2e requires clang");
        return;
    }
    let dir = std::env::temp_dir().join(format!("starry_ptrffi_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c_path = dir.join("helper.c");
    std::fs::write(&c_path, "int is_null(void *p) { return p == 0; }").unwrap();
    let obj_path = dir.join(if cfg!(windows) {
        "helper.obj"
    } else {
        "helper.o"
    });
    let clang_path = if Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        std::path::PathBuf::from("clang")
    } else {
        [
            r"C:\Program Files\LLVM\bin",
            r"C:\Program Files (x86)\LLVM\bin",
        ]
        .iter()
        .map(|d| std::path::Path::new(d).join("clang.exe"))
        .find(|p| p.is_file())
        .expect("clang available per clang_available()")
    };
    let cc = Command::new(&clang_path)
        .arg("-c")
        .arg(&c_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        cc.status.success(),
        "compile helper.c: {}",
        String::from_utf8_lossy(&cc.stderr)
    );
    let source_path = dir.join("ptrffi.starry");
    std::fs::write(
        &source_path,
        "extern \"c\" fun is_null(p: *const int) : int\nfun main() { print(unsafe { is_null(null) }) }\n",
    )
    .unwrap();
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join("ptrffi.exe")
    } else {
        dir.join("ptrffi")
    };
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .arg("build")
        .arg(&source_path)
        .arg(&exe_path)
        .arg("--")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe_path).output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1\n");

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn borrow_deref_end_to_end_native() {
    // M2 slice 2 e2e：&x / *p / *p = v 端到端（native 为验收路径；
    // 解释器无地址空间模型，指针指令一律 trap——interp.rs 已知局限）
    let Some(out) = build_and_run(
        "borrow",
        "fun main() {\n    var y = 7\n    val p = &y\n    print(*p)\n    val q = &mut y\n    *q = 9\n    print(y)\n    print(*q)\n}",
    ) else {
        return;
    };
    assert_eq!(out, "7\n9\n9\n");
}

#[test]
fn mutable_borrow_feeds_c_out_param() {
    // M2 slice 2 FFI e2e：&mut 借用跨 C ABI 边界（显式 as *mut 收窄，
    // 引用→裸指针不做隐式转换）。C 经指针改写 Starry 局部，回读验证。
    if !clang_available() {
        eprintln!("SKIP: mutable-borrow FFI e2e requires clang");
        return;
    }
    let dir = std::env::temp_dir().join(format!("starry_borrow_ffi_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c_path = dir.join("helper.c");
    std::fs::write(&c_path, "int twice(int *p) { *p *= 2; return *p; }").unwrap();
    let obj_path = dir.join(if cfg!(windows) {
        "helper.obj"
    } else {
        "helper.o"
    });
    let clang_path = if Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        std::path::PathBuf::from("clang")
    } else {
        [
            r"C:\Program Files\LLVM\bin",
            r"C:\Program Files (x86)\LLVM\bin",
        ]
        .iter()
        .map(|d| std::path::Path::new(d).join("clang.exe"))
        .find(|p| p.is_file())
        .expect("clang available per clang_available()")
    };
    let cc = Command::new(&clang_path)
        .arg("-c")
        .arg(&c_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        cc.status.success(),
        "compile helper.c: {}",
        String::from_utf8_lossy(&cc.stderr)
    );
    let source_path = dir.join("borrow_ffi.starry");
    std::fs::write(
        &source_path,
        "extern \"c\" fun twice(p: *mut int) : int\nfun main() {\n    var x = 21\n    print(unsafe { twice((&mut x) as *mut int) })\n    print(x)\n}\n",
    )
    .unwrap();
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join("borrow_ffi.exe")
    } else {
        dir.join("borrow_ffi")
    };
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .arg("build")
        .arg(&source_path)
        .arg(&exe_path)
        .arg("--")
        .arg(&obj_path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe_path).output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n42\n");

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn primary_constructor_runs_end_to_end() {
    // M3 e2e：主构造（val/var 字段、命名/默认/仅构造参数）+ 深度可变性
    let Some(out) = build_and_run(
        "primary_ctor",
        "struct Point(val x: int, var y: int)\nstruct R(val x: int = 5)\nstruct C(val x: int, noisy: int)\nfun main() {\n    val p = Point(1, 2)\n    print(p.x)\n    p.y = 9\n    print(p.y)\n    val q = Point(y = 4, x = 3)\n    print(q.x + q.y)\n    print(R().x)\n    print(C(7, 8).x)\n}",
    ) else {
        return;
    };
    assert_eq!(out, "1\n9\n7\n5\n7\n");
}
/// M5：目录构建助手——写多文件工程，starryc build <dir>，运行 exe。
/// None = 无工具链（调用方跳过）。
fn build_dir_and_run(name: &str, files: &[(&str, &str)], manifest: Option<&str>) -> Option<String> {
    if !clang_available() {
        eprintln!("SKIP: multi-file build e2e requires clang");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("starry_m5_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    for (rel, content) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, content).unwrap();
    }
    if let Some(toml) = manifest {
        std::fs::write(dir.join("starry.toml"), toml).unwrap();
    }
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .arg("build")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "multi-file build failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    // exe 名 = manifest name（无 manifest = 目录名）
    let exe_name = manifest
        .and_then(|t| t.lines().find(|l| l.trim_start().starts_with("name")))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.to_string())
        });
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join(format!("{exe_name}.exe"))
    } else {
        dir.join(&exe_name)
    };
    let run = Command::new(&exe_path).output().unwrap();
    assert_eq!(run.status.code(), Some(0), "run failed: {exe_path:?}");
    let out = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    Some(out)
}

#[test]
fn multi_file_build_merges_root_scope() {
    // M5 切片 1：多文件合并根作用域——跨文件调用、package 头剥离共存
    let Some(out) = build_dir_and_run(
        "merge",
        &[
            (
                "util.starry",
                "package demo.core\n\nfun double(n: int) : int = n * 2\n",
            ),
            (
                "main.starry",
                "package demo.app\n\nfun main() {\n    print(double(21))\n}\n",
            ),
        ],
        None,
    ) else {
        return;
    };
    assert_eq!(out, "42\n");
}

#[test]
fn manifest_srcs_layout_and_naming() {
    // M5：starry.toml 的 srcs 布局 + name 决定 exe 名
    let toml = "name = \"m5app\"\nsrcs = [\"src\"]";
    let Some(out) = build_dir_and_run(
        "m5app",
        &[
            ("src/lib.starry", "fun triple(n: int) : int = n * 3\n"),
            (
                "src/main.starry",
                "fun main() {\n    print(triple(14))\n}\n",
            ),
            ("starry.toml", toml),
        ],
        Some(toml),
    ) else {
        return;
    };
    assert_eq!(out, "42\n");
}

#[test]
fn checked_overflow_traps_at_runtime_with_message() {
    // M1（Q5）：+ 默认受检；运行期溢出 → starry_overflow_panic 打印算子名，exit 134
    let Some((_out, err, code)) = build_and_exec(
        "ovf_add",
        "fun main() : int {\n    val x = 9223372036854775807\n    return x + 1\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, 134, "code={code} stderr: {err}");
    assert!(err.contains("overflow: +"), "stderr: {err}");
}

#[test]
fn div_by_zero_traps_at_runtime_with_message() {
    // M1（Q5）D4：除零与 INT_MIN/-1 纳入受检（守卫在 sdiv 之前）
    let Some((_out, err, code)) = build_and_exec(
        "ovf_div",
        "fun main() : int {\n    val x = 1\n    return x / 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, 134, "code={code} stderr: {err}");
    assert!(err.contains("division by zero: /"), "stderr: {err}");
}

#[test]
fn wrapping_operators_wrap_at_runtime() {
    // M1（Q5）：+% *% 显式回绕，不 trap；i64 MAX +% 1 = MIN，i32 乘法回绕 = 0
    let Some(out) = build_and_run(
        "wrapping",
        "fun main() {\n    val x = 9223372036854775807\n    print(x +% 1)\n    val a: i32 = 65536\n    val b: i32 = 65536\n    print(a *% b)\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "-9223372036854775808\n0\n");
}

#[test]
fn class_arc_end_to_end_native() {
    // M4 slice 1：class 构造/字段/复制 retain/隐式 release 全链路原生跑
    let Some(out) = build_and_run(
        "class_arc",
        "class P(val x: int, var y: int)\nfun main() {\n    val a = P(1, 2)\n    val b = a\n    b.y = 9\n    print(a.x + b.y)\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "10\n");
}

#[test]
fn class_reference_equality_is_pointer_compare() {
    // M4 slice 2a：引用相等 = 指针比较——同对象 true，同值不同对象 false
    let Some(out) = build_and_run(
        "class_eq",
        "class P(val x: int)\nfun main() {\n    val a = P(1)\n    val b = a\n    val c = P(1)\n    print(a == b)\n    print(a == c)\n    print(a != c)\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "true\nfalse\ntrue\n");
}

#[test]
fn class_methods_and_deinit_end_to_end() {
    // M4 slice 2b：方法静态分派 + 共享引用 + 隐式 release 触发 deinit
    let Some(out) = build_and_run(
        "class_methods",
        "class Counter(var count: int) {\n    fun bump() : int {\n        self.count += 1\n        return self.count\n    }\n    deinit {\n        print(self.count)\n    }\n}\nfun main() {\n    val a = Counter(10)\n    print(a.bump())\n    print(a.bump())\n    val b = a\n    print(b.bump())\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "11\n12\n13\n13\n");
}

#[test]
fn weak_var_does_not_extend_lifetime() {
    // M4 slice 3：weak 不延长生命周期——两对象的 deinit 都触发（若错为
    // retain，对象永不释放、deinit 不打印）。drop 逆序：b 先（2），a 后（1）
    let Some(out) = build_and_run(
        "class_weak",
        "class Node(var name: int) {\n    deinit {\n        print(self.name)\n    }\n}\nfun main() {\n    val a = Node(1)\n    val b = Node(2)\n    weak var wa = a\n    weak var wb = b\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "2\n1\n");
}

#[test]
fn nullable_class_ownership_end_to_end() {
    // M4 slice 4a: nullable class bindings share class ownership discipline.
    // pa retains (deinit of the shared object fires once at if-exit);
    // ctor-into-nullable skips retain (deinit fires once at main exit);
    // null literal init retains nothing (release of null is a runtime no-op).
    let Some(out) = build_and_run(
        "class_nullable_own",
        "class Point(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    if (true) {\n        val a = Point(1)\n        val pa: Point? = a\n        print(pa!!.x)\n    }\n    print(99)\n    val pn: Point? = Point(3)\n    print(98)\n    val pnull: Point? = null\n    print(97)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "1\n1\n99\n98\n97\n3\n");
}

#[test]
fn nullable_class_safe_ops_end_to_end() {
    // M4 slice 4a: == null / != null, !!, ?. (payload = object ptr),
    // and ?? elvis over nullable class.
    let Some(out) = build_and_run(
        "class_nullable_ops",
        "class Point(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    val a = Point(5)\n    val pa: Point? = a\n    val v: int? = pa?.x\n    print(v!!)\n    val pn: Point? = null\n    val d: int? = pn?.x\n    if (d == null) {\n        print(88)\n    }\n    if (pa != null) {\n        print(77)\n    }\n    val q = pa ?? Point(7)\n    print(q.x)\n    val r = pn ?? Point(9)\n    print(r.x)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "5\n88\n77\n5\n9\n9\n5\n");
}

#[test]
fn class_params_and_returns_end_to_end() {
    // M4 slice 4c: +0 borrowed params, +1 owned returns. deinit fires
    // exactly once per object: a (main binding), b (+1 return binding),
    // c (ctor-into-param binding inside callee, released at callee exit).
    let Some(out) = build_and_run(
        "class_params_returns",
        "class P(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun getx(p: P): int {\n    return p.x\n}\nfun make(v: int): P {\n    return P(v)\n}\nfun twice(p: P): int {\n    val local = p\n    return local.x + p.x\n}\nfun main(): int {\n    val a = P(1)\n    print(getx(a))\n    val b = make(2)\n    print(b.x)\n    print(twice(a))\n    print(a.x)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "1\n2\n2\n1\n2\n1\n");
}

#[test]
fn nullable_class_params_returns_end_to_end() {
    // M4 slice 4c: ?P params and returns over the flag+payload ABI.
    let Some(out) = build_and_run(
        "class_nullable_params_returns",
        "class P(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun maybe(v: int): P? {\n    if (v > 0) {\n        return P(v)\n    }\n    return null\n}\nfun readx(p: P?): int {\n    if (p != null) {\n        return p!!.x\n    }\n    return 0\n}\nfun main(): int {\n    val a = maybe(5)\n    print(readx(a))\n    val b = maybe(-1)\n    print(readx(b))\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "5\n0\n5\n");
}

#[test]
fn class_reassignment_ownership_end_to_end() {
    // M4 slice 4d: reassignment releases the old payload and retains a
    // borrowed rhs, keeping the one-reference-per-var invariant.
    // Deinit order: 1 (b = a releases P(1)), 2 (b = P(3) releases P(2)),
    // 3 (main exit). Self-assignment is a no-op.
    let Some(out) = build_and_run(
        "class_reassign",
        "class P(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    var b = P(1)\n    if (true) {\n        val a = P(2)\n        b = a\n        print(b.x)\n    }\n    print(b.x)\n    b = P(3)\n    b = b\n    print(b.x)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "1\n2\n2\n2\n3\n3\n");
}

#[test]
fn plus1_return_and_temporaries_end_to_end() {
    // M4 slice 4b (Plan C): unified +1 returns and full-expression temp
    // release. Bare ctor call and member access on a temp both fire
    // deinit right after their statement; the id() passthrough retains
    // the borrowed param so the caller-side object stays alive.
    let Some(out) = build_and_run(
        "class_plus1_temps",
        "class Bare(val v: int) {\n    deinit {\n        print(self.v)\n    }\n}\nfun make(): Bare {\n    return Bare(7)\n}\nfun id(p: Bare): Bare {\n    return p\n}\nfun main(): int {\n    make()\n    print(1)\n    print(make().v)\n    val a = Bare(3)\n    if (true) {\n        val b = id(a)\n        print(b.v)\n    }\n    print(a.v)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "7\n1\n7\n7\n3\n3\n3\n");
}

#[test]
fn weak_upgrade_end_to_end() {
    // M4 slice 4b (Plan C): weak read = upgrade to ?P. Alive target reads
    // work via !!/?. ??; a ctor-initialized weak var releases at the
    // statement end (weak holds no strong ref), so upgrade yields null.
    // Every object deinits exactly once (count balance).
    let Some(out) = build_and_run(
        "class_weak_upgrade",
        "class P(val x: int) {\n    deinit {\n        print(self.x)\n    }\n}\nfun main(): int {\n    var i = 0\n    while i < 100000 {\n        weak var w = P(i)\n        val t = w\n        if (t != null) {\n            print(t!!.x)\n        }\n        i += 100000\n    }\n    val a = P(1)\n    weak var wa = a\n    val b = P(2)\n    weak var wb = b\n    print(wa!!.x)\n    print(wb!!.x)\n    val na = wa\n    print(na!!.x)\n    val v: int? = wa?.x\n    print(v!!)\n    val fb = wa ?? P(5)\n    print(fb.x)\n    return 0\n}\n",
    ) else {
        return;
    };
    assert_eq!(out, "0\n1\n2\n1\n1\n1\n2\n1\n");
}
