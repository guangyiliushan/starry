pub type TempId = usize;
pub type LabelId = usize;

/// TAC 操作数
///
/// 三地址码中每条指令的操作数可以是临时变量、命名变量、
/// 常量或标签。对应龙书 §8.3 中三地址码操作数的定义。
#[derive(Debug, Clone)]
pub enum Operand {
    /// 临时变量 t0, t1, ...
    Temp(TempId),
    /// 命名变量 x, y, ...
    Name(String),
    /// 整数常量
    IntConst(i64),
    /// 浮点常量
    FloatConst(f64),
    /// 布尔常量
    BoolConst(bool),
    /// 字符串常量（索引到字符串池）
    StrConst(usize),
    /// 标签引用
    Label(LabelId),
}

impl std::fmt::Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Temp(id) => write!(f, "t{}", id),
            Operand::Name(name) => write!(f, "{}", name),
            Operand::IntConst(v) => write!(f, "{}", v),
            Operand::FloatConst(v) => write!(f, "{}", v),
            Operand::BoolConst(v) => write!(f, "{}", v),
            Operand::StrConst(idx) => write!(f, "str#{}", idx),
            Operand::Label(id) => write!(f, "L{}", id),
        }
    }
}

impl PartialEq for Operand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Operand::Temp(a), Operand::Temp(b)) => a == b,
            (Operand::Name(a), Operand::Name(b)) => a == b,
            (Operand::IntConst(a), Operand::IntConst(b)) => a == b,
            (Operand::FloatConst(a), Operand::FloatConst(b)) => a.to_bits() == b.to_bits(),
            (Operand::BoolConst(a), Operand::BoolConst(b)) => a == b,
            (Operand::StrConst(a), Operand::StrConst(b)) => a == b,
            (Operand::Label(a), Operand::Label(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Operand {}

impl std::hash::Hash for Operand {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Operand::Temp(v) => v.hash(state),
            Operand::Name(v) => v.hash(state),
            Operand::IntConst(v) => v.hash(state),
            Operand::FloatConst(v) => v.to_bits().hash(state),
            Operand::BoolConst(v) => v.hash(state),
            Operand::StrConst(v) => v.hash(state),
            Operand::Label(v) => v.hash(state),
        }
    }
}
