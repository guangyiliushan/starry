//! 模块/函数发射：两遍（temp 存储分类 → 指令发射）。
//!
//! 存储分类（每个 temp 恰好一个 entry alloca，外部 mem2reg 可提升）：
//! - Cell：i64 槽（标量与单槽窄聚合共用）
//! - WideCell：i128 槽（i128 标量；16 字节）
//! - Block(n)：`[n x i64]` 块（不含 i128 的聚合基址）
//! - WideBlock(n)：`[n x i128]` 块（含 i128 成员的聚合；按类型加宽）
//! - PtrCell(n)/WidePtrCell(n)：`ptr` 槽装聚合指针（聚合实参/返回值）
//!
//! 不透明指针风格（LLVM 15+）：无指针 bitcast，load/store/ret 聚合
//! 经 `ptr`；槽寻址统一 `getelementptr [n x E], ptr p, i64 0, i64 i`
//! （E = i64 或 i128，Cell 即自身，idx 恒 0；tag/flag 是槽 0 里的 i8，
//! 写时就地 `store i8 ..., ptr` —— 天然免 bitcast）。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ast::interner::{Interner, Symbol};
use irc::layout::scalar_slots;
use irc::tac::{
    BinOp, Callee, Const, FunBody, Instr, Operand, RuntimeFn, TacOutput, TempId, Term, UnOp,
};
use sem::{DefArena, DefId, DefKind, FloatKind, IntKind, TyArena, TyId, TyKind};

use crate::ty::{Lc, TyClass, class_of, float_lit, fun_name, scalar_ty};

pub fn emit_module(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &mut Interner,
) -> String {
    let mut g = Global::default();
    // M1 (D2 contract): pre-intern every emitted function name; the checked
    // trap message carries the function name (span approximation). Interning
    // must happen here because Lc holds a shared &Interner afterwards.
    for fun in tac
        .funs
        .iter()
        .chain(tac.globals.iter().filter_map(|gl| gl.init.as_ref()))
    {
        let def = fun.def;
        let name = format!(
            "{}.{}",
            sanitize(interner.resolve(defs.get(def).name)),
            def.0
        );
        let sym = interner.intern(&name);
        let id = g.str_id(sym);
        g.fun_str.insert(def, (id, name.len() + 1));
    }
    let lc = Lc {
        tys,
        defs,
        interner,
    };
    let mut body = String::new();
    let mut defined = Vec::new();
    for fun in &tac.funs {
        defined.push(fun.def);
        emit_fun(&lc, &mut g, fun, &mut body);
    }
    emit_main_wrapper(&lc, tac, &mut body);
    // 顶层全局存储声明（init 函数可能引用其他全局，先声明存储）
    let mut globals = String::new();
    for global in &tac.globals {
        let name = fun_name(&lc, global.def);
        let storage_ty = if scalar_ty(lc.tys, lc.defs, global.ty) == "i8*" {
            "i8*"
        } else if contains_i128(&lc, global.ty) {
            "i128"
        } else {
            "i64"
        };
        let zero = if storage_ty == "i8*" { "null" } else { "0" };
        let _ = writeln!(globals, "{} = global {} {}", name, storage_ty, zero);
    }
    // 全局初始化函数：必须在字符串常量 flush 之前发射——init 表达式里
    // 新 intern 的字符串只在此时注册，提前 flush 会输出未定义的 @.str.N。
    for global in &tac.globals {
        if let Some(init) = &global.init {
            emit_fun_named(
                &lc,
                &mut g,
                init,
                &mut body,
                format!("@__starry_init_global.{}", global.def.0),
            );
        }
    }
    // 外部函数 / runtime declare
    let mut decls = String::new();
    for (name, (ret, params)) in std::mem::take(&mut g.extern_decls) {
        let _ = writeln!(decls, "declare {} @{}({})", ret, name, params);
    }
    for (name, (ret, params)) in std::mem::take(&mut g.runtime_decls) {
        if name == "starry_overflow_panic" {
            // M1 (D2 contract): noreturn via attribute group (IR-valid form)
            let _ = writeln!(decls, "declare {} @{}({}) #0", ret, name, params);
        } else {
            let _ = writeln!(decls, "declare {} @{}({})", ret, name, params);
        }
    }
    if g.used_ovf_panic {
        let _ = writeln!(decls, "attributes #0 = {{ noreturn }}");
    }

    let mut out = String::new();
    let used_strs = g.used_strs.clone();
    for (sym, id) in &g.strings {
        if !used_strs.contains(id) {
            continue;
        }
        let text = lc.interner.resolve(*sym);
        let _ = writeln!(
            out,
            "@.str.{} = private constant [{} x i8] c\"{}\\00\"",
            id,
            text.len() + 1,
            escape_cstr(text)
        );
    }
    out.push_str(&globals);
    out.push_str(&decls);
    out.push_str(&body);
    let _ = defined;
    out
}

