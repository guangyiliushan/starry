//! 展示辅助模块
//!
//! dump-nfa 与错误消息共用的转义显示。转义能力直接来自 std 的
//! `char::escape_debug()` 迭代器，经 [`fmt::Write::write_char`] 逐字符
//! 写入——全程零堆分配。注意 `str::escape_debug()` 返回的是迭代器而
//! 非 `Cow`；本模块不提供手写的、返回 `String` 的 escape 函数。
//!
//! # 示例
//!
//! ```
//! use std::fmt::Write;
//! use lex::display::write_escaped_str;
//!
//! let mut out = String::new();
//! write_escaped_str(&mut out, "a\tb").unwrap();
//! assert_eq!(out, "a\\tb");
//! ```

use std::fmt;

/// 将字符串转义后写入 formatter（控制字符/引号显示为转义形式）
pub fn write_escaped_str<W: fmt::Write>(f: &mut W, s: &str) -> fmt::Result {
    for c in s.chars() {
        write_escaped_char(f, c)?;
    }
    Ok(())
}

/// 将单个字符转义后写入 formatter（如转移边标签）
pub fn write_escaped_char<W: fmt::Write>(f: &mut W, c: char) -> fmt::Result {
    for part in c.escape_debug() {
        f.write_char(part)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_content() {
        // 干净输入原样透传
        let mut out = String::new();
        write_escaped_str(&mut out, "plain").unwrap();
        assert_eq!(out, "plain");

        // 控制字符显示为转义形式
        let mut out = String::new();
        write_escaped_str(&mut out, "a\tb").unwrap();
        assert_eq!(out, "a\\tb");

        // 单字符路径
        let mut out = String::new();
        write_escaped_char(&mut out, '\n').unwrap();
        assert_eq!(out, "\\n");
    }
}
