# Optimization notes（中间代码优化阶段实现契约）

优化阶段的门禁与决策记录，与 `parser-notes.md`、`semantic-notes.md` 同位。
教科书写法（龙书 ch6/8/9）：中间代码生成产出**三地址码（TAC）**，
优化 pass 在 TAC + 基本块流图上做，最后 TAC 机械翻译成 LLVM IR 文本。
纯 std（LLVM IR 以**文本**发射，不用 inkwell/llvm-sys——Cargo.toml 零依赖
红线不变；`lli`/`llc` 只是外部验证工具，不进 crate）。

## 管线位置

```text
SemanticOutput ─→ [irc crate] 三地址码生成（中间代码生成阶段，另一个会话）
               ─→ TacOutput（基本块 + 三地址指令 + 临时变量）
               ─→ [opt crate] 优化 pass（本阶段）
               ─→ 优化后 TacOutput（同类型，原地变换）
               ─→ [llvme crate] LLVM IR 文本发射
               ─→ *.ll ─→（外部 lli/llc）→ 机器码
```

## 为什么是 TAC 而不是直接优化 LLVM IR

1. 教科书一致性：常量折叠、复制传播、CSE、活性分析都是定义在
   三地址码 + 流图上的（龙书 8.4–9.4），IR gen 按同一抽象出码。
2. 工作量：TAC 是非 SSA 的 flat 指令序列，pass 实现最短；LLVM IR 是
   SSA + 类型化指令，在上面做教科书 pass 要先处理 phi/类型系统。
3. 翻译是近机械的：每条 TAC 对应 1–2 条 LLVM 指令（见下）。

## 表示形态裁决（三轮评审结论，2026-10-22）

- **存储形态 = 四元式**（显式 `dst` + 全局 `TempId`），非三元式/
  间接三元式：v1 pass 集（LICM 外提/DCE/折叠删除）是重排密集操作，
  四元式移动指令零引用修改（龙书 6.1.3 的对比结论）；指令自包含，
  快照/打印逐行可 diff。
- **DAG**：仅作基本块内表达式工具（龙书 6.1.4/8.5），不进 irc——
  局部 CSE 归 opt，单一事实来源；值编号是 opt 扩展位。
- **SSA**：不进 crate（龙书 ch9 之前不做 phi/支配边界；升 SSA 交给
  外部 `opt -mem2reg`）。
- **RPN/后缀式**：排除出主路径（栈式 IR 与基本块控制流互斥），
  仅未来栈机后端的 roadmap 一笔。
- **`UnOp::Conv(TyId)`：确认**（2026-10-22 优化会话裁决）。龙书 6.5
  将类型转换视为一元算子（inttofloat/floattoint），教科书一致；
  备选 `Instr::Conv { dst, a, from, to }` 的字段与 `Un` 完全重复，被否。
  opt 侧影响：CSE/LICM/DCE 走既有 `Un` 臂零新增 match；常量折叠识别
  Conv 随"Conv 常量折叠"扩展位落地（v1 折叠的双常数前置条件对单
  操作数 Conv 本就不成立）；DCE 视 Conv 为纯计算，未使用即删。

## TAC 形状（irc crate 的产出契约）

```rust
pub enum Operand { Temp(TempId), Const(Const) }
pub enum Const { Int(i128), Float(f64), Bool(bool), Null, Str(Symbol), Unit }
// UnOp 带载荷：Conv = 一元算子（龙书 6.5），2026-10-22 优化会话已确认
pub enum UnOp { Neg, Not, BitNot, Conv(TyId /*from*/) }
pub enum Instr {
    Bin { dst: TempId, op: BinOp, l: Operand, r: Operand, ty: TyId },
    Un  { dst: TempId, op: UnOp,  a: Operand,           ty: TyId },
    Copy{ dst: TempId, src: Operand,                    ty: TyId },
    Load{ dst: TempId, base: TempId, idx: Operand,      ty: TyId },  // 槽寻址
    Store{ base: TempId, idx: Operand, src: Operand,    ty: TyId },
    Jmp { block: BlockId },
    Br  { cond: Operand, then_: BlockId, else_: BlockId },
    Param{ a: Operand, ty: TyId },
    Slot { dst: TempId, count: u32 },  // 2026-10-27：聚合基址声明（数组填充的动态槽写入尺寸）
    Call { dst: Option<TempId>, callee: Callee,         ty: TyId },
}
pub enum Callee { Def(DefId), Runtime(RuntimeFn) }  // 2026-10-27 修正案
pub enum RuntimeFn { StrConv /*标量→string，源类型在 Param.ty*/, StrConcat }
pub enum Term { Jmp(BlockId), Br { cond, then_, else_ }, Ret { value, ty }, Unreachable }
pub struct Block { pub id: BlockId, pub instrs: Vec<Instr>, pub term: Term }
pub struct TacOutput { pub funs: Vec<FunBody>, pub diags: Vec<Diagnostic> } // 有 Error 则 opt 不启动
```

