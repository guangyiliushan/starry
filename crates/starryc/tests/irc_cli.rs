//! 进程级 `irc` 测试：parse → sem → irc 的 exit 码与 TAC 输出。

use std::process::Command;

fn irc_file(name: &str, source: &str) -> (Option<i32>, String, String) {
    let path =
        std::env::temp_dir().join(format!("starryc_irc_{}_{name}.starry", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["irc", path.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn irc_clean_source_prints_tac() {
    let (code, stdout, stderr) = irc_file("clean", "fun f(a: int): int {\n    return a + 1\n}");
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stdout.contains("%t1 = add i64 %t0, 1") && stdout.contains("ret %t1 : i64"),
        "{stdout}"
    );
}

#[test]
fn irc_sem_errors_gate_lowering() {
    let (code, stdout, stderr) = irc_file("bad", "fun f(): int {\n    return nosuch\n}");
    assert_eq!(code, Some(1));
    assert!(stderr.contains("SemUndefinedName"), "{stderr}");
    assert_eq!(stdout, "");
}

#[test]
fn irc_unsupported_construct_reports_and_stops() {
    // 元组解构绑定（非简单绑定模式）在 M-later
    let (code, stdout, stderr) = irc_file(
        "destructure",
        "fun f(): int {\n    val (x, y) = (1, 2)\n    return x\n}",
    );
    assert_eq!(code, Some(1));
    assert!(stderr.contains("IrcUnsupportedLowering"), "{stderr}");
    assert_eq!(stdout, "");
}
