//! Byte-level CSS lexing helpers shared by the text rewrites (`media`, `gradient`). They skip the tokens
//! whose contents must never be rewritten — comments, strings, escapes, unquoted `url(…)` — and never
//! split a multi-byte character, so callers can slice the text at the positions they return.

pub(crate) fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'-' || c >= 0x80
}

pub(crate) fn ident_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len()
        && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'-') || b[i] >= 0x80)
    {
        i += 1;
    }
    i
}

pub(crate) fn skip_comment(css: &str, i: usize) -> usize {
    css[i + 2..].find("*/").map_or(css.len(), |e| i + 2 + e + 2)
}

/// Past a string token. Like the CSS tokenizer, an unescaped newline ends an unterminated string.
pub(crate) fn skip_string(b: &[u8], i: usize) -> usize {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() && b[j] != q {
        if b[j] == b'\n' {
            return j;
        }
        if b[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(b.len())
}

/// Past a backslash and the whole character it escapes (which may be multi-byte).
pub(crate) fn skip_escape(css: &str, i: usize) -> usize {
    i + 1 + css[i + 1..].chars().next().map_or(0, char::len_utf8)
}

/// `url(` not preceded by an identifier character and not followed by a quote: its contents are one
/// unquoted-url token, which may contain `/*`, quotes or `@`. A quoted `url("…")` is scanned normally.
pub(crate) fn is_unquoted_url(b: &[u8], i: usize) -> bool {
    let after = i + 4;
    b.len() > after
        && b[i..after].eq_ignore_ascii_case(b"url(")
        && (i == 0
            || !(b[i - 1].is_ascii_alphanumeric()
                || matches!(b[i - 1], b'_' | b'-' | b'\\')
                || b[i - 1] >= 0x80))
        && !matches!(
            b[after..].iter().find(|c| !c.is_ascii_whitespace()),
            Some(b'"' | b'\'')
        )
}

/// Past the `)` closing an unquoted `url(`.
pub(crate) fn skip_unquoted_url(b: &[u8], i: usize) -> usize {
    let mut j = i + 4;
    while j < b.len() && b[j] != b')' {
        if b[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(b.len())
}

/// `css` with whatever is still open at its end closed: a comment, a string, an unquoted `url(`, and every
/// `{`, `(` and `[` block, innermost first. CSS Syntax 3 §5.4 closes them at the end of the input anyway, so
/// the result means the same; it keeps fulgur 0.40.0's `::marker` pre-pass from panicking on a block opened
/// at the very end of a stylesheet (F10).
pub(crate) fn close_open_blocks(css: &str) -> std::borrow::Cow<'_, str> {
    let b = css.as_bytes();
    let mut open: Vec<u8> = Vec::new();
    let mut tail = String::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => match css[i + 2..].find("*/") {
                Some(e) => i += 2 + e + 2,
                None => {
                    tail.push_str("*/");
                    break;
                }
            },
            q @ (b'"' | b'\'') => {
                let mut j = i + 1;
                while j < b.len() && b[j] != q && b[j] != b'\n' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                if j >= b.len() {
                    tail.push(q as char);
                    break;
                }
                i = j + 1;
            }
            b'\\' => i = skip_escape(css, i),
            b'u' | b'U' if is_unquoted_url(b, i) => {
                let end = skip_unquoted_url(b, i);
                if b.get(end - 1) != Some(&b')') {
                    tail.push(')');
                    break;
                }
                i = end;
            }
            c @ (b'{' | b'(' | b'[') => {
                open.push(c);
                i += 1;
            }
            c @ (b'}' | b')' | b']') => {
                let opener = match c {
                    b'}' => b'{',
                    b')' => b'(',
                    _ => b'[',
                };
                // A closer only closes its own kind of block; otherwise it is an ordinary token.
                if open.last() == Some(&opener) {
                    open.pop();
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    for c in open.iter().rev() {
        tail.push(match c {
            b'{' => '}',
            b'(' => ')',
            _ => ']',
        });
    }
    if tail.is_empty() {
        std::borrow::Cow::Borrowed(css)
    } else {
        std::borrow::Cow::Owned(format!("{css}{tail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_what_is_open_at_the_end() {
        let c = |s: &str| close_open_blocks(s).into_owned();
        assert_eq!(c("a{"), "a{}");
        assert_eq!(c("@media print{p{color:red"), "@media print{p{color:red}}");
        assert_eq!(c("a{b:calc(1px + (2px"), "a{b:calc(1px + (2px))}");
        assert_eq!(c("a{content:'x"), "a{content:'x'}");
        assert_eq!(c("a{/* x"), "a{/* x*/}");
        assert_eq!(c("a{b:url(x.png"), "a{b:url(x.png)}");
        assert_eq!(c("a{b:'}'"), "a{b:'}'}");
        assert_eq!(c("a{b:(})"), "a{b:(})}");
    }

    #[test]
    fn leaves_complete_css_alone() {
        for css in [
            "",
            "a{b:c}",
            "a{content:'{'} /* { */ b{c:url(x{.png)}",
            "a{b:'x\n}",
            "}}a{}",
            "a{b:\\{}",
        ] {
            assert_eq!(close_open_blocks(css), css, "{css:?}");
        }
    }
}
