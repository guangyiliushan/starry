# starry-semantic

语义分析模块，实现基于属性文法的多遍分析流水线，覆盖名称解析、类型检查、常量折叠、中间代码生成（三地址码 / 四元式 / 三元式）以及基本块划分与 DAG 构建。

## 架构总览

模块按职责分为五个层次：

```
analyzer/     ← 各遍次的具体求值器（硬编码版与规则驱动版）
attribute/    ← 属性文法求值框架（核心遍历引擎）
ir/           ← 中间表示（TAC / 四元式 / 三元式 / 基本块 / DAG）
rules/        ← 声明式语义规则 DSL（解析、调度、执行）
顶层模块       ← 流水线编排、符号表、类型系统、依赖分析、错误模型
```

## 属性文法求值框架 —— attribute/

实现龙书 §5 的语法制导翻译（SDT）与属性文法模型，是整个语义分析的**核心遍历引擎**。

### 核心概念

- **继承属性（L 属性）**：自顶向下、自左向右传递的上下文信息。对应 `AttrContext`，包含当前作用域、期望返回类型、是否位于循环内部、当前函数作用域及所在遍次。每个节点进入时通过不可变拷贝创建子节点的上下文视图。
- **综合属性（S 属性）**：自底向上汇总的计算结果。对应 `AttrResult`，包含静态类型、常量标志、常量折叠后的值、左值标志、IR 指令序列及结果存放位置。
- **深度优先求值器**：`evaluate()` 函数，按"先入后出"策略遍历 AST——先调用继承属性计算逻辑进入节点，递归子节点收集综合属性，最后调用综合属性计算逻辑离开节点并向上传递结果。

### 文件职责

| 文件 | 职责 |
|---|---|
| `context.rs` | 定义继承属性上下文 AttrContext、遍次标识 Pass（Warmup / NameResolution / TypeChecking / IrGeneration）、预热统计 WarmupStats |
| `env.rs` | 作用域化符号表 Env，管理作用域链（Scope / ScopeId），支持 push/pop/resolve/define/define_placeholder 操作 |
| `eval.rs` | 属性文法核心编排——AttrEvaluator trait（enter / leave）、PassEvaluator trait（run）、DFS 遍历函数 evaluate()、IrModule 定义 |
| `result.rs` | 综合属性载体 AttrResult（类型、常量性、折叠值、左值性、IR 指令、存放位置） |
| `store.rs` | 非侵入式侧表——AstWalker 按 DFS 序分配 NodeId，AttrStore 跨遍次存储 PassAttrs，PassAttrs 按 NodeId 索引 NodeAttrs |

### 执行流程

1. AstWalker 对 AST 做 DFS 遍历，为每个节点分配连续整数 NodeId
2. 各遍次通过 AttrEvaluator.enter 计算继承属性（进入节点时）
3. 递归遍历子节点，收集子节点的综合属性 AttrResult
4. 通过 AttrEvaluator.leave 计算当前节点的综合属性（离开节点时）
5. 综合属性通过 PassAttrs 存入侧表，按 NodeId 索引

## 中间表示 —— ir/

实现龙书 §8 的中间代码生成体系，将 AST 逐步降级为多种中间表示形式。

### 表示层级

| 表示形式 | 结构 | 用途 |
|---|---|---|
| 三地址码 TAC | `result = left op right` 形式的线形指令序列 | 最接近 AST 的中间表示，便于遍历与理解 |
| 四元式 | `(op, arg1, arg2, result)` 统一字段结构 | 标准化操作符为 TacOp 枚举，便于优化遍次统一处理 |
| 三元式 | `(op, arg1, arg2)`，结果通过指令位置索引引用 | 节省 result 字段，指令移动时索引需重定位 |
| 间接三元式 | 三元式基础上增加间接映射表 | 可对指令重排序而不破坏引用关系 |

### 文件职责

