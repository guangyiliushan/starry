//! 表达式 lowering：每个 ExprWrap → Operand（Temp 或 Const）。
//! 类型取自 expr_ty 侧表；隐式提升与显式 as 统一走 `coerce` 插 Conv
//! （操作数类型均匀不变量，opt-notes）。

use ast::syntax::expr::{CastKind, ExprKind, ExprWrap};
use ast::syntax::node::{DiagCode, Span};
use ast::token::{LiteralKind, OperatorKind};
use sem::{DefKind, TyId, TyKind};

use crate::lower::{Ctx, Place};
use crate::tac::{BinOp, Const, Instr, Operand, TempId, Term, UnOp};

/// lower_jump 的目标种类（与 sem::FlowKind 对齐，避免公开依赖其私有语义）
#[derive(Debug, Clone, Copy)]
pub(crate) enum FlowTarget {
    Break,
    Continue,
}

pub(crate) fn lower_expr(ctx: &mut Ctx, e: &ExprWrap) -> Operand {
    match &e.kind {
        ExprKind::Literal(kind) => literal(ctx, *kind, e.span),
        ExprKind::Bool(b) => Operand::Const(Const::Bool(*b)),
        ExprKind::Null => nullable_null(ctx, e),
        ExprKind::Ident(name) => ident(ctx, *name, e.span),
        ExprKind::Path(path) => {
            if path.segments.len() == 1 {
                ident(ctx, path.segments[0].name, e.span)
            } else {
                ctx.diag(e.span, "多段路径的 lowering 在 M2（import 命名空间）");
                Operand::Const(Const::Unit)
            }
        }
        ExprKind::Binary { op, lhs, rhs } => binary(ctx, e, *op, lhs, rhs),
        ExprKind::Unary { op, operand } => unary(ctx, e, *op, operand),
        ExprKind::Cast { expr, kind, ty: _ } => cast(ctx, e, expr, *kind),
        ExprKind::Call {
            callee,
            type_args,
            args,
        } => call(ctx, e, callee, type_args, args),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => lower_if(ctx, e, cond, then_block, else_block.as_deref()),
        ExprKind::Unsafe { block } | ExprKind::Comptime { block } => {
            // sem 按普通块定型/核查（semantic-notes）；lowering 同纪律
            match ctx.lower_block(block) {
                Some((op, ty)) => op_or_unit(ctx, op, ty),
                None => Operand::Const(Const::Unit),
            }
        }
        ExprKind::Is { .. } | ExprKind::In { .. } => {
            ctx.diag(e.span, "is/in 的 lowering 在 M2（模式/区间 desugar）");
            Operand::Const(Const::Unit)
        }
        ExprKind::When { .. } => {
            ctx.diag(e.span, "when 的 lowering 在 M2（Br 链 + pat_res）");
            Operand::Const(Const::Unit)
        }
        ExprKind::Index { base, index } => lower_index(ctx, e, base, index),
        ExprKind::Member { base, safe, .. } => lower_member(ctx, e, base, *safe),
        ExprKind::Tuple(elems) => tuple_literal(ctx, e, elems),
        ExprKind::Array(elems) => array_literal(ctx, e, elems),
        ExprKind::ArrayRepeat { value, count } => array_repeat(ctx, e, value, *count),
        ExprKind::StringInterp { .. } => {
            ctx.diag(
                e.span,
                "字符串插值的 lowering 在 M3（stringify+concat 契约）",
            );
            Operand::Const(Const::Unit)
        }
        ExprKind::ForceUnwrap { expr } => unwrap(ctx, e, expr, ctx.expr_ty(e)),
        ExprKind::Lambda { .. }
        | ExprKind::Concurrent { .. }
        | ExprKind::Spawn(_)
        | ExprKind::Async { .. }
        | ExprKind::Await { .. }
        | ExprKind::Proceed
        | ExprKind::Super { .. }
        | ExprKind::MacroCall { .. } => {
            // sem v1 对这些构造本来就报 SemUnsupportedConstruct，门禁挡住；
            // 到达这里 = 契约破缺，诊断兜底
            ctx.diag(e.span, "sem 跳过清单构造穿透门禁（内部不变量破缺）");
            Operand::Const(Const::Unit)
        }
        ExprKind::Error(_) | ExprKind::Missing(_) => {
            ctx.diag(e.span, "错误/缺失表达式穿透 sem 门禁（内部不变量破缺）");
            Operand::Const(Const::Unit)
        }
    }
}

fn op_or_unit(ctx: &mut Ctx, op: Operand, ty: TyId) -> Operand {
    if ctx.is_unit(ty) {
        Operand::Const(Const::Unit)
    } else {
        op
    }
}

/// 标识符读取：Temp 直接用；Slot 标量发 Load，聚合逐槽拷贝出新基址
/// （var 读取 = 快照语义，sem v1 全拷贝）
fn ident(ctx: &mut Ctx, name: ast::interner::Symbol, span: Span) -> Operand {
    match ctx.lookup(name) {
        Some((place, ty)) => read_place(ctx, place, ty, span),
        None => {
            ctx.diag(span, "标识符不在 lowering 环境（sem 契约破缺）");
            Operand::Const(Const::Int(0))
        }
    }
}

