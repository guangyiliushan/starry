//! Sample corpus gate.
//!
//! Layout:
//! - `samples/*.starry`          — runs clean through `starryc check` (all must pass),
//!   and (toolchain permitting) builds, runs, and matches `samples/golden/*.stdout`
//! - `samples/negative/*.starry` — rejected syntax/semantics (all must fail)
//! - `samples/roadmap/*.starry`  — spec examples for not-yet-implemented features
//!   (trait/class/impl/spawn/generics): expected to fail until those land, kept
//!   as executable documentation of the remaining language surface.
//!
//! Any top-level sample failing, or any negative/roadmap sample unexpectedly
//! passing, fails the gate — the corpus is a regression meter, not a floor.

use std::path::{Path, PathBuf};
use std::process::Command;

fn check(path: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .args(["check", path.to_str().expect("utf-8 sample path")])
        .output()
        .expect("run starryc check");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn starry_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("starry"))
        .collect();
    files.sort();
    files
}

/// check 过的样例 → 原生可执行 → stdout。None = 无工具链（调用方 SKIP）。
fn build_and_run(path: &Path) -> Option<String> {
    let clang_ok = Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || [
            r"C:\Program Files\LLVM\bin",
            r"C:\Program Files (x86)\LLVM\bin",
        ]
        .iter()
        .any(|d| std::path::Path::new(d).join("clang.exe").is_file());
    if !clang_ok {
        return None;
    }
    let dir = std::env::temp_dir().join(format!(
        "starry_sample_{}_{}",
        std::process::id(),
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let exe_path: PathBuf = if cfg!(windows) {
        dir.join("sample.exe")
    } else {
        dir.join("sample")
    };
    let build = Command::new(env!("CARGO_BIN_EXE_starryc"))
        .arg("build")
        .arg(path)
        .arg(&exe_path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "sample build failed for {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe_path).output().unwrap();
    assert_eq!(run.status.code(), Some(0), "sample run failed: {path:?}");
    let out = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    Some(out)
}

#[test]
fn sample_corpus_matches_implementation_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .canonicalize()
        .expect("tracked samples directory should exist in the starry checkout");

    // 1) Positive corpus: every top-level sample must check clean.
    let positive = starry_files(&root);
    assert!(
        positive.len() >= 5,
        "expected the runnable sample corpus, found {}",
        positive.len()
    );
    let mut failures = Vec::new();
    for path in &positive {
        let (ok, err) = check(path);
        if !ok {
            failures.push(format!(
                "{}: {}",
                path.display(),
                err.lines().next().unwrap_or("")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "top-level samples must all check clean:\n{}",
        failures.join("\n")
    );

    // 1b) Run-pass（审查 P0-4）：正样例不止 check——build → 运行 → stdout
    // 与 samples/golden/<name>.stdout 逐字相等。golden 是验收产物，缺文件
    // 即失败（新样例必须随 golden 一起提交）。无工具链时显式 SKIP 运行段
    // （check 段仍强制执行），与 build_cli 同纪律。
    for path in &positive {
        let golden = root.join("golden").join(format!(
            "{}.stdout",
            path.file_stem().unwrap().to_string_lossy()
        ));
        let expected = std::fs::read_to_string(&golden)
            .unwrap_or_else(|e| panic!("missing golden stdout {}: {e}", golden.display()))
            .replace("\r\n", "\n");
        let Some(actual) = build_and_run(path) else {
            eprintln!("SKIP: run-pass for {} (no clang toolchain)", path.display());
            continue;
        };
        assert_eq!(
            actual,
            expected,
            "stdout drift in {} (update golden only on intentional behavior change)",
            path.display()
        );
    }

    // 2) Negative corpus: rejected constructs must keep failing.
    let negative = root.join("negative");
    let neg_files = starry_files(&negative);
    assert!(neg_files.len() >= 3, "negative corpus shrank");
    for path in &neg_files {
        let (ok, _) = check(path);
        assert!(
            !ok,
            "negative sample unexpectedly passed: {}",
            path.display()
        );
    }

    // 3) Roadmap corpus: spec examples for unimplemented features.
    let roadmap = root.join("roadmap");
    let road_files = starry_files(&roadmap);
    assert!(road_files.len() >= 6, "roadmap corpus shrank");
    for path in &road_files {
        let (ok, _) = check(path);
        assert!(
            !ok,
            "roadmap sample passed — move it to the top-level corpus: {}",
            path.display()
        );
    }
}
