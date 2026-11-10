# i128 聚合布局规范（S3/P3-1，2026-11-10）

状态：布局 spec（0.6 S3 = Q5 2×i64 的实施基准；P3-2/P3-3 的对照表）
原则（0.3.3）：先布局/ABI，后语义；变更波及 IRC 逻辑槽、llvme 发射、
interp 对齐与 golden——以本文为准，实现不得偏离。

## 1. 不变项

- **标量 i128**：LLVM `i128`（alloca/param/ret 单值），不受本规范影响。
- **class 字段**：继续稳定拒收 i128（R4/M4 契约）。
- **i128 标量 FFI**：继续拒收（C ABI 未承诺）。
- interp 槽单元：u128（f988bd3 起）。

## 2. 聚合内 i128 = 2 逻辑槽（lo/hi）

`scalar_slots(I128)` 在**聚合摊平**语境 = 2（lo 在前、hi 在后）；
标量局部temps 语境保持 1 槽（`Slot { count }` 的标量路径不放大）。

各 kind 的槽账（`S(x)` = scalar_slots）：

| kind | 槽数 | 槽类型序 |
|---|---|---|
| struct | Σ S(field) | 逐字段；i128 字段 = [lo i64, hi i64] |
| tuple | Σ S(elem) | 同 struct |
| array `[T; n]` | S(T)·n | 元素逐个；i128 元素 = 2 槽 |
| enum | 1 + max Σ S(variant params) | tag i8 槽 + 最大变体 payload；i128 形参 = 2 槽 |
| range | 2·S(inner) + 1 | lo/hi 各 S(inner) + 开闭 i8 槽；inner=i128 → 各 2 槽 |
| nullable | 1 + S(inner) | flag i8 槽 + payload；inner=i128 → payload 2 槽 |
| class | 1（对象指针标量化） | 不变 |

## 3. 槽值语义（interp 对齐）

- i128 成员的**两槽都存全值**（cell_lo = cell_hi = 128 位全值，冗余但
  自洽）；Load/Stock 按成员整体读写（读槽 0 即全值）。
- llvme 是唯一做 **split/join** 的边界：`[2 x i64]` 视图下
  slot_lo = trunc(v)、slot_hi = lshr(v, 64)；反向 join = zext | shl。

## 4. llvme 发射

- 含 i128 成员的聚合 LLVM 类型：`[Σ(成员宽) x i64]`（i128 成员计 2），
  不再使用 `[k x i128]`；`contains_i128` 特例收敛到「判断聚合是否含
  i128」一处（决定 [2 x i64] 拆分），Wide 三变体删除。
- 标量 i128 的 alloca/param/ret 仍 `i128`。
- 数组/元组/枚举/nullable/range 的 Load/Store/按槽访问全部经
  split-join；无标量 i128 直穿聚合的路径。

## 5. 验收（P3-1..3）

1. 槽账断言测试：上表每个 kind 的 scalar_slots/slot_ty/field_slot
   （target 态，随 P3-2 落地转绿）。
2. IR verifier + golden：含 i128 聚合程序发射 `[Σ x i64]`。
3. diff_native：i128 聚合构造/字段读写/数组/嵌套/负数/min/max 入选。
4. 内存/性能（P3-4）：**已记录（2026-11-10，见 §7）——微基准触发反转条件，待用户复审。**

## 6. 明确不做

- C ABI 布局承诺（offsetof/sizeof 对拍推迟到 repr(C) 专项）。
- i128 class 字段、i128 标量 FFI。
- 32 位目标（x64 主张不变）。

## 7. P3-4 读数（2026-11-10，x64 Windows，--release clang -O2）

### 7.1 内存（IR alloca 尺寸）

| 类型 | 旧（[k x i128]） | 新（[M x i64]） | Δ |
|---|---|---|---|
| struct Dense{ i128, i128, int } | [3 x i128] = 48B | [5 x i64] = 40B | **−17%** |
| [Dense; 5000] | 240KB | 200KB | **−17%** |
| [i128; n]（纯） | 16B/元素 | 2×8 = 16B/元素 | 0 |

### 7.2 运行时（i128-dense 微基准：[Dense;100] × 2000 轮 fill+sum，
400K 次成员访问；min of 5）

| 变体 | min | Δ |
|---|---|---|---|
| i64 对照（无 i128） | 18ms | — |
| i128（S3 2×i64 split/join） | 24ms | **+33.9%（触发 >10% 门）** |

负载构成：几乎 100% 为聚合成员访问 + 每轮 5 槽 Copy ×2（fill+sum 各一遍）
——指令数放大（5 槽 vs 旧 3 宽槽）主导，非算术 split 开销（裁决表实测
split 0.72ns < wide 1.04ns）。标量 i128 算术不在 S3 触及范围（仍单槽 i128）。

### 7.3 反转条件裁定（待用户）

- 微基准触发 P3 反转条件（>10%）。选项：
  1. **维持 A（2×i64）**：真实程序 i128-dense 占比远低于微基准；接受聚合
     访问的常数开销，换 −17% 内存与统一槽模型；
  2. **复审 B（宽槽）**：恢复 [k x i128]（i128 成员单槽）；放弃 −17% 内存
     与槽统一，保留 i128-dense 访问速度；
  3. **A + 优化**：聚合 Copy 改按字节 memcpy（含 i128 聚合消除逐槽指令膨胀）
     ——预期吃掉大部分 +33.9%（Copy 体积主导），保留 A 的一切优点。
- 推荐 **3**：成本一个 memcpy 化的 Copy 路径（中等），两全。待批。