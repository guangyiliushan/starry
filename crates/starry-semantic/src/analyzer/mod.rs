mod ir_gen;
mod ir_gen_rules;
mod legacy;
mod name_resolution;
mod name_resolution_rules;
mod rule_driven;
mod type_check;
mod type_check_rules;
mod warmup;

pub use ir_gen::IrGenerator;
pub use ir_gen_rules::build_ir_gen_rules;
pub use legacy::Analyzer;
pub use name_resolution::NameResolutionEvaluator;
pub use name_resolution_rules::build_name_resolution_rules;
pub use rule_driven::RuleDrivenEvaluator;
pub use type_check::TypeCheckEvaluator;
pub use type_check_rules::build_type_check_rules;
pub use warmup::WarmupEvaluator;
