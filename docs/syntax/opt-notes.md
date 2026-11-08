# Optimization notes（中间代码优化阶段实现契约）

优化阶段的门禁与决策记录，与 `parser-notes.md`、`semantic-notes.md` 同位。
教科书写法（龙书 ch6/8/9）：中间代码生成产出**三地址码（TAC）**，
优化 pass 在 TAC + 基本块流图上做，最后 TAC 机械翻译成 LLVM IR 文本。
纯 std（LLVM IR 以**文本**发射，不用 inkwell/llvm-sys——Cargo.toml 零依赖
红线不变；`lli`/`llc` 只是外部验证工具，不进 crate）。

## M1（Q5）受检算术的折叠纪律

- 受检 `Add/Sub/Mul` 只在「位宽上不溢出」时折叠：`checked_*` 抓 w=128 的
  i128 真溢出，`sx` 回环抓窄位宽；溢出一律不折叠（留给运行期 trap），
  **不折成回绕值**（会改变语义）。
- `WrappingAdd/Sub/Mul`（`+%`、`-%`、`*%`）恒按回绕折叠。
- `Div/Rem` 守卫扩到 INT_MIN ÷ -1（poison），除零守卫不变。
- llvme：整型加减乘发 `llvm.s{add,sub,mul}.with.overflow.<ty>` + 溢出分支
  （`starry_overflow_panic` + unreachable + 续标签；槽式存储无 phi，无需修补）；
  除法先守卫后 `sdiv/srem`。

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
    LoadGlobal{ dst: TempId, global: DefId, ty: TyId },  // top-level global read
    StoreGlobal{ global: DefId, src: Operand, ty: TyId }, // top-level global write
    Jmp { block: BlockId },
    Br  { cond: Operand, then_: BlockId, else_: BlockId },
    Param{ a: Operand, ty: TyId },
    Slot { dst: TempId, count: u32 },  // 2026-10-27：聚合基址声明（数组填充的动态槽写入尺寸）
    Call { dst: Option<TempId>, callee: Callee,         ty: TyId },
}
pub enum Callee { Def(DefId), Extern(DefId), Runtime(RuntimeFn) }  // 2026-10-27 修正案；Extern(DefId) = extern "c"（2026-11）
pub enum RuntimeFn { StrConv /*标量→string，源类型在 Param.ty*/, StrConvChar /*char→string*/, StrConcat, StrEq /*(string,string)→bool 内容比较*/, Print /*标量→stdout*/, PrintChar /*char→stdout*/ }
pub enum Term { Jmp(BlockId), Br { cond, then_, else_ }, Ret { value, ty }, Unreachable }
pub struct Block { pub id: BlockId, pub instrs: Vec<Instr>, pub term: Term }
pub struct TacOutput { funs, globals: Vec<GlobalBody>, diags } // opt gates on diagnostics
```

### 命名与不变量

- **TempId/BlockId**：per-function 单调递增、分配后永不改名、永不复用、
  空洞合法（DCE 删指令不留名）——pass 间 diff 只显示真实变化；
  Display 层可按引用序重编号（纯展示，不改 IR 内部名）。任何 pass
  不得 renumber。
- **操作数类型均匀不变量**：同一指令的操作数类型一致——sem 提升格 +
  irc 强制插 `Conv` 保证；llvme 依赖此不变量，不做 per-operand 检查。
- 常量类型从所属指令的 `ty` 取（`Const` 不自带类型）；浮点发射用
  LLVM 的**位模式十六进制**字面量（float/double 均为 `0x` + 16 位模式文本）
  保证位精确往返（validator 层抓违例）。不是 C99 的 `0x1.5p+5`——
  那是 LLVM 拒收的语法（见「后端正确性修正案」）。
- **有符号假设**：v1 全部整型按有符号处理，`Conv` 翻译为
  sext/sitofp/fptosi 族；未来无符号类型按 `IntKind` 分派，IR 形状不变。
- 类型注解带在指令上：llvme 不看 `expr_ty` 侧表，只看 TAC 里的 `ty`。

## 数据布局（lowering 与 llvme 共享的表示决策）

- 标量：iN/f64/bool/unit/null 直接对应 LLVM iN/double/i1（null 常量按 i64 兜底）。
- **聚合值 = 基址 temp + Load/Store 槽寻址**。
- **基址 vs 值的判据是类型种类，不是槽数**（2026-11-04 修正案，见下）：
  `irc::layout::is_aggregate` 与 llvme 的 `TyClass` 同口径——
  int/float/bool/char/string/unit/null 按值，其余（struct/enum/array/
  tuple/range/nullable/Named）按基址。`struct P { x: int }` 与 `int`
  同为 1 槽，但前者是基址、后者是值。
- **和类型（enum/nullable）槽位布局**：
  - 槽 0 = tag，宽度 i8（>255 变体 → `IrcTooManyVariants` 诊断）；
  - 槽 1..k = payload，按变体声明序摊平，每个标量一槽（**默认槽步长
    8 字节**；聚合含 i128 成员时该块整体加宽到 16 字节/槽，
    标量按自身宽度写，读按槽类型）；
  - nullable = 2 变体和类型特例 `{i8 flag, payload}`；
  - llvme 对每个 `Named(enum)` 取最大槽数变体定 LLVM 类型
    （含 i128 时 `{i8, [k x i128]}`，否则 `{i8, [k x i64]}`），
    确定性可复现；
  - struct 按 `sig.fields` 摊平；array = len 个元素槽；tuple 同理；
  - range = lo、hi、i8 开闭标记（`..`=0，`..=`=1）三槽。
- **局部表示**：`val`（不可变）→ Temp（跨块引用合法：前驱支配后继，
  无需 phi）；`var`（可变）→ slot + Load/Store；if/when 作表达式
  产生值 → 结果槽（两分支 Store、汇合 Load——用内存绕开 phi，
  与 mem2reg 立场一致）。
- 函数名编码 `name_defid`（防 Symbol 重名导致 LLVM 重定义，确定性）。

### 后端正确性修正案（2026-11-04）

三处都是「金样测试全绿但产物非法」——验收层当时只比对文本，没跑过真
LLVM。修法各记一条，防回退：

- **基址/值判据按类型种类，不按槽数**。`read_place` / `read_slots` /
  `copy_slots` / 赋值与结果槽汇合原先用 `scalar_slots(ty) > 1` 判「聚合」，
  于是 `struct P { x: int }`（1 槽）被折成标量值：`var p = P(1)` 发出
  `store %t1[0] = %t0 : P`（把基址当 i64 值存），`p.x` 又把读出的标量当
  基址做 GEP。现统一走 `irc::layout::is_aggregate`（与 llvme `TyClass`
  同口径）：聚合含单槽一律「基址 + 槽」，Store/Load 的 `ty` 是**槽类型**。
  `copy_slots` 的单槽快路径对聚合源先取槽值，标量/常量才直传。
- **浮点常量发位模式十六进制**，不是 C99 十六进制浮点。`float_lit` 曾发
  `0x1.8p0`，LLVM 22 直接 `expected instruction opcode`；正确形式是
  `0x3FF8000000000000` 这类 64 位模式文本；clang 22 拒收 8 位 f32 写法。NaN/Inf
  分支本就是位模式，只有常规值路径错。
- **验收层必须真跑**。`llvm-as` 缺席时回落 `clang -x ir -c`，两者都找不到
  才 `SKIP:` 显式打印；`build_cli` 同纪律。跳过即静默变绿，正是上述两处
  能长期存活的原因。

## lowering 规则速查（irc v1）

- 短路布尔（`&&`/`||`/`!`）：Br 链物化为 bool temp（龙书 6.6）。
- `when`：主语先求值成 temp（单求值，龙书 6.8）→ 源序逐分支 Br 链
  （模式条件降比较，`pat_res` 给变体序号做 tag 比较；guard 内嵌）→
  值版走结果槽汇合 → 链尾 fallthrough 恒发 `unreachable`（LLVM 块
  必须终结；防 sem 松弛项穿透）。
- `for`：desugar 为下标循环（Array/Range 两种 iterable）。
- `x in a..b` → `x>=a && x<b`（`..=` 用 `<=`）；`x in arr` → 线性扫描循环。
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
（pat_res 变体序号 + payload 槽绑定）/Struct/元组/数组（含
rest 槽位映射，rest 不产生绑定）/Range；Or 支持共享绑定（每替代
一个分支，命中后写共享绑定槽，汇合再读出）；
`is`（绑定限 then 分支作用域，negated 取反）；`when`（主语单求值 →
源序 Br 链，模式/guard 内嵌，值语境结果槽，链尾 unreachable）；`in`
（区间 → 双比较短路，`..` 不含/`..=` 含上界；数组 → 线性扫描循环）；
`for`（数组下标循环；Range 字面量计数循环；物化 Range 值按开闭标记槽选择 `<`/`<=`（2026-11 起第 3 槽 i8 标记）——
原「Range 类型不区分开闭」corner 已闭合）；`defer`（块作用域栈 +
return/break/continue 出口全量发射，出口后清空防重复；顺带修复 M1
潜在 bug：块尾表达式现于块作用域内求值）；`??`（flag 分支 + 结果槽
汇合）；`?.` 安全调用（标量字段：flag 检查 + nullable 结果传播）；
and/or 操作数常量短路在发射期折叠。

**M3（2026-10-27）运行时契约与赋值补全**：`Callee` 枚举修正案——
调用目标分 `Def(DefId)`（用户函数）与 `Runtime(RuntimeFn)`（运行时
extern 入口：v1 = StrConv/StrConvChar/StrConcat/StrEq/Print/PrintChar，名字由 llvme 映射
`starry_str_conv_*`/`starry_str_conv_char`/`starry_str_concat`/`starry_str_eq`/`starry_print_*`/`starry_print_char` 并 declare；std-only 红线下
运行时以符号存在不进 crate。opt 侧影响：五个 pass 对 Call 均按
"有副作用"处理，callee 形状不可见，零改动成立）。StringInterp
lowering：逐 part StrConv（源类型由前置 Param 的 ty 携带）+ 左结合
StrConcat 折叠；插值裸 chunk 的 span 形态（parser string_chunk 无
引号）在字面量层兼容。赋值补全：复合赋值读-改-写 desugar（十个
复合算子全族）；赋值目标扩展到成员字段（字段槽写入）、数组动态索引与
常量元组下标（parser 记录 const_index；动态数组下标按 i × 元素槽数）；`literal()`
兼容插值 chunk span。
String `+` shares the StrConcat runtime with interpolation; scalar top-level globals use LoadGlobal/StoreGlobal plus a synthetic GlobalBody.init function called before main.

**仍发 `IrcUnsupportedLowering`**：grow、复合基座（成员/索引链）及
nullable/`?.` 基座的赋值（写穿透的 lvalue 基座解算限 Ident）、
nullable/aggregate stringification, aggregate/destructured top-level globals (scalar top-level globals are implemented).
成员/索引赋值的基座经 lvalue_base 直通 Slot/Temp
原存储（var 的 read_place 是快照拷贝，经它写入会丢失——2026-10-27
审查修复，金样断言赋值后读取见新值）。

- **M2 指针指令（AddrOf/LoadPtr/StorePtr）**：AddrOf 纯地址计算（DCE 可删）；
  LoadPtr/StorePtr 与 Load/Store 同副作用契约（永不删）。**DCE 使用扫描
  契约**：三者都读 ptr/base 槽——漏标会把 AddrOf 误判死代码删掉，
  下游 loadptr 拿到未初始化指针（M2 实测抓到：native 挂起）。
## 优化 pass（v1，按序）

教科书局部/全局基础集，全部纯函数 `&mut FunBody`：

1. **常量折叠**：Bin/Un 双常数操作数 → 单 Const（含跳转条件定值 →
   无条件化 Br→Jmp）。**折叠在指令 `ty` 的位宽上算**（2026-10-31
   修正案，见下「常量折叠位宽修正案」）：整型按 N 位补码规约后运算，
   移位量判据 `0..N`、除零判规约后的除数；f32 逐次舍入到单精度。
2. **复制传播**：`x = y` 后凡 x 的使用点可用 y（局部，同一基本块内）。
3. **死代码消除**：结果不再被使用的纯计算指令删除（无副作用谓词：
   Load/Call/Store 永不删；活性 = 全函数使用集——val temp 跨块引用合法（前驱支配后继），块内扫描会误删只在后继块使用的定义；级联由不动点消化）。
4. **局部 CSE**：基本块内可用表达式表（龙书 8.5），`x op y` 已算过
   则复用旧 Temp。
5. **循环不变代码外提（LICM，2026-10-30 落地）**：支配集找
   回边定自然循环（preheader = header 唯一循环外前驱且无条件进
   header，否则零次执行循环会发明计算）；无 trap/UB 族 Bin/Un
   （Div/Rem/Shl/Shr 除外——除零/越界移位语义）且操作数循环外定值（多定义 phi 汇合物化 temp 除外——irc 短路布尔/in 扫描产物，值随路径而变）
   → 提升到 preheader。

v1 跳过（记入扩展位，不归本阶段）：全局数据流（到达定值/可用表达式
全局版——ch9 落地时 SSA 进 crate）、强度削弱、归纳变量消除、寄存器
分配——机器码层面 LLVM 后端全包。教科书后半程 pass 需要时按同一
`&mut FunBody` 接口加。扩展位另记：跳表 switch、Conv 常量折叠、
值编号/DAG。

### 常量折叠位宽修正案（2026-10-31）

折叠早期在 i128 上做、忽略指令 `ty` 的位宽；未优化代码由 llvme 发成
`ty` 位宽的 LLVM 指令、按 N 位回绕，两者对**非模运算**给出不同值——
`(2000000000 + 2000000000) / 2 : i32` 曾折成 2000000000，未优化算
-147483648（静默错编）。加/减/乘是模运算，末位截断后位正确；比较则
必须比较**规约后**的操作数。

- 折叠语义域 `Domain` 由 `TyArena::kind` 推：`Int(N)`/`F32`/`F64`/
  `Bool`；聚合/字符串/unit 无算术域不折叠；char 按 llvme 的 i32 同位宽。
- 整型先按 N 位补码规约操作数（`sx`），结果再规约回 N 位 → 折叠产物
  必落在 `ty` 值域内，发射端不再出现超位宽字面量。
- 移位量判据 `0..N`（原为 i128 的 `0..128`）；除零判据用规约后的除数
  （i8 域 `x / 256` 的除数规约后是 0，未优化代码同样除零）。
- `optimize(tac: &mut TacOutput, tys: &TyArena)`：`tys` 只服务折叠取
  位宽，其余 pass 不看类型。
- 测试：`opt` 单元钉板 9 条（窄位宽整型与 f32 在 surface 语法里做不出
  来——字面量默认 i32/f64 且提升格只加宽不收窄，故直接钉
  `fold_bin`/`fold_un`/`sx`/`domain_of`）+ 全链快照 5 条（位宽回绕、
  回绕后除法、回绕后比较、越界移位不折叠、i64 域正常折叠不退化）
  + 解释器差分对拍 6 条（见下）。
- **遗留**：字面量本身的值域与默认 i32 型在 sem 不核查（semantic-notes
  未来条款），超域字面量仍以 i128 值进 TAC；折叠已按位宽规约，但
  llvme 整型常量发射（直接印 i128）与之独立，未在本次改动内统一。

### 解释器位宽模型（2026-10-31，与折叠修正同批）

参考解释器早先「int 一律按 i64」的扁平模型（模块注释曾记为「验控制流
与数据流，不验位宽」）已反转：整型值在槽里存 **N 位补码符号扩展**形式，
算术在 N 位上回绕、移位量判据 `0..N`、conv 按源/目标位宽 sext/trunc、
浮点按 `ty` 精度逐次舍入——与 llvme 发出的 N 位 LLVM 指令逐位一致。
扁平模型会让解释器对窄位宽断言出真实代码给不出的值，也就当不了位宽类
错编的语义基准。

- LLVM poison 一律表现为 trap（越界移位、fptosi 越界/NaN），不发明值，
  与 opt「不折叠越界移位/除零」同一立场。
- 已知局限：槽是 u64，`i128` 按 64 位建模。
- 已知局限：`extern "c"` 调用的程序无法经参考解释器验证——`Callee::Extern` 一律 `UnknownFun` trap（外部符号只存在于 native 链接）。
- **差分验证层就此可用**：`opt` 侧 `opt_preserves_semantics_*`（6 条）
  优化前后各跑一遍解释器、断言逐值相等。回归验证过：把折叠改回 i128
  语义，`fold_wrap`/`fold_compare` 两条立刻红（快照测试看不出异常）。
- 注意：解释器会在使用点对常量再规约一次，所以「单个超域常量」在对拍
  里看不出来，必须是折叠链算错才露——这层自愈本身是好事，但意味着
  对拍要覆盖链式折叠才有效。

## llvme 落地（2026-10-27）

存储模型定案：每个 temp = 入口块恰好一个 alloca（Cell = i64 槽，
i128 用 16 字节 WideCell；Block(n) = [n x i64]，含 i128 成员时
WideBlock = [n x i128]；PtrCell = ptr 槽装聚合指针——聚合实参/返回值
按指针，sem v1 形参不可变故别名安全）；槽寻址统一
`getelementptr [n x E], ptr p, i64 0, i64 i`（E = i64/i128），tag/flag 是槽 0 里的
i8（写时就地 store i8，免 bitcast——不透明指针风格，LLVM 15+）；
升 SSA 全交外部 `opt -mem2reg`。temp 存储分类两遍法：常量槽访问 /
Slot 声明 / 聚合返回值推断槽数。浮点一律十六进制字面量（f32/f64
各按位模式）；字符串常量 @.str.N private constant + gep 操作数；
运行时入口按调用点参数类型实例化声明（starry_str_conv_i64、starry_print_i32 等）；
main 包装 = 无参 main 存在时 `define i32 @main`：先跑全局 init，入口返回值收敛到 i32（int trunc/sext、bool 零扩展、unit/聚合按 0）。
`irc::layout` 公开为 LayoutView trait（irc/llvme 共享槽布局的
跨 crate 契约）。

2026-10-31 正确性修正：LLVM 15+ 不透明指针允许按值类型直接
load/store，标量读写统一走 `op_val`/`store_val`，不再让 Copy 特例恒按
i64 搬运，也不发无效的指针 bitcast。Conv 按 `from` 加载源值；GEP 下标与
`default_int` 类型一致（i32 是合法 GEP 下标，无需强转 i64）；入口
prologue 以 `br` 终结后进入首个 TAC 块。可选验收层优先调用 llvm-as，
缺失时回落 `clang -x ir -c`，两者都没有才跳过。槽宽在 2026-11 审查中
按类型加宽（只对 i128 标量/含 i128 成员的聚合），见下节。

## 工具链基线与链接取舍（2026-11 补记）

- **LLVM 基线 = 15+**：不透明指针是槽模型的硬前提（`store i8 ..., ptr` 免
  bitcast 全靠它）。`starryc llvme` 的校验器优先 llvm-as、回落 clang，
  两者皆无显式打印 SKIP；版本低于 15 的行为不受支持。
- **`-nostdlib` 与 target triple**：Windows 链接行硬编码 `-nostdlib` +
  `-lkernel32`——runtime 是自包含的（自带 mainCRTStartup 与
  write_str），不引入 CRT 初始化；代价是 clang 报
  `overriding the module target triple` 警告（IR 未声明 target，clang 用
  宿主默认值覆盖）。取舍记录在此：要消除警告需在 IR 头发射 target
  triple/datalayout，但钉死 triple 会牺牲交叉编译自由度，v1 保持宽松。

## 2026-11 审查修复（opt / irc / 后端）

- **比较折叠的类型**：`fold_fun` 把 `Bin` 折成 `Copy` 时，比较算子的目标
  类型取 `bool_`（`Bin.ty` 是操作数公共型，直接复用会发出
  `store i32 true` → clang 拒绝）。
- **全局初始化依赖序**：irc 收尾对 `TacOutput.globals` 做拓扑排序
  （直接 `LoadGlobal` + 调用链闭包）；`val b = a + 3` 声明在 `a` 之前时
  仍先初始化 `a`。环报 `IrcUnsupportedLowering`（不静默按源序执行）。
- **字符串相等**：`==`/`!=`/字符串模式走 `RuntimeFn::StrEq`（runtime
  `starry_str_eq` 按内容比较），不再对 `i8*` 发 `icmp` 指针比较。
- **聚合结构相等**：irc 递归发射逐槽比较（struct 字段、数组/元组元素、
  range 三槽、nullable flag+payload、enum tag+payload 分支）；`Ne`
  取反。字符串字段仍走 runtime 内容比较。
- **聚合 coerce**：数组/元组/区间逐槽 `Load → Conv → Store` 到新块；
  `[int; 2] = [7, 8]` 的元素加宽不再留 4 字节 store / 8 字节 load。
- **物化 Range**：范围值携带开闭标记（第 3 槽），`in`/`for` 按标记
  选择 `<`/`<=`。
- **i128**：i128 标量用 16 字节 WideCell；含 i128 成员的聚合用
  WideBlock（`[n x i128]`），其余聚合仍是 8 字节/槽。runtime 入口用
  `(hi, lo)` 拆分传参，`fmt_i128` 用按位长除法（不依赖 `__udivti3`）。
- **extern "c" 的 i128 拒收**：sem 声明点报 `SemUnsupportedConstruct`（见 semantic-notes）；llvme 对 runtime 入口拆 hi/lo 的规避不适用于 extern（C 原型属用户）。
- **值 → Nullable 的显式构造**：`coerce` 补 flag=1 + payload 路径——原落到 copy_slots 多槽 Const 分支，同一常量写进每个槽（flag=4/payload=4），native 侧 br 按 i1 低比特判真直接走错分支（差分 harness 抓到，`m ?? 9` 非空侧错值）。
- **差分 harness（P1-8）**：`llvme/tests/diff_native.rs` 同一源码双路径对拍——irc 参考解释器（`run_named_capturing` 收集 print，`print_line` 与 runtime 文本形态对齐）vs 原生程序 stdout，14 用例逐行一致。选型纪律：extern/浮点打印/i128 不入选（已知局限）。interp 侧修 Const::Str 预 intern（数组/返回里的字符串常量原被读成 0）。
- **环状全局初始化诊断带声明 span**：`reorder_globals` 的环诊断定位到环的回边目标成员的 `Def.span`（原恒 0..0）；递归 DFS 改显式栈迭代（声明数不受 parser 深度限制）。
- **CLI**：诊断改 `path:line:col`；`build` 缺 `main` 直接报错；
  `llvme` 用 clang 校验产物后才输出。

## LLVM IR 发射契约（llvme）

```rust
pub fn emit(tac: &TacOutput, tys: &TyArena, defs: &DefArena, interner: &Interner) -> String
```

- 不调用任何 LLVM 库；funs → `define`，Block → 标签，Temp → `%tN`。
- main 包装：`define i32 @main()` 先按依赖序调用全局 init，再把 Starry
  `main` 的返回值收敛为 i32（int → trunc/sext，bool → zext，unit/其余
  → 0）；缺少 `fun main()` 由 `starryc build` 直接报错，不再落到链接器。
- **聚合返回值**：callee 仍 `ret ptr`（指向自己的槽），调用方在 Call 的
  目标 temp 上分配 `[n x i64]`（含 i128 成员时 `[n x i128]`）并逐槽拷贝；
  调用方持有的永远是自己的块，第二次调用不会覆写前一次的值。聚合实参
  按基址传（callee 只读）。
- **extern "c"**：sem 在 `Def.external` 标记；irc 发 `Callee::Extern`；
  llvme 以 `declare <ret> @symbol(<params>)` 发射并按符号名调用。
- **runtime 入口**：`starry_print_{i1,i8,i16,i32,i64,i128,f32,f64,string,
  char}`、`starry_str_conv_*`、`starry_str_concat`、`starry_str_eq`。
  i1 实参先 `zext` 到 i32（C 形参是 `int`，高位不干净会把 false 打成
  true）；i4 先 `sext` 到 i8；i128 拆成 `(hi, lo)` 两个 i64（MSVC ABI
  的 `__int128` 与 LLVM `i128` 传参不一致，实测段错误）。
- **槽宽**：i128 标量/含 i128 成员的聚合加宽到 16 字节/槽，普通聚合
  仍 8 字节/槽；GEP 下标按元素计，i128 标量/成员不再越界写栈。
- `starryc llvme` 在 clang 可用时先 `clang -x ir -c` 校验产物，非法 IR
  exit 1（`store i32 true` 一类不再静默输出）。
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
