//! starryc — Starry 编译器入口
//!
//! 当前提供这些子命令：
//! - `starryc match <regex> <input>`：打印最长匹配长度
//!   （exit 0 匹配 / exit 1 无匹配 / exit 2 用法错误）
//! - `starryc dump-nfa <regex>`：打印 NFA 状态与边
//! - `starryc tokenize <source>`：词法 token 流
//! - `starryc check <file>`：parse → sem 全链诊断
//!   （exit 0 干净 / exit 1 有诊断，逐条 stderr `path:start..end: code: msg`）
//! - `starryc irc <file>`：parse → sem → irc，TAC 文本 stdout
//! - `starryc opt <file>`：irc → opt，优化后 TAC 文本 stdout
//! - `starryc llvme <file>`：parse → sem → irc → opt → llvme，.ll 文本 stdout
//!   （exit 0 干净 / exit 1 有诊断，格式同 check）
//! - `starryc build <file> [output]`：编译并调用 clang 链接 runtime，
//!   生成可直接执行的本机程序（机器层失败 exit 1；用法错误 exit 2）

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ast::syntax::decl::{DeclKind, ImportKind};
use lex::TokenKind;
use lex::display::write_escaped_str;
use lex::lexer::Lexer;
use lex::nfa::NFA;
use lex::regex::Translator;
use lex::regex::parse::parse;

mod package;
use package::{PackageGraph, PackageId, load_graph, parse_manifest};

fn main() -> ExitCode {
    // args_os + into_string：非 UTF-8 参数优雅报错而非 panic
    let mut args: Vec<String> = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(s) => args.push(s),
            Err(bad) => {
                eprintln!("starryc: 参数不是有效的 UTF-8: {}", bad.to_string_lossy());
                return usage();
            }
        }
    }

    match args.as_slice() {
        [cmd, pattern, input] if cmd == "match" => cmd_match(pattern, input),
        [cmd, pattern] if cmd == "dump-nfa" => cmd_dump_nfa(pattern),
        [cmd, source] if cmd == "tokenize" => cmd_tokenize(source),
        [cmd, path] if cmd == "check" => cmd_check(path),
        [cmd, path] if cmd == "irc" => cmd_irc(path),
        [cmd, path] if cmd == "opt" => cmd_opt(path),
        [cmd, path] if cmd == "llvme" => cmd_llvme(path),
        [cmd, rest @ ..] if cmd == "build" => {
            let release = rest.iter().any(|a| a.as_str() == "--release");
            let clean: Vec<String> = rest
                .iter()
                .filter(|a| a.as_str() != "--release")
                .cloned()
                .collect();
            cmd_build_args(&clean, release)
        }
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "用法: starryc match <regex> <input> | starryc dump-nfa <regex> | starryc tokenize <source> | starryc check <file> | starryc irc <file> | starryc opt <file> | starryc llvme <file> | starryc build <file> [output] [-- <clang 链接参数>]"
    );
    ExitCode::from(2)
}

fn build_nfa(pattern: &str) -> Result<NFA, String> {
    let ast = parse(pattern).map_err(|e| e.to_string())?;
    let hir = Translator::new().translate(&ast);
    Ok(NFA::from_hir(&hir, TokenKind::Identifier))
}

