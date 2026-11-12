# Nullable ABI 审计（S2/P2-4 前置切片，2026-11-12）

状态：审计报告（可空捕获已落地；本文档盘点可空值的全部 ABI 流向，
登记未覆盖面，为后续切片提供依据）。

## 1. 可空值的两种形态与 ABI 判别

| 形态 | 判别 | ABI |
|---|---|---|
| 可空 class（`P?`） | `class_of` = Agg | 块指针：参数/返回/实参按 `ptr` 传递 2 槽块（flag i8 槽 0 + payload ptr 槽 1）；间接调用返回经 `emit_agg_result_copy` 槽拷贝（2026-11-12 修复） |
| 可空标量（`int?` 等） | `class_of` = Agg（同走块路径） | 2 槽（flag i8 + 标量 payload）；局部按 slot_ty 逐槽 ✓ |

`scalar_ty` **无 Nullable 臂** → 回落 `"i64"`：仅可在「整槽视图」语境
出现（flag 槽的 i64 视图——小端低字节 = flag ✓ 既有约定）；出现在
payload/多槽语境 = 路由 bug。

## 2. 已验证闭环（本切片前已落地）

- 局部绑定/形参/返回/实参：块路径（class_of Agg）✓
- `?.`/`??`/`!!`/`== null`：flag 分支 + payload 逐槽 ✓
- weak upgrade 读取：物化 `?P` +1 纪律 ✓
- 间接调用返回：`emit_agg_result_copy` ✓（2026-11-12 修复——此前把
  callee 栈地址当值存，可空捕获探针抓到）
- 闭包捕获可空 class：2 槽 env + flag 分支 retain/release ✓

## 3. 审计发现的未覆盖面（登记）

1. **顶层标量可空全局**（`val x: int? = 5`）：全局存储 =
   `scalar_ty(int?) = "i64"` 单槽 → payload 槽缺失（读 payload =
   越界/垃圾）。局部的 2 槽块模型未延伸到全局存储。v1 稳定拒收或
   全局 2 槽化，需切片。
2. **指向可空的指针**（`*const int?`）：LoadPtr/StorePtr 的 dst 分类
   按 Cell（i64）→ payload 槽缺失。T2 边缘 + 可空组合，v1 未支持
   语法面（parser 可拒绝），登记。
3. **scalar_ty 显式 Nullable 臂**：防御性——当前回落 "i64" 依赖
   「flag 槽 i64 视图」约定；建议后续加显式臂（`Nullable(scalar) =>
   "i64"` 带注释）或改 panic，防新调用点误用。

## 4. 逆向引用

- 可空捕获实现：closure-capture-spec.md（批次 3）
- emit_agg_result_copy：llvme/src/emit.rs（间接调用 Agg 返回）
- flag 槽 i64 视图约定：interp decode / slot_ty(Nullable, 0) = i8
