# 闭包捕获规范（S2/P2-2，2026-11-10）

状态：spec（P2-3 实现基准；前置 = closure-env-spike 决策记录 + 0.6 S2）
裁定：boxed env（`Fn` 保持 1 slot）+ 值捕获；不做借用捕获（S2/A′）

## 1. env box = ARC 对象（零新 runtime）

捕获 lambda 的环境盒**复用 M4 ARC 对象模型**：

- 分配：`starry_alloc_object(bytes, deinit)`——header
  （strong/weak/meta）+ payload；
- payload 布局：`[fn_ptr, capture_0, capture_1, …]`（槽 = 8B，同聚合
  槽模型；fn_ptr = 合成 `__lambda.N` 函数的指针）；
- `meta` = 环境释放器：最后 strong 释放时逐捕获 release（class 捕获
  的 +1 在此归还；标量捕获无动作）——与 class `deinit` 同一 meta
  回调机制，runtime 零新增；
- `Fn` 值 = env box 对象指针（1 slot）；无捕获 lambda **统一 boxed**
  （空捕获 box，fn_ptr 槽 0）——消除「裸 fn vs box」双形态
  （closure-env-spike §2 决策）。

## 2. 调用约定

`f(args…)`，f 为 `Fn` 值：

1. `%box = f`（对象指针）；
2. `%fn = load ptr, box.payload[0]`；
3. `call %fn(%box, args…)`——**env box 永远为隐式首参**（与 M4
   `self` 注入同纪律）；无捕获亦传 box（统一签名）。

合成 `__lambda.N` 签名 = `(env: ptr, 捕获形参…, 显式形参…) -> ret`；
lambda 体对捕获名的读取经 env 槽（lowering 在 lambda_lower 内绑定
捕获名 → `box.payload[k]`）。

## 3. 捕获决策（值捕获；按引用类型分类）

lambda 体引用的**外层局部名**（非形参、非全局）：

| 外层绑定类型 | 捕获动作 | lambda 内可见值 |
|---|---|---|
| 标量（int/float/bool/char/string/unit） | copy（按当前值快照） | 快照值 |
| class（含 `?Class`） | retain +1，存对象指针 | 同一对象 |
| 结构性聚合（struct/tuple/array/enum/range/nullable-聚合） | 逐槽 copy 到 box 槽 | 拷贝副本 |
| 顶层 `val` 全局 | **不捕获**——按名直读全局（地址稳定） | 全局当前值 |
| 顶层 `var` 全局 | **稳定拒收**（SemUnsupportedConstruct） | — |
| 外层 lambda 形参 | 同外层局部（标量 copy / class +1） | — |

纪律（S2 裁定）：lambda 内**不可对捕获名赋值**（捕获是快照/共享指针，
赋值既不写回外层也无意义）→ 稳定拒收；v1 不做可变捕获/引用捕获。

## 4. 逃逸规则

- lambda 值（box 指针）可返回、存入 struct 字段/数组/全局 `var`?、
  传参——box 走 ARC copy/scope-release，逃逸天然支持；
- **全局 `var` 存 Fn 值**：v1 拒收（聚合全局已拒；Fn 全局同纪律）；
- 捕获外层 `var` 局部本身合法（快照语义：捕获时刻的值，后续外层
  修改不生效，lambda 内修改被拒）——写进用例表防误解。

## 5. 捕获矩阵用例表（P2-3/P2-4 验收）

| # | 用例 | 期望 |
|---|---|---|
| 1 | 无捕获 `{ it + 1 }` | e2e 不变（统一 boxed 后仍 42/7） |
| 2 | 标量捕获 `val k = 10; { (n: int) -> n + k }` | 值正确 |
| 3 | 多捕获 `{ (n: int) -> n + k1 + k2 }` | 槽序 = 源码序 |
| 4 | class 捕获 + 逃逸返回 | deinit 平衡（box 释放时 -1） |
| 5 | lambda 存 struct 字段再调用 | 逃逸 + ARC 正确 |
| 6 | lambda 返回 lambda（嵌套） | 内层捕获外层形参合法 |
| 7 | 捕获后 lambda 内赋值捕获名 | 稳定拒收 |
| 8 | 捕获外层 `var` 后外层再改 | 快照语义（值不变）e2e |
| 9 | lambda 内读全局 `val` | 允许（直读） |
| 10 | lambda 内读/写全局 `var` | 稳定拒收 |
| 11 | 捕获 class 后 deinit 计数 | 恰好一次释放 |

## 6. P2-3 实现切面（备忘）

- sem：lambda 捕获分析（外层名集合 → 捕获表挂 lambda_defs）；
  违例稳定拒收（§3/§4 的 SemUnsupportedConstruct）；
- irc：lambda_lower 改 boxed：AllocClass(fn_ptr+捕获) + 捕获名绑定
  env 槽；间接调用（fn_value_call）改 box 约定（load fn_ptr + 首参
  env）；`runtime_print`/拷贝路径不受影响（Fn 仍 1 slot Scalar）；
- llvme：`Const::Fun` → box 构造（全局 init 或入口 alloca 处）；
  间接调用 load box[0]；env 首参注入；
- interp：TAC 层新增 box 指令（AllocClosure/CallClosure 或复用
  AllocClass + RuntimeFn）——最小侵入方案 P2-3 定；
- golden/差分：lambda 用例全量入 diff（§5 矩阵）。

## 6b. 切片账（P2-3 分批）

- **P2-3a（已落地，`568d50f`）**：sem 捕获分析基建——捕获表
  （`lambda_caps`，首现序）+ 三类稳定拒收（mutable 全局引用/
  lambda 内捕获名赋值/捕获快照语义）；env box = ARC 对象不变。
- **P2-3b（下一批）**：irc boxed lowering（AllocClosure/
  Callee::Closure——所有 Fn 值统一 closure box，具名函数引用亦装箱）+
  llvme 发射 + interp 注册表（无堆实现）+ 矩阵用例。class 捕获的
  deinit meta 合成随 P2-3b（标量捕获先行时 alloc deinit=null，
  class 捕获暂缓——矩阵 #4/#11 归 P2-3b）。

## 7. 明确不做（S2 非目标）

- 借用捕获/逃逸分析/可变捕获/shared box；
- thick（fn+env 双槽）表示；裸 fn 与 box 双形态；
- C 回调（有捕获闭包不可过 extern "c"——spike 记录）。
