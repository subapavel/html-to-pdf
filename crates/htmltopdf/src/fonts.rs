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

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

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

    /// Whether the engine's CSS parser reads the keyword as a generic family. Those six are resolved by the
    /// engine itself, through the map `RendererBuilder::build` hands it (F70), wherever they come from:
    /// the user-agent sheet, a `<link>`ed sheet, an attribute we leave unrewritten. `ui-*`, `math`, `emoji`
    /// and `fangsong` are plain family names to it, and only our keyword rewrite makes them resolve.
    pub(crate) fn is_engine_generic(self) -> bool {
        matches!(
            self,
            GenericFamily::Serif
                | GenericFamily::SansSerif
                | GenericFamily::Monospace
                | GenericFamily::Cursive
                | GenericFamily::Fantasy
                | GenericFamily::SystemUi
        )
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
    /// A `font-family` list on SVG `<text>` (inline `<svg>`, or an SVG used as an image).
    Svg,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Origin::Stylesheet => "bundled stylesheet",
            Origin::StyleElement => "<style> element",
            Origin::StyleAttribute => "style attribute",
            Origin::Svg => "SVG text",
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
    /// The same scan with every generic keyword that the engine does not resolve itself (see
    /// [`GenericFamily::is_engine_generic`]) turned into a family name of that spelling, for CSS whose
    /// keywords are not rewritten to bundled families.
    fn with_generics_unmapped(mut self) -> Scan {
        fn unmap(entries: &mut [Entry]) {
            for e in entries {
                match e {
                    Entry::Generic(g) if !g.is_engine_generic() => {
                        *e = Entry::Name(g.keyword().to_string())
                    }
                    Entry::Var { fallback, .. } => unmap(fallback),
                    _ => {}
                }
            }
        }
        for d in &mut self.decls {
            unmap(&mut d.entries);
        }
        for lists in self.vars.values_mut() {
            for list in lists {
                unmap(list);
            }
        }
        self
    }

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
    scan_block(&mut parser, &mut scan, origin, &mut Vec::new());
    scan
}

/// A generic keyword in a `font-family` list: its byte range in the scanned text, and which one it is.
type GenericSpan = (Range<usize>, GenericFamily);

/// Where the generic keywords are in `css` (a stylesheet or a declaration list), in source order.
fn generic_spans(css: &str) -> Vec<GenericSpan> {
    let mut spans = Vec::new();
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    scan_block(
        &mut parser,
        &mut Scan::default(),
        Origin::Stylesheet,
        &mut spans,
    );
    spans
}

fn scan_block(
    p: &mut Parser<'_, '_>,
    scan: &mut Scan,
    origin: Origin,
    spans: &mut Vec<GenericSpan>,
) {
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
                    scan_block(p, scan, origin, spans);
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
                let mut found = Vec::new();
                let entries = match kind {
                    Prop::Shorthand => parse_font_shorthand(p, &mut found),
                    _ => Some(parse_list(p, &mut found)),
                };
                // A custom property is only known to be a font list where it is used. `math`, `emoji` and
                // `fangsong` are also values of other properties (`display: math`), so there they stay.
                if matches!(kind, Prop::Custom) {
                    found.retain(|(_, g)| {
                        !matches!(
                            g,
                            GenericFamily::Math | GenericFamily::Emoji | GenericFamily::FangSong
                        )
                    });
                }
                spans.extend(found);
                // `parse_list` stops before `;` / `!important` / the end of the block.
                let raw = p.slice_from(start).trim().to_string();
                skip_to_statement_end(p, scan, origin, spans);
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

/// Skips the rest of a declaration. A `{}` block means it was a qualified rule whose selector merely
/// starts like a declaration (`font:hover { … }`): its block is scanned as rules and the statement ends.
fn skip_to_statement_end(
    p: &mut Parser<'_, '_>,
    scan: &mut Scan,
    origin: Origin,
    spans: &mut Vec<GenericSpan>,
) {
    loop {
        match p.next() {
            Ok(Token::Semicolon) | Err(_) => return,
            Ok(Token::CurlyBracketBlock) => {
                let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    scan_block(p, scan, origin, spans);
                    Ok(())
                });
                return;
            }
            Ok(_) => {}
        }
    }
}

