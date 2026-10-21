# Semantic notes（sem crate 实现契约）

语义分析的实现门禁与决策记录，与 `parser-notes.md` 同位。三遍 pass +
最小线性 move 检查，纯 std、全确定性（`BTreeMap`/`BTreeSet`，不依赖
迭代序之外的任何顺序保证）。

## Pass 顺序与理由

```
ParseOutput ─→ declare（作用域树 + defs + 顶层签名 lower）
            ─→ resolve（名字 → DefId；label/flow；pattern 构造器）
            ─→ check（自底向上定型 → expr_ty；Error 投毒抑制级联）
            ─→ moveck（Own 值线性移动检查，join 点保守并集）
            ─→ SemanticOutput（AST 原样 + arena + 侧表）
```

顺序被依赖锁死：前向引用要求名字全量注册先于一切解析（Starry 允许
文件内先用后声明）；签名 lower 在名字注册完成后统一做（别名循环守卫
`Def.resolving`）；resolve 只绑定不定型；check 定型时读 resolve 的侧表；
moveck 最后跑（需要完整 expr_def）。

## 侧表契约（核心设计）

语义数据零内嵌 AST。名字 → 类型/定义的全部结果走 `SideTable`：
`NodeId` 索引的稠密 `Vec<Option<T>>`，容量 = `node_count`，O(1)，
越界返回 `None`（受控，不 panic）。公开侧表：`expr_ty`、`expr_def`、
`pat_res`、`flow_res`。IR 生成阶段以 `SemanticOutput` 为输入契约。

- `TyArena`：结构去重（相等类型只存一份）；`TyId` Copy，跨 pass 稳定。
- `DefArena` / `ScopeArena`：`DefId` / `ScopeId` 索引；作用域符号表
  `BTreeMap<Symbol, DefId>`（确定性门 + O(log n) 编译期规模无感）。

## 投毒与级联抑制

`TyKind::Error` 为投毒类型：出错子树统一 Error；`assignable` 对
Error 恒真，出错子树不再级联诊断。parser 诊断原样保留在前，语义诊断
附加在后。`Diagnostic` 以稳定 `DiagCode` 为准，message 仅供人读。

## 类型系统 v1

- 字面量默认：`Integer → i32`、`Float → f64`。
- 内建标量：`i4..i128`、`int`(=i64)、`f32/f64`、`bool`、`string`、
  `char`、`unit` lower 为原子 `TyKind`；`Own`/`Shared` lower 为
  `Named(builtin_def, args)`。
- 提升格（`assignable`/`join`）：整型按位宽 `i4→…→i128` 加宽；
  `int → float`；`float` 按 f32/f64；`Null → Nullable(_)`；
  命名类型/数组/元组/区间按结构相等。Kotlin 风格整型加宽的教学化简。
- `when` 结果型 = 分支体 join；`if` 双分支 join。
- `fun` 块体**无尾表达式时不与声明返回型比较**（返回经由 return
  语句逐点核查）；声明返回型 + 表达式体（`=` 体）才比较。v1 松弛语义，
  防止 `fun f(): int { return 1 }` 假阳性。

## when 穷尽性（v1 保守规则）

- 有 `else` → 通过。
- 枚举主语：按 `pat_res` 变体索引覆盖计数，缺 → `SemNonExhaustiveWhen`。
- bool 主语：true/false 双字面量覆盖，缺 → `SemMissingElse`。
- 其余主语无 else → `SemMissingElse`。

**顶层裸名规则**（EBNF 歧义的裁决，parser 与 sem 共同实现）：
`BindingPattern` 与 `ConstructorPattern` 均可为裸 `IDENT`。parser 在
when 分支顶层（`arrow_types == false`）把裸名解析为单段 Constructor
候选；嵌套层（元组/数组/构造器实参）恒为 Binding。sem 的 resolve 对
单段 Constructor 不在作用域时报错推迟——check 依主语枚举经
`member_lookup` 精化补写 `pat_res`；精化失败 → `SemUndefinedName`。
两段的 `E.A` 由 parser 的 Dot 分支直接产出 Constructor/Struct 模式。
绑定意图请用带标注/`mut` 形式或 `is` 条件。

## 可变性

`DefKind::LocalBinding.mutable` = 是否可重新赋值，由 `val`/`var`
决定（`var → mutable`）。模式里的 `mut`（`val mut x`）是值可变性标记，
不授予重新赋值权（sem v1 不区分内部可变性）。

## moveck（M3.5 近似）

逐 fun 体 / 顶层绑定初始化 walk。`Own` 绑定（类型 `Named(Own, _)`）
的首次值使用即移动（`Ident` 且 def 为 LocalBinding）；`=` 赋值重置；
分支/循环取两态**并集**（保守：宁过度拒绝，不漏报）。lambda/spawn/macro
内部 resolve 期即未解析，天然跳过。

## v1 跳过清单（遇即 `SemUnsupportedConstruct`）

泛型（fun 类型形参/调用实例化）、方法调用（成员 callee）、impl、
actor、provide、union/dyn/Self 类型、lambda 定型、spawn 内部、
宏展开（独立前置 pass，规格待定；调用点报 `SemMacroExpansion`）。
lambda/spawn 内部整体不解析不定型。

## 诊断码

全部 `Sem*` 前缀，定义在 `crates/ast/src/syntax/node.rs::DiagCode`，
19 个（SemUndefinedName … SemUnknownMember）。解析器码不受影响。