fn cmd_match(pattern: &str, input: &str) -> ExitCode {
    let nfa = match build_nfa(pattern) {
        Ok(nfa) => nfa,
        Err(msg) => {
            eprintln!("starryc: 无效正则: {msg}");
            return ExitCode::FAILURE;
        }
    };

    match nfa.match_prefix(input) {
        // Some(n) → stdout 打印长度；None → "no match"，空匹配(0)与不匹配不可折叠
        Some(len) => {
            println!("{len}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("no match");
            ExitCode::FAILURE
        }
    }
}

fn cmd_dump_nfa(pattern: &str) -> ExitCode {
    let nfa = match build_nfa(pattern) {
        Ok(nfa) => nfa,
        Err(msg) => {
            eprintln!("starryc: 无效正则: {msg}");
            return ExitCode::FAILURE;
        }
    };

    for (id, state) in nfa.states().iter().enumerate() {
        for &target in &state.epsilons {
            println!("S{id} -ε-> S{target}");
        }
        for edge in &state.edges {
            println!("S{id} --{}--> S{}", edge.trans, edge.target);
        }
    }
    println!("start: S{}", nfa.start_state());
    for (id, kind) in nfa.accepting().iter().enumerate() {
        if let Some(kind) = kind {
            println!("accept: S{id} ({kind})");
        }
    }
    ExitCode::SUCCESS
}

/// 内置的 Starry 起步词法规则（ident / number）；关键字后分类
fn build_lexer() -> Result<Lexer, lex::LexerError> {
    Lexer::starry()
}

fn cmd_tokenize(source: &str) -> ExitCode {
    let lexer = match build_lexer() {
        Ok(lexer) => lexer,
        Err(e) => {
            eprintln!("starryc: {e}");
            return ExitCode::FAILURE;
        }
    };

    // lexeme 经 escape_debug 展示（display 纪律一致）
    for token in lexer.tokenize(source) {
        let mut lexeme = String::new();
        let _ = write_escaped_str(&mut lexeme, token.text(source));
        println!(
            "{}..{}\t{}\t{}",
            token.span.start, token.span.end, token.kind, lexeme
        );
    }
    ExitCode::SUCCESS
}

/// 诊断渲染：`path:line:col: Code: message`（字节 span → 行列；
/// 行/列从 1 起，列按 Unicode 标量计数）
fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn print_diags(path: &str, source: &str, diags: &[ast::syntax::node::Diagnostic]) {
    for d in diags {
        let (line, col) = line_col(source, d.span.start as usize);
        eprintln!("{path}:{line}:{col}: {:?}: {}", d.code, d.message);
    }
}

/// parse → sem 全链：诊断走 stderr（`path:line:col: code: message`），
/// 有诊断 exit 1，干净 exit 0；机器层失败（读文件/资源）单列
fn cmd_check(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("starryc: 无法读取 {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let parsed = match parser::parse_source(&source) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("starryc: {path}: 解析失败（机器层）: {e}");
            return ExitCode::FAILURE;
        }
    };
    let output = sem::analyze(parsed);
    if output.diagnostics.is_empty() {
        return ExitCode::SUCCESS;
    }
    print_diags(path, &source, &output.diagnostics);
    ExitCode::FAILURE
}

/// parse → sem → irc：TAC 文本走 stdout（starryc irc <file>）；
/// sem/irc 诊断走 stderr（同 check 格式），exit 0 干净 / exit 1 有诊断
fn cmd_irc(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("starryc: 无法读取 {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let parsed = match parser::parse_source(&source) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("starryc: {path}: 解析失败（机器层）: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut output = sem::analyze(parsed);
    if !output.diagnostics.is_empty() {
        print_diags(path, &source, &output.diagnostics);
        return ExitCode::FAILURE;
    }
    let tac = irc::lower(&mut output, &source);
    if !tac.diags.is_empty() {
        print_diags(path, &source, &tac.diags);
        return ExitCode::FAILURE;
    }
    print!("{}", tac.dump(&output.tys, &output.defs, &output.interner));
    ExitCode::SUCCESS
}

/// parse → sem → irc → opt：优化后 TAC 文本走 stdout；诊断同 irc 子命令
fn cmd_opt(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("starryc: 无法读取 {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let parsed = match parser::parse_source(&source) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("starryc: {path}: 解析失败（机器层）: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut output = sem::analyze(parsed);
    if !output.diagnostics.is_empty() {
        print_diags(path, &source, &output.diagnostics);
        return ExitCode::FAILURE;
    }
    let mut tac = irc::lower(&mut output, &source);
    if !tac.diags.is_empty() {
        print_diags(path, &source, &tac.diags);
        return ExitCode::FAILURE;
    }
    opt::optimize(&mut tac, &output.tys);
    print!("{}", tac.dump(&output.tys, &output.defs, &output.interner));
    ExitCode::SUCCESS
}

/// parse → sem → irc → opt → llvme 的共用编译链；诊断已打印，失败只返回 exit code。
fn compile_ll(path: &str) -> Result<String, ExitCode> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("starryc: 无法读取 {path}: {e}");
            return Err(ExitCode::FAILURE);
        }
    };
    let owned = source;
    compile_source(&owned, &|diags| print_diags(path, &owned, diags))
}

