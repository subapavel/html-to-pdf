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
