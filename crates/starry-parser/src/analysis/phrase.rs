use crate::cfg::{ContextFreeGrammar, Production, Symbol, TerminalId};
use crate::parser::ParseTreeNode;
use crate::token_mapper;
use crate::ast_builder::GrammarAstBuilder;
use starry_ast::{AstBuilder, AstNode, Ident, Span};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Phrase {
    pub non_terminal: String,
    pub production_index: usize,
    pub leaf_start: usize,
    pub leaf_end: usize,
    pub leaf_count: usize,
    pub children_count: usize,
}

impl Phrase {
    pub fn is_epsilon(&self) -> bool {
        self.leaf_count == 0 && self.children_count == 1
    }
}

impl fmt::Display for Phrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}..{}] ({} leaves, prod #{})",
            self.non_terminal,
            self.leaf_start,
            self.leaf_end,
            self.leaf_count,
            self.production_index
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectPhrase {
    pub phrase: Phrase,
    pub leaf_texts: Vec<String>,
}

impl fmt::Display for DirectPhrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} → {} (leaves [{}..{}])",
            self.phrase.non_terminal,
            self.leaf_texts.join(" "),
            self.phrase.leaf_start,
            self.phrase.leaf_end
        )
    }
}

pub struct PhraseAnalyzer;

impl PhraseAnalyzer {
    pub fn new() -> Self {
        PhraseAnalyzer
    }

    pub fn find_phrases(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> Vec<Phrase> {
        let mut phrases = Vec::new();
        let mut leaf_index = 0usize;
        self.collect_phrases(tree, cfg, &mut leaf_index, &mut phrases);
        phrases
    }

    fn collect_phrases(
        &self,
        node: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
        leaf_index: &mut usize,
        phrases: &mut Vec<Phrase>,
    ) -> (usize, usize) {
        match node {
            ParseTreeNode::NonTerminal { name, children } => {
                let start = *leaf_index;
                for child in children {
                    self.collect_phrases(child, cfg, leaf_index, phrases);
                }
                let end = *leaf_index;

                if let Some((prod_idx, _)) = self.find_matching_production(name, children, cfg) {
                    phrases.push(Phrase {
                        non_terminal: name.clone(),
                        production_index: prod_idx,
                        leaf_start: start,
                        leaf_end: end,
                        leaf_count: end.saturating_sub(start),
                        children_count: children.len(),
                    });
                }
                (start, end)
            }
            ParseTreeNode::Terminal { .. } => {
                let pos = *leaf_index;
                *leaf_index += 1;
                (pos, *leaf_index)
            }
            ParseTreeNode::Epsilon => (*leaf_index, *leaf_index),
        }
    }

    fn find_matching_production<'a>(
        &self,
        name: &str,
        children: &[ParseTreeNode],
        cfg: &'a ContextFreeGrammar,
    ) -> Option<(usize, &'a Production)> {
        let nt_id = *cfg.non_terminal_map.get(name)?;

        cfg.productions
            .iter()
            .enumerate()
            .find(|(_, prod)| {
                prod.lhs == nt_id && self.rhs_matches_children(&prod.rhs, children, cfg)
            })
    }

    fn rhs_matches_children(
        &self,
        rhs: &[Symbol],
        children: &[ParseTreeNode],
        cfg: &ContextFreeGrammar,
    ) -> bool {
        if rhs.len() != children.len() {
            return false;
        }

        rhs.iter()
            .zip(children.iter())
            .all(|(sym, child)| self.symbol_matches_child(sym, child, cfg))
    }

