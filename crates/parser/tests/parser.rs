use ast::syntax::decl::{DeclKind, ParamModifier};
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

#[test]
fn constructor_modifiers_precede_init() {
    let output = parse_source("class C { public init() { } }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn parameter_modifiers_are_exclusive() {
    for (source, expected) in [
        ("fun f(val x: int) { }", ParamModifier::Val),
        ("fun f(var x: int) { }", ParamModifier::Var),
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
        let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
            panic!("expected fun, got {source}");
        };
        assert_eq!(fun.params[0].modifier, expected);
    }

    let output = parse_source("fun f(val mut x: int) { }").unwrap();
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn primary_constructor_properties_are_marked() {
    let output = parse_source("class C(val x: int, var y: int) { }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Class(class) = &output.ast.items[0].kind else {
        panic!("expected class");
    };
    let params = class.primary_ctor.as_ref().unwrap().params.as_slice();
    assert_eq!(params[0].modifier, ParamModifier::Val);
    assert_eq!(params[1].modifier, ParamModifier::Var);
}

#[test]
fn extern_namespace_newlines_parse() {
    let output =
        parse_source("extern \"c\" namespace math {\n    fun sin(x: f64) : f64;\n}").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn native_import_language_is_validated() {
    let output = parse_source("import \"rust\" \"crate.rs\"").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );
}

#[test]
fn macro_streams_preserve_balanced_groups() {
    let init = binding_init("val v = m!(f(1))");
    let ExprKind::MacroCall { tokens, .. } = init else {
        panic!("expected macro call");
    };
    let [
        ast::syntax::expr::TokenTree::Token(_),
        ast::syntax::expr::TokenTree::Group {
            tokens,
            close: Some(_),
            ..
        },
    ] = tokens.as_slice()
    else {
        panic!("expected inner group: {tokens:?}");
    };
    assert!(matches!(tokens[0], ast::syntax::expr::TokenTree::Token(_)));

    let init = binding_init("val v = m!([a, b])");
    let ExprKind::MacroCall { tokens, .. } = init else {
        panic!("expected macro call");
    };
    assert!(matches!(
        tokens[0],
        ast::syntax::expr::TokenTree::Group { .. }
    ));
}

#[test]
fn unterminated_macro_group_emits_diagnostic() {
    let output = parse_source("val v = m!(f(1)").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );
}

#[test]
fn declaration_nesting_is_bounded_without_stack_overflow() {
    let depth = 150;
    let mut source = "class C { ".repeat(depth);
    source.push_str(&"}".repeat(depth));
    let output = parse_source(&source).unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::DepthLimitExceeded)
    );
}

#[test]
fn oversized_array_lengths_report_out_of_range() {
    let output = parse_source("val a = [0; 999999999999999999999999]").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::IntegerOutOfRange)
    );
    let output = parse_source("fun f(x: [int; 999999999999999999999999]) { }").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::IntegerOutOfRange)
    );
}