/// 读取值存放处为操作数：聚合 Slot → 逐槽拷贝出新基址（快照）
pub(crate) fn read_place(ctx: &mut Ctx, place: Place, ty: TyId, span: Span) -> Operand {
    match place {
        Place::Temp(t) => Operand::Temp(t),
        Place::Slot(slot) => {
            if crate::layout::scalar_slots(ctx, ty) <= 1 {
                let dst = ctx.temp();
                ctx.emit(Instr::Load {
                    dst,
                    base: slot,
                    idx: Operand::Const(Const::Int(0)),
                    ty,
                });
                Operand::Temp(dst)
            } else {
                let base = ctx.temp();
                copy_slots(ctx, (base, 0), (Operand::Temp(slot), 0), ty, ty, span);
                Operand::Temp(base)
            }
        }
    }
}

/// 操作数必须是聚合基址 temp（构造/字面量的产物）；常量形态 = 契约破缺
fn as_base(ctx: &mut Ctx, op: Operand, span: Span) -> TempId {
    match op {
        Operand::Temp(t) => t,
        Operand::Const(_) => {
            ctx.diag(span, "聚合基址为常量（sem 契约破缺）");
            ctx.temp()
        }
    }
}

/// 逐槽拷贝（带跨槽类型转换）：src 槽 src_off+s 经 coerce 写入
/// dst[dst_off+s]。槽数不一致 = 契约破缺（诊断后置信下游不启动）。
/// 单槽快路径不插 Load（标量操作数直接 Store）。
/// 逐槽拷贝（带跨槽类型转换）：src 槽 src_off+s 经 coerce 写入
/// dst[dst_off+s]。槽数不一致 = 契约破缺（诊断后置信下游不启动）。
/// 单槽快路径不插 Load（标量操作数直接 Store）。
/// 参数以 (基址, 槽偏移) 元组成对传入。
pub(crate) fn copy_slots(
    ctx: &mut Ctx,
    dst: (TempId, usize),
    src: (Operand, usize),
    src_ty: TyId,
    dst_ty: TyId,
    span: Span,
) {
    let (dst_base, dst_off) = dst;
    let (src, src_off) = src;
    let n = crate::layout::scalar_slots(ctx, dst_ty);
    if n != crate::layout::scalar_slots(ctx, src_ty) {
        ctx.diag(span, "槽数不一致的聚合拷贝（sem 契约破缺）");
        return;
    }
    if n <= 1 {
        let sty = crate::layout::slot_ty(ctx, dst_ty, 0);
        let v = coerce(ctx, src, src_ty, sty);
        ctx.emit(Instr::Store {
            base: dst_base,
            idx: Operand::Const(Const::Int(dst_off as i128)),
            src: v,
            ty: sty,
        });
        return;
    }
    for s in 0..n {
        let from_sty = crate::layout::slot_ty(ctx, src_ty, src_off + s);
        let to_sty = crate::layout::slot_ty(ctx, dst_ty, dst_off + s);
        let v = match &src {
            Operand::Temp(b) => {
                let t = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: t,
                    base: *b,
                    idx: Operand::Const(Const::Int((src_off + s) as i128)),
                    ty: from_sty,
                });
                Operand::Temp(t)
            }
            Operand::Const(c) => Operand::Const(c.clone()),
        };
        let v = coerce(ctx, v, from_sty, to_sty);
        ctx.emit(Instr::Store {
            base: dst_base,
            idx: Operand::Const(Const::Int((dst_off + s) as i128)),
            src: v,
            ty: to_sty,
        });
    }
}

fn literal(ctx: &mut Ctx, kind: LiteralKind, span: Span) -> Operand {
    let text = span.slice(ctx.source);
    match kind {
        LiteralKind::Integer => Operand::Const(int_literal(ctx, text, span)),
        LiteralKind::Float => {
            let v: f64 = text
                .replace('_', "")
                .parse()
                .expect("sem 门禁后的浮点字面量必可解析");
            Operand::Const(Const::Float(v))
        }
        LiteralKind::String => {
            let inner = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .expect("lexer 保证引号配对");
            let decoded = unescape(ctx, inner, span);
            Operand::Const(Const::Str(ctx.intern(&decoded)))
        }
        LiteralKind::Char => {
            let inner = text
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .expect("lexer 保证引号配对");
            let unescaped = unescape(ctx, inner, span);
            // char 在 LLVM 侧为 i32（ty_name）；取第一个标量值
            let c = unescaped.chars().next().unwrap_or('\0');
            Operand::Const(Const::Int(c as i128))
        }
    }
}

