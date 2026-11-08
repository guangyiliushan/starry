//! 三地址码（TAC）类型：irc 的产出契约，opt/llvme 的消费契约。
//! 形状依据 `docs/syntax/opt-notes.md`（三轮评审钉死）。
//!
//! 核心不变量（irc 发射纪律，opt/llvme 依赖）：
//! - TempId/BlockId per-function 单调递增、永不改名、永不复用、空洞合法；
//!   任何 pass 不得 renumber（pass 间 diff 只显示真实变化）。
//! - 同一指令的操作数类型一致（sem 提升格 + irc 强制插 Conv 保证）。

use std::fmt::Write as _;

use ast::interner::{Interner, Symbol};
use ast::syntax::node::Diagnostic;
use sem::{DefArena, DefId, FloatKind, IntKind, TyArena, TyId, TyKind};

/// 临时变量 id（per-function 单调分配）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TempId(pub u32);

/// 基本块 id（per-function 单调分配；entry = BlockId(0)）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// 常量操作数（类型从所属指令的 `ty` 取——操作数类型均匀不变量）
#[derive(Debug, Clone)]
pub enum Const {
    Int(i128),
    Float(f64),
    Bool(bool),
    Null,
    /// 字符串字面量（interned 内容）
    Str(Symbol),
    /// unit 值（无副作用表达式的占位结果）
    Unit,
}

/// 操作数
#[derive(Debug, Clone)]
pub enum Operand {
    Temp(TempId),
    Const(Const),
}

/// 二元算子（比较结果恒为 bool；逻辑与/或走 Br 链，见 opt-notes）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// 一元算子；`Conv(from)` = 数值转换（目标类型在指令 `ty` 上，全有符号）
#[derive(Debug, Clone, Copy)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    Conv(TyId),
}

/// 调用目标：用户函数（sem 解析的 DefId）、外部符号（extern "c" 声明）
/// 或运行时入口（llvme 侧 extern declare；std-only 红线下运行时以符号
/// 存在，不进 crate）
#[derive(Debug, Clone, Copy)]
pub enum Callee {
    Def(DefId),
    Extern(DefId),
    Runtime(RuntimeFn),
}

/// 运行时入口（v1：字符串化、拼接与 print；名字由 llvme 映射为
/// starry_str_conv_* / starry_str_concat / starry_print_*）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    /// 标量 → string（源类型由前置 Param 指令的 ty 携带）
    StrConv,
    /// char → string（按字符编码，而非码点整数）
    StrConvChar,
    /// (string, string) → string
    StrConcat,
    /// (string, string) → bool（内容比较；`==`/`!=` 与字符串模式共用）
    StrEq,
    /// print(标量)：源类型由前置 Param 指令的 ty 携带，返回 unit
    Print,
    /// print(char)：按字符编码写出
    PrintChar,
}

/// 三地址指令（终止符除外）
#[derive(Debug, Clone)]
pub enum Instr {
    Bin {
        dst: TempId,
        op: BinOp,
        l: Operand,
        r: Operand,
        ty: TyId,
    },
    Un {
        dst: TempId,
        op: UnOp,
        a: Operand,
        ty: TyId,
    },
    Copy {
        dst: TempId,
        src: Operand,
        ty: TyId,
    },
    /// 槽寻址：`dst = base[idx]`（聚合值的标量槽；标量 slot 的 idx 恒为 0）
    Load {
        dst: TempId,
        base: TempId,
        idx: Operand,
        ty: TyId,
    },
    Store {
        base: TempId,
        idx: Operand,
        src: Operand,
        ty: TyId,
    },
    /// 顶层全局标量读取：`dst = @global`（跨函数持久存储）
    LoadGlobal {
        dst: TempId,
        global: DefId,
        ty: TyId,
    },
    /// 顶层全局标量写入：`@global = src`
    StoreGlobal {
        global: DefId,
        src: Operand,
        ty: TyId,
    },
    /// M2 取址：`dst = &base`（base 槽的地址；llvme 发 alloca 地址）
    AddrOf {
        dst: TempId,
        base: TempId,
        ty: TyId,
    },
    /// M2 经指针读：`dst = *ptr`（ptr 槽里存的是地址）
    LoadPtr {
        dst: TempId,
        ptr: TempId,
        ty: TyId,
    },
    /// M2 经指针写：`*ptr = src`
    StorePtr {
        ptr: TempId,
        src: Operand,
        ty: TyId,
    },
    /// 聚合基址声明：dst 是 count 个槽的块基址。当前由数组填充
    /// [v; n] 发射（其槽写入全动态，llvme 需静态尺寸分配存储）。
    /// opt 侧契约：声明非计算值——DCE 永不删，fold/CSE/copy-prop 跳过
    Slot {
        dst: TempId,
        count: u32,
    },
    /// 调用实参（按形参序发射，紧邻随后的 Call）
    Param {
        a: Operand,
        ty: TyId,
    },
    Call {
        dst: Option<TempId>,
        callee: Callee,
        ty: TyId,
    },
}

