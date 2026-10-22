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
  `int → float`；`float` 按 f32/f64；`T → Nullable(T)`（Null 与任意
  具体类型均可提升入可空，可空间按内层递归、变度不变）；
  汇合侧 `null ∨ T = T?`、`Nullable(S) ∨ T = Nullable(S ∨ T)`；
  命名类型/数组/元组/区间按结构相等。Kotlin 风格整型加宽的教学化简。
- `when` 结果型 = 分支体 join；`if` 双分支 join。
- 比较运算（`<` 族）限定可序类型（数值/字符）；等值按可汇合判定。
- `fun` 返回核查：**仅声明了返回型**才与块体尾表达式比较（表达式体
  `= expr` 恒比较）。未声明返回型按 unit 推断：尾表达式求值但不核查——
  `fun main() { helper() }` 是惯用形状，不得假阳性；裸 `return` 等价
  返回 unit，非 unit 返回型报错。块体无尾表达式时不比较（返回经由
  return 语句逐点核查）。

## Unknown 哨兵（与 Error 投毒分立）

`TyKind::Unknown` = 「类型尚不可推定」（顶层绑定依赖序单遍推断推不出，
如 `val a = b` 先于 `val b = 1` 出现）。与 `Error`（真错误、投毒抑制
级联）相反：`assignable(Unknown, _)` 恒 false、不投毒——使用点照常诊断
（`if a {}` 报 `SemConditionNotBool`），check 收尾再钉一条「无法推定」。
绑定注解槽默认值为 Unknown（`DefSig` 默认），`needs_infer` = Error∨Unknown。

## 成员与关系运算

- `is`：`值 is 模式`，模式绑定进当前作用域（if/while 的 val 条件同纪律）。
- `in`/`!in`：`值 in 集合`，右操作数是集合表达式（EBNF RangeExpr 侧，
  parser 以 range 绑定幂解析），左操作数是元素，按元素类型核查可指派；
  数组/区间以外的一侧报 `SemTypeMismatch`。
- 类型别名：值位置同样是 `TypeObject`（与类型名同纪律）；`type Q = P` 的
  构造调用经 `Def.alias_resolved` 预解析目标走到 P 的字段构造器。

## 值构造与调用

- 枚举变体值：`E.A` / `Opt.Some`（成员访问）的类型即所属枚举
  （`member_field_ty` 的 Enum 臂）；未知变体报 `SemUnknownMember`。
- 构造器调用：callee 定型为 `Named(struct)` → 按 `sig.fields` 走槽位机
  （位置/命名实参皆可）；`Named(enum)` 的 `E.A(x)` / `Opt.Some(x)` →
  变体实参**仅位置形**（裸名槽参数无可信名），命名实参报
  `SemUnsupportedConstruct`。
- 函数值调用：callee 定型为 `TyKind::Fn`（形参/绑定持有的函数值），
  仅位置实参、无默认值补位；命名实参与默认值是具名函数的专有路径。
- **类型对象分立**：类型名在值位置出现得到 `TyKind::TypeObject(DefId)`
  （`def_value_ty`），与「该类型的值」不是同一类型——除 type_call 的
  构造器分支外不可指派/不可调用（`val p = P`、`E(1)` 均诊断）。
  枚举类型对象的成员访问是命名空间访问（`E.A`），在 member_field_ty
  解包为枚举值型；结构体静态访问（`P.x`）仍不支持。
- 未知命名实参报 `SemUnknownParam` 且不级联「missing argument」。

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
决定（`var → mutable`）；for 循环变量与 `val` 条件绑定（`if/while val`）
一律不可变。模式里的 `mut`（`val mut x`）是值可变性标记，
不授予重新赋值权（sem v1 不区分内部可变性）。结构体字段可变性不核查
（v1 松弛，见跳过清单）。

## moveck（M3.5 近似）

逐 fun 体 / 顶层绑定初始化 walk。`Own` 绑定与 **Own 形参**（类型
`Named(Own, _)`）的首次值使用即移动（`Ident` 且 def 为
LocalBinding/Param）；`=` 赋值重置；分支/循环取两态**并集**
（保守：宁过度拒绝，不漏报）。lambda/spawn/macro 内部 resolve 期即
未解析，天然跳过。

## v1 跳过清单（遇即 `SemUnsupportedConstruct`）

泛型（fun 类型形参/调用实例化；签名不 lower，单一
`SemUnsupportedConstruct` 不级联）、方法调用（成员 callee）、impl、
actor、provide、union/dyn/Self 类型、lambda 定型、spawn 内部、
宏展开（独立前置 pass，规格待定；调用点报 `SemMacroExpansion`）。`nclass 声明（无构造路径/方法调用；声明点报一条即注册，避免级联）。
lambda/spawn 内部整体不解析不定型。

`StringInterp` 不在跳过清单：sem 已完整定型（结果 `string`，parts
逐一定型，`"a${nosuch}b"` 的 `SemUndefinedName` 在 part 定型时暴露）——
进跳过清单等于丢掉唯一的实用价值（内部诊断）。lowering（stringify +
concat，或运行时 extern call）归 IR 阶段，运行时入口命名属 IR 契约
（见 opt-notes）。

以下「声称支持但 v1 不核查」的松弛项也已记录在案：

- 明确赋值分析（`val x: int; return x` 不报未初始化）
- 结构体字段可变性（`p.x = 1` 不核查 mutable；class 字段规则待定）
- `Own<T>` 的构造入口（moveck 测试以标注绑定近似，自带一条
  `SemTypeMismatch`，断言完整诊断序列钉住这一点）
- `loop` 的 break 值定型（break_values 收集后未汇入循环型；
  For/While 的 break 值同样不接）
- import 存在性核查（`import a.b.*` 空转不查目标）
- 间接类型别名循环（`A→B→A` 惰性 lower，仅使用时报告；
  直接循环已报 `SemTypeCycle`）
- 直接自引用 struct（`struct N { next: N }` 接受，无限大小留给 codegen）
- 内建类型名大小写敏感（`String` ≠ `string`，样例需随语法修订）

## 诊断码

全部 `Sem*` 前缀，定义在 `crates/ast/src/syntax/node.rs::DiagCode`，
19 个（SemUndefinedName … SemUnknownMember）。解析器码不受影响。