/// M5：编译一段（已合并或单文件的）源码；诊断渲染注入（多文件按偏移表
/// 归属到具体文件）。
fn compile_source(
    source: &str,
    render: &impl Fn(&[ast::syntax::node::Diagnostic]),
) -> Result<String, ExitCode> {
    let parsed = match parser::parse_source(source) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("starryc: 解析失败（机器层）: {e}");
            return Err(ExitCode::FAILURE);
        }
    };
    let mut output = sem::analyze(parsed);
    if !output.diagnostics.is_empty() {
        render(&output.diagnostics);
        return Err(ExitCode::FAILURE);
    }
    let mut tac = irc::lower(&mut output, source);
    if !tac.diags.is_empty() {
        render(&tac.diags);
        return Err(ExitCode::FAILURE);
    }
    opt::optimize(&mut tac, &output.tys);
    Ok(llvme::emit(
        &tac,
        &output.tys,
        &output.defs,
        &mut output.interner,
    ))
}

/// parse → sem → irc → opt → llvme：.ll 文本走 stdout；诊断同 irc 子命令
fn cmd_llvme(path: &str) -> ExitCode {
    match compile_ll(path) {
        Ok(text) => {
            if let Err(msg) = verify_ir(&text) {
                eprintln!("starryc: {path}: 生成的 LLVM IR 未通过校验:\n{msg}");
                return ExitCode::FAILURE;
            }
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

/// 用 clang 对生成的 IR 做语法/类型校验（llvme 子命令的出口门禁，
/// 防止把 `store i32 true` 一类的非法 IR 静默输出）。clang 不可用时跳过。
fn verify_ir(text: &str) -> Result<(), String> {
    // 校验器优先 llvm-as（纯 IR 汇编器），缺失回落 clang -x ir -c；
    // 两者都不可用显式打印 SKIP 说明（不静默跳过——审查 P0-5）
    let llvm_as = llvm_tool_path("llvm-as");
    if std::process::Command::new(&llvm_as)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        let pid = std::process::id();
        let ll = std::env::temp_dir().join(format!("starry_verify_{pid}.ll"));
        if std::fs::write(&ll, text.as_bytes()).is_ok() {
            let bc = std::env::temp_dir().join(format!("starry_verify_{pid}.bc"));
            let out = std::process::Command::new(&llvm_as)
                .arg(&ll)
                .arg("-o")
                .arg(&bc)
                .output();
            // 校验失败保留 .ll（调试可观测；成功路径照常清理）
            match out {
                Ok(o) if o.status.success() => {
                    let _ = std::fs::remove_file(&ll);
                    let _ = std::fs::remove_file(&bc);
                    return Ok(());
                }
                Ok(o) => {
                    return Err(format!(
                        "{}\n(IR kept at {})",
                        String::from_utf8_lossy(&o.stderr),
                        ll.display()
                    ));
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&ll);
                    let _ = std::fs::remove_file(&bc);
                }
            }
        }
    }
    let clang = llvm_tool_path("clang");
    if !std::process::Command::new(&clang)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("starryc: 注意: 无 llvm-as/clang，IR 校验跳过（显式 SKIP）");
        return Ok(());
    }
    let pid = std::process::id();
    let dir = std::env::temp_dir();
    let ll = dir.join(format!("starry_verify_{pid}.ll"));
    let obj = dir.join(format!("starry_verify_{pid}.o"));
    if let Err(e) = std::fs::write(&ll, text.as_bytes()) {
        return Err(format!("无法写临时 IR 文件: {e}"));
    }
    let out = std::process::Command::new(&clang)
        .arg("-x")
        .arg("ir")
        .arg("-c")
        .arg(&ll)
        .arg("-o")
        .arg(&obj)
        .output();
    // 校验失败保留 .ll（调试可观测；成功路径照常清理）
    match out {
        Ok(o) if o.status.success() => {
            let _ = std::fs::remove_file(&ll);
            let _ = std::fs::remove_file(&obj);
            Ok(())
        }
        Ok(o) => Err(format!(
            "{}\n(IR kept at {})",
            String::from_utf8_lossy(&o.stderr),
            ll.display()
        )),
        Err(_) => Ok(()),
    }
}

/// starryc build：.ll + 内嵌 split runtime → clang → 可执行文件。
/// runtime 不是 Cargo 依赖；源码在编译 starryc 时经 include_str! 固定。
const RUNTIME_HEADER: &str = include_str!("../../../runtime/starry_runtime.h");
const RUNTIME_COMMON_C: &str = include_str!("../../../runtime/common.c");
const RUNTIME_WIN32_C: &str = include_str!("../../../runtime/win32.c");
const RUNTIME_POSIX_C: &str = include_str!("../../../runtime/posix.c");