#[derive(Default)]
struct Global {
    strings: BTreeMap<Symbol, usize>,
    runtime_decls: BTreeMap<String, (String, String)>,
    extern_decls: BTreeMap<String, (String, String)>,
    /// M1 (D2): def to (@.str id, byte len incl NUL) for trap messages
    fun_str: BTreeMap<DefId, (usize, usize)>,
    /// M1 (D2): starry_overflow_panic was referenced (emit noreturn attrs)
    used_ovf_panic: bool,
    /// M1 (D2): @.str ids actually referenced (unused fun-name strings are
    /// not emitted, keeping .ll goldens stable)
    used_strs: std::collections::BTreeSet<usize>,
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
    /// i64 槽（普通标量/单槽窄聚合）
    Cell,
    /// i128 槽（i128 标量；16 字节）
    WideCell,
    /// [n x i64] 块（不含 i128 的聚合）
    Block(u32),
    /// [n x i128] 块（含 i128 成员的聚合；按类型加宽，避免所有槽翻倍）
    WideBlock(u32),
    /// ptr 槽装聚合指针（聚合实参/返回值），n = 指向块的槽数
    PtrCell(u32),
    /// 同 PtrCell，但指向 [n x i128] 块
    WidePtrCell(u32),
}

/// 类型是否含 i128（决定聚合块/槽是否加宽到 16 字节）
fn contains_i128(lc: &Lc, ty: TyId) -> bool {
    match lc.tys.kind(ty) {
        TyKind::Int(IntKind::I128) => true,
        TyKind::Array(e, _) | TyKind::Nullable(e) | TyKind::Range(e) => contains_i128(lc, *e),
        TyKind::Tuple(es) => es.iter().any(|e| contains_i128(lc, *e)),
        TyKind::Named(def, _) => match lc.defs.get(*def).kind {
            sem::DefKind::Struct | sem::DefKind::Class => lc
                .defs
                .get(*def)
                .sig
                .fields
                .iter()
                .any(|(_, t)| contains_i128(lc, *t)),
            sem::DefKind::Enum => lc
                .defs
                .get(*def)
                .sig
                .variants
                .iter()
                .any(|(_, ps)| ps.iter().any(|(_, t)| contains_i128(lc, *t))),
            _ => false,
        },
        _ => false,
    }
}

