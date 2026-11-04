//! 模块/函数发射：两遍（temp 存储分类 → 指令发射）。
//!
//! 存储分类（每个 temp 恰好一个 entry alloca，外部 mem2reg 可提升）：
//! - Cell：i64 槽（标量与单槽聚合共用）
//! - Block(n)：`[n x i64]` 块（多槽聚合基址；n 来自常量槽访问、
//!   Slot 声明、聚合返回值推断）
//! - PtrCell(n)：`ptr` 槽装聚合指针（聚合实参/返回值）
//!
//! 不透明指针风格（LLVM 15+）：无指针 bitcast，load/store/ret 聚合
//! 经 `ptr`；槽寻址统一 `getelementptr [n x i64], ptr p, i64 0, i64 i`
//! （Cell 即自身，idx 恒 0；tag/flag 是槽 0 里的 i8，写时就地
//! `store i8 ..., ptr` —— 天然免 bitcast）。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ast::interner::{Interner, Symbol};
use irc::layout::scalar_slots;
use irc::tac::{
    BinOp, Callee, Const, FunBody, Instr, Operand, RuntimeFn, TacOutput, TempId, Term, UnOp,
};
use sem::{DefArena, FloatKind, IntKind, TyArena, TyId, TyKind};

use crate::ty::{Lc, TyClass, class_of, float_lit, fun_name, scalar_ty};

pub fn emit_module(tac: &TacOutput, tys: &TyArena, defs: &DefArena, interner: &Interner) -> String {
    let lc = Lc {
        tys,
        defs,
        interner,
    };
    let mut g = Global::default();
    let mut body = String::new();
    let mut defined = Vec::new();
    for fun in &tac.funs {
        defined.push(fun.def);
        emit_fun(&lc, &mut g, fun, &mut body);
    }
    emit_main_wrapper(&lc, tac, &mut body);
    // 外部函数 declare（被调用但无体：native import 等）
    for (name, (ret, params)) in std::mem::take(&mut g.extern_decls) {
        let _ = writeln!(body, "declare {} @{}({})", ret, name, params);
    }
    for (name, (ret, params)) in std::mem::take(&mut g.runtime_decls) {
        let _ = writeln!(body, "declare {} @{}({})", ret, name, params);
    }

    let mut out = String::new();
    for (sym, id) in &g.strings {
        let text = lc.interner.resolve(*sym);
        let _ = writeln!(
            out,
            "@.str.{} = private constant [{} x i8] c\"{}\\00\"",
            id,
            text.len() + 1,
            escape_cstr(text)
        );
    }
    // 顶层全局存储 + 初始化函数（main 包装先 call init 再写回 @global）。
    for global in &tac.globals {
        let name = fun_name(&lc, global.def);
        let storage_ty = if scalar_ty(lc.tys, global.ty) == "i8*" {
            "i8*"
        } else {
            "i64"
        };
        let zero = if storage_ty == "i8*" { "null" } else { "0" };
        let _ = writeln!(out, "{} = global {} {}", name, storage_ty, zero);
    }
    out.push_str(&body);
    for global in &tac.globals {
        if let Some(init) = &global.init {
            emit_fun_named(
                &lc,
                &mut g,
                init,
                &mut out,
                format!("@__starry_init_global.{}", global.def.0),
            );
        }
    }
    let _ = defined;
    out
}

#[derive(Default)]
struct Global {
    strings: BTreeMap<Symbol, usize>,
    runtime_decls: BTreeMap<String, (String, String)>,
    extern_decls: BTreeMap<String, (String, String)>,
}

impl Global {
    fn str_id(&mut self, sym: Symbol) -> usize {
        if let Some(&id) = self.strings.get(&sym) {
            return id;
        }
        let id = self.strings.len();
        self.strings.insert(sym, id);
        id
    }
}