| 文件 | 职责 |
|---|---|
| `tac.rs` | 三地址码指令定义——二元运算、一元运算、赋值、条件/无条件跳转、标签、函数调用、返回 |
| `quadruple.rs` | 四元式定义 Quadruple，统一操作符 TacOp 枚举，TacInstr → Quadruple 转换 |
| `triple.rs` | 三元式定义 Triple，操作数引用 TripleRef（直接操作数 / 指令索引），四元式 → 三元式转换函数 |
| `basic_block.rs` | 基本块划分——按龙书算法识别首指令（入口 / 跳转目标 / 跳转后下一条），将 TAC 序列切分为单入口单出口的基本块列表 |
| `dag.rs` | 有向无环图构建——从基本块指令序列中识别公共子表达式，合并等价的运算节点，附加变量名到节点上 |
| `operand.rs` | 操作数类型——临时变量 TempId、命名变量、整型/浮点/布尔/字符串常量、标签引用 LabelId |
| `temp.rs` | 临时变量管理器 TempManager（new_temp / reset） |
| `label.rs` | 标签管理器 LabelManager（new_label / reset） |

### 转换链

AST → 递归遍历产生 TacInstr → 转换为 Quadruple → 转换为 Triple（间接三元式）

同时 TacInstr 序列经基本块划分器切分为 BasicBlock 列表；每个基本块可进一步构建 DAG 用于公共子表达式消除。

## 声明式语义规则 DSL —— rules/

提供声明式规则定义语言，允许用户以简洁的 DSL 语法描述语义动作，替代硬编码的求值器逻辑。

### 核心概念

- **语义规则**：一组"模式 + 前序动作 + 后序动作"的三元组。模式匹配 AST 节点，前序动作在进入节点时执行（继承属性计算），后序动作在离开节点时执行（综合属性计算）。
- **动作**：原子语义操作，包括设置综合属性、条件检查（语义错误检测）、常量折叠、作用域 push/pop、符号注册/查找、IR 指令发射等。
- **值表达式**：描述属性值来源的表达式树——可引用子节点属性（`@0.ty`、`@1.is_const`）、当前节点属性、内置函数调用（promote、is_numeric、is_compatible）、布尔运算等。
- **DSL 语法**：`@section` 标记规则所属遍次（`@type_check` / `@ir_gen` / `@name_resolution`）；`name pattern` 定义规则头；缩进行定义前序/后序动作。

### 文件职责

| 文件 | 职责 |
|---|---|
| `action.rs` | 动作类型定义——SetSynthesized、Check（条件检查 + 错误模板）、FoldIfConst（常量折叠）、Emit（IR 指令发射）、PushScope/PopScope、DefinePlaceholder、LookupSymbol、PassThrough 等；ValueExpr 值表达式体系（子属性引用、内置函数调用、布尔/类型常量）；ErrorTemplate 错误模板；InstrTemplate 指令模板 |
| `rule.rs` | 语义规则定义——SemanticRule（名称 + 模式 + 前序/后序动作）；NodePattern（精确匹配 / 按操作符匹配 / 通配匹配）；AstNodeKind 枚举 |
| `table.rs` | 规则表 RuleTable——按优先级存储规则，提供按 AST 节点匹配查找的方法 |
| `rule_parser.rs` | DSL 解析器——将声明式规则文本解析为 ParsedRule 列表；build_rule_tables 按 section 分组为三个 RuleTable（名称解析 / 类型检查 / IR 生成） |
| `rule_display.rs` | 模式 → 可读字符串的转换 |
| `macros.rs` | build_rules! 声明式宏——在编译期构造 RuleTable |
| `context.rs` | 规则驱动求值器的运行时上下文 ActionContext——聚合当前节点、子结果、环境、错误收集器、临时变量/标签管理器 |

### 调度逻辑

1. Parser 将 DSL 文本解析为 ParsedRule 列表
2. build_rule_tables 按遍次分组为三个 RuleTable
3. RuleDrivenEvaluator 在遍历 AST 时，对每个节点查表匹配规则
4. 匹配到规则后，按序执行 pre_actions（继承属性 → 递归子节点 → post_actions（综合属性 / 检查 / 发射 IR）

## 分析器实现 —— analyzer/

提供两类求值器实现：硬编码版（手工编写语义逻辑）和规则驱动版（由 DSL 规则表驱动）。

### 文件职责

