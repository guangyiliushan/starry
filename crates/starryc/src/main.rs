//! starryc — Starry 编译器入口
//!
//! 当前提供 regex → NFA 管道的两个子命令：
//! - `starryc match <regex> <input>`：打印最长匹配长度
//!   （exit 0 匹配 / exit 1 无匹配 / exit 2 用法错误）
//! - `starryc dump-nfa <regex>`：打印 NFA 状态与边

use std::process::ExitCode;

use ast::token::LiteralKind;
use lex::nfa::NFA;
use lex::display::write_escaped_str;
use lex::lexer::Lexer;
use lex::regex::parse::parse;
use lex::regex::Translator;
use lex::TokenKind;

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
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("用法: starryc match <regex> <input> | starryc dump-nfa <regex> | starryc tokenize <source>");
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
    Lexer::build(vec![
        ("[a-zA-Z_][a-zA-Z0-9_]*", TokenKind::Identifier),
        ("[0-9]+", TokenKind::Literal(LiteralKind::Integer)),
    ])
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
        let _ = write_escaped_str(&mut lexeme, token.lexeme.as_ref()); // String 写入不失败
        println!(
            "{}..{}\t{}\t{}",
            token.span.start, token.span.end, token.kind, lexeme
        );
    }
    ExitCode::SUCCESS
}

