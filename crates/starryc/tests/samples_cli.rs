//! Sample corpus gate: the tracked `samples/` directory is a progress meter.
//! The threshold is a floor, not an exact target, so improvements do not
//! require churning this test.

use std::process::Command;

#[test]
fn sample_corpus_passes_floor() {
    let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .canonicalize()
        .expect("tracked samples directory should exist in the starry checkout");
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&samples).expect("read samples") {
        let path = entry.expect("sample entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("starry") {
            continue;
        }
        total += 1;
        let out = Command::new(env!("CARGO_BIN_EXE_starryc"))
            .args(["check", path.to_str().expect("utf-8 sample path")])
            .output()
            .expect("run starryc check");
        if out.status.success() {
            passed += 1;
        } else {
            failures.push(format!(
                "{}: {}",
                path.display(),
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
        }
    }
    eprintln!("sample corpus: {passed}/{total}");
    assert!(
        total >= 14,
        "expected the full sample corpus, found {total}"
    );
    assert!(
        passed >= 5,
        "sample corpus regressed: {passed}/{total}\n{}",
        failures.join("\n")
    );
}
