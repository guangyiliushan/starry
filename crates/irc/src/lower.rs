//! lowering 驱动与语句层：顶层声明 → 函数体 → 语句/控制流。
//! 表达式 lowering 在 `expr.rs`（同一 Ctx 的跨文件 impl）。
//!
//! M1 边界（sem 已接受、本层暂发 `IrcUnsupportedLowering` 的构造，见
//! opt-notes）：when/for/defer、模式绑定以外的 Binding、复合赋值、
//! 聚合值（struct/enum/array/tuple/nullable）、StringInterp、is/in。

use std::collections::HashMap;

use ast::interner::{Interner, Symbol};
use ast::syntax::decl::{BindingDecl, ClassMember, DeclKind, FunDecl, FunctionBody};
use ast::syntax::expr::ExprWrap as ExprWrap2;
use ast::syntax::expr::{Block, Condition};
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile, Span};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::token::OperatorKind;
use sem::{DefArena, DefId, DefKind, FlowRes, SideTable, TyArena, TyId, TyKind};

use crate::expr;
use crate::tac::{BlockId, Callee, FunBody, GlobalBody, Instr, Operand, TacOutput, TempId, Term};

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
    pub decl_def: &'a [Option<DefId>],
    pub scopes: &'a sem::ScopeArena,
    pub interner: &'a mut Interner,
}

/// 值的存放处：Temp = 不可变局部（SSA 式单赋值，例外：短路布尔/in
/// 扫描的 phi 汇合物化 temp 两分支各写一次，见 expr.rs 物化族）；
/// Slot = 可变局部/未初始化局部（Load/Store 寻址，idx 恒为 0 的标量槽）
#[derive(Debug, Clone, Copy)]
pub(crate) enum Place {
    Temp(TempId),
    Slot(TempId),
    /// 顶层全局标量（跨函数持久存储；LoadGlobal/StoreGlobal）
    Global(DefId),
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

impl crate::layout::LayoutView for SemRef<'_> {
    fn tys(&self) -> &TyArena {
        self.tys
    }
    fn defs(&self) -> &DefArena {
        self.defs
    }
    fn i8_ty(&self) -> TyId {
        self.tys.unit
    }
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
    /// M4 slice 1：class 局部绑定的隐式 drop 队列（与 defers 栈同深同生命周期）。
    /// 块出口 / return / break 与 defer 同纪律逆序 release。
    class_drops: Vec<Vec<(Place, TyId, bool)>>,
    /// M4 slice 4b (Plan C): +1 class temporaries within a statement
    /// (ctor/call returns/weak upgrade/?? and if class joins).
    /// Bindings/returns/assignments take (transfer); the rest release
    /// at statement end (or after cond eval / at branch end) -
    /// C++ [class.temporary]-style full-expression discipline, no pool.
    stmt_temps: Vec<(TempId, TyId)>,
    /// BlockId → blocks 下标（switch_to 维护；block_pos O(1)）
    block_index: std::collections::HashMap<BlockId, usize>,
    /// 顶层全局：Symbol → (DefId, 类型)；只含标量（v1）
    globals: &'r HashMap<Symbol, (DefId, TyId)>,
    /// 声明了但未 lower 的顶层全局名（聚合/非简单模式）：使用点给稳定
    /// unsupported 诊断（M-later），不再落「契约破缺」断言
    pub(crate) unsupported_globals: &'r std::collections::HashSet<Symbol>,
}

