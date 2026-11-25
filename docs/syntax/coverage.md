# Starry v0.4 EBNF ↔ AST 覆盖率交叉表

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
| — | PointerType (`*const T` / `*mut T`) | TypeKind::Pointer | mutable, inner | D-007 / M2：bare `*T` 诊断；类型+FFI；取址/解引未排期 | ✓ |
| — | RefType (`&T` / `&mut T`) | TypeKind::Ref | mutable, inner | D-007 / M2：T1 借用类型层；表达式未排期 | ✓ |
| — | NullableType（前缀 `?T` + 原子后缀糖） | TypeKind::Nullable | inner | 复合后缀拒绝（?[T; N] 提示）；两种写法同一 AST | ✓ |
| — | AddrOfExpr（&x / &mut x / *p） | ExprKind::AddrOf / Deref | expr（+mutable） | M2：借用检查 v1 保守；`is_place` 收 Deref | ✓ |
| 1a | LoopExpr (`loop Block`) | ExprKind::Loop | label, body | `loop` ∈ hard；label 仅由块尾 LoopStatement 归一而来（parser-notes） | ✓ |
| 2 | OperatorExpr 12 级分层 | ExprKind::Binary | lhs, rhs | 十 AssignOp + 逻辑 + 比较 + 范围 | ✓ |
| 3 | CastExpr (as/as?/as!) | ExprKind::Cast | expr, ty | as/as?/as! ∈ 规则表 | ✓ |
| 4 | UnaryExpr (含 await) | ExprKind::Unary | operand | !/-/~/&/*/await | ✓ |
| 5 | PostfixExpr (. / [] / call / ?. / !!) | ExprKind::Call/Index/Member | base, args/index | . / [ / ( / ?. / !! | ✓ |
| 6 | Primary (unit/tuple/array/null/self/super/Path) | ExprKind::Literal/Ident/Path | — | null/self/super ∈ hard | ✓ |
| 6a | `true` / `false` / `null` | ExprKind::Bool(bool) / ExprKind::Null | operand | hard keywords are not LiteralKind duplicates | ✓ |
| 7 | WhenExpr (两种形态) | ExprKind::When（待 parser） | subject, branches | "when" ∈ hard | ✓ |
| 8 | WhenBranch → (Expr \| Block) [HIGH-RISK] | WhenBranch | pattern, body | -> ∈ 规则表 | ✓ |
| 9 | GenericArgs 关联绑定 [HIGH-RISK] | TypeArg = Type or IDENT=Type | args | < > ∈ 标点 | ✓ |
| 10 | SuperItem 构造实参 [HIGH-RISK] | SuperSuffix | args | super ∈ hard | ✓ |
| 11 | Pattern alternatives | PatKind::Range/Tuple/Array/Constructor/Struct | elements | `..`、`[`、`]`、`{`、`}` | ✓ |

## 附加声明

| 声明 | 正文落点 |
|------|---------|
| 插值字符串模式栈（设计缺口） | 审计行"设计缺口·归 lexer 计划" |
| 注释跳过层（已实现：块注释嵌套、行注释） | 审计行"待决策" |
| NL 前言约定（不逐产生式塞 NL） | EBNF 前言声明 |
| mutable 模式 | pat.rs BindingPattern mutable 字段 |
| 参数修饰符单态 | ParamModifier = None \| Mutable \| Val \| Var |
| 参数默认值（Decl→Expr walk 边） | decl.rs Param 默认值字段 |
| 条件绑定无 `let` | RefutableCondition = Expr \| "val" Pattern "=" Expr |
| declaration 全量 | DeclKind / ClassMember / TraitMember（ProviderItem 等为 AST 遗留变体，parser 不可达，待清理） |
| `ParameterList` 显式化 | Parameter / ParameterList 前言定义 |
| v0.4 插值字符串（parser 子扫描） | ExprKind::StringInterp + parse_string_literal |
| v0.4 具名实参 | CallArg { name, value } |
| v0.4 `?.`/`!!` 后缀 | Member { safe } / ExprKind::ForceUnwrap；无 `?` 传播 |
| v0.4 unsafe 块 | ExprKind::Unsafe（concurrent/proceed 已随 D-014 删除；AST 遗留变体待清理） |
| v0.4 循环标签 `'ident` | TokenKind::Label；break/continue/loop 标签槽位 |
| 赋值目标语法检查 | AssignmentStatement 只接受 Place（PostfixExpr）；InvalidAssignmentTarget |
| TypeParameter 恢复 | TypeParamName::Valid/Error，非法名不再伪装为 Symbol |
| v0.4 宏调用 `name!(...)` | ExprKind::MacroCall + TokenTree 流；展开后置（调用点 `SemMacroExpansion`） |
| v0.4 数组 `[T; N]` / `[v; N]` | TypeKind::Array / ExprKind::ArrayRepeat |
| v0.4 module 声明移除 | SourceFile 无 package 头；模块路径为编译器 segment 元数据；`by` 降为 slot word |
| v0.4 LoopExpr / LoopStatement 分立 | ExprKind::Loop（表达式，label 字段承块尾归一）/ StmtKind::Loop（带标签语句，块尾时归一为 LoopExpr） |
| 元组下标常量槽 | ExprKind::Index.const_index（parser 解码整数字面量；动态下标仅数组） |

## v0.4 gap closures（本轮补全）

| # | EBNF 产生式 | AST 变体 | walk 访问 | 终端审计 | 状态 |
|---|------------|---------|----------|---------|------|
| 12 | UnitType `()` | `TypeKind::Unit` | — | `(` `)` | ✓ 变体原先从未构造 |
| 13 | `Self` 类型 | `TypeKind::SelfType` | — | `Self` ∈ hard | ✓ 变体原先从未构造 |
| 14 | `Self` 表达式 [ CallSuffix ] | `ExprKind::Ident("Self")` + Call | args | `Self` ∈ hard | ✓ postfix 循环接住 call suffix |
| 15 | `!is` / `!in` 对称 | `ExprKind::Is { negated, pattern }` / `In { negated, rhs }` | lhs+pattern（is，rhs=IsOperand 模式）/ lhs+rhs（in，rhs=RangeExpr 集合） | `is` `!is` `in` `!in` | ✓ IsOperand 无结构体尾 |
| 16 | TuplePatternElement / ArrayPatternElement 含 `..` | `PatKind::Tuple { elements, rest }` / `Array { elements, rest }` | elements | `..` ≤ 1 次，任意位置 | ✓ `DuplicateRestPattern` |
| 17 | WhenPattern 顶层标注 = NoArrowType | `PatKind::Binding.ty` | ty | `->` 分支箭头不吞 | ✓ 嵌套标注仍为 Type |
| 18 | ImplDecl TypeParams? | `ImplDecl.type_params` | type_params, type_ref, trait_ref | `<` `>` | ✓ |
| 19 | Bound := Type { "&" Type } | `TypeParam.bounds` / `WhereConstraint.bounds` | bounds | `&` | ✓ 内联与 where 同构 |
| 20 | ExternBody 块形式 | `ExternBody::Block { members }` | members | `{` `}` | ✓ |
| 21 | FunSignature / AssociatedType 可选 `;` | 同上 | — | `;` 冗余终止符，宽松接收 | ✓ |
| 22 | TypeAliasDecl `"=" Type`（D-014：关键字改为 `alias`） | `TypeAliasDecl.ty` | ty | `=` | ✓ parser 原先漏吃 `=` |
| 23 | `let` 非关键字 | `StmtKind::Binding`（恢复形态） | pattern, ty, init | `let` + IDENT → `LetKeyword` | ✓ 不再静默拆两句 |

### 终端审计补充

| 终端 | 归属 | 状态 |
|---|---|---|
| `Self` | hard keyword；类型位与表达式位均可用，不接 type args | ✓ |
| `self` | hard keyword；仅表达式位，类型位拒绝 | ✓ |
| `..` | 模式 rest；至多一次，任意位置；表达式位仍是 range | ✓ |
| `!is` / `!in` | relational 层，与 `is` / `in` 同构 | ✓ |
| `let` | 非关键字；`LetKeyword` 诊断 | ✓ |
| `;` | 仅作 `FunSignature`/`AssociatedType` 的冗余终止符；语句位仍拒绝 | ✓ |

## D-014 关键字收敛（2026-11-25）

- 已从词表删除并在 **parse 级**拒收：actor/aspect/provide/macro/comptime/
  spawn/detached/concurrent/thread/fiber/process/grow/bits/before/after/
  around/proceed/singleton/scoped/transient/bind/to/out/async/await/
  const/inline/infix/vararg/package/namespace。
- 新增：`alias`（类型别名 hard keyword）、`deinit`（slot word，
  `DeinitBlock := «deinit» Block`）。
- AST 仍保留上述特性的遗留变体（parser 不可达），清理归 T12。
