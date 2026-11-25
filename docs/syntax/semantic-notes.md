# Semantic notes（sem crate 实现契约）

语义分析的实现门禁与决策记录，与 `parser-notes.md` 同位。三遍 pass +
结构化明确赋值 + 最小线性 move 检查，纯 std、全确定性
迭代序之外的任何顺序保证）。

## Pass 顺序与理由

```
ParseOutput ─→ declare（作用域树 + defs + 顶层签名 lower）
            ─→ resolve（名字 → DefId；label/flow；pattern 构造器）
            ─→ check（自底向上定型 → expr_ty；Error 投毒抑制级联）
            ─→ definite（结构化明确赋值；分支交集、循环零次保守）
            ─→ moveck（Own 值线性移动检查，join 点保守并集）
            ─→ SemanticOutput（AST 原样 + arena + 侧表）
```

顺序被依赖锁死：前向引用要求名字全量注册先于一切解析（Starry 允许
文件内先用后声明）；签名 lower 在名字注册完成后统一做（别名循环守卫
`Def.resolving`）；resolve 只绑定不定型；check 定型时读 resolve 的侧表；
definite 在 check 后跑（读完整 expr_def/pat_binding_def），moveck 最后跑。

## 侧表契约（核心设计）

语义数据零内嵌 AST。名字 → 类型/定义的全部结果走 `SideTable`：
`NodeId` 索引的稠密 `Vec<Option<T>>`，容量 = `node_count`，O(1)，
越界返回 `None`（受控，不 panic）。公开侧表：`expr_ty`、`expr_def`、
`pat_res`、`flow_res`、`pat_binding_def`（绑定模式
→ LocalBinding DefId）、`ty_of_node`（类型注解节点 → 定型；irc 取模式
标注的 widening 目标）。IR 生成阶段以 `SemanticOutput` 为输入契约。

- `TyArena`：结构去重（相等类型只存一份）；`TyId` Copy，跨 pass 稳定。
- `DefArena` / `ScopeArena`：`DefId` / `ScopeId` 索引；作用域符号表
  `BTreeMap<Symbol, DefId>`（确定性门 + O(log n) 编译期规模无感）。

## 投毒与级联抑制

`TyKind::Error` 为投毒类型：出错子树统一 Error；`assignable` 对
Error 恒真，出错子树不再级联诊断。parser 诊断原样保留在前，语义诊断
附加在后。`Diagnostic` 以稳定 `DiagCode` 为准，message 仅供人读。

## 类型系统 v1

- 字面量默认（M1）：`Float → f64`；`Integer` 未约束默认 **i64**（超 i64
  取 i128，超 i128 报 `IntegerOutOfRange`）。**上下文定型**：标注
  （`val x: i32 = 1`）、形参（`f(1)`）、返回（`return 5`）把整数字面量
  钉到目标宽度，值域按目标宽度诊断（越界 = IntegerOutOfRange，
  不是类型不匹配）。显式窄化仍走 `as`；二元运算操作数不做上下文传播
  （`f(1)` 传播，`1 + 2` 不传播——操作数各自按未约束定型后 join）。
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
- 元组下标 v1 仅接受整数字面量；动态下标只属于数组，非字面量索引报 `SemUnsupportedConstruct`。
- 类型别名：值位置同样是 `TypeObject`（与类型名同纪律）；`type Q = P` 的
  构造调用经 `Def.alias_resolved` 预解析目标走到 P 的字段构造器。

## 值构造与调用

- 枚举变体值：`E.A` / `Opt.Some`（成员访问）的类型即所属枚举
  （`member_field_ty` 的 Enum 臂）；未知变体报 `SemUnknownMember`。
- 构造器调用：callee 定型为 `Named(struct)` → 按 `sig.fields` 走槽位机
  （位置/命名实参皆可）；`Named(enum)` 的 `E.A(x)` / `Opt.Some(x)` →
  变体实参走同一槽位机（变体形参名在 declare 期保留，命名实参
  `Opt.Some(value = x)` 同样可用）。
- 函数值调用：callee 定型为 `TyKind::Fn`（形参/绑定持有的函数值），
  仅位置实参、无默认值补位；命名实参与默认值是具名函数的专有路径。
- **类型对象分立**：类型名在值位置出现得到 `TyKind::TypeObject(DefId)`
  （`def_value_ty`），与「该类型的值」不是同一类型——除 type_call 的
  构造器分支外不可指派/不可调用（`val p = P`、`E(1)` 均诊断）。
  枚举类型对象的成员访问是命名空间访问（`E.A`），在 member_field_ty
  解包为枚举值型；结构体静态访问（`P.x`）仍不支持。