impl Storage {
    fn slots(&self) -> u32 {
        match self {
            Storage::Block(n) | Storage::WideBlock(n) => *n,
            Storage::PtrCell(n) | Storage::WidePtrCell(n) => *n,
            _ => 1,
        }
    }
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
                Instr::AddrOf { dst, base, .. } => {
                    max = max.max(dst.0).max(base.0);
                }
                Instr::LoadPtr { dst, ptr, .. } => {
                    max = max.max(dst.0).max(ptr.0);
                }
                Instr::OffsetPtr { dst, base, .. } => {
                    max = max.max(dst.0).max(base.0);
                }
                Instr::AllocClass { dst, .. } => {
                    max = max.max(dst.0);
                }
                Instr::StorePtr { ptr, src, .. } => {
                    max = max.max(ptr.0);
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
    let mut wide: Vec<bool> = vec![false; n];
    for (t, ty) in &fun.params {
        if class_of(lc.tys, lc.defs, *ty) == TyClass::Agg {
            ptr[t.0 as usize] = Some(scalar_slots(lc, *ty).max(1) as u32);
        }
        if contains_i128(lc, *ty) {
            wide[t.0 as usize] = true;
        }
    }
    let mark = |t: TempId, ty: TyId, wide: &mut [bool]| {
        if contains_i128(lc, ty) {
            wide[t.0 as usize] = true;
        }
    };
    for b in &fun.blocks {
        for i in &b.instrs {
            match i {
                Instr::Bin { dst, ty, .. } | Instr::Un { dst, ty, .. } => {
                    mark(*dst, *ty, &mut wide);
                }
                Instr::Copy { dst, src, ty } => {
                    mark(*dst, *ty, &mut wide);
                    if let Operand::Temp(t) = src {
                        mark(*t, *ty, &mut wide);
                    }
                }
                Instr::Slot { dst, count } => {
                    req[dst.0 as usize] = req[dst.0 as usize].max(*count as usize)
                }
                Instr::Load { dst, base, idx, ty } => {
                    mark(*dst, *ty, &mut wide);
                    mark(*base, *ty, &mut wide);
                    if let Operand::Const(Const::Int(k)) = idx {
                        req[base.0 as usize] = req[base.0 as usize].max(*k as usize + 1);
                    }
                }
                Instr::Store { base, idx, src, ty } => {
                    mark(*base, *ty, &mut wide);
                    if let Operand::Temp(t) = src {
                        mark(*t, *ty, &mut wide);
                    }
                    if let Operand::Const(Const::Int(k)) = idx {
                        req[base.0 as usize] = req[base.0 as usize].max(*k as usize + 1);
                    }
                }
                Instr::AddrOf { .. }
                | Instr::LoadPtr { .. }
                | Instr::StorePtr { .. }
                | Instr::OffsetPtr { .. }
                | Instr::AllocClass { .. } => {}
                Instr::LoadGlobal { dst, ty, .. } => mark(*dst, *ty, &mut wide),
                Instr::StoreGlobal { src, ty, .. } => {
                    if let Operand::Temp(t) = src {
                        mark(*t, *ty, &mut wide);
                    }
                }
                Instr::Param { a, ty } => {
                    if let Operand::Temp(t) = a {
                        mark(*t, *ty, &mut wide);
                    }
                }
                Instr::Call { dst, ty, .. } => {
                    if let Some(d) = dst
                        && class_of(lc.tys, lc.defs, *ty) == TyClass::Agg
                    {
                        // 聚合返回值在调用方分配块并整体拷入：callee 的
                        // 返回值只是 callee 栈槽地址，直接持有会悬垂。
                        req[d.0 as usize] = req[d.0 as usize].max(scalar_slots(lc, *ty).max(1));
                        mark(*d, *ty, &mut wide);
                    }
                }
            }
        }
        if let Term::Ret {
            value: Some(Operand::Temp(t)),
            ty,
        } = &b.term
        {
            if class_of(lc.tys, lc.defs, *ty) == TyClass::Agg {
                req[t.0 as usize] = req[t.0 as usize].max(scalar_slots(lc, *ty));
            }
            mark(*t, *ty, &mut wide);
        }
    }
    (0..n)
        .map(|i| match ptr[i] {
            Some(n) if wide[i] => Storage::WidePtrCell(n),
            Some(n) => Storage::PtrCell(n),
            None if req[i] > 1 && wide[i] => Storage::WideBlock(req[i] as u32),
            None if req[i] > 1 => Storage::Block(req[i] as u32),
            _ if wide[i] => Storage::WideCell,
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
    /// M1（D2 契约）：当前函数名——受检 trap 消息携带（函数名即源位置近似）
    fun: (usize, usize),
}

impl<'e, 'l> FnEmit<'e, 'l> {
    fn fresh(&mut self) -> String {
        let s = format!("%u{}", self.u);
        self.u += 1;
        s
    }

    /// temp 的槽地址（ptr 值，指向槽元素）
    fn slot_addr(&mut self, t: TempId, idx: &Operand) -> String {
        let st = self.st[t.0 as usize];
        let (base, elem) = match st {
            Storage::Cell | Storage::WideCell => return format!("%t{}.slot", t.0),
            Storage::Block(_) => (format!("%t{}.slot", t.0), "i64"),
            Storage::WideBlock(_) => (format!("%t{}.slot", t.0), "i128"),
            Storage::PtrCell(_) => {
                let p = self.fresh();
                let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                (p, "i64")
            }
            Storage::WidePtrCell(_) => {
                let p = self.fresh();
                let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                (p, "i128")
            }
        };
        let n = st.slots();
        let i = self.op_val(idx, self.lc.tys.default_int);
        let ity = scalar_ty(self.lc.tys, self.lc.defs, self.lc.tys.default_int);
        let gep = self.fresh();
        let _ = writeln!(
            self.out,
            "  {} = getelementptr [{} x {}], ptr {}, i64 0, {} {}",
            gep, n, elem, base, ity, i
        );
        gep
    }

    /// 常量/临时量的值文本（标量按 ty 加载；聚合返回指针值）
    fn op_val(&mut self, op: &Operand, ty: TyId) -> String {
        match op {
            Operand::Temp(t) => {
                if class_of(self.lc.tys, self.lc.defs, ty) == TyClass::Agg {
                    // 聚合值 = 其块/指针的地址
                    match self.st[t.0 as usize] {
                        Storage::PtrCell(_) | Storage::WidePtrCell(_) => {
                            let p = self.fresh();
                            let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                            p
                        }
                        // Block/Cell 聚合值的当前槽地址；%tN 只在形参值域存在，
                        // 本地聚合必须经 %tN.slot（否则未定义 + 丢失 var 更新）。
                        _ => format!("%t{}.slot", t.0),
                    }
                } else {
                    let lty = scalar_ty(self.lc.tys, self.lc.defs, ty);
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
            Const::Int(v) => {
                // M4 slice 4a: ptr-typed target (class value | nullable class payload) 0 renders null
                if *v == 0 && scalar_ty(self.lc.tys, self.lc.defs, ty) == "ptr" {
                    "null".to_string()
                } else {
                    format!("{v}")
                }
            }
            Const::Float(v) => float_lit(
                *v,
                matches!(self.lc.tys.kind(ty), TyKind::Float(FloatKind::F32)),
            ),
            Const::Bool(b) => format!("{b}"),
            // 裸指针的 null = LLVM null（整数 0 作 ptr 操作数非法）
            Const::Null
                if matches!(self.lc.tys.kind(ty), TyKind::Pointer { .. })
                    || matches!(
                        self.lc.tys.kind(ty),
                        TyKind::Named(d, _) if matches!(self.lc.defs.get(*d).kind, DefKind::Class)
                    ) =>
            {
                "null".into()
            }
            Const::Null => "0".into(),
            Const::Unit => "0".into(),
            // M8 slice 1：函数引用 = 全局函数指针
            Const::Fun(d) => fun_name(self.lc, *d),
            Const::Str(sym) => {
                let id = self.g.str_id(*sym);
                self.g.used_strs.insert(id);
                let len = self.lc.interner.resolve(*sym).len() + 1;
                format!("getelementptr ([{len} x i8], ptr @.str.{id}, i64 0, i64 0)")
            }
        }
    }

    /// 把值写入 temp 的槽（按 ty；Cell/Block 第一槽）
    fn store_val(&mut self, t: TempId, ty: TyId, val: String) {
        match self.st[t.0 as usize] {
            Storage::PtrCell(_) | Storage::WidePtrCell(_) => {
                let _ = writeln!(self.out, "  store ptr {}, ptr %t{}.slot", val, t.0);
            }
            _ => {
                let lty = scalar_ty(self.lc.tys, self.lc.defs, ty);
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
    let ret_ty = if class_of(lc.tys, lc.defs, fun.ret) == TyClass::Agg {
        "ptr".to_string()
    } else {
        scalar_ty(lc.tys, lc.defs, fun.ret)
    };
    let params: Vec<String> = fun
        .params
        .iter()
        .map(|(t, ty)| {
            let pty = if class_of(lc.tys, lc.defs, *ty) == TyClass::Agg {
                "ptr".to_string()
            } else {
                scalar_ty(lc.tys, lc.defs, *ty)
            };
            format!("{} %t{}", pty, t.0)
        })
        .collect();
    let _ = writeln!(body, "define {} {}({}) {{", ret_ty, name, params.join(", "));

    // entry allocas
    let fun_str = *g.fun_str.get(&fun.def).unwrap_or_else(|| {
        panic!(
            "fun name pre-interned (M1 D2): def {:?} name {:?}",
            fun.def,
            lc.interner.resolve(lc.defs.get(fun.def).name)
        )
    });
    let mut e = FnEmit {
        lc,
        g,
        st: &st,
        out: body,
        u: 0,
        fun: fun_str,
    };
    for (i, s) in st.iter().enumerate() {
        match s {
            Storage::Cell => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca i64", i);
            }
            Storage::WideCell => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca i128", i);
            }
            Storage::Block(n) => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca [{} x i64]", i, n);
            }
            Storage::WideBlock(n) => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca [{} x i128]", i, n);
            }
            Storage::PtrCell(_) | Storage::WidePtrCell(_) => {
                let _ = writeln!(e.out, "  %t{}.slot = alloca ptr", i);
            }
        }
    }
    // 形参是 %tN 值；存入各自槽
    for (t, ty) in &fun.params {
        if class_of(lc.tys, lc.defs, *ty) == TyClass::Agg {
            let _ = writeln!(e.out, "  store ptr %t{}, ptr %t{}.slot", t.0, t.0);
        } else {
            let lty = scalar_ty(lc.tys, lc.defs, *ty);
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

/// M1（Q5）：受检整型加减乘——llvm.s{add,sub,mul}.with.overflow 序列，
/// 溢出分支 call starry_overflow_panic + unreachable，正常路径落入新续标签
/// （后续指令顺延；槽式存储无 phi，无需修补）
/// M1 (D2/D4 contract): checked trap call. noreturn decl; the message
/// carries the op code and the current function name (span approximation).
fn emit_panic(e: &mut FnEmit, code: i8) {
    e.g.used_ovf_panic = true;
    e.g.runtime_decls
        .entry("starry_overflow_panic".into())
        .or_insert(("void".into(), "i8, ptr".into()));
    let (id, len) = e.fun;
    e.g.used_strs.insert(id);
    let _ = writeln!(
        e.out,
        "  call void @starry_overflow_panic(i8 {}, ptr getelementptr ([{} x i8], ptr @.str.{}, i64 0, i64 0))",
        code, len, id
    );
    let _ = writeln!(e.out, "  unreachable");
}

fn emit_checked_arith(e: &mut FnEmit, dst: TempId, op: BinOp, ty: TyId, lv: &str, rv: &str) {
    let tname = scalar_ty(e.lc.tys, e.lc.defs, ty);
    let intrinsic = match op {
        BinOp::Add => "llvm.sadd.with.overflow",
        BinOp::Sub => "llvm.ssub.with.overflow",
        _ => "llvm.smul.with.overflow",
    };
    let code = match op {
        BinOp::Add => 1,
        BinOp::Sub => 2,
        _ => 3,
    };
    e.g.runtime_decls
        .entry("starry_overflow_panic".into())
        .or_insert(("void".into(), "i8, ptr".into()));
    let pair = e.fresh();
    let _ = writeln!(
        e.out,
        "  {} = call {{ {}, i1 }} @{}.{}({} {}, {} {})",
        pair, tname, intrinsic, tname, tname, lv, tname, rv
    );
    let ov = e.fresh();
    let _ = writeln!(
        e.out,
        "  {} = extractvalue {{ {}, i1 }} {}, 1",
        ov, tname, pair
    );
    let lp = e.fresh();
    let lc = e.fresh();
    let _ = writeln!(e.out, "  br i1 {}, label {}, label {}", ov, lp, lc);
    let _ = writeln!(e.out, "{}:", &lp[1..]);
    emit_panic(e, code);
    let _ = writeln!(e.out, "{}:", &lc[1..]);
    let res = e.fresh();
    let _ = writeln!(
        e.out,
        "  {} = extractvalue {{ {}, i1 }} {}, 0",
        res, tname, pair
    );
    e.store_val(dst, ty, res);
}

/// M1（Q5）：受检整型除法——sdiv/srem 前显式守卫除零与 INT_MIN ÷ -1
/// （LLVM 对两者是 poison/硬件异常，统一为可读的 overflow panic）
fn emit_checked_div(e: &mut FnEmit, dst: TempId, op: BinOp, ty: TyId, lv: &str, rv: &str) {
    let tname = scalar_ty(e.lc.tys, e.lc.defs, ty);
    e.g.runtime_decls
        .entry("starry_overflow_panic".into())
        .or_insert(("void".into(), "i8, ptr".into()));
    let zero_code = if op == BinOp::Div { 4 } else { 6 };
    let min_code = if op == BinOp::Div { 5 } else { 7 };
    // i4 以 i8 存储，守卫常量按语义位宽取最小值
    let min_text = match e.lc.tys.kind(ty) {
        TyKind::Int(IntKind::I4) => "-8",
        TyKind::Int(IntKind::I8) => "-128",
        TyKind::Int(IntKind::I16) => "-32768",
        TyKind::Int(IntKind::I32) => "-2147483648",
        TyKind::Int(IntKind::I64) => "-9223372036854775808",
        _ => "-170141183460469231731687303715884105728",
    };
    let z = e.fresh();
    let _ = writeln!(e.out, "  {} = icmp eq {} {}, 0", z, tname, rv);
    let at_min = e.fresh();
    let _ = writeln!(
        e.out,
        "  {} = icmp eq {} {}, {}",
        at_min, tname, lv, min_text
    );
    let neg1 = e.fresh();
    let _ = writeln!(e.out, "  {} = icmp eq {} {}, -1", neg1, tname, rv);
    // D4: zero guard first (code 4/6), then INT_MIN/-1 (code 5/7)
    let lz = e.fresh();
    let lzc = e.fresh();
    let _ = writeln!(e.out, "  br i1 {}, label {}, label {}", z, lz, lzc);
    let _ = writeln!(e.out, "{}:", &lz[1..]);
    emit_panic(e, zero_code);
    let _ = writeln!(e.out, "{}:", &lzc[1..]);
    let both = e.fresh();
    let _ = writeln!(e.out, "  {} = and i1 {}, {}", both, at_min, neg1);
    let lm = e.fresh();
    let lmc = e.fresh();
    let _ = writeln!(e.out, "  br i1 {}, label {}, label {}", both, lm, lmc);
    let _ = writeln!(e.out, "{}:", &lm[1..]);
    emit_panic(e, min_code);
    let _ = writeln!(e.out, "{}:", &lmc[1..]);
    let v = e.fresh();
    let mnemonic = if op == BinOp::Div { "sdiv" } else { "srem" };
    let _ = writeln!(e.out, "  {} = {} {} {}, {}", v, mnemonic, tname, lv, rv);
    e.store_val(dst, ty, v);
}
fn emit_instr(e: &mut FnEmit, pending: &mut Vec<(String, String)>, instr: &Instr) {
    match instr {
        Instr::Bin { dst, op, l, r, ty } => {
            let lv = e.op_val(l, *ty);
            let rv = e.op_val(r, *ty);
            // bin_llvm 自管 fresh：内联实现（借用冲突回避）
            let is_float = matches!(e.lc.tys.kind(*ty), TyKind::Float(_));
            // M1（Q5）：整型 + - * 走 llvm.*.with.overflow 受检序列，
            // 除法先显式守卫除零/INT_MIN÷-1；+% -% *% 与浮点保持原路
            if !is_float && matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
                emit_checked_arith(e, *dst, *op, *ty, &lv, &rv);
                return;
            }
            if !is_float && matches!(op, BinOp::Div | BinOp::Rem) {
                emit_checked_div(e, *dst, *op, *ty, &lv, &rv);
                return;
            }
            let tname = scalar_ty(e.lc.tys, e.lc.defs, *ty);
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
                // M1（Q5）：wrapping 变体发裸指令（回绕 = LLVM 原生语义）
                BinOp::WrappingAdd => "add",
                BinOp::WrappingSub => "sub",
                BinOp::WrappingMul => "mul",
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
                        let _ = writeln!(
                            e.out,
                            "  {} = fneg {} {}",
                            v,
                            scalar_ty(e.lc.tys, e.lc.defs, *ty),
                            av
                        );
                    } else {
                        let _ = writeln!(
                            e.out,
                            "  {} = sub {} 0, {}",
                            v,
                            scalar_ty(e.lc.tys, e.lc.defs, *ty),
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
                        scalar_ty(e.lc.tys, e.lc.defs, *ty),
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
                        scalar_ty(e.lc.tys, e.lc.defs, *from),
                        av,
                        scalar_ty(e.lc.tys, e.lc.defs, *ty)
                    );
                }
            }
            e.store_val(*dst, *ty, v);
        }
        Instr::Copy { dst, src, ty } if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Scalar => {
            let v = e.op_val(src, *ty);
            e.store_val(*dst, *ty, v);
        }
        Instr::Copy { dst, src, ty } => {
            // 聚合拷贝：逐槽搬运；槽元素宽度按类型（含 i128 → [n x i128]）
            let n = scalar_slots(e.lc, *ty).max(1);
            let elem = if contains_i128(e.lc, *ty) {
                "i128"
            } else {
                "i64"
            };
            match src {
                Operand::Temp(_) => {
                    let src_ptr = e.op_val(src, *ty);
                    let dst_ptr = e.op_val(&Operand::Temp(*dst), *ty);
                    for i in 0..n {
                        let sa = e.fresh();
                        let _ = writeln!(
                            e.out,
                            "  {} = getelementptr [{} x {}], ptr {}, i64 0, i64 {}",
                            sa, n, elem, src_ptr, i
                        );
                        let v = e.fresh();
                        let _ = writeln!(e.out, "  {} = load {}, ptr {}", v, elem, sa);
                        let da = e.fresh();
                        let _ = writeln!(
                            e.out,
                            "  {} = getelementptr [{} x {}], ptr {}, i64 0, i64 {}",
                            da, n, elem, dst_ptr, i
                        );
                        let _ = writeln!(e.out, "  store {} {}, ptr {}", elem, v, da);
                    }
                }
                Operand::Const(_) => {
                    let v = e.op_val(src, *ty);
                    e.store_val(*dst, *ty, v);
                }
            }
        }
        // M2 取址：base 的 alloca 地址本身就是 ptr 值
        Instr::AddrOf { dst, base, ty } => {
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let _ = writeln!(
                e.out,
                "  store {} %t{}.slot, ptr %t{}.slot",
                lty, base.0, dst.0
            );
        }
        // M2 经指针读：槽里取地址，再按 ty 读
        Instr::LoadPtr { dst, ptr, ty } => {
            let p = e.fresh();
            let _ = writeln!(e.out, "  {} = load ptr, ptr %t{}.slot", p, ptr.0);
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let v = e.fresh();
            let _ = writeln!(e.out, "  {} = load {}, ptr {}", v, lty, p);
            let _ = writeln!(e.out, "  store {} {}, ptr %t{}.slot", lty, v, dst.0);
        }
        // M2 经指针写
        Instr::StorePtr { ptr, src, ty } => {
            let p = e.fresh();
            let _ = writeln!(e.out, "  {} = load ptr, ptr %t{}.slot", p, ptr.0);
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let v = e.op_val(src, *ty);
            let _ = writeln!(e.out, "  store {} {}, ptr {}", lty, v, p);
        }
        // M4 slice 2b: class alloc -- deinit fn ptr (or null) goes to header meta
        Instr::AllocClass {
            dst, bytes, deinit, ..
        } => {
            e.g.runtime_decls
                .entry("starry_alloc_object".into())
                .or_insert(("ptr".into(), "i64, ptr".into()));
            let d = match deinit {
                Some(d) => fun_name(e.lc, *d),
                None => "null".to_string(),
            };
            let v = e.fresh();
            let _ = writeln!(
                e.out,
                "  {} = call ptr @starry_alloc_object(i64 {}, ptr {})",
                v, bytes, d
            );
            let _ = writeln!(e.out, "  store ptr {}, ptr %t{}.slot", v, dst.0);
        } // M4 slice 1: object field addressing (GEP i8, const byte offset)
        Instr::OffsetPtr {
            dst, base, bytes, ..
        } => {
            let p = e.fresh();
            let _ = writeln!(e.out, "  {} = load ptr, ptr %t{}.slot", p, base.0);
            let v = e.fresh();
            let _ = writeln!(
                e.out,
                "  {} = getelementptr i8, ptr {}, i64 {}",
                v, p, bytes
            );
            let _ = writeln!(e.out, "  store ptr {}, ptr %t{}.slot", v, dst.0);
        }
        Instr::Load { dst, base, idx, ty } => {
            let addr = e.slot_addr(*base, idx);
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let v = e.fresh();
            let _ = writeln!(e.out, "  {} = load {}, ptr {}", v, lty, addr);
            e.store_val(*dst, *ty, v);
        }
        Instr::Store { base, idx, src, ty } => {
            let addr = e.slot_addr(*base, idx);
            let v = e.op_val(src, *ty);
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let _ = writeln!(e.out, "  store {} {}, ptr {}", lty, v, addr);
        }
        Instr::LoadGlobal { dst, global, ty } => {
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
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
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
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
            let pty = if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Agg {
                "ptr".to_string()
            } else {
                scalar_ty(e.lc.tys, e.lc.defs, *ty)
            };
            let v = e.op_val(a, *ty);
            pending.push((v, pty));
        }
        Instr::Call { dst, callee, ty } => {
            let mut args: Vec<String> = pending
                .drain(..)
                .map(|(v, t)| format!("{} {}", t, v))
                .collect();
            let mut is_str_eq = false;
            let (callee_name, ret) = match callee {
                // M8 slice 1：函数值间接调用——args[0] 为函数指针
                // （"ptr %tN"），去掉类型前缀放在被调位；
                // 形参签名由其余 pending 的 pty 携带
                Callee::Indirect => {
                    if args.is_empty() {
                        let _ =
                            writeln!(e.out, "  ; ERROR: indirect call without a function pointer");
                        ("@starry_indirect_missing".to_string(), "void".to_string())
                    } else {
                        let target = args.remove(0);
                        let target_val = target.strip_prefix("ptr ").unwrap_or(&target).to_string();
                        let ret = if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Agg {
                            "ptr".to_string()
                        } else {
                            scalar_ty(e.lc.tys, e.lc.defs, *ty)
                        };
                        (target_val, ret)
                    }
                }
                Callee::Def(d) => {
                    let ret = if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Agg {
                        "ptr".to_string()
                    } else {
                        scalar_ty(e.lc.tys, e.lc.defs, *ty)
                    };
                    (fun_name(e.lc, *d), ret)
                }
                Callee::Extern(d) => {
                    let (name, ret_ty, params) = {
                        let def = e.lc.defs.get(*d);
                        let ret_ty = if class_of(e.lc.tys, e.lc.defs, def.sig.ret) == TyClass::Agg {
                            "ptr".to_string()
                        } else {
                            scalar_ty(e.lc.tys, e.lc.defs, def.sig.ret)
                        };
                        let params: Vec<String> = def
                            .sig
                            .params
                            .iter()
                            .map(|(_, pty)| {
                                if class_of(e.lc.tys, e.lc.defs, *pty) == TyClass::Agg {
                                    "ptr".to_string()
                                } else {
                                    scalar_ty(e.lc.tys, e.lc.defs, *pty)
                                }
                            })
                            .collect();
                        (
                            sanitize(e.lc.interner.resolve(def.name)),
                            ret_ty,
                            params.join(", "),
                        )
                    };
                    e.g.extern_decls
                        .entry(name.clone())
                        .or_insert((ret_ty.clone(), params));
                    (format!("@{name}"), ret_ty)
                }
                Callee::Runtime(RuntimeFn::StrConcat) => {
                    e.g.runtime_decls
                        .entry("starry_str_concat".into())
                        .or_insert(("i8*".into(), "i8*, i8*".into()));
                    ("@starry_str_concat".to_string(), "i8*".to_string())
                }
                Callee::Runtime(RuntimeFn::StrEq) => {
                    e.g.runtime_decls
                        .entry("starry_str_eq".into())
                        .or_insert(("i32".into(), "i8*, i8*".into()));
                    is_str_eq = true;
                    ("@starry_str_eq".to_string(), "i32".to_string())
                }
                Callee::Runtime(RuntimeFn::StrConv) => {
                    let pty = runtime_param_ty(&args);
                    if pty == "i128" {
                        // Windows/MSVC ABI 下 C 的 __int128 与 IR i128 传参
                        // 不一致（实测段错误）：拆成 hi/lo 两个 i64 传。
                        args = split_i128(e, &args);
                        e.g.runtime_decls
                            .entry("starry_str_conv_i128".into())
                            .or_insert(("i8*".into(), "i64, i64".into()));
                        ("@starry_str_conv_i128".to_string(), "i8*".to_string())
                    } else if pty == "i1" {
                        widen_bool(e, &mut args);
                        e.g.runtime_decls
                            .entry("starry_str_conv_i1".into())
                            .or_insert(("i8*".into(), "i32".into()));
                        ("@starry_str_conv_i1".to_string(), "i8*".to_string())
                    } else {
                        let pty = narrow_i4(e, &mut args, pty);
                        let name = format!("starry_str_conv_{}", runtime_suffix(&pty));
                        e.g.runtime_decls
                            .entry(name.clone())
                            .or_insert(("i8*".into(), pty));
                        (format!("@{name}"), "i8*".to_string())
                    }
                }
                Callee::Runtime(RuntimeFn::StrConvChar) => {
                    e.g.runtime_decls
                        .entry("starry_str_conv_char".into())
                        .or_insert(("i8*".into(), "i32".into()));
                    ("@starry_str_conv_char".to_string(), "i8*".to_string())
                }
                Callee::Runtime(RuntimeFn::Print) => {
                    let pty = runtime_param_ty(&args);
                    if pty == "i128" {
                        args = split_i128(e, &args);
                        e.g.runtime_decls
                            .entry("starry_print_i128".into())
                            .or_insert(("void".into(), "i64, i64".into()));
                        ("@starry_print_i128".to_string(), "void".to_string())
                    } else if pty == "i1" {
                        // C 入口是 int：i1 必须零扩展，否则高位的垃圾位会让
                        // `v ?` 把 false 打成 true（MSVC ABI 不保证清洁高位）。
                        widen_bool(e, &mut args);
                        e.g.runtime_decls
                            .entry("starry_print_i1".into())
                            .or_insert(("void".into(), "i32".into()));
                        ("@starry_print_i1".to_string(), "void".to_string())
                    } else {
                        let pty = narrow_i4(e, &mut args, pty);
                        let name = format!("starry_print_{}", runtime_suffix(&pty));
                        e.g.runtime_decls
                            .entry(name.clone())
                            .or_insert(("void".into(), pty));
                        (format!("@{name}"), "void".to_string())
                    }
                }
                Callee::Runtime(RuntimeFn::Retain) => {
                    e.g.runtime_decls
                        .entry("starry_retain".into())
                        .or_insert(("ptr".into(), "ptr".into()));
                    ("@starry_retain".to_string(), "ptr".to_string())
                }
                Callee::Runtime(RuntimeFn::Release) => {
                    e.g.runtime_decls
                        .entry("starry_release".into())
                        .or_insert(("void".into(), "ptr".into()));
                    ("@starry_release".to_string(), "void".to_string())
                }
                Callee::Runtime(RuntimeFn::WeakRetain) => {
                    e.g.runtime_decls
                        .entry("starry_weak_retain".into())
                        .or_insert(("ptr".into(), "ptr".into()));
                    ("@starry_weak_retain".to_string(), "ptr".to_string())
                }
                Callee::Runtime(RuntimeFn::WeakDrop) => {
                    e.g.runtime_decls
                        .entry("starry_weak_drop".into())
                        .or_insert(("void".into(), "ptr".into()));
                    ("@starry_weak_drop".to_string(), "void".to_string())
                }
                Callee::Runtime(RuntimeFn::WeakUpgrade) => {
                    e.g.runtime_decls
                        .entry("starry_weak_upgrade".into())
                        .or_insert(("ptr".into(), "ptr".into()));
                    ("@starry_weak_upgrade".to_string(), "ptr".to_string())
                }
                Callee::Runtime(RuntimeFn::PrintChar) => {
                    e.g.runtime_decls
                        .entry("starry_print_char".into())
                        .or_insert(("void".into(), "i32".into()));
                    ("@starry_print_char".to_string(), "void".to_string())
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
                if is_str_eq {
                    let c = e.fresh();
                    let _ = writeln!(e.out, "  {} = icmp ne i32 {}, 0", c, v);
                    if let Some(d) = dst {
                        e.store_val(*d, e.lc.tys.bool_, c);
                    }
                } else if let Some(d) = dst {
                    if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Agg {
                        // 聚合返回值：callee 栈槽 → 调用方块（逐槽拷贝）
                        let n = scalar_slots(e.lc, *ty).max(1);
                        let elem = if contains_i128(e.lc, *ty) {
                            "i128"
                        } else {
                            "i64"
                        };
                        let dst_ptr = e.op_val(&Operand::Temp(*d), *ty);
                        for i in 0..n {
                            let sa = e.fresh();
                            let _ = writeln!(
                                e.out,
                                "  {} = getelementptr [{} x {}], ptr {}, i64 0, i64 {}",
                                sa, n, elem, v, i
                            );
                            let val = e.fresh();
                            let _ = writeln!(e.out, "  {} = load {}, ptr {}", val, elem, sa);
                            let da = e.fresh();
                            let _ = writeln!(
                                e.out,
                                "  {} = getelementptr [{} x {}], ptr {}, i64 0, i64 {}",
                                da, n, elem, dst_ptr, i
                            );
                            let _ = writeln!(e.out, "  store {} {}, ptr {}", elem, val, da);
                        }
                    } else {
                        e.store_val(*d, *ty, v);
                    }
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

/// i4 实参在 C ABI 上没有对应类型：sext 到 i8 后按 i8 runtime 入口调用
///（print_i8/str_conv_i8 已存在），避免 i4 形参链接失败。
fn narrow_i4(e: &mut FnEmit, args: &mut [String], pty: String) -> String {
    if pty != "i4" || args.is_empty() {
        return pty;
    }
    let val = args[0].split_whitespace().nth(1).unwrap_or("0");
    let v = e.fresh();
    let _ = writeln!(e.out, "  {} = sext i4 {} to i8", v, val);
    args[0] = format!("i8 {}", v);
    "i8".into()
}

/// bool 实参按 C 的 int 传递（i1 直接传 i32 形参时高位不确定）
fn widen_bool(e: &mut FnEmit, args: &mut [String]) {
    if args.is_empty() {
        return;
    }
    let val = args[0].split_whitespace().nth(1).unwrap_or("0");
    let v = e.fresh();
    let _ = writeln!(e.out, "  {} = zext i1 {} to i32", v, val);
    args[0] = format!("i32 {}", v);
}

/// i128 实参拆成 (hi, lo) 两个 i64：MSVC ABI 的 __int128 与 LLVM i128
/// 传参不一致，跨 C runtime 边界必须走两个 64 位参数。
fn split_i128(e: &mut FnEmit, args: &[String]) -> Vec<String> {
    let v = args
        .first()
        .and_then(|a| a.split_whitespace().nth(1))
        .unwrap_or("0");
    let hi128 = e.fresh();
    let _ = writeln!(e.out, "  {} = lshr i128 {}, 64", hi128, v);
    let hi = e.fresh();
    let _ = writeln!(e.out, "  {} = trunc i128 {} to i64", hi, hi128);
    let lo = e.fresh();
    let _ = writeln!(e.out, "  {} = trunc i128 {} to i64", lo, v);
    vec![format!("i64 {}", hi), format!("i64 {}", lo)]
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
                if class_of(e.lc.tys, e.lc.defs, *ty) == TyClass::Agg {
                    let p = e.op_val(v, *ty);
                    let _ = writeln!(e.out, "  ret ptr {}", p);
                } else {
                    let val = e.op_val(v, *ty);
                    let _ = writeln!(
                        e.out,
                        "  ret {} {}",
                        scalar_ty(e.lc.tys, e.lc.defs, *ty),
                        val
                    );
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
        let ret = if class_of(lc.tys, lc.defs, fun.ret) == TyClass::Agg {
            "ptr".to_string()
        } else {
            scalar_ty(lc.tys, lc.defs, fun.ret)
        };
        let _ = writeln!(body, "define i32 @main() {{");
        let mut next_global_temp = 0usize;
        for global in &tac.globals {
            if global.init.is_some() {
                let gty = if scalar_ty(lc.tys, lc.defs, global.ty) == "i8*" {
                    "i8*".to_string()
                } else if contains_i128(lc, global.ty) {
                    "i128".to_string()
                } else {
                    scalar_ty(lc.tys, lc.defs, global.ty)
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
        // 入口返回值：int 型经 trunc/sext 收敛到 i32；bool 零扩展；
        // unit/其余形态按 0 退出（不再无条件丢弃 `fun main(): int` 的值）。
        match lc.tys.kind(fun.ret) {
            TyKind::Unit => {
                let _ = writeln!(body, "  call void {}()", fun_name(lc, fun.def));
                let _ = writeln!(body, "  ret i32 0");
            }
            TyKind::Int(_) => {
                let sty = scalar_ty(lc.tys, lc.defs, fun.ret);
                let r = format!("%u{next_global_temp}");
                next_global_temp += 1;
                let _ = writeln!(body, "  {} = call {} {}()", r, sty, fun_name(lc, fun.def));
                if sty == "i32" {
                    let _ = writeln!(body, "  ret i32 {}", r);
                } else {
                    let e = format!("%u{next_global_temp}");
                    let op = if sty == "i128" || sty == "i64" {
                        "trunc"
                    } else {
                        "sext"
                    };
                    let _ = writeln!(body, "  {} = {} {} {} to i32", e, op, sty, r);
                    let _ = writeln!(body, "  ret i32 {}", e);
                }
            }
            TyKind::Bool => {
                let r = format!("%u{next_global_temp}");
                next_global_temp += 1;
                let _ = writeln!(body, "  {} = call i1 {}()", r, fun_name(lc, fun.def));
                let e = format!("%u{next_global_temp}");
                let _ = writeln!(body, "  {} = zext i1 {} to i32", e, r);
                let _ = writeln!(body, "  ret i32 {}", e);
            }
            _ => {
                let _ = writeln!(body, "  call {} {}()", ret, fun_name(lc, fun.def));
                let _ = writeln!(body, "  ret i32 0");
            }
        }
        let _ = writeln!(body, "}}");
        let _ = writeln!(body);
        return;
    }
}