/// 基本块终止符（cf. opt-notes：term = Jmp/Br/Ret）
#[derive(Debug, Clone)]
pub enum Term {
    Jmp(BlockId),
    Br {
        cond: Operand,
        then_: BlockId,
        else_: BlockId,
    },
    Ret {
        value: Option<Operand>,
        ty: TyId,
    },
    /// 新块占位与不可达 fallthrough（sem 松弛项穿透；LLVM 侧发 unreachable）
    Unreachable,
}

/// 基本块
#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub instrs: Vec<Instr>,
    pub term: Term,
}

/// 函数体：`params` 是入口临时变量（t0..t(n-1)，声明序；llvme 映射为
/// LLVM 形参）
#[derive(Debug)]
pub struct FunBody {
    pub def: DefId,
    pub params: Vec<(TempId, TyId)>,
    pub ret: TyId,
    pub blocks: Vec<Block>,
    pub entry: BlockId,
}

/// 顶层全局声明（v1：标量存储；init 为合成初始化函数体，main 前调用）
#[derive(Debug)]
pub struct GlobalBody {
    pub def: DefId,
    pub ty: TyId,
    pub init: Option<FunBody>,
}

/// irc 产物
#[derive(Debug, Default)]
pub struct TacOutput {
    pub funs: Vec<FunBody>,
    pub globals: Vec<GlobalBody>,
    pub diags: Vec<Diagnostic>,
}

/// LLVM 风格的类型名（dump 展示用；聚合类型 M2 前显示 `?`）
fn ty_name(tys: &TyArena, ty: TyId) -> &'static str {
    match tys.kind(ty) {
        TyKind::Int(IntKind::I4) => "i4",
        TyKind::Int(IntKind::I8) => "i8",
        TyKind::Int(IntKind::I16) => "i16",
        TyKind::Int(IntKind::I32) => "i32",
        TyKind::Int(IntKind::I64) => "i64",
        TyKind::Int(IntKind::I128) => "i128",
        TyKind::Float(FloatKind::F32) => "float",
        TyKind::Float(FloatKind::F64) => "double",
        TyKind::Bool => "i1",
        TyKind::Char => "i32",
        TyKind::String => "str",
        TyKind::Unit => "unit",
        TyKind::Null => "null",
        _ => "?",
    }
}

/// dump 展示的类型名：聚合类型经 defs/interner 解码（P、T?、[T; n]）
fn ty_display(tys: &TyArena, defs: &DefArena, interner: &Interner, ty: TyId) -> String {
    match tys.kind(ty) {
        TyKind::Named(def, _) => interner.resolve(defs.get(*def).name).to_string(),
        TyKind::Nullable(inner) => format!("{}?", ty_display(tys, defs, interner, *inner)),
        TyKind::Array(el, n) => format!("[{}; {}]", ty_display(tys, defs, interner, *el), n),
        TyKind::Tuple(es) => {
            let parts: Vec<String> = es
                .iter()
                .map(|e| ty_display(tys, defs, interner, *e))
                .collect();
            format!("({})", parts.join(", "))
        }
        TyKind::Range(inner) => format!("range<{}>", ty_display(tys, defs, interner, *inner)),
        TyKind::Fn(params, ret) => format!(
            "fn({}) -> {}",
            params
                .iter()
                .map(|p| ty_display(tys, defs, interner, *p))
                .collect::<Vec<_>>()
                .join(", "),
            ty_display(tys, defs, interner, *ret)
        ),
        _ => ty_name(tys, ty).to_string(),
    }
}

