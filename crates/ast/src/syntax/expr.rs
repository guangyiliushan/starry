//! 表达式与 block 空间（Pratt 解析产物，糖全保留；v0.3 文法对齐中）

use super::node::{ErrorNode, MissingNode, NodeId, Path, Span, Symbol};
use super::pat::{PatKind, PatWrap};
use super::stmt::StmtWrap;
use super::ty::TypeArg;
use super::ty::TypeWrap;
use crate::token::{LiteralKind, OperatorKind, Token};

#[derive(Debug, Clone)]
pub struct ExprWrap {
    pub id: NodeId,
    pub span: Span,
    pub kind: ExprKind,
}

impl ExprWrap {
    pub const fn new(id: NodeId, span: Span, kind: ExprKind) -> Self {
        Self { id, span, kind }
    }

    pub fn missing(id: NodeId, span: Span) -> Self {
        Self::new(id, span, ExprKind::Missing(MissingNode { id, span }))
    }
}

/// 表达式语句；分号/NL 分隔符不进入 AST。
#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: ExprWrap,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<StmtWrap>,
    pub tail: Option<Box<ExprWrap>>,
}

impl LambdaParams {
    /// 显式参数列表的绑定形展开：`{ (a, b) -> … }` 整体解析成单元素元组
    /// 模式（= 参数表），`(x)` 单元素 = 单参数；无括号逐个绑定原样。
    /// 非 Binding 形（wildcard/destructure）原样返回，由调用方按需处理。
    pub fn param_kinds(&self) -> Vec<&PatKind> {
        match self {
            LambdaParams::It => vec![],
            LambdaParams::Explicit(ps) => {
                let flat: Vec<&PatKind> = if ps.len() == 1 {
                    match &ps[0].kind {
                        PatKind::Tuple {
                            elements,
                            rest: None,
                        } => elements.iter().map(|p| &p.kind).collect(),
                        other => vec![other],
                    }
                } else {
                    ps.iter().map(|p| &p.kind).collect()
                };
                flat
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum LambdaParams {
    /// `«it»` sugar，保留原形。
    It,
    Explicit(Vec<PatWrap>),
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(LiteralKind),
    Bool(bool),
    Null,
    Ident(Symbol),
    Path(Path),
    Binary {
        op: OperatorKind,
        lhs: Box<ExprWrap>,
        rhs: Box<ExprWrap>,
    },
    Is {
        lhs: Box<ExprWrap>,
        pattern: Box<PatWrap>,
        negated: bool,
    },
    /// `值 in 集合`：右操作数是集合表达式（EBNF RelationalExpr 的 RangeExpr 侧）
    In {
        lhs: Box<ExprWrap>,
        rhs: Box<ExprWrap>,
        negated: bool,
    },
    Unary {
        op: OperatorKind,
        operand: Box<ExprWrap>,
    },
    /// 取址 &x / &mut x（D-007 / M2，T1 借用）。sem 做保守借用检查。
    AddrOf {
        mutable: bool,
        expr: Box<ExprWrap>,
    },
    /// 解引 *p（pointer/ref → 内层值；赋值目标走 StorePtr）。
    Deref {
        expr: Box<ExprWrap>,
    },
    Cast {
        expr: Box<ExprWrap>,
        ty: Box<TypeWrap>,
        kind: CastKind,
    },
    Call {
        callee: Box<ExprWrap>,
        type_args: Vec<TypeArg>,
        args: Vec<CallArg>,
    },
    Index {
        base: Box<ExprWrap>,
        index: Box<ExprWrap>,
        /// Tuple subscripts must be integer literals; parser records the slot here.
        const_index: Option<usize>,
    },
    Member {
        base: Box<ExprWrap>,
        name: Symbol,
        safe: bool,
    },
    ForceUnwrap {
        expr: Box<ExprWrap>,
    },
    Lambda {
        params: LambdaParams,
        body: Box<Block>,
    },
    /// loop 表达式：值来自带值 break（EBNF LoopExpr）。label 仅由块尾
    /// LoopStatement 归一而来（'fun f() { ''outer: loop { break ''outer 1 } }'），
    /// 值位置直接写 loop 表达式时 parser 不接受标签。
    Loop {
        label: Option<Symbol>,
        body: Box<Block>,
    },
    StringInterp {
        parts: Vec<ExprWrap>,
    },
    When {
        subject: Option<Box<ExprWrap>>,
        branches: Vec<WhenBranch>,
    },
    If {
        cond: Box<Condition>,
        then_block: Box<Block>,
        else_block: Option<Box<Block>>,
    },
    Tuple(Vec<ExprWrap>),
    Array(Vec<ExprWrap>),
    ArrayRepeat {
        value: Box<ExprWrap>,
        count: u32,
    },
    Spawn(SpawnExpr),
    /// `async Block`（EBNF AsyncExpr 取块，非任意表达式）
    Async {
        block: Box<Block>,
    },
    Await {
        expr: Box<ExprWrap>,
    },
    Comptime {
        block: Box<Block>,
    },
    Unsafe {
        block: Box<Block>,
    },
    Concurrent {
        block: Box<Block>,
    },
    Proceed,
    Super {
        suffix: SuperSuffix,
    },
    MacroCall {
        name: Path,
        tokens: Vec<TokenTree>,
    },
    Error(ErrorNode),
    Missing(MissingNode),
}

/// Refutable condition: ordinary expression or `val Pattern = Expr`.
#[derive(Debug, Clone)]
pub enum Condition {
    Expr(Box<ExprWrap>),
    Val {
        pattern: Box<PatWrap>,
        expr: Box<ExprWrap>,
    },
}

#[derive(Debug, Clone)]
pub enum SpawnScheduler {
    Detached,
    Concurrent,
}

#[derive(Debug, Clone, Copy)]
pub enum SpawnKind {
    Thread,
    Fiber,
    Process,
}

#[derive(Debug, Clone)]
pub struct SpawnExpr {
    pub scheduler: Option<SpawnScheduler>,
    pub kind: Option<SpawnKind>,
    pub actor: Option<ActorConstruction>,
    pub body: Box<Block>,
}

/// `spawn actor Name(args?)`：actor 构造点。
#[derive(Debug, Clone)]
pub struct ActorConstruction {
    pub name: Path,
    pub args: Vec<CallArg>,
}

#[derive(Debug, Clone)]
pub enum SuperSuffix {
    Member(Symbol),
    Call(Vec<CallArg>),
}

/// Raw token tree；宏展开不在 parser 内进行。
#[derive(Debug, Clone)]
pub enum TokenTree {
    Token(Token),
    Group {
        open: Token,
        tokens: Vec<TokenTree>,
        close: Option<Token>,
    },
}

#[derive(Debug, Clone)]
/// 实参：命名（`name = value`）或位置。
pub struct CallArg {
    pub name: Option<Symbol>,
    pub value: ExprWrap,
}

#[derive(Debug, Clone)]
pub struct WhenBranch {
    pub pattern: WhenPattern,
    pub guard: Option<Box<ExprWrap>>,
    pub body: WhenBody,
}

#[derive(Debug, Clone)]
pub enum WhenPattern {
    Expression(ExprWrap),
    Pattern(PatWrap),
    Else,
}

#[derive(Debug, Clone)]
pub enum WhenBody {
    Expr(ExprWrap),
    Block(Block),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    As,
    AsQuestion,
    AsBang,
}
