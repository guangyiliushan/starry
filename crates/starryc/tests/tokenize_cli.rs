//! 进程级 tokenize 测试：token 流内容与 exclusive 字节 span。

use std::process::Command;

fn tokenize(source: &str) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["tokenize", source])
        .output()
        .unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn tokenize_stream_and_spans() {
    let (code, out) = tokenize("if x1 42");
    assert_eq!(code, Some(0));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("0..2"), "{out}");
    assert!(lines[0].contains("keyword"), "{out}");
    assert!(lines[1].starts_with("3..5"), "{out}");
    assert!(lines[1].contains("identifier"), "{out}");
    assert!(lines[2].starts_with("6..8"), "{out}");
    assert!(lines[2].contains("integer"), "{out}");
}

#[test]
fn tokenize_unknown_multibyte_char() {
    // 未知字符按字符推进：🦀 = 4 字节，不切碎、不 panic
    let (code, out) = tokenize("🦀");
    assert_eq!(code, Some(0));
    assert!(out.starts_with("0..4"), "{out}");
    assert!(out.contains("unknown"), "{out}");
}
