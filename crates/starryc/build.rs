//! Build script: expose the current git short hash as GIT_HASH for
//! `starryc --version`. Falls back to "unknown" outside a git checkout.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let git_dir = Path::new(&manifest).join("../../.git");
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
    let hash = head_hash(&git_dir).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={hash}");
}

fn head_hash(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        let reference = reference.trim();
        if let Ok(hash) = fs::read_to_string(git_dir.join(reference)) {
            return Some(short(&hash));
        }
        let packed = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
        let needle = format!(" {reference}");
        for line in packed.lines() {
            if line.ends_with(&needle) {
                return line.split(' ').next().map(short);
            }
        }
        None
    } else {
        Some(short(head))
    }
}

fn short(hash: &str) -> String {
    hash.trim().chars().take(8).collect()
}