- 未知命名实参报 `SemUnknownParam` 且不级联「missing argument」。
- **`print` 内建**：根作用域注册 `DefKind::BuiltinPrint`；一个位置标量实参（int/float/bool/char/string）定型为 unit，irc 走 `RuntimeFn::Print` 并按实参类型实例化 runtime 入口。

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
不授予重新赋值权（sem v1 不区分内部可变性）。字段/元素赋值穿透到
根绑定：`val p` 的 `p.x = 1`、`val a` 的 `a[i] = 1` 与参数成员赋值
一律报 `SemAssignToVal`（v1 把 `val` 聚合视为整体不可变）。

## 第七轮裁决（穷尽性、聚合格、别名）

- **不可驳模式即穷尽**：`when` 分支出现不可驳模式（`_`、全解构元组/数组/
  结构体、含不可驳替代的 Or）不再要求 else（`is_irrefutable` 判定）。
- **数组/区间逐元素走格**（对齐 Tuple 臂）：`[int; 3] = [1, 2, 3]` 经
  i32→int 加宽成立；无需改动 int=64 位或字面量默认——提升格本就含
  加宽边。`in` 的元素相容按**汇合**判定（比较语义，双向）：
  `x: int in 1..5` 的区间元素是 i32 字面量，i64 与 i32 可汇合。
- **别名急切 lower**：`lower_top_level_sigs` 期统一 alias_target——
  值位置（`F.A`、`O.Some(1)`、模式路径）不经 lower_ty，惰性会让
  alias_resolved 缺席；`def_value_ty` 归一到 TypeObject(alias_resolved)，
  member_lookup 随之跟进（含链式别名）。循环守卫不变（lower_ty 内）。
- **方法调用诊断**：Member 作调用目标时字段查询静默（
  member_field_ty_ex diagnose=false），查不到报「方法调用不支持」，
  不再被 unknown-field 误导；字段存在但非函数类型走 SemNotCallable。
- **`self`/`Self`**（parser 落成同名 Ident）在成员函数外给
  SemSelfOutsideMember，而非未定义名。
- **join 的 TypeObject 判定先于相等短路**（否则 [P, P] 漏网）。
## 模式绑定作用域裁决（第六轮，后端连通后横切）

- 或模式 `A(x) | B(x)`：各替代绑定**同名同变量**——declare 只注册首个
  替代，后续替代的同名词视为同一变量（否则 when/for 分支的标准写法
  被误报 SemDuplicateDef）。
- `is` 表达式的模式绑定注册进**当前作用域**（不新开）：`if x is Some(v)`
  的 then 块经父链可见 v；两个兄弟 is 表达式绑定同名则按块内重名
  纪律报 SemDuplicateDef（有意松弛，见跳过清单的遮蔽项）。
- when/is 的元组/数组模式对聚合主语**按元素类型解构精化**。
- rest 模式不绑定变量，只参与元组/数组槽位映射；含绑定 Or 保持同名同变量纪律。

## definite assignment（结构化保守版）

只追踪 LocalBinding：声明后置未初始化，简单赋值置已初始化；if/when 取分支
交集，循环按零次执行保守（循环内赋值不外溢到出口）。诊断码
`SemUninitializedBinding`；目标是拒绝确定的未初始化读取，不把本 pass
扩成完整数据流求解器。

## moveck（M3.5 近似）

逐 fun 体 / 顶层绑定初始化 walk。`Own` 绑定与 **Own 形参**（类型
`Named(Own, _)`）的首次值使用即移动（`Ident` 且 def 为
LocalBinding/Param）；`=` 赋值重置；分支/循环取两态**并集**
（保守：宁过度拒绝，不漏报）。lambda/spawn/macro 内部 resolve 期即
未解析，天然跳过。

## v1 跳过清单（遇即 `SemUnsupportedConstruct`）

> 2026-11-24 P0 校准：泛型、方法调用、impl、class/ARC、weak、deinit、
> 闭包捕获均已落地，从本清单移除；本清单只保留真实拒收面。

**parse 级拒收（D-014 已从词表删除，2026-11-25）**：actor、provide、aspect（含
before/after/around/proceed）、spawn、async/await/concurrent、宏声明、comptime、
grow 语句、const/inline/infix/vararg modifier、out 型变——词法/文法已移除，
不再进入 sem。
rest 模式与含绑定 Or 已在 sem/irc 解冻。
lambda 内部已在 M8 slice 2 落地（期望 Fn 类型驱动的定型 + body resolve + irc lowering）。

`StringInterp` 不在跳过清单：sem 已完整定型（结果 `string`，parts
逐一定型，`"a${nosuch}b"` 的 `SemUndefinedName` 在 part 定型时暴露）——
进跳过清单等于丢掉唯一的实用价值（内部诊断）。lowering（stringify +
concat，或运行时 extern call）归 IR 阶段，运行时入口命名属 IR 契约
（见 opt-notes）。