/// 整数字面量取值：词法规则含 0x/0o/0b 前缀（`lexer.rs` 规则表），
/// 且全仓无任何普通字面量的值域核查（IntegerOutOfRange 只覆盖数组
/// repeat 计数）——故解析与值域诊断都在本层：越界 i128 发
/// `IntegerOutOfRange` 并回退 0（诊断后置信下游不启动）。
fn int_literal(ctx: &mut Ctx, text: &str, span: Span) -> Const {
    let cleaned: String = text.replace('_', "");
    let (digits, radix) = match cleaned.as_str() {
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0x") => (&s[2..], 16),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0o") => (&s[2..], 8),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0b") => (&s[2..], 2),
        s => (s, 10),
    };
    // 词法字符类保证 digits 合法（char 规则的 hex 位/数字位），
    // from_str_radix 只可能溢出——无 panic 路径
    match i128::from_str_radix(digits, radix) {
        Ok(v) => Const::Int(v),
        Err(_) => {
            ctx.diag_code(
                DiagCode::IntegerOutOfRange,
                span,
                "integer literal out of range",
            );
            Const::Int(0)
        }
    }
}

/// 转义序列解码，与 `parser::decode_string_token` 的接受集对齐：
/// `\n \t \r \0 \\ \" \'` 与 `\u{XXXXXX}`。未知转义保留原样——
/// 表达式字符串的词法校验是宽容的（`\\[^\r\n]` 任意字符），错误
/// 形态只可能来自 `\u{...}`，由本层发 `InvalidEscape`。
fn unescape(ctx: &mut Ctx, s: &str, span: Span) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('u') => {
                // \u{XXXXXX}：词法对 char 字面量保证良构；字符串侧任意
                let mut hex = String::new();
                let mut closed = chars.next() == Some('{');
                if closed {
                    closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        hex.push(c);
                    }
                }
                match if closed {
                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                } else {
                    None
                } {
                    Some(decoded) => out.push(decoded),
                    None => {
                        ctx.diag_code(DiagCode::InvalidEscape, span, "invalid unicode escape");
                        out.push('\u{FFFD}');
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn binop(op: OperatorKind) -> Option<BinOp> {
    Some(match op {
        OperatorKind::Plus => BinOp::Add,
        OperatorKind::Minus => BinOp::Sub,
        OperatorKind::Star => BinOp::Mul,
        OperatorKind::Slash => BinOp::Div,
        OperatorKind::Percent => BinOp::Rem,
        OperatorKind::EqEq => BinOp::Eq,
        OperatorKind::BangEq => BinOp::Ne,
        OperatorKind::Lt => BinOp::Lt,
        OperatorKind::LtEq => BinOp::Le,
        OperatorKind::Gt => BinOp::Gt,
        OperatorKind::GtEq => BinOp::Ge,
        OperatorKind::BitAnd => BinOp::BitAnd,
        OperatorKind::BitOr => BinOp::BitOr,
        OperatorKind::BitXor => BinOp::BitXor,
        OperatorKind::Shl => BinOp::Shl,
        OperatorKind::Shr => BinOp::Shr,
        _ => return None,
    })
}

fn binary(
    ctx: &mut Ctx,
    e: &ExprWrap,
    op: OperatorKind,
    lhs: &ExprWrap,
    rhs: &ExprWrap,
) -> Operand {
    if op == OperatorKind::And || op == OperatorKind::Or {
        return materialize_logic(ctx, op, lhs, rhs);
    }
    let Some(bop) = binop(op) else {
        ctx.diag(e.span, "该二元运算符无 TAC 映射（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    let l_ty = ctx.expr_ty(lhs);
    let r_ty = ctx.expr_ty(rhs);
    // ty 字段 = 操作数类型（llvme 按算子推结果型：算术 = ty，比较 = i1）。
    // 比较的操作数公共型按 sem 提升格取可指派方向；算术直接用结果型
    // （sem join 已把结果定为操作数的公共型）
    let is_cmp = matches!(
        bop,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    );
    // 操作数公共型：比较的结果型是 bool，操作数型须按提升格另算
    // （assignable(r, l) 时 l 就是公共型，否则取 r——sem 保证可汇合）
    let ty = if is_cmp {
        if ctx.sem.tys.assignable(r_ty, l_ty) {
            l_ty
        } else {
            r_ty
        }
    } else {
        ctx.expr_ty(e)
    };
    let l_op = lower_expr(ctx, lhs);
    let l = coerce(ctx, l_op, l_ty, ty);
    let r_op = lower_expr(ctx, rhs);
    let r = coerce(ctx, r_op, r_ty, ty);
    let dst = ctx.temp();
    ctx.emit(Instr::Bin {
        dst,
        op: bop,
        l,
        r,
        ty,
    });
    Operand::Temp(dst)
}

/// 短路布尔物化（龙书 6.6 跳转代码；Br 链，见 opt-notes）：
/// `l && r` / `l || r` → bool temp
fn materialize_logic(ctx: &mut Ctx, op: OperatorKind, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let is_and = op == OperatorKind::And;
    let result = ctx.temp();
    let bool_ty = ctx.expr_ty(lhs); // && 两侧恒 bool（sem SemConditionNotBool 门禁）

    let eval_rhs = ctx.reserve_block(); // 右侧求值块
    let done_true = ctx.reserve_block(); // result = true
    let done_false = ctx.reserve_block(); // result = false
    let merge = ctx.reserve_block();

    let l = lower_expr(ctx, lhs);
    let (then_, else_) = if is_and {
        (eval_rhs, done_false)
    } else {
        (done_true, eval_rhs)
    };
    ctx.terminate(Term::Br {
        cond: l,
        then_,
        else_,
    });

    ctx.switch_to(eval_rhs);
    let r = lower_expr(ctx, rhs);
    ctx.terminate(Term::Br {
        cond: r,
        then_: done_true,
        else_: done_false,
    });

    ctx.switch_to(done_true);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(true)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));

    ctx.switch_to(done_false);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));

    ctx.switch_to(merge);
    Operand::Temp(result)
}

