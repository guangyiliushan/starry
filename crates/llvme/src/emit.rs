//! 模块/函数发射：两遍（temp 存储分类 → 指令发射）。
//!
//! 存储分类（每个 temp 恰好一个 entry alloca，外部 mem2reg 可提升）：
//! - Cell：i64 槽（标量）
//! - WideCell：i128 标量槽（alloca i128；S3 后仅标量 i128）
//! - Block(n)：`[n x i64]` 块（聚合；i128 成员占 2 槽 lo/hi，
//!   member 访问在 emit 侧 split/join——见 i128-layout-spec）
//! - PtrCell(n)：`ptr` 槽装聚合指针（聚合实参/返回值）
//!
//! 不透明指针风格（LLVM 15+）：无指针 bitcast，load/store/ret 聚合
//! 经 `ptr`；槽寻址统一 `getelementptr [n x E], ptr p, i64 0, i64 i`
//! （E = i64 或 i128，Cell 即自身，idx 恒 0；tag/flag 是槽 0 里的 i8，
//! 写时就地 `store i8 ..., ptr` —— 天然免 bitcast）。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ast::interner::{Interner, Symbol};
use irc::layout::{field_slot, scalar_slots};
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
    let deinit_sym = interner.intern("deinit");
    let lc = Lc {
        tys,
        defs,
        interner,
    };
    for global in &tac.globals {
        g.global_slots
            .insert(global.def, scalar_slots(&lc, global.ty).max(1) as u32);
    }
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
        let is_agg = class_of(lc.tys, lc.defs, global.ty) == TyClass::Agg;
        let storage_ty = if scalar_ty(lc.tys, lc.defs, global.ty) == "i8*" {
            "i8*".to_string()
        } else if is_agg {
            format!("[{} x i64]", scalar_slots(&lc, global.ty).max(1))
        } else if contains_i128(&lc, global.ty) {
            "i128".to_string()
        } else {
            "i64".to_string()
        };
        let zero = if is_agg {
            "zeroinitializer"
        } else if storage_ty == "i8*" {
            "null"
        } else {
            "0"
        };
        let _ = writeln!(globals, "{} = global {} {}", name, storage_ty, zero);
    }
    // Phase 3a: one stable TypeInfo per class. The vtable field stays null
    // until virtual dispatch is enabled; deinit is resolved from class members.
    for (def_id, def) in defs.iter().enumerate() {
        if !matches!(def.kind, DefKind::Class) {
            continue;
        }
        let def_id = DefId(def_id as u32);
        // D-019: a derived class without a user deinit inherits the nearest
        // base deinit; a derived user deinit chains to that base in IRC.
        let mut deinit = None;
        let mut cursor = def_id;
        while deinit.is_none() {
            let Some(ms) = defs.get(cursor).member_scope else {
                break;
            };
            deinit = defs
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    candidate.scope == ms
                        && candidate.name == deinit_sym
                        && matches!(candidate.kind, DefKind::Fun)
                })
                .map(|(id, _)| DefId(id as u32));
            let Some(base) = defs.get(cursor).sig.supertypes.first().copied() else {
                break;
            };
            cursor = base;
        }
        let vt = tac
            .class_vtables
            .iter()
            .find(|table| table.class_def == def_id);
        let vtable_name = format!(
            "@classvtable.{}.{}",
            sanitize(interner.resolve(def.name)),
            def_id.0
        );
        let vtable_ptr = match vt {
            Some(table) => {
                let count = table.methods.len();
                let init = if count == 0 {
                    "zeroinitializer".to_string()
                } else {
                    format!(
                        "[{}]",
                        table
                            .methods
                            .iter()
                            .map(|method| format!("ptr {}", fun_name(&lc, *method)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let _ = writeln!(
                    globals,
                    "{vtable_name} = private constant [{count} x ptr] {init}"
                );
                format!("ptr {vtable_name}")
            }
            None => "ptr null".to_string(),
        };
        let deinit_ptr = deinit
            .map(|d| format!("ptr {}", fun_name(&lc, d)))
            .unwrap_or_else(|| "ptr null".to_string());
        let _ = writeln!(
            globals,
            "{} = private constant {{ ptr, ptr, i64, ptr }} {{ {}, {}, i64 {}, ptr null }}",
            crate::ty::typeinfo_name(&lc, def_id),
            deinit_ptr,
            vtable_ptr,
            def_id.0
        );
    }
    // Phase 2a: emit one fixed ABI vtable per registered impl.
    for vt in &tac.vtables {
        let trait_name = sanitize(interner.resolve(defs.get(vt.trait_def).name));
        let type_name = sanitize(interner.resolve(defs.get(vt.type_def).name));
        let name = format!(
            "@vtable.{}.{}.{}.{}",
            trait_name, vt.trait_def.0, type_name, vt.type_def.0
        );
        let mut elems = Vec::with_capacity(vt.methods.len() + 3);
        elems.push("ptr null".to_string());
        elems.push("ptr inttoptr (i64 24 to ptr)".to_string());
        elems.push("ptr inttoptr (i64 8 to ptr)".to_string());
        for method in &vt.methods {
            elems.push(format!("ptr {}", fun_name(&lc, *method)));
        }
        let _ = writeln!(
            globals,
            "{} = private constant [{} x ptr] [{}]",
            name,
            elems.len(),
            elems.join(", ")
        );
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
    // S2/P2-3b-2：闭包 env 释放函数（meta 回调）在模块末 flush
    for f in &g.env_release_fns {
        out.push_str(f);
    }
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
    /// S2/P2-3b-2：闭包 env 释放函数（meta 回调）文本，模块末统一 flush
    env_release_fns: Vec<String>,
    /// S2/P2-3b-2：env 释放函数命名计数
    env_release_n: u32,
    /// M1 (D2): starry_overflow_panic was referenced (emit noreturn attrs)
    used_ovf_panic: bool,
    /// 纯值聚合 global 的槽宽（LoadGlobal slot 的 GEP 尺寸）
    global_slots: BTreeMap<DefId, u32>,
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
    /// [n x i64] 块（聚合；含 i128 成员时 n 按 2/成员计——i128-layout-spec）
    Block(u32),
    /// ptr 槽装聚合指针（聚合实参/返回值），n = 指向块的槽数
    PtrCell(u32),
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
            Storage::Block(n) | Storage::PtrCell(n) => *n,
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
        // S3：wide 仅标量 i128（聚合含 i128 → Block(M x i64)，不加宽）
        if contains_i128(lc, *ty) && class_of(lc.tys, lc.defs, *ty) == TyClass::Scalar {
            wide[t.0 as usize] = true;
        }
    }
    let mark = |t: TempId, ty: TyId, wide: &mut [bool]| {
        if contains_i128(lc, ty) && class_of(lc.tys, lc.defs, ty) == TyClass::Scalar {
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
                    if let Operand::Const(Const::Int(k)) = idx {
                        // S3：i128 成员占 2 槽——req 覆盖到 k + 成员槽数
                        req[base.0 as usize] =
                            req[base.0 as usize].max(*k as usize + scalar_slots(lc, *ty).max(1));
                    }
                }
                Instr::Store { base, idx, src, ty } => {
                    if let Operand::Temp(t) = src {
                        mark(*t, *ty, &mut wide);
                    }
                    if let Operand::Const(Const::Int(k)) = idx {
                        req[base.0 as usize] =
                            req[base.0 as usize].max(*k as usize + scalar_slots(lc, *ty).max(1));
                    }
                }
                Instr::LoadPtr { dst, ty, .. } => {
                    // S2/P2-3b-3：i128 捕获经 LoadPtr 装载——dst 需 WideCell
                    if contains_i128(lc, *ty) && class_of(lc.tys, lc.defs, *ty) == TyClass::Scalar {
                        wide[dst.0 as usize] = true;
                    }
                }
                Instr::AddrOf { .. }
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
            Some(n) => Storage::PtrCell(n),
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
            Storage::Block(_) | Storage::PtrCell(_) => {
                let base = if matches!(st, Storage::Block(_)) {
                    format!("%t{}.slot", t.0)
                } else {
                    let p = self.fresh();
                    let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                    p
                };
                (base, "i64")
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

    /// S3：i128 成员的 hi 槽地址（idx+1 的 gep；lo 槽由 slot_addr 给出）
    fn slot_addr_next(&mut self, t: TempId, idx: &Operand) -> String {
        let st = self.st[t.0 as usize];
        let (base, elem) = match st {
            Storage::Cell | Storage::WideCell => return format!("%t{}.slot", t.0),
            Storage::Block(_) | Storage::PtrCell(_) => {
                let base = if matches!(st, Storage::Block(_)) {
                    format!("%t{}.slot", t.0)
                } else {
                    let p = self.fresh();
                    let _ = writeln!(self.out, "  {} = load ptr, ptr %t{}.slot", p, t.0);
                    p
                };
                (base, "i64")
            }
        };
        let n = st.slots();
        let ity = scalar_ty(self.lc.tys, self.lc.defs, self.lc.tys.default_int);
        // 常量索引：idx+1 直接字面量；非常量：寄存器 add
        let gep_idx = match idx {
            Operand::Const(Const::Int(k)) => format!("{}", *k + 1),
            _ => {
                let iv = self.op_val(idx, self.lc.tys.default_int);
                let one = self.fresh();
                let _ = writeln!(self.out, "  {} = add {} {}, 1", one, ity, iv);
                one
            }
        };
        let gep = self.fresh();
        let _ = writeln!(
            self.out,
            "  {} = getelementptr [{} x {}], ptr {}, i64 0, {} {}",
            gep, n, elem, base, ity, gep_idx
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
            Const::Vtable(trait_def, type_def) => format!(
                "@vtable.{}.{}.{}.{}",
                sanitize(self.lc.interner.resolve(self.lc.defs.get(*trait_def).name)),
                trait_def.0,
                sanitize(self.lc.interner.resolve(self.lc.defs.get(*type_def).name)),
                type_def.0
            ),
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
            Storage::PtrCell(_) => {
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
    // S2/P2-3b-2：Param 是否携带 Fn 值（Retain/Release 的 tag 分派依据）
    let mut pending_fn: Vec<bool> = Vec::new();
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
            Storage::PtrCell(_) => {
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
            emit_instr(&mut e, &mut pending, &mut pending_fn, instr);
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
/// Static ARC-leaf predicate shared by capture creation and env release.
fn emit_type_has_arc(e: &FnEmit, ty: TyId) -> bool {
    match e.lc.tys.kind(ty).clone() {
        TyKind::Nullable(inner) => emit_type_has_arc(e, inner),
        TyKind::Fn(..) => true,
        TyKind::Dyn(..) => true,
        TyKind::Named(def, _) => match e.lc.defs.get(def).kind {
            DefKind::Class => true,
            DefKind::Struct => {
                e.lc.defs
                    .get(def)
                    .sig
                    .fields
                    .iter()
                    .any(|(_, t)| emit_type_has_arc(e, *t))
            }
            DefKind::Enum => {
                e.lc.defs
                    .get(def)
                    .sig
                    .variants
                    .iter()
                    .any(|(_, params)| params.iter().any(|(_, t)| emit_type_has_arc(e, *t)))
            }
            _ => false,
        },
        TyKind::Tuple(fields) => fields.iter().any(|t| emit_type_has_arc(e, *t)),
        TyKind::Array(elem, _) => emit_type_has_arc(e, elem),
        _ => false,
    }
}

/// Recursively emit release code for one closure-env capture at `slot`.
/// Returns whether any ARC leaf was released; scalar leaves are no-ops.
/// Enum ARC payload is rejected at capture creation, so no tag CFG is needed.
fn emit_env_release_value(e: &mut FnEmit, out: &mut String, slot: i64, ty: TyId) -> bool {
    match e.lc.tys.kind(ty).clone() {
        TyKind::Nullable(inner) => {
            if !emit_type_has_arc(e, inner) {
                return false;
            }
            let flag_addr = e.fresh();
            let _ = writeln!(
                out,
                "  {flag_addr} = getelementptr i8, ptr %env, i64 {}",
                8 * slot
            );
            let flag = e.fresh();
            let _ = writeln!(out, "  {flag} = load i8, ptr {flag_addr}");
            let rel = e.fresh();
            let end = e.fresh();
            let _ = writeln!(
                out,
                "  br i1 {flag}, label {}, label {}",
                &rel[1..],
                &end[1..]
            );
            let _ = writeln!(out, "{}:", &rel[1..]);
            emit_env_release_value(e, out, slot + 1, inner);
            let _ = writeln!(out, "  br label {}", &end[1..]);
            let _ = writeln!(out, "{}:", &end[1..]);
            true
        }
        TyKind::Named(def, _) if matches!(e.lc.defs.get(def).kind, DefKind::Class) => {
            let addr = e.fresh();
            let _ = writeln!(
                out,
                "  {addr} = getelementptr i8, ptr %env, i64 {}",
                8 * slot
            );
            let v = e.fresh();
            let _ = writeln!(out, "  {v} = load ptr, ptr {addr}");
            let _ = writeln!(out, "  call void @starry_release_class(ptr {v})");
            true
        }
        TyKind::Fn(..) => {
            let addr = e.fresh();
            let _ = writeln!(
                out,
                "  {addr} = getelementptr i8, ptr %env, i64 {}",
                8 * slot
            );
            let v = e.fresh();
            let _ = writeln!(out, "  {v} = load ptr, ptr {addr}");
            let _ = writeln!(out, "  call void @starry_release(ptr {v})");
            true
        }
        TyKind::Dyn(..) => {
            let addr = e.fresh();
            let _ = writeln!(
                out,
                "  {addr} = getelementptr i8, ptr %env, i64 {}",
                8 * slot
            );
            let v = e.fresh();
            let _ = writeln!(out, "  {v} = load ptr, ptr {addr}");
            let _ = writeln!(out, "  call void @starry_release_class(ptr {v})");
            true
        }
        TyKind::Named(def, _) if matches!(e.lc.defs.get(def).kind, DefKind::Struct) => {
            let fields = e.lc.defs.get(def).sig.fields.clone();
            let mut released = false;
            for (index, (_, fty)) in fields.iter().enumerate().rev() {
                let fslot = slot + field_slot(e.lc, def, index) as i64;
                released |= emit_env_release_value(e, out, fslot, *fty);
            }
            released
        }
        TyKind::Tuple(fields) => {
            let total: i64 = fields.iter().map(|t| scalar_slots(e.lc, *t) as i64).sum();
            let mut cursor = slot + total;
            let mut released = false;
            for fty in fields.iter().rev() {
                cursor -= scalar_slots(e.lc, *fty) as i64;
                released |= emit_env_release_value(e, out, cursor, *fty);
            }
            released
        }
        TyKind::Array(elem, len) => {
            let es = scalar_slots(e.lc, elem).max(1) as i64;
            let mut released = false;
            for i in (0..len as i64).rev() {
                released |= emit_env_release_value(e, out, slot + i * es, elem);
            }
            released
        }
        _ => false,
    }
}
fn emit_instr(
    e: &mut FnEmit,
    pending: &mut Vec<(String, String)>,
    pending_fn: &mut Vec<bool>,
    instr: &Instr,
) {
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
            // 聚合拷贝：逐 i64 槽搬运（S3：i128 成员 2 槽，统一 [M x i64]）
            let n = scalar_slots(e.lc, *ty).max(1);
            let elem = "i64";
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
            dst,
            bytes,
            deinit,
            type_info,
            env_caps,
            ty,
        } => {
            // S2/P2-3b-2：闭包 box（有 class 捕获）合成 env 释放函数——
            // meta 逐 class 捕获槽 release（标量槽无动作）。Phase 3a classes
            // instead use the typed allocation entry with a TypeInfo global.
            let d = if let Some(caps) = env_caps {
                e.g.runtime_decls
                    .entry("starry_alloc_box".into())
                    .or_insert(("ptr".into(), "i64, ptr".into()));
                e.g.runtime_decls
                    .entry("starry_release".into())
                    .or_insert(("void".into(), "ptr".into()));
                let mut slot: i64 = 1;
                let mut ftxt = String::new();
                let mut has_release = false;
                for ty in caps {
                    has_release |= emit_env_release_value(e, &mut ftxt, slot, *ty);
                    slot += scalar_slots(e.lc, *ty) as i64;
                }
                if !has_release {
                    "null".to_string()
                } else {
                    e.g.env_release_n += 1;
                    let name = format!("__closure_env_release.{}", e.g.env_release_n);
                    let mut full = format!("define private void @{}(ptr %env) {{\n", name);
                    full.push_str(&ftxt);
                    let _ = writeln!(full, "  ret void");
                    let _ = writeln!(full, "}}");
                    let _ = writeln!(full);
                    e.g.env_release_fns.push(full);
                    format!("@{name}")
                }
            } else if let Some(type_def) = type_info {
                e.g.runtime_decls
                    .entry("starry_alloc_object".into())
                    .or_insert(("ptr".into(), "i64, ptr".into()));
                crate::ty::typeinfo_name(e.lc, *type_def)
            } else {
                e.g.runtime_decls
                    .entry("starry_alloc_object".into())
                    .or_insert(("ptr".into(), "i64, ptr".into()));
                match deinit {
                    Some(d) => fun_name(e.lc, *d),
                    None => "null".to_string(),
                }
            };
            let v = e.fresh();
            let _ = writeln!(
                e.out,
                "  {} = call ptr @starry_alloc_object(i64 {}, ptr {})",
                v, bytes, d
            );
            // S2/P2-3b（修正 7）：闭包 box 的值 = 对象指针 | 1（tag 位）。
            // 裸 fn 值 bit0=0（函数 16B 对齐）；HeapAlloc 16B 对齐 →
            // bit0 可用。Indirect 调用按 bit0 分派。
            let stored = if matches!(e.lc.tys.kind(*ty), TyKind::Fn(..)) {
                let t = e.fresh();
                let _ = writeln!(e.out, "  {} = getelementptr i8, ptr {}, i64 1", t, v);
                t
            } else {
                v
            };
            let _ = writeln!(e.out, "  store ptr {}, ptr %t{}.slot", stored, dst.0);
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
            // S3：块槽内的 i128 成员 = [lo, hi] 两 i64 槽——join 后给值
            let is_i128_in_block = matches!(e.lc.tys.kind(*ty), TyKind::Int(IntKind::I128))
                && matches!(
                    e.st[base.0 as usize],
                    Storage::Block(_) | Storage::PtrCell(_)
                );
            if is_i128_in_block {
                let lo = e.fresh();
                let _ = writeln!(e.out, "  {} = load i64, ptr {}", lo, addr);
                let hi_addr = e.slot_addr_next(*base, idx);
                let hi = e.fresh();
                let _ = writeln!(e.out, "  {} = load i64, ptr {}", hi, hi_addr);
                let lo128 = e.fresh();
                let _ = writeln!(e.out, "  {} = zext i64 {} to i128", lo128, lo);
                let hi128 = e.fresh();
                let _ = writeln!(e.out, "  {} = zext i64 {} to i128", hi128, hi);
                let sh = e.fresh();
                let _ = writeln!(e.out, "  {} = shl i128 {}, 64", sh, hi128);
                let v = e.fresh();
                let _ = writeln!(e.out, "  {} = or i128 {}, {}", v, lo128, sh);
                e.store_val(*dst, *ty, v);
            } else {
                let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
                let v = e.fresh();
                let _ = writeln!(e.out, "  {} = load {}, ptr {}", v, lty, addr);
                e.store_val(*dst, *ty, v);
            }
        }
        Instr::Store { base, idx, src, ty } => {
            let addr = e.slot_addr(*base, idx);
            let v = e.op_val(src, *ty);
            // S3：块槽内的 i128 成员 = [lo, hi] 两 i64 槽——split 后两 store
            let is_i128_in_block = matches!(e.lc.tys.kind(*ty), TyKind::Int(IntKind::I128))
                && matches!(
                    e.st[base.0 as usize],
                    Storage::Block(_) | Storage::PtrCell(_)
                );
            if is_i128_in_block {
                let lo = e.fresh();
                let _ = writeln!(e.out, "  {} = trunc i128 {} to i64", lo, v);
                let sh = e.fresh();
                let _ = writeln!(e.out, "  {} = lshr i128 {}, 64", sh, v);
                let hi = e.fresh();
                let _ = writeln!(e.out, "  {} = trunc i128 {} to i64", hi, sh);
                let _ = writeln!(e.out, "  store i64 {}, ptr {}", lo, addr);
                let hi_addr = e.slot_addr_next(*base, idx);
                let _ = writeln!(e.out, "  store i64 {}, ptr {}", hi, hi_addr);
            } else {
                let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
                let _ = writeln!(e.out, "  store {} {}, ptr {}", lty, v, addr);
            }
        }
        Instr::LoadGlobal {
            dst,
            global,
            slot,
            ty,
        } => {
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let addr = if *slot == 0 {
                fun_name(e.lc, *global)
            } else {
                let n = e.g.global_slots.get(global).copied().unwrap_or(1);
                let p = e.fresh();
                let ity = scalar_ty(e.lc.tys, e.lc.defs, e.lc.tys.default_int);
                let _ = writeln!(
                    e.out,
                    "  {p} = getelementptr [{n} x i64], ptr {}, i64 0, {ity} {slot}",
                    fun_name(e.lc, *global)
                );
                p
            };
            let v = e.fresh();
            let _ = writeln!(e.out, "  {v} = load {lty}, ptr {addr}");
            e.store_val(*dst, *ty, v);
        }
        Instr::StoreGlobal {
            global,
            slot,
            src,
            ty,
        } => {
            let v = e.op_val(src, *ty);
            let lty = scalar_ty(e.lc.tys, e.lc.defs, *ty);
            let addr = if *slot == 0 {
                fun_name(e.lc, *global)
            } else {
                let n = e.g.global_slots.get(global).copied().unwrap_or(1);
                let p = e.fresh();
                let ity = scalar_ty(e.lc.tys, e.lc.defs, e.lc.tys.default_int);
                let _ = writeln!(
                    e.out,
                    "  {p} = getelementptr [{n} x i64], ptr {}, i64 0, {ity} {slot}",
                    fun_name(e.lc, *global)
                );
                p
            };
            let _ = writeln!(e.out, "  store {lty} {v}, ptr {addr}");
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
            // S2/P2-3b-2：Fn 值标记（Retain/Release 的 tag 分派依据）
            pending_fn.push(matches!(e.lc.tys.kind(*ty), TyKind::Fn(..)));
        }
        Instr::Call { dst, callee, ty } => {
            let mut args: Vec<String> = pending
                .drain(..)
                .map(|(v, t)| format!("{} {}", t, v))
                .collect();
            let fn_flags: Vec<bool> = std::mem::take(pending_fn);
            // S2/P2-3b（修正 7）：函数值间接调用 = runtime tag 分派
            //（静态 Const::Fun 直调特例在 emit_indirect_call 内）
            if matches!(callee, Callee::Indirect) {
                emit_indirect_call(e, *dst, *ty, &mut args);
                return;
            }
            // S2/P2-3b-2：Fn 值的 retain/release = tag 分派（box untag 后
            // 真操作；裸 fn no-op）——见 emit_fn_arc_op
            if matches!(
                callee,
                Callee::Runtime(RuntimeFn::Retain) | Callee::Runtime(RuntimeFn::Release)
            ) && fn_flags.first() == Some(&true)
            {
                emit_fn_arc_op(
                    e,
                    matches!(callee, Callee::Runtime(RuntimeFn::Release)),
                    &mut args,
                );
                return;
            }
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
                Callee::Runtime(RuntimeFn::ReleaseClass) => {
                    e.g.runtime_decls
                        .entry("starry_release_class".into())
                        .or_insert(("void".into(), "ptr".into()));
                    ("@starry_release_class".to_string(), "void".to_string())
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
                Callee::Runtime(RuntimeFn::PrintOwnedString) => {
                    e.g.runtime_decls
                        .entry("starry_print_owned_string".into())
                        .or_insert(("void".into(), "i8*".into()));
                    ("@starry_print_owned_string".to_string(), "void".to_string())
                }
                Callee::Runtime(RuntimeFn::FreeString) => {
                    e.g.runtime_decls
                        .entry("starry_free_string".into())
                        .or_insert(("void".into(), "i8*".into()));
                    ("@starry_free_string".to_string(), "void".to_string())
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
                        let elem = "i64";
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

/// S2/P2-3b（修正 7）：函数值间接调用的 runtime tag 分派。
/// 值约定：裸 fn = 原函数指针（bit0=0）；闭包 box = payload 指针 | 1。
/// bit0=0 → 裸 fn 直调（无 env）；bit0=1 → box-1 取 payload[0] 为目标、
/// box-1 作 env 首参（与 lambda 体 OffsetPtr 读取同一 raw 基址约定）。
/// `@name` 常量（Const::Fun 静态裸 fn）直调，零分派开销——C 回调
/// 契约（矩阵 #15）与既有 fn_values IR 形态均不变。
/// S2/P2-3b-3：聚合返回值的槽拷贝（与标准 Call 臂同式）——callee 返回
/// 栈槽地址，caller 逐 i64 槽拷入 dst 块（nullable class 等聚合 Fn 返回
/// 经此落地；直接 store_val 会把栈地址当值存——可空捕获探针抓到）
fn emit_agg_result_copy(e: &mut FnEmit, dst: TempId, ty: TyId, v: &str) {
    let n = scalar_slots(e.lc, ty).max(1);
    let elem = "i64";
    let dst_ptr = e.op_val(&Operand::Temp(dst), ty);
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
}

fn emit_indirect_call(e: &mut FnEmit, dst: Option<TempId>, ty: TyId, args: &mut Vec<String>) {
    if args.is_empty() {
        let _ = writeln!(e.out, "  ; ERROR: indirect call without a function pointer");
        return;
    }
    let target = args.remove(0);
    let tv = target.strip_prefix("ptr ").unwrap_or(&target).to_string();
    let ret = if class_of(e.lc.tys, e.lc.defs, ty) == TyClass::Agg {
        "ptr".to_string()
    } else {
        scalar_ty(e.lc.tys, e.lc.defs, ty)
    };
    let rest = args.join(", ");
    if tv.starts_with('@') {
        if ret == "void" {
            let _ = writeln!(e.out, "  call void {}({})", tv, rest);
        } else {
            let v = e.fresh();
            let _ = writeln!(e.out, "  {} = call {} {}({})", v, ret, tv, rest);
            if let Some(d) = dst {
                e.store_val(d, ty, v);
            }
        }
        return;
    }
    let iv = e.fresh();
    let _ = writeln!(e.out, "  {} = ptrtoint ptr {} to i64", iv, tv);
    let bit = e.fresh();
    let _ = writeln!(e.out, "  {} = and i64 {}, 1", bit, iv);
    let cond = e.fresh();
    let _ = writeln!(e.out, "  {} = icmp ne i64 {}, 0", cond, bit);
    let bare_l = e.fresh();
    let box_l = e.fresh();
    let merge_l = e.fresh();
    let _ = writeln!(e.out, "  br i1 {}, label {}, label {}", cond, box_l, bare_l);
    let _ = writeln!(e.out, "{}:", &bare_l[1..]);
    if ret == "void" {
        let _ = writeln!(e.out, "  call void {}({})", tv, rest);
    } else {
        let v = e.fresh();
        let _ = writeln!(e.out, "  {} = call {} {}({})", v, ret, tv, rest);
        if let Some(d) = dst {
            if class_of(e.lc.tys, e.lc.defs, ty) == TyClass::Agg {
                emit_agg_result_copy(e, d, ty, &v);
            } else {
                e.store_val(d, ty, v);
            }
        }
    }
    let _ = writeln!(e.out, "  br label %{}", &merge_l[1..]);
    let _ = writeln!(e.out, "{}:", &box_l[1..]);
    let boxp = e.fresh();
    let _ = writeln!(e.out, "  {} = getelementptr i8, ptr {}, i64 -1", boxp, tv);
    let fp = e.fresh();
    let _ = writeln!(e.out, "  {} = load ptr, ptr {}", fp, boxp);
    let boxed_args = if rest.is_empty() {
        format!("ptr {}", boxp)
    } else {
        format!("ptr {}, {}", boxp, rest)
    };
    if ret == "void" {
        let _ = writeln!(e.out, "  call void {}({})", fp, boxed_args);
    } else {
        let v = e.fresh();
        let _ = writeln!(e.out, "  {} = call {} {}({})", v, ret, fp, boxed_args);
        if let Some(d) = dst {
            if class_of(e.lc.tys, e.lc.defs, ty) == TyClass::Agg {
                emit_agg_result_copy(e, d, ty, &v);
            } else {
                e.store_val(d, ty, v);
            }
        }
    }
    let _ = writeln!(e.out, "  br label %{}", &merge_l[1..]);
    let _ = writeln!(e.out, "{}:", &merge_l[1..]);
}

/// S2/P2-3b-2：Fn 值（tagged box | 裸 fn）的 retain/release——bit0 分派：
/// box → untag 后对 ARC 对象真操作；裸 fn → no-op。
fn emit_fn_arc_op(e: &mut FnEmit, release: bool, args: &mut Vec<String>) {
    let target = args.remove(0);
    let tv = target.strip_prefix("ptr ").unwrap_or(&target).to_string();
    let iv = e.fresh();
    let _ = writeln!(e.out, "  {} = ptrtoint ptr {} to i64", iv, tv);
    let bit = e.fresh();
    let _ = writeln!(e.out, "  {} = and i64 {}, 1", bit, iv);
    let cond = e.fresh();
    let _ = writeln!(e.out, "  {} = icmp ne i64 {}, 0", cond, bit);
    let do_l = e.fresh();
    let end_l = e.fresh();
    let _ = writeln!(e.out, "  br i1 {}, label {}, label {}", cond, do_l, end_l);
    let _ = writeln!(e.out, "{}:", &do_l[1..]);
    let boxp = e.fresh();
    let _ = writeln!(e.out, "  {} = getelementptr i8, ptr {}, i64 -1", boxp, tv);
    if release {
        e.g.runtime_decls
            .entry("starry_release".into())
            .or_insert(("void".into(), "ptr".into()));
        let _ = writeln!(e.out, "  call void @starry_release(ptr {})", boxp);
    } else {
        e.g.runtime_decls
            .entry("starry_retain".into())
            .or_insert(("ptr".into(), "ptr".into()));
        let _ = writeln!(e.out, "  call ptr @starry_retain(ptr {})", boxp);
    }
    let _ = writeln!(e.out, "  br label %{}", &end_l[1..]);
    let _ = writeln!(e.out, "{}:", &end_l[1..]);
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
            if global.init.is_some() && class_of(lc.tys, lc.defs, global.ty) == TyClass::Agg {
                // 纯值聚合 init 已直接写入多槽全局；这里只触发初始化。
                let init = format!("@__starry_init_global.{}", global.def.0);
                let _ = writeln!(body, "  call void {init}()");
            } else if global.init.is_some() {
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