/// Parses a comma-separated `font-family` list up to (not including) `;`, `!` or the end of the block.
/// Records the source range of every entry that is a bare generic keyword in `spans`.
fn parse_list(p: &mut Parser<'_, '_>, spans: &mut Vec<GenericSpan>) -> Vec<Entry> {
    #[derive(Default)]
    struct Cur {
        idents: Vec<String>,
        /// Source range of the first identifier, for a one-identifier entry.
        span: Option<Range<usize>>,
        single: Option<Entry>,
        other: bool,
    }
    impl Cur {
        fn flush(&mut self, out: &mut Vec<Entry>, spans: &mut Vec<GenericSpan>) {
            let c = std::mem::take(self);
            let entry = if c.other || (c.single.is_some() && !c.idents.is_empty()) {
                Entry::Other
            } else if let Some(e) = c.single {
                e
            } else if c.idents.len() == 1 {
                let id = &c.idents[0];
                if CSS_WIDE.iter().any(|k| k.eq_ignore_ascii_case(id)) {
                    Entry::CssWide
                } else if id.eq_ignore_ascii_case("-moz-fixed") {
                    // Stylo's alias of `monospace` (the engine's UA sheet writes it). The engine resolves it
                    // with the rest of the generics, so it counts as mapped; it is not rewritten, as it
                    // names the generic the engine rescales the font size for.
                    Entry::Generic(GenericFamily::Monospace)
                } else if let Some(g) = GenericFamily::from_keyword(id) {
                    if let Some(span) = c.span {
                        spans.push((span, g));
                    }
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
        // So that the state below is where the next token starts.
        p.skip_whitespace();
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
            Token::Comma => cur.flush(&mut out, spans),
            Token::Ident(id) => {
                if cur.idents.is_empty() {
                    cur.span = Some(state.position().byte_index()..p.position().byte_index());
                }
                cur.idents.push(id.to_string());
            }
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
                        parse_list(p, spans)
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
    cur.flush(&mut out, spans);
    out
}

/// `font: [style || variant || weight || stretch]? size[/line-height]? family-list`.
/// Returns `None` when the shorthand cannot be interpreted (system font keywords, `var()` before the
/// family list, CSS-wide keyword): such a declaration is ignored.
fn parse_font_shorthand(
    p: &mut Parser<'_, '_>,
    spans: &mut Vec<GenericSpan>,
) -> Option<Vec<Entry>> {
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
    Some(parse_list(p, spans))
}

/// The sites of an HTML document that the pre-render checks and rewrites care about: its `<style>` elements, its
/// `style=""` attributes and the `src` of its `<img>`s, in document order, each with whether it is inside an inline
/// `<svg>` (the `<svg>` element's own attributes included).
///
/// One walk of the text serves the font check ([`HtmlSites::scan`]), the image check ([`HtmlSites::img_srcs`]) and the
/// rewrites ([`HtmlSites::rewrite`]) of one document (F99f): each used to walk the whole text on its own. The ranges
/// are byte offsets into the text that was walked, so they are valid for that text only.
///
/// The walk is a deliberately small tag scanner, not an HTML parser; it understands comments, quoted/unquoted
/// attribute values and raw-text elements, which is what the checks need.
pub(crate) struct HtmlSites {
    sites: Vec<(HtmlSite, bool)>,
}

/// What [`HtmlSites::rewrite`] tells the rewrite about the site whose CSS it is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SiteInfo {
    /// A `style=""` value (otherwise the contents of a `<style>` element).
    pub attribute: bool,
    /// Inside an inline `<svg>`.
    pub in_svg: bool,
}

impl HtmlSites {
    pub(crate) fn walk(html: &str) -> Self {
        let mut sites = Vec::new();
        walk_html(html, |site, in_svg| sites.push((site, in_svg)));
        Self { sites }
    }

    /// Scans the CSS inside the document: `<style>` elements and `style=""` attributes, in document order.
    ///
    /// CSS inside an inline `<svg>` (its `<style>` elements and the `style=""` of the `<svg>` and its descendants) is
    /// not scanned: the SVG's text is shaped by usvg with the bundled fonts, and fulgur reports a family list there that
    /// matches nothing itself (`Origin::Svg`, F152), with the CSS generics usvg knows and the map for the rest.
    pub(crate) fn scan(&self, html: &str) -> Scan {
        let mut scan = Scan::default();
        for (site, in_svg) in &self.sites {
            match site {
                HtmlSite::Attribute(_) | HtmlSite::Element(_) if *in_svg => {}
                HtmlSite::Attribute(range) => {
                    let attr = scan_css(
                        &decode_entities(&html[range.clone()]),
                        Origin::StyleAttribute,
                    );
                    // `map_generics_in_html` leaves attributes with character references alone. The six generics
                    // the engine parses resolve through its own map anyway (F70); `ui-*` and the others would be
                    // plain names there and match nothing: check those as names.
                    scan.extend(if html[range.clone()].contains('&') {
                        attr.with_generics_unmapped()
                    } else {
                        attr
                    })
                }
                HtmlSite::Element(range) => {
                    scan.extend(scan_css(&html[range.clone()], Origin::StyleElement))
                }
                HtmlSite::ImgSrc(_) => {}
            }
        }
        scan
    }

    /// The `src` of every `<img>` that has the attribute, entity-decoded, in document order. An `<img>`
    /// without `src`, and anything inside a comment or a raw-text element (`<style>`, `<script>`,
    /// `<textarea>`, `<title>`), is not an image of the document and is not reported.
    pub(crate) fn img_srcs(&self, html: &str) -> Vec<String> {
        self.sites
            .iter()
            .filter_map(|(site, _)| match site {
                HtmlSite::ImgSrc(range) => Some(decode_entities(&html[range.clone()])),
                _ => None,
            })
            .collect()
    }

    /// `html` (the text that was walked) with `rewrite` applied to the CSS of every `<style>` element and every
    /// `style=""` value, in one pass. A `style=""` value that contains a character reference is left alone: its text
    /// is not the CSS the engine reads, and a rewrite could change what the reference decodes to.
    ///
    /// `rewrite` may chain several rewrites and decide per site which ones apply ([`SiteInfo`]). That is the same text
    /// as running the rewrites one after another over the whole document, because a rewrite only changes the CSS of a
    /// site and never what the walk sees around it (the end of a `<style>` element, a quote, a tag).
    pub(crate) fn rewrite<'a>(
        &self,
        html: &'a str,
        rewrite: impl Fn(&'a str, SiteInfo) -> Cow<'a, str>,
    ) -> Cow<'a, str> {
        let mut out = String::new();
        let mut last = 0;
        let mut changed = false;
        for (site, in_svg) in &self.sites {
            let (range, attribute) = match site {
                HtmlSite::Element(r) => (r, false),
                HtmlSite::Attribute(r) if !html[r.clone()].contains('&') => (r, true),
                HtmlSite::Attribute(_) | HtmlSite::ImgSrc(_) => continue,
            };
            let info = SiteInfo {
                attribute,
                in_svg: *in_svg,
            };
            if let Cow::Owned(css) = rewrite(&html[range.clone()], info) {
                out.push_str(&html[last..range.start]);
                out.push_str(&css);
                last = range.end;
                changed = true;
            }
        }
        if !changed {
            return Cow::Borrowed(html);
        }
        out.push_str(&html[last..]);
        Cow::Owned(out)
    }
}

/// The document's CSS scan (see [`HtmlSites::scan`]), for tests.
#[cfg(test)]
pub(crate) fn scan_html(html: &str) -> Scan {
    HtmlSites::walk(html).scan(html)
}

/// See [`HtmlSites::img_srcs`], for tests.
#[cfg(test)]
pub(crate) fn img_srcs(html: &str) -> Vec<String> {
    HtmlSites::walk(html).img_srcs(html)
}

/// `html` with [`map_generics`] applied to its `<style>` elements and `style=""` attributes (an attribute that contains
/// a character reference is left alone), except inside an inline `<svg>`: usvg does not read the CSS escapes
/// [`css_ident`] writes (`JetBrains\000020Mono` is the family `JetBrains`), and fulgur's SVG font resolver maps the
/// generics itself (F152). The renderer applies it together with its other rewrites in one pass (`Renderer::prepare`);
/// this is the single rewrite, for tests.
#[cfg(test)]
pub(crate) fn map_generics_in_html<'a>(html: &'a str, map: &GenericMap) -> Cow<'a, str> {
    HtmlSites::walk(html).rewrite(html, |css, site| {
        if site.in_svg {
            Cow::Borrowed(css)
        } else {
            map_generics(css, map)
        }
    })
}

