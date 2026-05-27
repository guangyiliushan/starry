pub mod first;
pub mod follow;
pub mod left_recursion;
pub mod left_recursion_elimination;
pub mod parsing_table;

pub use first::{FirstSet, FirstSetCalculator, NullableSet};
pub use follow::{FollowSet, FollowSetCalculator};
pub use left_recursion::{LeftRecursionDetector, LeftRecursionInfo, LeftRecursionType};
pub use left_recursion_elimination::LeftRecursionEliminator;
pub use parsing_table::{ParsingConflict, ParsingTable, ParsingTableBuilder, TableEntry};

/// 提供一个统一的左递归分析器，同时包含检测和消除功能
pub struct LeftRecursionAnalyzer {
    detector: LeftRecursionDetector,
    eliminator: LeftRecursionEliminator,
}

impl LeftRecursionAnalyzer {
    pub fn new() -> Self {
        LeftRecursionAnalyzer {
            detector: LeftRecursionDetector::new(),
            eliminator: LeftRecursionEliminator::new(),
        }
    }

    /// 检测文法中是否存在左递归
    pub fn detect(&self, cfg: &crate::cfg::ContextFreeGrammar) -> Vec<LeftRecursionInfo> {
        self.detector.detect(cfg)
    }

    /// 检测是否存在直接左递归
    pub fn detect_direct(&self, cfg: &crate::cfg::ContextFreeGrammar) -> Vec<LeftRecursionInfo> {
        self.detector.detect_direct(cfg)
    }

    /// 消除左递归（包括直接和间接左递归）
    pub fn eliminate(
        &self,
        cfg: &crate::cfg::ContextFreeGrammar,
    ) -> Result<crate::cfg::ContextFreeGrammar, String> {
        self.eliminator.eliminate(cfg)
    }
}

impl Default for LeftRecursionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
