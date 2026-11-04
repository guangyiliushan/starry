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

/// 源码 → 原生可执行 → stdout。返回 None 表示无工具链，调用方跳过。
fn build_and_run(name: &str, source: &str) -> Option<String> {
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
    assert!(
        run.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = String::from_utf8_lossy(&run.stdout).into_owned();

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&exe_path);
    let _ = std::fs::remove_dir_all(&dir);
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
