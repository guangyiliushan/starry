//! Starry 中间代码优化（opt）：TAC 上的教科书局部优化（龙书 8.4–8.5）。
//!
//! ```text
//! TacOutput ──→ fold（常量折叠：Bin/Un 双常数 → Copy const；Br 定值 → Jmp）
//!           ──→ copy（复制传播：块内 x = y 使用点替换，Call 为屏障）
//!           ──→ cse（局部公共子表达式：块内可用表达式表，交换律归一）
//!           ──→ dce（死代码消除：结果未使用的纯计算删除，Call/Load/
//!                     Store/Param/Slot 永不删）
//!           ──→ （块级不动点，至无变化；cap 8）
//! ```
//!
//! 契约依据 `docs/syntax/opt-notes.md`。全部纯函数、纯 std、确定性
//! （HashMap/HashSet，pass 不迭代它们、只有点查——确定性不依赖迭代序）；不改名/复用 TempId（空洞合法），pass 间
//! diff 只显示真实变化。v1 全部块局部（全局数据流 = 扩展位，ch9）。

use std::collections::{HashMap, HashSet};

use irc::tac::{BinOp, Const, FunBody, Instr, Operand, TacOutput, TempId, Term, UnOp};

/// 优化入口：逐函数跑块级不动点。
pub fn optimize(tac: &mut TacOutput) {
    for fun in &mut tac.funs {
        optimize_fun(fun);
    }
}

fn optimize_fun(fun: &mut FunBody) {
    for _ in 0..8 {
        let mut changed = false;
        changed |= fold_fun(fun);
        changed |= copy_fun(fun);
        changed |= cse_fun(fun);
        changed |= dce_fun(fun);
        if !changed {
            break;
        }
    }
}

/// 副作用谓词（契约钉死）：这些指令 DCE 永不删。
fn has_side_effects(instr: &Instr) -> bool {
    matches!(
        instr,
        Instr::Load { .. }
            | Instr::Store { .. }
            | Instr::Param { .. }
            | Instr::Call { .. }
            | Instr::Slot { .. }
    )
}

/// 指令写的 Temp（定义点）；Store/Param/Call(None) 不写。
fn def_temp(instr: &Instr) -> Option<TempId> {
    match instr {
        Instr::Bin { dst, .. }
        | Instr::Un { dst, .. }
        | Instr::Copy { dst, .. }
        | Instr::Load { dst, .. }
        | Instr::Slot { dst, .. } => Some(*dst),
        Instr::Call { dst: Some(dst), .. } => Some(*dst),
        _ => None,
    }
}

// ── 常量折叠 ──────────────────────────────────────────────

