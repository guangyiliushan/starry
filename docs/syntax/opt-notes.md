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
    Call { dst: Option<TempId>, callee: DefId,          ty: TyId },
}
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

## M1 落地边界（irc crate，2026-10-23）

已 lower：字面量/标识符、算术·位运算·比较（操作数公共型按提升格统一，
`ty` 字段=操作数型，比较结果型由 llvme 按算子推 i1）、短路布尔（Br 链
物化）、一元算子、`as` 数值转换（`UnOp::Conv`）、具名/命名实参调用 +
默认实参调用点求值、if 语句与 if 表达式（结果槽绕 phi）、while/loop、
break/continue（flow_res 回指）、return、val→Temp / var→slot / 无初始化
→slot（未定义值读取）、comptime/unsafe 块按普通块。

发 `IrcUnsupportedLowering`：when/for/defer/grow、模式绑定以外的
Binding、复合赋值、非标识符赋值目标、聚合值全家（struct/enum 构造、
数组/元组字面量、成员/索引访问、nullable、`as?`/`as!`/`!!`）、
`in`/`is`、StringInterp、if/while val 条件、带值 break、多段路径、
顶层非 fun 声明。

实现注记：BlockId 稀疏（merge 块先 reserve 后 switch），块寻址按 id
查表而非下标；dump 按 BlockId 排序输出；字符串字面量解码后 intern 进
`sem.interner`（`lower` 取 `&mut SemanticOutput`，Symbol 与 arena 同生
共死纪律不变）。

## 优化 pass（v1，按序）

教科书局部/全局基础集，全部纯函数 `&mut FunBody`：

1. **常量折叠**：Bin/Un 双常数操作数 → 单 Const（含跳转条件定值 →
   无条件化 Br→Jmp）。
2. **复制传播**：`x = y` 后凡 x 的使用点可用 y（局部，同一基本块内）。
3. **死代码消除**：结果不再被使用的纯计算指令删除（无副作用谓词：
   Load/Call/Store 永不删；活性分析 v1 只做块内反向扫描）。
4. **局部 CSE**：基本块内可用表达式表（龙书 8.5），`x op y` 已算过
   则复用旧 Temp。
5. **循环不变代码外提（LICM，选做）**：DFS 找回边定循环，循环内
   操作数均在循环外定值的 Bin/Un 提升到 preheader。

v1 跳过（记入扩展位，不归本阶段）：全局数据流（到达定值/可用表达式
全局版——ch9 落地时 SSA 进 crate）、强度削弱、归纳变量消除、寄存器
分配——机器码层面 LLVM 后端全包。教科书后半程 pass 需要时按同一
`&mut FunBody` 接口加。扩展位另记：跳表 switch、Conv 常量折叠、
值编号/DAG。

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
