use starry_ast::AstNode;
use starry_semantic::{
    AttrContext,
    Dag,
    DependencyGraph,
    Env,
    Pipeline,
    build_rule_tables,
    cse_and_to_triples,
    cse_quadruples,
    parse_rules,
    // analyzer::{build_ir_gen_rules, build_type_check_rules},
    pattern_to_string,
    quadruples_to_triples,
};

fn main() {
    let input = "( ( 1 + 2 ) * ( ( 1 + 2 ) + 3))";
    println!("输入表达式: {}\n", input);

    let ast: AstNode = input.parse().expect("解析失败");

    println!("【原始 AST 结构】");

    ast.print();
    println!();

    // 显示规则
    let rules_text = r#"
@type_check
literal AnyLiteral
  ty: Int
  is_const: true
  const_value: NodeValue
  is_lvalue: false

binary_arith BinaryOp(Add,Sub,Mul,Div,Mod)
  ty: promote(@0, @1)
  is_lvalue: false
  is_const: @0.is_const && @1.is_const

@ir_gen
ir_literal AnyLiteral
  Emit(LoadConst)

ir_binary BinaryOp(Add,Sub,Mul,Div,Mod,Eq,Ne,Lt,Le,Gt,Ge,And,Or)
  Emit(BinaryAssign)
"#;

    // 解析字符串规则
    let parsed_rules = parse_rules(rules_text);
    let (nr_rules, tc_rules, ir_rules) = build_rule_tables(&parsed_rules);

    // let tc_rules = build_type_check_rules();
    // let ir_rules = build_ir_gen_rules();

    println!("【类型检查规则】共 {} 条", tc_rules.rules().len());

    for rule in tc_rules.rules() {
        println!("  • {} : {}", rule.name, pattern_to_string(&rule.pattern));
    }
    println!();

    println!("【IR 生成规则】共 {} 条", ir_rules.rules().len());

    for rule in ir_rules.rules() {
        println!("  • {} : {}", rule.name, pattern_to_string(&rule.pattern));
    }
    println!();

    // 创建 Pipeline 并执行四遍流程
    let mut pipeline = Pipeline::new()
        .with_name_resolution_rules(nr_rules)
        .with_type_check_rules(tc_rules)
        .with_ir_gen_rules(ir_rules);

    // ==================== Pass 0: 预热遍 ====================

    println!("【Pass 0: 预热遍】");

    let warmup_stats = pipeline.run_warmup(&ast);
    println!("节点总数: {}", warmup_stats.node_count);
    println!("最大作用域深度: {}", warmup_stats.max_scope_depth);
    println!("估计符号数量: {}", warmup_stats.estimated_symbol_count);
    println!();

    println!("【预热遍后的 AST 属性】");
    pipeline.attr_store().warmup.print_ast(&ast);
    println!();

    println!("【Pass 1: 名称解析遍】");

    let mut env = Env::with_capacity(warmup_stats.clone());
    let ctx = AttrContext::root(env.current_scope()).with_warmup_stats(warmup_stats.clone());
    pipeline.run_name_resolution(&ast, &mut env, &ctx);

    println!("符号表条目:");
    for (scope_id, sym) in env.all_symbols() {
        println!("  作用域 {:?} : {} ({:?})", scope_id, sym.name, sym.status);
    }
    println!();

    let deps = env.collect_dependencies();
    if deps.is_empty() {
        println!("无依赖关系。");
    } else {
        let graph = DependencyGraph::from_dependencies(&deps);
        println!("依赖边:");
        for (from, to_list) in &deps {
            for to in to_list {
                println!("  {} → {}", from, to);
            }
        }
        match graph.topological_sort() {
            Ok(order) => println!("拓扑排序结果: {:?}", order),
            Err(cycle) => println!("检测到循环依赖: {:?}", cycle),
        }
    }
    println!();

    println!("【名称解析遍后的 AST 属性】");
    pipeline.attr_store().name_resolution.print_ast(&ast);
    println!();

    println!("【Pass 2: 类型检查遍】");

    pipeline.run_type_check(&ast, &mut env, &ctx);
    println!("类型检查完成。错误数: {}", pipeline.errors().len());
    for err in pipeline.errors() {
        println!("  错误: {}", err);
    }
    println!();

    println!("【类型检查遍后的 AST 属性】");
    pipeline.attr_store().type_check.print_ast(&ast);
    println!();

    println!("【Pass 3: IR 生成遍】");

    let ir_module = pipeline.run_ir_generation(&ast, &mut env, &ctx);

    println!("【IR 生成遍后的 AST 属性】");
    pipeline.attr_store().ir_generation.print_ast(&ast);
    println!();

    println!("【三地址码 (TAC)】");
    println!("  （常量直接内联到运算指令，无单独的 LoadConst）");

    for (i, instr) in ir_module.instructions.iter().enumerate() {
        println!("  {:3}: {}", i, instr);
    }
    println!();

    println!("【三元式 (Triples) — 未优化】");
    let triples = quadruples_to_triples(&ir_module.quadruples);
    for (i, triple) in triples.iter().enumerate() {
        println!("  {:3}: {}", i, triple);
    }
    println!();

    println!("【三元式 (Triples) — CSE 优化后（公共子表达式消除）】");
    let cse_triples = cse_and_to_triples(&ir_module.quadruples);
    for (i, triple) in cse_triples.iter().enumerate() {
        println!("  {:3}: {}", i, triple);
    }
    println!();

    println!("【四元式 (Quadruples) — 未优化】");
    for (i, quad) in ir_module.quadruples.iter().enumerate() {
        println!("  {:3}: {}", i, quad);
    }
    println!();

    println!("【四元式 (Quadruples) — CSE 优化后】");
    let cse_quads = cse_quadruples(&ir_module.quadruples);
    for (i, quad) in cse_quads.iter().enumerate() {
        println!("  {:3}: {}", i, quad);
    }
    println!();

    println!("【基本块与 DAG】");

    if let Some(block) = ir_module.basic_blocks.first() {
        let dag = Dag::build_from_block(block);
        println!("基本块指令序列:");
        for instr in &block.instructions {
            println!("  {}", instr);
        }
        println!();

        println!("DAG 节点:");
        for node in dag.nodes() {
            let ids = if node.identifiers.is_empty() {
                "—".to_string()
            } else {
                node.identifiers.join(", ")
            };
            println!(
                "  节点 {}: op={:?}, children={:?}, 附加标识符=[{}]",
                node.id, node.op, node.children, ids
            );
        }
    } else {
        println!("无基本块");
    }
    println!();

    println!("基本块划分:");
    for (i, block) in ir_module.basic_blocks.iter().enumerate() {
        println!("  Block {}:", i);
        for instr in &block.instructions {
            println!("    {}", instr);
        }
    }

    println!();

    println!("【合并后的完整属性表】");

    let merged = pipeline.attr_store().merged();
    merged.print_ast(&ast);
}