pub(crate) fn run<'x>(sem: &mut SemRef<'x>, source: &'x str, i8_ty: TyId) -> TacOutput {
    let ast: &'x SourceFile = sem.source;
    let mut out = TacOutput::default();

    // 顶层全局表：v1 只 lower 标量 Binding；聚合全局仍需 TAC 存储扩展。
    let mut globals: HashMap<Symbol, (DefId, TyId)> = HashMap::new();
    let mut skipped: std::collections::HashSet<Symbol> = std::collections::HashSet::new();
    for item in &ast.items {
        let DeclKind::Binding(binding) = &item.kind else {
            continue;
        };
        if !matches!(binding.pattern.kind, PatKind::Binding { .. }) {
            // 非简单模式的全局同样未 lower：登记其名，使用点给稳定诊断
            collect_pattern_names(&binding.pattern, &mut skipped);
        }
        let PatKind::Binding { name, .. } = &binding.pattern.kind else {
            out.diags.push(Diagnostic::error(
                DiagCode::IrcUnsupportedLowering,
                binding.pattern.span,
                "destructured top-level globals are not lowered in this milestone (M-later)",
            ));
            continue;
        };
        let Some(def) = sem.pat_binding_def.get(binding.pattern.id).copied() else {
            out.diags.push(Diagnostic::error(
                DiagCode::IrcUnsupportedLowering,
                binding.pattern.span,
                "顶层全局 pat_binding_def 缺失（sem 契约破缺）",
            ));
            continue;
        };
        let ty = sem.defs.get(def).sig.ty;
        if crate::layout::is_aggregate(sem, ty) {
            out.diags.push(Diagnostic::error(
                DiagCode::IrcUnsupportedLowering,
                binding.pattern.span,
                "aggregate top-level globals are not lowered in this milestone (M-later)",
            ));
            skipped.insert(*name);
            continue;
        }
        // M4 review fix: class/?P globals have no release site (the
        // Global arm of emit_class_release is a silent no-op) — reject
        // loudly instead of leaking the object and skipping deinit.
        let global_class = crate::layout::is_class(sem, ty)
            || matches!(sem.tys.kind(ty), TyKind::Nullable(inner) if crate::layout::is_class(sem, *inner));
        if global_class {
            out.diags.push(Diagnostic::error(
                DiagCode::IrcUnsupportedLowering,
                binding.pattern.span,
                "class top-level globals are not lowered in this slice (no release site)",
            ));
            skipped.insert(*name);
            continue;
        }
        globals.insert(*name, (def, ty));
    }
    out.globals.reserve(globals.len());

    // 全局初始化函数：main 前由 llvme 调用；解释器按序执行。
    for item in &ast.items {
        let DeclKind::Binding(binding) = &item.kind else {
            continue;
        };
        let PatKind::Binding { name, .. } = &binding.pattern.kind else {
            continue;
        };
        let Some(&(def, ty)) = globals.get(name) else {
            continue;
        };
        let init = if let Some(init) = &binding.init {
            let mut ctx = Ctx::new(sem, source, i8_ty, &globals, &skipped);
            let entry = ctx.reserve_block();
            ctx.switch_to(entry);
            ctx.fun.entry = entry;
            ctx.fun.def = def;
            ctx.fun.ret = ty;
            ctx.env.push(HashMap::new());
            let value = expr::lower_expr_as(&mut ctx, init, ty);
            if ctx.is_unit(ty) {
                ctx.terminate(Term::Ret { value: None, ty });
            } else {
                ctx.terminate(Term::Ret {
                    value: Some(value),
                    ty,
                });
            }
            out.diags.append(&mut ctx.diags);
            let init_fun = std::mem::replace(
                &mut ctx.fun,
                FunBody {
                    def,
                    params: Vec::new(),
                    ret: ty,
                    blocks: Vec::new(),
                    entry: BlockId(0),
                },
            );
            Some(init_fun)
        } else {
            None
        };
        out.globals.push(GlobalBody { def, ty, init });
    }

    // 函数体 lowering（顶层全局表穿入 Ctx，供 Ident 读写）。
    // M7：泛型原件无 expr_ty（sem 不检查），只 lower 实例化克隆；
    // def 经 decl_def 侧链取（克隆 def 与原件同 span，span 反查会撞车）
    for (i, item) in ast.items.iter().enumerate() {
        let DeclKind::Fun(_f) = &item.kind else {
            continue;
        };
        let Some(def) = sem.decl_def.get(i).copied().flatten() else {
            continue;
        };
        // M7 修复：跳过泛型原件按 DEF 判定——单态化克隆的 AST 仍带
        // 原 type_params，曾致克隆被跳过、调用悬空（预存 bug）
        if sem.defs.get(def).ty_params > 0 {
            continue;
        }
        let mut ctx = Ctx::new(sem, source, i8_ty, &globals, &skipped);
        ctx.lower_fun(def);
        out.diags.append(&mut ctx.diags);
        out.funs.push(ctx.fun);
    }
    // M4 slice 2b：class 成员函数与 deinit 的 lowering。self = 首参对象
    // 指针（静态分派）；deinit 由 release 经 header meta 回调。
    for (i, item) in ast.items.iter().enumerate() {
        let DeclKind::Class(c) = &item.kind else {
            continue;
        };
        let Some(class_def) = sem.decl_def.get(i).copied().flatten() else {
            continue;
        };
        let Some(ms) = sem.defs.get(class_def).member_scope else {
            continue;
        };
        for member in &c.members {
            match member {
                ClassMember::Declaration(d) if matches!(&d.kind, DeclKind::Fun(_)) => {
                    if let DeclKind::Fun(f) = &d.kind {
                        let Some(mdef) = sem.scopes.lookup_local(ms, f.name) else {
                            continue;
                        };
                        let mut ctx = Ctx::new(sem, source, i8_ty, &globals, &skipped);
                        ctx.lower_member_fun(f, mdef);
                        out.diags.append(&mut ctx.diags);
                        out.funs.push(ctx.fun);
                    }
                }
                ClassMember::Deinit(b) => {
                    let dname = sem.interner.intern("deinit");
                    let Some(mdef) = sem.scopes.lookup_local(ms, dname) else {
                        continue;
                    };
                    let mut ctx = Ctx::new(sem, source, i8_ty, &globals, &skipped);
                    ctx.lower_deinit(b, mdef);
                    out.diags.append(&mut ctx.diags);
                    out.funs.push(ctx.fun);
                }
                _ => {}
            }
        }
    }
    reorder_globals(&mut out, sem.defs);
    out
}