    fn symbol_matches_child(
        &self,
        symbol: &Symbol,
        child: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> bool {
        match (symbol, child) {
            (Symbol::Terminal(term_id), ParseTreeNode::Terminal { token }) => {
                let end_marker = TerminalId(cfg.terminals.len());
                token_mapper::matches_terminal(
                    *term_id,
                    &token.kind,
                    cfg.get_terminal_name(*term_id),
                    end_marker,
                )
            }
            (Symbol::NonTerminal(nt_id), ParseTreeNode::NonTerminal { name, .. }) => {
                cfg.get_non_terminal_name(*nt_id) == name
            }
            (Symbol::Epsilon, ParseTreeNode::Epsilon) => true,
            _ => false,
        }
    }

    pub fn find_direct_phrases(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> Vec<DirectPhrase> {
        let mut result = Vec::new();
        self.collect_direct(tree, cfg, &mut result, &mut 0);
        result
    }

    fn collect_direct(
        &self,
        node: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
        result: &mut Vec<DirectPhrase>,
        leaf_index: &mut usize,
    ) {
        match node {
            ParseTreeNode::NonTerminal { name, children } => {
                let all_leaf_children = children.iter().all(|c| {
                    matches!(c, ParseTreeNode::Terminal { .. } | ParseTreeNode::Epsilon)
                });

                if all_leaf_children {
                    let start = *leaf_index;
                    let mut leaf_texts = Vec::new();
                    for child in children {
                        match child {
                            ParseTreeNode::Terminal { token } => {
                                leaf_texts.push(token.lexeme.clone());
                                *leaf_index += 1;
                            }
                            ParseTreeNode::Epsilon => {}
                            _ => unreachable!(),
                        }
                    }
                    let end = *leaf_index;

                    if let Some((prod_idx, _)) = self.find_matching_production(name, children, cfg) {
                        result.push(DirectPhrase {
                            phrase: Phrase {
                                non_terminal: name.clone(),
                                production_index: prod_idx,
                                leaf_start: start,
                                leaf_end: end,
                                leaf_count: end.saturating_sub(start),
                                children_count: children.len(),
                            },
                            leaf_texts,
                        });
                    }
                } else {
                    for child in children {
                        self.collect_direct(child, cfg, result, leaf_index);
                    }
                }
            }
            ParseTreeNode::Terminal { .. } => {
                *leaf_index += 1;
            }
            ParseTreeNode::Epsilon => {}
        }
    }

    pub fn find_handle(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> Option<DirectPhrase> {
        let direct = self.find_direct_phrases(tree, cfg);
        direct.into_iter().min_by_key(|p| p.phrase.leaf_start)
    }

    pub fn parse_tree_to_ast(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> AstNode {
        HandleReducer::new().reduce_all(tree, cfg)
    }

    pub fn print_phrases(&self, tree: &ParseTreeNode, cfg: &ContextFreeGrammar) {
        let phrases = self.find_phrases(tree, cfg);
        println!("Phrases ({} total):", phrases.len());
        for (i, phrase) in phrases.iter().enumerate() {
            println!("  {}. {}", i + 1, phrase);
        }
    }

    pub fn print_direct_phrases(&self, tree: &ParseTreeNode, cfg: &ContextFreeGrammar) {
        let direct = self.find_direct_phrases(tree, cfg);
        println!("Direct Phrases ({} total):", direct.len());
        for (i, dp) in direct.iter().enumerate() {
            println!("  {}. {}", i + 1, dp);
        }
    }

    pub fn print_handle(&self, tree: &ParseTreeNode, cfg: &ContextFreeGrammar) {
        if let Some(handle) = self.find_handle(tree, cfg) {
            println!("Handle: {}", handle);
        } else {
            println!("No handle found (accept state)");
        }
    }
}

impl Default for PhraseAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HandleReducer;

impl HandleReducer {
    pub fn new() -> Self {
        HandleReducer
    }

    pub fn reduce_all(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> AstNode {
        let builder = GrammarAstBuilder::new(cfg);
        let mut events = Vec::new();
        let mut leaf_index = 0;
        self.reduce_node(tree, cfg, &builder, &mut events, &mut leaf_index)
            .0
    }

    fn reduce_node(
        &self,
        node: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
        builder: &GrammarAstBuilder,
        events: &mut Vec<(DirectPhrase, AstNode)>,
        leaf_index: &mut usize,
    ) -> (AstNode, Vec<String>) {
        match node {
            ParseTreeNode::Terminal { token } => {
                *leaf_index += 1;
                (builder.make_leaf(token), vec![token.lexeme.clone()])
            }
            ParseTreeNode::Epsilon => (
                AstNode::Ident(Ident {
                    span: Span::default(),
                    name: "<ε>".to_string(),
                }),
                Vec::new(),
            ),
            ParseTreeNode::NonTerminal { name, children } => {
                let start = *leaf_index;
                let mut reduced = Vec::with_capacity(children.len());
                let mut leaf_texts = Vec::new();

                for child in children {
                    let (child_ast, child_leaf_texts) =
                        self.reduce_node(child, cfg, builder, events, leaf_index);
                    reduced.push(child_ast);
                    leaf_texts.extend(child_leaf_texts);
                }

                let end = *leaf_index;
                let analyzer = PhraseAnalyzer::new();

                if let Some((prod_idx, prod)) =
                    analyzer.find_matching_production(name, children, cfg)
                {
                    let ast = builder.reduce_by_production(prod, reduced);
                    let handle = DirectPhrase {
                        phrase: Phrase {
                            non_terminal: name.clone(),
                            production_index: prod_idx,
                            leaf_start: start,
                            leaf_end: end,
                            leaf_count: end.saturating_sub(start),
                            children_count: children.len(),
                        },
                        leaf_texts: leaf_texts.clone(),
                    };
                    events.push((handle, ast.clone()));
                    return (ast, leaf_texts);
                }

                (pass_through_ast(reduced, name), leaf_texts)
            }
        }
    }

    pub fn reduce_step(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> Option<(DirectPhrase, AstNode)> {
        let builder = GrammarAstBuilder::new(cfg);
        let mut events = Vec::new();
        let mut leaf_index = 0;
        self.reduce_node(tree, cfg, &builder, &mut events, &mut leaf_index);
        events.into_iter().next()
    }

    fn count_leaves(&self, node: &ParseTreeNode) -> usize {
        match node {
            ParseTreeNode::Terminal { .. } => 1,
            ParseTreeNode::Epsilon => 0,
            ParseTreeNode::NonTerminal { children, .. } => {
                children.iter().map(|c| self.count_leaves(c)).sum()
            }
        }
    }

    pub fn reduce_with_trace(
        &self,
        tree: &ParseTreeNode,
        cfg: &ContextFreeGrammar,
    ) -> (AstNode, Vec<String>) {
        let mut trace = Vec::new();
        trace.push("=== Handle Reduction Loop ===".to_string());
        trace.push(format!("Initial tree leaves: {}", self.count_leaves(tree)));

        let builder = GrammarAstBuilder::new(cfg);
        let mut events = Vec::new();
        let mut leaf_index = 0;
        let (ast, _) = self.reduce_node(tree, cfg, &builder, &mut events, &mut leaf_index);

        for (step, (handle, _)) in events.iter().enumerate() {
            trace.push(format!(
                "Step {}: handle = {} -> {}",
                step + 1,
                handle.phrase.non_terminal,
                if handle.leaf_texts.is_empty() {
                    "ε".to_string()
                } else {
                    handle.leaf_texts.join(" ")
                }
            ));
        }

        trace.push("No more direct phrases (accept state)".to_string());
        trace.push(format!("Reduction complete in {} steps", events.len()));
        (ast, trace)
    }
}

impl Default for HandleReducer {
    fn default() -> Self {
        Self::new()
    }
}

fn pass_through_ast(children: Vec<AstNode>, name: &str) -> AstNode {
    let non_epsilon: Vec<AstNode> = children
        .into_iter()
        .filter(|n| !matches!(n, AstNode::Ident(i) if i.name == "<ε>"))
        .collect();
    match non_epsilon.len() {
        0 => AstNode::Ident(Ident {
            span: Span::default(),
            name: "<ε>".to_string(),
        }),
        1 => non_epsilon.into_iter().next().unwrap(),
        _ => AstNode::Ident(Ident {
            span: Span::default(),
            name: name.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;
    use crate::parser::Parser;
    use starry_ast::TokenStreamBuilder;

    fn parse_and_get_tree(grammar_str: &str, input_tokens: Vec<(&str, &str)>) -> (ContextFreeGrammar, ParseTreeNode) {
        let cfg = ContextFreeGrammar::parse(grammar_str).unwrap();

        let mut builder = TokenStreamBuilder::new();
        for (kind, val) in &input_tokens {
            match *kind {
                "int" => {
                    let v: i64 = val.parse().unwrap();
                    builder.add_integer(v, val.to_string());
                }
                "op" => { builder.add_operator(val.to_string()); }
                "del" => { builder.add_delimiter(val.to_string()); }
                "id" => { builder.add_identifier(val.to_string()); }
                _ => { builder.add_identifier(val.to_string()); }
            }
        }
        let stream = builder.build();

        let mut parser = Parser::new(cfg.clone(), stream);
        let tree = parser.parse().unwrap();
        (cfg, tree)
    }

    #[test]
    fn test_find_phrases_simple() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> num",
            vec![("int", "42")],
        );

        let analyzer = PhraseAnalyzer::new();
        let phrases = analyzer.find_phrases(&tree, &cfg);

        assert_eq!(phrases.len(), 1);
        assert_eq!(phrases[0].non_terminal, "E");
        assert_eq!(phrases[0].leaf_count, 1);
    }

    #[test]
    fn test_find_phrases_expression() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let analyzer = PhraseAnalyzer::new();
        let phrases = analyzer.find_phrases(&tree, &cfg);

        assert!(phrases.len() >= 2);
    }

    #[test]
    fn test_find_direct_phrases() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let analyzer = PhraseAnalyzer::new();
        let direct = analyzer.find_direct_phrases(&tree, &cfg);

        assert!(!direct.is_empty());
        for dp in &direct {
            assert!(!dp.leaf_texts.is_empty() || dp.phrase.is_epsilon());
        }
    }

    #[test]
    fn test_find_handle() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T\nT -> num",
            vec![("int", "42")],
        );

        let analyzer = PhraseAnalyzer::new();
        let handle = analyzer.find_handle(&tree, &cfg);

        assert!(handle.is_some());
        let h = handle.unwrap();
        assert_eq!(h.phrase.leaf_start, 0);
    }

    #[test]
    fn test_parse_tree_to_ast_simple() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> num",
            vec![("int", "42")],
        );

        let analyzer = PhraseAnalyzer::new();
        let ast = analyzer.parse_tree_to_ast(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_parse_tree_to_ast_expression() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let analyzer = PhraseAnalyzer::new();
        let ast = analyzer.parse_tree_to_ast(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_phrase_display() {
        let phrase = Phrase {
            non_terminal: "E".to_string(),
            production_index: 0,
            leaf_start: 0,
            leaf_end: 1,
            leaf_count: 1,
            children_count: 1,
        };
        let display = format!("{}", phrase);
        assert!(display.contains("E"));
        assert!(display.contains("0..1"));
    }

    #[test]
    fn test_direct_phrase_display() {
        let dp = DirectPhrase {
            phrase: Phrase {
                non_terminal: "T".to_string(),
                production_index: 1,
                leaf_start: 0,
                leaf_end: 1,
                leaf_count: 1,
                children_count: 1,
            },
            leaf_texts: vec!["42".to_string()],
        };
        let display = format!("{}", dp);
        assert!(display.contains("T"));
        assert!(display.contains("42"));
    }

    #[test]
    fn test_no_handle_on_empty_tree() {
        let cfg = ContextFreeGrammar::parse("S -> ε").unwrap();
        let tree = ParseTreeNode::epsilon();

        let analyzer = PhraseAnalyzer::new();
        let handle = analyzer.find_handle(&tree, &cfg);

        assert!(handle.is_none());
    }

    #[test]
    fn test_parse_tree_to_ast_with_parens() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> F * T | F\nF -> ( E ) | num",
            vec![("del", "("), ("int", "3"), ("op", "+"), ("int", "4"), ("del", ")")],
        );

        let analyzer = PhraseAnalyzer::new();
        let ast = analyzer.parse_tree_to_ast(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_phrase_matching_uses_exact_terminals() {
        let (cfg, tree) = parse_and_get_tree(
            "F -> [ E ] | ( E )\nE -> num",
            vec![("del", "("), ("int", "7"), ("del", ")")],
        );

        let analyzer = PhraseAnalyzer::new();
        let phrases = analyzer.find_phrases(&tree, &cfg);
        let root = phrases.last().unwrap();

        assert_eq!(root.non_terminal, "F");
        assert_eq!(root.production_index, 1);
    }

    #[test]
    fn test_handle_reducer_uses_exact_terminals() {
        let (cfg, tree) = parse_and_get_tree(
            "F -> [ E ] | ( E )\nE -> num",
            vec![("del", "("), ("int", "7"), ("del", ")")],
        );

        let reducer = HandleReducer::new();
        let (ast, trace) = reducer.reduce_with_trace(&tree, &cfg);

        assert!(trace.iter().any(|step| step.contains("handle = F")));
        assert_eq!(format!("{}", ast), "(7)");
    }

    #[test]
    fn test_handle_reducer_single_step() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> num",
            vec![("int", "42")],
        );

        let reducer = HandleReducer::new();
        let result = reducer.reduce_step(&tree, &cfg);

        assert!(result.is_some());
        let (handle, reduced_ast) = result.unwrap();
        assert_eq!(handle.phrase.non_terminal, "E");
        assert_eq!(handle.phrase.leaf_count, 1);
        assert_eq!(handle.leaf_texts, vec!["42"]);
        assert_eq!(format!("{}", reduced_ast), "42");
    }

    #[test]
    fn test_handle_reducer_reduce_all_simple() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> num",
            vec![("int", "42")],
        );

        let reducer = HandleReducer::new();
        let ast = reducer.reduce_all(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(display.contains("42"));
    }

    #[test]
    fn test_handle_reducer_reduce_all_expression() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let reducer = HandleReducer::new();
        let ast = reducer.reduce_all(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_handle_reducer_reduce_all_with_parens() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> F * T | F\nF -> ( E ) | num",
            vec![("del", "("), ("int", "3"), ("op", "+"), ("int", "4"), ("del", ")")],
        );

        let reducer = HandleReducer::new();
        let ast = reducer.reduce_all(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_handle_reduction_trace() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let reducer = HandleReducer::new();
        let (ast, trace) = reducer.reduce_with_trace(&tree, &cfg);

        assert!(!trace.is_empty());
        assert!(trace.iter().any(|s| s.contains("Handle Reduction Loop")));
        assert!(trace.iter().any(|s| s.contains("Reduction complete")));

        let display = format!("{}", ast);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_handle_reducer_vs_phrase_analyzer_same_result() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> num",
            vec![("int", "1"), ("op", "+"), ("int", "2")],
        );

        let analyzer = PhraseAnalyzer::new();
        let ast_direct = analyzer.parse_tree_to_ast(&tree, &cfg);

        let reducer = HandleReducer::new();
        let ast_handle = reducer.reduce_all(&tree, &cfg);

        assert_eq!(
            format!("{}", ast_direct),
            format!("{}", ast_handle),
            "HandleReducer and PhraseAnalyzer should produce identical ASTs"
        );
    }

