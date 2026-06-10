/// 类型变量 ID
///
/// 用于类型推断阶段的约束求解（Union-Find）。
/// 每个类型变量拥有唯一 ID，在约束生成阶段分配，
/// 在求解阶段通过 Union-Find 合并等价类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub usize);

/// 语义分析阶段的类型系统枚举
///
/// 对应鲸书中类型系统的核心概念：每个表达式节点在语义分析后
/// 必须绑定一个静态类型（综合属性），用于后续的类型检查与推导。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// 整型（i64）
    Int,
    /// 浮点型（f64）
    Float,
    /// 布尔型
    Bool,
    /// 字符串型
    String,
    /// 空类型（用于无返回值语句）
    Void,
    /// 错误类型（类型检查失败后的降级类型，避免级联报错）
    Error,
    /// 未解析的类型引用（名称解析阶段使用）
    ///
    /// 当遇到类型名但尚未解析到具体类型时使用此变体。
    /// 例如 `struct A { b: B }` 中 B 尚未定义时，B 的类型为 `Unresolved("B")`。
    /// 在类型检查阶段，所有 Unresolved 类型必须被解析或报错。
    Unresolved(String),
    /// 类型变量（类型推断阶段使用）
    ///
    /// 用于支持 `auto x = expr;` 风格的类型推断。
    /// 类型变量在约束生成阶段创建，在求解阶段通过 Union-Find 算法
    /// 合并等价类，最终回填为具体类型。
    Var(TypeVarId),
}

impl Ty {
    /// 判断是否为数值类型（Int 或 Float）
    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    /// 判断是否为标量类型（可参与比较运算）
    pub fn is_scalar(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float | Ty::Bool | Ty::String)
    }

    /// 判断类型是否已完全解析
    ///
    /// 已解析的类型可以参与类型检查运算；
    /// 未解析的类型（Unresolved/Var）需要先解析或求解。
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            Ty::Int | Ty::Float | Ty::Bool | Ty::String | Ty::Void
        )
    }

    /// 构造未解析类型的便捷方法
    pub fn unresolved(name: impl Into<String>) -> Self {
        Ty::Unresolved(name.into())
    }

    /// 类型提升：二元运算时，将左右子类型统一为可运算的公共类型
    pub fn promote(lhs: &Ty, rhs: &Ty) -> Ty {
        match (lhs, rhs) {
            (Ty::Error, _) | (_, Ty::Error) => Ty::Error,
            (Ty::Unresolved(_), _) | (_, Ty::Unresolved(_)) => Ty::Error,
            (Ty::Var(_), _) | (_, Ty::Var(_)) => Ty::Error,
            (Ty::Float, Ty::Int) | (Ty::Int, Ty::Float) | (Ty::Float, Ty::Float) => Ty::Float,
            (Ty::Int, Ty::Int) => Ty::Int,
            (Ty::Bool, Ty::Bool) => Ty::Bool,
            (Ty::String, Ty::String) => Ty::String,
            _ => Ty::Error,
        }
    }

    /// 判断两个类型是否兼容（赋值或比较时）
    pub fn is_compatible(&self, other: &Ty) -> bool {
        self == other
            || matches!(
                (self, other),
                (Ty::Error, _) | (_, Ty::Error)
            )
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::Float => write!(f, "float"),
            Ty::Bool => write!(f, "bool"),
            Ty::String => write!(f, "string"),
            Ty::Void => write!(f, "void"),
            Ty::Error => write!(f, "<error>"),
            Ty::Unresolved(name) => write!(f, "<unresolved:{}>", name),
            Ty::Var(id) => write!(f, "?{}", id.0),
        }
    }
}
