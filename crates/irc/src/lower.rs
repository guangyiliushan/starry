//! lowering 驱动与语句层：顶层声明 → 函数体 → 语句/控制流。
//! 表达式 lowering 在 `expr.rs`（同一 Ctx 的跨文件 impl）。
//!
//! M1 边界（sem 已接受、本层暂发 `IrcUnsupportedLowering` 的构造，见
//! opt-notes）：when/for/defer、模式绑定以外的 Binding、复合赋值、
//! 聚合值（struct/enum/array/tuple/nullable）、StringInterp、is/in。

use std::collections::HashMap;

use ast::interner::{Interner, Symbol};
use ast::syntax::decl::{BindingDecl, DeclKind, FunDecl, FunctionBody};
use ast::syntax::expr::{Block, Condition};
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile, Span};
use ast::syntax::pat::PatKind;
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::token::OperatorKind;
use sem::{DefArena, DefId, DefKind, FlowRes, SideTable, TyArena, TyId, TyKind};

use crate::expr;
use crate::tac::{BlockId, FunBody, Instr, Operand, TacOutput, TempId, Term};

/// SemanticOutput 的字段级拆分（lib.rs lower 处 destructure 产物）：
/// 共享侧表/arena + interner 的可变字段借，disjoint 故可同船
pub(crate) struct SemRef<'a> {
    pub source: &'a SourceFile,
    pub tys: &'a TyArena,
    pub defs: &'a DefArena,
    pub expr_ty: &'a SideTable<TyId>,
    pub expr_def: &'a SideTable<DefId>,
    pub pat_res: &'a SideTable<sem::PatRes>,
    pub pat_binding_def: &'a SideTable<DefId>,
    pub flow_res: &'a SideTable<FlowRes>,
    pub ty_of_node: &'a SideTable<TyId>,
    pub interner: &'a mut Interner,
}

/// 值的存放处：Temp = 不可变局部（SSA 式单赋值，例外：短路布尔/in
/// 扫描的 phi 汇合物化 temp 两分支各写一次，见 expr.rs 物化族）；
/// Slot = 可变局部/未初始化局部（Load/Store 寻址，idx 恒为 0 的标量槽）
#[derive(Debug, Clone, Copy)]
pub(crate) enum Place {
    Temp(TempId),
    Slot(TempId),
}

/// 具名/匿名循环的已解析跳转目标（flow_res 回指 node 匹配）。
/// defer_depth = 进入循环时 defer 栈深：break/continue 只发射
/// 深度以上的 defer（循环内注册的），外层 defer 归各自的出口
#[derive(Debug, Clone, Copy)]
pub(crate) struct LoopCtx {
    pub node: NodeId,
    pub break_b: BlockId,
    pub cont_b: BlockId,
    pub defer_depth: usize,
    /// loop 表达式的共享结果槽（带值 break 写入；语句循环为 None）
    pub value_slot: Option<TempId>,
    pub value_ty: TyId,
}

impl<'r, 'a> crate::layout::LayoutView for Ctx<'r, 'a> {
    fn tys(&self) -> &TyArena {
        self.sem.tys
    }
    fn defs(&self) -> &DefArena {
        self.sem.defs
    }
    fn i8_ty(&self) -> TyId {
        self.i8_ty
    }
}

pub(crate) struct Ctx<'r, 'a> {
    pub sem: &'r mut SemRef<'a>,
    pub source: &'a str,
    /// tag/flag 槽的 i8 类型（lower 入口 intern，全 lowering 共享）
    pub i8_ty: TyId,
    pub diags: Vec<Diagnostic>,
    pub fun: FunBody,
    next_temp: u32,
    next_block: u32,
    cur: BlockId,
    cur_open: bool,
    /// 作用域栈：Symbol → (存放处, 类型)；param 在根作用域
    env: Vec<HashMap<Symbol, (Place, TyId)>>,
    loops: Vec<LoopCtx>,
    /// defer 栈（与块作用域同深）：每块一个待发射 defer 体队列（克隆持有，
    /// 规避 AST 引用生命期穿透 Ctx——defer 是低频构造，克隆成本可忽略）
    defers: Vec<Vec<crate::syntax_reexport::Block>>,
    /// BlockId → blocks 下标（switch_to 维护；block_pos O(1)）
    block_index: std::collections::HashMap<BlockId, usize>,
}

