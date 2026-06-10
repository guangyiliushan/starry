use starry_ast::AstNode;

use super::rule::SemanticRule;

#[derive(Debug, Clone)]
pub struct RuleTable {
    rules: Vec<SemanticRule>,
}

impl RuleTable {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: SemanticRule) {
        self.rules.push(rule);
    }

    pub fn find(&self, node: &AstNode) -> Option<&SemanticRule> {
        self.rules.iter().find(|r| r.matches(node))
    }

    pub fn rules(&self) -> &[SemanticRule] {
        &self.rules
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for RuleTable {
    fn default() -> Self {
        Self::new()
    }
}
