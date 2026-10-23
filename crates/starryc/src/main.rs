//! starryc — Starry 编译器入口
//!
//! 当前提供四个子命令：
//! - `starryc match <regex> <input>`：打印最长匹配长度
//!   （exit 0 匹配 / exit 1 无匹配 / exit 2 用法错误）
//! - `starryc dump-nfa <regex>`：打印 NFA 状态与边
//! - `starryc tokenize <source>`：词法 token 流
//! - `starryc check <file>`：parse → sem 全链诊断
//!   （exit 0 干净 / exit 1 有诊断，逐条 stderr `path:start..end: code: msg`）

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
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "用法: starryc match <regex> <input> | starryc dump-nfa <regex> | starryc tokenize <source> | starryc check <file>"
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
