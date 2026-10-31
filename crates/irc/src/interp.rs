//! TAC 参考解释器（教科书 lli 等价物）——语义验证层。
//!
//! 动机（opt-notes 缺点清单 #1）：快照测试只看 TAC 文本形状，语义错
//! 编抓不到；本解释器直接「跑」TAC 并断言结果。存储即 llvme 的槽
//! 模型：每 temp = u64 槽单元（惰性增长），字符串进侧表（cell 存
//! id+1，0 = null）；标量/聚合在解释器里都只是「若干槽」，与 TAC
//! 表示语义一致，无需存储分类。int 位宽折叠为 i64（位宽语义归
//! llvme/LLVM 层——ponytail：解释器验控制流与数据流，不验位宽）。

use ast::interner::Interner;
use sem::{DefArena, DefId, FloatKind, TyArena, TyKind};

use crate::tac::{
    BinOp, Callee, Const, FunBody, Instr, Operand, RuntimeFn, TacOutput, TempId, Term, UnOp,
};

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
    let fun = tac
        .funs
        .iter()
        .find(|f| interner.resolve(defs.get(f.def).name) == name)
        .ok_or_else(|| InterpError::UnknownFun(name.into()))?;
    run_fun(tac, tys, defs, interner, fun, args)
}

pub fn run_fun(
    tac: &TacOutput,
    tys: &TyArena,
    defs: &DefArena,
    interner: &Interner,
    fun: &FunBody,
    args: &[Value],
) -> Result<Value, InterpError> {
    let _ = defs; // 签名与 emit 对称（名字查找在 run_named 消费）
    if fun.params.len() != args.len() {
        return Err(InterpError::Arity);
    }
    let mut machine = Machine {
        tac,
        tys,
        interner,
        strings: Vec::new(),
    };
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
                Instr::Store { base, .. } => Some(base.0),
                Instr::Call { dst, .. } => dst.map(|d| d.0),
                Instr::Param { .. } => None,
            })
        }))
        .max()
        .unwrap_or(0) as usize
        + 1;
    let mut cells: Vec<Vec<u64>> = vec![Vec::new(); n];
    for ((t, ty), arg) in fun.params.iter().zip(args) {
        machine.bind_arg(&mut cells[t.0 as usize], *ty, arg)?;
    }
    machine.exec(fun, &mut cells, 0).map(|ret| match ret {
        Some((v, ty)) => machine.decode(v, ty),
        None => Value::Unit,
    })
}

struct Machine<'a> {
    tac: &'a TacOutput,
    tys: &'a TyArena,
    interner: &'a Interner,
    strings: Vec<String>,
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
                cell[0] = *v as u64;
                Ok(())
            }
            (_, Value::F(v)) => {
                cell.resize(1.max(cell.len()), 0);
                cell[0] = v.to_bits();
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
        match self.tys.kind(ty) {
            TyKind::Int(_) => Value::I(v as i64),
            TyKind::Float(_) => Value::F(f64::from_bits(v)),
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
            _ => Value::I(v as i64),
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
                // 字符串常量操作数：经侧表——Machine 不可变，此处用占位，
                // Str 操作数只出现在 Param（interp 的 Param 处理特判）
                Const::Str(_) => 0,
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
                let is_float = matches!(self.tys.kind(*ty), TyKind::Float(_));
                let v = if is_float {
                    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
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
                    f.to_bits()
                } else {
                    let (x, y) = (a as i64, b as i64);
                    match op {
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
                        BinOp::Shl => (x << (y & 63)) as u64,
                        BinOp::Shr => (x >> (y & 63)) as u64,
                        BinOp::Eq => return self.set_bool(cells, dst, x == y),
                        BinOp::Ne => return self.set_bool(cells, dst, x != y),
                        BinOp::Lt => return self.set_bool(cells, dst, x < y),
                        BinOp::Le => return self.set_bool(cells, dst, x <= y),
                        BinOp::Gt => return self.set_bool(cells, dst, x > y),
                        BinOp::Ge => return self.set_bool(cells, dst, x >= y),
                    }
                };
                Self::put(cells, *dst, v);
                Ok(())
            }
            Instr::Un { dst, op, a, ty } => {
                let v = self.op(cells, a);
                let out = match op {
                    UnOp::Neg => {
                        if matches!(self.tys.kind(*ty), TyKind::Float(_)) {
                            (-f64::from_bits(v)).to_bits()
                        } else {
                            (v as i64).wrapping_neg() as u64
                        }
                    }
                    UnOp::Not => u64::from(v == 0),
                    UnOp::BitNot => !(v as i64) as u64,
                    UnOp::Conv(from) => self.conv(*from, *ty, v),
                };
                Self::put(cells, *dst, out);
                Ok(())
            }
            Instr::Copy { dst, src, .. } => {
                let v = self.op(cells, src);
                Self::put(cells, *dst, v);
                Ok(())
            }
            Instr::Load { dst, base, idx, .. } => {
                let i = self.op(cells, idx) as usize;
                let v = cells[base.0 as usize].get(i).copied().unwrap_or(0);
                Self::put(cells, *dst, v);
                Ok(())
            }
            Instr::Store { base, idx, src, .. } => {
                let i = self.op(cells, idx) as usize;
                let v = self.op(cells, src);
                let slot = &mut cells[base.0 as usize];
                if slot.len() <= i {
                    slot.resize(i + 1, 0);
                }
                slot[i] = v;
                Ok(())
            }
            Instr::Slot { dst, count } => {
                let slot = &mut cells[dst.0 as usize];
                if slot.len() < *count as usize {
                    slot.resize(*count as usize, 0);
                }
                Ok(())
            }
            Instr::Param { a, ty } => {
                // Const::Str 操作数在 Param 位：经侧表取 id
                let v = match a {
                    Operand::Const(Const::Str(sym)) => {
                        let s = self.interner.resolve(*sym);
                        self.str_id(s)
                    }
                    _ => self.op(cells, a),
                };
                pending.push((v, *ty));
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
                            _ => return Err(InterpError::Trap("strconv ty")),
                        };
                        self.str_id(&s)
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

    fn conv(&self, from: sem::TyId, to: sem::TyId, v: u64) -> u64 {
        use TyKind::*;
        match (self.tys.kind(from), self.tys.kind(to)) {
            (Float(_), Int(_)) => (f64::from_bits(v) as i64) as u64,
            (Int(_), Float(_)) => ((v as i64) as f64).to_bits(),
            (Float(FloatKind::F64), Float(FloatKind::F32))
            | (Float(FloatKind::F32), Float(FloatKind::F64)) => v,
            _ => v,
        }
    }
}

// 静默引用（arity 与 def 路径的防御性使用）
#[allow(dead_code)]
fn _use(defs: &DefArena, d: DefId) -> bool {
    defs.len() > d.0 as usize
}
