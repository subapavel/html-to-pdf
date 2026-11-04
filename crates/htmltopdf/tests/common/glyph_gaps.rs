//! Reader for `fixtures/glyph-gaps.txt` (F101): the fixtures that contain a character no bundled font maps. Shared by
//! the tests that render every fixture; `scripts/render-fixtures.sh` and `scripts/bench.py` read the same file.
//! Included with `#[path = "common/glyph_gaps.rs"] mod glyph_gaps;`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

/// Fixture directory name -> the characters it is listed for, in file order.
pub fn listed() -> BTreeMap<String, Vec<char>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/glyph-gaps.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let name = words.next().unwrap().to_string();
        let chars: Vec<char> = words
            .map(|w| {
                let hex = w
                    .strip_prefix("U+")
                    .unwrap_or_else(|| panic!("glyph-gaps.txt:{}: `{w}` is not U+XXXX", n + 1));
                char::from_u32(u32::from_str_radix(hex, 16).unwrap_or_else(|_| {
                    panic!("glyph-gaps.txt:{}: `{w}` is not hexadecimal", n + 1)
                }))
                .unwrap_or_else(|| panic!("glyph-gaps.txt:{}: `{w}` is not a character", n + 1))
            })
            .collect();
        assert!(
            !chars.is_empty(),
            "glyph-gaps.txt:{}: `{name}` lists no code point",
            n + 1
        );
        assert!(
            out.insert(name.clone(), chars).is_none(),
            "glyph-gaps.txt:{}: `{name}` is listed twice",
            n + 1
        );
    }
    out
}

/// Whether the fixture is listed, so it is rendered without `strict_fonts`.
pub fn is_listed(fixture: &str) -> bool {
    listed().contains_key(fixture)
}