fn escape_cstr(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\22"),
            b'\\' => out.push_str("\\5C"),
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{:02X}", b);
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Storage {
    /// i64 槽
    Cell,
    /// [n x i64] 块
    Block(u32),
    /// ptr 槽装聚合指针（聚合实参/返回值），n = 指向块的槽数
    PtrCell(u32),
}

fn see_op(op: &Operand, max: &mut u32) {
    if let Operand::Temp(t) = op {
        *max = (*max).max(t.0);
    }
}

fn temp_max(fun: &FunBody) -> u32 {
    let mut max = fun.params.iter().map(|(t, _)| t.0).max().unwrap_or(0);
    for b in &fun.blocks {
        for i in &b.instrs {
            match i {
                Instr::Bin { dst, l, r, .. } => {
                    max = max.max(dst.0);
                    see_op(l, &mut max);
                    see_op(r, &mut max);
                }
                Instr::Un { dst, a, .. } => {
                    max = max.max(dst.0);
                    see_op(a, &mut max);
                }
                Instr::Copy { dst, src, .. } => {
                    max = max.max(dst.0);
                    see_op(src, &mut max);
                }
                Instr::Load { dst, base, idx, .. } => {
                    max = max.max(dst.0).max(base.0);
                    see_op(idx, &mut max);
                }
                Instr::LoadGlobal { dst, .. } => max = max.max(dst.0),
                Instr::StoreGlobal { .. } => {}
                Instr::Store { base, idx, src, .. } => {
                    max = max.max(base.0);
                    see_op(idx, &mut max);
                    see_op(src, &mut max);
                }
                Instr::Slot { dst, .. } => max = max.max(dst.0),
                Instr::Param { a, .. } => see_op(a, &mut max),
                Instr::Call { dst, .. } => {
                    if let Some(d) = dst {
                        max = max.max(d.0);
                    }
                }
            }
        }
        match &b.term {
            Term::Ret { value: Some(v), .. } => see_op(v, &mut max),
            Term::Br { cond, .. } => see_op(cond, &mut max),
            _ => {}
        }
    }
    max
}

/// 第一遍：temp → 存储分类
fn classify(lc: &Lc, fun: &FunBody) -> Vec<Storage> {
    let n = (temp_max(fun) + 1) as usize;
    let mut req: Vec<usize> = vec![0; n];
    let mut ptr: Vec<Option<u32>> = vec![None; n];
    for (t, ty) in &fun.params {
        if class_of(lc.tys, *ty) == TyClass::Agg {
            ptr[t.0 as usize] = Some(scalar_slots(lc, *ty).max(1) as u32);
        }
    }
    for b in &fun.blocks {
        for i in &b.instrs {
            match i {
                Instr::Slot { dst, count } => {
                    req[dst.0 as usize] = req[dst.0 as usize].max(*count as usize)
                }
                Instr::Load { base, idx, .. } | Instr::Store { base, idx, .. } => {
                    if let Operand::Const(Const::Int(k)) = idx {
                        req[base.0 as usize] = req[base.0 as usize].max(*k as usize + 1);
                    }
                }
                Instr::Copy { dst, ty, .. } => {
                    req[dst.0 as usize] = req[dst.0 as usize].max(scalar_slots(lc, *ty).max(1))
                }
                Instr::Call { dst, ty, .. } => {
                    if let Some(d) = dst
                        && class_of(lc.tys, *ty) == TyClass::Agg
                    {
                        ptr[d.0 as usize] = Some(scalar_slots(lc, *ty).max(1) as u32);
                    }
                }
                _ => {}
            }
        }
        if let Term::Ret {
            value: Some(Operand::Temp(t)),
            ty,
        } = &b.term
            && class_of(lc.tys, *ty) == TyClass::Agg
        {
            req[t.0 as usize] = req[t.0 as usize].max(scalar_slots(lc, *ty));
        }
    }
    (0..n)
        .map(|i| match ptr[i] {
            Some(n) => Storage::PtrCell(n),
            None if req[i] > 1 => Storage::Block(req[i] as u32),
            _ => Storage::Cell,
        })
        .collect()
}

struct FnEmit<'e, 'l> {
    lc: &'l Lc<'l>,
    g: &'e mut Global,
    st: &'e [Storage],
    out: &'e mut String,
    u: u32,
}

impl<'e, 'l> FnEmit<'e, 'l> {
    fn fresh(&mut self) -> String {
        let s = format!("%u{}", self.u);
        self.u += 1;
        s
    }

    /// temp 的槽地址（ptr 值，指向 i64 槽）
    fn slot_addr(&mut self, t: TempId, idx: &Operand) -> String {
        match self.st[t.0 as usize] {
            Storage::Cell => format!("%t{}.slot", t.0),
            Storage::Block(n) | Storage::PtrCell(n) => {
                let base = if matches!(self.st[t.0 as usize], Storage::PtrCell(_)) {
                    let p = self.fresh();
                    let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                    p
                } else {
                    format!("%t{}.slot", t.0)
                };
                let i = self.op_val(idx, self.lc.tys.default_int);
                let ity = scalar_ty(self.lc.tys, self.lc.tys.default_int);
                let gep = self.fresh();
                let _ = writeln!(
                    self.out,
                    "  {} = getelementptr [{} x i64], ptr {}, i64 0, {} {}",
                    gep, n, base, ity, i
                );
                gep
            }
        }
    }

    /// 常量/临时量的值文本（标量按 ty 加载；聚合返回指针值）
    fn op_val(&mut self, op: &Operand, ty: TyId) -> String {
        match op {
            Operand::Temp(t) => {
                if class_of(self.lc.tys, ty) == TyClass::Agg {
                    // 聚合值 = 其块/指针的地址
                    match self.st[t.0 as usize] {
                        Storage::PtrCell(_) => {
                            let p = self.fresh();
                            let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                            p
                        }
                        // Block/Cell 聚合值的当前槽地址；%tN 只在形参值域存在，
                        // 本地聚合必须经 %tN.slot（否则未定义 + 丢失 var 更新）。
                        _ => format!("%t{}.slot", t.0),
                    }
                } else {
                    let lty = scalar_ty(self.lc.tys, ty);
                    let v = self.fresh();
                    let _ = writeln!(self.out, "  {} = load {}, ptr %t{}.slot", v, lty, t.0);
                    v
                }
            }
            Operand::Const(c) => self.const_val(c, ty),
        }
    }

    fn const_val(&mut self, c: &Const, ty: TyId) -> String {
        match c {
            Const::Int(v) => format!("{v}"),
            Const::Float(v) => float_lit(
                *v,
                matches!(self.lc.tys.kind(ty), TyKind::Float(FloatKind::F32)),
            ),
            Const::Bool(b) => format!("{b}"),
            Const::Null => "0".into(),
            Const::Unit => "0".into(),
            Const::Str(sym) => {
                let id = self.g.str_id(*sym);
                let len = self.lc.interner.resolve(*sym).len() + 1;
                format!("getelementptr ([{len} x i8], ptr @.str.{id}, i64 0, i64 0)")
            }
        }
    }

    /// 把值写入 temp 的槽（按 ty；Cell/Block 第一槽）
    fn store_val(&mut self, t: TempId, ty: TyId, val: String) {
        match self.st[t.0 as usize] {
            Storage::PtrCell(_) => {
                let _ = writeln!(self.out, "  store ptr {}, ptr %t{}.slot", val, t.0);
            }
            _ => {
                let lty = scalar_ty(self.lc.tys, ty);
                let _ = writeln!(self.out, "  store {} {}, ptr %t{}.slot", lty, val, t.0);
            }
        }
    }
}

fn int_rank(k: &IntKind) -> u8 {
    match k {
        IntKind::I4 => 0,
        IntKind::I8 => 1,
        IntKind::I16 => 2,
        IntKind::I32 => 3,
        IntKind::I64 => 4,
        IntKind::I128 => 5,
    }
}

fn emit_fun(lc: &Lc, g: &mut Global, fun: &FunBody, body: &mut String) {
    let name = fun_name(lc, fun.def);
    emit_fun_named(lc, g, fun, body, name);
}

fn emit_fun_named(lc: &Lc, g: &mut Global, fun: &FunBody, body: &mut String, name: String) {
    let st = classify(lc, fun);
    let ret_ty = if class_of(lc.tys, fun.ret) == TyClass::Agg {
        "ptr".to_string()
    } else {
        scalar_ty(lc.tys, fun.ret)
    };
    let params: Vec<String> = fun
        .params
        .iter()
        .map(|(t, ty)| {
            let pty = if class_of(lc.tys, *ty) == TyClass::Agg {
                "ptr".to_string()
            } else {
                scalar_ty(lc.tys, *ty)
            };
            format!("{} %t{}", pty, t.0)
        })
        .collect();
    let _ = writeln!(body, "define {} {}({}) {{", ret_ty, name, params.join(", "));

    // entry allocas
    let mut e = FnEmit {
        lc,
        g,
        st: &st,
        out: body,
        u: 0,
    };
    for (i, s) in st.iter().enumerate() {
        match s {
            Storage::Cell => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca i64", i);
            }
            Storage::Block(n) => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca [{} x i64]", i, n);
            }
            Storage::PtrCell(_) => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca ptr", i);
            }
        }
    }
    // 形参是 %tN 值；存入各自槽
    for (t, ty) in &fun.params {
        if class_of(lc.tys, *ty) == TyClass::Agg {
            let _ = writeln!(e.out, "  store ptr %t{}, ptr %t{}.slot", t.0, t.0);
        } else {
            let lty = scalar_ty(lc.tys, *ty);
            let _ = writeln!(e.out, "  store {} %t{}, ptr %t{}.slot", lty, t.0, t.0);
        }
    }

    if let Some(first) = fun.blocks.first() {
        let _ = writeln!(e.out, "  br label %{}", block_label(first.id.0));
    }

    let mut pending: Vec<(String, String)> = Vec::new();
    for (bi, b) in fun.blocks.iter().enumerate() {
        let _ = bi;
        let _ = writeln!(e.out, "{}:", block_label(b.id.0));
        for instr in &b.instrs {
            emit_instr(&mut e, &mut pending, instr);
        }
        emit_term(&mut e, &b.term);
    }
    let _ = writeln!(e.out, "}}");
    let _ = writeln!(e.out);
}

