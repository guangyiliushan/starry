# Feature matrix（逐层实现状态）

每个语言特性在编译链五层的状态：parser → sem → irc → llvme → runtime
（外加 irc 参考解释器作语义验证层）。状态记号：**✅** 完成、**🟡** 部分
（限制见备注）、**❌** 未实现（sem 给稳定 `SemUnsupportedConstruct` / `SemObjectUnsafe` 或
irc 给 `IrcUnsupportedLowering`，见 semantic-notes 跳过清单；已删关键字（D-014）在 parse 级拒收）。

来源：`coverage.md`（EBNF↔AST 覆盖）、`semantic-notes.md`（语义契约）、
`opt-notes.md`（ lowering/优化契约）。本文是**状态矩阵**，不改契约；
契约变化改上述三份，本表随里程碑更新。

## 标量核心（Feature Group 0）

| 特性 | parser | sem | irc | llvme | runtime | 备注 |
|---|---|---|---|---|---|---|
| 整数字面量（全 radix、下划线） | ✅ | ✅ | ✅ | ✅ | ✅ | 未约束默认 i64（超 i64 取 i128，再超报 `IntegerOutOfRange`）；标注/形参/返回上下文按目标宽度定型（M1） |
| 浮点字面量（f32/f64 位模式） | ✅ | ✅ | ✅ | ✅ | ✅ | 十六进制位模式发射 |
| 字符串字面量/转义 | ✅ | ✅ | ✅ | ✅ | ✅ | 插值 chunk span 兼容 |
| char 字面量 | ✅ | ✅ | ✅ | ✅ | ✅ | UTF-8 码点 |
| bool / unit / null | ✅ | ✅ | ✅ | ✅ | ✅ | null = Nullable 提升 |
| 算术/位运算/比较 | ✅ | ✅ | ✅ | ✅ | ✅ | checked 默认溢出（stderr panic，exit 134）；wrapping 显式 `+%` `-%` `*%`；比较结果 i1 |
| 短路布尔 / ! | ✅ | ✅ | ✅ | ✅ | — | Br 链物化 |
| `as` 数值转换 | ✅ | ✅ | ✅ | ✅ | — | 全有符号 Conv |
| `print` 内建 | ✅ | ✅ | ✅ | ✅ | ✅ | 标量六类入口（i1/i128 特判） |
| 具名/默认/命名实参 | ✅ | ✅ | ✅ | ✅ | — | 默认值调用点重放 |
| 函数值类型 `(int, int) -> int` | ✅ | ✅ | ✅ | ✅ | — | M8 + P2-3b-1：具名函数作值 + 无捕获 lambda（裸 fn 直调）+ 标量捕获 lambda（ARC box + tag 位分派）+ 经形参间接调用 + 绑定位置 lambda（注解/参数标注驱动定型） |

## 数据布局（Feature Group 1）

| 特性 | parser | sem | irc | llvme | runtime | 备注 |
|---|---|---|---|---|---|---|
| struct + 构造/字段 | ✅ | ✅ | ✅ | ✅ | — | 位置/命名实参槽位机；M3 主构造（val/var 头参数=字段） |
| 字段可变性三态（M3） | ✅ | ✅ | ✅ | ✅ | — | var 字段恒可写 / val 字段恒拒 / 无修饰按根绑定 |
| enum + 变体/tag | ✅ | ✅ | ✅ | ✅ | — | tag i8（>255 诊断）；结构相等 |
| 数组（字面量/动态索引/填充） | ✅ | ✅ | ✅ | ✅ | — | 字面量按目标元素宽直写（2026-11 修复） |
| 元组 | ✅ | ✅ | ✅ | ✅ | — | 常量下标 |
| 区间（`..`/`..=`） | ✅ | ✅ | ✅ | ✅ | — | 物化值携开闭标记槽 |
| nullable（`?`/`??`/`!!`/`?.`） | ✅ | ✅ | ✅ | ✅ | — | 值→Nullable 显式构造（2026-11 修复） |
| 顶层值全局 | ✅ | ✅ | ✅ | ✅ | — | 依赖序初始化；纯值 `[n x i64]`；class 进程根、i128 wide ABI、bare fn 全局；boxed Fn 拒收 |
| 结构相等（==/!=） | ✅ | ✅ | ✅ | ✅ | — | 字符串走 `starry_str_eq` 内容比较 |

## 控制流与模式（Feature Group 2）