| 文件 | 职责 |
|---|---|
| `warmup.rs` | WarmupEvaluator——纯计数 DFS，统计节点总数、最大作用域深度、估计符号数量，产出 WarmupStats 供后续遍历预分配内存 |
| `name_resolution.rs` | NameResolutionEvaluator——硬编码版名称解析：遇到 Block 推入新作用域，遇到 Ident 注册占位符符号（仅注册名字，类型标记为 Unresolved） |
| `name_resolution_rules.rs` | 名称解析规则表——定义 `nr_ident` 规则（AnyIdent → DefinePlaceholder）和 `nr_block` 规则（AnyBlock → PushScope / PopScope） |
| `type_check.rs` | TypeCheckEvaluator——硬编码版类型检查：处理字面量/标识符/二元/一元/括号表达式的类型推导、语义规则校验和常量折叠 |
| `type_check_rules.rs` | 类型检查规则表——声明式定义 arithmetic（算术运算类型提升 + 检查）、compare（比较运算 → Bool）、logical（逻辑运算 → Bool）、unary（取负/取反/正号）、literal、ident、block、expr_stmt 等规则 |
| `ir_gen.rs` | IrGenerator——S 属性求值器，自底向上为每个表达式节点分配临时变量并生成 TAC 指令，返回 AttrResult 携带指令序列和存放位置 |
| `ir_gen_rules.rs` | IR 生成规则表——声明式定义 binary（Emit BinaryAssign）、unary（Emit UnaryAssign）、literal（Emit LoadConst）、ident（Emit CopyName）、block（PushScope / PopScope / SequenceLast）等规则 |
| `rule_driven.rs` | RuleDrivenEvaluator——通用规则驱动求值器，加载 RuleTable 后在遍历中匹配规则并执行动作；涵盖值表达式求值、布尔表达式求值、动作分发执行、错误生成等核心逻辑 |
| `legacy.rs` | Analyzer——单遍语义分析器（向后兼容），适用于无前向引用、无类型推断的简单语言 |

### 两种求值模式的关系

硬编码版和规则驱动版在语义上等价：
- 硬编码版直接调用 Rust 代码实现 enter/leave 逻辑
- 规则驱动版将语义逻辑外化为 DSL 规则文本，在运行时解释执行
- Pipeline 优先使用规则驱动版（若用户提供了规则表），否则回退到硬编码版

## 流水线 —— pipeline.rs

编排四遍语义分析流程，是模块的统一入口。

### 四遍流程

| 遍次 | 名称 | 属性方向 | 职责 |
|---|---|---|---|
| Pass 0 | 预热遍 | DFS 计数 | 统计节点数、最大作用域深度、估计符号数，为后续预分配内存 |
| Pass 1 | 名称解析遍 | L 属性（自顶向下） | 沿作用域链注册所有标识符为占位符，构建符号表骨架，支持前向引用 |
| Pass 2 | 依赖分析 | 图算法 | 基于符号表收集依赖边，构建依赖图并执行拓扑排序（Kahn 算法），检测循环依赖 |
| Pass 3 | 类型检查遍 | 混合策略 | 解析所有占位符类型，执行类型推导与检查，进行常量折叠，报告语义错误 |
| Pass 4 | IR 生成遍 | S 属性（自底向上） | 为每个表达式分配临时变量，生成 TAC 指令序列，包装为 IrModule |

### 依赖分析

在名称解析与类型检查之间插入依赖分析阶段：从 Env 收集所有符号的依赖边，构建 DependencyGraph，使用 Kahn 算法做拓扑排序。若排序失败（存在环），报告循环依赖错误。

## 类型系统 —— ty.rs

定义 Ty 枚举作为语义分析阶段的统一类型表示：Int / Float / Bool / String / Void / Error（错误降级）/ Unresolved（占位符，名称解析阶段）/ Var（类型变量，推断阶段）。

关键操作：
- 类型提升 promote：二元算术运算时统一左右子类型的公共类型（Int + Float → Float）
- 兼容性检查 is_compatible：赋值或比较时检查左右类型是否兼容
- 解析性检查 is_resolved：判断类型是否已完全确定

## 符号表 —— symbol.rs

定义符号条目 Symbol（名称、类型、常量/可变标志、状态、种类、依赖列表、栈偏移量）及符号状态机。

### 符号状态转换

Placeholder（名称解析遍注册） → Resolved（类型检查遍解析）

占位符机制支持前向引用：名称解析遍仅注册 Identifier 名字而不确定类型，类型检查遍再沿作用域链解析为具体类型。若类型检查结束后仍有占位符，报告 UnresolvedType 错误。

## 错误模型 —— error.rs

