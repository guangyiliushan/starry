//! 声明空间：Decl → Expr / Type / Pattern 的语法边。

use super::expr::{Block, CallArg, ExprWrap};
use super::modifier::ModifierList;
use super::node::{ErrorNode, MissingNode, NodeId, Path, Span, Symbol};
use super::ty::{TypeParam, TypeWrap};

#[derive(Debug, Clone)]
pub struct DeclNode {
    pub id: NodeId,
    pub span: Span,
    pub kind: DeclKind,
}

#[derive(Debug, Clone)]
pub enum DeclKind {
    Import(ImportDecl),
    /// M5 slice 2a: package segment marker（合并源中每文件一个；
    /// 首段亦镜像 SourceFile.package，sem 按序跟踪当前包）
    PackageSeg(crate::syntax::node::Path),
    Extern(ExternDecl),
    Fun(FunDecl),
    Binding(BindingDecl),
    Class(ClassDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Union(UnionDecl),
    Trait(TraitDecl),
    TypeAlias(TypeAliasDecl),
    Aspect(AspectDecl),
    Provide(ProvideDecl),
    Macro(MacroDecl),
    Impl(ImplDecl),
    Actor(ActorDecl),
    Comptime(ComptimeDecl),
    Error(ErrorNode),
    Missing(MissingNode),
}

#[derive(Debug, Clone)]
pub enum ImportSuffix {
    None,
    Star,
    Alias(Symbol),
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    Qualified {
        path: Path,
        suffix: ImportSuffix,
    },
    Native {
        language: NativeLanguage,
        path: Symbol,
        alias: Option<Symbol>,
        members: Vec<FunSignature>,
    },
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub kind: ImportKind,
}

#[derive(Debug, Clone)]
pub enum NativeLanguage {
    C,
    Cpp,
}

#[derive(Debug, Clone)]
pub struct FunSignature {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret: Option<TypeWrap>,
}

#[derive(Debug, Clone)]
pub enum ExternBody {
    Fun(FunSignature),
    Block { members: Vec<FunSignature> },
}

#[derive(Debug, Clone)]
pub struct ExternDecl {
    pub language: Symbol,
    pub body: ExternBody,
}

#[derive(Debug, Clone)]
pub struct WhereConstraint {
    pub ty: TypeWrap,
    pub bounds: Vec<TypeWrap>,
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret: Option<TypeWrap>,
    pub where_clause: Vec<WhereConstraint>,
    pub body: Option<FunctionBody>,
}

#[derive(Debug, Clone)]
pub enum FunctionBody {
    Block(Block),
    Expr(ExprWrap),
}

#[derive(Debug, Clone)]
pub struct BindingDecl {
    pub modifiers: ModifierList,
    pub immutable: bool,
    pub pattern: crate::syntax::pat::PatWrap,
    pub ty: Option<TypeWrap>,
    pub init: Option<Box<ExprWrap>>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub modifier: ParamModifier,
    pub name: Symbol,
    pub ty: Option<TypeWrap>,
    pub default: Option<Box<ExprWrap>>,
}

/// Grammar-visible parameter modifier; variants map 1:1 to EBNF choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamModifier {
    None,
    Mutable,
    Val,
    Var,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub primary_ctor: Option<PrimaryConstructor>,
    pub supertypes: Vec<SuperItem>,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone)]
pub struct PrimaryConstructor {
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct SuperItem {
    pub ty: TypeWrap,
    pub args: Option<Vec<CallArg>>,
    pub by: Option<Box<ExprWrap>>,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Declaration(Box<DeclNode>),
    Constructor(Box<ConstructorDecl>),
    Accessor(Box<AccessorDecl>),
    Init(Block),
    /// M4 slice 2b：`deinit { }`——末引用释放时经 header meta 派发（R4）
    Deinit(Block),
}

#[derive(Debug, Clone)]
pub struct ConstructorDecl {
    pub modifiers: ModifierList,
    pub params: Vec<Param>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct AccessorDecl {
    pub getter: Option<Block>,
    pub setter: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    /// M3 主构造函数头参数：`val`/`var` 参数声明字段，无修饰符参数
    /// 仅构造期使用（不存字段）。空 = 无头构造（仅 body 字段槽位构造）。
    pub primary_ctor: Vec<Param>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub modifiers: ModifierList,
    /// M3 显式 val/var（Some(false)/Some(true)）；None = 无修饰（legacy）
    pub mutable: Option<bool>,
    pub name: Symbol,
    pub ty: TypeWrap,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
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
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub supertypes: Vec<SuperItem>,
    pub members: Vec<TraitMember>,
}

#[derive(Debug, Clone)]
pub enum TraitMember {
    AssociatedType(AssociatedTypeDecl),
    Signature(FunSignature),
    Fun(Box<FunDecl>),
    Declaration(Box<DeclNode>),
}

#[derive(Debug, Clone)]
pub struct AssociatedTypeDecl {
    pub name: Symbol,
    pub bound: Option<TypeWrap>,
    pub default: Option<TypeWrap>,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub modifiers: ModifierList,
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeWrap,
}

#[derive(Debug, Clone)]
pub enum AspectMember {
    Declaration(DeclNode),
    Advice(AdviceDecl),
}

#[derive(Debug, Clone)]
pub enum Pointcut {
    Name(Path),
    Member {
        ty: TypeWrap,
        member: PointcutMember,
    },
}

#[derive(Debug, Clone)]
pub enum PointcutMember {
    Wildcard,
    Name(Symbol),
}

#[derive(Debug, Clone)]
pub enum AdviceKind {
    Before,
    After,
    Around,
}

#[derive(Debug, Clone)]
pub struct AdviceDecl {
    pub kind: AdviceKind,
    pub target: Pointcut,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct AspectDecl {
    pub name: Symbol,
    pub type_params: Vec<TypeParam>,
    pub members: Vec<AspectMember>,
}

#[derive(Debug, Clone)]
pub enum ProviderLifetime {
    Singleton,
    Scoped,
    Transient,
}

#[derive(Debug, Clone)]
pub struct ProviderDecl {
    pub lifetime: ProviderLifetime,
    pub ty: TypeWrap,
    pub constructor: ExprWrap,
}

#[derive(Debug, Clone)]
pub struct ProviderBinding {
    pub interface: TypeWrap,
    pub implementation: TypeWrap,
}

#[derive(Debug, Clone)]
pub enum ProviderItem {
    Decl(ProviderDecl),
    Binding(ProviderBinding),
}

#[derive(Debug, Clone)]
pub struct ProvideDecl {
    pub name: Symbol,
    pub items: Vec<ProviderItem>,
}

#[derive(Debug, Clone)]
pub struct ActorDecl {
    pub name: Symbol,
    pub primary_ctor: Option<PrimaryConstructor>,
    pub body: ClassBody,
}

#[derive(Debug, Clone)]
pub struct ClassBody {
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone)]
pub struct ComptimeDecl {
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct MacroDecl {
    pub name: Symbol,
    pub params: Vec<Symbol>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub type_params: Vec<TypeParam>,
    pub type_ref: TypeWrap,
    pub trait_ref: TypeWrap,
    pub members: Vec<ClassMember>,
}