### 命名与不变量

- **TempId/BlockId**：per-function 单调递增、分配后永不改名、永不复用、
  空洞合法（DCE 删指令不留名）——pass 间 diff 只显示真实变化；
  Display 层可按引用序重编号（纯展示，不改 IR 内部名）。任何 pass
  不得 renumber。
- **操作数类型均匀不变量**：同一指令的操作数类型一致——sem 提升格 +
  irc 强制插 `Conv` 保证；llvme 依赖此不变量，不做 per-operand 检查。
- 常量类型从所属指令的 `ty` 取（`Const` 不自带类型）；浮点发射用
  十六进制浮点字面量（`0x1.5p+5`）保证位精确往返（llvm-as 层抓违例）。
- **有符号假设**：v1 全部整型按有符号处理，`Conv` 翻译为
  sext/sitofp/fptosi 族；未来无符号类型按 `IntKind` 分派，IR 形状不变。
- 类型注解带在指令上：llvme 不看 `expr_ty` 侧表，只看 TAC 里的 `ty`。

## 数据布局（lowering 与 llvme 共享的表示决策）

- 标量：iN/f64/bool/unit 直接对应 LLVM iN/double/i1。
- **聚合值 = 基址 temp + Load/Store 槽寻址**。
- **和类型（enum/nullable）槽位布局**：
  - 槽 0 = tag，宽度 i8（>255 变体 → `IrcTooManyVariants` 诊断）；
  - 槽 1..k = payload，按变体声明序摊平，每个标量一槽（8 字节），
    非 i64 槽访问由 llvme 发 bitcast（隔离在 llvme 一处）；
  - nullable = 2 变体和类型特例 `{i8 flag, payload}`；
  - llvme 对每个 `Named(enum)` 取最大槽数变体定 LLVM 类型
    `{i8, [k x i64]}`，确定性可复现；
  - struct 按 `sig.fields` 摊平；array = len 个元素槽；tuple 同理。
- **局部表示**：`val`（不可变）→ Temp（跨块引用合法：前驱支配后继，
  无需 phi）；`var`（可变）→ slot + Load/Store；if/when 作表达式
  产生值 → 结果槽（两分支 Store、汇合 Load——用内存绕开 phi，
  与 mem2reg 立场一致）。
- 函数名编码 `name_defid`（防 Symbol 重名导致 LLVM 重定义，确定性）。

## lowering 规则速查（irc v1）

- 短路布尔（`&&`/`||`/`!`）：Br 链物化为 bool temp（龙书 6.6）。
- `when`：主语先求值成 temp（单求值，龙书 6.8）→ 源序逐分支 Br 链
  （模式条件降比较，`pat_res` 给变体序号做 tag 比较；guard 内嵌）→
  值版走结果槽汇合 → 链尾 fallthrough 恒发 `unreachable`（LLVM 块
  必须终结；防 sem 松弛项穿透）。
- `for`：desugar 为下标循环（Array/Range 两种 iterable）。
- `x in a..b` → `x>=a && x<=b`；`x in arr` → 线性扫描循环。
- `defer`：lowering 上下文维护 defer 栈，return/break/continue/块出口
  逆序发射。
- 枚举构造 `Opt.Some(x)`：Store tag + Store payload 槽；`E.A` 无参
  变体只写 tag；`!!` → Br flag，失败走 `unreachable`。
- `StringInterp`：sem 已完整定型（结果 `string`，parts 逐一定型，
  内部错误在 sem 报——不进跳过清单，见 semantic-notes）。irc
  lowering = 逐 part stringify + concat 的运行时操作序列；是否落成
  extern call、运行时入口命名，是 IR 开工时的运行时契约设计。
  part 的可字符串化规则（如 `int?`）依赖 trait 体系，解冻后再回访。
- comptime/unsafe 块按普通块 lowering（与 sem 现有行为一致）。
- 生成期不消重（无 DAG/值编号）：表达式重复原样发射，交给 opt 的
  局部 CSE——单一事实来源。

## 落地边界（irc crate）

