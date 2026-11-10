# closure-env spike（S2/P2-1，2026-11-10）

状态：spike 决策记录（0.6 S2 的 P2-1 出口产物）
前置：0.6 裁定 S2 = boxed env + 值捕获（A′）

## 1. 现状证据（探针实测）

- **Fn 值 = 1 slot**：`val g = add` + 形参传递 + 间接调用全链 e2e
  （m5 探针 42 ✓）；TAC 形态 `param %t0 : fn(i64) -> i64` 单槽；
  llvme 发裸函数指针。
- **无捕获 lambda** = 合成 `__lambda` fun + `Const::Fun(ldef)`，
  与具名函数同一表示（`extra_funs` 追加）。
- **显式参数形态**（spike 发现并修复两个预存缺陷，见 §3）：
  `{ (n) -> … }` / `{ (n: int) -> … }` / `{ (a, b) -> … }` 全部 e2e 通过
  （18 / 7 / 42 / 18,7）。
- 本机尺寸：`fn` = 8B；`Box<dyn Fn>` = 16B（裁决表引用）。

## 2. 决策记录（P2-1 出口门禁三项）

| 项 | 结论 |
|---|---|
| **Fn 槽数** | **保持 1 slot**。boxed env 方案：捕获时堆分配 env box，Fn 值 = box 指针；box 布局 `[fn_ptr, captures…]`，调用经 box[0] 取函数指针。无捕获 lambda 免分配：Fn 值直接 = 裸 fn 指针（现状），调用方按「box? fn？」无法区分——**改为统一 boxed**（无捕获也分配空 box）或「裸 fn 指针 + 低位标记」二选一，**P2-2 spec 定：推荐统一 boxed**（简单、无位 tricks；无捕获 lambda 分配一个 8B box 的成本可接受，逃逸场景本就需要堆）。 |
| **env 布局** | box = header? 不引入——`[fn_ptr, capture_0, capture_1, …]` 连续槽；捕获按源码顺序；class 捕获 = +1 move-in（R3 纪律）；标量 copy。 |
| **C 回调** | 无捕获 lambda/具名函数 = 裸 fn 指针，可直接过 `extern "c"`（现 M6 契约不变）；**有捕获闭包不可作 C 回调**（需要 boxed 调用约定）——负测试在 P2-4 落。 |

## 3. spike 期间发现并修复的预存缺陷（3 个，均已回归测试）

1. **显式 lambda 参数从未绑定**（M8 预存）：`{ (a, b) -> … }` 整体解析为
   单元素元组模式（= 参数表），sem/irc 只认顶层 `Binding` → 参数 undefined。
   修复：`LambdaParams::param_kinds()` 展平助手 + 两消费方接入。
2. **嵌套函数值调用 param 错配**：`f(f(x))` 外层 callee 的 param 先入队，
   被内层 call 的 pending 整队吞掉 → IR 畸形/panic。
   修复：`fn_value_call` 先降全部实参、后压 param。
3. **双 lambda 同程序越界 panic**：`block_index` 是 Ctx 级单例 map，
   `lambda_lower` 换出 blocks/next_block 但不换出索引；第二 lambda 的
   块 id（saved_nb 回退导致复用）撞残留条目 → Vacant 失败不 push →
   空块发射越界。修复：lambda_lower 保存/恢复 `block_index`。

## 4. P2-2 输入（下一步 spec）

- env box 的 IR 形态（alloca + store fn_ptr/captures vs literal struct）
- 捕获矩阵用例表（标量/class/嵌套/逃逸/self）
- 统一 boxed vs 裸+标记的最终定夺（本记录推荐前者）
