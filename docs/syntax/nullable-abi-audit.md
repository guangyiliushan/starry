# Nullable ABI 审计（S2/P2-4 前置切片，2026-11-12）

状态：实现审计（2026-11-18）。可空值槽模型已延伸到纯值全局与指针 pointee；class/可空 class 与 i128 继续走专用 ABI 门禁。

## 1. 可空值的两种形态与 ABI 判别

| 形态 | 判别 | ABI |
|---|---|---|
| 可空 class（`P?`） | `class_of` = Agg | 块指针：参数/返回/实参按 `ptr` 传递 2 槽块（flag i8 槽 0 + payload ptr 槽 1）；间接调用返回经 `emit_agg_result_copy` 槽拷贝（2026-11-12 修复） |
| 可空标量（`int?` 等） | `class_of` = Agg（同走块路径） | 2 槽（flag i8 + 标量 payload）；局部按 slot_ty 逐槽 ✓ |

`scalar_ty` 已有显式 `Nullable => "i64"` 臂：仅可在「整槽视图」语境
出现（flag 槽的 i64 视图——小端低字节 = flag）；出现在 payload/多槽
语境仍视为路由 bug。多槽语境一律走 `scalar_slots` / `slot_ty`。

## 2. 已验证闭环（本切片前已落地）

- 局部绑定/形参/返回/实参：块路径（class_of Agg）✓
- `?.`/`??`/`!!`/`== null`：flag 分支 + payload 逐槽 ✓
- weak upgrade 读取：物化 `?P` +1 纪律 ✓
- 间接调用返回：`emit_agg_result_copy` ✓（2026-11-12 修复——此前把
  callee 栈地址当值存，可空捕获探针抓到）
- 闭包捕获可空 class：2 槽 env + flag 分支 retain/release ✓

## 3. 当前 ABI 覆盖与稳定门禁

1. **纯值顶层全局**：全局 LLVM 存储按 `scalar_slots` 展开为
   `[n x i64]`；`LoadGlobal` / `StoreGlobal` 带 slot。聚合初始化函数
   直接逐槽写全局并 `ret void`，不返回 callee 栈基址。
2. **顶层标量可空全局**：走 `[2 x i64]` flag+payload；读侧物化两槽，
   写侧逐槽 store。`i128?` 与可空 class 仍拒收。
3. **指向可空的指针**：pointee 按 `slot_ty` 逐槽 `LoadPtr` / `StorePtr`；
   slot > 0 经 8-byte `OffsetPtr` 寻址。class payload 与 i128 payload
   拒收。
4. **`value_abi_unsafe` 门禁**：class、可空 class、Fn、以及任意递归包含
   i128 的值必须继续拒收，并给稳定诊断；这些类型需要字段级 ARC 或
   专用 wide-slot ABI，不能落入 i64 槽拷贝。

## 4. 剩余验证面

- Fn var × 可空 class 捕获需 targeted native e2e + ASan。
- ASan harness 未接入。
- 超大栈帧：`__chkstk` 逐页降序探测 + 8MB 预留已落地；
  1.1MB/1.2MB 边界 regression 见 build_cli（p0_stack_probe_*）。

## 5. 逆向引用

- 可空捕获实现：closure-capture-spec.md（批次 3）
- emit_agg_result_copy：llvme/src/emit.rs（间接调用 Agg 返回）
- flag 槽 i64 视图约定：interp decode / slot_ty(Nullable, 0) = i8