fn llvm_tool_path(name: &str) -> PathBuf {
    if std::process::Command::new(name)
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return PathBuf::from(name);
    }
    for dir in [
        r"C:\Program Files\LLVM\bin",
        r"C:\Program Files (x86)\LLVM\bin",
    ] {
        let candidate = Path::new(dir).join(format!("{name}.exe"));
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

/// `build <file> [output] [-- <clang 链接参数>...]`：`--` 之后的参数原样追加到
/// clang 链接行（额外 .obj/.o、`-l`、`-L` 等 extern "c" FFI 用）
fn cmd_build_args(args: &[String], release: bool) -> ExitCode {
    let (head, link_args) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[][..]),
    };
    match head {
        [path] => cmd_build(path, None, link_args, release),
        [path, output] => cmd_build(path, Some(output), link_args, release),
        _ => usage(),
    }
}
// ==================== Package graph v1 ====================
//
// Package roots are resolved through manifests first. Only then are their
// source files collected in graph postorder; each merged unit remembers the
// owning PackageId. This keeps ownership explicit even though v1 still lowers
// packages into one merged source set.

/// Recursively collect .starry sources (caller orders/dedups by canonical path).
fn collect_starry(
    dir: &Path,
    src_root: &Path,
    out: &mut Vec<(PathBuf, PathBuf, PackageId)>,
    owner: PackageId,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("无法读取源目录 {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("无法读取源目录 {}: {e}", dir.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("无法读取 {} 的文件类型: {e}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "symlinked package source entry is not allowed: {}",
                entry.path().display()
            ));
        }
        let p = entry.path();
        if p.is_dir() {
            collect_starry(&p, src_root, out, owner)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("starry") {
            out.push((p, src_root.to_path_buf(), owner));
        }
    }
    Ok(())
}

/// 合并源集：每个文件一段，记录路径、起始偏移、原文、注入前缀长与所属包，
/// 供诊断归属和跨包解析（M5 slice 2a：目录默认包前缀注入会平移段内 span）。
struct SourceUnit {
    path: PathBuf,
    base: u32,
    text: String,
    delta: u32,
    owner: PackageId,
}

struct SourceSet {
    merged: String,
    files: Vec<SourceUnit>,
}

