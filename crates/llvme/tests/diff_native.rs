//! 差分 harness（审查 P1-8）：irc 参考解释器 vs 原生程序逐行对拍。
//! 同一源码两条路径执行，print 输出必须逐行一致：
//! - interp：`run_named_capturing` 收集 printed（`print_line` 与 runtime
//!   starry_print_* 的文本形态对齐）
//! - native：llvme 发射 .ll → clang + 内嵌 runtime 链接 → 运行 stdout
//!
//! 选型纪律：extern "c" 程序不可 interp（已知局限）、浮点打印文本两侧
//! 不同源（C fmt_double vs interp 占位）——均不入选。i128 已随 interp
//! 全 128 位建模入选（2026-11）。
//! trap 程序（受检溢出/除零）不走本路径——TRAP_CASES 的
//! interp_and_native_agree_on_traps 单独断言两侧一致 trap。
//! 无工具链时显式 SKIP（与 build_cli 同纪律）。

use std::path::PathBuf;
use std::process::Command;

const RUNTIME_HEADER: &str = include_str!("../../../runtime/starry_runtime.h");
const RUNTIME_COMMON_C: &str = include_str!("../../../runtime/common.c");
const RUNTIME_WIN32_C: &str = include_str!("../../../runtime/win32.c");
const RUNTIME_POSIX_C: &str = include_str!("../../../runtime/posix.c");

