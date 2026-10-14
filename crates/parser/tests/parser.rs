use ast::syntax::decl::DeclKind;
use ast::syntax::expr::ExprKind;
use ast::syntax::ty::TypeKind;
use parser::{Parser, parse_source};

fn canonical_snapshot(source: &str, output: &parser::ParseOutput) -> String {
    let mut snapshot = String::new();
    snapshot.push_str("SOURCE\n");
    snapshot.push_str(source);
    snapshot.push_str("\n\nSYMBOLS\n");
    for index in 0..output.interner.len() {
        snapshot.push_str(&format!(
            "{} = {}\n",
            index,
            output.interner.resolve(ast::interner::Symbol(index as u32))
        ));
    }
    snapshot.push_str("\nDIAGNOSTICS\n");
    for diagnostic in &output.diagnostics {
        snapshot.push_str(&format!(
            "{:?} {:?} {:?} {}\n",
            diagnostic.code, diagnostic.span, diagnostic.severity, diagnostic.message
        ));
    }
    snapshot.push_str("\nNODES\n");
    snapshot.push_str(&output.node_count.to_string());
    snapshot.push_str("\n\nAST\n");
    snapshot.push_str(&format!("{:#?}", output.ast));
    snapshot
}

fn check_snapshot(name: &str, source: &str) {
    let output = parse_source(source).unwrap();
    let snapshot = canonical_snapshot(source, &output).trim_end().to_string();
    let path = std::env::var("STARTRY_SNAPSHOT_DIR")
        .unwrap_or_else(|_| format!("tests/parser_{name}.snap"))
        .replace("{name}", name);
    if std::env::var("UPDATE_SNAP").is_ok_and(|value| value == "1") {
        std::fs::write(&path, snapshot).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("missing snapshot {path}: {error}"))
        .trim_end()
        .to_string();
    assert_eq!(snapshot, expected, "snapshot {name} drifted");
}

fn binding_init(source: &str) -> ExprKind {
    let output = parse_source(source).expect("parse should not fail at machine layer");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.ast.items.len(), 1);
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding, got {:?}", output.ast.items[0].kind);
    };
    binding
        .init
        .as_ref()
        .expect("binding has an initializer")
        .kind
        .clone()
}

#[test]
fn empty_source_has_canonical_root() {
    let output = parse_source("").unwrap();
    assert_eq!(output.node_count, 1);
    assert_eq!(output.ast.id.0, 0);
    assert_eq!(output.ast.span.start, 0);
    assert_eq!(output.ast.span.end, 0);
    assert!(output.ast.package.is_none());
    assert!(output.ast.items.is_empty());
    assert!(output.diagnostics.is_empty());
}

#[test]
fn package_prefix_parses_into_root_slot() {
    let output = parse_source("package app.core\nval x = 1").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let package = output.ast.package.as_ref().unwrap();
    assert_eq!(output.interner.resolve(package.segments[0].name), "app");
    assert_eq!(output.interner.resolve(package.segments[1].name), "core");
    assert_eq!(output.ast.items.len(), 1);
}

#[test]
fn val_binding_produces_real_ast() {
    let init = binding_init("val x = 1 + 2");
    let ExprKind::Binary { op, lhs, rhs } = init else {
        panic!("expected binary init");
    };
    assert_eq!(op, ast::token::OperatorKind::Plus);
    assert!(matches!(lhs.kind, ExprKind::Literal(_)));
    assert!(matches!(rhs.kind, ExprKind::Literal(_)));
}

#[test]
fn binding_can_have_named_type() {
    let output = parse_source("val x: int = 1").unwrap();
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding");
    };
    let ty = binding.ty.as_ref().unwrap();
    let TypeKind::Named { path, type_args } = &ty.kind else {
        panic!("expected named type");
    };
    assert_eq!(output.interner.resolve(path.segments[0].name), "int");
    assert!(type_args.is_empty());
}

