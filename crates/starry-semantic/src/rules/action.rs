use starry_ast::{BinaryOp, LiteralValue, UnaryOp};

use crate::ty::Ty;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttrField {
    Ty,
    IsConst,
    ConstValue,
    IsLvalue,
    Place,
    Instructions,
}

#[derive(Debug, Clone)]
pub enum Action {
    SetSynthesized {
        field: AttrField,
        value: ValueExpr,
    },
    SetInherited {
        child_index: usize,
        field: AttrField,
        value: ValueExpr,
    },
    Check {
        condition: ValueExpr,
        error: ErrorTemplate,
    },
    FoldIfConst {
        target_field: AttrField,
        op_fold: FoldOp,
    },
    PushScope,
    PopScope,
    DefinePlaceholder {
        name_expr: ValueExpr,
    },
    LookupSymbol {
        name_expr: ValueExpr,
    },
    Emit {
        instr_template: InstrTemplate,
    },
    NewTemp,
    PassThrough {
        child_index: usize,
    },
    ReturnError,
    SequenceLast,
    SequenceFirst,
}

#[derive(Debug, Clone)]
pub enum FoldOp {
    Binary { op: BinaryOp },
    Unary { op: UnaryOp },
    /// 使用当前节点的二元操作符进行折叠（DSL 中的 fold: Binary(@op)）
    NodeBinary,
    /// 使用当前节点的一元操作符进行折叠（DSL 中的 fold: Unary(@op)）
    NodeUnary,
}

#[derive(Debug, Clone)]
pub enum ValueExpr {
    ChildAttr {
        index: usize,
        field: AttrField,
    },
    CurrentAttr(AttrField),
    LiteralTy(LiteralValue),
    TyBuiltin(TyBuiltin),
    BinaryVal {
        op: BinValOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },
    UnaryVal {
        op: UnValOp,
        operand: Box<ValueExpr>,
    },
    Call {
        func: BuiltinFn,
        args: Vec<ValueExpr>,
    },
    NodeOp,
    NodeSpan,
    NodeName,
    NodeValue,
    ConstBool(bool),
    ConstTy(Ty),
    SymbolField(SymbolField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TyBuiltin {
    Promote,
    IsNumeric,
    IsCompatible,
    LiteralTy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFn {
    Promote,
    IsNumeric,
    IsCompatible,
    FoldBinary,
    FoldUnary,
    LastChild,
    FirstChild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinValOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnValOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolField {
    Ty,
    IsConst,
    IsPlaceholder,
}

#[derive(Debug, Clone)]
pub enum ErrorTemplate {
    InvalidBinaryOp,
    InvalidUnaryOp,
    UndefinedName,
    UnresolvedType,
    Redeclaration,
    TypeMismatch,
    General(String),
}

#[derive(Debug, Clone)]
pub enum InstrTemplate {
    BinaryAssign,
    UnaryAssign,
    LoadConst,
    CopyName,
}