fn llvm_tool_path(name: &str) -> Option<PathBuf> {
    if Command::new(name)
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Some(PathBuf::from(name));
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

/// (interp 行, native 行)。native 段无工具链返回 None。
fn both_outputs(case: usize, source: &str) -> Option<(Vec<String>, Vec<String>)> {
    let parsed = parser::parse_source(source).expect("machine parse");
    let mut sem_out = sem::analyze(parsed);
    assert!(
        sem_out.diagnostics.is_empty(),
        "sem diags in CASES[{case}]: {:?}",
        sem_out.diagnostics.first()
    );
    let tac = irc::lower(&mut sem_out, source);
    assert!(tac.diags.is_empty(), "irc diags: {:?}", tac.diags.first());

    // 路径 A：参考解释器（main 无参）
    let (_, printed) = irc::interp::run_named_capturing(
        &tac,
        &sem_out.tys,
        &sem_out.defs,
        &sem_out.interner,
        "main",
        &[],
    )
    .expect("interp run");

    // 路径 B：native build + run
    let clang = llvm_tool_path("clang")?;
    let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &mut sem_out.interner);
    let dir = std::env::temp_dir().join(format!("starry_diff_{}_{}", std::process::id(), case));
    std::fs::create_dir_all(&dir).unwrap();
    let ll = dir.join("diff.ll");
    let common = dir.join("runtime_common.c");
    let platform = dir.join(if cfg!(windows) {
        "runtime_win32.c"
    } else {
        "runtime_posix.c"
    });
    let exe: PathBuf = if cfg!(windows) {
        dir.join("diff.exe")
    } else {
        dir.join("diff")
    };
    std::fs::write(&ll, &text).unwrap();
    std::fs::write(
        &common,
        RUNTIME_COMMON_C.replacen("#include \"starry_runtime.h\"", RUNTIME_HEADER, 1),
    )
    .unwrap();
    std::fs::write(
        &platform,
        if cfg!(windows) {
            RUNTIME_WIN32_C
        } else {
            RUNTIME_POSIX_C
        },
    )
    .unwrap();
    let mut cmd = Command::new(&clang);
    cmd.arg(&ll).arg(&common).arg(&platform);
    if cfg!(windows) {
        cmd.arg("-nostdlib").arg("-lkernel32");
    }
    let built = cmd.arg("-o").arg(&exe).output().unwrap();
    assert!(
        built.status.success(),
        "clang link: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&exe).output().unwrap();
    assert_eq!(run.status.code(), Some(0), "native run failed");
    let native: Vec<String> = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    Some((printed, native))
}

/// 差分用例：每个都是完整可运行程序（check/irc 零诊断、无 extern、
/// 无浮点打印、无 i128 依赖——选型纪律见模块头）。
const CASES: &[&str] = &[
    // 整型位宽（字面量定型 → 最小可容纳；M1 后受检算术不溢出、
    // 回绕走显式 +%（Q5）——溢出程序是 trap，按选型纪律不入 diff）
    "fun main() {\n    print(2147483647 +% 1)\n    print(2147483646 + 1)\n    print(3000000000)\n    print(10 as i16)\n    print(-5 as i8)\n    print(7 * 6)\n}",
    // 除/余/位运算/移位
    "fun main() {\n    print(7 / 2)\n    print(7 % 2)\n    print(12 & 10)\n    print(12 | 3)\n    print(5 ^ 3)\n    print(1 << 4)\n    print(256 >> 4)\n    print(~0 as i8)\n}",
    // 比较与布尔短路
    "fun main() {\n    print(3 > 2)\n    print(2 >= 3)\n    print(1 == 1)\n    print(1 != 1)\n    print(true && false)\n    print(false || true)\n    print(!true)\n}",
    // 字符串：拼接/插值/相等/数组成员
    "fun main() {\n    print(\"a\" + \"b\")\n    print(\"v=${41 + 1}\")\n    print(\"x\" == \"x\")\n    print(\"x\" != \"y\")\n    print(\"b\" in [\"a\", \"b\"])\n    print(\"z\" in [\"a\", \"b\"])\n}",
    // char
    "fun main() {\n    print('a')\n    print('你')\n}",
    // struct：构造/字段/逐字段相等
    "struct P { x: int, y: int }\nfun main() {\n    val p = P(1, 2)\n    print(p.x)\n    print(p.y)\n    print(p == P(1, 2))\n    print(p == P(1, 3))\n}",
    // 数组/元组：索引/聚合相等/动态下标
    "fun main() {\n    val a: [int; 3] = [7, 8, 9]\n    print(a[0])\n    print(a[2])\n    var i = 1\n    print(a[i])\n    val t = (1, 2)\n    print(t[1])\n    print(a == [7, 8, 9])\n    print((1, 2) == (1, 3))\n}",
    // i128：超 64 位字面量/算术/回绕/比较（interp 全 128 位建模，2026-11）
    "fun main() {
    print(9223372036854775808)
    val a: i128 = 9223372036854775807
    print(a +% 1)
    print(a *% 2)
    print(a > 0)
    val big = 170141183460469231731687303715884105727
    print(big +% 1)
    print(big -% 1)
}",
    // i128 聚合：struct 字段/数组/2×i64 lo/hi split-join（S3/P3-2）
    "struct Big {\n    a: i128,\n    b: int,\n    c: i128,\n}\nfun main(): int {\n    val x = Big(170141183460469231731687303715884105727, 7, -170141183460469231731687303715884105727)\n    var arr: [i128; 3] = [1 as i128, 9223372036854775808, -9223372036854775809]\n    print(x.a)\n    print(x.b)\n    print(x.c)\n    print(arr[0])\n    print(arr[1])\n    print(arr[2])\n    arr[1] = 18446744073709551617\n    print(arr[1])\n    return 0\n}", // nullable：elvis / == null / 构造取 payload
    "enum Opt { None, Some(int) }\nfun main() {\n    val o: Opt = Opt.Some(5)\n    print(o == Opt.Some(5))\n    print(o == Opt.None)\n    val n: int? = null\n    print(n == null)\n    print(n ?? 9)\n    val m: int? = 4\n    print(m ?? 9)\n}",
    // 区间：字面量 in / 物化 in（开闭）/ for 扫描
    "fun main() {\n    print(2 in 1..5)\n    print(5 in 1..5)\n    print(5 in 1..=5)\n    val r = 1..3\n    print(3 in r)\n    for x in r { print(x) }\n}",
    // 控制流值：if/when/loop 带值 break/块尾 loop
    "fun main() {\n    print(if 1 > 0 { 10 } else { 20 })\n    print(when 5 {\n        5 -> 50\n        else -> 0\n    })\n    val v = loop {\n        break 7\n    }\n    print(v)\n    print(pick())\n}\nfun pick() : int {\n    'outer: loop {\n        break 'outer 42\n    }\n}",
    // 注意：可空/聚合实参不入选——interp 实参经 Value 快照只传首槽
    //（已知局限，见 interp.rs 模块文档）；这类用例归 build_cli 原生断言
    // 全局依赖序 + 默认实参 + 具名实参
    "val b: int = a + 3\nval a: int = 2\nfun bump(n: int = 40) : int = n + 1\nfun main() {\n    print(b)\n    print(bump())\n    print(bump(n = 9))\n}",
    // 枚举 tag+payload 相等 / when 穷尽
    "enum E { A, B(int), C(string) }\nfun main() {\n    print(E.A == E.A)\n    print(E.B(1) == E.B(1))\n    print(E.B(1) == E.B(2))\n    print(E.C(\"s\") == E.C(\"s\"))\n}",
    // 模式：is 绑定 / for 元组解构 / if val
    "enum Opt { None, Some(int) }\nfun main() {\n    val o = Opt.Some(11)\n    if o is Opt.Some(v) {\n        print(v)\n    }\n    if val Opt.Some(w) = o {\n        print(w + 1)\n    }\n    val ps: [(int, int); 2] = [(1, 2), (3, 4)]\n    for (x, y) in ps { print(x + y) }\n}",
    // 复合赋值 + 聚合实参快照（var 实参在调用内不改写外部）
    "struct P { x: int }\nfun bump(p: P) : int {\n    return p.x + 1\n}\nfun main() {\n    var s = 1\n    s += 4\n    s *= 3\n    print(s)\n    val p = P(5)\n    print(bump(p))\n    print(p.x)\n}",
    // S2/P2-3b：闭包——标量捕获 box + 裸 fn 混用（tag 分派两侧对拍）
    "fun twice(f: (int) -> int, x: int): int {\n    return f(f(x))\n}\nfun apply(f: (int) -> int, x: int): int {\n    return f(x) + f(x + 1)\n}\nfun main(): int {\n    val k = 10\n    print(twice({ (n: int) -> n + k }, 1))\n    val m = 100\n    print(apply({ (n: int) -> n + k + m }, 1))\n    print(twice({ it * 3 }, 2))\n    print(twice({ (n: int) -> n * 3 }, 2))\n    return 0\n}",
    // 闭包——绑定位置 lambda（注解 / 参数标注推断）+ 零参捕获
    "fun main(): int {\n    val k = 10\n    val f: (int) -> int = { (n: int) -> n + k }\n    print(f(5))\n    val g = { (n: int) -> n * 2 }\n    print(g(21))\n    val h: () -> int = { () -> k + 1 }\n    print(h())\n    return 0\n}",
    // 闭包——捕获外层 fn 形参并逃逸返回（矩阵 #6）+ 捕获外层局部
    "fun mkadd(a: int): (int) -> int {\n    return { (b: int) -> a + b }\n}\nfun main(): int {\n    val inner = mkadd(1)\n    print(inner(2))\n    val outer = 100\n    val wrap = { (x: int) -> x + outer }\n    print(wrap(5))\n    return 0\n}",
    // 闭包——快照语义（矩阵 #8）：捕获后外层修改不影响已捕获值
    "fun apply(f: (int) -> int, x: int): int {\n    return f(x) + f(x + 1)\n}\nfun main(): int {\n    var x = 1\n    print(apply({ (n: int) -> n + x }, 5))\n    x = 100\n    print(apply({ (n: int) -> n + x }, 5))\n    return 0\n}",
];

