# Feature matrix（逐层实现状态）

每个语言特性在编译链五层的状态：parser → sem → irc → llvme → runtime
（外加 irc 参考解释器作语义验证层）。状态记号：**✅** 完成、**🟡** 部分
（限制见备注）、**❌** 未实现（sem 给稳定 `SemUnsupportedConstruct` 或
irc 给 `IrcUnsupportedLowering`，见 semantic-notes 跳过清单）。

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
| 函数值类型 `(int, int) -> int` | ✅ | ✅ | ✅ | ✅ | — | M8 slice 1：具名函数作值（传递/绑定/返回）+ 经形参间接调用全链路 |

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
| 顶层标量全局 | ✅ | ✅ | ✅ | ✅ | — | 依赖序初始化；聚合全局 ❌（稳定诊断） |
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
| main 返回值=退出码 | ✅ | ✅ | ✅ | ✅ | — | int trunc/sext、bool zext |
| 模块系统（M5 slice 2a–3） | ✅ | ✅ | ✅ | ✅ | — | package 段 + 目录默认 + 包命名空间 + `import a.b`/`import util` 限定访问 + 可见性（默认 public/internal 包私有/private 文件私有/protected 拒收）；循环/自导入合法 |

## 未实现（稳定诊断层）

| 特性 | 拒收点 | 备注 |
|---|---|---|
| class/ARC（M4 切片 1–4b，Plan C 所有权闭环） | ✅ | 构造/字段/方法/deinit/weak var + upgrade 读取（?P）/可空 class/形参+0/返回+1 统一/重赋值所有权/语句级 +1 临时释放（`stmt_temps`，full-expression 纪律）；继承、二级构造后置 || trait/impl/aspect/provide | sem | 跳过清单 |
| 泛型 bounds/variance/where、泛型 enum | sem | bounds 依赖 trait（M9 前）；泛型 enum 后置。已落地：fun/struct 单态化、显式/推断类型实参、类型形参默认值 |
| lambda 定型（捕获语义待裁定）、spawn/async/await、宏展开、grow | sem | 跳过清单 |
| 指针 `*const T` / `*mut T`（T2 thin） | ✅ | D-007 / M2 已落地：类型+FFI+`&x`/`*p`+StorePtr；T2 checked pointer 后置 M10（Q7） |
| 引用 `&T` / `&mut T`（T1） | 🟡 | D-007 / M2：类型 + `&x`/`*p` 落地；v1 保守借用（R6：局部名/不逃逸/全局拒借/不可变只读）；重叠 &mut/区间追踪后置 M10 前专项 |
| 聚合顶层全局 | irc | `IrcUnsupportedLowering` |
| extern 程序 interp 验证 | interp | `UnknownFun` trap（已知局限） |
| i128 位宽语义 | interp | 按 64 位建模（已知局限） |

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