**M1（2026-10-23）标量核心**：字面量（全 radix + 值域诊断 + 转义解码）、
标识符、算术·位运算·比较（操作数公共型按提升格统一，`ty` 字段=操作数
型，比较结果型由 llvme 按算子推 i1）、短路布尔（Br 链物化）、一元算子、
`as` 数值转换（`UnOp::Conv`）、具名/命名实参调用 + 默认实参调用点求值、
if 语句与 if 表达式（结果槽绕 phi）、while/loop、break/continue
（flow_res 回指）、return、val→Temp / var→slot / 无初始化→slot
（未定义值读取）、comptime/unsafe 块按普通块。

**M2a（2026-10-26）数据布局**：槽位布局全案（`layout.rs`：标量 1 槽、
struct 按字段序、和类型 tag(i8)+payload、enum payload 取最大变体、
array=len×元素槽、tuple/range 摊平）；struct 构造器调用（位置/命名实参
槽位机）、枚举变体值 `E.A` 与变体构造 `Opt.Some(x)`、数组成员访问
`p.x`（字段槽 Load）、数组字面量/动态索引（idx = i×元素槽数）、
元组字面量、数组填充 `[v; n]`（值一次求值 + 下标循环，体积与 n
无关）、nullable：`null`/`as?`/`as!`/`!!`（flag 槽 + 运行时分支 +
unreachable 兜底）、聚合 var 赋值与读取的逐槽快照拷贝、聚合实参按
基址传递（sem v1 形参不可变，别名安全）、dump 类型名解码（P、T?、
[T; n]）。

实现注记：BlockId 稀疏（merge 块先 reserve 后 switch），块寻址按 id
查表而非下标；dump 按 BlockId 排序输出；字符串字面量解码后 intern 进
`sem.interner`（`lower` 取 `&mut SemanticOutput`）；成员解析 sem v1
不公开字段/变体侧表——irc 经 `layout::resolve_member` 按名本地解析
（命名类型成员名唯一，与 sem member_lookup 等价；别名走
`Def.alias_resolved`）。

**M2b（2026-10-26）控制流与模式**：模式匹配 lowering（`lower_pattern`，
is/when/for/val 条件共用：字面量/布尔/Null/Wildcard/Binding/Constructor
（pat_res 变体序号 + payload 槽绑定）/Struct/元组/数组（无 rest）/
Range；Or 仅无绑定形态，含绑定 Or 与 rest 模式在循环值定型解冻后）；
`is`（绑定限 then 分支作用域，negated 取反）；`when`（主语单求值 →
源序 Br 链，模式/guard 内嵌，值语境结果槽，链尾 unreachable）；`in`
（区间 → 双比较短路，`..` 不含/`..=` 含上界；数组 → 线性扫描循环）；
`for`（数组下标循环；Range 字面量计数循环；物化 Range 值按闭区间——
Range 类型不区分开闭，记为表示层 corner）；`defer`（块作用域栈 +
return/break/continue 出口全量发射，出口后清空防重复；顺带修复 M1
潜在 bug：块尾表达式现于块作用域内求值）；`??`（flag 分支 + 结果槽
汇合）；`?.` 安全调用（标量字段：flag 检查 + nullable 结果传播）；
and/or 操作数常量短路在发射期折叠。

**M3（2026-10-27）运行时契约与赋值补全**：`Callee` 枚举修正案——
调用目标分 `Def(DefId)`（用户函数）与 `Runtime(RuntimeFn)`（运行时
extern 入口：v1 = StrConv/StrConcat，名字由 llvme 映射
`starry_str_conv`/`starry_str_concat` 并 declare；std-only 红线下
运行时以符号存在不进 crate。opt 侧影响：五个 pass 对 Call 均按
"有副作用"处理，callee 形状不可见，零改动成立）。StringInterp
lowering：逐 part StrConv（源类型由前置 Param 的 ty 携带）+ 左结合
StrConcat 折叠；插值裸 chunk 的 span 形态（parser string_chunk 无
引号）在字面量层兼容。赋值补全：复合赋值读-改-写 desugar（十个
复合算子全族）；赋值目标扩展到成员字段（字段槽写入）与数组索引
（slot 寻址）；`literal()` 兼容插值 chunk span。

**仍发 `IrcUnsupportedLowering`**：grow、带值 break（sem 循环值定型
松弛项）、多段路径、元组下标、非简单绑定模式（元组/解构 val）、
含绑定 Or/rest 模式、聚合元素的 for/in/索引/字段赋值、`?.`
与复合基座（成员/索引链）及 nullable 基座的赋值（写穿透的
lvalue 基座解算限 Ident）、nullable/聚合 part 的字符串化、顶层
声明（需全局存储语义——TAC 存储类扩展，归 llvme/runtime 协同的
独立里程碑）。成员/索引赋值的基座经 lvalue_base 直通 Slot/Temp
原存储（var 的 read_place 是快照拷贝，经它写入会丢失——2026-10-27
审查修复，金样断言赋值后读取见新值）。