fn unary(ctx: &mut Ctx, e: &ExprWrap, op: OperatorKind, operand: &ExprWrap) -> Operand {
    let uop = match op {
        OperatorKind::Minus => UnOp::Neg,
        OperatorKind::Bang => UnOp::Not,
        OperatorKind::BitNot => UnOp::BitNot,
        _ => {
            ctx.diag(e.span, "该一元运算符无 TAC 映射（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let ty = ctx.expr_ty(e);
    let a = lower_expr(ctx, operand);
    let dst = ctx.temp();
    ctx.emit(Instr::Un {
        dst,
        op: uop,
        a,
        ty,
    });
    Operand::Temp(dst)
}

/// as / as? / as!：`as` 数值转换走 Conv（全有符号）；as?/as! 依赖
/// nullable 槽布局，M2
fn cast(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap, kind: CastKind) -> Operand {
    match kind {
        CastKind::As => {
            let from = ctx.expr_ty(inner);
            let to = ctx.expr_ty(e);
            let op = lower_expr(ctx, inner);
            coerce(ctx, op, from, to)
        }
        CastKind::AsQuestion => nullable_cast(ctx, e, inner),
        CastKind::AsBang => unwrap(ctx, e, inner, ctx.expr_ty(e)),
    }
}

/// 隐式提升 + 显式窄化的统一出口：数值类型不同即插 Conv。
/// 非数值的 from≠to 由 sem 门禁保证不发生（到达即契约破缺，原样透传）。
pub(crate) fn coerce(ctx: &mut Ctx, op: Operand, from: TyId, to: TyId) -> Operand {
    if from == to || ctx.is_unit(to) || ctx.is_unit(from) {
        return op;
    }
    // 常量快路径：值的类型随指令走（操作数均匀不变量的常量侧），同类
    // 数值（整→整、浮→浮）无需 Conv；跨类（整↔浮）必须留 Conv，否则
    // llvme 会把整数印成浮点字面量
    if matches!(op, Operand::Const(_)) {
        let same_class = matches!(
            (ctx.sem.tys.kind(from), ctx.sem.tys.kind(to)),
            (TyKind::Int(_), TyKind::Int(_)) | (TyKind::Float(_), TyKind::Float(_))
        );
        if same_class {
            return op;
        }
    }
    let numeric = matches!(
        (ctx.sem.tys.kind(from), ctx.sem.tys.kind(to)),
        (TyKind::Int(_), TyKind::Int(_))
            | (TyKind::Int(_), TyKind::Float(_))
            | (TyKind::Float(_), TyKind::Int(_))
            | (TyKind::Float(_), TyKind::Float(_))
    );
    if !numeric {
        return op;
    }
    let dst = ctx.temp();
    ctx.emit(Instr::Un {
        dst,
        op: UnOp::Conv(from),
        a: op,
        ty: to,
    });
    Operand::Temp(dst)
}

fn call(
    ctx: &mut Ctx,
    e: &ExprWrap,
    callee: &ExprWrap,
    type_args: &[ast::syntax::ty::TypeArg],
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    if !type_args.is_empty() {
        ctx.diag(e.span, "泛型调用的 lowering 在跳过清单解冻后");
        return Operand::Const(Const::Unit);
    }
    // callee 分派：expr_def 优先；变体构造等路径 expr_def 缺失——
    // callee 为成员访问时按名本地解析（layout::resolve_member）
    let def = ctx
        .sem
        .expr_def
        .get(e.id)
        .or_else(|| ctx.sem.expr_def.get(callee.id))
        .copied();
    if def.is_none()
        && let ExprKind::Member { base, name, .. } = &callee.kind
        && let Some(res) = crate::layout::resolve_member(ctx, ctx.expr_ty(base), *name)
    {
        match res {
            crate::layout::MemberRes::Variant { parent, index } => {
                return variant_ctor(ctx, e, parent, index, args);
            }
            crate::layout::MemberRes::Field { .. } => {
                ctx.diag(e.span, "字段名被当作构造器调用（sem 契约破缺）");
                return Operand::Const(Const::Unit);
            }
        }
    }
    let Some(mut def) = def else {
        ctx.diag(e.span, "调用目标无 expr_def（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    if let DefKind::TypeAlias = ctx.sem.defs.get(def).kind {
        def = match ctx.sem.defs.get(def).alias_resolved {
            Some(target) => target,
            None => {
                ctx.diag(e.span, "别名无预解析目标（sem 契约破缺）");
                return Operand::Const(Const::Unit);
            }
        };
    }
    match ctx.sem.defs.get(def).kind {
        DefKind::Struct => return struct_ctor(ctx, e, def, args),
        DefKind::EnumVariant { parent, index } => {
            return variant_ctor(ctx, e, parent, index, args);
        }
        DefKind::Fun => {}
        _ => {
            ctx.diag(e.span, "该调用目标无 lowering 路径（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    }
    // 克隆签名槽（sem 审查已认可 per-call 克隆的规模；避免借用冲突）
    let (param_tys, param_defaults, ret) = {
        let sig = &ctx.sem.defs.get(def).sig;
        (sig.params.clone(), sig.param_defaults.clone(), sig.ret)
    };
    let mut slots: Vec<Option<Operand>> = vec![None; param_tys.len()];
    let mut dup_or_unknown = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match param_tys.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(e.span, "命名实参无对应形参（SemUnknownParam 漏报）");
                    dup_or_unknown = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "实参槽位冲突（SemArityMismatch 漏报）");
            dup_or_unknown = true;
            continue;
        }
        let v = lower_expr(ctx, &arg.value);
        let v = coerce(ctx, v, ctx.expr_ty(&arg.value), param_tys[idx].1);
        slots[idx] = Some(v);
    }
    // 默认值补位：在调用点求值 callee 的默认表达式
    if !dup_or_unknown {
        for idx in 0..slots.len() {
            if slots[idx].is_none() {
                if !param_defaults.get(idx).copied().unwrap_or(false) {
                    ctx.diag(e.span, "缺少实参且无默认值（SemArityMismatch 漏报）");
                    continue;
                }
                let Some(decl) = ctx.find_fun_decl(def) else {
                    continue;
                };
                let Some(default) = decl.params.get(idx).and_then(|p| p.default.as_deref()) else {
                    ctx.diag(e.span, "默认实参 AST 缺失（sem 契约破缺）");
                    continue;
                };
                let v = lower_expr(ctx, default);
                let v = coerce(ctx, v, ctx.expr_ty(default), param_tys[idx].1);
                slots[idx] = Some(v);
            }
        }
    }
    for (idx, slot) in slots.into_iter().enumerate() {
        if let Some(v) = slot {
            ctx.emit(Instr::Param {
                a: v,
                ty: param_tys[idx].1,
            });
        }
    }
    let dst = if ctx.is_unit(ret) {
        None
    } else {
        Some(ctx.temp())
    };
    ctx.emit(Instr::Call {
        dst,
        callee: def,
        ty: ret,
    });
    match dst {
        Some(t) => Operand::Temp(t),
        None => Operand::Const(Const::Unit),
    }
}

fn lower_if(
    ctx: &mut Ctx,
    e: &ExprWrap,
    cond: &ast::syntax::expr::Condition,
    then_block: &ast::syntax::expr::Block,
    else_block: Option<&ast::syntax::expr::Block>,
) -> Operand {
    let ast::syntax::expr::Condition::Expr(cond_expr) = cond else {
        ctx.diag(e.span, "if val 条件的 lowering 在 M2（模式条件）");
        return Operand::Const(Const::Unit);
    };
    let value_ctx = !ctx.is_unit(ctx.expr_ty(e));
    let result_ty = ctx.expr_ty(e);

    let then_b = ctx.reserve_block();
    let else_b = ctx.reserve_block();
    let merge_b = ctx.reserve_block();

    let c = lower_expr(ctx, cond_expr);
    ctx.terminate(Term::Br {
        cond: c,
        then_: then_b,
        else_: else_b,
    });

    // 值语境：结果槽（两分支 Store、汇合 Load——用内存绕开 phi）
    let result_slot = if value_ctx { Some(ctx.temp()) } else { None };

    ctx.switch_to(then_b);
    let then_tail = ctx.lower_block(then_block);
    if let (Some(slot), Some((op, from))) = (result_slot, then_tail) {
        let v = coerce(ctx, op, from, result_ty);
        ctx.emit(Instr::Store {
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            src: v,
            ty: result_ty,
        });
    }
    ctx.terminate(Term::Jmp(merge_b));

    ctx.switch_to(else_b);
    if let Some(b) = else_block {
        let tail = ctx.lower_block(b);
        if let (Some(slot), Some((op, from))) = (result_slot, tail) {
            let v = coerce(ctx, op, from, result_ty);
            ctx.emit(Instr::Store {
                base: slot,
                idx: Operand::Const(Const::Int(0)),
                src: v,
                ty: result_ty,
            });
        }
    }
    ctx.terminate(Term::Jmp(merge_b));

    ctx.switch_to(merge_b);
    match result_slot {
        Some(slot) => {
            let dst = ctx.temp();
            ctx.emit(Instr::Load {
                dst,
                base: slot,
                idx: Operand::Const(Const::Int(0)),
                ty: result_ty,
            });
            Operand::Temp(dst)
        }
        None => Operand::Const(Const::Unit),
    }
}

// ---- 聚合值 lowering（M2a：槽位布局）----

/// null：和类型特例——只写 flag 槽 0（payload 不初始化，flag=0 时永不读）
fn nullable_null(ctx: &mut Ctx, e: &ExprWrap) -> Operand {
    let _ = e;
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    Operand::Temp(base)
}

/// 无参枚举变体值 E.A：tag 槽写入变体序号
fn variant_value(ctx: &mut Ctx, _e: &ExprWrap, index: usize) -> Operand {
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(index as i128)),
        ty: ctx.i8_ty,
    });
    Operand::Temp(base)
}

/// struct 构造器调用 P(1, y = 2)：槽位机（位置/命名实参），逐字段 Store
fn struct_ctor(
    ctx: &mut Ctx,
    e: &ExprWrap,
    def: sem::DefId,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    let fields = ctx.sem.defs.get(def).sig.fields.clone();
    let mut slots: Vec<Option<Operand>> = vec![None; fields.len()];
    let mut bad = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match fields.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(e.span, "命名实参无对应字段（SemUnknownParam 漏报）");
                    bad = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "构造实参槽位冲突（SemArityMismatch 漏报）");
            bad = true;
            continue;
        }
        let v = lower_expr(ctx, &arg.value);
        slots[idx] = Some(coerce(ctx, v, ctx.expr_ty(&arg.value), fields[idx].1));
    }
    if !bad {
        for slot in slots.iter() {
            if slot.is_none() {
                ctx.diag(e.span, "缺少构造实参（SemArityMismatch 漏报）");
            }
        }
    }
    let base = ctx.temp();
    for (idx, v) in slots.into_iter().enumerate() {
        if let Some(v) = v {
            let off = crate::layout::field_slot(ctx, def, idx);
            copy_slots(
                ctx,
                (base, off),
                (v, 0),
                fields[idx].1,
                fields[idx].1,
                e.span,
            );
        }
    }
    Operand::Temp(base)
}