以下「声称支持但 v1 不核查」的松弛项也已记录在案：

- 结构体字段可变性已核查（见「可变性」：val 聚合整体不可写）
- `Own<T>` 的构造入口（moveck 测试以标注绑定近似，自带一条
  `SemTypeMismatch`，断言完整诊断序列钉住这一点）
- import 存在性核查（`import a.b.*` 空转不查目标）
- ~~`as` 不核查可转换性~~ **已修复（P0，2026-11）**：`as`/`as?`/`as!` 现按
  数值↔数值、同型、Ref/Pointer 互转核查，其余 `SemTypeMismatch`。
- 内建类型名大小写敏感（`String` ≠ `string`，样例需随语法修订）
- 间接类型别名循环（`A→B→A` 惰性 lower，仅使用时报告；
  直接循环已报 `SemTypeCycle`）
- 整数字面量的值域：超出 i32 自动取 i64/i128，超出 i128 报
  `IntegerOutOfRange`；`as` 窄化仍按显式转换接受
- 直接自引用 struct/enum（`struct N { next: N }`）按无限大小拒绝
  （`SemTypeCycle`，codegen 不再栈溢出）

## 2026-11 审查修复（语义契约）

- **默认实参参与 resolve/check**：`fun f(x: int = expr)` 的 `expr` 在
  参数作用域内解析并定型一次；类型不符报 `SemTypeMismatch`，irc 调用点
  重放 AST 时 `expr_ty/expr_def` 已就绪。默认值可引用函数/全局。
- **`if val` / `for` 模式走同一主语下推**：`refine_pattern_types` 统一
  处理构造器（补 `pat_res`）与元组/数组元素类型，`if val Some(v)`、
  `for (a, b) in pairs` 与 `when` 的模式绑定语义一致。
- **物化 Range 保存开闭**：`Range(T)` 布局 = lo、hi、i8 开闭标记
  （`..`=0，`..=`=1）；`in`/`for` 消费物化值时按标记选择 `<`/`<=`，
  与区间字面量路径一致（不再有「物化后按闭区间」的 corner）。
- **结构相等**：字符串 `==`/`!=` 走 `starry_str_eq` 内容比较；struct/
  数组/元组/区间/nullable/enum 逐槽递归比较（enum 先比 tag 再比该变体
  payload）。聚合比较不再落到 LLVM `icmp` 指针。
- **递归聚合类型**：struct/enum 按值包含自身（含 nullable 字段）报
  `SemTypeCycle`，避免 `scalar_slots` 无限递归。
- **val 聚合不可写**：字段/元素赋值穿透根绑定，`val` 与参数报
  `SemAssignToVal`。
- **extern "c" 的 i128 标量 ABI 拒收**：i128 形参/返回值跨 C ABI 在本目标上与 IR i128 传参不一致（llvme 对 runtime 入口拆 hi/lo 规避，extern 无法替用户改 C 原型）——声明点报 `SemUnsupportedConstruct`；别名经 lower 归一为 `Int(I128)` 同样命中。聚合参数按指针传递不受影响。
- **递归类型检测与全局依赖排序改显式栈迭代**：声明数不受 parser 深度限制，数万级 struct 字段链 / 全局初始化链的递归 DFS 会爆栈；诊断根序按声明源序稳定（原 HashMap 迭代序随进程随机）。
- **if val 两段构造器精化**：`if val Opt.Some(w) = o` 的构造器路径此前只在
  `is` 表达式解析（pat_res），if/while 的 val 条件漏 `resolve_pattern`——w 不被
  payload 精化（差分 harness 抓到）。现三处同纪律。

## 当前契约 vs 设计差距（M0 登记，2026-11-07）

以下差距按 implementation-plan 的里程碑归属，**当前实现不伪装成设计目标**：

- **指针（D-007 / M2）**：`*const T`/`*mut T` 类型+FFI 传参+`&x`/`*p`
  表达式已落地（v1 保守借用见下节）。bare `*T` 现诊断（ExpectedToken）。
  剩余 T2 checked pointer 后置 M10（Q7 裁定）。
- **Raw pointer unsafe gate**：`*const T` / `*mut T` 的解引读取、写入、复合读写都在同一
  `unsafe_depth` 门下检查；`&T` / `&mut T` 与裸指针创建/转换保持安全。
- **可空（D-007 / M2）**：前缀 `?T` + 原子后缀糖 `T?` 已落地（同一
  `Nullable` AST）；复合后缀 `[T; N]?`/`(T, U)?` 拒绝并提示前缀形态。
