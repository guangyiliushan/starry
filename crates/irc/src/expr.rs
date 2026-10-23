//! 表达式 lowering：每个 ExprWrap → Operand（Temp 或 Const）。
//! 类型取自 expr_ty 侧表；隐式提升与显式 as 统一走 `coerce` 插 Conv
//! （操作数类型均匀不变量，opt-notes）。

use ast::syntax::expr::{CastKind, ExprKind, ExprWrap};
use ast::syntax::node::Span;
use ast::token::{LiteralKind, OperatorKind};
use sem::{DefKind, TyId, TyKind};

use crate::lower::{Ctx, Place};
use crate::tac::{BinOp, Const, Instr, Operand, Term, UnOp};

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
        ExprKind::Null => Operand::Const(Const::Null),
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
        ExprKind::Index { .. } | ExprKind::Member { .. } => {
            ctx.diag(e.span, "索引/成员访问的 lowering 在 M2（槽寻址）");
            Operand::Const(Const::Unit)
        }
        ExprKind::Tuple(_) | ExprKind::Array(_) | ExprKind::ArrayRepeat { .. } => {
            ctx.diag(e.span, "元组/数组字面量的 lowering 在 M2（槽布局）");
            Operand::Const(Const::Unit)
        }
        ExprKind::StringInterp { .. } => {
            ctx.diag(
                e.span,
                "字符串插值的 lowering 在 M3（stringify+concat 契约）",
            );
            Operand::Const(Const::Unit)
        }
        ExprKind::ForceUnwrap { .. } => {
            ctx.diag(e.span, "!! 的 lowering 在 M2（nullable 布局）");
            Operand::Const(Const::Unit)
        }
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

/// 标识符：Temp 直接用；Slot 发 Load（标量槽 idx 0）
fn ident(ctx: &mut Ctx, name: ast::interner::Symbol, span: Span) -> Operand {
    match ctx.lookup(name) {
        Some((Place::Temp(t), _)) => Operand::Temp(t),
        Some((Place::Slot(slot), ty)) => {
            let dst = ctx.temp();
            ctx.emit(Instr::Load {
                dst,
                base: slot,
                idx: Operand::Const(Const::Int(0)),
                ty,
            });
            Operand::Temp(dst)
        }
        None => {
            ctx.diag(span, "标识符不在 lowering 环境（sem 契约破缺）");
            Operand::Const(Const::Int(0))
        }
    }
}

fn literal(ctx: &mut Ctx, kind: LiteralKind, span: Span) -> Operand {
    let text = span.slice(ctx.source);
    match kind {
        LiteralKind::Integer => {
            // sem IntegerOutOfRange 门禁保证 i128 可解析；下划线分隔符剥除
            let v: i128 = text
                .replace('_', "")
                .parse()
                .expect("sem 门禁后的整数字面量必可解析");
            Operand::Const(Const::Int(v))
        }
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
            Operand::Const(Const::Str(ctx.intern(&unescape(inner))))
        }
        LiteralKind::Char => {
            let inner = text
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .expect("lexer 保证引号配对");
            let unescaped = unescape(inner);
            // char 在 LLVM 侧为 i32（ty_name）；取第一个标量值
            let c = unescaped.chars().next().unwrap_or('\0');
            Operand::Const(Const::Int(c as i128))
        }
    }
}

/// 常见转义序列解码；未知转义保留原样（lexer 校验层负责报错）。
fn unescape(s: &str) -> String {
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
    if kind != CastKind::As {
        ctx.diag(e.span, "as?/as! 的 lowering 在 M2（nullable 槽布局）");
        return Operand::Const(Const::Unit);
    }
    let from = ctx.expr_ty(inner);
    let to = ctx.expr_ty(e);
    let op = lower_expr(ctx, inner);
    coerce(ctx, op, from, to)
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
    let def = ctx
        .sem
        .expr_def
        .get(e.id)
        .or_else(|| ctx.sem.expr_def.get(callee.id))
        .copied();
    let Some(def) = def else {
        ctx.diag(e.span, "调用目标无 expr_def（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    if !matches!(ctx.sem.defs.get(def).kind, DefKind::Fun) {
        ctx.diag(e.span, "struct/enum 构造器的 lowering 在 M2（槽布局）");
        return Operand::Const(Const::Unit);
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