/// 枚举变体构造 Opt.Some(x)：tag + payload 槽（实参仅位置形，sem 已拒命名）
fn variant_ctor(
    ctx: &mut Ctx,
    e: &ExprWrap,
    parent: sem::DefId,
    index: usize,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    let params = ctx.sem.defs.get(parent).sig.variants[index].1.clone();
    if args.len() != params.len() {
        ctx.diag(e.span, "变体实参数不匹配（SemArityMismatch 漏报）");
    }
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(index as i128)),
        ty: ctx.i8_ty,
    });
    for (i, arg) in args.iter().enumerate() {
        if i >= params.len() {
            break;
        }
        let v = lower_expr(ctx, &arg.value);
        let v = coerce(ctx, v, ctx.expr_ty(&arg.value), params[i].1);
        let off = 1 + crate::layout::variant_param_slot(ctx, parent, index, i);
        copy_slots(ctx, (base, off), (v, 0), params[i].1, params[i].1, e.span);
    }
    Operand::Temp(base)
}

/// 成员名提取（Member 的 name 字段）
fn member_name(e: &ExprWrap) -> ast::interner::Symbol {
    match &e.kind {
        ExprKind::Member { name, .. } => *name,
        _ => ast::interner::Symbol(u32::MAX),
    }
}

/// EnumVariant DefId → 其父 enum DefId
fn def_of_parent(ctx: &mut Ctx, def: sem::DefId) -> sem::DefId {
    match ctx.sem.defs.get(def).kind {
        DefKind::EnumVariant { parent, .. } => parent,
        _ => def,
    }
}

