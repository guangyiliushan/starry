//! TAC 参考解释器（教科书 lli 等价物）——语义验证层。
//!
//! 动机（opt-notes 缺点清单 #1）：快照测试只看 TAC 文本形状，语义错
//! 编抓不到；本解释器直接「跑」TAC 并断言结果。存储即 llvme 的槽
//! 模型：每 temp = u64 槽单元（惰性增长），字符串进侧表（cell 存
//! id+1，0 = null）；标量/聚合在解释器里都只是「若干槽」，与 TAC
//! 表示语义一致，无需存储分类。
//!
//! **位宽模型（2026-10-31，取代早先「int 一律按 i64」的扁平模型）**：
//! 整型值在槽里存 **N 位补码的符号扩展形式**（N = 指令 `ty` 的位宽），
//! 算术在 N 位上回绕、移位量判据 `0..N`、conv 按源/目标位宽规约——与
//! llvme 发出的 N 位 LLVM 指令逐位一致。扁平模型会让解释器对窄位宽
//! 断言出真实代码给不出的值（`(2000000000 + 2000000000) / 2 : i32`
//! 算成 2000000000，LLVM 算 -147483648），也就抓不到折叠位宽那类错编。
//! 浮点按 `ty` 的精度算（f32 逐次舍入到单精度）。LLVM 的 poison 在
//! 解释器里一律表现为 trap（越界移位、fptosi 越界/NaN），不发明值。
//! 已知局限：槽是 u64，`i128` 按 64 位建模。
//! 已知局限：`extern "c"` 调用的程序无法经本解释器验证——外部符号只
//! 存在于 native 链接，Callee::Extern 一律 UnknownFun trap。
//! 已知局限：聚合/可空实参经 Value 快照只传递首槽——多槽实参的 interp
//! 语义不完整（差分 harness 因此不选此类用例；native 断言归 build_cli）。

use ast::interner::{Interner, Symbol};
use sem::{DefArena, DefId, FloatKind, IntKind, TyArena, TyKind};

use crate::tac::{
    BinOp, Callee, Const, FunBody, Instr, Operand, RuntimeFn, TacOutput, TempId, Term, UnOp,
};

/// N 位补码规约（低位截断 + 符号扩展回 u64）——与 opt 的 `sx` 同模型
fn narrow(v: u64, w: u32) -> u64 {
    debug_assert!((1..=64).contains(&w));
    let s = 64 - w;
    ((v << s) as i64 >> s) as u64
}

/// 整型位宽；非整型返回 None。i128 按 64 建模（槽是 u64）
fn int_width(kind: &TyKind) -> Option<u32> {
    Some(match kind {
        TyKind::Int(IntKind::I4) => 4,
        TyKind::Int(IntKind::I8) => 8,
        TyKind::Int(IntKind::I16) => 16,
        TyKind::Int(IntKind::I32) => 32,
        TyKind::Int(IntKind::I64) | TyKind::Int(IntKind::I128) => 64,
        TyKind::Char => 32,
        _ => return None,
    })
}

/// 槽里的浮点位模式 → f64 值（f32 值以「已加宽的 f64」存放）
fn f64_of(bits: u64, fk: FloatKind) -> f64 {
    let f = f64::from_bits(bits);
    match fk {
        FloatKind::F32 => f as f32 as f64,
        FloatKind::F64 => f,
    }
}

/// 浮点值 → 槽位模式（目标精度逐次舍入）
fn bits_of_f(f: f64, fk: FloatKind) -> u64 {
    match fk {
        FloatKind::F32 => (f as f32 as f64).to_bits(),
        FloatKind::F64 => f.to_bits(),
    }
}

/// `fptosi` 目标位宽的可表示域 `[lo, hi)`——上界开：`2^(w-1)` 越界。
/// 越界/NaN 是 LLVM poison，解释器以 trap 表示。
fn int_range(w: u32) -> (f64, f64) {
    let half = 2f64.powi(w.min(64) as i32 - 1);
    (-half, half)
}

