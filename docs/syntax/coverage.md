# Starry v0.3 EBNF ↔ AST 覆盖率交叉表

## Legend

| 状态 | 含义 |
|------|------|
| 本计划阶段 1 扩张 | lexer 规则表在本计划阶段 1 新增 |
| 驱动层合成 | Nl 等由驱动层合成，不经 DFA |
| 设计缺口·归 lexer 计划 | 已知能力缺口，归属 lexer 后续计划 |
| 待决策 | 文法决策未定（如注释嵌套） |

## 声称 ↔ 正文落点

每条摘要声称的修正必须在正文找到落点（可 grep 核验）。

## 产生式 ↔ AST 变体（11 项完备性核对）

| # | EBNF 产生式 | AST 变体 | walk 访问 | 终端审计 | 状态 |
|---|------------|---------|----------|---------|------|
| 1 | LambdaExpr (params+arrow optional) | ExprKind::Lambda | params, body | «move» 无终端 | ✓ |
| 2 | OperatorExpr 12 级分层 | ExprKind::Binary | lhs, rhs | 十 AssignOp + 逻辑 + 比较 + 范围 | ✓ |
| 3 | CastExpr (as/as?/as!) | ExprKind::Cast | expr, ty | as/as?/as! ∈ 规则表 | ✓ |
| 4 | UnaryExpr (含 await) | ExprKind::Unary | operand | !/-/~/&/*/await | ✓ |
| 5 | PostfixExpr (. / [] / call / ?. / !!) | ExprKind::Call/Index/Member | base, args/index | . / [ / ( / ?. / !! | ✓ |
| 6 | Primary (unit/tuple/array/null/self/super/Path) | ExprKind::Literal/Ident/Path | — | null/self/super ∈ hard | ✓ |
| 7 | WhenExpr (两种形态) | ExprKind::When（待 parser） | subject, branches | "when" ∈ hard | ✓ |
| 8 | WhenBranch → (Expr \| Block) [HIGH-RISK] | WhenBranch | pattern, body | -> ∈ 规则表 | ✓ |
| 9 | GenericArgs 关联绑定 [HIGH-RISK] | TypeArg = Type or IDENT=Type | args | < > ∈ 标点 | ✓ |
| 10 | SuperItem 构造实参 [HIGH-RISK] | SuperSuffix | args | super ∈ hard | ✓ |
| 11 | is/in 模式 [HIGH-RISK] | PatKind::Is/In | ty, expr | is ∈ hard; !in = !+in | ✓ |

## 附加声明

| 声明 | 正文落点 |
|------|---------|
| 插值字符串模式栈（设计缺口） | 审计行"设计缺口·归 lexer 计划" |
| 注释跳过层（待决策） | 审计行"待决策" |
| comptime 槽位收窄（expression_or_block → stmt-only） | coverage 行 comptime |
| NL 前言约定（不逐产生式塞 NL） | EBNF 前言声明 |
| mutable 模式 | pat.rs BindingPattern mutable 字段 |
| 参数默认值（Decl→Expr walk 边） | decl.rs Param 默认值字段 |