/// M1.5（D2/D4 conformance）：trap 程序两侧必须一致——interp Trap 且 native
/// exit 134 + stderr 带区分后的原因。溢出正是 interp/native 最该对拍的
/// 地方（「把溢出移出 diff」被驳回，本类即回应）。
const TRAP_CASES: &[(&str, &str)] = &[
    (
        "add overflow",
        "fun main() : int {\n    val x = 9223372036854775807\n    return x + 1\n}",
    ),
    (
        "div by zero",
        "fun main() : int {\n    val x = 1\n    return x / 0\n}",
    ),
    (
        "narrow mul overflow",
        "fun main() : int {\n    val a: i8 = 100\n    val b: i8 = 100\n    return a * b\n}",
    ),
];

/// Deterministic bounded expression fuzzing. The generator stays reproducible,
/// uses checked/wrapping integer forms only, and keeps i64 operands small enough
/// that checked forms cannot trap.
fn differential_fuzz_source(seed: u64) -> String {
    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    let mut state = seed | 1;
    let leaf = |state: &mut u64| ((next(state) % 19) as i64) - 9;
    let ops = ["+", "-", "*", "+%", "-%", "*%"];
    let mut expression = leaf(&mut state).to_string();
    for _ in 0..5 {
        let rhs = leaf(&mut state);
        let op = ops[(next(&mut state) % ops.len() as u64) as usize];
        expression = format!("({expression} {op} {rhs})");
    }
    format!("fun main(): int {{\n    print({expression})\n    return 0\n}}")
}

