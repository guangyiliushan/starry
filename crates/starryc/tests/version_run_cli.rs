//! CLI gate: `--version` and `run`（Phase 0 尾项）

use std::process::Command;

fn starryc() -> &'static str {
    env!("CARGO_BIN_EXE_starryc")
}

#[test]
fn version_prints_name_version_and_hash() {
    let out = Command::new(starryc())
        .arg("--version")
        .output()
        .expect("run starryc --version");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("starryc 0.1.0"), "stdout: {text}");
    assert!(text.contains('(') && text.contains(')'), "stdout: {text}");
}

#[test]
fn run_builds_executes_and_matches_stdout() {
    let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/01_block_continuation.starry");
    let run = Command::new(starryc())
        .args(["run", sample.to_str().expect("utf-8 sample path")])
        .output()
        .expect("run starryc run");
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "-2\n7\n");
}

#[test]
fn run_propagates_nonzero_exit_code() {
    let dir = std::env::temp_dir().join(format!("starry_run_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("exit7.starry");
    std::fs::write(&src, "fun main(): int {\n    return 7\n}").expect("write source");
    let run = Command::new(starryc())
        .args(["run", src.to_str().expect("utf-8 src")])
        .output()
        .expect("run starryc run");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "");
}