- **引用（D-007 / M2）**：`&T`/`&mut T`/`?&T` 类型 + `&x`/`*p` 表达式已
  落地；借用检查为 v1 保守规则（见下节）。

## 字段可变性三态与主构造（M3）

- 主构造：`struct Point(val x: int, var y: int)`——`val`/`var` 头参数
  声明字段（槽序 = 参数序 ++ body 序，重名报 SemDuplicateDef）；无修饰符
  参数仅构造期使用（求值保留、值不存）；默认值与具名实参同函数纪律。
- 字段可变性三态（`DefKind::StructField.mutable`）：`Some(true)`=var 字段
  恒可写（`val` 根绑定也拦不住——Swift 深度可变性）；`Some(false)`=val
  字段恒拒（`var` 根绑定也写不了）；`None`=legacy 无修饰符 body 字段，
  按根绑定穿透检查（兼容既有语义）。
- body 字段可带显式 `val`/`var` 硬关键字前缀；引用字段（头参数与 body
  同纪律）在 declare 期拒绝（M2 非逃逸规则的 M3 延申）。

## T1 借用规则（M2，v1 保守版）

- 借用目标限**局部名**（Ident）；复合表达式目标、`&` 复合目标（字段/索引链）
  报 `SemUnsupportedConstruct`。
- **顶层全局不可借用**（跨函数生命周期 = 逃逸）：`SemUnsupportedConstruct`。
- `&mut` 要求可变绑定（`var`）；`val`/参数报 `SemAssignToVal`。
- **非逃逸**：引用不可存（struct 引用字段在 declare 期拒绝）、不可返回
  （返回型 `&T` 拒绝）；引用只与同一 `Ref` 类型等价（格无聚合臂）。
- 经解引赋值要求 pointee 可变（`*mut`/`&mut`），否则 `SemAssignToVal`。
- 引用 → 裸指针（FFI 边界）须显式 `as`（`(&mut x) as *mut T`），
  不做隐式转换；`as` 比前缀 `&` 绑定松，取址后转换要加括号。
- 重叠 `&mut` 追踪（活借用区间数据流）未实现——v1 的规则集是
  「不可逃逸 + 不可变借用只读」的最小安全面，区间追踪列为 M10 前专项。
- **主构造（M3）**：✅ 已落地（见上节「字段可变性三态与主构造」）；
  剩余：泛型 struct 默认值逐实例化检查未排期。
- **class/ARC（M4）**：稳定拒收不变；spike 已按 D-004 重写（ARC 默认、
  arena 后置）。R3/R4 已裁定 v1 形态（线程本地非原子、`weak var`、
  `deinit`、引用相等、默认 final、无二级构造）。class 继承语法稳定拒收
  （`SemUnsupportedConstruct`，"v1 classes are final"；2026-11 审查修复）。
- **泛型（M7 slice 1）**：fun/struct 单态化已落地（见 generic-mono-spike.md）；
  trait bounds/形参默认值/实参推断/泛型 enum 已落地（R2 核心 exit
  承诺边界仅约束工期，不约束能力清单）。
- **trait/impl（M9）**：声明/静态分派/bounds/泛型 impl 已落地。契约：
  impl 头对泛型 enum/struct 的实例化**延后到签名 lower 后注册**
  （`register_impl`，2026-11 修复——declare 期急切实例化会缓存
  sig.variants/params 未 lower 的空壳 spec，方法分派/构造槽位随之
  破缺）；type_def 指向实例化 spec，重复 impl 按同一 spec 拒收；
  trait 不作类型标注（静态分派 only）；v1 不限 orphan（单 crate）。

## class + ARC（M4 slice 1，R3/R4）

- **值模型**：class 值 = 对象指针（标量化）；header 定案 24B（strong/weak/
  meta——D-004 ABI 先钉），payload 紧跟 header，字段槽全 i64（i128/聚合
  字段 slice 1 拒收，保 8B 槽寻址：OffsetPtr = 24 + 槽*8）。
- **构造**：主构造 val/var 头参数 = 字段（M3 三态同纪律）；`P(...)` 经
  `starry_alloc_object` 分配，构造者持 strong=1（免 retain）。
- **复制与释放**：`val b = a` 发 `starry_retain`；class 局部绑定注册
  **隐式 drop**（与 defer 同出口纪律：块出口/return/break 逆序
  `starry_release`）。move 消除（构造期 retain/release 对）是 M4 优化项。
