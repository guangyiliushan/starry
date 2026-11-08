//! 进程级 `check` 测试：parse → sem 全链的 exit 码与诊断输出。

use std::process::Command;

fn check_file(name: &str, source: &str) -> (Option<i32>, String) {
    let path = std::env::temp_dir().join(format!(
        "starryc_check_{}_{name}.starry",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_clean_source_exits_zero() {
    let (code, err) = check_file("clean", "fun main() {\n    val x = 1 + 2\n}");
    assert_eq!(code, Some(0), "{err}");
}

#[test]
fn check_reports_diagnostics_with_spans() {
    let (code, err) = check_file("bad", "fun main() {\n    nosuch()\n}");
    assert_eq!(code, Some(1));
    assert!(err.contains("SemUndefinedName"), "{err}");
    // 诊断按 `path:line:col` 打印（nosuch 在第 2 行第 5 列）
    assert!(err.contains(":2:5:"), "{err}");
    assert!(err.contains("undefined name"), "{err}");
}

#[test]
fn check_semantic_errors_after_clean_parse() {
    let (code, err) = check_file("sem", "fun f() : int {\n    return \"s\"\n}");
    assert_eq!(code, Some(1));
    assert!(err.contains("SemTypeMismatch"), "{err}");
}

#[test]
fn check_missing_file_is_machine_failure() {
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["check", "definitely_not_here.starry"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("starryc"), "{err}");
}
