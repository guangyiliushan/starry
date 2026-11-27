# M7 spike：泛型单态化（sem 级特化）

日期：2026-11-08。状态：slice 1 设计定稿，实施中。

## 决策（与 class-arc-spike 同位，供 M7 后续切片追溯）

1. **单态化层级 = sem AST 特化**（Rust monomorphization 思路，rustc-dev-guide
   参考）。泛型 fun 的每个实例化 = 克隆声明 AST + NodeId 重编号 + 走完整
   declare/resolve/check。实例化产物是普通 def：**irc/opt/llvme 零改动**
   （名字 = `名.DefId` 已天然唯一）。
2. **struct 实例化 = def 级特化**（无 AST 克隆）：值类型一律
   `Named(spec_def, [])`，spec def 的 sig.fields/params 是代入后的具体类型，
   member_scope 克隆字段 def（保 M3 可变性三态）。布局/member 解析
   走 sig，零改动。
3. **类型形参 = `DefKind::TypeParam { index }` + `TyKind::Param(u32)`**。
   泛型 def 的签名 eager lower 成 Param 类型；调用点做纯 TyId 结构代入
   （`TyArena::subst`）。
4. **显式类型实参**：`id<int>(1)`、`Box<int>(1)`。类型实参已在 resolve 期
   lower（ty_of_node 缓存）。v1 不做实参推断——泛型体内递归调用须写
   `f<T>(x)`（T 在克隆体里已具体）。
5. **波次不动点**：check 发现并排队实例化 → 一波克隆追加进 items →
   三个 pass 按 `items_done` 偏移增量处理 → 新实例化进下一波，直到空。
   definite/moveck 最后全量跑（克隆是普通 fun；泛型原件跳过）。
6. **泛型原件不检查**（无 bounds 可依；Rust 对泛型体查 bounds，v1 无
   bounds → 只在实例化点检查）。限制写入 semantic-notes。
7. **slice 1 拒收面**：泛型 enum/union/trait/alias 声明保持
   `SemUnsupportedConstruct`；类型形参的 bounds/默认值/variance、
   where 子句拒收；泛型 struct 主构造参数默认值拒收；泛型 struct 模式
   拒收；非泛型调用带类型实参 = `SemTypeArgMismatch`（新码）；
   泛型 `main` 拒收（入口无法实例化）。`Weak<T>`/trait bounds 随 M4/M9。
8. **irc 侧**：`call()` 的 type_args 跳过清单条解冻；泛型 fun 原件 item
   跳过 lowering；def↔item 改走 `decl_def` 侧链（克隆 def 与原件同 span，
   span 反查会撞车）。

## 不变量

- NodeId 全局唯一：克隆经 `renumber_decl` 顺序重编号（基址 =
  `SemState.next_node`），侧表按需 `grow`；debug_assert 仍在。
- 确定性：实例化注册表 `BTreeMap<(DefId, Vec<TyId>), DefId>`；波次按
  发现序追加 items。
- 幂等：实例化去重（同一 (def, args) 只特化一次）；自递归
  `f<T>(x)` 命中已注册实例 → 有界。