fn block_label(id: u32) -> String {
    format!("b{}", id)
}

fn emit_instr(e: &mut FnEmit, pending: &mut Vec<(String, String)>, instr: &Instr) {
    match instr {
        Instr::Bin { dst, op, l, r, ty } => {
            let lv = e.op_val(l, *ty);
            let rv = e.op_val(r, *ty);
            // bin_llvm 自管 fresh：内联实现（借用冲突回避）
            let is_float = matches!(e.lc.tys.kind(*ty), TyKind::Float(_));
            let tname = scalar_ty(e.lc.tys, *ty);
            let mnemonic = match op {
                BinOp::Add => {
                    if is_float {
                        "fadd"
                    } else {
                        "add"
                    }
                }
                BinOp::Sub => {
                    if is_float {
                        "fsub"
                    } else {
                        "sub"
                    }
                }
                BinOp::Mul => {
                    if is_float {
                        "fmul"
                    } else {
                        "mul"
                    }
                }
                BinOp::Div => {
                    if is_float {
                        "fdiv"
                    } else {
                        "sdiv"
                    }
                }
                BinOp::Rem => {
                    if is_float {
                        "frem"
                    } else {
                        "srem"
                    }
                }
                BinOp::BitAnd => "and",
                BinOp::BitOr => "or",
                BinOp::BitXor => "xor",
                BinOp::Shl => "shl",
                BinOp::Shr => "ashr",
                BinOp::Eq => {
                    if is_float {
                        "fcmp oeq"
                    } else {
                        "icmp eq"
                    }
                }
                BinOp::Ne => {
                    if is_float {
                        "fcmp one"
                    } else {
                        "icmp ne"
                    }
                }
                BinOp::Lt => {
                    if is_float {
                        "fcmp olt"
                    } else {
                        "icmp slt"
                    }
                }
                BinOp::Le => {
                    if is_float {
                        "fcmp ole"
                    } else {
                        "icmp sle"
                    }
                }
                BinOp::Gt => {
                    if is_float {
                        "fcmp ogt"
                    } else {
                        "icmp sgt"
                    }
                }
                BinOp::Ge => {
                    if is_float {
                        "fcmp oge"
                    } else {
                        "icmp sge"
                    }
                }
            };
            let v = e.fresh();
            let _ = writeln!(e.out, "  {} = {} {} {}, {}", v, mnemonic, tname, lv, rv);
            // 比较结果是 i1，dst 槽按 i1 存
            let store_ty = if matches!(
                op,
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
            ) {
                e.lc.tys.bool_
            } else {
                *ty
            };
            e.store_val(*dst, store_ty, v);
        }
        Instr::Un { dst, op, a, ty } => {
            let av = match op {
                UnOp::Conv(from) => e.op_val(a, *from),
                _ => e.op_val(a, *ty),
            };
            let v = e.fresh();
            match op {
                UnOp::Neg => {
                    if matches!(e.lc.tys.kind(*ty), TyKind::Float(_)) {
                        let _ =
                            writeln!(e.out, "  {} = fneg {} {}", v, scalar_ty(e.lc.tys, *ty), av);
                    } else {
                        let _ = writeln!(
                            e.out,
                            "  {} = sub {} 0, {}",
                            v,
                            scalar_ty(e.lc.tys, *ty),
                            av
                        );
                    }
                }
                UnOp::Not => {
                    let _ = writeln!(e.out, "  {} = xor i1 {}, true", v, av);
                }
                UnOp::BitNot => {
                    let _ = writeln!(
                        e.out,
                        "  {} = xor {} {}, -1",
                        v,
                        scalar_ty(e.lc.tys, *ty),
                        av
                    );
                }
                UnOp::Conv(from) => {
                    use TyKind::*;
                    let k = match (e.lc.tys.kind(*from), e.lc.tys.kind(*ty)) {
                        (Int(f), Int(t)) => {
                            if int_rank(t) > int_rank(f) {
                                "sext"
                            } else {
                                "trunc"
                            }
                        }
                        (Int(_), Float(_)) => "sitofp",
                        (Float(_), Int(_)) => "fptosi",
                        (Float(FloatKind::F32), Float(FloatKind::F64)) => "fpext",
                        (Float(FloatKind::F64), Float(FloatKind::F32)) => "fptrunc",
                        _ => "bitcast",
                    };
                    let _ = writeln!(
                        e.out,
                        "  {} = {} {} {} to {}",
                        v,
                        k,
                        scalar_ty(e.lc.tys, *from),
                        av,
                        scalar_ty(e.lc.tys, *ty)
                    );
                }
            }
            e.store_val(*dst, *ty, v);
        }
        Instr::Copy { dst, src, ty } if class_of(e.lc.tys, *ty) == TyClass::Scalar => {
            let v = e.op_val(src, *ty);
            e.store_val(*dst, *ty, v);
        }
        Instr::Copy { dst, src, ty } => {
            let v = match src {
                Operand::Const(_) => e.op_val(src, *ty),
                Operand::Temp(t) => {
                    // 槽位拷贝（i64 位搬运；单槽聚合同形）
                    let v = e.fresh();
                    let _ = writeln!(e.out, "  {} = load i64, ptr %t{}.slot", v, t.0);
                    v
                }
            };
            match e.st[dst.0 as usize] {
                Storage::PtrCell(_) => e.store_val(*dst, *ty, v),
                _ => {
                    let _ = writeln!(e.out, "  store i64 {}, ptr %t{}.slot", v, dst.0);
                }
            }
        }
        Instr::Load { dst, base, idx, ty } => {
            let addr = e.slot_addr(*base, idx);
            let lty = scalar_ty(e.lc.tys, *ty);
            let v = e.fresh();
            let _ = writeln!(e.out, "  {} = load {}, ptr {}", v, lty, addr);
            e.store_val(*dst, *ty, v);
        }
        Instr::Store { base, idx, src, ty } => {
            let addr = e.slot_addr(*base, idx);
            let v = e.op_val(src, *ty);
            let lty = scalar_ty(e.lc.tys, *ty);
            let _ = writeln!(e.out, "  store {} {}, ptr {}", lty, v, addr);
        }
        Instr::LoadGlobal { dst, global, ty } => {
            let lty = scalar_ty(e.lc.tys, *ty);
            let v = e.fresh();
            let _ = writeln!(
                e.out,
                "  {} = load {}, ptr {}",
                v,
                lty,
                fun_name(e.lc, *global)
            );
            e.store_val(*dst, *ty, v);
        }
        Instr::StoreGlobal { global, src, ty } => {
            let v = e.op_val(src, *ty);
            let lty = scalar_ty(e.lc.tys, *ty);
            let _ = writeln!(
                e.out,
                "  store {} {}, ptr {}",
                lty,
                v,
                fun_name(e.lc, *global)
            );
        }
        Instr::Slot { .. } => {
            // 纯声明：alloca 已在入口按分类发出
        }
        Instr::Param { a, ty } => {
            let pty = if class_of(e.lc.tys, *ty) == TyClass::Agg {
                "ptr".to_string()
            } else {
                scalar_ty(e.lc.tys, *ty)
            };
            let v = e.op_val(a, *ty);
            pending.push((v, pty));
        }
        Instr::Call { dst, callee, ty } => {
            let args: Vec<String> = pending
                .drain(..)
                .map(|(v, t)| format!("{} {}", t, v))
                .collect();
            let (callee_name, ret) = match callee {
                Callee::Def(d) => {
                    let ret = if class_of(e.lc.tys, *ty) == TyClass::Agg {
                        "ptr".to_string()
                    } else {
                        scalar_ty(e.lc.tys, *ty)
                    };
                    (fun_name(e.lc, *d), ret)
                }
                Callee::Runtime(RuntimeFn::StrConcat) => {
                    e.g.runtime_decls
                        .entry("starry_str_concat".into())
                        .or_insert(("i8*".into(), "i8*, i8*".into()));
                    ("@starry_str_concat".to_string(), "i8*".to_string())
                }
                Callee::Runtime(RuntimeFn::StrConv) => {
                    let pty = runtime_param_ty(&args);
                    let name = format!("starry_str_conv_{}", runtime_suffix(&pty));
                    e.g.runtime_decls
                        .entry(name.clone())
                        .or_insert(("i8*".into(), pty));
                    (format!("@{name}"), "i8*".to_string())
                }
                Callee::Runtime(RuntimeFn::Print) => {
                    let pty = runtime_param_ty(&args);
                    let name = format!("starry_print_{}", runtime_suffix(&pty));
                    e.g.runtime_decls
                        .entry(name.clone())
                        .or_insert(("void".into(), pty));
                    (format!("@{name}"), "void".to_string())
                }
            };
            if ret == "void" {
                let _ = writeln!(e.out, "  call void {}({})", callee_name, args.join(", "));
            } else {
                let v = e.fresh();
                let _ = writeln!(
                    e.out,
                    "  {} = call {} {}({})",
                    v,
                    ret,
                    callee_name,
                    args.join(", ")
                );
                if let Some(d) = dst {
                    e.store_val(*d, *ty, v);
                }
            }
        }
    }
}