/// What [`walk_html`] reports, in document order: CSS, and the `src` of images.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HtmlSite {
    /// The byte range of a `style=""` attribute's raw (entity-encoded) value.
    Attribute(std::ops::Range<usize>),
    /// The byte range of a `<style>` element's contents.
    Element(std::ops::Range<usize>),
    /// The byte range of the raw (entity-encoded) value of an `<img>`'s `src` attribute; empty for
    /// `src=""` and a bare `src`.
    ImgSrc(std::ops::Range<usize>),
}

/// The elements the walk treats specially.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Template,
    Noscript,
    Svg,
    Img,
    Style,
    Script,
    Textarea,
    Title,
    Other,
}

impl Tag {
    /// ASCII case-insensitive, without allocating: this runs for every tag of the document.
    fn of(name: &str) -> Tag {
        let n = name.as_bytes();
        let is = |t: &str| n.eq_ignore_ascii_case(t.as_bytes());
        match n.len() {
            3 if is("svg") => Tag::Svg,
            3 if is("img") => Tag::Img,
            5 if is("style") => Tag::Style,
            5 if is("title") => Tag::Title,
            6 if is("script") => Tag::Script,
            8 if is("template") => Tag::Template,
            8 if is("noscript") => Tag::Noscript,
            8 if is("textarea") => Tag::Textarea,
            _ => Tag::Other,
        }
    }

