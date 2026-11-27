//! Package graph v1 CLI integration tests.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "starry_pkg_graph_cli_{}_{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn manifest(name: &str, version: &str, deps: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n\n[dependencies]\n{deps}")
}

fn dependency(path: &str, version: &str) -> String {
    format!("math = {{ path = \"{path}\", version = \"{version}\" }}")
}

fn build(root: &Path, output: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_starryc"));
    command.arg("build").arg(root);
    if let Some(output) = output {
        command.arg(output);
    }
    command.output().unwrap()
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
        || [
            r"C:\Program Files\LLVM\bin",
            r"C:\Program Files (x86)\LLVM\bin",
        ]
        .iter()
        .any(|dir| Path::new(dir).join("clang.exe").is_file())
}

fn run(exe: &Path) -> (i32, String) {
    let exe = if cfg!(windows) && exe.extension().is_none() {
        exe.with_extension("exe")
    } else {
        exe.to_path_buf()
    };
    let run = Command::new(&exe).output().unwrap();
    (
        run.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&run.stdout).into_owned(),
    )
}

fn one_package(name: &str) -> PathBuf {
    let root = temp(name);
    let math = root.join("math");
    let app = root.join("app");
    write(&math, "starry.toml", &manifest("math", "1.2.0", ""));
    write(&math, "lib.starry", "fun double(n: int): int = n * 2\n");
    write(
        &app,
        "starry.toml",
        &manifest("app", "1.0.0", &dependency("../math", "1.2.0")),
    );
    write(
        &app,
        "main.starry",
        "import math.double\nfun main(): int {\n    return double(21)\n}\n",
    );
    root
}

fn nested_package_in(name: &str) -> PathBuf {
    let root = temp(name);
    let core = root.join("core");
    let math = root.join("math");
    let app = root.join("app");
    write(&core, "starry.toml", &manifest("core", "1.0.0", ""));
    write(&core, "lib.starry", "fun one(): int = 1\n");
    write(
        &math,
        "starry.toml",
        &manifest(
            "math",
            "1.2.0",
            "core = { path = \"../core\", version = \"1.0.0\" }",
        ),
    );
    write(
        &math,
        "lib.starry",
        "import core.one\nfun two(): int = one() + 1\n",
    );
    write(
        &app,
        "starry.toml",
        &manifest(
            "app",
            "2.0.0",
            "core = { path = \"../core\", version = \"1.0.0\" }\nmath = { path = \"../math\", version = \"1.2.0\" }",
        ),
    );
    write(
        &app,
        "main.starry",
        "import core.one\nimport math.two\nfun main(): int {\n    return one() * 10 + two()\n}\n",
    );
    root
}

fn nested_package() -> PathBuf {
    nested_package_in("nested")
}

fn native_build(root: &Path, name: &str, expected: i32) -> String {
    if !clang_available() {
        eprintln!("SKIP: package graph native run requires clang");
        return String::new();
    }
    let exe = temp(name).join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    });
    let output = build(root, Some(&exe));
    assert!(
        output.status.success(),
        "build failed: {}",
        combined(&output)
    );
    let (code, stdout) = run(&exe);
    assert_eq!(code, expected, "exit code for {name}");
    stdout
}

