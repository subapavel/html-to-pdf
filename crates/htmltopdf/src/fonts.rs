//! Fonts the crate ships itself, and the static `font-family` check.
//!
//! Fonts are bundled bytes (never host fonts), so a document that names a face the caller did not
//! supply would otherwise lose its text silently. Two defences live here:
//!
//! 1. A built-in last-resort face (see ADR-0002) that keeps text such as IBANs and variable symbols
//!    (`font-mono`) in the PDF.
//! 2. A static scan of every `font-family` / `font` declaration (bundled CSS, `<style>` blocks,
//!    `style=""` attributes) that reports lists in which *no* entry can be satisfied, before rendering.
//!
//! The scan is deliberately conservative: anything it cannot interpret (`calc()`, unknown `var()` in a
//! shorthand, CSS-wide keywords) counts as resolved, so it never rejects a document it does not
//! understand. It is a safety net, not a cascade implementation.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, ParserInput, Token};

/// JetBrains Mono Regular (SIL OFL 1.1, licence text in `fonts/OFL.txt`). Covers Latin Extended-A.
pub(crate) const BUILTIN_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");

/// Family name of [`BUILTIN_MONO`], as it appears in the font's name table.
pub(crate) const BUILTIN_MONO_FAMILY: &str = "JetBrains Mono";

/// Generic `font-family` keywords (CSS Fonts 4, `<generic-family>`), plus the `ui-*` and other
/// keywords that the CSS engine in use parses as plain family names. `htmltopdf` maps all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GenericFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    SystemUi,
    UiSerif,
    UiSansSerif,
    UiMonospace,
    UiRounded,
    Math,
    Emoji,
    FangSong,
}

impl GenericFamily {
    pub(crate) const ALL: [GenericFamily; 13] = [
        GenericFamily::Serif,
        GenericFamily::SansSerif,
        GenericFamily::Monospace,
        GenericFamily::Cursive,
        GenericFamily::Fantasy,
        GenericFamily::SystemUi,
        GenericFamily::UiSerif,
        GenericFamily::UiSansSerif,
        GenericFamily::UiMonospace,
        GenericFamily::UiRounded,
        GenericFamily::Math,
        GenericFamily::Emoji,
        GenericFamily::FangSong,
    ];

    /// The CSS keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            GenericFamily::Serif => "serif",
            GenericFamily::SansSerif => "sans-serif",
            GenericFamily::Monospace => "monospace",
            GenericFamily::Cursive => "cursive",
            GenericFamily::Fantasy => "fantasy",
            GenericFamily::SystemUi => "system-ui",
            GenericFamily::UiSerif => "ui-serif",
            GenericFamily::UiSansSerif => "ui-sans-serif",
            GenericFamily::UiMonospace => "ui-monospace",
            GenericFamily::UiRounded => "ui-rounded",
            GenericFamily::Math => "math",
            GenericFamily::Emoji => "emoji",
            GenericFamily::FangSong => "fangsong",
        }
    }

    fn from_keyword(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|g| g.keyword().eq_ignore_ascii_case(s))
    }
}

/// Where a `font-family` list was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A stylesheet bundled with the renderer.
    Stylesheet,
    /// A `<style>` element in the document.
    StyleElement,
    /// A `style=""` attribute in the document.
    StyleAttribute,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Origin::Stylesheet => "bundled stylesheet",
            Origin::StyleElement => "<style> element",
            Origin::StyleAttribute => "style attribute",
        })
    }
}

/// A `font-family` list in which no entry matches a registered face or a mapped generic family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedFamily {
    /// The list as written, e.g. `'Nope', "Other"`.
    pub font_family: String,
    pub origin: Origin,
}