| 特性 | parser | sem | irc | llvme | runtime | 备注 |
|---|---|---|---|---|---|---|
| if/when 表达式值 | ✅ | ✅ | ✅ | ✅ | — | 结果槽绕 phi |
| when 穷尽性 | ✅ | ✅ | ✅ | ✅ | — | 枚举覆盖/布尔双字面量/不可驳模式 |
| 循环（for/while/loop） | ✅ | ✅ | ✅ | ✅ | — | 块尾 loop 归一 LoopExpr（2026-11） |
| 标签 + 带值 break | ✅ | ✅ | ✅ | ✅ | — | `'outer: loop { break 'outer 42 }` |
| defer | ✅ | ✅ | ✅ | ✅ | — | 出口逆序 |
| 模式匹配（is/when/for/val 绑定） | ✅ | ✅ | ✅ | ✅ | — | 构造器/元组/数组/rest/Or 共享绑定 |
| `if val` / `while val` | ✅ | ✅ | ✅ | ✅ | — | 两段构造器精化（2026-11 修复） |
| `in` 成员判定 | ✅ | ✅ | ✅ | ✅ | — | 区间/数组扫描；字符串元素内容比较 |

## 运行时契约（Feature Group 3）

| 特性 | parser | sem | irc | llvme | runtime | 备注 |
|---|---|---|---|---|---|---|
| 字符串拼接/插值 | ✅ | ✅ | ✅ | ✅ | ✅ | StrConv/StrConcat 链 |
| 字符串相等 | ✅ | ✅ | ✅ | ✅ | ✅ | `starry_str_eq` |
| `extern "c"` FFI | ✅ | ✅ | ✅ | ✅ | — | extern 调用须 `unsafe` 块（M6）；`build -- <clang 参数>` 链接；i128 标量 ABI 拒收 |
| 原生运行时平台 | — | — | — | ✅ | ✅ | Windows freestanding（Heap*/WriteFile/ExitProcess + mainCRTStartup）；Linux libc（malloc/free/write/_exit）；公共符号与 trap exit 134 稳定 |
| main 返回值=退出码 | ✅ | ✅ | ✅ | ✅ | — | int trunc/sext、bool zext |
| 模块系统（M5 slice 2a–3） | ✅ | ✅ | ✅ | ✅ | — | manifest 包身份 + 目录推导模块路径（无 package 头）+ 包命名空间 + `import a.b`/`import util` 限定访问 + 可见性（默认 public/internal 包私有/private 文件私有/protected 拒收）；循环/自导入合法 |
| class/ARC（M4 切片 1–4b，Plan C） | ✅ | ✅ | ✅ | ✅ | ✅ | 构造/字段/方法/deinit/weak var + upgrade 读取（`?P`）/可空 class/形参 +0/返回 +1 统一/重赋值所有权/语句级 +1 临时释放（`stmt_temps`，full-expression 纪律）；type-directed drop traversal 已建，但 ARC 聚合元素/字段仍稳定拒收 |
| 闭包捕获（P2-3b-1/2） | ✅ | ✅ | ✅ | ✅ | ✅ | 标量/class 捕获 box（tagged）+ runtime tag 分派 + 快照语义 + box ARC（meta 逐 class 捕获 release，deinit 平衡 e2e）+ C 边界拒收；可空/i128/聚合捕获归批次 3（见未实现表） |
| 泛型单态化（M7） | ✅ | ✅ | ✅ | ✅ | — | fun/struct/enum 单态化；泛型 alias use-site 代入；显式/推断实参、类型形参默认值、bounds 兑现；泛型 trait 声明已入表（impl 参数核对后置）；impl 注册延后到签名 lower 后 |
| trait/impl 静态分派（M9） | ✅ | ✅ | ✅ | ✅ | — | trait 声明/impl/bounds/泛型 impl/单路径继承；派生 impl 复用 supertrait impl 满足继承方法；diamond/cycle 拒收；关联类型/default method 仍拒收 || `dyn Trait` class-root 动态分发 | ✅ | ✅ | ✅ | ✅ | ✅ | Phase 2a：胖指针 data+vtable，vtable `[drop,size,align,methods...]`；`?dyn`/`?.`/`!!` 已覆盖；struct-borrowed dyn、upcast、dyn 存聚合/全局仍拒收 |
| 指针 `*const T`/`*mut T`（T2 thin） | ✅ | ✅ | ✅ | ✅ | — | M2：类型 + FFI + `&x`/`*p` + StorePtr；raw 解引读取/写入/复合读写须 unsafe（`SemUnsafeRequired`），创建/转换安全；T2 checked pointer 后置 M10（Q7） |
| 引用 `&T`/`&mut T`（T1） | ✅ | ✅ | ✅ | ✅ | — | M2；v1 保守借用（R6：局部名/不逃逸/全局拒借/不可变只读）；重叠 `&mut`/区间追踪后置 M10 前专项 |
| `--release` 构建 | — | — | — | ✅ | — | clang `-O2`；Windows freestanding CRT stubs、Linux 正常 libc；语言语义不变（checked 溢出所有模式生效，D5） |

## 未实现（稳定诊断层）

| 特性 | 拒收点 | 备注 |
|---|---|---|
| class 继承 / 二级构造（init） | sem | R4：v1 默认 final、无二级构造，稳定拒收 |

## 近期 ABI 落地状态