    /// The (lowercase) name of a raw-text element, whose contents run to its end tag.
    fn raw_text_name(self) -> Option<&'static str> {
        match self {
            Tag::Style => Some("style"),
            Tag::Script => Some("script"),
            Tag::Textarea => Some("textarea"),
            Tag::Title => Some("title"),
            _ => None,
        }
    }
}

/// The tag scanner behind [`HtmlSites`], reporting CSS and image sources in document order, each site with whether it
/// is inside an inline `<svg>` (the `<svg>` element's own attributes included).
fn walk_html(html: &str, mut found: impl FnMut(HtmlSite, bool)) {
    let bytes = html.as_bytes();
    let mut i = 0;
    // Open `<svg>` elements.
    let mut svg = 0usize;
    // Open `<template>` and `<noscript>` elements: their content is inert (it is never rendered, here
    // there is no scripting), so images in it are not images of the document. CSS in them is still
    // reported, like CSS anywhere else, because the engine's style handling does not know about them.
    let mut inert = 0usize;
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
            if is_end_tag(rest, "template") || is_end_tag(rest, "noscript") {
                inert = inert.saturating_sub(1);
            }
            if is_end_tag(rest, "svg") {
                svg = svg.saturating_sub(1);
            }
            i += rest.find('>').map_or(rest.len(), |e| e + 1);
            continue;
        }
        // Opening tag.
        let mut j = i + 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let tag = Tag::of(&html[i + 1..j]);
        if matches!(tag, Tag::Template | Tag::Noscript) {
            inert += 1;
        }
        if tag == Tag::Svg {
            svg += 1;
        }
        let mut self_closing = false;
        loop {
            while j < bytes.len() && (bytes[j].is_ascii_whitespace() || bytes[j] == b'/') {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            if bytes[j] == b'>' {
                self_closing = bytes[j - 1] == b'/';
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
            let name = &html[name_start..j];
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = j..j;
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
                    value = vs..j.min(bytes.len());
                    j = (j + 1).min(bytes.len());
                } else {
                    let vs = j;
                    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                        j += 1;
                    }
                    value = vs..j;
                }
            }
            if name.eq_ignore_ascii_case("style") {
                found(HtmlSite::Attribute(value), svg > 0);
            } else if tag == Tag::Img && inert == 0 && name.eq_ignore_ascii_case("src") {
                found(HtmlSite::ImgSrc(value), svg > 0);
            }
        }
        if tag == Tag::Svg && self_closing {
            svg -= 1;
        }
        i = j;
        if let Some(raw) = tag.raw_text_name() {
            let end = find_end_tag(&html[i..], raw);
            if tag == Tag::Style {
                found(HtmlSite::Element(i..i + end), svg > 0);
            }
            i += end;
        }
    }
}

/// Whether `rest` starts with the end tag `</tag` (ASCII case-insensitive, followed by a delimiter).
fn is_end_tag(rest: &str, tag: &str) -> bool {
    let b = rest.as_bytes();
    b.starts_with(b"</")
        && b.get(2..2 + tag.len())
            .is_some_and(|n| n.eq_ignore_ascii_case(tag.as_bytes()))
        && b.get(2 + tag.len())
            .is_none_or(|c| c.is_ascii_whitespace() || matches!(c, b'>' | b'/'))
}

/// Offset of the first `</tag` (ASCII case-insensitive) in `rest`, or `rest.len()`.
fn find_end_tag(rest: &str, tag: &str) -> usize {
    let b = rest.as_bytes();
    let mut from = 0;
    while let Some(off) = rest[from..].find("</") {
        let at = from + off;
        let name = at + 2..at + 2 + tag.len();
        if b.get(name.clone())
            .is_some_and(|n| n.eq_ignore_ascii_case(tag.as_bytes()))
        {
            return at;
        }
        from = at + 2;
    }
    rest.len()
}