const STEP_CAP: u64 = 10_000_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I(i64),
    F(f64),
    B(bool),
    S(String),
    Null,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpError {
    UnknownFun(String),
    AggArg,
    Arity,
    Trap(&'static str),
    StepLimit,
}

/// 按名字找到函数并执行（测试入口；名字经 interner 解码比较）
pub fn run_named(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &Interner,
    name: &str,
    args: &[Value],
) -> Result<Value, InterpError> {
    Ok(run_named_capturing(tac, tys, defs, interner, name, args)?.0)
}

/// run_named + 收集 print 输出（差分 harness：与 native stdout 对拍，P1-8）
pub fn run_named_capturing(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &Interner,
    name: &str,
    args: &[Value],
) -> Result<(Value, Vec<String>), InterpError> {
    let fun = tac
        .funs
        .iter()
        .find(|f| interner.resolve(defs.get(f.def).name) == name)
        .ok_or_else(|| InterpError::UnknownFun(name.into()))?;
    run_fun_capturing(tac, tys, defs, interner, fun, args)
}

pub fn run_fun(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &Interner,
    fun: &FunBody,
    args: &[Value],
) -> Result<Value, InterpError> {
    Ok(run_fun_capturing(tac, tys, defs, interner, fun, args)?.0)
}

fn run_fun_capturing(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &Interner,
    fun: &FunBody,
    args: &[Value],
) -> Result<(Value, Vec<String>), InterpError> {
    let _ = defs; // 签名与 emit 对称（名字查找在 run_named 消费）
    if fun.params.len() != args.len() {
        return Err(InterpError::Arity);
    }
    let mut strings: Vec<String> = Vec::new();
    let mut str_ids: std::collections::HashMap<Symbol, u64> = std::collections::HashMap::new();
    // 预 intern 全部 Const::Str：op() 是只读的，常量侧表必须先建——
    // 否则数组/元组/返回里的字符串常量被读成 0（差分 harness 抓到）
    let const_ops: Vec<Operand> = tac
        .funs
        .iter()
        .chain(tac.globals.iter().filter_map(|g| g.init.as_ref()))
        .flat_map(|f| {
            let mut ops: Vec<Operand> = Vec::new();
            for block in &f.blocks {
                for instr in &block.instrs {
                    match instr {
                        Instr::Bin { l, r, .. } => ops.extend([l.clone(), r.clone()]),
                        Instr::Un { a, .. } => ops.push(a.clone()),
                        Instr::Copy { src, .. } => ops.push(src.clone()),
                        Instr::Load { idx, .. } => ops.push(idx.clone()),
                        Instr::Store { idx, src, .. } => ops.extend([idx.clone(), src.clone()]),
                        Instr::Param { a, .. } => ops.push(a.clone()),
                        Instr::StorePtr { src, .. } => ops.push(src.clone()),
                        _ => {}
                    }
                }
                if let Term::Ret { value: Some(v), .. } = &block.term {
                    ops.push(v.clone());
                }
            }
            ops
        })
        .collect();
    for op in const_ops {
        if let Operand::Const(Const::Str(sym)) = op
            && !str_ids.contains_key(&sym)
        {
            let text = interner.resolve(sym);
            let id = strings
                .iter()
                .position(|s| s == text)
                .map(|i| i as u64 + 1)
                .unwrap_or_else(|| {
                    strings.push(text.to_string());
                    strings.len() as u64
                });
            str_ids.insert(sym, id);
        }
    }
    let mut machine = Machine {
        tac,
        tys,
        strings,
        str_ids,
        globals: Vec::new(),
        printed: Vec::new(),
    };
    // 全局初始化：按源序执行合成 init 函数，后续函数经 globals 表读写。
    for global in &tac.globals {
        let value = match &global.init {
            Some(init) => machine
                .exec_nested(init, &[], 0)?
                .map(|(v, _)| v)
                .unwrap_or(0),
            None => 0,
        };
        machine.globals.push((global.def, value));
    }
    // 槽存储：temp → u64 单元（惰性增长）
    let n = fun
        .params
        .iter()
        .map(|(t, _)| t.0)
        .chain(fun.blocks.iter().flat_map(|b| {
            b.instrs.iter().flat_map(|i| match i {
                Instr::Bin { dst, .. }
                | Instr::Un { dst, .. }
                | Instr::Copy { dst, .. }
                | Instr::Load { dst, .. }
                | Instr::Slot { dst, .. } => Some(dst.0),
                Instr::AddrOf { dst, base, .. } => Some(dst.0.max(base.0)),
                Instr::LoadPtr { dst, ptr, .. } => Some(dst.0.max(ptr.0)),
                Instr::StorePtr { ptr, .. } => Some(ptr.0),
                Instr::Store { base, .. } => Some(base.0),
                Instr::LoadGlobal { dst, .. } => Some(dst.0),
                Instr::Call { dst, .. } => dst.map(|d| d.0),
                Instr::Param { .. } | Instr::StoreGlobal { .. } => None,
            })
        }))
        .max()
        .unwrap_or(0) as usize
        + 1;
    let mut cells: Vec<Vec<u64>> = vec![Vec::new(); n];
    for ((t, ty), arg) in fun.params.iter().zip(args) {
        machine.bind_arg(&mut cells[t.0 as usize], *ty, arg)?;
    }
    let value = machine.exec(fun, &mut cells, 0).map(|ret| match ret {
        Some((v, ty)) => machine.decode(v, ty),
        None => Value::Unit,
    })?;
    Ok((value, machine.printed))
}

struct Machine<'a> {
    tac: &'a TacOutput,
    tys: &'a TyArena,
    strings: Vec<String>,
    /// Const::Str → 侧表 id（预 intern；op() 只读，查表不 intern）
    str_ids: std::collections::HashMap<Symbol, u64>,
    globals: Vec<(DefId, u64)>,
    /// print 输出收集（差分 harness；每条 = 一行，与 runtime write_line 对齐）
    printed: Vec<String>,
}