定义 SemanticError 枚举，覆盖：未定义名称、重复声明、类型不匹配、非法二元/一元运算、非法赋值、给常量赋值、循环外 break/continue、返回类型不匹配、循环依赖、未解析类型、隐式转换失败等。

每个错误携带 Span 表示源码位置。

## 依赖分析 —— dependency.rs

DependencyGraph 实现：从符号依赖对构建有向图，使用 Kahn 算法做拓扑排序，DFS 检测环路并返回环路径。

## 完整执行路径

以 demo_attribute.rs 为例的端到端流程：

1. **输入**：算术表达式字符串，经 starry-ast 解析为 AstNode
2. **规则加载**：DSL 规则文本 → parse_rules → build_rule_tables → 得到三个 RuleTable
3. **创建流水线**：Pipeline::new → with_name_resolution_rules / with_type_check_rules / with_ir_gen_rules
4. **Pass 0 预热**：WarmupEvaluator DFS 遍历 → AstWalker 分配 NodeId → 统计 node_count / max_scope_depth / estimated_symbol_count → 存入 AttrStore.warmup
5. **Pass 1 名称解析**：NameResolutionEvaluator（或 RuleDrivenEvaluator + name_resolution_rules）自顶向下注册占位符 → 符号表填充 → 存入 AttrStore.name_resolution
6. **依赖分析**：从符号表收集依赖关系 → DependencyGraph 拓扑排序 → 检测循环依赖
7. **Pass 2 类型检查**：TypeCheckEvaluator（或 RuleDrivenEvaluator + type_check_rules）解析占位符类型、执行类型检查与常量折叠 → 语义错误收集 → 存入 AttrStore.type_check
8. **Pass 3 IR 生成**：IrGenerator（或 RuleDrivenEvaluator + ir_gen_rules）自底向上为每个表达式分配临时变量并发射 TAC 指令 → 构造 IrModule
9. **输出展示**：三地址码序列 → 转四元式 → 转三元式（间接三元式，临时变量引用转为指令索引）→ 基本块切分 → DAG 构建 → 合并属性表打印

## 模块依赖关系

```
pipeline ──────────────── 编排层
  │
  ├── analyzer/ ────────── 求值器层
  │     ├── 依赖 attribute/ 框架（AttrEvaluator / evaluate）
  │     └── 依赖 rules/ 规则表（可选，规则驱动模式）
  │
  ├── attribute/ ───────── 属性文法引擎
  │     ├── 使用 ir/ 操作数类型
  │     └── 使用 ty 类型系统
  │
  ├── ir/ ──────────────── 中间表示
  │     └── 独立模块，被 analyzer 和 pipeline 使用
  │
  ├── rules/ ───────────── 规则 DSL
  │     ├── 引用 ty 类型常量
  │     └── 引用 ir 指令模板
  │
  ├── symbol / ty / error / dependency ── 基础模块
  │     └── 被其余各层共同依赖
```

## 与编译理论概念的对应

| 理论概念 | 模块映射 |
|---|---|
| 属性文法 / SDT | attribute/（AttrEvaluator / evaluate / AttrContext / AttrResult） |
| 继承属性 | AttrContext（前序传递的上下文） |
| 综合属性 | AttrResult（后序汇总的计算结果） |
| 翻译模式 | 规则中的 pre_actions + post_actions 序列 |
| 语义规则 / 语义动作 | rules/action.rs（Action 枚举） |
| 符号表 | attribute/env.rs（Env / Scope） |
| 类型系统 | ty.rs（Ty 枚举） |
| 三地址码 TAC | ir/tac.rs（TacInstr） |
| 四元式 | ir/quadruple.rs（Quadruple） |
| 三元式 / 间接三元式 | ir/triple.rs（Triple / TripleRef） |
| 基本块 | ir/basic_block.rs（BasicBlock） |
| DAG / 公共子表达式消除 | ir/dag.rs（Dag） |
| 依赖图 / 拓扑排序 | dependency.rs（DependencyGraph） |
| 多遍分析 | pipeline.rs（Pipeline 四遍流程） |
| 声明式规则 | rules/rule_parser.rs（DSL 解析） |
| 常量折叠 | 类型检查遍 + FoldIfConst 动作 |
| 占位符 / 前向引用 | SymbolStatus::Placeholder → Resolved |