/// 模式内全部绑定名（非简单顶层全局的 skipped 登记用，与 declare.walk 同形）
fn collect_pattern_names(pat: &PatWrap, out: &mut std::collections::HashSet<Symbol>) {
    match &pat.kind {
        PatKind::Binding { name, .. } => {
            out.insert(*name);
        }
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                collect_pattern_names(p, out);
            }
        }
        _ => {}
    }
}

/// 顶层全局初始化按依赖序执行（`val b = a + 3` 先于 `val a` 声明时，
/// 仍先初始化 a）。依赖 = init 函数直接读的全局 + 其调用链读的全局；
/// 环报 IrcUnsupportedLowering（无法定义顺序，不静默按源序），
/// 诊断定位到环的回边目标成员的声明（Def.span，一条即拒绝整个环）。
fn reorder_globals(out: &mut TacOutput, defs: &DefArena) {
    use std::collections::{BTreeSet, HashMap};
    if out.globals.len() < 2 {
        return;
    }
    let index: HashMap<DefId, usize> = out
        .globals
        .iter()
        .enumerate()
        .map(|(i, g)| (g.def, i))
        .collect();
    let mut reads: HashMap<DefId, BTreeSet<DefId>> = HashMap::new();
    let mut calls: HashMap<DefId, BTreeSet<DefId>> = HashMap::new();
    let mut scan = |fun: &FunBody| {
        let mut r = BTreeSet::new();
        let mut c = BTreeSet::new();
        for b in &fun.blocks {
            for i in &b.instrs {
                match i {
                    Instr::LoadGlobal { global, .. } => {
                        r.insert(*global);
                    }
                    Instr::Call {
                        callee: Callee::Def(d),
                        ..
                    } => {
                        c.insert(*d);
                    }
                    _ => {}
                }
            }
        }
        reads.insert(fun.def, r);
        calls.insert(fun.def, c);
    };
    for fun in &out.funs {
        scan(fun);
    }
    for g in &out.globals {
        if let Some(init) = &g.init {
            scan(init);
        }
    }
    // 调用链闭包：被调函数读的全局也是调用者读的全局
    loop {
        let mut changed = false;
        let keys: Vec<DefId> = calls.keys().copied().collect();
        for def in keys {
            let callees = calls.get(&def).cloned().unwrap_or_default();
            let mut add = BTreeSet::new();
            for c in &callees {
                if let Some(rs) = reads.get(c) {
                    add.extend(rs.iter().copied());
                }
            }
            let entry = reads.entry(def).or_default();
            let before = entry.len();
            entry.extend(add);
            if entry.len() != before {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let n = out.globals.len();
    let deps: Vec<Vec<usize>> = out
        .globals
        .iter()
        .map(|g| {
            reads
                .get(&g.def)
                .map(|rs| {
                    rs.iter()
                        .filter_map(|d| index.get(d).copied())
                        .filter(|&i| i != index[&g.def])
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    // 显式栈迭代 DFS（声明数不受 parser 深度限制，深链递归会爆栈）
    let mut state = vec![0u8; n];
    let mut order = Vec::with_capacity(n);
    let mut cycles: Vec<usize> = Vec::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(top) = stack.last_mut() {
            let node = top.0;
            if top.1 < deps[node].len() {
                let d = deps[node][top.1];
                top.1 += 1;
                match state[d] {
                    0 => {
                        state[d] = 1;
                        stack.push((d, 0));
                    }
                    1 if !cycles.contains(&d) => {
                        cycles.push(d);
                    }
                    _ => {}
                }
            } else {
                state[node] = 2;
                order.push(node);
                stack.pop();
            }
        }
    }
    for &i in &cycles {
        let span = defs.get(out.globals[i].def).span;
        out.diags.push(Diagnostic::error(
            DiagCode::IrcUnsupportedLowering,
            span,
            "cyclic global initializers cannot be ordered",
        ));
    }
    let globals = std::mem::take(&mut out.globals);
    let mut slots: Vec<Option<GlobalBody>> = globals.into_iter().map(Some).collect();
    out.globals = order.into_iter().filter_map(|i| slots[i].take()).collect();
}

impl<'r, 'a> Ctx<'r, 'a> {
    // sem 的可变访问只用于字符串字面量 intern（expr::literal）
    pub(crate) fn intern(&mut self, s: &str) -> ast::interner::Symbol {
        self.sem.interner.intern(s)
    }
    fn new(
        sem: &'r mut SemRef<'a>,
        source: &'a str,
        i8_ty: TyId,
        globals: &'r HashMap<Symbol, (DefId, TyId)>,
        unsupported_globals: &'r std::collections::HashSet<Symbol>,
    ) -> Self {
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
            class_drops: Vec::new(),
            stmt_temps: Vec::new(),
            block_index: std::collections::HashMap::new(),
            globals,
            unsupported_globals,
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

    /// 以 DefId 找函数声明 AST（默认值表达式在 callee 侧，需要 AST 原文）。
    /// 返回 'a 生命期（借自 sem.source，与 &mut self 发射无冲突）。
    /// M7：优先 decl_def 侧链（克隆 def 与原件同 span，span 反查会撞车）
    pub(crate) fn find_fun_decl(&self, def: DefId) -> Option<&'a FunDecl> {
        let source: &'a SourceFile = self.sem.source;
        if let Some(idx) = self.sem.decl_def.iter().position(|d| *d == Some(def))
            && let DeclKind::Fun(decl) = &source.items[idx].kind
        {
            return Some(decl);
        }
        let span = self.sem.defs.get(def).span;
        for item in &source.items {
            if let DeclKind::Fun(decl) = &item.kind
                && item.span == span
            {
                return Some(decl);
            }
        }
        None
    }

    /// 以 DefId 找 struct 声明 AST（主构造默认值在声明侧，需要 AST 原文）
    pub(crate) fn find_struct_decl(&self, def: DefId) -> Option<&'a ast::syntax::decl::StructDecl> {
        let source: &'a SourceFile = self.sem.source;
        if let Some(idx) = self.sem.decl_def.iter().position(|d| *d == Some(def))
            && let DeclKind::Struct(decl) = &source.items[idx].kind
        {
            return Some(decl);
        }
        let span = self.sem.defs.get(def).span;
        for item in &source.items {
            if let DeclKind::Struct(decl) = &item.kind
                && item.span == span
            {
                return Some(decl);
            }
        }
        None
    }

    /// 主构造第 idx 个参数的默认表达式
    pub(crate) fn find_struct_default(
        &self,
        def: DefId,
        idx: usize,
    ) -> Option<&'a ast::syntax::expr::ExprWrap> {
        self.find_struct_decl(def)?
            .primary_ctor
            .get(idx)?
            .default
            .as_deref()
    }

    // ---- 函数 ----

    /// M4 slice 2b：class 成员函数 lowering——self 作首参（对象指针，
    /// sig.ty 通道取 class 值类型），声明形参随后；体在 self 作用域解析
    /// 的 env 下 lowering（self 名由 sem resolve 期绑定）。
    fn lower_member_fun(&mut self, f: &FunDecl, mdef: DefId) {
        let class_ty = self.sem.defs.get(mdef).sig.ty;
        let ret = self.sem.defs.get(mdef).sig.ret;
        self.fun.def = mdef;
        self.fun.ret = ret;
        let entry = self.reserve_block();
        self.switch_to(entry);
        self.fun.entry = entry;
        self.env.push(HashMap::new());
        let self_t = self.temp();
        self.fun.params.push((self_t, class_ty));
        let self_name = self.sem.interner.intern("self");
        self.env[0].insert(self_name, (Place::Temp(self_t), class_ty));
        let params = self.sem.defs.get(mdef).sig.params.clone();
        for (name, ty) in &params {
            let t = self.temp();
            self.fun.params.push((t, *ty));
            self.env[0].insert(*name, (Place::Temp(t), *ty));
        }
        match &f.body {
            Some(FunctionBody::Block(b)) => {
                let tail = self.lower_block(b);
                self.finish_fun(tail, b.tail.as_deref());
            }
            Some(FunctionBody::Expr(e)) => {
                let op = expr::lower_expr_as(self, e, ret);
                // M4 slice 4b (Plan C): expression-body returns are +1
                if !self.is_unit(ret) {
                    self.assume_class_value(&op, e, ret);
                }
                let value = if self.is_unit(ret) { None } else { Some(op) };
                self.terminate(Term::Ret { value, ty: ret });
            }
            None => {
                self.fun.blocks.clear();
            }
        }
    }

    /// M4 slice 2b：deinit lowering——只有 self 一个形参，返回 unit
    fn lower_deinit(&mut self, body: &Block, mdef: DefId) {
        let class_ty = self.sem.defs.get(mdef).sig.ty;
        self.fun.def = mdef;
        self.fun.ret = self.sem.tys.unit;
        let entry = self.reserve_block();
        self.switch_to(entry);
        self.fun.entry = entry;
        self.env.push(HashMap::new());
        let self_t = self.temp();
        self.fun.params.push((self_t, class_ty));
        let self_name = self.sem.interner.intern("self");
        self.env[0].insert(self_name, (Place::Temp(self_t), class_ty));
        let tail = self.lower_block(body);
        self.finish_fun(tail, None);
    }

    fn lower_fun(&mut self, def: DefId) {
        let Some(decl) = self.find_fun_decl(def) else {
            self.diag(
                self.sem.defs.get(def).span,
                "函数声明 AST 缺失（sem 契约破缺）",
            );
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
                self.finish_fun(tail, b.tail.as_deref());
            }
            Some(FunctionBody::Expr(e)) => {
                let op = expr::lower_expr_as(self, e, self.fun.ret);
                // M4 slice 4b (Plan C): expression-body returns are +1
                if !self.is_unit(self.fun.ret) {
                    self.assume_class_value(&op, e, self.fun.ret);
                }
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

    fn finish_fun(&mut self, tail: Option<(Operand, TyId)>, tail_expr: Option<&ExprWrap2>) {
        match tail {
            Some((op, from)) => {
                let op = expr::coerce(self, op, from, self.fun.ret);
                // M4 slice 4b (Plan C): tail-expression returns are +1
                if let Some(te) = tail_expr {
                    self.assume_class_value(&op, te, self.fun.ret);
                }
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
        self.class_drops.push(Vec::new());
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
        let drops = self.class_drops.pop().unwrap_or_default();
        for (place, ty, weak) in drops.iter().rev() {
            self.emit_class_release(*place, *ty, *weak);
        }
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
                self.drain_stmt_temps();
                self.lower_jump(s.id, expr::FlowTarget::Break, s.span, value.as_deref());
                return;
            }
            StmtKind::Continue { .. } => {
                self.drain_stmt_temps();
                self.lower_jump(s.id, expr::FlowTarget::Continue, s.span, None);
                return;
            }
            StmtKind::Return { value } => {
                let op = value
                    .as_ref()
                    .map(|v| expr::lower_expr_as(self, v, self.fun.ret));
                // M4 slice 4b (Plan C)：class 返回统一 +1——借用值
                // （标识符/字段等 +0）先 retain；+1-origin 转移
                if let (Some(v), Some(op_ref)) = (value.as_ref(), op.as_ref()) {
                    self.assume_class_value(op_ref, v, self.fun.ret);
                }
                self.drain_stmt_temps();
                self.emit_defers_above(0);
                self.terminate(Term::Ret {
                    value: op,
                    ty: self.fun.ret,
                });
                return;
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
        // M4 slice 4b (Plan C)：语句末释放未转移的 +1 class 临时值
        //（Return/Break/Continue 臂已自行 drain 并终止当前块）
        self.drain_stmt_temps();
    }

    fn lower_binding(&mut self, b: &BindingDecl) {
        if !matches!(b.pattern.kind, PatKind::Binding { .. }) {
            let Some(init) = &b.init else {
                self.diag(b.pattern.span, "无初始化解构绑定的 lowering 未排期");
                return;
            };
            let to = self
                .sem
                .pat_binding_def
                .get(b.pattern.id)
                .map(|d| self.sem.defs.get(*d).sig.ty)
                .filter(|ty| !matches!(self.sem.tys.kind(*ty), TyKind::Unknown))
                .unwrap_or_else(|| self.expr_ty(init));
            let op = expr::lower_expr_as(self, init, to);
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
        let to = target.unwrap_or_else(|| self.expr_ty(init));
        let op = expr::lower_expr_as(self, init, to);
        // M4 slice 1：class 值的复制 = retain + 块出口隐式 release；
        // 构造者（init 为 class 构造调用）持 strong=1，免 retain。
        // M4 slice 4a: nullable class bindings share class ownership discipline
        // (materialized flag+payload, then retain payload, release at scope exit)
        let nullable_class = matches!(
            self.sem.tys.kind(to),
            TyKind::Nullable(inner) if crate::layout::is_class(self, *inner)
        );
        let class_binding = crate::layout::is_class(self, to) || nullable_class;
        // M4 slice 3：weak var——初始化时 weak_retain（weak++），
        // 块出口 weak_drop；读取/重赋值 sem 已拒收（无悬垂面）
        let weak_binding = self
            .sem
            .pat_binding_def
            .get(b.pattern.id)
            .is_some_and(|def| {
                matches!(
                    self.sem.defs.get(*def).kind,
                    DefKind::LocalBinding { weak: true, .. }
                )
            });
        // M4 slice 4c: +1 return convention——init 为任意调用（构造/函数/方法）
        // 时结果已持 strong 引用，绑定免 retain，仅登记块出口 release。
        // 形参 = +0 借用：调用点不 retain，被调侧绑定显式 retain。
        // M4 slice 4b (Plan C): +1-origin inits (call/??/if/null) and
        // registered +1 temps (weak upgrade) transfer to the binding
        // (take => skip the retain arms); borrowed inits retain below.
        let taken = if class_binding {
            self.take_stmt_temp(&op)
        } else {
            false
        };
        let is_ctor = class_binding && (taken || crate::expr::plus1_origin(self, init));
        // null 字面量初始化：payload 恒空，retain 是噪音（runtime 虽守卫）
        let is_null_init = matches!(&init.kind, ast::syntax::expr::ExprKind::Null);
        if b.immutable {
            if crate::layout::is_aggregate(self, to) {
                // 聚合：不可变 → 别名构造基址（无重赋值，零拷贝安全）
                if let Operand::Temp(t) = op {
                    if nullable_class {
                        if !is_ctor && !is_null_init {
                            self.emit_class_retain(Place::Temp(t), to);
                        }
                        self.class_drops
                            .last_mut()
                            .expect("class drop stack in sync with defers")
                            .push((Place::Temp(t), to, false));
                    }
                    self.env_top().insert(*name, (Place::Temp(t), to));
                    return;
                }
            }
            let t = self.temp();
            if crate::layout::is_aggregate(self, to) {
                expr::copy_slots(self, (t, 0), (op, 0), to, to, b.pattern.span);
            } else {
                self.emit(Instr::Copy {
                    dst: t,
                    src: op,
                    ty: to,
                });
            }
            self.env_top().insert(*name, (Place::Temp(t), to));
            if class_binding {
                if !is_ctor {
                    self.emit(Instr::Param {
                        a: Operand::Temp(t),
                        ty: to,
                    });
                    self.emit(Instr::Call {
                        dst: Some(t),
                        callee: Callee::Runtime(crate::tac::RuntimeFn::Retain),
                        ty: to,
                    });
                }
                self.class_drops
                    .last_mut()
                    .expect("class drop stack in sync with defers")
                    .push((Place::Temp(t), to, false));
            }
        } else {
            let slot = self.temp();
            if crate::layout::is_aggregate(self, to) {
                expr::copy_slots(self, (slot, 0), (op.clone(), 0), to, to, b.pattern.span);
            } else {
                self.emit(Instr::Store {
                    base: slot,
                    idx: Operand::Const(crate::tac::Const::Int(0)),
                    src: op.clone(),
                    ty: to,
                });
            }
            self.env_top().insert(*name, (Place::Slot(slot), to));
            if weak_binding {
                let wr = self.temp();
                self.emit(Instr::Param { a: op, ty: to });
                self.emit(Instr::Call {
                    dst: Some(wr),
                    callee: Callee::Runtime(crate::tac::RuntimeFn::WeakRetain),
                    ty: to,
                });
                self.emit(Instr::Store {
                    base: slot,
                    idx: Operand::Const(crate::tac::Const::Int(0)),
                    src: Operand::Temp(wr),
                    ty: to,
                });
                self.class_drops
                    .last_mut()
                    .expect("class drop stack in sync with defers")
                    .push((Place::Slot(slot), to, weak_binding));
            } else if class_binding {
                if !is_ctor && !is_null_init {
                    if nullable_class {
                        // M4 slice 4a: retain the payload (slot 1), not the flag
                        self.emit_class_retain(Place::Slot(slot), to);
                    } else {
                        self.emit(Instr::Param { a: op, ty: to });
                        self.emit(Instr::Call {
                            dst: Some(slot),
                            callee: Callee::Runtime(crate::tac::RuntimeFn::Retain),
                            ty: to,
                        });
                    }
                }
                self.class_drops
                    .last_mut()
                    .expect("class drop stack in sync with defers")
                    .push((Place::Slot(slot), to, weak_binding));
            }
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
            let v = expr::lower_expr_as(self, value, to);
            // M4 slice 4d: class/?P var 重赋值所有权——release 旧 payload，
            // 写回后按 rhs 来源补 retain（调用结果 +1 / null 字面量免 retain）。
            // 不变式：var 恒持有恰好一个引用（与绑定时登记的 drop 配对）。
            let nullable_class = matches!(
                self.sem.tys.kind(to),
                TyKind::Nullable(inner) if crate::layout::is_class(self, *inner)
            );
            let classish = crate::layout::is_class(self, to) || nullable_class;
            let self_assign = match (&target.kind, &value.kind) {
                (
                    ast::syntax::expr::ExprKind::Ident(tn),
                    ast::syntax::expr::ExprKind::Ident(vn),
                ) => tn == vn,
                _ => false,
            };
            if classish && !self_assign {
                let slot_only = matches!(&target.kind, ast::syntax::expr::ExprKind::Ident(name)
                    if matches!(self.lookup(*name), Some((Place::Slot(_), _))));
                if slot_only {
                    let place = match &target.kind {
                        ast::syntax::expr::ExprKind::Ident(name) => match self.lookup(*name) {
                            Some((pl, _)) => pl,
                            None => {
                                self.diag(target.span, "赋值目标不在环境（sem 契约破缺）");
                                return;
                            }
                        },
                        _ => unreachable!("slot_only 已限定 Ident"),
                    };
                    // 旧值 release（null 载荷为 runtime no-op）
                    self.emit_class_release(place, to, false);
                    let rhs_plus1 = crate::expr::plus1_origin(self, value);
                    if rhs_plus1 {
                        // M4 slice 4b (Plan C): +1 temp transfers into the var
                        let _ = self.take_stmt_temp(&v);
                    }
                    expr::assign_to(self, target, v, target.span);
                    if !rhs_plus1 {
                        self.emit_class_retain(place, to);
                    }
                    return;
                }
            }
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
        // M4 slice 4b (Plan C)：条件临时值每迭代释放
        self.drain_stmt_temps();
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
                        if crate::layout::is_aggregate(self, value_ty) {
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
        } else if crate::layout::is_aggregate(self, result_ty) {
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

    /// for Pattern in 物化 Range 值：lo/hi/开闭标记三槽（槽 2n = i8 标记，`..`=0 按 `<`、`..=`=1 按 `<=`）
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
        let n = crate::layout::scalar_slots(self, inner).max(1);
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
            idx: Operand::Const(crate::tac::Const::Int(n as i128)),
            ty: inner,
        });
        let flag = self.temp();
        self.emit(Instr::Load {
            dst: flag,
            base: rbase,
            idx: Operand::Const(crate::tac::Const::Int((2 * n) as i128)),
            ty: self.i8_ty,
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
        let cmp = expr::range_upper(
            self,
            Operand::Temp(c),
            Operand::Temp(hi),
            Operand::Temp(flag),
            inner,
        );
        self.terminate(Term::Br {
            cond: cmp,
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
        for q in (depth..self.class_drops.len()).rev() {
            let drops: Vec<(Place, TyId, bool)> = self.class_drops[q].drain(..).rev().collect();
            for (place, ty, weak) in drops {
                self.emit_class_release(place, ty, weak);
            }
        }
    }

    /// M4 slice 1：class 绑定的隐式 release。Temp 直持对象指针；
    /// Slot 先 Load 槽 0。与 defer 同出口纪律。
    /// M4 slice 4a: nullable class 的 payload（对象指针）在槽 1
    /// （槽 0 是 i8 flag）；标量 class 的指针在槽 0。
    fn class_ptr_operand(&mut self, place: Place, ty: TyId) -> Option<(Operand, TyId)> {
        let inner = match self.sem.tys.kind(ty) {
            TyKind::Nullable(inner) => Some(*inner),
            _ => None,
        };
        let (base, idx, lty) = match place {
            Place::Temp(t) => (t, if inner.is_some() { 1 } else { 0 }, inner.unwrap_or(ty)),
            Place::Slot(s) => (s, if inner.is_some() { 1 } else { 0 }, inner.unwrap_or(ty)),
            Place::Global(_) => return None,
        };
        if idx == 0 && inner.is_none() {
            return Some((Operand::Temp(base), lty));
        }
        let v = self.temp();
        self.emit(Instr::Load {
            dst: v,
            base,
            idx: Operand::Const(crate::tac::Const::Int(idx)),
            ty: lty,
        });
        Some((Operand::Temp(v), lty))
    }

    // ---- M4 slice 4b (Plan C): +1 temp discipline ----

    /// Register a +1 class temporary (ctor/call/upgrade/join).
    pub(crate) fn note_stmt_temp(&mut self, t: TempId, ty: TyId) {
        self.stmt_temps.push((t, ty));
    }

    /// Consumer takeover: transfer (unregister) when the operand is a
    /// registered +1 temporary.
    pub(crate) fn take_stmt_temp(&mut self, op: &Operand) -> bool {
        if let Operand::Temp(t) = op
            && let Some(pos) = self.stmt_temps.iter().position(|(x, _)| x == t)
        {
            self.stmt_temps.swap_remove(pos);
            return true;
        }
        false
    }

    /// Statement end / after cond eval / branch end: release untransferred
    /// +1 temporaries.
    pub(crate) fn drain_stmt_temps(&mut self) {
        let pending: Vec<(TempId, TyId)> = self.stmt_temps.drain(..).collect();
        for (t, ty) in pending {
            self.emit_class_release(Place::Temp(t), ty, false);
        }
    }

    /// Unified takeover at binding/return/assignment: transfer +1-origin
    /// (take), retain borrowed values. No-op for non-class types.
    pub(crate) fn assume_class_value(
        &mut self,
        op: &Operand,
        e: &ast::syntax::expr::ExprWrap,
        ty: TyId,
    ) {
        if !crate::expr::classish_ty(self, ty) {
            return;
        }
        if crate::expr::plus1_origin(self, e) {
            let _ = self.take_stmt_temp(op);
        } else if let Operand::Temp(t) = op {
            self.emit_class_retain(Place::Temp(*t), ty);
        }
    }

    /// M4 slice 4a: retain class 值；nullable class 经 class_ptr_operand 取 payload
    pub(crate) fn emit_class_retain(&mut self, place: Place, ty: TyId) {
        let Some((op, pty)) = self.class_ptr_operand(place, ty) else {
            return;
        };
        self.emit(Instr::Param { a: op, ty: pty });
        self.emit(Instr::Call {
            dst: None,
            callee: Callee::Runtime(crate::tac::RuntimeFn::Retain),
            ty: self.sem.tys.unit,
        });
    }

    fn emit_class_release(&mut self, place: Place, ty: TyId, weak: bool) {
        let Some((op, pty)) = self.class_ptr_operand(place, ty) else {
            return;
        };
        // Param ty must be the payload (class) ty: the payload temp is a
        // scalar ptr; passing outer ?P (agg) releases the cell address
        self.emit(Instr::Param { a: op, ty: pty });
        let rt = if weak {
            crate::tac::RuntimeFn::WeakDrop
        } else {
            crate::tac::RuntimeFn::Release
        };
        self.emit(Instr::Call {
            dst: None,
            callee: Callee::Runtime(rt),
            ty: self.sem.tys.unit,
        });
    }

    pub(crate) fn lookup(&self, name: Symbol) -> Option<(Place, TyId)> {
        self.env
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name).copied())
            .or_else(|| {
                self.globals
                    .get(&name)
                    .map(|(def, ty)| (Place::Global(*def), *ty))
            })
    }
}