- **线程模型**：R3 线程本地非原子（跨线程 atomic 后置）。
- **slice 2b：方法 + deinit**——`x.m(args)` 静态分派（self 作首参对象
  指针注入，`@m.N` 按 DefId 唯一命名）；成员体经 resolve/check
  全流程（self 绑定为 Param，sig.ty 复用为 class 值类型通道）；
  `deinit { }` 走 init 同款文本槽位词（R4 拼写裁定；不升硬关键字，
  与 init 先例一致）；末引用 release 经 **header meta = deinit 函数
  指针**回调（AllocClass 指令两参：payload 字节 + deinit 或 null）。
  **隐式 self 裸字段名稳定拒收**——方法/deinit 体内须显式 `self.x`
  （裸字段名经 Ident 守卫给 `SemUnsupportedConstruct`："field ... requires
  self."；irc 仅绑定 self，契约一致性 2026-11 审查修复）；verify 覆盖
  断言在已有错误诊断时跳过（不再 panic 盖诊断）。
- **slice 3：weak var**——`weak` 走 **lexicon 修饰词表**（15 个，syntaxgen
  正道；语句级 `weak var` 前瞻分支，peak=weak 且次位 var/val 才走绑定）。
  初始化 `starry_weak_retain`（weak++），块出口 `starry_weak_drop`
  （weak--；strong==0 && weak==0 释放 header）。**e2e 语义验证**：weak 持有
  下两对象的 deinit 均触发（`2 1`），证明不延长生命周期。
  **slice 3 拒收面**：weak 读取（裸名与成员访问两臂——upgrade 零化
  语义 slice 4）、weak 重赋值（簿记未排期）、`weak val`（须 var）、
  非 class 初始化（SemTypeMismatch）。
- **slice 1/2b 拒收面**（稳定诊断）：i128/聚合 class 字段、主构造/
  方法默认值、二级构造（init 块，R4）、class body 字段绑定（字段走
  主构造 val/var 头参数）、仅构造参数之外的裸字段名。
- **slice 4c：class 形参/返回（+0/+1）**——形参 = **+0 借用**：调用点
  不 retain，所有权留在调用方；被调侧绑定形参时显式 retain、块出口
  release（平衡）。返回 = **+1 owned**：调用点绑定接管 release——
  绑定对**任意调用 init**（构造/函数/方法，统一 +1 约定）免 retain，
  仅登记块出口 release。`?P` 同机制（聚合返回 = 基址指针 ABI，槽机
  逐槽拷贝）。e2e：deinit 每对象恰好一次；callee 内 `val local = p`
  retain/drop 平衡；函数返回 +1 绑定不重复计数。
- **slice 4b（Plan C）：所有权闭环 + weak upgrade**——裁定否决 A（autorelease
  池：推迟 deinit、不修复 return 路径、循环峰值）与 B（立即绑定：不修复
  `val b = id(a)` UAF、打断链式表达式、无官方先例），选 **C：统一 +1 返回 +
  full-expression 临时释放**（Clang ARC `objc_loadWeakRetained`、C++
  `[class.temporary]` 同模型）。三件套：
  ① **+1 返回统一**：`return e` 若 e 为 +0 借用（标识符/字段/形参）先 retain
  再返回（修 `id(p)` UAF）；+1-origin（调用/null/??/if-class）直接转移。
  ② **语句级临时释放**：`stmt_temps` 登记构造/调用返回值/upgrade/汇合点的
  +1 临时值；绑定/返回/赋值 take 转移，其余在**语句末/条件求值后/分支末**
  release（路径局部，正确；无池）。`??` 与 if 的 class 结果在分支内归一化
  为 +1（借用分支 retain、+1 分支转移）。修裸 `make()`/`make().x` 泄漏。
  ③ **weak upgrade**：`starry_weak_upgrade`（strong>0 → strong++ 返回对象；
  strong==0 → null；不碰 weak 计数）。weak 读取定型 `?P`，直接 `!!`/`?.`/
  `??`/绑定；upgrade 结果按 +1 临时值纪律回收。`w.x`（非安全成员）仍拒
  （需 `?.`/`!!`）；weak 重赋值仍拒。`weak var w = P(..)`（构造初始化）
  在语句末放弃强引用——weak 不延长生命周期的正确语义。
  e2e：三探针（裸调用/成员临时、return-p UAF、upgrade 全形态 + 1e5 循环）
  deinit 逐对象恰好一次。
- **slice 4d：class/?P var 重赋值所有权**——重赋值先 release 旧 payload
  （null 载荷为 runtime no-op），写回后按 rhs 来源补 retain：调用结果
  （+1）与 null 字面量免 retain，借用值（标识符/成员）显式 retain。
  不变式：var 恒持恰好一个引用，与绑定时登记的块出口 drop 配对——
  修复「`b = a` 不 retain 致悬垂 / 旧值不 release 致泄漏」。语法自赋值
  `b = b` 特判为 no-op（否则先 release 后 retain 会过零释放）。e2e：
  跨作用域借用重赋值后旧对象 deinit、新对象可读；可空 var 全链平衡。
