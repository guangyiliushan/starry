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
        [cmd, path] if cmd == "build" => cmd_build(path, None),
        [cmd, path, output] if cmd == "build" => cmd_build(path, Some(output)),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "用法: starryc match <regex> <input> | starryc dump-nfa <regex> | starryc tokenize <source> | starryc check <file> | starryc irc <file> | starryc opt <file> | starryc llvme <file> | starryc build <file> [output]"
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

/// parse → sem 全链：诊断走 stderr（`path:start..end: code: message`），
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
    for d in &output.diagnostics {
        eprintln!(
            "{path}:{}..{}: {:?}: {}",
            d.span.start, d.span.end, d.code, d.message
        );
    }
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
        for d in &output.diagnostics {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
        return ExitCode::FAILURE;
    }
    let tac = irc::lower(&mut output, &source);
    if !tac.diags.is_empty() {
        for d in &tac.diags {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
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
        for d in &output.diagnostics {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
        return ExitCode::FAILURE;
    }
    let mut tac = irc::lower(&mut output, &source);
    if !tac.diags.is_empty() {
        for d in &tac.diags {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
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
    let parsed = match parser::parse_source(&source) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("starryc: {path}: 解析失败（机器层）: {e}");
            return Err(ExitCode::FAILURE);
        }
    };
    let mut output = sem::analyze(parsed);
    if !output.diagnostics.is_empty() {
        for d in &output.diagnostics {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
        return Err(ExitCode::FAILURE);
    }
    let mut tac = irc::lower(&mut output, &source);
    if !tac.diags.is_empty() {
        for d in &tac.diags {
            eprintln!(
                "{path}:{}..{}: {:?}: {}",
                d.span.start, d.span.end, d.code, d.message
            );
        }
        return Err(ExitCode::FAILURE);
    }
    opt::optimize(&mut tac, &output.tys);
    Ok(llvme::emit(
        &tac,
        &output.tys,
        &output.defs,
        &output.interner,
    ))
}

/// parse → sem → irc → opt → llvme：.ll 文本走 stdout；诊断同 irc 子命令
fn cmd_llvme(path: &str) -> ExitCode {
    match compile_ll(path) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

/// starryc build：.ll + 内嵌 C runtime → clang → 可执行文件。
/// runtime 不是 Cargo 依赖；源码在编译 starryc 时经 include_str! 固定。
const RUNTIME_C: &str = include_str!("../../../runtime/starry_runtime.c");

fn cmd_build(path: &str, output: Option<&str>) -> ExitCode {
    let text = match compile_ll(path) {
        Ok(text) => text,
        Err(code) => return code,
    };
    let exe = output.map(PathBuf::from).unwrap_or_else(|| {
        let source = Path::new(path);
        if cfg!(windows) {
            source.with_extension("exe")
        } else {
            source.with_extension("")
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
    let mut clang = std::process::Command::new("clang");
    clang.arg(&ll_path).arg(&c_path);
    if cfg!(windows) {
        // The bundled runtime is freestanding and provides mainCRTStartup.
        clang.arg("-nostdlib").arg("-lkernel32");
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
            eprintln!("starryc: 无法启动 clang（请确认 LLVM/clang 在 PATH 中）: {e}");
            ExitCode::FAILURE
        }
    }
}