impl std::fmt::Display for UnresolvedFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}` ({})", self.font_family, self.origin)
    }
}

/// Lower-cased (ASCII) family names of the registered faces. CSS Fonts 4 §5.1: names match
/// ASCII case-insensitively.
pub(crate) type FamilySet = BTreeSet<String>;

/// Generic keyword to the registered family it maps to.
pub(crate) type GenericMap = BTreeMap<GenericFamily, String>;

pub(crate) fn fold(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Family names of every face in a font file, using the rule fontique applies when it registers the
/// font: the typographic family (name ID 16), else the family (ID 1); the English (US) name if there
/// is one, else the first.
pub(crate) fn family_names(font: &[u8]) -> Vec<String> {
    let count = ttf_parser::fonts_in_collection(font).unwrap_or(1);
    let mut out: Vec<String> = Vec::new();
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(font, index) else {
            continue;
        };
        let pick = |id: u16| -> Option<String> {
            let names = face.names();
            let mut first = None;
            for i in 0..names.len() {
                let Some(n) = names.get(i) else { continue };
                if n.name_id != id {
                    continue;
                }
                let Some(s) = n.to_string() else { continue };
                if n.language_id == 0x0409 {
                    return Some(s);
                }
                first.get_or_insert(s);
            }
            first
        };
        if let Some(name) = pick(16).or_else(|| pick(1)) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------------------------

/// One comma-separated entry of a `font-family` list.
#[derive(Debug, Clone, PartialEq)]
enum Entry {
    Name(String),
    Generic(GenericFamily),
    /// `inherit`, `initial`, `unset`, `revert`, `revert-layer`: the value comes from elsewhere.
    CssWide,
    Var {
        name: String,
        fallback: Vec<Entry>,
    },
    /// Anything this scan does not interpret.
    Other,
}

#[derive(Debug, Clone)]
struct Decl {
    raw: String,
    entries: Vec<Entry>,
    origin: Origin,
}

/// Font-related declarations found in some CSS, in document order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Scan {
    decls: Vec<Decl>,
    /// All values ever declared for each custom property (`--font-mono` → every list assigned to it).
    vars: BTreeMap<String, Vec<Vec<Entry>>>,
}

impl Scan {
    pub(crate) fn extend(&mut self, other: Scan) {
        self.decls.extend(other.decls);
        for (k, v) in other.vars {
            self.vars.entry(k).or_default().extend(v);
        }
    }
}

const CSS_WIDE: [&str; 5] = ["inherit", "initial", "unset", "revert", "revert-layer"];

/// Scans a stylesheet or a declaration list (the content of a `style=""` attribute).
pub(crate) fn scan_css(css: &str, origin: Origin) -> Scan {
    let mut scan = Scan::default();
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    scan_block(&mut parser, &mut scan, origin);
    scan
}

fn scan_block(p: &mut Parser<'_, '_>, scan: &mut Scan, origin: Origin) {
    let mut at_statement_start = true;
    loop {
        let tok = match p.next() {
            Ok(t) => t.clone(),
            Err(_) => return,
        };
        match tok {
            Token::Semicolon => at_statement_start = true,
            Token::CurlyBracketBlock => {
                let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    scan_block(p, scan, origin);
                    Ok(())
                });
                at_statement_start = true;
            }
            Token::Ident(name) if at_statement_start => {
                at_statement_start = false;
                let kind = if name.eq_ignore_ascii_case("font-family") {
                    Some(Prop::Family)
                } else if name.eq_ignore_ascii_case("font") {
                    Some(Prop::Shorthand)
                } else if name.starts_with("--") {
                    Some(Prop::Custom)
                } else {
                    None
                };
                let Some(kind) = kind else { continue };
                if p.try_parse(|p| p.expect_colon()).is_err() {
                    continue;
                }
                let start = p.position();
                let entries = match kind {
                    Prop::Shorthand => parse_font_shorthand(p),
                    _ => Some(parse_list(p)),
                };
                // `parse_list` stops before `;` / `!important` / the end of the block.
                let raw = p.slice_from(start).trim().to_string();
                skip_to_statement_end(p);
                at_statement_start = true;
                let Some(entries) = entries else { continue };
                match kind {
                    Prop::Custom => scan.vars.entry(name.to_string()).or_default().push(entries),
                    _ => scan.decls.push(Decl {
                        raw,
                        entries,
                        origin,
                    }),
                }
            }
            _ => at_statement_start = false,
        }
    }
}

#[derive(Clone, Copy)]
enum Prop {
    Family,
    Shorthand,
    Custom,
}

fn skip_to_statement_end(p: &mut Parser<'_, '_>) {
    loop {
        match p.next() {
            Ok(Token::Semicolon) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

/// Parses a comma-separated `font-family` list up to (not including) `;`, `!` or the end of the block.
fn parse_list(p: &mut Parser<'_, '_>) -> Vec<Entry> {
    #[derive(Default)]
    struct Cur {
        idents: Vec<String>,
        single: Option<Entry>,
        other: bool,
    }
    impl Cur {
        fn flush(&mut self, out: &mut Vec<Entry>) {
            let c = std::mem::take(self);
            let entry = if c.other || (c.single.is_some() && !c.idents.is_empty()) {
                Entry::Other
            } else if let Some(e) = c.single {
                e
            } else if c.idents.len() == 1 {
                let id = &c.idents[0];
                if CSS_WIDE.iter().any(|k| k.eq_ignore_ascii_case(id)) {
                    Entry::CssWide
                } else if let Some(g) = GenericFamily::from_keyword(id) {
                    Entry::Generic(g)
                } else {
                    Entry::Name(id.clone())
                }
            } else if !c.idents.is_empty() {
                // Unquoted multi-word name: identifiers joined by single spaces (CSS Fonts 4 §4.2).
                Entry::Name(c.idents.join(" "))
            } else {
                return; // empty slot, e.g. a trailing comma
            };
            out.push(entry);
        }
    }

    let mut out = Vec::new();
    let mut cur = Cur::default();
    loop {
        let state = p.state();
        let tok = match p.next() {
            Ok(t) => t.clone(),
            Err(_) => break,
        };
        match tok {
            Token::Semicolon => {
                p.reset(&state);
                break;
            }
            Token::Delim('!') => {
                p.reset(&state);
                break;
            }
            Token::Comma => cur.flush(&mut out),
            Token::Ident(id) => cur.idents.push(id.to_string()),
            Token::QuotedString(s) => {
                if cur.single.is_some() || !cur.idents.is_empty() {
                    cur.other = true;
                }
                cur.single = Some(Entry::Name(s.to_string()));
            }
            Token::Function(f) if f.eq_ignore_ascii_case("var") => {
                let parsed = p.parse_nested_block(|p| -> Result<Entry, ParseError<'_, ()>> {
                    let name = p.expect_ident()?.to_string();
                    let fallback = if p.try_parse(|p| p.expect_comma()).is_ok() {
                        parse_list(p)
                    } else {
                        Vec::new()
                    };
                    Ok(Entry::Var { name, fallback })
                });
                match parsed {
                    Ok(e) if cur.single.is_none() && cur.idents.is_empty() => cur.single = Some(e),
                    _ => cur.other = true,
                }
            }
            _ => cur.other = true,
        }
    }
    cur.flush(&mut out);
    out
}

/// `font: [style || variant || weight || stretch]? size[/line-height]? family-list`.
/// Returns `None` when the shorthand cannot be interpreted (system font keywords, `var()` before the
/// family list, CSS-wide keyword): such a declaration is ignored.
fn parse_font_shorthand(p: &mut Parser<'_, '_>) -> Option<Vec<Entry>> {
    const PREFIX: [&str; 13] = [
        "normal",
        "italic",
        "oblique",
        "small-caps",
        "bold",
        "bolder",
        "lighter",
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "semi-expanded",
        "expanded",
    ];
    const SIZE_KEYWORDS: [&str; 11] = [
        "xx-small",
        "x-small",
        "small",
        "medium",
        "large",
        "x-large",
        "xx-large",
        "xxx-large",
        "larger",
        "smaller",
        "math",
    ];
    loop {
        let tok = p.next().ok()?.clone();
        match tok {
            Token::Ident(id) if PREFIX.iter().any(|k| k.eq_ignore_ascii_case(&id)) => {}
            Token::Ident(id) if id.eq_ignore_ascii_case("ultra-expanded") => {}
            Token::Ident(id) if id.eq_ignore_ascii_case("extra-expanded") => {}
            Token::Number { .. } => {}
            Token::Dimension { ref unit, .. }
                if ["deg", "grad", "rad", "turn"]
                    .iter()
                    .any(|u| u.eq_ignore_ascii_case(unit)) => {}
            Token::Dimension { .. } | Token::Percentage { .. } => break,
            Token::Ident(id) if SIZE_KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(&id)) => break,
            Token::Function(f) if !f.eq_ignore_ascii_case("var") => {
                let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    while p.next().is_ok() {}
                    Ok(())
                });
                break;
            }
            _ => return None,
        }
    }
    if p.try_parse(|p| p.expect_delim('/')).is_ok() {
        match p.next().ok()?.clone() {
            Token::Function(_) => {
                let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    while p.next().is_ok() {}
                    Ok(())
                });
            }
            Token::Number { .. }
            | Token::Dimension { .. }
            | Token::Percentage { .. }
            | Token::Ident(_) => {}
            _ => return None,
        }
    }
    Some(parse_list(p))
}

/// Scans the CSS inside an HTML document: `<style>` elements and `style=""` attributes, in document
/// order. This is a deliberately small tag scanner, not an HTML parser; it understands comments,
/// quoted/unquoted attribute values and raw-text elements, which is what the scan needs.
pub(crate) fn scan_html(html: &str) -> Scan {
    let bytes = html.as_bytes();
    let mut scan = Scan::default();
    let mut i = 0;
    while let Some(off) = html[i..].find('<') {
        i += off;
        let rest = &html[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |e| e + 3);
            continue;
        }
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        if !next.is_ascii_alphabetic() {
            // `</x>`, `<!doctype>`, `<?xml?>` or a stray `<`.
            i += rest.find('>').map_or(rest.len(), |e| e + 1);
            continue;
        }
        // Opening tag.
        let mut j = i + 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let tag = html[i + 1..j].to_ascii_lowercase();
        loop {
            while j < bytes.len() && (bytes[j].is_ascii_whitespace() || bytes[j] == b'/') {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            if bytes[j] == b'>' {
                j += 1;
                break;
            }
            let name_start = j;
            while j < bytes.len()
                && !bytes[j].is_ascii_whitespace()
                && !matches!(bytes[j], b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let name = html[name_start..j].to_ascii_lowercase();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = "";
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let q = bytes[j];
                    let vs = j + 1;
                    j = vs;
                    while j < bytes.len() && bytes[j] != q {
                        j += 1;
                    }
                    value = &html[vs..j.min(bytes.len())];
                    j = (j + 1).min(bytes.len());
                } else {
                    let vs = j;
                    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                        j += 1;
                    }
                    value = &html[vs..j];
                }
            }
            if name == "style" {
                scan.extend(scan_css(&decode_entities(value), Origin::StyleAttribute));
            }
        }
        i = j;
        if matches!(tag.as_str(), "style" | "script" | "textarea" | "title") {
            let close = format!("</{tag}");
            let lower = html[i..].to_ascii_lowercase();
            let end = lower.find(&close).unwrap_or(lower.len());
            if tag == "style" {
                scan.extend(scan_css(&html[i..i + end], Origin::StyleElement));
            }
            i += end;
        }
    }
    scan
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------------------------

const MAX_VAR_DEPTH: usize = 16;

/// Custom properties visible to a declaration: the bundled CSS plus the document.
struct Vars<'a> {
    bundled: &'a Scan,
    document: &'a Scan,
}

impl<'a> Vars<'a> {
    fn get(&self, name: &str) -> Vec<&'a Vec<Entry>> {
        self.bundled
            .vars
            .get(name)
            .into_iter()
            .chain(self.document.vars.get(name))
            .flatten()
            .collect()
    }
}

fn resolves(
    entries: &[Entry],
    vars: &Vars<'_>,
    registered: &FamilySet,
    generics: &GenericMap,
    depth: usize,
) -> bool {
    if depth > MAX_VAR_DEPTH || entries.is_empty() {
        return true; // cycle or nothing to judge: do not reject
    }
    entries.iter().any(|e| match e {
        Entry::Name(n) => registered.contains(&fold(n)),
        Entry::Generic(g) => generics
            .get(g)
            .is_some_and(|family| registered.contains(&fold(family))),
        Entry::CssWide | Entry::Other => true,
        Entry::Var { name, fallback } => {
            let values = vars.get(name);
            if values.is_empty() {
                resolves(fallback, vars, registered, generics, depth + 1)
            } else {
                values
                    .into_iter()
                    .any(|v| resolves(v, vars, registered, generics, depth + 1))
            }
        }
    })
}

/// Returns every `font-family` list (bundled CSS first, then the document, each in source order) in
/// which no entry matches a registered family or a mapped generic. `var()` is resolved against the
/// union of all declarations of the custom property across both scans.
pub(crate) fn check_font_families(
    bundled: &Scan,
    document: &Scan,
    registered: &FamilySet,
    generics: &GenericMap,
) -> Vec<UnresolvedFamily> {
    let vars = Vars { bundled, document };
    let mut out: Vec<UnresolvedFamily> = Vec::new();
    for d in bundled.decls.iter().chain(&document.decls) {
        if !resolves(&d.entries, &vars, registered, generics, 0) {
            let u = UnresolvedFamily {
                font_family: d.raw.clone(),
                origin: d.origin,
            };
            if !out.contains(&u) {
                out.push(u);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered(names: &[&str]) -> FamilySet {
        names.iter().map(|n| fold(n)).collect()
    }

    fn run(css: &str, reg: &[&str]) -> Vec<String> {
        run_with(css, reg, &GenericMap::new())
    }

    fn run_with(css: &str, reg: &[&str], generics: &GenericMap) -> Vec<String> {
        check_font_families(
            &scan_css(css, Origin::Stylesheet),
            &Scan::default(),
            &registered(reg),
            generics,
        )
        .into_iter()
        .map(|u| u.font_family)
        .collect()
    }

    #[test]
    fn builtin_family_name_matches_the_name_table() {
        assert_eq!(family_names(BUILTIN_MONO), vec![BUILTIN_MONO_FAMILY]);
    }

    #[test]
    fn quoted_and_unquoted_names() {
        assert!(run("p{font-family:'Inter'}", &["Inter"]).is_empty());
        assert!(run("p{font-family:Inter}", &["Inter"]).is_empty());
        assert!(run("p{font-family:\"JetBrains Mono\"}", &["JetBrains Mono"]).is_empty());
        // Unquoted multi-word names are legal and identifier-joined.
        assert!(run("p{font-family:JetBrains   Mono}", &["JetBrains Mono"]).is_empty());
        assert_eq!(run("p{font-family:'Nope'}", &["Inter"]), vec!["'Nope'"]);
    }

    #[test]
    fn names_match_ascii_case_insensitively() {
        assert!(run("p{font-family:INTER}", &["Inter"]).is_empty());
        assert!(run("p{FONT-FAMILY:'inter'}", &["Inter"]).is_empty());
    }

    #[test]
    fn one_matching_entry_is_enough() {
        assert!(run("p{font-family:'Nope', Inter, 'Other'}", &["Inter"]).is_empty());
        assert_eq!(
            run("p{font-family:'Nope', 'Other'}", &["Inter"]),
            vec!["'Nope', 'Other'"]
        );
    }

    #[test]
    fn generics_count_only_when_mapped() {
        assert_eq!(
            run("p{font-family:monospace}", &["Inter"]),
            vec!["monospace"]
        );
        let mut g = GenericMap::new();
        g.insert(GenericFamily::Monospace, "Inter".into());
        assert!(run_with("p{font-family:'Nope', MONOSPACE}", &["Inter"], &g).is_empty());
        // Mapped to a family that is not registered: still unresolved.
        g.insert(GenericFamily::Monospace, "Missing".into());
        assert_eq!(
            run_with("p{font-family:monospace}", &["Inter"], &g),
            vec!["monospace"]
        );
        // A quoted generic keyword is a family name, not the keyword.
        let mut g = GenericMap::new();
        g.insert(GenericFamily::Monospace, "Inter".into());
        assert_eq!(
            run_with("p{font-family:'monospace'}", &["Inter"], &g),
            vec!["'monospace'"]
        );
    }

    #[test]
    fn var_resolves_against_all_declarations_with_fallback() {
        let css = ":root{--a:'Nope', Inter} p{font-family:var(--a)}";
        assert!(run(css, &["Inter"]).is_empty());
        let css = ":root{--a:'Nope'} p{font-family:var(--a)}";
        assert_eq!(run(css, &["Inter"]), vec!["var(--a)"]);
        // Undefined variable: the fallback decides.
        assert!(run("p{font-family:var(--x, Inter)}", &["Inter"]).is_empty());
        assert_eq!(
            run("p{font-family:var(--x, 'Nope')}", &["Inter"]),
            vec!["var(--x, 'Nope')"]
        );
        // Defined variable wins over the fallback.
        let css = ":root{--a:'Nope'} p{font-family:var(--a, Inter)}";
        assert_eq!(run(css, &["Inter"]), vec!["var(--a, Inter)"]);
        // Union of every declaration of the property (e.g. a dark theme override).
        let css = ":root{--a:'Nope'} .d{--a:Inter} p{font-family:var(--a)}";
        assert!(run(css, &["Inter"]).is_empty());
        // Chains and cycles.
        let css = ":root{--a:var(--b);--b:Inter} p{font-family:var(--a)}";
        assert!(run(css, &["Inter"]).is_empty());
        let css = ":root{--a:var(--a)} p{font-family:var(--a)}";
        assert!(run(css, &["Inter"]).is_empty());
    }

    #[test]
    fn tailwind_shaped_stylesheet() {
        let css = r#"@layer theme{:root,:host{--font-sans:"Inter", ui-sans-serif, system-ui, sans-serif;
            --font-mono:"JetBrains Mono", ui-monospace, monospace;--default-font-family:var(--font-sans);
            --default-mono-font-family:var(--font-mono)}}
            @layer base{html,:host{font-family:var(--default-font-family,ui-sans-serif,system-ui)}
            code,kbd{font-family:var(--default-mono-font-family,ui-monospace,monospace)}}
            @layer utilities{.font-mono{font-family:var(--font-mono)}@media (min-width:40rem){.sm\:font-sans{font-family:var(--font-sans)}}}
            @property --tw-x{syntax:"*";inherits:false}"#;
        assert!(run(css, &["Inter", "JetBrains Mono"]).is_empty());
        let bad = run(css, &["Inter"]);
        assert_eq!(
            bad,
            vec![
                "var(--default-mono-font-family,ui-monospace,monospace)",
                "var(--font-mono)"
            ]
        );
    }

    #[test]
    fn css_wide_keywords_and_unknown_values_are_skipped() {
        for v in [
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "INHERIT",
        ] {
            assert!(run(&format!("p{{font-family:{v}}}"), &[]).is_empty(), "{v}");
        }
        assert!(run("p{font-family:calc(1px)}", &[]).is_empty());
        assert!(run("p{font-family:'Nope' !important}", &["Nope"]).is_empty());
        assert_eq!(run("p{font-family:'Nope' !important}", &[]), vec!["'Nope'"]);
    }

    #[test]
    fn font_shorthand() {
        assert_eq!(
            run("p{font:12px/1.2 \"Nope\"}", &["Inter"]),
            vec!["12px/1.2 \"Nope\""]
        );
        assert!(run("p{font:12px/1.2 \"Nope\", Inter}", &["Inter"]).is_empty());
        assert!(run("p{font:italic small-caps bold 14px Inter}", &["Inter"]).is_empty());
        assert_eq!(run("p{font:600 1.5rem Nope}", &[]), vec!["600 1.5rem Nope"]);
        assert_eq!(run("p{font:1em Nope}", &[]), vec!["1em Nope"]);
        assert_eq!(
            run("p{font:large/normal Nope}", &[]),
            vec!["large/normal Nope"]
        );
        assert_eq!(
            run("p{font:calc(1px + 2px) Nope}", &[]),
            vec!["calc(1px + 2px) Nope"]
        );
        // Not interpretable: ignored rather than guessed at.
        assert!(run("p{font:caption}", &[]).is_empty());
        assert!(run("p{font:inherit}", &[]).is_empty());
        assert!(run("p{font:var(--f)}", &[]).is_empty());
    }

    #[test]
    fn selectors_and_unrelated_properties_are_not_declarations() {
        // `a:hover` has the shape `ident : ident`; nested rules after it must still be scanned.
        assert_eq!(
            run("a:hover{font-family:'Nope'} b{color:red}", &["Inter"]),
            vec!["'Nope'"]
        );
        assert!(run("p{content:'font-family:Nope'; color:red}", &[]).is_empty());
        assert_eq!(
            run("@media print{p{color:red;font-family:'Nope'}}", &[]),
            vec!["'Nope'"]
        );
    }

    #[test]
    fn html_style_elements_and_attributes() {
        let html = r#"<!doctype html><html><head><style>.a{font-family:'InStyle'}</style>
            <!-- <p style="font-family:'InComment'"> -->
            <script>var s = "<p style='font-family:InScript'>";</script></head>
            <body><p style="font-family:'Nope'">x</p>
            <p STYLE='font-family:&quot;Quoted&quot;, serif'>y</p>
            <p style=font-family:Bare>z</p><img src=a.png alt="style='font-family:Alt'">
            </body></html>"#;
        let scan = scan_html(html);
        let got: Vec<(String, Origin)> = scan
            .decls
            .iter()
            .map(|d| (d.raw.clone(), d.origin))
            .collect();
        assert_eq!(
            got,
            vec![
                ("'InStyle'".into(), Origin::StyleElement),
                ("'Nope'".into(), Origin::StyleAttribute),
                ("\"Quoted\", serif".into(), Origin::StyleAttribute),
                ("Bare".into(), Origin::StyleAttribute),
            ]
        );
    }

    #[test]
    fn document_declarations_see_bundled_variables() {
        let bundled = scan_css(":root{--font-mono:'JetBrains Mono'}", Origin::Stylesheet);
        let doc = scan_html(
            "<p style=\"font-family:var(--font-mono)\">x</p><b style=\"font-family:Nope\">y</b>",
        );
        let bad = check_font_families(
            &bundled,
            &doc,
            &registered(&["JetBrains Mono"]),
            &GenericMap::new(),
        );
        assert_eq!(
            bad,
            vec![UnresolvedFamily {
                font_family: "Nope".into(),
                origin: Origin::StyleAttribute
            }]
        );
    }

    #[test]
    fn output_is_in_document_order_and_deduplicated() {
        let bad = run(
            "a{font-family:'B'} b{font-family:'A'} c{font-family:'B'}",
            &[],
        );
        assert_eq!(bad, vec!["'B'", "'A'"]);
    }
}