#[test]
fn pratt_multiplication_binds_tighter_than_addition() {
    let init = binding_init("val x = 1 + 2 * 3");
    let ExprKind::Binary { op, lhs, rhs } = init else {
        panic!("expected binary init");
    };
    assert_eq!(op, ast::token::OperatorKind::Plus);
    assert!(matches!(lhs.kind, ExprKind::Literal(_)));
    assert!(matches!(rhs.kind, ExprKind::Binary { .. }));
}

#[test]
fn subtraction_is_left_associative() {
    let init = binding_init("val x = 10 - 4 - 3");
    let ExprKind::Binary { op, lhs, .. } = init else {
        panic!("expected binary init");
    };
    assert_eq!(op, ast::token::OperatorKind::Minus);
    let ExprKind::Binary { op, .. } = lhs.kind else {
        panic!("expected nested binary lhs");
    };
    assert_eq!(op, ast::token::OperatorKind::Minus);
}

#[test]
fn true_false_null_use_keyword_ast_variants() {
    assert!(matches!(binding_init("val t = true"), ExprKind::Bool(true)));
    assert!(matches!(
        binding_init("val f = false"),
        ExprKind::Bool(false)
    ));
    assert!(matches!(binding_init("val n = null"), ExprKind::Null));
}

#[test]
fn zero_arg_and_it_lambdas_parse() {
    assert!(matches!(
        binding_init("val f = { -> 42 }"),
        ExprKind::Lambda { .. }
    ));
    assert!(matches!(
        binding_init("val f = { it + 1 }"),
        ExprKind::Lambda { .. }
    ));
    assert!(matches!(
        binding_init("val f = { x -> x + 1 }"),
        ExprKind::Lambda { .. }
    ));
}

#[test]
fn slot_words_can_be_binding_names_but_hard_keywords_cannot() {
    let output = parse_source("val move = 1").unwrap();
    assert!(matches!(output.ast.items[0].kind, DeclKind::Binding(_)));
    let output = parse_source("val it = 1").unwrap();
    assert!(matches!(output.ast.items[0].kind, DeclKind::Binding(_)));

    let output = parse_source("val mut = 1").unwrap();
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        binding.pattern.kind,
        ast::syntax::pat::PatKind::Error(_)
    ));
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn is_and_in_expressions_parse() {
    assert!(matches!(
        binding_init("val b = x is T"),
        ExprKind::Is { .. }
    ));
    assert!(matches!(
        binding_init("val b = x in xs"),
        ExprKind::In { negated: false, .. }
    ));
    assert!(matches!(
        binding_init("val b = x !in xs"),
        ExprKind::In { negated: true, .. }
    ));
}

#[test]
fn generic_call_is_speculatively_parsed_without_ghost_diagnostics() {
    let init = binding_init("val v = f<int>(1)");
    let ExprKind::Call {
        type_args, args, ..
    } = init
    else {
        panic!("expected generic call");
    };
    assert_eq!(type_args.len(), 1);
    assert_eq!(args.len(), 1);

    // The `<` is relational here; the failed generic-call attempt must roll
    // back both cursor state and diagnostics.
    let output = parse_source("val b = f < int && x > 1").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(matches!(
        binding_init("val b = f < int && x > 1"),
        ExprKind::Binary { .. }
    ));
}

#[test]
fn deleted_let_token_produces_partial_ast_and_diagnostic() {
    let output = parse_source("let x = 1").unwrap();
    assert!(matches!(output.ast.items[0].kind, DeclKind::Error(_)));
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn if_val_refutable_binding_parses_without_let() {
    let output = parse_source("val r = if val Some(x) = opt { x } else { 0 }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn patterns_support_or_and_range() {
    let output = parse_source("val 1 | 2 | 3 = x").unwrap();
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        binding.pattern.kind,
        ast::syntax::pat::PatKind::Or(_)
    ));

    let output = parse_source("val 1..=9 = x").unwrap();
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        binding.pattern.kind,
        ast::syntax::pat::PatKind::Range {
            inclusive: true,
            ..
        }
    ));
}

