//! Starry LLVM IR 文本发射（llvme）：TacOutput → .ll 文本。
//!
//! ```text
//! TacOutput ──→ emit（两遍：temp 存储分类 → 指令发射）
//!           ──→ String（.ll；llvm-as/lli 可选验收层在测试侧）
//! ```
//!
//! 设计决策记录见 `docs/syntax/opt-notes.md`（槽布局、存储模型、
//! main 包装、可选验收层）。存储模型：每个 temp = entry 块 alloca
//! （标量/单槽 = i64 槽，聚合 = [N x i64] 块）；聚合实参按指针传入
//! （sem v1 形参不可变，别名安全）；非 i64 槽访问就地 bitcast；
//! 浮点一律十六进制字面量（位精确往返）；聚合值的 load/store/ret
//! 经指针，标量经值。升 SSA 与全部优化交给外部 `opt -mem2reg`。

mod emit;
mod ty;

use ast::interner::Interner;
use irc::tac::TacOutput;
use sem::{DefArena, TyArena};

/// LLVM IR 文本发射入口（opt-notes 钉死的签名）
pub fn emit(tac: &TacOutput, tys: &TyArena, defs: &DefArena, interner: &Interner) -> String {
    // API 级契约门：有诊断的 TAC 不发射（下游契约同 opt）
    if !tac.diags.is_empty() {
        return String::new();
    }
    emit::emit_module(tac, tys, defs, interner)
}
