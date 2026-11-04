//! Starry 中间代码优化（opt）：TAC 上的教科书局部优化（龙书 8.4–8.5）。
//!
//! ```text
//! TacOutput ──→ fold（常量折叠：Bin/Un 双常数 → Copy const；Br 定值 → Jmp；
//!                     在指令 `ty` 的位宽上算——见 `Domain`）
//!           ──→ copy（复制传播：块内 x = y 使用点替换，Call 为屏障）
//!           ──→ cse（局部公共子表达式：块内可用表达式表，交换律归一）
//!           ──→ dce（死代码消除：全函数使用集——val temp 跨块引用，
//!                     块内扫描会误删——纯计算未使用即删，Call/Load/Store/Param/Slot 永不删）
//!           ──→ licm（循环不变外提：支配集定回边/自然循环；无 trap 族
//!                     Bin/Un 且操作数循环外定值 → preheader）
//!           ──→ （块级不动点，至无变化；cap 8）
//! ```
//!
//! 契约依据 `docs/syntax/opt-notes.md`。全部纯函数、纯 std、确定性
//! （HashMap/HashSet，pass 不迭代它们、只有点查——确定性不依赖迭代序）；不改名/复用 TempId（空洞合法），pass 间
//! diff 只显示真实变化。v1 全部块局部（全局数据流 = 扩展位，ch9）。

use std::collections::{HashMap, HashSet};

use irc::tac::{BinOp, BlockId, Const, FunBody, Instr, Operand, TacOutput, TempId, Term, UnOp};
use sem::{FloatKind, IntKind, TyArena, TyId, TyKind};

/// 优化入口：逐函数跑块级不动点。`tys` 只供常量折叠取 `ty` 的位宽
/// （折叠在指令位宽上做，见 `Domain`），其余 pass 不看类型。
pub fn optimize(tac: &mut TacOutput, tys: &TyArena) {
    // API 级契约门（opt-notes「有 Error 则 opt 不启动」）：直接调库
    // 也不能带诊断的 TAC 进优化器
    if !tac.diags.is_empty() {
        return;
    }
    for fun in &mut tac.funs {
        optimize_fun(fun, tys);
    }
}