/// 成员访问：E.A（命名空间，expr_def=EnumVariant，不求值 base）；
/// p.x（expr_def=StructField → 槽 Load）。?. 的 nullable 安全调用在 M2b
fn lower_member(ctx: &mut Ctx, e: &ExprWrap, base: &ExprWrap, safe: bool) -> Operand {
    if safe {
        ctx.diag(e.span, "?. 安全调用的 lowering 在 M2b（flag 传播）");
        return Operand::Const(Const::Unit);
    }
    // 先查 expr_def（sem 写入时直接可用）；缺失则按名本地解析
    let resolved =
        ctx.sem
            .expr_def
            .get(e.id)
            .copied()
            .map(|def| match &ctx.sem.defs.get(def).kind {
                DefKind::EnumVariant { index, .. } => crate::layout::MemberRes::Variant {
                    parent: def_of_parent(ctx, def),
                    index: *index,
                },
                DefKind::StructField { parent, index } => crate::layout::MemberRes::Field {
                    parent: *parent,
                    index: *index,
                },
                _ => crate::layout::MemberRes::Field {
                    parent: def,
                    index: usize::MAX,
                },
            });
    let resolved = resolved.or_else(|| match &base.kind {
        ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Member { .. } => {
            crate::layout::resolve_member(ctx, ctx.expr_ty(base), member_name(e))
        }
        _ => None,
    });
    let Some(res) = resolved else {
        ctx.diag(e.span, "成员访问无法解析（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    match res {
        crate::layout::MemberRes::Variant { index, .. } => variant_value(ctx, e, index),
        crate::layout::MemberRes::Field { parent, index } => {
            let bop = lower_expr(ctx, base);
            let b = as_base(ctx, bop, e.span);
            let slot = crate::layout::field_slot(ctx, parent, index);
            let ty = ctx.expr_ty(e);
            let dst = ctx.temp();
            ctx.emit(Instr::Load {
                dst,
                base: b,
                idx: Operand::Const(Const::Int(slot as i128)),
                ty,
            });
            Operand::Temp(dst)
        }
    }
}

/// 索引访问 a[i]：slot 寻址 = i × 元素槽数（元素槽数 >1 时插 Mul）
fn lower_index(ctx: &mut Ctx, e: &ExprWrap, base: &ExprWrap, index: &ExprWrap) -> Operand {
    let bty = ctx.expr_ty(base);
    let elem = match ctx.sem.tys.kind(bty) {
        TyKind::Array(el, _) => *el,
        _ => {
            ctx.diag(e.span, "非数组索引的 lowering 未支持（元组下标等）");
            return Operand::Const(Const::Unit);
        }
    };
    let bop = lower_expr(ctx, base);
    let b = as_base(ctx, bop, e.span);
    let mut idx = lower_expr(ctx, index);
    let es = crate::layout::scalar_slots(ctx, elem);
    if es > 1 {
        let ity = ctx.expr_ty(index);
        let dst = ctx.temp();
        ctx.emit(Instr::Bin {
            dst,
            op: BinOp::Mul,
            l: idx,
            r: Operand::Const(Const::Int(es as i128)),
            ty: ity,
        });
        idx = Operand::Temp(dst);
    }
    let dst = ctx.temp();
    ctx.emit(Instr::Load {
        dst,
        base: b,
        idx,
        ty: elem,
    });
    Operand::Temp(dst)
}

/// 数组字面量 [1, 2, 3]：基址 + 逐元素槽 Store
fn array_literal(ctx: &mut Ctx, e: &ExprWrap, elems: &[ExprWrap]) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem = match ctx.sem.tys.kind(ty) {
        TyKind::Array(el, _) => *el,
        _ => {
            ctx.diag(e.span, "数组字面量类型非 Array（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let es = crate::layout::scalar_slots(ctx, elem);
    let base = ctx.temp();
    let mut off = 0;
    for el in elems {
        let v = lower_expr(ctx, el);
        let v = coerce(ctx, v, ctx.expr_ty(el), elem);
        copy_slots(ctx, (base, off), (v, 0), elem, elem, e.span);
        off += es;
    }
    Operand::Temp(base)
}

/// 元组字面量 (a, b)：槽序即元素序
fn tuple_literal(ctx: &mut Ctx, e: &ExprWrap, elems: &[ExprWrap]) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem_tys: Vec<TyId> = match ctx.sem.tys.kind(ty) {
        TyKind::Tuple(es) => es.clone(),
        _ => {
            ctx.diag(e.span, "元组字面量类型非 Tuple（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let base = ctx.temp();
    let mut off = 0;
    for (i, el) in elems.iter().enumerate() {
        let ety = elem_tys.get(i).copied().unwrap_or(ty);
        let v = lower_expr(ctx, el);
        let v = coerce(ctx, v, ctx.expr_ty(el), ety);
        copy_slots(ctx, (base, off), (v, 0), ety, ety, e.span);
        off += crate::layout::scalar_slots(ctx, ety);
    }
    Operand::Temp(base)
}

/// 数组填充 [v; n]：值一次求值 + 下标循环（n 为 AST 常量；循环形态
/// 避免按 count 展开指令，count 任意大都恒定体积）
fn array_repeat(ctx: &mut Ctx, e: &ExprWrap, value: &ExprWrap, count: u32) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem = match ctx.sem.tys.kind(ty) {
        TyKind::Array(el, _) => *el,
        _ => {
            ctx.diag(e.span, "数组填充类型非 Array（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let es = crate::layout::scalar_slots(ctx, elem);
    let v = lower_expr(ctx, value);
    let v = coerce(ctx, v, ctx.expr_ty(value), elem);
    let base = ctx.temp();
    let ity = ctx.sem.tys.default_int;
    // 元素序计数器（slot）
    let counter = ctx.temp();
    ctx.emit(Instr::Store {
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    let cond_b = ctx.reserve_block();
    let body_b = ctx.reserve_block();
    let done_b = ctx.reserve_block();
    ctx.terminate(Term::Jmp(cond_b));

    ctx.switch_to(cond_b);
    let c = ctx.temp();
    ctx.emit(Instr::Load {
        dst: c,
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    let cmp = ctx.temp();
    ctx.emit(Instr::Bin {
        dst: cmp,
        op: BinOp::Lt,
        l: Operand::Temp(c),
        r: Operand::Const(Const::Int(count as i128)),
        ty: ity,
    });
    ctx.terminate(Term::Br {
        cond: Operand::Temp(cmp),
        then_: body_b,
        else_: done_b,
    });

    ctx.switch_to(body_b);
    let i = ctx.temp();
    ctx.emit(Instr::Load {
        dst: i,
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    if es <= 1 {
        ctx.emit(Instr::Store {
            base,
            idx: Operand::Temp(i),
            src: v,
            ty: elem,
        });
    } else {
        let vb = as_base(ctx, v, e.span);
        for s in 0..es {
            let sty = crate::layout::slot_ty(ctx, elem, s);
            let t = ctx.temp();
            ctx.emit(Instr::Load {
                dst: t,
                base: vb,
                idx: Operand::Const(Const::Int(s as i128)),
                ty: sty,
            });
            let off = if s == 0 {
                Operand::Temp(i)
            } else {
                let o = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: o,
                    op: BinOp::Add,
                    l: Operand::Temp(i),
                    r: Operand::Const(Const::Int(s as i128)),
                    ty: ity,
                });
                Operand::Temp(o)
            };
            ctx.emit(Instr::Store {
                base,
                idx: off,
                src: Operand::Temp(t),
                ty: sty,
            });
        }
    }
    let one = ctx.temp();
    ctx.emit(Instr::Bin {
        dst: one,
        op: BinOp::Add,
        l: Operand::Temp(i),
        r: Operand::Const(Const::Int(1)),
        ty: ity,
    });
    ctx.emit(Instr::Store {
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Temp(one),
        ty: ity,
    });
    ctx.terminate(Term::Jmp(cond_b));

    ctx.switch_to(done_b);
    Operand::Temp(base)
}

/// as?：nullable → nullable 运行时 flag 分支拷贝；非 nullable 源恒非空
/// （flag=1 + payload 转换拷贝）
fn nullable_cast(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap) -> Operand {
    let to = ctx.expr_ty(e);
    let u_inner = match ctx.sem.tys.kind(to) {
        TyKind::Nullable(inner) => *inner,
        // sem 若给非 nullable 结果型（如恒非空推断），退化为数值转换
        _ => {
            let from = ctx.expr_ty(inner);
            let op = lower_expr(ctx, inner);
            return coerce(ctx, op, from, to);
        }
    };
    let from = ctx.expr_ty(inner);
    let op = lower_expr(ctx, inner);
    let base = ctx.temp();
    match ctx.sem.tys.kind(from) {
        TyKind::Nullable(_) => {
            let sb = as_base(ctx, op, e.span);
            let flag = ctx.temp();
            ctx.emit(Instr::Load {
                dst: flag,
                base: sb,
                idx: Operand::Const(Const::Int(0)),
                ty: ctx.i8_ty,
            });
            let nonnull_b = ctx.reserve_block();
            let null_b = ctx.reserve_block();
            let merge_b = ctx.reserve_block();
            ctx.terminate(Term::Br {
                cond: Operand::Temp(flag),
                then_: nonnull_b,
                else_: null_b,
            });
            ctx.switch_to(nonnull_b);
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(1)),
                ty: ctx.i8_ty,
            });
            copy_slots(ctx, (base, 1), (Operand::Temp(sb), 1), from, to, e.span);
            ctx.terminate(Term::Jmp(merge_b));
            ctx.switch_to(null_b);
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(0)),
                ty: ctx.i8_ty,
            });
            ctx.terminate(Term::Jmp(merge_b));
            ctx.switch_to(merge_b);
            Operand::Temp(base)
        }
        _ => {
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(1)),
                ty: ctx.i8_ty,
            });
            copy_slots(ctx, (base, 1), (op, 0), from, u_inner, e.span);
            Operand::Temp(base)
        }
    }
}

/// as! / !!：拆 nullable——flag 检查，null 走 unreachable（sem 已担保
/// 非空的契约破缺路径），payload 逐槽转换为目标型
fn unwrap(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap, to: TyId) -> Operand {
    let from = ctx.expr_ty(inner);
    let op = lower_expr(ctx, inner);
    let s_inner = match ctx.sem.tys.kind(from) {
        TyKind::Nullable(inner) => *inner,
        // 对非 nullable 源拆包 = 纯转换
        _ => return coerce(ctx, op, from, to),
    };
    let sb = as_base(ctx, op, e.span);
    let flag = ctx.temp();
    ctx.emit(Instr::Load {
        dst: flag,
        base: sb,
        idx: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    let ok_b = ctx.reserve_block();
    let bad_b = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: Operand::Temp(flag),
        then_: ok_b,
        else_: bad_b,
    });
    ctx.switch_to(bad_b);
    ctx.terminate(Term::Unreachable);
    ctx.switch_to(ok_b);
    if crate::layout::scalar_slots(ctx, to) <= 1 {
        let sty = crate::layout::slot_ty(ctx, from, 1);
        let t = ctx.temp();
        ctx.emit(Instr::Load {
            dst: t,
            base: sb,
            idx: Operand::Const(Const::Int(1)),
            ty: sty,
        });
        coerce(ctx, Operand::Temp(t), sty, to)
    } else {
        let base = ctx.temp();
        copy_slots(ctx, (base, 0), (Operand::Temp(sb), 1), s_inner, to, e.span);
        Operand::Temp(base)
    }
}
