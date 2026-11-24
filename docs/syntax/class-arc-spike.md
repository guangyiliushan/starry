# Class / ARC spike（2026-11-07，按 D-004 重写）

状态：**已对齐 D-004**（class = 引用类型 + ARC 默认；arena/region 后置可选）。
本文取代 2026-11 初版 spike——初版的「对象语义三选一」与「倾向 A（按值
class）」结论作废（D-003/D-004 已确认引用语义，不再开放比选）。
现状（2026-11-24 校准）：class/ARC 已按 M4 Plan C 全链落地（构造/字段/
方法/deinit/weak var/可空 class）；本 spike §1 拍板项均已实现。
剩余缺口：ARC 聚合（struct 字段/tuple/array/enum payload）、继承、
二级构造 init、static、protected（P0 起稳定拒收）。

## 1. 已拍板（D-003/D-004，无需重开）

- class = 引用类型；`Point(...)` 分配对象；变量持引用；`?Point` 可空引用。
- struct/enum/union = 值类型；构造语法与 class 相同，**类型种类决定分配/引用**。
- ARC 默认：header（strong/weak/metadata）+ payload；构造 strong=1；复制 retain、
  作用域 release；最后一个 strong 释放时 `deinit` + 释放。
- `Weak<T>`/`weak` 破循环；`deinit` 释放资源（拼写见 §5 待确认项）。
- release 关的是 T2 generation 检查，**不是 ARC**；ARC 始终开启。
- arena/region 仅后续 opt-in（`arena { ... }`），帧/批处理临时数据用。

## 2. 落地顺序（M4 内部）

1. **对象布局**：header + payload 的槽位扩展（当前槽模型无 header 概念，
   `layout.rs` 需 class 种类分支；metadata 含 deinit/size/vtable 指针）。
2. **方法分派**：sem 侧 `x.m(...)` 绑定 (class_def, method_def)；`self` 注入；
   irc 侧 = 首参传对象地址的具名调用（单态静态分派优先，vtable 后补）。
3. **ARC 指令**：irc 增 retain/release/deinit 触发点（构造/复制/存取/返回/
   作用域退出）；`+0/+1` 调用约定；move/borrow 消除留给优化。
4. **`deinit`**：最后 strong 释放时经 metadata 派发。
5. **`weak`**：v1 无泛型，用 `weak var` 拼写（D-008 待确认）；zeroing 语义。
6. **class 相等**：默认引用相等（指针比较）；结构相等后置，写进语义笔记。

## 3. 与现有机制的接口

- moveck 的 `Own<T>` 近似：class 引用天然可复制（retain），`Own` 继续只管
  值类型移动；不冲突。
- 模式匹配 Struct 臂：class 解构跟随字段可变性规则（v1 无访问控制）。
- FFI：class 引用不直接过 C ABI（M6）；C 侧只见到 thin 指针或句柄。

## 4. ARC 代价承认（D-004 要求记录在案）

header 16–24B/对象、retain/release 开销、跨线程 atomic 竞争、循环需 weak、
release 不可关 ARC、热路径短命 class 不友好。缓解：struct 承担热数据、
move/borrow 消除、非逃逸栈提升（后置）、v1 线程本地非原子。

## 4.5 spike 基准结论（2026-11-08，drafts/m4-arc-bench-results.md）

5 负载（交错跑 + 逐对差值中位数，clang -O2）：ARC delta over malloc——
链表 19%、二叉树 7%、**rope 58%**、消息环 15%（class/struct 0.99x）；
weak 环 ~36ns/op。形式命中阈值 A/B → 触发复审；建议继续 M4（裁定待用户）。

spike 固化进 M4 契约的四条：

- **weak 绝不延长生命周期**——只破循环（D-004 形态：strong 表持有 + weak 环）。
  语言文档必须写明：strong=0 后 upgrade 按零化语义拒访问。
- **release 消除**：构造期 `x = retain(n); release(n)` 对在 move 语义下
  不需要——M4 优化项，WL1 一半 release 属此类，有量化收益预期。
- **分配单点**：混用分配器（payload 指针当 header 释放）立即堆损坏——
  正式实现 alloc/dealloc 必须单点进出。
- **验收基准**：rope 形态与消息环形态进 M4 回归基准，ARC delta 占比
  列验收项（阈值参照 spike 数字）。

## 5. 待确认（不进 M4 前不给实现承诺）

- D-003：继承默认 final（建议 Kotlin 式，`open` 才可继承）。
- D-008：`deinit`/`weak` 词表改动（硬 keyword vs modifier word）。
- D-012：v1 线程模型（线程本地非原子 vs atomic）。
- D-002：class 二级构造（`init` 块与主构造的相对顺序）。
- class 相等：结构相等是否提供 `==` 重载入口。