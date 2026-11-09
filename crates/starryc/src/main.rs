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

use lex::TokenKind;
use lex::display::write_escaped_str;
use lex::lexer::Lexer;
use lex::nfa::NFA;
use lex::regex::Translator;
use lex::regex::parse::parse;

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
        [cmd, rest @ ..] if cmd == "build" => cmd_build_args(rest),
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
            let _ = std::fs::remove_file(&ll);
            let _ = std::fs::remove_file(&bc);
            match out {
                Ok(o) if o.status.success() => return Ok(()),
                Ok(o) => return Err(String::from_utf8_lossy(&o.stderr).to_string()),
                Err(_) => {}
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
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&obj);
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(_) => Ok(()),
    }
}

/// starryc build：.ll + 内嵌 C runtime → clang → 可执行文件。
/// runtime 不是 Cargo 依赖；源码在编译 starryc 时经 include_str! 固定。
const RUNTIME_C: &str = include_str!("../../../runtime/starry_runtime.c");

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
fn cmd_build_args(args: &[String]) -> ExitCode {
    let (head, link_args) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[][..]),
    };
    match head {
        [path] => cmd_build(path, None, link_args),
        [path, output] => cmd_build(path, Some(output), link_args),
        _ => usage(),
    }
}

// ==================== M5 模块系统（切片 1：manifest + 多文件合并） ====================
//
// v1 模型：**合并根作用域**——srcs 下所有 .starry 剥掉 package 头后按
// 确定性序（路径排序）拼成一个虚拟源，一次 parse/sem/irc 全链。import
// 在合并根作用域内解析（跨文件天然可见）；import 循环在单实例模型下
// 自然消解（注册表/增量/循环检测后置，见 implementation-plan M5）。
// 诊断经偏移表归属回原文件（package 头剥除以空白填充，行列不漂移）。

/// starry.toml 的 v1 最小集（std-only 手写行解析；未知键忽略前向兼容）
#[derive(Debug, Default)]
struct Manifest {
    name: Option<String>,
    srcs: Vec<String>,
}

fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取 manifest {}: {e}", path.display()))?;
    let mut m = Manifest::default();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("{}:{}: 期望 key = value", path.display(), i + 1));
        };
        let k = k.trim();
        let v = v.trim();
        match k {
            "name" => m.name = Some(unquote(v)?),
            "srcs" => m.srcs = parse_str_list(v)?,
            _ => {} // 未知键：前向兼容忽略
        }
    }
    Ok(m)
}

fn unquote(v: &str) -> Result<String, String> {
    v.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string())
        .ok_or_else(|| format!("manifest 值须为双引号字符串: {v}"))
}

fn parse_str_list(v: &str) -> Result<Vec<String>, String> {
    let inner = v
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| format!("manifest 列表须为 [\"a\", \"b\"] 形: {v}"))?;
    inner.split(',').map(|s| unquote(s.trim())).collect()
}

/// 递归收集 dir 下全部 .starry（调用方排序去重，保证确定性）
fn collect_starry(
    dir: &Path,
    src_root: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("无法读取源目录 {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_starry(&p, src_root, out)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("starry") {
            out.push((p, src_root.to_path_buf()));
        }
    }
    Ok(())
}

/// 合并源集：每个文件一段，记录 (路径, 起始偏移, 原文, 注入前缀长)
/// 供诊断归属（M5 slice 2a：目录默认包前缀注入会平移段内 span）
struct SourceSet {
    merged: String,
    files: Vec<(PathBuf, u32, String, u32)>,
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

fn merge_sources(files: &[(PathBuf, PathBuf)]) -> Result<SourceSet, String> {
    let mut merged = String::new();
    let mut table = Vec::new();
    for (f, src_root) in files {
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
        table.push((f.clone(), merged.len() as u32, text, delta));
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
            .rposition(|(_, base, _, _)| (*base as usize) <= pos)
            .unwrap_or(0);
        let (path, base, text, delta) = &set.files[idx];
        let (line, col) = line_col(text, pos - *base as usize - *delta as usize);
        eprintln!(
            "{}:{line}:{col}: {:?}: {}",
            path.display(),
            d.code,
            d.message
        );
    }
}

enum BuildInput {
    Single(PathBuf),
    Multi {
        root: PathBuf,
        manifest: Manifest,
        set: SourceSet,
    },
}

fn resolve_build_input(path: &str) -> Result<BuildInput, String> {
    let p = Path::new(path);
    if p.is_dir() || p.extension().and_then(|e| e.to_str()) == Some("toml") {
        let (root, manifest) = if p.is_dir() {
            let mp = p.join("starry.toml");
            let m = if mp.is_file() {
                parse_manifest(&mp)?
            } else {
                Manifest::default()
            };
            (p.to_path_buf(), m)
        } else {
            (
                p.parent().unwrap_or(Path::new(".")).to_path_buf(),
                parse_manifest(p)?,
            )
        };
        let srcs = if manifest.srcs.is_empty() {
            vec![".".to_string()]
        } else {
            manifest.srcs.clone()
        };
        let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
        for s in &srcs {
            let sr = root.join(s);
            collect_starry(&sr, &sr, &mut files)?;
        }
        files.sort();
        files.dedup();
        if files.is_empty() {
            return Err(format!("{}: 没有找到 .starry 源文件", root.display()));
        }
        let set = merge_sources(&files)?;
        Ok(BuildInput::Multi {
            root,
            manifest,
            set,
        })
    } else {
        Ok(BuildInput::Single(p.to_path_buf()))
    }
}
fn cmd_build(path: &str, output: Option<&str>, link_args: &[String]) -> ExitCode {
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
        BuildInput::Multi { root, manifest, .. } => {
            let name = manifest.name.clone().unwrap_or_else(|| {
                root.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "starry-app".to_string())
            });
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
    let c_path = dir.join(format!("starry_runtime_{pid}.c"));
    if let Err(e) = std::fs::write(&ll_path, text.as_bytes())
        .and_then(|_| std::fs::write(&c_path, RUNTIME_C.as_bytes()))
    {
        eprintln!("starryc: 无法写临时构建文件: {e}");
        return ExitCode::FAILURE;
    }
    let clang_path = llvm_tool_path("clang");
    let mut clang = std::process::Command::new(&clang_path);
    clang.arg(&ll_path).arg(&c_path);
    if cfg!(windows) {
        // The bundled runtime is freestanding and provides mainCRTStartup.
        clang.arg("-nostdlib").arg("-lkernel32");
    }
    // `--` 透传的链接参数（extern "c" FFI：额外目标文件 / -l / -L）
    for arg in link_args {
        clang.arg(arg);
    }
    let status = clang.arg("-o").arg(&exe).status();
    let _ = std::fs::remove_file(&ll_path);
    let _ = std::fs::remove_file(&c_path);
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
            eprintln!("starryc: 无法启动 clang（已查 PATH 与 C:\\Program Files\\LLVM\\bin）: {e}");
            ExitCode::FAILURE
        }
    }
}
