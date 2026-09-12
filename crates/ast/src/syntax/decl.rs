//! 声明空间
//!
//! Decl → Expr / Type / Pattern 的语法边在此连通：
//! 函数体 / 绑定初始化 / 参数默认值均引用 ExprWrap。

use super::expr::{Block, ExprStmt, ExprWrap};
use super::modifier::ModifierList;
use super::node::{NodeId, Span, Symbol};
use super::ty::TypeWrap;

#[derive(Debug, Clone)]
pub struct DeclNode {
    pub id: NodeId,
    pub span: Span,
    pub kind: DeclKind,
}

/// 声明种类（与 EBNF Declaration + TopLevelDecl 对齐，13 + 7 = 全量）
#[derive(Debug, Clone)]
pub enum DeclKind {
    Fun(FunDecl),
    Binding(BindingDecl),
    Class(ClassDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Union(UnionDecl),
    Trait(TraitDecl),
    TypeAlias(TypeAliasDecl),
    /// extern "C" fun / extern "C++" namespace（占位）
    Extern(ExternDecl),
    /// aspect 声明（AOP）
    Aspect(AspectDecl),
    /// provide 声明（DI）
    Provide(ProvideDecl),
    /// macro 声明（占位）
    Macro(MacroDecl),
    /// impl 块
    Impl(ImplDecl),
    /// comptime 块（仅语句位置；顶层不允许）
    Comptime(ComptimeDecl),
    /// actor 声明
    Actor(ActorDecl),
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub ret: Option<TypeWrap>,
    pub body: Option<FunctionBody>,
}

/// 函数体两种形态（EBNF: Block | "=" Expr）
#[derive(Debug, Clone)]
pub enum FunctionBody {
    Block(Block),
    Expr(ExprWrap),
}

#[derive(Debug, Clone)]
pub struct BindingDecl {
    pub immutable: bool,
    pub name: Symbol,
    pub ty: Option<TypeWrap>,
    pub init: Option<Box<ExprWrap>>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub mutable: bool,
    pub name: Symbol,
    pub ty: Option<TypeWrap>,
    pub default: Option<Box<ExprWrap>>,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub primary_ctor: Option<PrimaryConstructor>,
    pub supertypes: Vec<super::ty::TypeWrap>,
    pub members: Vec<DeclNode>,
}

#[derive(Debug, Clone)]
pub struct PrimaryConstructor {
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub ty: TypeWrap,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Symbol,
    pub args: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct UnionDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub supertypes: Vec<super::ty::TypeWrap>,
    pub members: Vec<DeclNode>,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<String>,
    pub ty: TypeWrap,
}

/// extern "C" fun 签名（占位）
#[derive(Debug, Clone)]
pub struct ExternDecl {
    pub language: String,
    pub is_namespace: bool,
    pub name: Symbol,
}

/// aspect 声明（AOP 占位）
#[derive(Debug, Clone)]
pub struct AspectDecl {
    pub name: Symbol,
    pub members: Vec<DeclNode>,
}

/// provide 声明（DI 占位）
#[derive(Debug, Clone)]
pub struct ProvideDecl {
    pub name: Symbol,
}

/// macro 声明（占位）
#[derive(Debug, Clone)]
pub struct MacroDecl {
    pub name: Symbol,
}

/// impl 块
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub type_ref: TypeWrap,
    pub members: Vec<DeclNode>,
}

/// comptime 块（仅语句位置；顶层不允许）
#[derive(Debug, Clone)]
pub struct ComptimeDecl {
    pub block: Block,
}

/// actor 声明
#[derive(Debug, Clone)]
pub struct ActorDecl {
    pub name: Symbol,
}