// ---------------------------------------------------------------------------------------------
// Generic-family mapping
// ---------------------------------------------------------------------------------------------

/// The default generic map (F3b): `monospace` and `ui-monospace` go to the built-in face, every other
/// generic to `default_family` (the first bundled font, or the built-in face), then `overrides` apply.
pub(crate) fn generic_map(default_family: &str, overrides: &GenericMap) -> GenericMap {
    let mut map: GenericMap = GenericFamily::ALL
        .into_iter()
        .map(|g| {
            let family = match g {
                GenericFamily::Monospace | GenericFamily::UiMonospace => BUILTIN_MONO_FAMILY,
                _ => default_family,
            };
            (g, family.to_string())
        })
        .collect();
    map.extend(overrides.iter().map(|(g, f)| (*g, f.clone())));
    map
}

/// `css` (a stylesheet) with every generic keyword in a `font-family` list replaced by the family it
/// maps to. The engine runs without host fonts, so a generic keyword alone resolves to nothing and its
/// text is dropped; a bundled family name does resolve. Quoted names are names, not keywords, and stay.
pub(crate) fn map_generics<'a>(css: &'a str, map: &GenericMap) -> Cow<'a, str> {
    replace_spans(css, generic_spans(css).into_iter(), map)
}

fn replace_spans<'a>(
    text: &'a str,
    spans: impl Iterator<Item = GenericSpan>,
    map: &GenericMap,
) -> Cow<'a, str> {
    let mut out = String::new();
    let mut last = 0;
    for (r, g) in spans {
        let Some(family) = map.get(&g) else { continue };
        out.push_str(&text[last..r.start]);
        out.push_str(&css_ident(family));
        last = r.end;
    }
    if last == 0 {
        return Cow::Borrowed(text);
    }
    out.push_str(&text[last..]);
    Cow::Owned(out)
}

/// `name` as a single CSS identifier, which `font-family` reads as that family name (CSS Fonts 4 §4.2).
/// Every character outside `[A-Za-z0-9_-]` and non-ASCII, and a leading digit or hyphen, is written as a
/// six-digit hex escape: no spaces, quotes or `&`, so the result is safe in CSS and in an HTML attribute.
fn css_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        let plain = c.is_ascii_alphabetic()
            || c == '_'
            || !c.is_ascii()
            || (i > 0 && (c.is_ascii_digit() || c == '-'));
        if plain {
            out.push(c);
        } else {
            out.push_str(&format!("\\{:06x}", c as u32));
        }
    }
    out
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
pub(crate) mod tests {
    use super::*;

    #[test]
    fn img_srcs_reads_every_attribute_syntax_in_document_order() {
        let html = r#"<IMG SRC="a.png"><p>x</p><img src='b&amp;c.png' alt=x><img alt=y src=d.png>
            <img src=""><img src><img><img data-src="e.png"><img alt="src=f.png">"#;
        assert_eq!(img_srcs(html), ["a.png", "b&c.png", "d.png", "", ""]);
    }

    #[test]
    fn img_srcs_skips_inert_template_and_noscript_content() {
        let html = r#"<img src="a.png"><template><img src="b.png"><template><img src="c.png"></template><img src="d.png"></template>
            <NOSCRIPT><img src="e.png"></NOSCRIPT><img src="f.png"><noscripts><img src="g.png">"#;
        assert_eq!(img_srcs(html), ["a.png", "f.png", "g.png"]);
    }

    #[test]
    fn img_srcs_skips_comments_and_raw_text_elements() {
        let html = r#"<!-- <img src="a.png"> --><style>x{}</style><script>"<img src='b.png'>"</script>
            <textarea><img src="c.png"></textarea><title><img src="d.png"></title><img src="e.png">"#;
        assert_eq!(img_srcs(html), ["e.png"]);
    }

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

    fn map() -> GenericMap {
        generic_map("Inter", &GenericMap::new())
    }

    #[test]
    fn generic_keywords_are_replaced_by_bundled_families() {
        let m = |css: &str| map_generics(css, &map()).into_owned();
        assert_eq!(
            m("p{font-family:monospace}"),
            "p{font-family:JetBrains\\000020Mono}"
        );
        assert_eq!(
            m("p { font-family: 'Nope', Sans-Serif !important }"),
            "p { font-family: 'Nope', Inter !important }"
        );
        assert_eq!(
            m("p{font:italic 12px/1.5 serif}"),
            "p{font:italic 12px/1.5 Inter}"
        );
        assert_eq!(
            m(":root{--f:ui-monospace, monospace} p{font-family:var(--x, system-ui)}"),
            ":root{--f:JetBrains\\000020Mono, JetBrains\\000020Mono} p{font-family:var(--x, Inter)}"
        );
        assert_eq!(
            m("@media print{p{font-family:cursive}}"),
            "@media print{p{font-family:Inter}}"
        );
    }