impl<'a> Machine<'a> {
    fn str_id(&mut self, s: &str) -> u64 {
        for (i, e) in self.strings.iter().enumerate() {
            if e == s {
                return i as u64 + 1;
            }
        }
        self.strings.push(s.to_string());
        self.strings.len() as u64
    }

    fn bind_arg(
        &mut self,
        cell: &mut Vec<u64>,
        ty: sem::TyId,
        arg: &Value,
    ) -> Result<(), InterpError> {
        match (self.tys.kind(ty), arg) {
            (TyKind::Nullable(_), Value::Null) => {
                cell.resize(1, 0);
                Ok(())
            }
            (TyKind::Nullable(inner), v) => {
                let inner = *inner;
                cell.resize(2, 0);
                cell[0] = 1;
                self.bind_arg_at(cell, 1, inner, v)
            }
            (_, Value::I(v)) => {
                cell.resize(1.max(cell.len()), 0);
                // 入参按形参位宽规约：调用点传进来的值要与体内同域
                cell[0] = narrow(*v as u64, int_width(self.tys.kind(ty)).unwrap_or(64));
                Ok(())
            }
            (_, Value::F(v)) => {
                cell.resize(1.max(cell.len()), 0);
                let fk = match self.tys.kind(ty) {
                    TyKind::Float(fk) => *fk,
                    _ => FloatKind::F64,
                };
                cell[0] = bits_of_f(*v, fk);
                Ok(())
            }
            (_, Value::B(v)) => {
                cell.resize(1.max(cell.len()), 0);
                cell[0] = u64::from(*v);
                Ok(())
            }
            (_, Value::S(s)) => {
                let id = self.str_id(s);
                cell.resize(1.max(cell.len()), 0);
                cell[0] = id;
                Ok(())
            }
            (_, Value::Null) => {
                cell.resize(1.max(cell.len()), 0);
                cell[0] = 0;
                Ok(())
            }
            (_, Value::Unit) => Ok(()),
        }
    }

    fn bind_arg_at(
        &mut self,
        cell: &mut [u64],
        at: usize,
        ty: sem::TyId,
        v: &Value,
    ) -> Result<(), InterpError> {
        // payload 槽绑定（nullable 实参内部）；聚合 payload 暂不支持
        if matches!(self.tys.kind(ty), TyKind::Named(..)) {
            return Err(InterpError::AggArg);
        }
        let mut sub = Vec::new();
        self.bind_arg(&mut sub, ty, v)?;
        cell[at] = sub.first().copied().unwrap_or(0);
        Ok(())
    }

