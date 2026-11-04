//! End-to-end `starryc build` test: source -> native exe -> stdout.
//! Skips cleanly when clang is not on PATH (runtime validator policy).

use std::process::Command;

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[test]
fn build_print_program_and_run_it() {
    if !clang_available() {
        eprintln!("clang not found; skipping starryc build e2e test");
        return;
    }
    let dir = std::env::temp_dir().join(format!("starry_build_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source_path = dir.join("hello.starry");
    std::fs::write(
        &source_path,
        "fun main() {\n    print(\"hi ${41 + 1}\")\n}\n",
    )
    .unwrap();
    let exe_path = if cfg!(windows) {
        dir.join("hello.exe")
    } else {
        dir.join("hello")
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
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hi 42\n");

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&exe_path);
}
