//! 端到端集成测试：regex 字符串 → parse → translate → optimize（from_hir 内部）
//! → NFA → match_prefix 全链路。

use lex::nfa::NFA;
use lex::regex::parse::parse;
use lex::regex::Translate;
use lex::TokenKind;

fn match_len(pattern: &str, input: &str) -> Option<usize> {
    let ast = parse(pattern).unwrap();
    let hir = Translate::new().translate(&ast);
    let nfa = NFA::from_hir(&hir, TokenKind::Identifier);
    nfa.match_prefix(input)
}

#[test]
fn literal() {
    assert_eq!(match_len("a", "a"), Some(1));
    assert_eq!(match_len("a", "b"), None);
}

#[test]
fn sequence() {
    assert_eq!(match_len("ab", "ab"), Some(2));
    assert_eq!(match_len("ab", "ac"), None);
}

#[test]
fn choice() {
    assert_eq!(match_len("a|b", "b"), Some(1));
}

#[test]
fn star_empty_match_is_some_zero() {
    // 空匹配与不匹配语义不可折叠：Some(0) != None
    assert_eq!(match_len("a*", "b"), Some(0));
    assert_eq!(match_len("a*", ""), Some(0));
    assert_eq!(match_len("a", "b"), None);
}

#[test]
fn char_class() {
    assert_eq!(match_len("[a-z]+", "abc9"), Some(3));
    assert_eq!(match_len("[0-9]", "5"), Some(1));
}

#[test]
fn repeat_range() {
    assert_eq!(match_len("a{2,3}", "aaaa"), Some(3));
    assert_eq!(match_len("a{2,3}", "a"), None);
}

#[test]
fn case_insensitive_basic() {
    assert_eq!(match_len("(?i)abc", "aBc"), Some(3));
    // 'Z' 换位地雷回归：大写字母折叠不 panic
    assert_eq!(match_len("(?i)Z", "z"), Some(1));
    assert_eq!(match_len("(?i)Z", "Z"), Some(1));
    // 非 ASCII 不折叠；(?i)1 无 lower==upper 特判也正确
    assert_eq!(match_len("(?i)é", "é"), Some(2));
    assert_eq!(match_len("(?i)1", "1"), Some(1));
}

#[test]
fn case_insensitive_scope() {
    // a 敏感、b 不敏感
    assert_eq!(match_len("a(?i)b", "aB"), Some(2));
    assert_eq!(match_len("a(?i)b", "Ab"), None);

    // 组内不敏感、出组恢复敏感
    assert_eq!(match_len("((?i)a)b", "Ab"), Some(2));
    assert_eq!(match_len("((?i)a)b", "ab"), Some(2));

    // 负向标记关闭不敏感
    // (?-i) 之后 b、c 均恢复敏感：AbC 的 'C' 不再匹配
    assert_eq!(match_len("((?i)a(?-i)b)c", "Abc"), Some(3));
    assert_eq!(match_len("((?i)a(?-i)b)c", "abc"), Some(3));
    assert_eq!(match_len("((?i)a(?-i)b)c", "ABc"), None);

    // 顶层跨 Alt：(?i) 作用到模式末尾
    assert_eq!(match_len("(?i)a|b", "B"), Some(1));
    assert_eq!(match_len("(?i)a|b", "A"), Some(1));

    // 分支继承（PCRE 语义）：同组内 c 跟随 (?i)
    assert_eq!(match_len("(a(?i)b|c)", "C"), Some(1));
    assert_eq!(match_len("(a(?i)b|c)", "aB"), Some(2));
}

#[test]
fn multi_rule_longest_match_wins() {
    let hir_ab = Translate::new().translate(&parse("ab").unwrap());
    let hir_a = Translate::new().translate(&parse("a").unwrap());

    let nfa = NFA::from_hir_multi(vec![
        (hir_ab, TokenKind::Identifier),
        (hir_a, TokenKind::Eof),
    ]);

    assert_eq!(nfa.match_prefix("ab"), Some(2));
    assert_eq!(nfa.match_prefix("a"), Some(1));
}

#[test]
fn multi_rule_same_length_first_rule_wins() {
    // 同长冲突首规则胜：迭代序 = 规则分配序的行为化验证
    let hir1 = Translate::new().translate(&parse("a").unwrap());
    let hir2 = Translate::new().translate(&parse("a").unwrap());

    let nfa = NFA::from_hir_multi(vec![
        (hir1, TokenKind::Identifier),
        (hir2, TokenKind::Eof),
    ]);

    let current = nfa.step(&nfa.epsilon_closure_of(nfa.start_state()), 'a');
    let winner = current
        .iter()
        .find(|&&id| nfa.is_accepting(id))
        .expect("两个规则都应接受 'a'");

    assert_eq!(nfa.token_kind(*winner), Some(TokenKind::Identifier));
}