    fn decode(&self, v: u64, ty: sem::TyId) -> Value {
        let kind = self.tys.kind(ty);
        match kind {
            TyKind::Int(_) => Value::I(narrow(v, int_width(kind).unwrap_or(64)) as i64),
            TyKind::Float(fk) => Value::F(f64_of(v, *fk)),
            TyKind::Bool => Value::B(v != 0),
            TyKind::String => {
                if v == 0 {
                    Value::Null
                } else {
                    Value::S(self.strings[(v - 1) as usize].clone())
                }
            }
            TyKind::Nullable(_) => {
                if v == 0 {
                    Value::Null
                } else {
                    Value::I(v as i64)
                }
            }
            TyKind::Unit => Value::Unit,
            // char/聚合兜底：整型按位宽规约，其余原样
            _ => Value::I(narrow(v, int_width(kind).unwrap_or(64)) as i64),
        }
    }

    fn op(&self, cells: &[Vec<u64>], op: &Operand) -> u64 {
        match op {
            Operand::Temp(t) => cells[t.0 as usize].first().copied().unwrap_or(0),
            Operand::Const(c) => match c {
                Const::Int(v) => *v as i64 as u64,
                Const::Float(v) => v.to_bits(),
                Const::Bool(b) => u64::from(*b),
                Const::Null | Const::Unit => 0,
                // 字符串常量：查预 intern 的侧表（构建期已全量收集）
                Const::Str(sym) => self.str_ids.get(sym).copied().unwrap_or(0),
            },
        }
    }

    fn exec(
        &mut self,
        fun: &FunBody,
        cells: &mut [Vec<u64>],
        depth: usize,
    ) -> Result<Option<(u64, sem::TyId)>, InterpError> {
        if depth > 512 {
            return Err(InterpError::Trap("call depth"));
        }
        let mut cur = fun.entry;
        let mut steps = 0u64;
        let mut pending: Vec<(u64, sem::TyId)> = Vec::new();
        loop {
            steps += 1;
            if steps > STEP_CAP {
                return Err(InterpError::StepLimit);
            }
            let block = fun
                .blocks
                .iter()
                .find(|b| b.id == cur)
                .ok_or(InterpError::Trap("bad block"))?;
            for instr in &block.instrs {
                self.step(fun, cells, &mut pending, instr, depth)?;
            }
            match &block.term {
                Term::Jmp(b) => cur = *b,
                Term::Br { cond, then_, else_ } => {
                    cur = if self.op(cells, cond) != 0 {
                        *then_
                    } else {
                        *else_
                    };
                }
                Term::Ret { value, ty } => {
                    return Ok(value.as_ref().map(|v| (self.op(cells, v), *ty)));
                }
                Term::Unreachable => return Err(InterpError::Trap("unreachable")),
            }
        }
    }