#[test]
fn super_and_actor_constructions_preserve_named_args() {
    let output = parse_source("class C : Base { fun f() { super(a = 1) } }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Class(class) = &output.ast.items[0].kind else {
        panic!("expected class");
    };
    let Some(ast::syntax::decl::ClassMember::Declaration(declaration)) = class.members.first()
    else {
        panic!("expected declaration");
    };
    let ast::syntax::decl::DeclKind::Fun(fun) = &declaration.kind else {
        panic!("expected fun");
    };
    let Some(ast::syntax::decl::FunctionBody::Block(block)) = &fun.body else {
        panic!("expected block");
    };
    let tail = block.tail.as_ref().expect("expected tail expression");
    let ExprKind::Super { suffix } = &tail.kind else {
        panic!("expected super, got {:?}", tail.kind);
    };
    let ast::syntax::expr::SuperSuffix::Call(args) = suffix else {
        panic!("expected super call");
    };
    assert_eq!(args.len(), 1);
    assert_eq!(
        args[0].name.map(|symbol| output.interner.resolve(symbol)),
        Some("a")
    );

    let output = parse_source("fun f() { spawn actor Worker(a = 1) { work() } }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn imports_after_declarations_report_grammar_violation() {
    let output = parse_source("val x = 1\nimport std.io.*").unwrap();
    assert!(!output.diagnostics.is_empty());
    assert_eq!(output.ast.items.len(), 2);
}

#[test]
fn raw_native_and_extern_strings_decode_before_interning() {
    let output = parse_source(r#"import "c" "math.h""#).unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Import(import) = &output.ast.items[0].kind else {
        panic!("expected import");
    };
    let ast::syntax::decl::ImportKind::Native { language, path, .. } = &import.kind else {
        panic!("expected native import");
    };
    assert!(matches!(language, ast::syntax::decl::NativeLanguage::C));
    assert_eq!(output.interner.resolve(*path), "math.h");

    let output = parse_source(r#"extern "c" fun f();"#).unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Extern(extern_decl) = &output.ast.items[0].kind else {
        panic!("expected extern");
    };
    assert_eq!(output.interner.resolve(extern_decl.language), "c");
}

#[test]
fn superitem_by_consumes_slot_word_before_expression() {
    let output = parse_source("class C : Base by Delegate { }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Class(class) = &output.ast.items[0].kind else {
        panic!("expected class");
    };
    let supertype = class.supertypes.first().expect("supertype");
    let Some(by) = &supertype.by else {
        panic!("expected by expression");
    };
    assert!(matches!(by.kind, ExprKind::Ident(_)));
}

#[test]
fn type_parameters_validate_names_and_keep_exact_spans() {
    let source = "fun f<T>(x: T) = x";
    let output = parse_source(source).unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun");
    };
    let param = fun.type_params.first().expect("type param");
    assert_eq!(param.span, ast::token::Span::new(6, 7));

    let output = parse_source("fun f<1>() { }").unwrap();
    assert!(!output.diagnostics.is_empty());
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun");
    };
    let param = fun.type_params.first().expect("type param");
    assert!(matches!(
        param.name,
        ast::syntax::ty::TypeParamName::Error(_)
    ));
}

#[test]
fn variance_has_no_undeclared_in_out_state() {
    let output = parse_source("fun f<in out>(x: out) = x").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun");
    };
    let param = fun.type_params.first().expect("type param");
    assert_eq!(param.variance, Some(ast::syntax::ty::Variance::In));
    let ast::syntax::ty::TypeParamName::Valid(name) = &param.name else {
        panic!("expected valid type parameter name");
    };
    assert_eq!(output.interner.resolve(*name), "out");
}

#[test]
fn malformed_when_branches_emit_diagnostics() {
    let output = parse_source("fun f() { when x { else x } }").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );

    let output = parse_source("fun f() { when x {").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );
}