## 优化 pass（v1，按序）

教科书局部/全局基础集，全部纯函数 `&mut FunBody`：

1. **常量折叠**：Bin/Un 双常数操作数 → 单 Const（含跳转条件定值 →
   无条件化 Br→Jmp）。
2. **复制传播**：`x = y` 后凡 x 的使用点可用 y（局部，同一基本块内）。
3. **死代码消除**：结果不再被使用的纯计算指令删除（无副作用谓词：
   Load/Call/Store 永不删；活性 = 全函数使用集——val temp 跨块引用合法（前驱支配后继），块内扫描会误删只在后继块使用的定义；级联由不动点消化）。
4. **局部 CSE**：基本块内可用表达式表（龙书 8.5），`x op y` 已算过
   则复用旧 Temp。
5. **循环不变代码外提（LICM，选做）**：DFS 找回边定循环，循环内
   操作数均在循环外定值的 Bin/Un 提升到 preheader。

v1 跳过（记入扩展位，不归本阶段）：全局数据流（到达定值/可用表达式
全局版——ch9 落地时 SSA 进 crate）、强度削弱、归纳变量消除、寄存器
分配——机器码层面 LLVM 后端全包。教科书后半程 pass 需要时按同一
`&mut FunBody` 接口加。扩展位另记：跳表 switch、Conv 常量折叠、
值编号/DAG。

## llvme 落地（2026-10-27）

存储模型定案：每个 temp = 入口块恰好一个 alloca（Cell = i64 槽；
Block(n) = [n x i64]；PtrCell = ptr 槽装聚合指针——聚合实参/返回值
按指针，sem v1 形参不可变故别名安全）；槽寻址统一
`getelementptr [n x i64], ptr p, i64 0, i64 i`，tag/flag 是槽 0 里的
i8（写时就地 store i8，免 bitcast——不透明指针风格，LLVM 15+）；
升 SSA 全交外部 `opt -mem2reg`。temp 存储分类两遍法：常量槽访问 /
Slot 声明 / 聚合返回值推断槽数。浮点一律十六进制字面量（f32/f64
各按位模式）；字符串常量 @.str.N private constant + gep 操作数；
运行时入口按调用点参数类型实例化声明（starry_str_conv.i64 等）；
main 包装 = 无参 main 存在时 `define i32 @main` 调它返 0。
`irc::layout` 公开为 LayoutView trait（irc/llvme 共享槽布局的
跨 crate 契约）。可选验收层：llvm-as 在测试侧调用，无工具链即跳过。

## LLVM IR 发射契约（llvme）

```rust
pub fn emit(tac: &TacOutput, tys: &TyArena, defs: &DefArena, interner: &Interner) -> String
```

- 不调用任何 LLVM 库；funs → `define`，Block → 标签，Temp → `%tN`。
- main 包装：`define i32 @main()` 调 Starry `main`，返回 i32 0。
- `Const` → LLVM 字面量；`TyId` → `i32`/`double` 等；槽布局见上。
- **验证 = 可选验收层**：机器上有 LLVM 工具链则 `llvm-as`/`lli` 行为
  对拍、外部 `opt -mem2reg` 升 SSA 后复验；没有则跳过并在测试输出
  明示，不阻塞 fmt/test/clippy 核心门禁。

## 诊断与门禁

- `TacOutput.diags`：M1 实际——sem 干净但 lowering 未覆盖的构造发
  `IrcUnsupportedLowering` 后置诊断（starryc 层见诊断即不打印 TAC、exit 1；
  降级继续产出部分 TAC 以便调试，下游 opt 契约「有 Error 不启动」不变）。
- `Irc*` 码面：`IrcUnsupportedLowering`（M1 里程碑边界）、
  `IrcTooManyVariants`（>255 变体的枚举，sem 已接受但槽位布局无法表示——
  用户可触达，诊断而非 panic）。
- 其余 post-sem 契约破缺 = 内部不变量 → panic/debug_assert（Rust 惯例）。
- 与全仓一致：`cargo fmt --check`、全 workspace 测试、
  `clippy -D warnings`、`syntaxgen --check`、`git diff --check`。
  每个 pass 配最小钉板测试（fold/copy/DCE/CSE 各一正一负）。