| 特性 | parser | sem | irc | llvme | runtime | 备注 |
|---|---|---|---|---|---|---|
| lambda 聚合/可空 class 捕获 | ✅ | ✅ | ✅ | ✅ | ✅ | env 按 scalar_slots 展开；ARC 叶借用 retain；llvme env release 递归 class/Fn/nullable/struct/tuple/array；enum ARC payload 拒收 |
| lambda i128 捕获 | ✅ | ✅ | ✅ | ✅ | — | 2 槽 WideCell 标量经 env scalar_slots 布局；与 aggregate capture 同一游标模型 |
| 顶层标量可空全局 | ✅ | ✅ | ✅ | ✅ | — | nullable-abi-audit.md §3.3：`[2 x i64]` flag+payload；`i128?` 使用 wide payload，可空 class 是进程根 |
| 指向可空的类型解引用（`*p` / `*p = v`） | ✅ | ✅ | ✅ | ✅ | — | D-007 类型语法契约保留；pointee 逐槽读写；class/可空 class 走 ARC、i128 走 wide payload（e2e 已钉） |
| lambda-in-lambda 字面嵌套 + 柯里化调用 | ✅ | ✅ | ✅ | ✅ | — | parser body 位 `{` 消歧（深度感知 `->` 前瞻）+ 柯里化 Fn 值链；捕获外层参数逐层正确（e2e 3/7/15） |

## 稳定拒收登记

| 特性 | 拒收点 | 备注 |
|---|---|---|
| lambda 存 struct 字段（矩阵 #5） | sem | 修正 3：Fn 字段稳定拒收（主构造头字段 + body 字段两路已接，declare 侧） |
| union 声明 / accessor（get/set） | sem | P0 trust：此前静默接受，现声明点稳定拒收（D-014 保留字） |
| macro 声明 / comptime / grow / spawn / actor / aspect / provide / async / await / concurrent / detached / thread / fiber / process / variance / const / inline / infix / vararg / namespace / static | parse | D-014：已从词表移除，parse 级拒收（2026-11-25） |
| protected（顶层 + class 成员） | sem | P0 trust：继承未实现，稳定拒收 |
| open/abstract/sealed/override/operator | sem | P0 trust：无实现路径的 modifier，声明点稳定拒收（const/inline/infix/vararg/static 已从词表移除） |
| `as` 非数值/非指针转换（如 string→int） | sem | P0 trust：只允许数值↔数值、同型、Ref/Pointer 互转；其余 SemTypeMismatch |
| impl 头 trait 充当类型实参（`impl T for L<T>`） | sem | P0 trust：lower_ty 拒收（此前静默注册成 spec） |
| ARC aggregate payload | sem/irc/llvme | struct/tuple/array/class 字段与 enum payload 走 type-directed copy/drop；enum Borrowed copy/tag drop 按 active variant；scope/return/release 已有 e2e 回归 |
| variance（类型形参型变标注） | parse | D-014：已从文法移除（parse 级拒收）；未来用 `+`/`-` 标注实现 |
| 宏调用展开 `name!(...)` | sem | `SemMacroExpansion`（宏声明已删，D-014；展开后置） |
| 聚合/宽值顶层全局 | ✅ | 纯值 `[n x i64]`；class 是进程根；i128 wide ABI；bare fn 全局允许；boxed Fn/未支持组合拒收 |
| extern 程序 interp 验证 | interp | `UnknownFun` trap（已知局限） |
| T2 checked pointer | — | 后置 M10（Q7） |
| M10 并发（Mutex/actor、Send/Sync） | — | 模型选择待用户裁定 |

## Conformance（P1-7 骨架）

Feature Group 定义：**FG0** 标量核心（上表第一组）、**FG1** 数据布局、
**FG2** 控制流与模式、**FG3** 运行时契约——仅表分组的命名，
与所有权 tier（ownership T0/T1/T2 = 值 / & &mut / class、*const *mut，
见 language-design D-007 与 M10）正交分离，不混用。conformance 断言：

- FG0：每个标量类型种类经 `parse → sem → irc → interp` 往返保持值
  （`irc/tests/interp_semantics.rs::tier0_scalar_conformance`——测试名
沿用旧称，语义 = FG0 conformance）。
- FG1+：由差分 harness（`llvme/tests/diff_native.rs`）逐层对拍担保
  （interp vs native 逐行一致）；新增类型种类时两处的用例表都要加行。

## 关键字（D-014 冻结，2026-11-25）

v1 预算冻结：**35 hard / 10 modifier / 8 slot = 53**（2026-11-25 删 `static`：顶层 fun/val/var 加包命名空间已覆盖），不再减少。保留待实现的
capability 关键字：`operator`（运算符重载）、
`union`（重叠字段值类型）、`get`/`set`（属性）、`by`（委托，随继承落地）、
`it`/`move`（lambda 推断/捕获）。`it` 与 `move { ... }` 形式已有 e2e；`move` 当前为捕获形式标记，ARC 捕获统一借用 retain。
已实现新增：`alias`（类型别名）、`deinit`（析构）。