#[test]
fn unterminated_provider_and_aspect_members_reach_eof() {
    for source in ["provide P {", "aspect A {"] {
        let output = parse_source(source).unwrap();
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn function_body_is_required_and_class_body_is_optional() {
    let output = parse_source("fun f()").unwrap();
    let DeclKind::Fun(fun) = &output.ast.items[0].kind else {
        panic!("expected fun");
    };
    assert!(fun.body.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );

    let output = parse_source("class C").unwrap();
    let DeclKind::Class(class) = &output.ast.items[0].kind else {
        panic!("expected class");
    };
    assert!(class.members.is_empty());
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn bare_super_requires_a_suffix() {
    let output = parse_source("val x = super").unwrap();
    assert!(matches!(output.ast.items[0].kind, DeclKind::Binding(_)));
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );
}

#[test]
fn grow_requires_integer_bits_and_assignment_requires_place() {
    let output = parse_source("fun f() { grow x to 64 bits }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let output = parse_source("fun f() { grow x to y bits }").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );

    let output = parse_source("fun f(a: [int; 1]) { a[0] += 1 }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let output = parse_source("fun f(a: int, b: int) { a + b = c }").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::InvalidAssignmentTarget)
    );
}

#[test]
fn native_import_member_block_requires_close() {
    let output = parse_source(r#"import "c" "math.h" { fun f();"#).unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken)
    );
}

#[test]
fn interpolation_matching_uses_sublexical_tokens() {
    let output = parse_source(r#"val s = "${ '}' }""#).unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding");
    };
    let Some(init) = &binding.init else {
        panic!("expected initializer");
    };
    let ExprKind::StringInterp { parts } = &init.kind else {
        panic!("expected interpolation, got {:?}", init.kind);
    };
    assert_eq!(parts.len(), 1);
    assert!(matches!(parts[0].kind, ExprKind::Literal(_)));
}

#[test]
fn named_string_escape_errors_are_stable() {
    let output = parse_source(r#"import "c" "bad\q.h""#).unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == ast::syntax::node::DiagCode::InvalidEscape)
    );
}

// ---------------------------------------------------------------------------
// v0.4 gap closures: `Self`/`self`, unit type, `!is`, rest patterns, when-branch
// annotations, impl type params, intersection bounds, extern blocks,
// `FunSignature` terminators, and the `let` non-keyword.
// ---------------------------------------------------------------------------

use ast::syntax::decl::{ExternBody, FunctionBody, ImportKind};
use ast::syntax::node::DiagCode;
use ast::syntax::pat::PatKind;
use ast::syntax::stmt::StmtKind;

fn binding_ty(source: &str) -> Option<TypeKind> {
    let output = parse_source(source).expect("machine-level parse");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Binding(binding) = &output.ast.items[0].kind else {
        panic!("expected binding, got {:?}", output.ast.items[0].kind);
    };
    binding.ty.as_ref().map(|ty| ty.kind.clone())
}

fn fun_of(source: &str) -> ast::syntax::decl::FunDecl {
    let output = parse_source(source).expect("machine-level parse");
    assert!(
        output.diagnostics.is_empty(),
        "{source}: {:?}",
        output.diagnostics
    );
    let DeclKind::Fun(function) = &output.ast.items[0].kind else {
        panic!("expected function, got {:?}", output.ast.items[0].kind);
    };
    function.clone()
}

/// The first branch pattern of `fun f() { when x { PATTERN -> 1 } }`.
fn when_branch_pattern(source: &str) -> PatKind {
    let function = fun_of(source);
    let Some(FunctionBody::Block(block)) = &function.body else {
        panic!("expected block body");
    };
    // A trailing `when` is the block's tail expression, not a statement.
    let expression: &ast::syntax::expr::ExprWrap = match block.tail.as_deref() {
        Some(tail) => tail,
        None => {
            let Some(statement) = block.stmts.first() else {
                panic!("expected an expression");
            };
            let StmtKind::Expr(expression) = &statement.kind else {
                panic!("expected expression statement, got {:?}", statement.kind);
            };
            &expression.expr
        }
    };
    let ast::syntax::expr::ExprKind::When { branches, .. } = &expression.kind else {
        panic!("expected when, got {:?}", expression.kind);
    };
    let ast::syntax::expr::WhenPattern::Pattern(pattern) = &branches[0].pattern else {
        panic!("expected a pattern branch");
    };
    pattern.kind.clone()
}

#[test]
fn self_type_is_a_type_and_self_value_is_not() {
    // Positive: `Self` in a type slot and as a primary expression.
    assert!(matches!(
        binding_ty("val x: Self"),
        Some(TypeKind::SelfType)
    ));

    let function = fun_of("fun f() : Self = Self(1)");
    assert!(matches!(
        function.ret.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::SelfType)
    ));
    let Some(FunctionBody::Expr(expression)) = &function.body else {
        panic!("expected expression body");
    };
    // `Self` is a primary; the postfix loop supplies the call suffix.
    assert!(
        matches!(expression.kind, ast::syntax::expr::ExprKind::Call { .. }),
        "{:?}",
        expression.kind
    );

    // Negative: lowercase `self` is a value, never a type.
    let output = parse_source("fun f(a: self) : int = 0").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == DiagCode::UnexpectedToken),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn self_type_takes_no_type_arguments() {
    let output = parse_source("fun f(a: Self<T>) : int = 0").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.code == DiagCode::UnexpectedToken),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn unit_type_parses_where_ebnf_allows_it() {
    assert!(matches!(binding_ty("val x: ()"), Some(TypeKind::Unit)));
    for source in [
        "fun f() : () = ()",
        "fun f() : () { }",
        "fun f(a: ()) : int = 0",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
    // `()` still works as the empty parameter list of a function type.
    let output = parse_source("fun f() : () -> int = 0").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn type_alias_consumes_its_equals_sign() {
    for source in ["type Alias = Point", "type Alias<T> = Point"] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
        assert!(matches!(output.ast.items[0].kind, DeclKind::TypeAlias(_)));
    }
}

#[test]
fn is_and_not_is_are_symmetric() {
    for (source, negated) in [("val x = a is b", false), ("val x = a !is b", true)] {
        match binding_init(source) {
            ExprKind::Is { negated: found, .. } => assert_eq!(found, negated, "{source}"),
            other => panic!("{source}: expected Is, got {other:?}"),
        }
    }
    // `!in` keeps the shape it already had.
    match binding_init("val x = a !in b") {
        ExprKind::In { negated: true, .. } => {}
        other => panic!("expected negated In, got {other:?}"),
    }
}

#[test]
fn rest_patterns_record_their_position() {
    for (source, rest) in [
        ("fun f() { when x { [..] -> 1 } }", Some(0usize)),
        ("fun f() { when x { [1, ..] -> 1 } }", Some(1)),
        ("fun f() { when x { [1, 2, ..] -> 1 } }", Some(2)),
        ("fun f() { when x { [.., 3] -> 1 } }", Some(0)),
        ("fun f() { when x { [1, .., 3] -> 1 } }", Some(1)),
        ("fun f() { when x { [1, 2, .., 3, 4] -> 1 } }", Some(2)),
        ("fun f() { when x { [1, 2] -> 1 } }", None),
        ("fun f() { when x { (1, .., 3) -> 1 } }", Some(1)),
        ("fun f() { when x { (.., 3) -> 1 } }", Some(0)),
        ("fun f() { when x { (1, ..) -> 1 } }", Some(1)),
        ("fun f() { when x { (1, 2) -> 1 } }", None),
    ] {
        let found = match when_branch_pattern(source) {
            PatKind::Array { rest, .. } | PatKind::Tuple { rest, .. } => rest,
            other => panic!("{source}: expected array/tuple pattern, got {other:?}"),
        };
        assert_eq!(found, rest, "{source}");
    }
}

#[test]
fn rest_pattern_may_appear_only_once() {
    for source in [
        "fun f() { when x { [.., ..] -> 1 } }",
        "fun f() { when x { (.., ..) -> 1 } }",
        "fun f() { when x { [1, .., 2, .., 3] -> 1 } }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diag| diag.code == DiagCode::DuplicateRestPattern),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn when_branch_binding_annotation_is_arrow_less() {
    // The branch arrow must not be swallowed by `FunctionType := … ["->" T]`.
    for source in [
        "fun f() { when x { a: int -> 1 } }",
        "fun f() { when x { mut a: int -> 1 } }",
        "fun f() { when x { a: Map<K, V> -> 1 } }",
        "fun f() { when x { Some(a: int) -> 1 } }",
        "fun f() { when x { a: ((int) -> int) -> 1 } }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
    // Outside a `when` branch a binding annotation may still be a function type.
    let output = parse_source("fun f() { val g: (int) -> int = h }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn impl_declaration_binds_its_own_type_parameters() {
    let output = parse_source("impl<T> Show for List<T> { }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Impl(implementation) = &output.ast.items[0].kind else {
        panic!("expected impl");
    };
    assert_eq!(implementation.type_params.len(), 1);

    // Without `<T>` an impl has no parameters of its own.
    let output = parse_source("impl Show for List<T> { }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Impl(implementation) = &output.ast.items[0].kind else {
        panic!("expected impl");
    };
    assert!(implementation.type_params.is_empty());
}

#[test]
fn bounds_accept_an_intersection_in_both_slots() {
    let output = parse_source("class C<T : A & B & D> { }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Class(class) = &output.ast.items[0].kind else {
        panic!("expected class");
    };
    assert_eq!(class.type_params[0].bounds.len(), 3);

    let output = parse_source("fun f<T>(t: T) : int where T : A & B = 0").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Fun(function) = &output.ast.items[0].kind else {
        panic!("expected function");
    };
    assert_eq!(function.where_clause[0].bounds.len(), 2);
}

#[test]
fn inline_bounds_report_a_missing_intersection_operand() {
    for source in ["class C<T : A & > { }", "fun f<T : >() { }"] {
        let output = parse_source(source).unwrap();
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diag| diag.code == ast::syntax::node::DiagCode::ExpectedToken),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn extern_block_lists_multiple_functions() {
    let output = parse_source("extern \"c\" { fun a() : int fun b() : int; }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Extern(external) = &output.ast.items[0].kind else {
        panic!("expected extern");
    };
    let ExternBody::Block { members } = &external.body else {
        panic!("expected block body, got {:?}", external.body);
    };
    assert_eq!(members.len(), 2);

    for source in [
        "extern \"c\" fun a() : int;",
        "extern \"c\" fun a() : int",
        "extern \"c\" namespace std { fun malloc(n: int) : int; }",
        "extern \"c\" { }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn bodyless_trait_members_accept_a_redundant_semicolon() {
    for source in [
        "trait T { fun area() : f64 }",
        "trait T { fun area() : f64; }",
        "trait T { type Item }",
        "trait T { type Item; }",
        "trait T { type Out : Bound = int; }",
        "trait T { fun a() : int; fun b() : int }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
    // A top-level `fun` still needs a body.
    let output = parse_source("fun f() : int").unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diag| diag.message == "expected function body"),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn native_import_members_are_all_kept() {
    let output = parse_source("import \"c\" \"h\" { fun a() : int; fun b() : int; }").unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let DeclKind::Import(import) = &output.ast.items[0].kind else {
        panic!("expected import");
    };
    let ImportKind::Native { members, .. } = &import.kind else {
        panic!("expected native import");
    };
    assert_eq!(members.len(), 2, "members must not be dropped");

    for source in [
        "import \"c\" \"h\" { fun a() : int fun b() : int }",
        "import \"c\" \"h\" { fun a() : int, fun b() : int }",
        "import \"c\" \"h\" { }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn let_binding_is_an_error_and_stays_one_statement() {
    for source in [
        "fun f() { let x = 1 }",
        "fun f() { let x }",
        "fun f() { let mut x = 1 }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diag| diag.code == DiagCode::LetKeyword),
            "{source}: {:?}",
            output.diagnostics
        );
        let DeclKind::Fun(function) = &output.ast.items[0].kind else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(block)) = &function.body else {
            panic!("expected block body");
        };
        assert_eq!(
            block.stmts.len(),
            1,
            "{source} must recover as one binding, not split into two statements"
        );
        assert!(
            matches!(block.stmts[0].kind, StmtKind::Binding(_)),
            "{source}"
        );
    }
}

#[test]
fn let_stays_an_ordinary_identifier() {
    // The guard only fires on `let <identifier>`, never on `let` used as a name.
    for source in [
        "val let = 1",
        "fun f() { let }",
        "fun f() { let = 1 }",
        "fun f() { let(1) }",
        "fun f() { let.x }",
        "fun f() { val x = let }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diag| diag.code != DiagCode::LetKeyword),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn pointcut_accepts_the_wildcard_member() {
    for source in [
        "aspect A { after Type.* { } }",
        "aspect A { around demo.P.* { } }",
        "aspect A { before demo.P.log { } }",
        "aspect A { before demo.Point { } }",
    ] {
        let output = parse_source(source).unwrap();
        assert!(
            output.diagnostics.is_empty(),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}