fn optimize_fun(fun: &mut FunBody, tys: &TyArena) {
    for _ in 0..8 {
        let mut changed = false;
        changed |= fold_fun(fun, tys);
        changed |= copy_fun(fun);
        changed |= cse_fun(fun);
        changed |= dce_fun(fun);
        changed |= licm_fun(fun);
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
            | Instr::LoadGlobal { .. }
            | Instr::StoreGlobal { .. }
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

/// 折叠语义域：**折叠必须在指令 `ty` 的位宽上做**，不是 i128
/// （2026-10-31 折叠位宽修正案）。未优化代码由 llvme 发成 `ty` 位宽的
/// LLVM 指令、按 N 位回绕；在 i128 上折叠会让 Div/Rem/Shr 这类非模运算
/// 折出与未优化代码不同的值（`(2000000000 + 2000000000) / 2 : i32`
/// 曾折成 2000000000，未优化算 -147483648）。加/减/乘是模运算，末位
/// 截断后"碰巧"位正确，比较则必须比较规约后的操作数。
/// 聚合/字符串/unit 无算术域，不折叠。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Domain {
    Int(u32),
    F32,
    F64,
    Bool,
}

fn domain_of(tys: &TyArena, ty: TyId) -> Option<Domain> {
    Some(match tys.kind(ty) {
        TyKind::Int(IntKind::I4) => Domain::Int(4),
        TyKind::Int(IntKind::I8) => Domain::Int(8),
        TyKind::Int(IntKind::I16) => Domain::Int(16),
        TyKind::Int(IntKind::I32) => Domain::Int(32),
        TyKind::Int(IntKind::I64) => Domain::Int(64),
        TyKind::Int(IntKind::I128) => Domain::Int(128),
        // llvme 把 char 发 i32（llvme::ty::scalar_ty），折叠同位宽
        TyKind::Char => Domain::Int(32),
        TyKind::Float(FloatKind::F32) => Domain::F32,
        TyKind::Float(FloatKind::F64) => Domain::F64,
        TyKind::Bool => Domain::Bool,
        _ => return None,
    })
}

/// 整数按 N 位补码规约（低位截断 + 符号扩展回 i128）——与 LLVM 对 iN
/// 常量/指令的截断语义一致。折叠结果经它规约后必落在 `ty` 值域内，
/// 发射端不再出现超位宽字面量。
fn sx(v: i128, w: u32) -> i128 {
    debug_assert!((1..=128).contains(&w));
    let s = 128 - w;
    (v << s) >> s
}

fn fold_fun(fun: &mut FunBody, tys: &TyArena) -> bool {
    let mut changed = false;
    for block in &mut fun.blocks {
        for instr in &mut block.instrs {
            let folded = match instr {
                Instr::Bin { op, l, r, ty, .. } => match (l, r) {
                    (Operand::Const(l), Operand::Const(r)) => {
                        domain_of(tys, *ty).and_then(|d| fold_bin(*op, l, r, d))
                    }
                    _ => None,
                },
                Instr::Un { op, a, ty, .. } => {
                    // Conv 折叠 = 扩展位（opt-notes），v1 跳过
                    if matches!(op, UnOp::Conv(_)) {
                        None
                    } else if let Operand::Const(c) = a {
                        domain_of(tys, *ty).and_then(|d| fold_un(*op, c, d))
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

/// 双操作数常量折叠：在 `dom` 的位宽上算，结果规约回该位宽。
fn fold_bin(op: BinOp, l: &Const, r: &Const, dom: Domain) -> Option<Const> {
    use BinOp::*;
    match dom {
        Domain::Int(w) => {
            let (Const::Int(l), Const::Int(r)) = (l, r) else {
                return None;
            };
            // 先按位宽规约操作数：未优化代码在 N 位上看的就是规约后的值
            let (l, r) = (sx(*l, w), sx(*r, w));
            let i = match op {
                Add => l.wrapping_add(r),
                Sub => l.wrapping_sub(r),
                Mul => l.wrapping_mul(r),
                // 除零不折叠：不发明 trap 语义。判据是规约后的除数——
                // i8 域里 `x / 256` 的除数规约后是 0，未优化代码同样除零
                Div if r != 0 => l.wrapping_div(r),
                Rem if r != 0 => l.wrapping_rem(r),
                Lt => return Some(Const::Bool(l < r)),
                Le => return Some(Const::Bool(l <= r)),
                Gt => return Some(Const::Bool(l > r)),
                Ge => return Some(Const::Bool(l >= r)),
                Eq => return Some(Const::Bool(l == r)),
                Ne => return Some(Const::Bool(l != r)),
                BitAnd => l & r,
                BitOr => l | r,
                BitXor => l ^ r,
                // 越界移位不折叠：不发明 UB 语义。判据是目标位宽，不是
                // i128 的 128——i32 域的 `1 << 40` 未优化代码是越界移位
                Shl if (0..i128::from(w)).contains(&r) => l.wrapping_shl(r as u32),
                Shr if (0..i128::from(w)).contains(&r) => l.wrapping_shr(r as u32),
                _ => return None,
            };
            Some(Const::Int(sx(i, w)))
        }
        Domain::F32 => {
            let (Const::Float(l), Const::Float(r)) = (l, r) else {
                return None;
            };
            // f32 域逐次舍入到单精度：与 LLVM 的 fadd/fcmp float 一致，
            // 在 f64 上算再舍入会差一个中间结果的舍入
            let (l, r) = (*l as f32, *r as f32);
            let f = match op {
                Add => l + r,
                Sub => l - r,
                Mul => l * r,
                Div => l / r,
                // frem 位精确往返复杂，v1 跳过
                Lt => return Some(Const::Bool(l < r)),
                Le => return Some(Const::Bool(l <= r)),
                Gt => return Some(Const::Bool(l > r)),
                Ge => return Some(Const::Bool(l >= r)),
                Eq => return Some(Const::Bool(l == r)),
                Ne => return Some(Const::Bool(l != r)),
                _ => return None,
            };
            Some(Const::Float(f as f64))
        }
        Domain::F64 => {
            let (Const::Float(l), Const::Float(r)) = (l, r) else {
                return None;
            };
            match op {
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
            }
        }
        Domain::Bool => {
            let (Const::Bool(l), Const::Bool(r)) = (l, r) else {
                return None;
            };
            match op {
                Eq => Some(Const::Bool(l == r)),
                Ne => Some(Const::Bool(l != r)),
                _ => None,
            }
        }
    }
}

fn fold_un(op: UnOp, c: &Const, dom: Domain) -> Option<Const> {
    // `!` 是逻辑取反（irc 解释器按 v == 0 求值），只在 bool 常量上折叠
    if let (UnOp::Not, Const::Bool(b)) = (op, c) {
        return Some(Const::Bool(!b));
    }
    match dom {
        Domain::Int(w) => {
            let Const::Int(i) = c else {
                return None;
            };
            match op {
                UnOp::Neg => Some(Const::Int(sx((*i).wrapping_neg(), w))),
                UnOp::BitNot => Some(Const::Int(sx(!*i, w))),
                _ => None,
            }
        }
        Domain::F32 => {
            let Const::Float(f) = c else {
                return None;
            };
            match op {
                UnOp::Neg => Some(Const::Float((-(*f as f32)) as f64)),
                _ => None,
            }
        }
        Domain::F64 => {
            let Const::Float(f) = c else {
                return None;
            };
            match op {
                UnOp::Neg => Some(Const::Float(-*f)),
                _ => None,
            }
        }
        Domain::Bool => None,
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
                Instr::StoreGlobal { src, .. } => subst(src, &map, &mut changed),
                Instr::Param { a, .. } => subst(a, &map, &mut changed),
                Instr::LoadGlobal { .. } | Instr::Call { .. } | Instr::Slot { .. } => {}
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
    // 活性 = 全函数使用集：val temp 跨块引用合法（前驱支配后继，见
    // opt-notes 数据布局），块内扫描会把"只在后继块使用"的定义误删。
    // temp 单定义（irc 发射纪律），"函数内无任何使用"即精确死亡判定；
    // 级联（消费者删后生产者变死）由不动点循环消化。
    let mut used: HashSet<TempId> = HashSet::new();
    for block in &fun.blocks {
        match &block.term {
            Term::Ret {
                value: Some(Operand::Temp(t)),
                ..
            }
            | Term::Br {
                cond: Operand::Temp(t),
                ..
            } => {
                used.insert(*t);
            }
            _ => {}
        }
        for instr in &block.instrs {
            match instr {
                Instr::Bin { l, r, .. } => {
                    mark(l, &mut used);
                    mark(r, &mut used);
                }
                Instr::Un { a, .. } | Instr::Copy { src: a, .. } | Instr::Param { a, .. } => {
                    mark(a, &mut used)
                }
                Instr::Load { base, idx, .. } => {
                    used.insert(*base);
                    mark(idx, &mut used);
                }
                Instr::Store { base, idx, src, .. } => {
                    used.insert(*base);
                    mark(idx, &mut used);
                    mark(src, &mut used);
                }
                Instr::StoreGlobal { src, .. } => mark(src, &mut used),
                // Call/LoadGlobal 无读操作数；Slot 无操作数
                Instr::Call { .. } | Instr::LoadGlobal { .. } | Instr::Slot { .. } => {}
            }
        }
    }
    let mut changed = false;
    for block in &mut fun.blocks {
        block.instrs.retain(|instr| {
            if has_side_effects(instr) {
                return true;
            }
            match def_temp(instr) {
                Some(dst) if !used.contains(&dst) => {
                    changed = true;
                    false
                }
                _ => true,
            }
        });
    }
    changed
}

// ── 循环不变代码外提（龙书 9.7） ─────────────────────────

/// v1 可提升算子：纯计算且无 trap/UB 族——Div/Rem（除零）与 Shl/Shr
/// （越界移位）在零次执行循环里外提会发明陷阱语义，保守排除。
fn hoistable_bin(op: BinOp) -> bool {
    !matches!(op, BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr)
}

/// 前驱表（Jmp/Br 目标；Ret/Unreachable 无出边）
fn preds_of(fun: &FunBody) -> HashMap<BlockId, Vec<BlockId>> {
    let mut preds: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for block in &fun.blocks {
        let targets: &[BlockId] = match &block.term {
            Term::Jmp(t) => std::slice::from_ref(t),
            Term::Br { then_, else_, .. } => &[*then_, *else_],
            _ => &[],
        };
        for t in targets {
            preds.entry(*t).or_default().push(block.id);
        }
    }
    preds
}

/// 支配集（数据流不动点：dom[entry]={entry}，dom[b]={b} ∪ ∩ dom[pred]）。
fn dominators(
    fun: &FunBody,
    preds: &HashMap<BlockId, Vec<BlockId>>,
) -> HashMap<BlockId, HashSet<BlockId>> {
    let all: HashSet<BlockId> = fun.blocks.iter().map(|b| b.id).collect();
    let mut dom: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
    for block in &fun.blocks {
        dom.insert(block.id, all.clone());
    }
    dom.insert(fun.entry, [fun.entry].into_iter().collect());
    // 块数编译期规模，不动点直接收敛
    loop {
        let mut changed = false;
        for block in &fun.blocks {
            if block.id == fun.entry {
                continue;
            }
            let Some(ps) = preds.get(&block.id) else {
                continue; // 不可达块：dom 保持全集，不支配任何回边判定
            };
            let mut new: HashSet<BlockId> = match ps.first() {
                Some(first) => dom[first].clone(),
                None => continue,
            };
            for p in &ps[1..] {
                new.retain(|d| dom[p].contains(d));
            }
            new.insert(block.id);
            if new != dom[&block.id] {
                dom.insert(block.id, new);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dom
}

/// temp 定义块（参数视为 entry 定义）。irc 纪律：temp 单定义，唯一
/// 例外是短路布尔/in 扫描的 phi 汇合物化 temp（两分支块各写一次）——
/// 多定义记 None：其值随路径而变，LICM 据此永不判其循环不变。
fn def_blocks(fun: &FunBody) -> HashMap<TempId, Option<BlockId>> {
    let mut defs: HashMap<TempId, Option<BlockId>> = HashMap::new();
    for (t, _) in &fun.params {
        defs.insert(*t, Some(fun.entry));
    }
    for block in &fun.blocks {
        for instr in &block.instrs {
            if let Some(dst) = def_temp(instr) {
                defs.entry(dst)
                    .and_modify(|d| {
                        if *d != Some(block.id) {
                            *d = None;
                        }
                    })
                    .or_insert(Some(block.id));
            }
        }
    }
    defs
}

fn licm_fun(fun: &mut FunBody) -> bool {
    let preds = preds_of(fun);
    let dom = dominators(fun, &preds);
    let defs = def_blocks(fun);
    let mut changed = false;
    // 回边：header 支配 source（irc 结构化输出下即循环尾部跳转）
    let mut backedges: Vec<(BlockId, BlockId)> = Vec::new();
    for block in &fun.blocks {
        let targets: &[BlockId] = match &block.term {
            Term::Jmp(t) => std::slice::from_ref(t),
            Term::Br { then_, else_, .. } => &[*then_, *else_],
            _ => &[],
        };
        for t in targets {
            if *t != block.id && dom[&block.id].contains(t) {
                backedges.push((*t, block.id));
            }
        }
    }
    backedges.sort_by_key(|(h, s)| (h.0, s.0));
    for (header, source) in backedges {
        // 自然循环：沿前驱从 source 回溯到 header（含两端）
        let mut loop_set: HashSet<BlockId> = [header, source].into_iter().collect();
        let mut stack = vec![source];
        while let Some(n) = stack.pop() {
            if n == header {
                continue;
            }
            for p in preds.get(&n).into_iter().flatten() {
                if loop_set.insert(*p) {
                    stack.push(*p);
                }
            }
        }
        // preheader = header 唯一循环外前驱，且无条件进 header（Jmp），
        // 否则外提点是有条件的，零次执行循环会发明计算
        let outside: Vec<BlockId> = match preds.get(&header) {
            Some(ps) => ps
                .iter()
                .copied()
                .filter(|p| !loop_set.contains(p))
                .collect(),
            None => continue,
        };
        let [pre] = outside[..] else {
            continue;
        };
        let Some(pre_idx) = fun.blocks.iter().position(|b| b.id == pre) else {
            continue;
        };
        if !matches!(fun.blocks[pre_idx].term, Term::Jmp(h) if h == header) {
            continue;
        }
        // 提升：循环内（块序保持）无 trap 族 Bin/Un 且全部操作数
        // 定值在循环外——单次扫描 + retain，每条指令至多提升一次
        let invariant = |instr: &Instr| -> bool {
            let (l, r) = match instr {
                Instr::Bin { op, l, r, .. } if hoistable_bin(*op) => (l, r),
                _ => return false,
            };
            [l, r].into_iter().all(|op| match op {
                Operand::Const(_) => true,
                Operand::Temp(t) => match defs.get(t).and_then(|d| *d) {
                    Some(b) => !loop_set.contains(&b),
                    // 多定义（phi 汇合）或未知 temp：不提升
                    None => false,
                },
            })
        };
        let mut hoisted: Vec<Instr> = Vec::new();
        for block in &fun.blocks {
            if loop_set.contains(&block.id) {
                hoisted.extend(block.instrs.iter().filter(|i| invariant(i)).cloned());
            }
        }
        if hoisted.is_empty() {
            continue;
        }
        for block in &mut fun.blocks {
            if loop_set.contains(&block.id) {
                block.instrs.retain(|i| !invariant(i));
            }
        }
        // preheader 尾部（终止符前）追加；块序遍历保证 def-use 序
        fun.blocks[pre_idx].instrs.extend(hoisted);
        changed = true;
    }
    changed
}

fn mark(op: &Operand, live: &mut HashSet<TempId>) {
    if let Operand::Temp(t) = op {
        live.insert(*t);
    }
}

#[cfg(test)]
mod tests {
    //! 折叠位宽的单元钉板：窄位宽用例在 surface 语法里做不出来
    //! （字面量默认 i32，且提升格只加宽），直接钉 fold_bin/fold_un。

    use super::*;

    /// 取折叠结果的整数值（`Const` 无 PartialEq，按变体断言）
    fn int(c: Option<Const>) -> i128 {
        match c {
            Some(Const::Int(i)) => i,
            other => panic!("不是整型常量: {other:?}"),
        }
    }

    fn boolean(c: Option<Const>) -> bool {
        match c {
            Some(Const::Bool(b)) => b,
            other => panic!("不是布尔常量: {other:?}"),
        }
    }

    fn float(c: Option<Const>) -> f64 {
        match c {
            Some(Const::Float(f)) => f,
            other => panic!("不是浮点常量: {other:?}"),
        }
    }

    fn add32(l: i128, r: i128) -> i128 {
        int(fold_bin(
            BinOp::Add,
            &Const::Int(l),
            &Const::Int(r),
            Domain::Int(32),
        ))
    }

    #[test]
    fn sx_narrows_to_width() {
        assert_eq!(sx(4_000_000_000, 32), -294_967_296);
        assert_eq!(sx(255, 8), -1);
        assert_eq!(sx(256, 8), 0);
        assert_eq!(sx(-1, 8), -1);
        assert_eq!(sx(i128::MAX, 128), i128::MAX);
    }

    #[test]
    fn fold_add_wraps_at_type_width() {
        // i32 域回绕，不是 i128 的 4000000000
        assert_eq!(add32(2_000_000_000, 2_000_000_000), -294_967_296);
    }

    #[test]
    fn fold_div_sees_wrapped_operand() {
        // 回绕后再除：i32 域 -294967296 / 2 = -147483648
        // （旧的 i128 折叠给出 2000000000，与未优化代码不同值）
        let sum = Const::Int(add32(2_000_000_000, 2_000_000_000));
        assert_eq!(
            int(fold_bin(BinOp::Div, &sum, &Const::Int(2), Domain::Int(32))),
            -147_483_648
        );
    }

    #[test]
    fn fold_compares_narrowed_operands() {
        // (2000000000 + 2000000000) < 0 在 i32 域为真（回绕成负数）
        let sum = Const::Int(add32(2_000_000_000, 2_000_000_000));
        assert!(boolean(fold_bin(
            BinOp::Lt,
            &sum,
            &Const::Int(0),
            Domain::Int(32)
        )));
    }

    #[test]
    fn fold_shift_guard_uses_type_width() {
        // i32 域移位量 ≥ 32 不折叠（不发明 UB）；31 仍在域内
        assert!(fold_bin(BinOp::Shl, &Const::Int(1), &Const::Int(40), Domain::Int(32)).is_none());
        assert_eq!(
            int(fold_bin(
                BinOp::Shl,
                &Const::Int(1),
                &Const::Int(31),
                Domain::Int(32)
            )),
            i32::MIN as i128
        );
    }

    #[test]
    fn fold_divisor_zero_after_narrowing_not_folded() {
        // i8 域里 `x / 256` 的除数规约后是 0：未优化代码同样除零，不折叠
        assert!(fold_bin(BinOp::Div, &Const::Int(7), &Const::Int(256), Domain::Int(8)).is_none());
    }

    #[test]
    fn fold_un_wraps_at_type_width() {
        assert_eq!(
            int(fold_un(UnOp::BitNot, &Const::Int(5), Domain::Int(8))),
            -6
        );
        // i8 的 -128 取负回绕成自己
        assert_eq!(
            int(fold_un(UnOp::Neg, &Const::Int(-128), Domain::Int(8))),
            -128
        );
    }

    #[test]
    fn fold_f32_rounds_to_single_precision() {
        // f32 域 1.0 + 2^-24 舍入回 1.0；f64 域给出 1.0000000596046448。
        // （f32 目前在 surface 做不出来：f64 字面量不收窄进 f32 变量，
        //   与窄位宽整型同因——故直接钉 Domain 层）
        let (one, eps) = (Const::Float(1.0), Const::Float(2f64.powi(-24)));
        assert_eq!(float(fold_bin(BinOp::Add, &one, &eps, Domain::F32)), 1.0);
        assert_ne!(float(fold_bin(BinOp::Add, &one, &eps, Domain::F64)), 1.0);
    }

    #[test]
    fn domain_of_refuses_non_scalar_and_pins_widths() {
        let mut tys = TyArena::new();
        assert_eq!(domain_of(&tys, tys.default_int), Some(Domain::Int(32)));
        assert_eq!(domain_of(&tys, tys.bool_), Some(Domain::Bool));
        assert_eq!(domain_of(&tys, tys.char_), Some(Domain::Int(32)));
        let i128_ty = tys.intern(TyKind::Int(IntKind::I128));
        assert_eq!(domain_of(&tys, i128_ty), Some(Domain::Int(128)));
        let f32_ty = tys.intern(TyKind::Float(FloatKind::F32));
        assert_eq!(domain_of(&tys, f32_ty), Some(Domain::F32));
        // 聚合/字符串/unit 无算术域：不折叠
        assert_eq!(domain_of(&tys, tys.unit), None);
        assert_eq!(domain_of(&tys, tys.string), None);
    }
}
