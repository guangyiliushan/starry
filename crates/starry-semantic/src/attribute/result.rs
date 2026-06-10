use crate::ir::operand::Operand;
use crate::ir::tac::TacInstr;
use crate::ty::Ty;
use starry_ast::LiteralValue;

/// 综合属性结果（S属性）
///
/// 对应龙书 §5.1 S属性文法：综合属性仅由子节点的属性决定。
/// 在 AST 遍历的**后序阶段**（离开节点时）计算得出，并通过函数返回值
/// 自底向上传递给父节点。
///
/// `AttrResult` 充当了 AST 节点的"属性槽"（Attribute Slot）：
/// 每个节点在遍历完成后，其类型、常量性、左值性等语义信息被封装在此结构体中，
/// 父节点通过 `child_results` 参数读取子节点的综合属性。
#[derive(Debug, Clone)]
pub struct AttrResult {
    /// 表达式的静态类型
    pub ty: Ty,

    /// 是否为编译时常量
    pub is_const: bool,

    /// 常量折叠后的值（仅当 `is_const` 为 `true` 时有效）
    pub const_value: Option<LiteralValue>,

    /// 是否为左值（可被赋值的目标）
    pub is_lvalue: bool,

    /// IR 生成阶段：该节点产出的 TAC 指令序列
    ///
    /// 在类型检查遍中为空；在 IR 生成遍中，每个表达式节点
    /// 会将计算所需的 TAC 指令附加在此字段中。
    pub instructions: Vec<TacInstr>,

    /// IR 生成阶段：该节点的结果存放位置
    ///
    /// 可以是临时变量（Operand::Temp）或命名变量（Operand::Name）。
    /// 父节点通过此字段获取子节点的计算结果，用于构建自身的 TAC 指令。
    pub place: Option<Operand>,
}

impl AttrResult {
    /// 创建字面量节点的标准综合属性
    pub fn literal(ty: Ty, value: LiteralValue) -> Self {
        Self {
            ty,
            is_const: true,
            const_value: Some(value),
            is_lvalue: false,
            instructions: Vec::new(),
            place: None,
        }
    }

    /// 创建标识符引用（左值）的综合属性
    pub fn lvalue(ty: Ty) -> Self {
        Self {
            ty,
            is_const: false,
            const_value: None,
            is_lvalue: true,
            instructions: Vec::new(),
            place: None,
        }
    }

    /// 创建纯计算结果的综合属性（非左值、非常量）
    pub fn computed(ty: Ty) -> Self {
        Self {
            ty,
            is_const: false,
            const_value: None,
            is_lvalue: false,
            instructions: Vec::new(),
            place: None,
        }
    }

    /// 创建错误降级综合属性
    pub fn error() -> Self {
        Self {
            ty: Ty::Error,
            is_const: false,
            const_value: None,
            is_lvalue: false,
            instructions: Vec::new(),
            place: None,
        }
    }

    /// 创建带 IR 信息的综合属性
    ///
    /// 用于 IR 生成遍，同时携带类型信息和 TAC 指令序列。
    pub fn with_ir(ty: Ty, place: Operand, instructions: Vec<TacInstr>) -> Self {
        Self {
            ty,
            is_const: false,
            const_value: None,
            is_lvalue: false,
            instructions,
            place: Some(place),
        }
    }

    /// 判断当前结果是否为错误降级
    pub fn is_error(&self) -> bool {
        matches!(self.ty, Ty::Error)
    }
}