#[test]
fn app_depends_on_one_local_package() {
    let root = one_package("one");
    assert_eq!(native_build(&root, "one_out", 42), String::new());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn app_depends_on_nested_package_graph() {
    let root = nested_package();
    assert_eq!(native_build(&root, "nested_out", 12), String::new());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn exact_dependency_version_matches() {
    let root = one_package("exact");
    let output = build(&root, None);
    if !clang_available() {
        assert!(
            !combined(&output).contains("dependency"),
            "{}",
            combined(&output)
        );
    } else {
        assert!(output.status.success(), "{}", combined(&output));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn version_mismatch_is_rejected() {
    let root = temp("version");
    let math = root.join("math");
    let app = root.join("app");
    write(&math, "starry.toml", &manifest("math", "1.3.0", ""));
    write(&math, "lib.starry", "fun f(): int = 1\n");
    write(
        &app,
        "starry.toml",
        &manifest("app", "1.0.0", &dependency("../math", "1.2.0")),
    );
    write(&app, "main.starry", "fun main(): int = 0\n");
    let output = build(&app, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("requests version 1.2.0, package has 1.3.0"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dependency_name_mismatch_is_rejected() {
    let root = temp("name");
    let target = root.join("math");
    let app = root.join("app");
    write(&target, "starry.toml", &manifest("other", "1.0.0", ""));
    write(&target, "lib.starry", "fun f(): int = 1\n");
    write(
        &app,
        "starry.toml",
        &manifest("app", "1.0.0", &dependency("../math", "1.0.0")),
    );
    write(&app, "main.starry", "fun main(): int = 0\n");
    let output = build(&app, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("dependency key `math` does not match package name `other`"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dependency_cycle_has_a_readable_path() {
    let root = temp("dep_cycle");
    let a = root.join("a");
    let b = root.join("b");
    write(
        &a,
        "starry.toml",
        &manifest("a", "1.0.0", "b = { path = \"../b\", version = \"1.0.0\" }"),
    );
    write(&a, "lib.starry", "");
    write(
        &b,
        "starry.toml",
        &manifest("b", "1.0.0", "a = { path = \"../a\", version = \"1.0.0\" }"),
    );
    write(&b, "lib.starry", "");
    let output = build(&a, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("a -> b -> a"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn import_cycle_is_rejected() {
    let root = temp("import_cycle");
    write(&root, "a/mod.starry", "import b.f\nfun unused(): int = 1\n");
    write(&root, "b/mod.starry", "import a.g\nfun unused(): int = 2\n");
    let output = build(&root, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("import cycle: a -> b -> a"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn self_import_is_rejected() {
    let root = temp("self_import");
    write(
        &root,
        "app/main.starry",
        "import app.helper\nfun helper(): int = 1\nfun main(): int = 0\n",
    );
    let output = build(&root, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("import cycle: app -> app"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn duplicate_package_identity_is_rejected() {
    let root = temp("duplicate");
    let shared_one = root.join("shared1");
    let shared_two = root.join("shared2");
    let left = root.join("left");
    let right = root.join("right");
    let app = root.join("app");
    for package in [&shared_one, &shared_two] {
        write(package, "starry.toml", &manifest("shared", "1.0.0", ""));
        write(package, "lib.starry", "fun f(): int = 1\n");
    }
    write(
        &left,
        "starry.toml",
        &manifest(
            "left",
            "1.0.0",
            "shared = { path = \"../shared1\", version = \"1.0.0\" }",
        ),
    );
    write(&left, "lib.starry", "");
    write(
        &right,
        "starry.toml",
        &manifest(
            "right",
            "1.0.0",
            "shared = { path = \"../shared2\", version = \"1.0.0\" }",
        ),
    );
    write(&right, "lib.starry", "");
    write(
        &app,
        "starry.toml",
        &manifest(
            "app",
            "1.0.0",
            "left = { path = \"../left\", version = \"1.0.0\" }\nright = { path = \"../right\", version = \"1.0.0\" }",
        ),
    );
    write(&app, "main.starry", "fun main(): int = 0\n");
    let output = build(&app, None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("duplicate package identity `shared`"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cross_package_public_import_passes() {
    let root = one_package("public");
    let output = build(&root, None);
    if !clang_available() {
        assert!(
            !combined(&output).contains("not visible"),
            "{}",
            combined(&output)
        );
    } else {
        assert!(output.status.success(), "{}", combined(&output));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cross_package_internal_and_private_are_rejected() {
    let cases = [
        ("internal", "internal fun secret(): int = 1"),
        ("private", "private fun secret(): int = 1"),
    ];
    for (kind, declaration) in cases {
        let root = temp(&format!("{kind}_access"));
        let math = root.join("math");
        let app = root.join("app");
        write(&math, "starry.toml", &manifest("math", "1.0.0", ""));
        write(&math, "lib.starry", &format!("{declaration}\n"));
        write(
            &app,
            "starry.toml",
            &manifest("app", "1.0.0", &dependency("../math", "1.0.0")),
        );
        write(
            &app,
            "main.starry",
            "import math.secret\nfun main(): int = secret()\n",
        );
        let output = build(&app, None);
        assert!(
            !output.status.success(),
            "{kind} cross-package access was accepted"
        );
        let text = combined(&output);
        assert!(
            text.contains("not visible") || text.contains("undefined name"),
            "{kind}: {text}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn transitive_import_without_direct_dependency_is_rejected() {
    let root = nested_package_in("transitive");
    write(
        &root.join("app"),
        "starry.toml",
        &manifest("app", "2.0.0", &dependency("../math", "1.2.0")),
    );
    let output = build(&root.join("app"), None);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("without a direct dependency"),
        "{}",
        combined(&output)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn malformed_manifest_subset_is_rejected() {
    let root = temp("malformed");
    write(
        &root,
        "starry.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\nmath = \"bad\"\n",
    );
    write(&root, "main.starry", "fun main(): int = 0\n");
    let output = build(&root, None);
    assert!(!output.status.success());
    let text = combined(&output);
    assert!(text.contains("dependency value must be"), "{text}");
    assert!(text.contains("starry.toml:6:"), "{text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn single_file_build_does_not_regress() {
    if !clang_available() {
        eprintln!("SKIP: single-file native regression requires clang");
        return;
    }
    let root = temp("single");
    write(
        &root,
        "single.starry",
        "fun main(): int {\n    print(\"single\")\n    return 0\n}\n",
    );
    assert_eq!(native_build(&root, "single_out", 0), "single\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn same_package_build_does_not_regress() {
    let root = temp("same_root");
    write(
        &root,
        "demo/core/util.starry",
        "fun double(n: int): int = n * 2\n",
    );
    write(
        &root,
        "demo/app/main.starry",
        "import demo.core.double\nfun main(): int {\n    return double(21)\n}\n",
    );
    assert_eq!(native_build(&root, "same_root_out", 42), String::new());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn build_order_is_deterministic() {
    let root = one_package("order_src");
    if !clang_available() {
        eprintln!("SKIP: deterministic native build requires clang");
        let _ = std::fs::remove_dir_all(&root);
        return;
    }
    let exe = temp("order").join(if cfg!(windows) { "app.exe" } else { "app" });
    let first = build(&root.join("app"), Some(&exe));
    let second = build(&root.join("app"), Some(&exe));
    assert!(first.status.success(), "{}", combined(&first));
    assert!(second.status.success(), "{}", combined(&second));
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(run(&exe), (42, String::new()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(temp("order"));
}