- **slice 4a：可空 class（`?P`）**——布局 = flag（i8，槽 0）+ payload
  （对象指针，槽 1）；`scalar_slots`/`slot_ty` 对 Named class 取 1 槽
  口径（与 is_aggregate 一致）。绑定纪律与 class 相同：物化后 retain
  payload（null 字面量初始化免 retain——payload 恒空），块出口 release
  payload（槽 1）。**release/retain 的 Param 类型必须取 payload 类型**：
  传外层 `?P`（聚合口径）会把槽单元地址当对象指针（已踩坑修复）。
  `== null`/`!= null`/`!!` 直接工作；`?.x` 走对象指针 + header 偏移
  （payload 是对象指针，不是内联字段槽）；`??` 逐槽拷贝天然成立。
  e2e：共享对象 deinit 恰好一次、ctor-into-nullable 免 retain、
  null 释放 no-op。**slice 4a 拒收面**：weak upgrade 读取（slice 4b
  裁定）。

- **解释器**：class 对象在原生堆，interp 无地址模型——class 程序
  一律 Trap（与 extern/指针程序同纪律，差分不入拍）。

## checked 溢出与 wrapping 算子（M1.5，Q5 + D1–D5 裁定）

- 整型 `+ - *` **默认受检、所有 build mode 生效**（D5：release 只关 T2
  generation，不关算术）。sem 编译期核查：常量 `+ - *` 溢出钉
  **`SemConstOverflow`**（D3：独立于字面量值域码）；常量 `/ 0`、`% 0` 钉
  **`SemDivisionByZero`**；常量 `MIN / -1` 钉 **`SemDivisionOverflow`**。
  运行期经 `llvm.sadd/ssub/smul.with.overflow` 序列 trap。
- **trap 契约（D2/D4）**：`starry_overflow_panic(code, fun)` 声明标
  `noreturn`（属性组）；向 **stderr** 打印区分后的原因 + 函数名（源位置
  近似），`ExitProcess(134)`。code 1–7：add/sub/mul 溢出、div by zero、
  div 溢出、rem by zero、rem 溢出。**不执行 defer/ARC 清理**（fatal abort，
  非可恢复 panic）；v1 运行时 Windows-only（`GetStdHandle`/`WriteFile`/
  `ExitProcess`，无 CRT）。
- 除法受检：除零与 INT_MIN ÷ -1 在 `sdiv/srem` 前显式守卫，两个守卫
  分支各自携带区分后的 code。
- 显式 wrapping：**`` +% -% *% ``**（D1 裁定：Zig 官方 wrapping 拼写；
  `+|` 保留给未来 saturating）——仅整型（不做字符串连接，浮点无回绕
  语义）；发裸指令，interp 保持回绕。
- **二元字面量上下文传播（D3）**：`val x: i32 = 2147483647 + 1`、
  `return …`（声明返回型）、实参位置的「两端皆整数字面量」算术式，先把
  操作数重定型到期望类型并重定型二元节点（`retype_binary_literals`，
  六处调用点）——否则默认 i64 先 join，窄上下文够不到。
- 提升格不变：默认 i64 使「窄域常量对」罕见——运行期窄算术需两操作数
  皆窄（形参/标注变量），此时按窄位宽受检。
- **release 语义（D5 裁定，写进 D-011 语义）**：算术 `+ - * / %` 在所有
  build mode 下恒受检；release 只关 T2 generation 检查（未实现），
  **不关算术**；性能不足用 `+%` 显式回绕，不改变 `+` 的语义；CI 已加
  debug/release 双跑断言 trap 行为一致。
- **i128 状态（M1.5 未清项，已按裁定记录）**：布局仍为「按类型宽槽」
  （i128 单槽 16B、含 i128 聚合 `[n x i128]`），**Q5 的 2×i64 尚未实施**——
  反转需 benchmark 证据（2×i64 要先实现才能对拍，鸡生蛋），列为 M1.6 专项
  待用户定向。interp i128 已全 128 位建模（2026-11 修复：槽 u128、
  checked/wrapping 128 位判据、`Value::I128`；i128 已入差分对拍）。

## extern "c" 调用的 unsafe 边界（M6）

- FFI 永远 unchecked：`extern "c"` 函数**只能在 `unsafe` 块内调用**，
  否则报 `SemUnsupportedConstruct`（「calling an `extern "c"` function
  requires an `unsafe` block」）。纯声明不调用无诊断。
- sem 用深度计数（`SemState.unsafe_depth`）实现：`unsafe` 表达式臂进出
  增减深度，深度贯穿普通块嵌套（unsafe 内嵌套块继承许可），离开块后
  归零。
- 检查点钉在命名函数调用定型路径（`type_call` 的 `expr_def → DefKind::Fun`
  臂）：外部符号才有 external 标记，普通函数调用零开销（一次 bool+usize
  比较）。