#[test]
fn deterministic_differential_fuzz_agrees() {
    if llvm_tool_path("clang").is_none() {
        eprintln!("SKIP: differential fuzz requires clang");
        return;
    }
    for i in 0..12u64 {
        let source =
            differential_fuzz_source(0x5354_4152_5259 ^ (i.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        let Some((interp_out, native_out)) = both_outputs(CASES.len() + i as usize, &source) else {
            unreachable!("clang presence checked above");
        };
        assert_eq!(
            interp_out, native_out,
            "differential fuzz divergence for seed {i}:\n{source}"
        );
    }
}

#[test]
fn interp_and_native_agree_on_traps() {
    for (name, source) in TRAP_CASES {
        let parsed = parser::parse_source(source).expect("machine parse");
        let mut sem_out = sem::analyze(parsed);
        assert!(
            sem_out.diagnostics.is_empty(),
            "sem diags in trap case {name}: {:?}",
            sem_out.diagnostics.first()
        );
        let tac = irc::lower(&mut sem_out, source);
        assert!(tac.diags.is_empty(), "irc diags: {:?}", tac.diags.first());
        let interp_res = irc::interp::run_named_capturing(
            &tac,
            &sem_out.tys,
            &sem_out.defs,
            &sem_out.interner,
            "main",
            &[],
        );
        assert!(
            matches!(interp_res, Err(irc::interp::InterpError::Trap(_))),
            "interp did not trap for {name}: {interp_res:?}"
        );
        let Some(clang) = llvm_tool_path("clang") else {
            continue;
        };
        let text = llvme::emit(&tac, &sem_out.tys, &sem_out.defs, &mut sem_out.interner);
        let dir = std::env::temp_dir().join(format!(
            "starry_diff_trap_{}_{}",
            std::process::id(),
            name.replace(" ", "_")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let ll = dir.join("t.ll");
        let common = dir.join("runtime_common.c");
        let platform = dir.join(if cfg!(windows) {
            "runtime_win32.c"
        } else {
            "runtime_posix.c"
        });
        let exe: PathBuf = if cfg!(windows) {
            dir.join("t.exe")
        } else {
            dir.join("t")
        };
        std::fs::write(&ll, &text).unwrap();
        std::fs::write(
            &common,
            RUNTIME_COMMON_C.replacen("#include \"starry_runtime.h\"", RUNTIME_HEADER, 1),
        )
        .unwrap();
        std::fs::write(
            &platform,
            if cfg!(windows) {
                RUNTIME_WIN32_C
            } else {
                RUNTIME_POSIX_C
            },
        )
        .unwrap();
        let mut cmd = Command::new(&clang);
        cmd.arg(&ll).arg(&common).arg(&platform);
        if cfg!(windows) {
            cmd.arg("-nostdlib").arg("-lkernel32");
        }
        let built = cmd.arg("-o").arg(&exe).output().unwrap();
        assert!(
            built.status.success(),
            "clang link: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run = Command::new(&exe).output().unwrap();
        assert_eq!(run.status.code(), Some(134), "native trap exit for {name}");
        let err = String::from_utf8_lossy(&run.stderr);
        assert!(
            err.contains(" in main"),
            "trap stderr lacks fun name for {name}: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn interp_and_native_outputs_agree_line_by_line() {
    if llvm_tool_path("clang").is_none() {
        eprintln!("SKIP: diff harness requires clang (PATH or C:\\Program Files\\LLVM\\bin)");
        return;
    }
    for (i, src) in CASES.iter().enumerate() {
        let Some((interp_out, native_out)) = both_outputs(i, src) else {
            unreachable!("clang presence checked above");
        };
        assert_eq!(
            interp_out, native_out,
            "interp vs native divergence in CASES[{i}]:\n{src}"
        );
    }
}
