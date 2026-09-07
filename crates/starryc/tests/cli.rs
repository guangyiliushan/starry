//! 进程级 CLI 测试：退出码矩阵（0 匹配 / 1 无匹配 / 2 用法错误）。

use std::process::Command;

fn starryc(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn exit_code_matrix() {
    // 空匹配：Some(0) → exit 0，stdout 打印长度（空匹配与不匹配不可折叠）
    let (code, out, _) = starryc(&["match", "a*", "b"]);
    assert_eq!((code, out.as_str()), (Some(0), "0\n"));

    // 不匹配：None → exit 1，stderr "no match"
    let (code, _, err) = starryc(&["match", "a", "b"]);
    assert_eq!(code, Some(1));
    assert_eq!(err.trim(), "no match");

    // 用法错误：exit 2
    let (code, _, err) = starryc(&["match", "a"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("用法"));
}

#[test]
fn match_basic() {
    let (code, out, _) = starryc(&["match", "[a-zA-Z][a-zA-Z0-9_]*", "abc123"]);
    assert_eq!((code, out.as_str()), (Some(0), "6\n"));
}

#[test]
fn dump_nfa_prints_states() {
    let (code, out, _) = starryc(&["dump-nfa", "a|b"]);
    assert_eq!(code, Some(0));
    assert!(out.contains("S0"));
    assert!(out.contains("start"));
    assert!(out.contains("accept"));
}