    #[test]
    fn moz_fixed_counts_as_monospace_and_is_not_rewritten() {
        let m = map();
        let reg = registered(&["Inter", BUILTIN_MONO_FAMILY]);
        for css in [
            "p{font-family:-moz-fixed}",
            "p{font-family:'Nope', -MOZ-FIXED}",
            "p{font:12px -moz-fixed}",
        ] {
            let bad = check_font_families(
                &scan_css(css, Origin::Stylesheet),
                &Scan::default(),
                &reg,
                &m,
            );
            assert!(bad.is_empty(), "{css}: {bad:?}");
            assert_eq!(map_generics(css, &m), css, "{css}");
        }
        // Unmapped (no mono face registered): unresolved like `monospace` itself.
        let bad = check_font_families(
            &scan_css("p{font-family:-moz-fixed}", Origin::Stylesheet),
            &Scan::default(),
            &registered(&["Inter"]),
            &m,
        );
        assert_eq!(bad.len(), 1);
    }

    #[test]
    fn a_rule_whose_selector_looks_like_a_declaration_does_not_hide_later_rules() {
        assert_eq!(
            map_generics("font:hover{color:red}.x{font-family:monospace}", &map()),
            "font:hover{color:red}.x{font-family:JetBrains\\000020Mono}"
        );
    }