fn runtime_param_ty(args: &[String]) -> String {
    args.first()
        .map(|a| a.split_whitespace().next().unwrap_or("i64").to_string())
        .unwrap_or_else(|| "i64".into())
}

fn runtime_suffix(pty: &str) -> String {
    match pty {
        "i8*" | "i8_" | "string" => "string".to_string(),
        "double" => "f64".to_string(),
        "float" => "f32".to_string(),
        _ => sanitize(pty),
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn emit_term(e: &mut FnEmit, term: &Term) {
    match term {
        Term::Jmp(b) => {
            let _ = writeln!(e.out, "  br label %{}", block_label(b.0));
        }
        Term::Br { cond, then_, else_ } => {
            let c = e.op_val(cond, e.lc.tys.bool_);
            let _ = writeln!(
                e.out,
                "  br i1 {}, label %{}, label %{}",
                c,
                block_label(then_.0),
                block_label(else_.0)
            );
        }
        Term::Ret { value, ty } => match value {
            Some(v) => {
                if class_of(e.lc.tys, *ty) == TyClass::Agg {
                    let p = e.op_val(v, *ty);
                    let _ = writeln!(e.out, "  ret ptr {}", p);
                } else {
                    let val = e.op_val(v, *ty);
                    let _ = writeln!(e.out, "  ret {} {}", scalar_ty(e.lc.tys, *ty), val);
                }
            }
            None => {
                let _ = writeln!(e.out, "  ret void");
            }
        },
        Term::Unreachable => {
            let _ = writeln!(e.out, "  unreachable");
        }
    }
}

fn emit_main_wrapper(lc: &Lc, tac: &TacOutput, body: &mut String) {
    for fun in &tac.funs {
        let def = lc.defs.get(fun.def);
        if lc.interner.resolve(def.name) != "main" || !fun.params.is_empty() {
            continue;
        }
        let ret = if class_of(lc.tys, fun.ret) == TyClass::Agg {
            "ptr".to_string()
        } else {
            scalar_ty(lc.tys, fun.ret)
        };
        let _ = writeln!(body, "define i32 @main() {{");
        let mut next_global_temp = 0usize;
        for global in &tac.globals {
            if global.init.is_some() {
                let gty = if scalar_ty(lc.tys, global.ty) == "i8*" {
                    "i8*".to_string()
                } else {
                    scalar_ty(lc.tys, global.ty)
                };
                let init = format!("@__starry_init_global.{}", global.def.0);
                let v = format!("%u{next_global_temp}");
                next_global_temp += 1;
                let _ = writeln!(body, "  {} = call {} {}()", v, gty, init);
                let _ = writeln!(
                    body,
                    "  store {} {}, ptr {}",
                    gty,
                    v,
                    fun_name(lc, global.def)
                );
            }
        }
        let _ = writeln!(body, "  call {} {}()", ret, fun_name(lc, fun.def));
        let _ = writeln!(body, "  ret i32 0");
        let _ = writeln!(body, "}}");
        let _ = writeln!(body);
        return;
    }
}