#[test]
fn type_nesting_is_bounded_without_stack_overflow() {
    let depth = 150;
    let mut source = String::from("val x: ");
    source.push_str(&"A<".repeat(depth));
    source.push('T');
    source.push_str(&">".repeat(depth));
    source.push_str(" = x");
    let output = parse_source(&source).unwrap();
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn aggregate_declarations_parse_without_placeholders() {
    let source = r#"
        package app.core
        import std.io.*
        import "c" "math.h" as cmath { fun sin(x: f64) : f64; }

        public fun id<T>(x: T) : T where T: Show = x
        public val pattern = value

        class Point(x: int, y: int) : Shape {
            init { }
            get { x }
        }

        actor Worker(counter: int) { }
        aspect Logging {
        before Point.run { }
        }
        provide Services {
            singleton Clock -> SystemClock()
            bind Clock to SystemClock
        }
        impl Show for Point { }
        macro repeat(name) { }
        comptime { 1 + 1 }
    "#;
    let output = parse_source(source).unwrap();
    for diag in output.diagnostics.clone() {
        let end = (diag.span.end as usize).min(source.len());
        let start = (diag.span.start as usize).min(end);
        eprintln!("{:?} at {:?}", diag, &source[start..end]);
    }
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
    assert!(output.ast.package.is_some());
    assert_eq!(output.ast.items.len(), 11);
}

#[test]
fn function_generic_decl_parses() {
    let output = parse_source("public fun id<T>(x: T) : T where T: Show = x").unwrap();
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
}

#[test]
fn class_decl_parses() {
    let output =
        parse_source("class Point(x: int, y: int) : Shape { init { } get { x } }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
}

#[test]
fn provider_decl_parses() {
    let output = parse_source(
        "provide Services { singleton Clock -> SystemClock() bind Clock to SystemClock }",
    )
    .unwrap();
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
}

#[test]
fn statements_and_control_flow_parse() {
    let source = r#"fun demo() {
'outer: for x in items {
            while val Some(y) = next() {
if y > 10 { break 'outer }
                continue
            }
            loop { break }
            count += 1
            defer { cleanup() }
            return count
        }
    }
    "#;
    let output = parse_source(source).unwrap();
    assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
}

#[test]
fn simple_control_flow_parse() {
    for source in [
        "fun a() { loop { break } }",
        "fun a() { 'outer: for x in xs { break 'outer } }",
        "fun a() { while val Some(x) = xs { continue } }",
        "fun a() { if x > 1 { return 1 } return 2 }",
        "fun a() { count += 1 }",
        "fun a() { defer { cleanup() } }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:#?}",
            output.diagnostics
        );
    }
}

#[test]
fn newline_terminates_block_statement() {
    let output = parse_source("fun d() : int {\n    val a = 1\n    -2\n}").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun, got {:?}", output.ast.items[0].kind);
    };
    let Some(ast::syntax::decl::FunctionBody::Block(block)) = &fun.body else {
        panic!("expected block body");
    };
    assert_eq!(block.stmts.len(), 1);
    assert!(
        block.tail.is_some(),
        "trailing -2 must stay a separate statement"
    );
}

#[test]
fn trailing_operator_continues_line() {
    let output = parse_source("fun d() {\n    val a = 1 +\n    2\n}").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun, got {:?}", output.ast.items[0].kind);
    };
    let Some(ast::syntax::decl::FunctionBody::Block(block)) = &fun.body else {
        panic!("expected block body");
    };
    assert_eq!(block.stmts.len(), 1);
    assert!(
        block.tail.is_none(),
        "trailing + must continue the initializer"
    );
}