pub(crate) fn run<'x>(sem: &mut SemRef<'x>, source: &'x str, i8_ty: TyId) -> TacOutput {
    // 先收集 fun 声明的 span（Ctx 持有 sem 的 &mut，不能同时持有 items 引用）
    let mut spans: Vec<Span> = Vec::new();
    for item in &sem.source.items {
        if matches!(item.kind, DeclKind::Fun(_)) {
            spans.push(item.span);
        }
    }
    let mut out = TacOutput::default();
    for span in spans {
        let mut ctx = Ctx::new(sem, source, i8_ty);
        ctx.lower_fun(span);
        out.diags.append(&mut ctx.diags);
        out.funs.push(ctx.fun);
    }
    // 其余顶层声明无 codegen 产物（struct/enum/import/extern 等静默跳过；
    // sem 已拒构造被入口门禁，到不了这里）
    out
}

impl<'r, 'a> Ctx<'r, 'a> {
    // sem 的可变访问只用于字符串字面量 intern（expr::literal）
    pub(crate) fn intern(&mut self, s: &str) -> ast::interner::Symbol {
        self.sem.interner.intern(s)
    }
    fn new(sem: &'r mut SemRef<'a>, source: &'a str, i8_ty: TyId) -> Self {
        let unit = sem.tys.unit;
        Self {
            sem,
            source,
            i8_ty,
            diags: Vec::new(),
            fun: FunBody {
                def: DefId(0),
                params: Vec::new(),
                ret: unit,
                blocks: Vec::new(),
                entry: BlockId(0),
            },
            next_temp: 0,
            next_block: 0,
            cur: BlockId(0),
            cur_open: false,
            env: Vec::new(),
            loops: Vec::new(),
            defers: Vec::new(),
            block_index: std::collections::HashMap::new(),
        }
    }

    // ---- id 与块管理（单调分配，永不复用；opt-notes 命名不变量）----

    pub(crate) fn temp(&mut self) -> TempId {
        let id = TempId(self.next_temp);
        self.next_temp += 1;
        id
    }