fn dir_package(f: &Path, src_root: &Path) -> Option<String> {
    let rel = f.parent()?.strip_prefix(src_root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn has_package_header(text: &str) -> bool {
    let lead = text.len() - text.trim_start_matches([' ', '\t', '\r', '\n']).len();
    text[lead..].starts_with("package ")
}

fn merge_sources(files: &[(PathBuf, PathBuf, PackageId)]) -> Result<SourceSet, String> {
    let mut merged = String::new();
    let mut table = Vec::new();
    for (f, src_root, owner) in files {
        let text =
            std::fs::read_to_string(f).map_err(|e| format!("无法读取 {}: {e}", f.display()))?;
        let (body, delta) = if has_package_header(&text) {
            (text.clone(), 0)
        } else {
            match dir_package(f, src_root) {
                Some(pkg) => {
                    let prefix = format!("package {pkg}\n");
                    (format!("{prefix}{text}"), prefix.len() as u32)
                }
                None => (text.clone(), 0),
            }
        };
        table.push(SourceUnit {
            path: f.clone(),
            base: merged.len() as u32,
            text,
            delta,
            owner: *owner,
        });
        merged.push_str(&body);
        merged.push('\n');
    }
    Ok(SourceSet {
        merged,
        files: table,
    })
}

/// 诊断渲染：按 span 偏移定位原文件，打 `file:line:col`
fn print_set_diags(set: &SourceSet, diags: &[ast::syntax::node::Diagnostic]) {
    for d in diags {
        let pos = d.span.start as usize;
        let idx = set
            .files
            .iter()
            .rposition(|unit| (unit.base as usize) <= pos)
            .unwrap_or(0);
        let unit = &set.files[idx];
        let (line, col) = line_col(&unit.text, pos - unit.base as usize - unit.delta as usize);
        eprintln!(
            "{}:{line}:{col}: {:?}: {}",
            unit.path.display(),
            d.code,
            d.message
        );
    }
}

/// Enforce the v1 direct-import rule over manifest package owners. Dotted
/// source package segments inside one package root remain package-local, while
/// imports into another PackageId must name a direct dependency.
fn check_direct_imports(set: &SourceSet, graph: &PackageGraph) -> Result<(), String> {
    let dotted = |path: &ast::syntax::node::Path, interner: &ast::interner::Interner| {
        path.segments
            .iter()
            .map(|segment| interner.resolve(segment.name))
            .collect::<Vec<_>>()
            .join(".")
    };
    let mut owners = std::collections::BTreeMap::new();
    for unit in &set.files {
        let parsed = parser::parse_source(&unit.text).map_err(|e| e.to_string())?;
        if let Some(package) = &parsed.ast.package {
            owners.insert(dotted(package, &parsed.interner), unit.owner);
        }
    }
    for unit in &set.files {
        let parsed = parser::parse_source(&unit.text).map_err(|e| e.to_string())?;
        let current = match &parsed.ast.package {
            Some(package) => dotted(package, &parsed.interner),
            None => {
                let root = graph.package(unit.owner).root.clone();
                dir_package(&unit.path, &root).unwrap_or_default()
            }
        };
        for item in &parsed.ast.items {
            let DeclKind::Import(import) = &item.kind else {
                continue;
            };
            let ImportKind::Qualified { path, .. } = &import.kind else {
                continue;
            };
            let segments: Vec<&str> = path
                .segments
                .iter()
                .map(|segment| parsed.interner.resolve(segment.name))
                .collect();
            let Some(target) = (1..=segments.len())
                .rev()
                .map(|end| segments[..end].join("."))
                .find(|candidate| owners.contains_key(candidate))
            else {
                continue;
            };
            let Some(&owner) = owners.get(&target) else {
                unreachable!("candidate was selected from owners")
            };
            if owner == unit.owner {
                continue;
            }
            let current_package = graph.package(unit.owner);
            let allowed = current_package
                .dependencies
                .iter()
                .any(|dependency| graph.package_id(&dependency.key) == Some(owner));
            if !allowed {
                return Err(format!(
                    "package `{current}` imports package `{target}` without a direct dependency"
                ));
            }
        }
    }
    Ok(())
}

/// Verify each merged unit still belongs to the package root that loaded it.
/// This turns graph ownership into a checked invariant instead of documentation.
fn check_source_ownership(set: &SourceSet, graph: &PackageGraph) -> Result<(), String> {
    for unit in &set.files {
        let package = graph.package(unit.owner);
        if !unit.path.starts_with(&package.root) {
            return Err(format!(
                "{} belongs to package `{}` but is outside its canonical root {}",
                unit.path.display(),
                package.manifest.name,
                package.root.display()
            ));
        }
    }
    Ok(())
}

enum BuildInput {
    Single(PathBuf),
    Multi {
        root: PathBuf,
        graph: PackageGraph,
        set: SourceSet,
    },
}

fn resolve_build_input(path: &str) -> Result<BuildInput, String> {
    let p = Path::new(path);
    if p.is_dir() || p.extension().and_then(|e| e.to_str()) == Some("toml") {
        let root = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        let manifest = if p.is_dir() {
            let manifest_path = root.join("starry.toml");
            manifest_path
                .is_file()
                .then(|| parse_manifest(&manifest_path).map_err(|e| e.to_string()))
                .transpose()?
        } else {
            Some(parse_manifest(p).map_err(|e| e.to_string())?)
        };
        let graph = load_graph(&root, manifest).map_err(|e| e.to_string())?;
        let mut files: Vec<(PathBuf, PathBuf, PackageId)> = Vec::new();
        for id in graph.order(graph.root()) {
            let package = graph.package(id);
            let src_root = if package.root.join("src").is_dir() {
                package.root.join("src")
            } else {
                package.root.clone()
            };
            collect_starry(&src_root, &src_root, &mut files, id)?;
            let package = graph.package(id);
            collect_starry(&package.root, &package.root, &mut files, id)?;
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files.dedup_by(|a, b| a.0 == b.0);
        if files.is_empty() {
            let root = graph.package(graph.root());
            return Err(package::PackageGraphError::NoSources {
                package: root.manifest.name.clone(),
                root: root.root.clone(),
            }
            .to_string());
        }
        let set = merge_sources(&files)?;
        check_source_ownership(&set, &graph)?;
        check_direct_imports(&set, &graph)?;
        package::check_import_cycles(&set.merged).map_err(|e| e.to_string())?;
        Ok(BuildInput::Multi { root, graph, set })
    } else {
        Ok(BuildInput::Single(p.to_path_buf()))
    }
}
fn cmd_build(path: &str, output: Option<&str>, link_args: &[String], release: bool) -> ExitCode {
    let input = match resolve_build_input(path) {
        Ok(input) => input,
        Err(msg) => {
            eprintln!("starryc: {msg}");
            return ExitCode::FAILURE;
        }
    };
    let text = match &input {
        BuildInput::Single(p) => match compile_ll(p.to_str().unwrap_or(path)) {
            Ok(text) => text,
            Err(code) => return code,
        },
        BuildInput::Multi { set, .. } => {
            match compile_source(&set.merged, &|diags| print_set_diags(set, diags)) {
                Ok(text) => text,
                Err(code) => return code,
            }
        }
    };
    if !text.contains("define i32 @main()") {
        eprintln!("starryc: {path}: 缺少入口函数 `fun main()`");
        return ExitCode::FAILURE;
    }
    let exe = output.map(PathBuf::from).unwrap_or_else(|| match &input {
        BuildInput::Single(_) => {
            let source = Path::new(path);
            if cfg!(windows) {
                source.with_extension("exe")
            } else {
                source.with_extension("")
            }
        }
        BuildInput::Multi { root, graph, .. } => {
            let name = graph.package(graph.root()).manifest.name.clone();
            if cfg!(windows) {
                root.join(format!("{name}.exe"))
            } else {
                root.join(name)
            }
        }
    });
    let pid = std::process::id();
    let dir = std::env::temp_dir();
    let ll_path = dir.join(format!("starry_build_{pid}.ll"));
    // Repository common.c includes the header; the embedded snapshot is
    // self-contained so concurrent builds cannot race on a temp include.
    let common_path = dir.join(format!("starry_runtime_common_{pid}.c"));
    let platform_path = dir.join(format!(
        "starry_runtime_{}_{}.c",
        if cfg!(windows) { "win32" } else { "posix" },
        pid
    ));
    if let Err(e) = std::fs::write(&ll_path, text.as_bytes())
        .and_then(|_| {
            std::fs::write(
                &common_path,
                RUNTIME_COMMON_C
                    .replacen("#include \"starry_runtime.h\"", RUNTIME_HEADER, 1)
                    .as_bytes(),
            )
        })
        .and_then(|_| {
            std::fs::write(
                &platform_path,
                if cfg!(windows) {
                    RUNTIME_WIN32_C.as_bytes()
                } else {
                    RUNTIME_POSIX_C.as_bytes()
                },
            )
        })
    {
        eprintln!("starryc: 无法写临时构建文件: {e}");
        return ExitCode::FAILURE;
    }
    let clang_path = llvm_tool_path("clang");
    let mut clang = std::process::Command::new(&clang_path);
    clang.arg(&ll_path).arg(&common_path).arg(&platform_path);
    if cfg!(windows) {
        // Windows stays freestanding: the runtime provides mainCRTStartup.
        clang.arg("-nostdlib").arg("-lkernel32");
        // P1-A0-0：默认 1MB 栈预留 < 大聚合数组帧——8MB 保证安全
        clang.arg("-Wl,/STACK:8388608");
    }
    if release {
        clang.arg("-O2");
    }
    // `--` 透传的链接参数（extern "c" FFI：额外目标文件 / -l / -L）
    for arg in link_args {
        clang.arg(arg);
    }
    let status = clang.arg("-o").arg(&exe).status();
    let _ = std::fs::remove_file(&ll_path);
    let _ = std::fs::remove_file(&common_path);
    let _ = std::fs::remove_file(&platform_path);
    match status {
        Ok(status) if status.success() => {
            println!("{}", exe.display());
            ExitCode::SUCCESS
        }
        Ok(status) => {
            eprintln!("starryc: clang 链接失败（exit {status}）");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("starryc: 无法启动 clang（已查 PATH 与常见 LLVM 安装位）: {e}");
            ExitCode::FAILURE
        }
    }
}