    #[test]
    fn generics_in_attributes_left_unmapped_are_checked_by_what_the_engine_resolves() {
        // The rewrite skips attributes with character references. The six generics the engine parses
        // still resolve, through its own map (F70); `ui-monospace` would be a plain name there, matching
        // nothing, so the check must not count it as mapped.
        let check = |html: &str| {
            check_font_families(
                &Scan::default(),
                &scan_html(html),
                &registered(&["Inter", BUILTIN_MONO_FAMILY]),
                &map(),
            )
        };
        assert!(check(r#"<p style="font-family:monospace">x</p>"#).is_empty());
        assert!(
            check(r#"<p style="font-family:monospace;content:&#39;x&#39;">x</p>"#).is_empty(),
            "monospace resolves in the engine whether or not we rewrite it"
        );
        for g in ["serif", "sans-serif", "cursive", "fantasy", "system-ui"] {
            let html = format!(r#"<p style="font-family:{g};content:&#39;x&#39;">x</p>"#);
            assert!(check(&html).is_empty(), "{g}");
        }
        assert_eq!(
            check(r#"<p style="font-family:ui-monospace;content:&#39;x&#39;">x</p>"#).len(),
            1
        );
        for g in [
            "ui-serif",
            "ui-sans-serif",
            "ui-rounded",
            "math",
            "emoji",
            "fangsong",
        ] {
            let html = format!(r#"<p style="font-family:{g};content:&#39;x&#39;">x</p>"#);
            assert_eq!(check(&html).len(), 1, "{g}");
        }
    }

    #[test]
    fn css_inside_an_inline_svg_is_neither_rewritten_nor_scanned() {
        // usvg reads `JetBrains\000020Mono` as the family `JetBrains`; fulgur's SVG resolver maps the generics and
        // reports an unmatched SVG list itself (F152).
        let html = r#"<p style="font-family:monospace">a</p><svg style="font-family:monospace"><style>text{font-family:ui-monospace}</style><text style="font-family:Georgia">b</text><svg/><text style="font-family:monospace">c</text></svg><p style="font-family:monospace">d</p><svg/><p style="font-family:monospace">e</p>"#;
        let mapped = map_generics_in_html(html, &map()).into_owned();
        assert_eq!(
            mapped.matches("JetBrains\\000020Mono").count(),
            3,
            "{mapped}"
        );
        assert_eq!(
            mapped.matches("font-family:monospace").count(),
            2,
            "{mapped}"
        );
        assert!(
            mapped.contains("text{font-family:ui-monospace}"),
            "{mapped}"
        );
        let bad = check_font_families(
            &Scan::default(),
            &scan_html(html),
            &registered(&["Inter", BUILTIN_MONO_FAMILY]),
            &map(),
        );
        assert!(bad.is_empty(), "{bad:?}");
        // CSS outside the SVG is still scanned.
        let outside = scan_html(r#"<svg></svg><p style="font-family:Georgia">x</p>"#);
        let bad = check_font_families(&Scan::default(), &outside, &registered(&["Inter"]), &map());
        assert_eq!(bad.len(), 1, "{bad:?}");
    }

    #[test]
    fn non_generic_uses_stay() {
        for css in [
            "p{font-family:'monospace', \"serif\"}",
            "p{font-family:Serif Pro}",
            "p{font-weight:bold} .monospace{color:red}",
            "p{content:'font-family: serif'}",
            ":root{--d:math; --e:emoji}",
            "p{display:math}",
        ] {
            assert_eq!(map_generics(css, &map()), css, "{css}");
        }
    }

    #[test]
    fn html_style_elements_and_attributes_are_mapped() {
        let html = r#"<style>p{font-family:serif}</style><p style="font-family: monospace">monospace</p><p style="font-family:&quot;X&quot;, serif">x</p>"#;
        assert_eq!(
            map_generics_in_html(html, &map()),
            r#"<style>p{font-family:Inter}</style><p style="font-family: JetBrains\000020Mono">monospace</p><p style="font-family:&quot;X&quot;, serif">x</p>"#
        );
    }

    #[test]
    fn css_ident_escapes_what_an_identifier_cannot_hold() {
        assert_eq!(css_ident("Inter"), "Inter");
        assert_eq!(css_ident("Noto Sans 2"), "Noto\\000020Sans\\0000202");
        assert_eq!(css_ident("1Font"), "\\000031Font");
        assert_eq!(css_ident("Šrift \"x\""), "Šrift\\000020\\000022x\\000022");
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

    /// The walk as it was before F99f (a `String` per tag and per attribute name), kept as the reference for the
    /// allocation-free one.
    fn reference_walk(html: &str) -> Vec<(HtmlSite, bool)> {
        let mut out = Vec::new();
        let bytes = html.as_bytes();
        let mut i = 0;
        let mut svg = 0usize;
        let mut inert = 0usize;
        while let Some(off) = html[i..].find('<') {
            i += off;
            let rest = &html[i..];
            if rest.starts_with("<!--") {
                i += rest.find("-->").map_or(rest.len(), |e| e + 3);
                continue;
            }
            let next = bytes.get(i + 1).copied().unwrap_or(0);
            if !next.is_ascii_alphabetic() {
                if is_end_tag(rest, "template") || is_end_tag(rest, "noscript") {
                    inert = inert.saturating_sub(1);
                }
                if is_end_tag(rest, "svg") {
                    svg = svg.saturating_sub(1);
                }
                i += rest.find('>').map_or(rest.len(), |e| e + 1);
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            let tag = html[i + 1..j].to_ascii_lowercase();
            if matches!(tag.as_str(), "template" | "noscript") {
                inert += 1;
            }
            if tag == "svg" {
                svg += 1;
            }
            let mut self_closing = false;
            loop {
                while j < bytes.len() && (bytes[j].is_ascii_whitespace() || bytes[j] == b'/') {
                    j += 1;
                }
                if j >= bytes.len() {
                    break;
                }
                if bytes[j] == b'>' {
                    self_closing = bytes[j - 1] == b'/';
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
                let mut value = j..j;
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
                        value = vs..j.min(bytes.len());
                        j = (j + 1).min(bytes.len());
                    } else {
                        let vs = j;
                        while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>'
                        {
                            j += 1;
                        }
                        value = vs..j;
                    }
                }
                if name == "style" {
                    out.push((HtmlSite::Attribute(value), svg > 0));
                } else if name == "src" && tag == "img" && inert == 0 {
                    out.push((HtmlSite::ImgSrc(value), svg > 0));
                }
            }
            if tag == "svg" && self_closing {
                svg -= 1;
            }
            i = j;
            if matches!(tag.as_str(), "style" | "script" | "textarea" | "title") {
                let end = find_end_tag(&html[i..], &tag);
                if tag == "style" {
                    out.push((HtmlSite::Element(i..i + end), svg > 0));
                }
                i += end;
            }
        }
        out
    }

    /// A document with every shape the walk distinguishes (also used by the `Renderer::prepare` test).
    pub(crate) const MIXED_HTML: &str = concat!(
        "<!DOCTYPE html><html><head><TITLE>a <style>x{}</style> <img src=t.png></TITLE>",
        "<style>@media print { .a { font-family: sans-serif } } .b { background: linear-gradient(in oklab, red, blue) }</style>",
        "<!-- <style>c{font-family:serif}</style> <img src=c.png> -->",
        "<STYLE media=all>p{font-family:monospace;color:red</STYLE>",
        "<script>var s = '<style>d{}</style><img src=s.png>';</script><textarea><img src=ta.png></textarea>",
        "</head><body>",
        "<p STYLE=\"font-family: serif\" class=x>one</p>",
        "<p style='background:conic-gradient(from 0deg in hsl, red, blue)'>two</p>",
        "<p style=font-family:ui-serif>three</p><p style=\"font-family: &quot;Inter&quot;, serif\">four</p>",
        "<p style>five</p><p style=\"\">six</p>",
        "<img src=\"a.png\"><IMG SRC='b&amp;c.png'><img alt=x><img src><img src=\"\" >",
        "<template><img src=in-template.png><p style=\"font-family:cursive\"></p></template>",
        "<noscript><img src=in-noscript.png></noscript><img src=after.png>",
        "<svg viewBox=\"0 0 1 1\" style=\"font-family: serif\"><style>text{font-family:serif}</style>",
        "<text style=\"font-family: sans-serif\">t</text><image href=x.png /><img src=svg-img.png></svg>",
        "<svg/><p style=\"font-family: system-ui\">after the self-closing svg</p>",
        "<custom-element style=\"font-family:fantasy\"></custom-element><br/><a href=x>y</a>",
        "<style>@media screen{.z{font-family:math}}</style></body></html> <",
    );

    #[test]
    fn the_allocation_free_walk_reports_what_the_reference_walk_reports() {
        let sites = |html: &str| HtmlSites::walk(html).sites;
        assert_eq!(sites(MIXED_HTML), reference_walk(MIXED_HTML));
        // The mixed document does exercise every kind of site, inside and outside an `<svg>`.
        let all = sites(MIXED_HTML);
        for (kind, in_svg) in [
            (0, false),
            (0, true),
            (1, false),
            (1, true),
            (2, false),
            (2, true),
        ] {
            assert!(
                all.iter().any(|(s, v)| *v == in_svg
                    && match s {
                        HtmlSite::Attribute(_) => kind == 0,
                        HtmlSite::Element(_) => kind == 1,
                        HtmlSite::ImgSrc(_) => kind == 2,
                    }),
                "kind {kind} in_svg {in_svg} missing"
            );
        }
        // Truncated documents (the walk must stop where the reference stops).
        for end in (0..MIXED_HTML.len()).filter(|&e| MIXED_HTML.is_char_boundary(e)) {
            assert_eq!(
                sites(&MIXED_HTML[..end]),
                reference_walk(&MIXED_HTML[..end]),
                "{end}"
            );
        }
    }

    #[test]
    fn the_walk_reports_the_same_sites_on_every_fixture_and_bench_document() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        for dir in std::fs::read_dir(root.join("fixtures/invoices")).unwrap() {
            files.push(dir.unwrap().path().join("index.html"));
        }
        for doc in ["b200-items", "b200-text", "b200-mixed"] {
            files.push(root.join(format!("bench/corpus/{doc}.html")));
        }
        let mut n = 0;
        for f in files.iter().filter(|f| f.exists()) {
            let html = std::fs::read_to_string(f).unwrap();
            assert_eq!(
                HtmlSites::walk(&html).sites,
                reference_walk(&html),
                "{}",
                f.display()
            );
            n += 1;
        }
        assert!(n >= 20, "only {n} documents found");
    }

    #[test]
    fn one_walk_serves_the_scan_the_image_sources_and_the_rewrite() {
        let sites = HtmlSites::walk(MIXED_HTML);
        let scan = sites.scan(MIXED_HTML);
        let srcs = sites.img_srcs(MIXED_HTML);
        // The same results from a fresh walk per question (what each call did before F99f).
        assert_eq!(format!("{:?}", scan_html(MIXED_HTML)), format!("{scan:?}"));
        assert_eq!(img_srcs(MIXED_HTML), srcs);
        assert_eq!(
            srcs,
            ["a.png", "b&c.png", "", "", "after.png", "svg-img.png"]
        );
        let rewritten = sites.rewrite(MIXED_HTML, |css, _| Cow::Owned(css.to_ascii_uppercase()));
        assert_eq!(
            HtmlSites::walk(&rewritten).sites.len(),
            sites.sites.len(),
            "a rewrite of the CSS does not change what the walk sees"
        );
        // A `style=""` value with a character reference is never handed to the rewrite.
        assert!(rewritten.contains("&quot;Inter&quot;, serif"));
        assert!(rewritten.contains("FONT-FAMILY: SERIF"));
    }
}