- `unsafe { }` 是块表达式，值 = 块尾式；FFI 调用的结果照常参与定型。
- Raw pointer dereference：operand type 是 `TyKind::Pointer` 时，读取、写入
  赋值与复合赋值都要求 `unsafe_depth > 0`；诊断码 `SemUnsafeRequired`，
  message 为「raw pointer dereference requires an unsafe block」。检查共用
  `require_unsafe_if_raw_deref`，并按 (span, code) 幂等，避免赋值目标先按
  表达式定型再进入赋值臂时重复报告。
- Closure 是函数边界：进入 lambda body 前保存并把 `unsafe_depth` 归零，
  body 检查后恢复；创建位置在 unsafe 中不给 body 授权。

## 闭包捕获（S2/P2-3，2026-11-11；spec = closure-capture-spec.md §6c/修正 7）

- **捕获分析（P2-3a，sem 已落地）**：`lambda_cap_stack`/`lambda_caps`
  （首现序）+ 三类稳定拒收——mutable 全局引用、lambda 内捕获名赋值、
  捕获快照语义（捕获外层 `var` 后外层再改不影响已捕获值）。
- **表示（P2-3b-1 落地，tagged 双形态）**：无捕获 lambda / 具名 fn =
  裸函数指针（`Const::Fun`，零分配，C 回调直通）；有捕获 lambda =
  ARC box（`AllocClass`，payload = [fn_ptr, caps...]）+ **tag 位**
  （值 = payload 指针 | 1；裸 fn bit0=0）。payload 布局：fn_ptr 在
  raw+0，caps 自 raw+8；lambda 体以 **raw box 为 env 首参** 读槽。
- **调用约定（修正 7）**：`Callee::Indirect` 统一 runtime tag 分派
  （bit0=0 裸 fn 直调；bit0=1 取 payload[0] + env 首参）——形参持有的
  Fn 值静态未知形态，分派开销实测 +3%（drafts/p2b-exp/RESULT.md）。
  `Const::Fun`（@ 常量）保留直调特例，零开销。
- **C 边界（矩阵 #16）**：捕获 lambda 作 extern "c" 实参 → irc 稳定
  拒收（tag 位会被当代码地址）；裸 fn 直通不变。
- **box ARC（批次 2 已落地，2026-11-11）**：class 捕获创建侧 retain
  +1；box 的 meta = 合成 env 释放函数（llvme 按 `env_caps` 生成，逐
  class 捕获槽 release）；Fn 绑定走 class 同纪律（创建 +1 take →
  scope 出口 release；借用拷贝显式 retain）——Retain/Release 按 tag
  位分派（裸 fn no-op）。`is_aggregate(Fn)` 改为 false（Fn = 标量化
  指针，与 scalar_slots=1 / class_of=Scalar 对齐——此前 Fn 落进聚合
  别名路径导致绑定 drop 缺失，已修）。e2e：矩阵 #4/#11 deinit 恰好
  一次（`class_capture_deinit_balance_e2e`）。
- **批次 3（同日落地）**：矩阵 #6（lambda 捕获外层 fn 形参 + 逃逸
  返回）与 self 捕获 e2e（deinit 平衡）；Fn struct 字段 declare 侧
  稳定拒收（主构造头字段 + body 字段两路，矩阵 #5 拒绝面）；4 个
  闭包用例入选 diff_native（interp 注册表 vs native tag 位逐行对拍，
  含绑定位置推断与快照语义）。
- **字段级 ARC traversal（2026-11-25 已开启）**：`emit_copy_value`/`emit_drop_value` 对 struct/tuple/array/type-directed 递归；enum payload 按 active tag 分支 retain/release。owned/borrowed 语义由语句临时表转移；struct 构造临时用结果型登记，避免第二次 retain 泄漏。e2e 钉住 scope/return/release 平衡。
- **Fn var 重赋值（已落地，2026-11-11）**：lower_assignment 的
  classish 门扩展到 Fn——release 旧 box / retain 或转移新值（tag 分派，
  裸 fn no-op）；e2e `fn_var_reassign_arc_e2e`（deinit 平衡）。
- **可空 class 捕获（已落地，2026-11-12）**：前置问题已修——
  `emit_indirect_call` 的 Agg 返回缺槽拷贝（间接调用把 callee 栈地址当
  值存）补齐 `emit_agg_result_copy`（与标准 Call 臂同式）；启用 2 槽
  env（flag i8 + payload ptr）+ 创建侧 flag 分支 retain + env 释放
  函数 flag 分支 release + body 2 槽重建（payload 从 box 借用）。
  e2e：Some（payload 保活）/ None（flag=0 不 retain）/ 柯里化
  （`mk(1)(2)`）全通。**柯里化调用**（callee = 调用结果表达式）call()
  增加 Fn 类型判别走 fn_value_call。**lambda-in-lambda 解析修复**：
  parse_lambda body 位 `{` 前瞻消歧（配对 `}` 前深度 1 的 `->` =
  嵌套 lambda，否则块；parse_lambda 期望 cursor 停在 `{`——调用侧
  不得预消费）。两个 lambda 扫描器 RBrace 深度下溢 saturating 防护。
