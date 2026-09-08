//! 声明空间

use super::modifier::ModifierList;
use super::node::{NodeId, Span};
use super::ty::TypeWrap;

#[derive(Debug, Clone)]
pub struct DeclNode {
    pub id: NodeId,
    pub span: Span,
    pub kind: DeclKind,
}

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
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub modifiers: ModifierList,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeWrap>,
    pub body: Option<Vec<DeclStmt>>,
}

#[derive(Debug, Clone)]
pub struct BindingDecl {
    pub immutable: bool,
    pub name: String,
    pub ty: Option<TypeWrap>,
    pub init: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub mutable: bool,
    pub name: String,
    pub ty: Option<TypeWrap>,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: String,
    pub members: Vec<DeclNode>,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: TypeWrap,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct UnionDecl {
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub ty: TypeWrap,
}

/// 声明块内的简写语句（v1：仅 expr-string 占位）
#[derive(Debug, Clone)]
pub struct DeclStmt {
    pub text: String,
}