    pub(crate) fn reserve_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        id
    }

    /// 切换到块 b：已 push（reserved 占位）则复用，否则 push。
    /// BlockId 是稀疏 id（merge 块先 reserve 后 switch），Vec 按下标
    /// 直取会越界——一切寻址经 block_pos
    pub(crate) fn switch_to(&mut self, b: BlockId) {
        if let std::collections::hash_map::Entry::Vacant(e) = self.block_index.entry(b) {
            e.insert(self.fun.blocks.len());
            self.fun.blocks.push(crate::tac::Block {
                id: b,
                instrs: Vec::new(),
                term: Term::Unreachable,
            });
        }
        self.cur = b;
        self.cur_open = true;
    }

    fn block_pos(&self, b: BlockId) -> usize {
        let pos = self.block_index.get(&b).copied();
        debug_assert!(pos.is_some(), "cur 必须是已 push 的块");
        pos.expect("cur 必须是已 push 的块")
    }

    /// 当前可发射块；已终止则惰性开新块（return 之后的死代码落这里）
    pub(crate) fn open(&mut self) -> BlockId {
        if !self.cur_open {
            let b = self.reserve_block();
            self.switch_to(b);
        }
        self.cur
    }

    pub(crate) fn emit(&mut self, instr: Instr) {
        let cur = self.open();
        let pos = self.block_pos(cur);
        self.fun.blocks[pos].instrs.push(instr);
    }

    pub(crate) fn terminate(&mut self, term: Term) {
        // 当前块已终止（如 return 之后的不可达路径）：终止请求作废，
        // 绝不在死代码路径上凭空开新块
        if !self.cur_open {
            return;
        }
        let cur = self.cur;
        let pos = self.block_pos(cur);
        self.fun.blocks[pos].term = term;
        self.cur_open = false;
    }

    // ---- 诊断与查询 ----

    pub(crate) fn diag(&mut self, span: Span, message: impl Into<String>) {
        self.diag_code(DiagCode::IrcUnsupportedLowering, span, message);
    }

    /// 任意稳定码的诊断（如字面量值域的 IntegerOutOfRange、InvalidEscape）
    pub(crate) fn diag_code(&mut self, code: DiagCode, span: Span, message: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, span, message));
    }

    pub(crate) fn expr_ty(&self, e: &ast::syntax::expr::ExprWrap) -> TyId {
        self.sem
            .expr_ty
            .get(e.id)
            .copied()
            .unwrap_or(self.sem.tys.unit)
    }

    pub(crate) fn is_unit(&self, ty: TyId) -> bool {
        matches!(self.sem.tys.kind(ty), TyKind::Unit)
    }

    fn find_fun_def(&mut self, span: Span) -> Option<DefId> {
        for i in 0..self.sem.defs.len() {
            let def = self.sem.defs.get(DefId(i as u32));
            if def.span == span && matches!(def.kind, DefKind::Fun) {
                return Some(DefId(i as u32));
            }
        }
        self.diag(span, "函数声明未找到对应 DefId（sem 契约破缺）");
        None
    }

    /// 以 DefId 找函数声明 AST（默认值表达式在 callee 侧，需要 AST 原文）。
    /// 返回 'a 生命期（借自 sem.source，与 &mut self 发射无冲突）
    pub(crate) fn find_fun_decl(&self, def: DefId) -> Option<&'a FunDecl> {
        let span = self.sem.defs.get(def).span;
        let source: &'a SourceFile = self.sem.source;
        for item in &source.items {
            if let DeclKind::Fun(decl) = &item.kind
                && item.span == span
            {
                return Some(decl);
            }
        }
        None
    }

    // ---- 函数 ----

    fn lower_fun(&mut self, span: Span) {
        let Some(def) = self.find_fun_def(span) else {
            return;
        };
        let Some(decl) = self.find_fun_decl(def) else {
            self.diag(span, "函数声明 AST 缺失（sem 契约破缺）");
            return;
        };
        self.fun.def = def;
        self.fun.ret = self.sem.defs.get(def).sig.ret;

        let entry = self.reserve_block();
        self.switch_to(entry);
        self.fun.entry = entry;
        self.env.push(HashMap::new());

        // 参数 = 入口临时变量（声明序；DefSig.params 与 decl.params 对齐）
        let params = self.sem.defs.get(def).sig.params.clone();
        debug_assert_eq!(params.len(), decl.params.len());
        for (name, ty) in &params {
            let t = self.temp();
            self.fun.params.push((t, *ty));
            self.env[0].insert(*name, (Place::Temp(t), *ty));
        }

        match &decl.body {
            Some(FunctionBody::Block(b)) => {
                let tail = self.lower_block(b);
                self.finish_fun(tail);
            }
            Some(FunctionBody::Expr(e)) => {
                let op = expr::lower_expr(self, e);
                let op = expr::coerce(self, op, self.expr_ty(e), self.fun.ret);
                let value = if self.is_unit(self.fun.ret) {
                    None
                } else {
                    Some(op)
                };
                self.terminate(Term::Ret {
                    value,
                    ty: self.fun.ret,
                });
            }
            None => {
                // 无体函数（abstract/外部签名）：无 codegen 产物
                self.fun.blocks.clear();
            }
        }
    }

    fn finish_fun(&mut self, tail: Option<(Operand, TyId)>) {
        match tail {
            Some((op, from)) => {
                let op = expr::coerce(self, op, from, self.fun.ret);
                let value = if self.is_unit(self.fun.ret) {
                    None
                } else {
                    Some(op)
                };
                self.terminate(Term::Ret {
                    value,
                    ty: self.fun.ret,
                });
            }
            None => {
                if self.is_unit(self.fun.ret) {
                    self.terminate(Term::Ret {
                        value: None,
                        ty: self.fun.ret,
                    });
                } else {
                    // sem 松弛项：非 unit 函数可无尾表达式（返回靠 return 语句），
                    // fallthrough = 不可达；LLVM 侧发 unreachable
                    self.terminate(Term::Unreachable);
                }
            }
        }
    }

    // ---- 块与语句 ----

    pub(crate) fn lower_block(&mut self, b: &Block) -> Option<(Operand, TyId)> {
        self.env.push(HashMap::new());
        self.defers.push(Vec::new());
        for s in &b.stmts {
            self.lower_stmt(s);
        }
        // 尾表达式在块作用域内求值（M1 潜在 bug 修复：绑定须对尾式可见）
        let tail = b
            .tail
            .as_ref()
            .map(|t| (expr::lower_expr(self, t), self.expr_ty(t)));
        let pending = self.defers.pop().unwrap_or_default();
        // defer 在块出口（尾式求值后、调用方终止块前）逆序发射
        self.emit_defers(&pending.iter().rev().cloned().collect::<Vec<_>>());
        self.env.pop();
        tail
    }

    fn lower_stmt(&mut self, s: &StmtWrap) {
        match &s.kind {
            StmtKind::Expr(expr_stmt) => {
                expr::lower_expr(self, &expr_stmt.expr);
            }
            StmtKind::Binding(b) => self.lower_binding(b),
            StmtKind::Assignment { target, op, value } => {
                self.lower_assignment(target, *op, value);
            }
            StmtKind::While { cond, body, .. } => self.lower_while(s.id, s.span, cond, body),
            StmtKind::Loop { body, .. } => self.lower_loop(s.id, s.span, body),
            StmtKind::Break { value, .. } => {
                self.lower_jump(s.id, expr::FlowTarget::Break, s.span, value.as_deref());
            }
            StmtKind::Continue { .. } => {
                self.lower_jump(s.id, expr::FlowTarget::Continue, s.span, None)
            }
            StmtKind::Return { value } => {
                let op = value.as_ref().map(|v| {
                    let op = expr::lower_expr(self, v);
                    expr::coerce(self, op, self.expr_ty(v), self.fun.ret)
                });
                self.emit_defers_above(0);
                self.terminate(Term::Ret {
                    value: op,
                    ty: self.fun.ret,
                });
            }
            StmtKind::For {
                pattern,
                iterable,
                body,
                ..
            } => self.lower_for(s.id, pattern, iterable, body),
            StmtKind::Defer { body } => {
                self.defers
                    .last_mut()
                    .expect("defer 栈与块同深")
                    .push((**body).clone());
            }
            StmtKind::Comptime { .. } => self.diag(s.span, "comptime 语句的 lowering 在 M2"),
            StmtKind::Grow { .. } => self.diag(s.span, "grow 的 lowering 未排期"),
            StmtKind::Error(_) | StmtKind::Missing(_) => {
                self.diag(s.span, "错误/缺失节点穿透 sem 门禁（内部不变量破缺）");
            }
        }
    }

    fn lower_binding(&mut self, b: &BindingDecl) {
        if !matches!(b.pattern.kind, PatKind::Binding { .. }) {
            let Some(init) = &b.init else {
                self.diag(b.pattern.span, "无初始化解构绑定的 lowering 未排期");
                return;
            };
            let op = expr::lower_expr(self, init);
            let from = self.expr_ty(init);
            let to = self
                .sem
                .pat_binding_def
                .get(b.pattern.id)
                .map(|d| self.sem.defs.get(*d).sig.ty)
                .filter(|ty| !matches!(self.sem.tys.kind(*ty), TyKind::Unknown))
                .unwrap_or(from);
            let op = expr::coerce(self, op, from, to);
            // sem 已限 Binding/Wildcard/Tuple/Array；条件必为 true
            expr::lower_pattern(self, &b.pattern, op, to, b.pattern.span);
            if !b.immutable {
                expr::make_pattern_bindings_mutable(self, &b.pattern);
            }
            return;
        }
        let PatKind::Binding { name, .. } = &b.pattern.kind else {
            unreachable!("上方已处理非简单模式");
        };
        let target = match self.sem.pat_binding_def.get(b.pattern.id) {
            Some(d) => Some(self.sem.defs.get(*d).sig.ty),
            None => {
                self.diag(b.pattern.span, "pat_binding_def 缺失（sem 契约破缺）");
                None
            }
        };
        let Some(init) = &b.init else {
            // 无初始化：slot；sem definite pass 已拒绝未初始化读取到达这里
            let slot = self.temp();
            let ty = target.unwrap_or(self.sem.tys.unit);
            self.env_top().insert(*name, (Place::Slot(slot), ty));
            return;
        };
        let op = expr::lower_expr(self, init);
        let from = self.expr_ty(init);
        let to = target.unwrap_or(from);
        let op = expr::coerce(self, op, from, to);
        let slots = crate::layout::scalar_slots(self, to);
        if b.immutable {
            if slots > 1 {
                // 聚合：不可变 → 别名构造基址（无重赋值，零拷贝安全）
                if let Operand::Temp(t) = op {
                    self.env_top().insert(*name, (Place::Temp(t), to));
                    return;
                }
            }
            let t = self.temp();
            if slots > 1 {
                expr::copy_slots(self, (t, 0), (op, 0), to, to, b.pattern.span);
            } else {
                self.emit(Instr::Copy {
                    dst: t,
                    src: op,
                    ty: to,
                });
            }
            self.env_top().insert(*name, (Place::Temp(t), to));
        } else {
            let slot = self.temp();
            if slots > 1 {
                expr::copy_slots(self, (slot, 0), (op, 0), to, to, b.pattern.span);
            } else {
                self.emit(Instr::Store {
                    base: slot,
                    idx: Operand::Const(crate::tac::Const::Int(0)),
                    src: op,
                    ty: to,
                });
            }
            self.env_top().insert(*name, (Place::Slot(slot), to));
        }
    }

    fn lower_assignment(
        &mut self,
        target: &ast::syntax::expr::ExprWrap,
        op: OperatorKind,
        value: &ast::syntax::expr::ExprWrap,
    ) {
        let Some(to) = self.target_ty_of(target) else {
            self.diag(target.span, "赋值目标类型不可推定（sem 契约破缺）");
            return;
        };
        if op == OperatorKind::Eq {
            let v = expr::lower_expr(self, value);
            let v = expr::coerce(self, v, self.expr_ty(value), to);
            expr::assign_to(self, target, v, target.span);
            return;
        }
        // 复合赋值：读-改-写 desugar（cur OP value → 同目标写回）
        let Some(bop) = expr::compound_binop(op) else {
            self.diag(target.span, "该赋值运算符无 lowering 路径（sem 契约破缺）");
            return;
        };
        let cur = expr::lower_expr(self, target);
        let cur = expr::coerce(self, cur, to, to);
        let v = expr::lower_expr(self, value);
        let v = expr::coerce(self, v, self.expr_ty(value), to);
        let r = self.temp();
        self.emit(Instr::Bin {
            dst: r,
            op: bop,
            l: cur,
            r: v,
            ty: to,
        });
        expr::assign_to(self, target, Operand::Temp(r), target.span);
    }

    /// 赋值目标的值类型（Ident 走环境绑定类型；成员/索引走 expr_ty）
    fn target_ty_of(&self, target: &ast::syntax::expr::ExprWrap) -> Option<TyId> {
        match &target.kind {
            ast::syntax::expr::ExprKind::Ident(name) => self.lookup(*name).map(|(_, ty)| ty),
            _ => self.sem.expr_ty.get(target.id).copied(),
        }
    }

    fn lower_while(&mut self, node: NodeId, _span: Span, cond: &Condition, body: &Block) {
        let cond_b = self.reserve_block();
        let body_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(cond_b));

        // val/is 条件的模式绑定每迭代刷新（cond_b 每轮重入），
        // 作用域覆盖 cond_b + body（scoped 时 lower_cond 推作用域）
        self.switch_to(cond_b);
        let (c, scoped) = expr::lower_cond(self, cond);
        self.terminate(Term::Br {
            cond: c,
            then_: body_b,
            else_: merge_b,
        });

        self.switch_to(body_b);
        self.loops.push(LoopCtx {
            node,
            break_b: merge_b,
            cont_b: cond_b,
            defer_depth: self.defers.len(),
            value_slot: None,
            value_ty: self.sem.tys.unit,
        });
        self.lower_block(body);
        self.loops.pop();
        if scoped {
            self.env_pop();
        }
        self.terminate(Term::Jmp(cond_b));

        self.switch_to(merge_b);
    }

    fn lower_loop(&mut self, node: NodeId, _span: Span, body: &Block) {
        let body_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(body_b));

        self.switch_to(body_b);
        self.loops.push(LoopCtx {
            node,
            break_b: merge_b,
            cont_b: body_b,
            defer_depth: self.defers.len(),
            value_slot: None,
            value_ty: self.sem.tys.unit,
        });
        self.lower_block(body);
        self.loops.pop();
        self.terminate(Term::Jmp(body_b));

        self.switch_to(merge_b);
    }

    fn lower_jump(
        &mut self,
        stmt: NodeId,
        kind: expr::FlowTarget,
        span: Span,
        value: Option<&ast::syntax::expr::ExprWrap>,
    ) {
        let Some(flow) = self.sem.flow_res.get(stmt) else {
            self.diag(span, "flow_res 缺失（sem 契约破缺）");
            return;
        };
        let target = self
            .loops
            .iter()
            .rev()
            .find(|l| l.node == flow.target)
            .map(|l| {
                (
                    match kind {
                        expr::FlowTarget::Break => l.break_b,
                        expr::FlowTarget::Continue => l.cont_b,
                    },
                    l.defer_depth,
                    l.value_slot,
                    l.value_ty,
                )
            });
        match target {
            Some((b, depth, value_slot, value_ty)) => {
                // break 值与 return 值同纪律：先求值并写入循环结果槽，再跑 defer。
                if let Some(v) = value {
                    let op = expr::lower_expr(self, v);
                    let op = expr::coerce(self, op, self.expr_ty(v), value_ty);
                    if let Some(slot) = value_slot {
                        if crate::layout::scalar_slots(self, value_ty) > 1 {
                            expr::copy_slots(self, (slot, 0), (op, 0), value_ty, value_ty, span);
                        } else {
                            self.emit(Instr::Store {
                                base: slot,
                                idx: Operand::Const(crate::tac::Const::Int(0)),
                                src: op,
                                ty: value_ty,
                            });
                        }
                    }
                }
                // 循环出口：只发射循环内注册的 defer（内→外），外层归各自出口
                self.emit_defers_above(depth);
                self.terminate(Term::Jmp(b));
            }
            None => self.diag(span, "break/continue 目标不在循环栈（sem 契约破缺）"),
        }
    }

    /// 无标签 loop 表达式：带值 break 写共享结果槽，merge 后读回。
    /// 无 break 的无限循环结果型是 unit，merge 不读槽（后续仍接不可达块）。
    pub(crate) fn lower_loop_expr(
        &mut self,
        e: &ast::syntax::expr::ExprWrap,
        body: &Block,
    ) -> Operand {
        let result_ty = self.expr_ty(e);
        let body_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(body_b));
        self.switch_to(body_b);
        let result_slot = self.temp();
        self.loops.push(LoopCtx {
            node: e.id,
            break_b: merge_b,
            cont_b: body_b,
            defer_depth: self.defers.len(),
            value_slot: Some(result_slot),
            value_ty: result_ty,
        });
        self.lower_block(body);
        self.loops.pop();
        self.terminate(Term::Jmp(body_b));
        self.switch_to(merge_b);
        if self.is_unit(result_ty) {
            Operand::Const(crate::tac::Const::Unit)
        } else if crate::layout::scalar_slots(self, result_ty) > 1 {
            Operand::Temp(result_slot)
        } else {
            let dst = self.temp();
            self.emit(Instr::Load {
                dst,
                base: result_slot,
                idx: Operand::Const(crate::tac::Const::Int(0)),
                ty: result_ty,
            });
            Operand::Temp(dst)
        }
    }

    pub(crate) fn env_top(&mut self) -> &mut HashMap<Symbol, (Place, TyId)> {
        self.env.last_mut().expect("env root scope")
    }

    /// for Pattern in iterable：Array → 下标循环；Range 字面量 → 计数循环。
    /// 循环变量绑定进 body 作用域（每迭代重写的 slot/快照拷贝）
    fn lower_for(
        &mut self,
        node: NodeId,
        pattern: &ast::syntax::pat::PatWrap,
        iterable: &ast::syntax::expr::ExprWrap,
        body: &Block,
    ) {
        let ity = self.expr_ty(iterable);
        let ity_kind = self.sem.tys.kind(ity).clone();
        let (elem_ty, len) = match &ity_kind {
            TyKind::Array(el, len) => (*el, *len as i128),
            _ => {
                if let ast::syntax::expr::ExprKind::Binary { op, lhs, rhs } = &iterable.kind
                    && matches!(
                        op,
                        ast::token::OperatorKind::DotDot | ast::token::OperatorKind::DotDotEq
                    )
                {
                    return self.lower_for_range(node, pattern, lhs, rhs, *op, body);
                }
                if matches!(ity_kind, TyKind::Range(_)) {
                    return self.lower_for_range_value(node, pattern, iterable, body);
                }
                self.diag(iterable.span, "for 只支持数组与区间迭代（sem 契约破缺）");
                return;
            }
        };
        let es = crate::layout::scalar_slots(self, elem_ty);
        // iterable 入口块一次求值（副作用只执行一次；基址 temp 穿入循环）
        let arr_op = expr::lower_expr(self, iterable);
        let arr = match arr_op {
            Operand::Temp(t) => t,
            Operand::Const(_) => {
                self.diag(iterable.span, "数组迭代源为常量（sem 契约破缺）");
                self.temp()
            }
        };
        let cty = self.sem.tys.default_int;
        let counter = self.temp();
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        let cond_b = self.reserve_block();
        let body_b = self.reserve_block();
        let step_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(cond_b);
        let c = self.temp();
        self.emit(Instr::Load {
            dst: c,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        let cmp = self.temp();
        self.emit(Instr::Bin {
            dst: cmp,
            op: crate::tac::BinOp::Lt,
            l: Operand::Temp(c),
            r: Operand::Const(crate::tac::Const::Int(len)),
            ty: cty,
        });
        self.terminate(Term::Br {
            cond: Operand::Temp(cmp),
            then_: body_b,
            else_: merge_b,
        });
        self.switch_to(body_b);
        self.loops.push(LoopCtx {
            node,
            break_b: merge_b,
            // continue → step（先自增再判条件；直跳 cond 会跳过自增死循环）
            cont_b: step_b,
            defer_depth: self.defers.len(),
            value_slot: None,
            value_ty: self.sem.tys.unit,
        });
        self.env_push();
        let elem = self.temp();
        let i = self.temp();
        self.emit(Instr::Load {
            dst: i,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        if es <= 1 {
            self.emit(Instr::Load {
                dst: elem,
                base: arr,
                idx: Operand::Temp(i),
                ty: elem_ty,
            });
        } else {
            let base_idx = self.temp();
            self.emit(Instr::Bin {
                dst: base_idx,
                op: crate::tac::BinOp::Mul,
                l: Operand::Temp(i),
                r: Operand::Const(crate::tac::Const::Int(es as i128)),
                ty: cty,
            });
            for sld in 0..es {
                let sty = crate::layout::slot_ty(self, elem_ty, sld);
                let t = self.temp();
                let off = if sld == 0 {
                    Operand::Temp(base_idx)
                } else {
                    let o = self.temp();
                    self.emit(Instr::Bin {
                        dst: o,
                        op: crate::tac::BinOp::Add,
                        l: Operand::Temp(base_idx),
                        r: Operand::Const(crate::tac::Const::Int(sld as i128)),
                        ty: cty,
                    });
                    Operand::Temp(o)
                };
                self.emit(Instr::Load {
                    dst: t,
                    base: arr,
                    idx: off,
                    ty: sty,
                });
                self.emit(Instr::Store {
                    base: elem,
                    idx: Operand::Const(crate::tac::Const::Int(sld as i128)),
                    src: Operand::Temp(t),
                    ty: sty,
                });
            }
        }
        expr::lower_pattern(self, pattern, Operand::Temp(elem), elem_ty, pattern.span);
        self.lower_block(body);
        self.env_pop();
        self.loops.pop();
        // 自增独立 step 块：body 以 break/return 结尾时自增不再
        // 落进 open() 惰性开的孤儿块
        self.terminate(Term::Jmp(step_b));
        self.switch_to(step_b);
        let i = self.temp();
        self.emit(Instr::Load {
            dst: i,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        let one = self.temp();
        self.emit(Instr::Bin {
            dst: one,
            op: crate::tac::BinOp::Add,
            l: Operand::Temp(i),
            r: Operand::Const(crate::tac::Const::Int(1)),
            ty: cty,
        });
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: Operand::Temp(one),
            ty: cty,
        });
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(merge_b);
    }

    /// for Pattern in lo..hi（含 ..=）：计数循环，绑定别名计数 slot
    fn lower_for_range(
        &mut self,
        node: NodeId,
        pattern: &ast::syntax::pat::PatWrap,
        lhs: &ast::syntax::expr::ExprWrap,
        rhs: &ast::syntax::expr::ExprWrap,
        op: ast::token::OperatorKind,
        body: &Block,
    ) {
        let lo = expr::lower_expr(self, lhs);
        let hi = expr::lower_expr(self, rhs);
        let cty = self.expr_ty(lhs);
        let counter = self.temp();
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: lo,
            ty: cty,
        });
        let cond_b = self.reserve_block();
        let body_b = self.reserve_block();
        let step_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(cond_b);
        let c = self.temp();
        self.emit(Instr::Load {
            dst: c,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        let cmp_op = if op == ast::token::OperatorKind::DotDot {
            crate::tac::BinOp::Lt
        } else {
            crate::tac::BinOp::Le
        };
        let cmp = self.temp();
        self.emit(Instr::Bin {
            dst: cmp,
            op: cmp_op,
            l: Operand::Temp(c),
            r: hi,
            ty: cty,
        });
        self.terminate(Term::Br {
            cond: Operand::Temp(cmp),
            then_: body_b,
            else_: merge_b,
        });
        self.switch_to(body_b);
        self.loops.push(LoopCtx {
            node,
            break_b: merge_b,
            cont_b: step_b,
            defer_depth: self.defers.len(),
            value_slot: None,
            value_ty: self.sem.tys.unit,
        });
        self.env_push();
        let v = self.temp();
        self.emit(Instr::Load {
            dst: v,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        expr::lower_pattern(self, pattern, Operand::Temp(v), cty, pattern.span);
        self.lower_block(body);
        self.env_pop();
        self.loops.pop();
        self.terminate(Term::Jmp(step_b));
        self.switch_to(step_b);
        let v = self.temp();
        self.emit(Instr::Load {
            dst: v,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: cty,
        });
        let one = self.temp();
        self.emit(Instr::Bin {
            dst: one,
            op: crate::tac::BinOp::Add,
            l: Operand::Temp(v),
            r: Operand::Const(crate::tac::Const::Int(1)),
            ty: cty,
        });
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: Operand::Temp(one),
            ty: cty,
        });
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(merge_b);
    }

    /// for Pattern in range_value：lo/hi 两槽（Range 类型不区分开闭，按闭区间）
    fn lower_for_range_value(
        &mut self,
        node: NodeId,
        pattern: &ast::syntax::pat::PatWrap,
        iterable: &ast::syntax::expr::ExprWrap,
        body: &Block,
    ) {
        let rty = self.expr_ty(iterable);
        let inner = match self.sem.tys.kind(rty) {
            TyKind::Range(inner) => *inner,
            _ => {
                self.diag(iterable.span, "Range 迭代类型破缺（sem 契约破缺）");
                return;
            }
        };
        let rop = expr::lower_expr(self, iterable);
        let rbase = match rop {
            Operand::Temp(t) => t,
            Operand::Const(_) => {
                self.diag(iterable.span, "Range 迭代源为常量（sem 契约破缺）");
                self.temp()
            }
        };
        let lo = self.temp();
        self.emit(Instr::Load {
            dst: lo,
            base: rbase,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: inner,
        });
        let hi = self.temp();
        self.emit(Instr::Load {
            dst: hi,
            base: rbase,
            idx: Operand::Const(crate::tac::Const::Int(1)),
            ty: inner,
        });
        let counter = self.temp();
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: Operand::Temp(lo),
            ty: inner,
        });
        let cond_b = self.reserve_block();
        let body_b = self.reserve_block();
        let step_b = self.reserve_block();
        let merge_b = self.reserve_block();
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(cond_b);
        let c = self.temp();
        self.emit(Instr::Load {
            dst: c,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: inner,
        });
        let cmp = self.temp();
        self.emit(Instr::Bin {
            dst: cmp,
            op: crate::tac::BinOp::Le,
            l: Operand::Temp(c),
            r: Operand::Temp(hi),
            ty: inner,
        });
        self.terminate(Term::Br {
            cond: Operand::Temp(cmp),
            then_: body_b,
            else_: merge_b,
        });
        self.switch_to(body_b);
        self.loops.push(LoopCtx {
            node,
            break_b: merge_b,
            cont_b: step_b,
            defer_depth: self.defers.len(),
            value_slot: None,
            value_ty: self.sem.tys.unit,
        });
        self.env_push();
        let v = self.temp();
        self.emit(Instr::Load {
            dst: v,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: inner,
        });
        expr::lower_pattern(self, pattern, Operand::Temp(v), inner, pattern.span);
        self.lower_block(body);
        self.env_pop();
        self.loops.pop();
        self.terminate(Term::Jmp(step_b));
        self.switch_to(step_b);
        let v = self.temp();
        self.emit(Instr::Load {
            dst: v,
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            ty: inner,
        });
        let one = self.temp();
        self.emit(Instr::Bin {
            dst: one,
            op: crate::tac::BinOp::Add,
            l: Operand::Temp(v),
            r: Operand::Const(crate::tac::Const::Int(1)),
            ty: inner,
        });
        self.emit(Instr::Store {
            base: counter,
            idx: Operand::Const(crate::tac::Const::Int(0)),
            src: Operand::Temp(one),
            ty: inner,
        });
        self.terminate(Term::Jmp(cond_b));
        self.switch_to(merge_b);
    }

    pub(crate) fn env_push(&mut self) {
        self.env.push(HashMap::new());
    }

    pub(crate) fn env_pop(&mut self) {
        self.env.pop();
    }

    /// 发射给定 defer 体（顺序 = 给定序；调用方负责内→外排序）。
    /// defer 体内的 return/break 穿透由 sem 门禁保证不发生
    fn emit_defers(&mut self, bodies: &[crate::syntax_reexport::Block]) {
        for body in bodies {
            let _ = self.lower_block(body);
        }
    }

    /// 发射并消费 defer 栈 depth 以上的全部 defer（自最内层向外，
    /// 与注册逆序一致），栈弹出到 depth。return 用 depth=0（函数
    /// 整体退出）；break/continue 用循环入口深度（只清循环内注册的）
    pub(crate) fn emit_defers_above(&mut self, depth: usize) {
        // 只掏空不弹栈：队列结构与 lower_block 出口的 push/pop 配对
        // 保持不变（弹空队列曾导致 lower_block 误弹外层队列——外层
        // defer 落进循环回边后的孤儿块）
        for q in (depth..self.defers.len()).rev() {
            let bodies: Vec<crate::syntax_reexport::Block> =
                self.defers[q].drain(..).rev().collect();
            for body in &bodies {
                let _ = self.lower_block(body);
            }
        }
    }

    pub(crate) fn lookup(&self, name: Symbol) -> Option<(Place, TyId)> {
        self.env
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name).copied())
    }
}
