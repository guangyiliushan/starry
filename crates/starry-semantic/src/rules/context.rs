use starry_ast::AstNode;

use crate::attribute::context::AttrContext;
use crate::attribute::env::Env;
use crate::attribute::result::AttrResult;
use crate::error::SemanticError;
use crate::ir::label::LabelManager;
use crate::ir::temp::TempManager;

pub struct ActionContext<'a> {
    pub node: &'a AstNode,
    pub child_results: &'a [AttrResult],
    pub ctx: &'a AttrContext,
    pub env: &'a mut Env,
    pub errors: &'a mut Vec<SemanticError>,
    pub temp_mgr: Option<&'a mut TempManager>,
    pub label_mgr: Option<&'a mut LabelManager>,
    pub result: AttrResult,
}