    fn step(
        &mut self,
        _fun: &FunBody,
        cells: &mut [Vec<u64>],
        pending: &mut Vec<(u64, sem::TyId)>,
        instr: &Instr,
        depth: usize,
    ) -> Result<(), InterpError> {
        match instr {
            Instr::Bin { dst, op, l, r, ty } => {
                let a = self.op(cells, l);
                let b = self.op(cells, r);
                let kind = self.tys.kind(*ty);
                let v = if let TyKind::Float(fk) = kind {
                    let fk = *fk;
                    let (x, y) = (f64_of(a, fk), f64_of(b, fk));
                    let f = match op {
                        BinOp::Add => x + y,
                        BinOp::Sub => x - y,
                        BinOp::Mul => x * y,
                        BinOp::Div => x / y,
                        BinOp::Rem => x % y,
                        BinOp::Eq => return self.set_bool(cells, dst, x == y),
                        BinOp::Ne => return self.set_bool(cells, dst, x != y),
                        BinOp::Lt => return self.set_bool(cells, dst, x < y),
                        BinOp::Le => return self.set_bool(cells, dst, x <= y),
                        BinOp::Gt => return self.set_bool(cells, dst, x > y),
                        BinOp::Ge => return self.set_bool(cells, dst, x >= y),
                        _ => return Err(InterpError::Trap("float bitop")),
                    };
                    bits_of_f(f, fk)
                } else {
                    // 整型在 `ty` 位宽上算：操作数先规约、结果再规约
                    let w = int_width(kind).unwrap_or(64);
                    let (x, y) = (narrow(a, w) as i64, narrow(b, w) as i64);
                    let raw = match op {
                        BinOp::Add => x.wrapping_add(y) as u64,
                        BinOp::Sub => x.wrapping_sub(y) as u64,
                        BinOp::Mul => x.wrapping_mul(y) as u64,
                        BinOp::Div => {
                            if y == 0 {
                                return Err(InterpError::Trap("div by zero"));
                            }
                            x.wrapping_div(y) as u64
                        }
                        BinOp::Rem => {
                            if y == 0 {
                                return Err(InterpError::Trap("rem by zero"));
                            }
                            x.wrapping_rem(y) as u64
                        }
                        BinOp::BitAnd => (x & y) as u64,
                        BinOp::BitOr => (x | y) as u64,
                        BinOp::BitXor => (x ^ y) as u64,
                        // 移位量判据是目标位宽（LLVM 对越界移位是 poison，
                        // 解释器以 trap 表示——不发明值，与 opt 不折叠一致）
                        BinOp::Shl | BinOp::Shr => {
                            if !(0..w as i64).contains(&y) {
                                return Err(InterpError::Trap("shift out of range"));
                            }
                            if matches!(op, BinOp::Shl) {
                                x.wrapping_shl(y as u32) as u64
                            } else {
                                (x >> y) as u64
                            }
                        }
                        BinOp::Eq => return self.set_bool(cells, dst, x == y),
                        BinOp::Ne => return self.set_bool(cells, dst, x != y),
                        BinOp::Lt => return self.set_bool(cells, dst, x < y),
                        BinOp::Le => return self.set_bool(cells, dst, x <= y),
                        BinOp::Gt => return self.set_bool(cells, dst, x > y),
                        BinOp::Ge => return self.set_bool(cells, dst, x >= y),
                    };
                    narrow(raw, w)
                };
                Self::put(cells, *dst, v);
                Ok(())
            }
            Instr::Un { dst, op, a, ty } => {
                let v0 = self.op(cells, a);
                let kind = self.tys.kind(*ty);
                let out = match op {
                    UnOp::Neg => match kind {
                        TyKind::Float(fk) => bits_of_f(-f64_of(v0, *fk), *fk),
                        _ => {
                            let w = int_width(kind).unwrap_or(64);
                            narrow((narrow(v0, w) as i64).wrapping_neg() as u64, w)
                        }
                    },
                    UnOp::Not => u64::from(v0 == 0),
                    UnOp::BitNot => {
                        let w = int_width(kind).unwrap_or(64);
                        narrow(!narrow(v0, w), w)
                    }
                    UnOp::Conv(from) => self.conv(*from, *ty, v0)?,
                };
                Self::put(cells, *dst, out);
                Ok(())
            }
            Instr::Copy { dst, src, ty } => {
                let v = self.norm(self.op(cells, src), *ty);
                Self::put(cells, *dst, v);
                Ok(())
            }
            Instr::Load { dst, base, idx, ty } => {
                let i = self.op(cells, idx) as usize;
                let v = cells[base.0 as usize].get(i).copied().unwrap_or(0);
                Self::put(cells, *dst, self.norm(v, *ty));
                Ok(())
            }
            Instr::Store { base, idx, src, ty } => {
                let i = self.op(cells, idx) as usize;
                let v = self.norm(self.op(cells, src), *ty);
                let slot = &mut cells[base.0 as usize];
                if slot.len() <= i {
                    slot.resize(i + 1, 0);
                }
                slot[i] = v;
                Ok(())
            }
            Instr::LoadGlobal { dst, global, ty } => {
                let v = self
                    .globals
                    .iter()
                    .find(|(def, _)| def == global)
                    .map(|(_, v)| *v)
                    .unwrap_or(0);
                Self::put(cells, *dst, self.norm(v, *ty));
                Ok(())
            }
            Instr::StoreGlobal { global, src, ty } => {
                let v = self.norm(self.op(cells, src), *ty);
                if let Some(entry) = self.globals.iter_mut().find(|(def, _)| def == global) {
                    entry.1 = v;
                }
                Ok(())
            }
            // M2 指针指令：解释器无地址空间模型，一律 trap
            //（差分 harness 不选指针程序；native build 为验收路径）
            Instr::AddrOf { .. } | Instr::LoadPtr { .. } | Instr::StorePtr { .. } => {
                Err(InterpError::Trap("pointer op (AddrOf/LoadPtr/StorePtr)"))
            }
            Instr::Slot { dst, count } => {
                let slot = &mut cells[dst.0 as usize];
                if slot.len() < *count as usize {
                    slot.resize(*count as usize, 0);
                }
                Ok(())
            }
            Instr::Param { a, ty } => {
                let v = self.op(cells, a);
                pending.push((self.norm(v, *ty), *ty));
                Ok(())
            }
            Instr::Call { dst, callee, ty } => {
                let args = std::mem::take(pending);
                let v = match callee {
                    Callee::Runtime(RuntimeFn::StrConv) => {
                        let (v, pty) = args.first().copied().unwrap_or((0, *ty));
                        let s = match self.tys.kind(pty) {
                            TyKind::Int(_) => format!("{}", v as i64),
                            TyKind::Float(_) => format!("{}", f64::from_bits(v)),
                            TyKind::Bool => {
                                if v != 0 {
                                    "true".to_string()
                                } else {
                                    "false".to_string()
                                }
                            }
                            TyKind::String => self.strings[(v.max(1) - 1) as usize].clone(),
                            TyKind::Char => char::from_u32(v as u32)
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
                            _ => return Err(InterpError::Trap("strconv ty")),
                        };
                        self.str_id(&s)
                    }
                    Callee::Runtime(RuntimeFn::StrConvChar) => {
                        let (v, _) = args.first().copied().unwrap_or((0, *ty));
                        let s = char::from_u32(v as u32)
                            .map(|c| c.to_string())
                            .unwrap_or_default();
                        self.str_id(&s)
                    }
                    Callee::Runtime(RuntimeFn::StrEq) => {
                        let a = args.first().map(|(v, _)| *v).unwrap_or(0);
                        let b = args.get(1).map(|(v, _)| *v).unwrap_or(0);
                        let sa = &self.strings[(a.max(1) - 1) as usize];
                        let sb = &self.strings[(b.max(1) - 1) as usize];
                        u64::from(sa == sb)
                    }
                    Callee::Runtime(RuntimeFn::Print) | Callee::Runtime(RuntimeFn::PrintChar) => {
                        // 差分验证（P1-8）：print 文本形态收集进 printed，
                        // 由 diff harness 与 native stdout 逐行对拍
                        let (v, pty) = args.first().copied().unwrap_or((0, *ty));
                        self.printed.push(self.print_line(v, pty));
                        0
                    }
                    Callee::Extern(def) => {
                        return Err(InterpError::UnknownFun(format!("extern def {}", def.0)));
                    }
                    Callee::Runtime(RuntimeFn::StrConcat) => {
                        let a = args.first().map(|(v, _)| *v).unwrap_or(0);
                        let b = args.get(1).map(|(v, _)| *v).unwrap_or(0);
                        let mut s = self.strings[(a.max(1) - 1) as usize].clone();
                        s.push_str(&self.strings[(b.max(1) - 1) as usize]);
                        self.str_id(&s)
                    }
                    Callee::Def(def) => {
                        let callee_fun = self
                            .tac
                            .funs
                            .iter()
                            .find(|f| f.def == *def)
                            .ok_or(InterpError::UnknownFun(format!("def {}", def.0)))?;
                        let decoded: Vec<Value> = callee_fun
                            .params
                            .iter()
                            .zip(args.iter())
                            .map(|((_, pty), (v, _))| self.decode(*v, *pty))
                            .collect();
                        match self.exec_nested(callee_fun, &decoded, depth)? {
                            Some((rv, _)) => rv,
                            None => 0,
                        }
                    }
                };
                if let Some(d) = dst {
                    Self::put(cells, *d, v);
                }
                Ok(())
            }
        }
    }