#[test]
fn when_branches_separate_by_newline_without_comma() {
    let output = parse_source(
        "fun g(s: int) : int = when s {\n    90..=100 -> 5\n    60..89 -> 3\n    else -> 0\n}",
    )
    .unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn when_pattern_branch_with_guard_parses() {
    let output =
        parse_source("fun f(s: int) : int = when s { 1..=9 if flag -> 1, else -> 0 }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn when_expression_branch_falls_back_without_arrow_pattern() {
    let output = parse_source("fun f(x: int) : int = when x { x > 1 -> 1, else -> 0 }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn quoted_label_break_with_value() {
    let output =
        parse_source("fun f() : int {\n    'outer: loop {\n        break 'outer 42\n    }\n}")
            .unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn struct_fields_allow_trailing_comma() {
    let output = parse_source("struct P {\n    x: int,\n    y: int,\n}").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn when_branch_bodies_accept_jump_statements() {
    let output = parse_source(
        "fun g(e: int) {\n    loop {\n        when e {\n            1 -> break,\n            else -> continue,\n        }\n    }\n}",
    )
    .unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn null_safe_postfix_chain_parses() {
    let output =
        parse_source("fun f(u: User) : String {\n    return u?.profile?.name!!\n}").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn named_arguments_parse() {
    let output = parse_source("fun f(a: int, b: int) : int { return f(a = 1, b = 2) }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let output = parse_source("fun f(a: int, b: int) : int { return f(1, b = 2) }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn fixed_arrays_parse() {
    let output = parse_source(
        "fun depth(z: [f64; 4096]) : f64 {\n    val buf = [0.0; 4096]\n    z[1] = buf[0]\n    return z[0]\n}",
    )
    .unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn operator_fun_parses_with_slot_words_as_identifiers() {
    let output = parse_source("operator fun plus(by: f64, to: f64) : f64 = by + to").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn repeated_parse_is_structurally_deterministic() {
    let source = "val x = 1 + 2 * 3";
    let parser = Parser::new().unwrap();
    let first = parser.parse(source).unwrap();
    let second = parser.parse(source).unwrap();
    assert_eq!(format!("{:?}", first.ast), format!("{:?}", second.ast));
    assert_eq!(first.node_count, second.node_count);
    assert_eq!(first.interner.len(), second.interner.len());
}

#[test]
fn parser_snapshot_is_stable() {
    check_snapshot("pipeline", "val x = 1");
}

#[test]
fn unsafe_proceed_concurrent_parse() {
    assert!(matches!(
        binding_init("val a = unsafe { 1 }"),
        ExprKind::Unsafe { .. }
    ));
    assert!(matches!(binding_init("val p = proceed"), ExprKind::Proceed));
    assert!(matches!(
        binding_init("val c = concurrent { 1 }"),
        ExprKind::Concurrent { .. }
    ));
}

#[test]
fn spawn_actor_construction_parses() {
    let output = parse_source("fun f() { spawn actor Worker(2) { work() } }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun, got {:?}", output.ast.items[0].kind);
    };
    let Some(ast::syntax::decl::FunctionBody::Block(block)) = &fun.body else {
        panic!("expected block body");
    };
    let Some(tail) = &block.tail else {
        panic!("expected spawn tail expression");
    };
    let ExprKind::Spawn(spawn) = &tail.kind else {
        panic!("expected spawn, got {:?}", tail.kind);
    };
    let actor = spawn.actor.as_ref().expect("actor construction");
    assert_eq!(
        output.interner.resolve(actor.name.segments[0].name),
        "Worker"
    );
    assert_eq!(actor.args.len(), 1);
}

#[test]
fn plain_and_escaped_strings_stay_literal() {
    assert!(matches!(
        binding_init("val s = \"plain\""),
        ExprKind::Literal(_)
    ));
    assert!(matches!(
        binding_init("val s = \"cost \\$5\""),
        ExprKind::Literal(_)
    ));
}

#[test]
fn string_interpolation_splits_parts() {
    let interp = binding_init("val s = \"x ${a + b} y $name!\"");
    let ExprKind::StringInterp { parts } = interp else {
        panic!("expected interpolation, got {interp:?}");
    };
    assert_eq!(parts.len(), 5);
    assert!(matches!(parts[0].kind, ExprKind::Literal(_)));
    assert!(matches!(parts[1].kind, ExprKind::Binary { .. }));
    assert!(matches!(parts[2].kind, ExprKind::Literal(_)));
    assert!(matches!(parts[3].kind, ExprKind::Ident(_)));
    assert!(matches!(parts[4].kind, ExprKind::Literal(_)));
}

#[test]
fn unclosed_interpolation_emits_diagnostic() {
    let output = parse_source("val s = \"${x\"").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::UnclosedInterpolation)
    );
}