- **指向可空的类型（2026-11-25 已支持 WP2 payload）**：D-007 契约保留；
  `*p` / `*p = v` 按 `Nullable(T)` 的 `scalar_slots` 逐槽访问，slot > 0
  经 `OffsetPtr` 偏移。class/可空 class 走 ARC，i128 用 wide payload。
  `value_abi_unsafe` 稳定拒收（需字段级 ARC 或 wide-slot ABI）。
- **顶层全局（2026-11-25 WP3 已落地）**：纯值按 `[n x i64]` 展开；
  `LoadGlobal` / `StoreGlobal` 携带 slot。`int?` 全局是 flag+payload；
  纯值聚合初始化函数逐槽写全局并 `ret void`。class/可空 class 是
  进程根（不保证 exit deinit）；i128 用 wide ABI；bare fn 全局允许，
  boxed Fn/未建模 ABI 稳定拒收。
  Fn var 重赋值的可空 class 变体已有手动 ARC probe 平衡，但 targeted
  native e2e 与 ASan harness 仍待接；超大栈帧 1MB 边界 regression 待补。
- **绑定位置 lambda（已落地，2026-11-11）**：`val f: Fn注解 = {…}` 把
  注解 Fn 型作期望压栈（与 check_call_args 实参压栈同纪律）；无注解
  绑定的显式参数全带类型标注时从标注构造参数表、返回型由 body 推断
  并回填 lambda def 的 sig.ret（占位 unit 必须回填——irc 按 def sig
  lower 签名）。`it` 形式/无标注参数仍稳定拒收（无信息可推）。
- **interp**：闭包注册表（无堆，`ClosureBox{fn_def, caps}`）；槽值
  编码与 native tag 对齐（裸 fn = def<<1，box = (id<<32)|1，payload
  地址 = (id<<32)+8j）；不建模 ARC（spec §4.4-6 纪律）。

## Trait object-safety 预检（WP5）\n\n- Trait 声明期对泛型方法、Self 返回值发稳定 `SemObjectUnsafe`；这是为\n  后续 dyn/ thick-pointer 切片提前建立的可实现边界，不是运行时 ABI。\n- v1 仍拒绝 trait 作为类型标注；当前诊断保持 `SemUnsupportedConstruct`。\n\n## 泛型 alias 与 trait 继承（WP4/WP5，2026-11-25）\n\n- 泛型 alias 在签名 lower 期保存 Param-bearing target；use-site 传实参后\n  `subst + concretize` 返回具体目标。非泛型 alias 仍急切解析，成员路径\n  行为不变。\n- trait 继承只允许单路径；diamond/cycle 稳定拒收。派生 impl 的继承方法\n  可由既有 supertrait impl 满足；派生 registry 不复制方法，静态分派仍由\n  各 trait 的最具体 impl 提供。\n- 泛型 trait 声明与签名入表，但 impl 实参核对/分派仍后置；关联类型与\n  trait default method 保持稳定拒收。\n\n## 诊断码

全部 `Sem*` 前缀，定义在 `crates/ast/src/syntax/node.rs::DiagCode`，
27 个（SemUndefinedName … SemUnknownMember、SemTypeArgMismatch、SemConstOverflow、SemDivisionByZero、SemDivisionOverflow、SemUnsafeRequired）；另有 `Irc*` 2 个属 irc
阶段（码表同址，见 opt-notes）。解析器码不受影响。

## 关键字收敛（D-014，2026-11-25）

- **parse 级拒收**（词已删）：actor/provide/aspect/spawn/async/await/
  concurrent/grow/comptime/macro 声明/const/inline/infix/vararg/out/namespace/static。
- **sem 级拒收**（保留字，语义待实现）：union 声明、accessor（get/set）、
  dyn/Self（trait 外）、宏调用展开（SemMacroExpansion）、
  open/abstract/sealed/override/operator、二级构造 init、protected。
- **已实现**：alias（类型别名）、deinit（析构）、by（superitem 委托槽，
  随继承实现）、it/move 形式（WP6 e2e；move 当前是捕获形式标记）。
- 冻结预算：35 hard / 10 modifier / 8 slot = 53，不再减少（2026-11-25 删 `namespace`、`static`）。