    #[test]
    fn test_handle_reducer_vs_phrase_analyzer_with_parens() {
        let (cfg, tree) = parse_and_get_tree(
            "E -> T + E | T\nT -> F * T | F\nF -> ( E ) | num",
            vec![("del", "("), ("int", "3"), ("op", "+"), ("int", "4"), ("del", ")")],
        );

        let analyzer = PhraseAnalyzer::new();
        let ast_direct = analyzer.parse_tree_to_ast(&tree, &cfg);

        let reducer = HandleReducer::new();
        let ast_handle = reducer.reduce_all(&tree, &cfg);

        assert_eq!(
            format!("{}", ast_direct),
            format!("{}", ast_handle),
            "HandleReducer and PhraseAnalyzer should produce identical ASTs"
        );
    }

    #[test]
    fn test_handle_reducer_empty_epsilon() {
        let cfg = ContextFreeGrammar::parse("S -> ε").unwrap();
        let tree = ParseTreeNode::epsilon();

        let reducer = HandleReducer::new();
        let ast = reducer.reduce_all(&tree, &cfg);

        let display = format!("{}", ast);
        assert!(display.contains("<ε>") || display.contains("ε"));
    }

    #[test]
    fn test_handle_reducer_no_handle_no_panic() {
        let cfg = ContextFreeGrammar::parse("S -> ε").unwrap();
        let tree = ParseTreeNode::epsilon();

        let reducer = HandleReducer::new();
        let result = reducer.reduce_step(&tree, &cfg);
        assert!(result.is_none());
    }
}