fn op_text(op: &Operand, interner: &Interner, out: &mut String) {
    match op {
        Operand::Temp(t) => {
            let _ = write!(out, "%t{}", t.0);
        }
        Operand::Const(Const::Int(v)) => {
            let _ = write!(out, "{v}");
        }
        Operand::Const(Const::Float(v)) => {
            let _ = write!(out, "{v}");
        }
        Operand::Const(Const::Bool(b)) => {
            let _ = write!(out, "{b}");
        }
        Operand::Const(Const::Null) => out.push_str("null"),
        Operand::Const(Const::Str(sym)) => {
            let _ = write!(out, "{:?}", interner.resolve(*sym));
        }
        Operand::Const(Const::Unit) => out.push_str("unit"),
    }
}

fn binop_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        BinOp::Shl => "shl",
        BinOp::Shr => "shr",
    }
}

impl TacOutput {
    /// 稳定文本形态：快照测试与 `starryc irc` 的输出（名字解码依赖
    /// defs/interner；类型名依赖 TyArena）
    pub fn dump(&self, tys: &TyArena, defs: &DefArena, interner: &Interner) -> String {
        let mut out = String::new();
        for fun in &self.funs {
            let name = interner.resolve(defs.get(fun.def).name);
            let _ = write!(out, "fun {name}(");
            for (i, (t, ty)) in fun.params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "%t{}: {}", t.0, ty_display(tys, defs, interner, *ty));
            }
            let _ = writeln!(out, ") -> {}:", ty_display(tys, defs, interner, fun.ret));
            let mut order: Vec<usize> = (0..fun.blocks.len()).collect();
            order.sort_by_key(|&i| fun.blocks[i].id.0);
            for i in order {
                let block = &fun.blocks[i];
                let _ = writeln!(out, "  b{}:", block.id.0);
                for instr in &block.instrs {
                    out.push_str("    ");
                    self.dump_instr(instr, tys, defs, interner, &mut out);
                    out.push('\n');
                }
                out.push_str("    ");
                self.dump_term(&block.term, tys, defs, interner, &mut out);
                out.push('\n');
            }
        }
        out
    }

    fn dump_instr(
        &self,
        instr: &Instr,
        tys: &TyArena,
        defs: &DefArena,
        interner: &Interner,
        out: &mut String,
    ) {
        match instr {
            Instr::Bin { dst, op, l, r, ty } => {
                let _ = write!(
                    out,
                    "%t{} = {} {} ",
                    dst.0,
                    binop_text(*op),
                    ty_display(tys, defs, interner, *ty)
                );
                op_text(l, interner, out);
                out.push_str(", ");
                op_text(r, interner, out);
            }
            Instr::Un { dst, op, a, ty } => match op {
                UnOp::Conv(from) => {
                    let _ = write!(out, "%t{} = conv ", dst.0);
                    op_text(a, interner, out);
                    let _ = write!(
                        out,
                        " : {} -> {}",
                        ty_display(tys, defs, interner, *from),
                        ty_display(tys, defs, interner, *ty)
                    );
                }
                UnOp::Neg | UnOp::Not | UnOp::BitNot => {
                    let name = match op {
                        UnOp::Neg => "neg",
                        UnOp::Not => "not",
                        _ => "bitnot",
                    };
                    let _ = write!(
                        out,
                        "%t{} = {name} {} ",
                        dst.0,
                        ty_display(tys, defs, interner, *ty)
                    );
                    op_text(a, interner, out);
                }
            },
            Instr::Copy { dst, src, ty } => {
                let _ = write!(out, "%t{} = copy ", dst.0);
                op_text(src, interner, out);
                let _ = write!(out, " : {}", ty_display(tys, defs, interner, *ty));
            }
            Instr::Load { dst, base, idx, ty } => {
                let _ = write!(
                    out,
                    "%t{} = load {} %t{}[",
                    dst.0,
                    ty_display(tys, defs, interner, *ty),
                    base.0
                );
                op_text(idx, interner, out);
                out.push(']');
            }
            Instr::Store { base, idx, src, ty } => {
                let _ = write!(out, "store %t{}[", base.0);
                op_text(idx, interner, out);
                out.push_str("] = ");
                op_text(src, interner, out);
                let _ = write!(out, " : {}", ty_display(tys, defs, interner, *ty));
            }
            Instr::LoadGlobal { dst, global, ty } => {
                let _ = write!(
                    out,
                    "%t{} = load_global {}.{} : {}",
                    dst.0,
                    interner.resolve(defs.get(*global).name),
                    global.0,
                    ty_display(tys, defs, interner, *ty)
                );
            }
            Instr::StoreGlobal { global, src, ty } => {
                out.push_str("store_global ");
                op_text(src, interner, out);
                let _ = write!(
                    out,
                    " -> {}.{} : {}",
                    interner.resolve(defs.get(*global).name),
                    global.0,
                    ty_display(tys, defs, interner, *ty)
                );
            }
            Instr::AddrOf { dst, base, .. } => {
                let _ = writeln!(out, "%t{} = addrof %t{}", dst.0, base.0);
            }
            Instr::LoadPtr { dst, ptr, .. } => {
                let _ = writeln!(out, "%t{} = loadptr %t{}", dst.0, ptr.0);
            }
            Instr::StorePtr { ptr, src, .. } => {
                let _ = write!(out, "storeptr %t{} = ", ptr.0);
                op_text(src, interner, out);
                let _ = writeln!(out);
            }
            Instr::Slot { dst, count } => {
                let _ = write!(out, "%t{} = slot [{}]", dst.0, count);
            }
            Instr::Param { a, ty } => {
                out.push_str("param ");
                op_text(a, interner, out);
                let _ = write!(out, " : {}", ty_display(tys, defs, interner, *ty));
            }
            Instr::Call { dst, callee, ty } => {
                if let Some(dst) = dst {
                    let _ = write!(out, "%t{} = ", dst.0);
                }
                let name = match callee {
                    Callee::Def(d) => interner.resolve(defs.get(*d).name).to_string(),
                    Callee::Extern(d) => {
                        format!("extern {}", interner.resolve(defs.get(*d).name))
                    }
                    Callee::Runtime(RuntimeFn::StrConv) => "starry_str_conv".to_string(),
                    Callee::Runtime(RuntimeFn::StrConvChar) => "starry_str_conv_char".to_string(),
                    Callee::Runtime(RuntimeFn::StrConcat) => "starry_str_concat".to_string(),
                    Callee::Runtime(RuntimeFn::StrEq) => "starry_str_eq".to_string(),
                    Callee::Runtime(RuntimeFn::Print) => "starry_print".to_string(),
                    Callee::Runtime(RuntimeFn::PrintChar) => "starry_print_char".to_string(),
                };
                let _ = write!(
                    out,
                    "call {name} : {}",
                    ty_display(tys, defs, interner, *ty)
                );
            }
        }
    }

    fn dump_term(
        &self,
        term: &Term,
        tys: &TyArena,
        defs: &DefArena,
        interner: &Interner,
        out: &mut String,
    ) {
        match term {
            Term::Jmp(b) => {
                let _ = write!(out, "jmp b{}", b.0);
            }
            Term::Br { cond, then_, else_ } => {
                out.push_str("br ");
                op_text(cond, interner, out);
                let _ = write!(out, " -> b{}, b{}", then_.0, else_.0);
            }
            Term::Ret { value, ty } => match value {
                Some(v) => {
                    out.push_str("ret ");
                    op_text(v, interner, out);
                    let _ = write!(out, " : {}", ty_display(tys, defs, interner, *ty));
                }
                None => {
                    let _ = write!(out, "ret : {}", ty_display(tys, defs, interner, *ty));
                }
            },
            Term::Unreachable => out.push_str("unreachable"),
        }
    }
}