fn fold_fun(fun: &mut FunBody) -> bool {
    let mut changed = false;
    for block in &mut fun.blocks {
        for instr in &mut block.instrs {
            let folded = match instr {
                Instr::Bin { op, l, r, .. } => match (l, r) {
                    (Operand::Const(l), Operand::Const(r)) => fold_bin(*op, l, r),
                    _ => None,
                },
                Instr::Un { op, a, .. } => {
                    // Conv 折叠 = 扩展位（opt-notes），v1 跳过
                    if matches!(op, UnOp::Conv(_)) {
                        None
                    } else if let Operand::Const(c) = a {
                        fold_un(*op, c)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(c) = folded {
                let (dst, ty) = match instr {
                    Instr::Bin { dst, ty, .. } | Instr::Un { dst, ty, .. } => (*dst, *ty),
                    _ => unreachable!("folded instr is Bin/Un by construction"),
                };
                *instr = Instr::Copy {
                    dst,
                    src: Operand::Const(c),
                    ty,
                };
                changed = true;
            }
        }
        if let Term::Br {
            cond: Operand::Const(Const::Bool(b)),
            then_,
            else_,
        } = &block.term
        {
            block.term = Term::Jmp(if *b { *then_ } else { *else_ });
            changed = true;
        }
    }
    changed
}

fn fold_bin(op: BinOp, l: &Const, r: &Const) -> Option<Const> {
    use BinOp::*;
    match (l, r) {
        (Const::Int(l), Const::Int(r)) => {
            let i = match op {
                Add => l.wrapping_add(*r),
                Sub => l.wrapping_sub(*r),
                Mul => l.wrapping_mul(*r),
                // 除零不折叠：不发明 trap 语义
                Div if *r != 0 => l.wrapping_div(*r),
                Rem if *r != 0 => l.wrapping_rem(*r),
                Lt => return Some(Const::Bool(l < r)),
                Le => return Some(Const::Bool(l <= r)),
                Gt => return Some(Const::Bool(l > r)),
                Ge => return Some(Const::Bool(l >= r)),
                Eq => return Some(Const::Bool(l == r)),
                Ne => return Some(Const::Bool(l != r)),
                BitAnd => l & r,
                BitOr => l | r,
                BitXor => l ^ r,
                // 越界移位不折叠：不发明 UB 语义
                Shl if (0..128).contains(r) => l.wrapping_shl(*r as u32),
                Shr if (0..128).contains(r) => l.wrapping_shr(*r as u32),
                _ => return None,
            };
            Some(Const::Int(i))
        }
        (Const::Float(l), Const::Float(r)) => match op {
            Add => Some(Const::Float(l + r)),
            Sub => Some(Const::Float(l - r)),
            Mul => Some(Const::Float(l * r)),
            Div => Some(Const::Float(l / r)),
            // frem 位精确往返复杂，v1 跳过
            Lt => Some(Const::Bool(l < r)),
            Le => Some(Const::Bool(l <= r)),
            Gt => Some(Const::Bool(l > r)),
            Ge => Some(Const::Bool(l >= r)),
            Eq => Some(Const::Bool(l == r)),
            Ne => Some(Const::Bool(l != r)),
            _ => None,
        },
        (Const::Bool(l), Const::Bool(r)) => match op {
            Eq => Some(Const::Bool(l == r)),
            Ne => Some(Const::Bool(l != r)),
            _ => None,
        },
        _ => None,
    }
}

fn fold_un(op: UnOp, c: &Const) -> Option<Const> {
    match (op, c) {
        (UnOp::Neg, Const::Int(i)) => Some(Const::Int(i.wrapping_neg())),
        (UnOp::Neg, Const::Float(f)) => Some(Const::Float(-f)),
        (UnOp::Not, Const::Bool(b)) => Some(Const::Bool(!b)),
        (UnOp::BitNot, Const::Int(i)) => Some(Const::Int(!i)),
        _ => None,
    }
}

// ── 复制传播 ──────────────────────────────────────────────

fn copy_fun(fun: &mut FunBody) -> bool {
    let mut changed = false;
    for block in &mut fun.blocks {
        // dst → 等价操作数（块内有效；定义点/Call 失效）
        let mut map: HashMap<TempId, Operand> = HashMap::new();
        for instr in &mut block.instrs {
            if matches!(instr, Instr::Call { .. }) {
                // Call 不透明副作用：保守清空（教科书保守假设）
                map.clear();
            }
            match instr {
                Instr::Bin { l, r, .. } => {
                    subst(l, &map, &mut changed);
                    subst(r, &map, &mut changed);
                }
                Instr::Un { a, .. } => subst(a, &map, &mut changed),
                Instr::Copy { src, .. } => subst(src, &map, &mut changed),
                Instr::Load { idx, .. } => subst(idx, &map, &mut changed),
                Instr::Store { idx, src, .. } => {
                    subst(idx, &map, &mut changed);
                    subst(src, &map, &mut changed);
                }
                Instr::Param { a, .. } => subst(a, &map, &mut changed),
                Instr::Call { .. } | Instr::Slot { .. } => {}
            }
            match instr {
                Instr::Copy { dst, src, .. } => {
                    map.remove(dst);
                    map.insert(*dst, src.clone());
                }
                _ => {
                    if let Some(dst) = def_temp(instr) {
                        map.remove(&dst);
                    }
                }
            }
        }
        match &mut block.term {
            Term::Br { cond, .. }
            | Term::Ret {
                value: Some(cond), ..
            } => subst(cond, &map, &mut changed),
            _ => {}
        }
    }
    changed
}

fn subst(op: &mut Operand, map: &HashMap<TempId, Operand>, changed: &mut bool) {
    if let Some(new) = match op {
        Operand::Temp(t) => map.get(t),
        _ => None,
    } {
        *op = new.clone();
        *changed = true;
    }
}

// ── 局部 CSE ──────────────────────────────────────────────

/// 可用表达式键：Bin/Un 的纯值指纹（交换律归一；Float 按位比较）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Bin { op: u8, l: OpKey, r: OpKey }, // op = BinOp as u8（irc 侧无 Hash/Ord derive）
    UnNeg(OpKey),
    UnNot(OpKey),
    UnBitNot(OpKey),
    UnConv { from: sem::TyId, a: OpKey },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum OpKey {
    Temp(u32),
    Int(i128),
    Float(u64),
    Bool(bool),
    Null,
    Unit,
}

fn op_key(op: &Operand) -> Option<OpKey> {
    Some(match op {
        Operand::Temp(t) => OpKey::Temp(t.0),
        Operand::Const(Const::Int(i)) => OpKey::Int(*i),
        Operand::Const(Const::Float(f)) => OpKey::Float(f.to_bits()),
        Operand::Const(Const::Bool(b)) => OpKey::Bool(*b),
        Operand::Const(Const::Null) => OpKey::Null,
        Operand::Const(Const::Unit) => OpKey::Unit,
        // 字符串常量不参与 CSE（无意义比较）
        Operand::Const(Const::Str(_)) => return None,
    })
}

/// 交换律算子：操作数对归一排序以命中更多等价。
fn commutative(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::Mul
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
    )
}

fn cse_fun(fun: &mut FunBody) -> bool {
    let mut changed = false;
    for block in &mut fun.blocks {
        // 表达式键 → 已算出的 Temp（块内可用表达式表，龙书 8.5）
        let mut avail: HashMap<Key, TempId> = HashMap::new();
        for instr in &mut block.instrs {
            match instr {
                Instr::Call { .. } | Instr::Store { .. } => avail.clear(),
                Instr::Bin { dst, op, l, r, ty } => {
                    let (Some(lk), Some(rk)) = (op_key(l), op_key(r)) else {
                        continue;
                    };
                    let (lk, rk) = if commutative(*op) && rk < lk {
                        (rk, lk)
                    } else {
                        (lk, rk)
                    };
                    let key = Key::Bin {
                        op: *op as u8,
                        l: lk,
                        r: rk,
                    };
                    match avail.get(&key) {
                        Some(old) => {
                            let src = Operand::Temp(*old);
                            *instr = Instr::Copy {
                                dst: *dst,
                                src,
                                ty: *ty,
                            };
                            changed = true;
                        }
                        None => {
                            avail.insert(key, *dst);
                        }
                    }
                }
                Instr::Un { dst, op, a, ty } => {
                    let Some(ak) = op_key(a) else {
                        continue;
                    };
                    let key = match op {
                        UnOp::Neg => Key::UnNeg(ak),
                        UnOp::Not => Key::UnNot(ak),
                        UnOp::BitNot => Key::UnBitNot(ak),
                        UnOp::Conv(from) => Key::UnConv { from: *from, a: ak },
                    };
                    match avail.get(&key) {
                        Some(old) => {
                            let src = Operand::Temp(*old);
                            *instr = Instr::Copy {
                                dst: *dst,
                                src,
                                ty: *ty,
                            };
                            changed = true;
                        }
                        None => {
                            avail.insert(key, *dst);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    changed
}

// ── 死代码消除 ────────────────────────────────────────────

fn dce_fun(fun: &mut FunBody) -> bool {
    let mut changed = false;
    for block in &mut fun.blocks {
        let mut live: HashSet<TempId> = HashSet::new();
        match &block.term {
            Term::Ret {
                value: Some(Operand::Temp(t)),
                ..
            }
            | Term::Br {
                cond: Operand::Temp(t),
                ..
            } => {
                live.insert(*t);
            }
            _ => {}
        }
        let mut keep = vec![true; block.instrs.len()];
        for (i, instr) in block.instrs.iter().enumerate().rev() {
            // 反向活性扫描：dst 不被后续使用且无副作用 → 死
            if !has_side_effects(instr) && !live.contains(&def_temp(instr).unwrap()) {
                keep[i] = false;
                changed = true;
                continue;
            }
            if let Some(dst) = def_temp(instr) {
                live.remove(&dst);
            }
            match instr {
                Instr::Bin { l, r, .. } => {
                    mark(l, &mut live);
                    mark(r, &mut live);
                }
                Instr::Un { a, .. } | Instr::Copy { src: a, .. } | Instr::Param { a, .. } => {
                    mark(a, &mut live)
                }
                Instr::Load { base, idx, .. } => {
                    live.insert(*base);
                    mark(idx, &mut live);
                }
                Instr::Store { base, idx, src, .. } => {
                    live.insert(*base);
                    mark(idx, &mut live);
                    mark(src, &mut live);
                }
                // Call 无读操作数；Slot 无操作数
                Instr::Call { .. } | Instr::Slot { .. } => {}
            }
        }
        if changed {
            let mut it = keep.into_iter();
            block.instrs.retain(|_| it.next().unwrap());
        }
    }
    changed
}

fn mark(op: &Operand, live: &mut HashSet<TempId>) {
    if let Operand::Temp(t) = op {
        live.insert(*t);
    }
}
