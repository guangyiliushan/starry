//! Starry 中间代码生成（irc）：SemanticOutput → TacOutput（三地址码）。
//!
//! ```text
//! SemanticOutput ──→ lower（装饰 AST 单遍 lowering，消费 expr_ty/expr_def/
//!                          flow_res/pat_binding_def 侧表）
//!                ──→ TacOutput（FunBody = 入口临时变量 + 基本块 + 三地址指令）
//! ```
//!
//! 设计决策记录见 `docs/syntax/opt-notes.md`（表示形态、数据布局、lowering
//! 规则速查的单一事实来源）。sem 侧有 Error 诊断时不 lowering（下游契约：
//! 有 Error 则 opt 不启动）；irc 自身诊断 = sem 已接受但 v1 里程碑边界
//! 尚未 lower 的构造（`IrcUnsupportedLowering`）。

mod expr;
mod lower;
pub mod tac;

use ast::syntax::node::DiagSeverity;
use sem::SemanticOutput;
use tac::TacOutput;

/// lowering 入口。`source` 用于字面量取值（AST 只存 LiteralKind，
/// 值在源码文本里）；`sem` 取 `&mut`：字符串字面量解码后 intern 进
/// `sem.interner`（Symbol 与 arena 同生共死纪律不变），opt/llvme 此后
/// 以 `&` 消费。
pub fn lower(sem: &mut SemanticOutput, source: &str) -> TacOutput {
    if sem
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, DiagSeverity::Error))
    {
        return TacOutput::default();
    }
    // 字段级拆分：共享侧表/arena + interner 的可变字段借（disjoint；
    // 字符串字面量解码后 intern 进 sem.interner——Symbol 与 arena 同生共死）
    let SemanticOutput {
        source: ast,
        interner,
        tys,
        defs,
        expr_ty,
        expr_def,
        flow_res,
        pat_binding_def,
        ..
    } = sem;
    let mut sr = lower::SemRef {
        source: ast,
        tys,
        defs,
        expr_ty,
        expr_def,
        flow_res,
        pat_binding_def,
        interner,
    };
    lower::run(&mut sr, source)
}