    fn exec_nested(
        &mut self,
        fun: &FunBody,
        args: &[Value],
        depth: usize,
    ) -> Result<Option<(u64, sem::TyId)>, InterpError> {
        // 子调用：独立槽存储（值语义拷贝，同 llvme 聚合参数按指针的
        // 差异在解释器层无感——sem v1 形参不可变，拷贝等价）
        let n = 64usize;
        let mut cells: Vec<Vec<u64>> = vec![
            Vec::new();
            n.max(
                fun.blocks
                    .iter()
                    .flat_map(|b| b.instrs.iter())
                    .filter_map(|i| match i {
                        Instr::Slot { dst, count } => Some(dst.0 as usize + *count as usize),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0)
                    + 8,
            )
        ];
        for ((t, ty), arg) in fun.params.iter().zip(args) {
            self.bind_arg(&mut cells[t.0 as usize], *ty, arg)?;
        }
        self.exec(fun, &mut cells, depth + 1)
    }

    /// print 输出的文本形态，与 runtime `starry_print_*` 对齐
    /// （i64 十进制、bool 小写、string 原文、char 按码点）。浮点的
    /// C fmt_double 15 位文本不复刻——差分用例排除浮点打印（已知局限）。
    fn print_line(&self, v: u64, ty: sem::TyId) -> String {
        match self.tys.kind(ty) {
            TyKind::String => self.strings[(v.max(1) - 1) as usize].clone(),
            TyKind::Char => char::from_u32(v as u32)
                .map(|c| c.to_string())
                .unwrap_or_default(),
            TyKind::Bool => if v != 0 { "true" } else { "false" }.to_string(),
            // 差分用例不含浮点打印；占位形态仅供人工排查
            TyKind::Float(_) => format!("{:?}", self.decode(v, ty)),
            _ => match self.decode(v, ty) {
                Value::I(i) => i.to_string(),
                other => format!("{other:?}"),
            },
        }
    }

    fn set_bool(&self, cells: &mut [Vec<u64>], dst: &TempId, b: bool) -> Result<(), InterpError> {
        Self::put(cells, *dst, u64::from(b));
        Ok(())
    }

    fn put(cells: &mut [Vec<u64>], t: TempId, v: u64) {
        let slot = &mut cells[t.0 as usize];
        if slot.is_empty() {
            slot.push(v);
        } else {
            slot[0] = v;
        }
    }

    /// 按 `ty` 位宽规约一个槽值/常量（非整型原样）
    fn norm(&self, v: u64, ty: sem::TyId) -> u64 {
        narrow(v, int_width(self.tys.kind(ty)).unwrap_or(64))
    }

    /// 数值转换（`UnOp::Conv`）：按源/目标位宽与精度规约。
    /// LLVM 的 fptosi 对 NaN/越界是 poison，此处以 trap 表示。
    fn conv(&self, from: sem::TyId, to: sem::TyId, v: u64) -> Result<u64, InterpError> {
        let (fk, tk) = (self.tys.kind(from), self.tys.kind(to));
        let (fw, tw) = (int_width(fk), int_width(tk));
        Ok(match (fk, tk) {
            // fptosi：向零截断后必须落在目标域
            (TyKind::Float(ff), _) if tw.is_some() => {
                let f = f64_of(v, *ff);
                let (lo, hi) = int_range(tw.unwrap_or(64));
                let t = f.trunc();
                if !f.is_finite() || t < lo || t >= hi {
                    return Err(InterpError::Trap("fptosi out of range"));
                }
                narrow(t as i64 as u64, tw.unwrap_or(64))
            }
            // sitofp：先按源位宽规约，再舍入到目标精度
            (_, TyKind::Float(tf)) if fw.is_some() => {
                bits_of_f(narrow(v, fw.unwrap_or(64)) as i64 as f64, *tf)
            }
            // fpext / fptrunc：按目标精度舍入
            (TyKind::Float(ff), TyKind::Float(tf)) => bits_of_f(f64_of(v, *ff), *tf),
            // sext / trunc：窄→宽符号扩展，宽→窄截断
            _ if fw.is_some() && tw.is_some() => {
                narrow(narrow(v, fw.unwrap_or(64)), tw.unwrap_or(64))
            }
            _ => v,
        })
    }
}

// 静默引用（arity 与 def 路径的防御性使用）
#[allow(dead_code)]
fn _use(defs: &DefArena, d: DefId) -> bool {
    defs.len() > d.0 as usize
}
