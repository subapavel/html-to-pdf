//! `htmltopdf` — HTML + Tailwind CSS → PDF for invoices and business documents.
//!
//! Thin, opinionated layer over [`fulgur`] (Blitz/Stylo/Taffy/Parley + Krilla).
//! No browser, no JavaScript, deterministic output.
//!
//! The crate owns three things fulgur deliberately leaves to the caller:
//!
//! 1. **Print medium defaults** — the document's own `@page` rules decide the page size and margins; where it says
//!    nothing the page is A4 with 15 mm margins, and what the caller sets in [`Page`] overrides the document (ADR-0005).
//! 2. **Asset bundling** — fonts and stylesheets are always explicit; nothing is
//!    read from the host system, so the same input renders byte-identically on a
//!    developer laptop, in CI and in a cloud agent session.
//! 3. **A small stable API** that our bindings (CLI today, Java/Node later) sit on,
//!    so a fulgur upgrade touches this crate only.
//!
//! ```no_run
//! use htmltopdf::{Renderer, RenderOptions};
//!
//! let pdf = Renderer::builder()
//!     .css_file("fixtures/tailwind/tailwind.css")?
//!     .font_file("fixtures/fonts/Inter-Regular.ttf")?
//!     .build()
//!     .render("<h1 class=\"text-2xl font-bold\">Faktura 2026-001</h1>", &RenderOptions::default())?;
//! std::fs::write("faktura.pdf", pdf)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fulgur::{AssetBundle, Engine, Margin};

mod css_text;
mod fonts;
mod gradient;
mod media;
mod property;

use fonts::{FamilySet, GenericMap, Scan};
pub use fonts::{GenericFamily, Origin, UnresolvedFamily};

/// Errors produced by this crate. Wraps fulgur's error so callers never
/// depend on fulgur directly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("render failed: {0}")]
    Render(#[from] fulgur::Error),
    /// No entry of these `font-family` lists matches a bundled face, so the text would be dropped from
    /// the PDF. Returned only with [`RendererBuilder::strict_fonts`]`(true)`; otherwise logged.
    #[error("font-family has no bundled face: {}", join_unresolved(.0))]
    UnresolvedFontFamilies(Vec<UnresolvedFamily>),
    /// Text for which the engine produced no glyph run, so it is left out of the PDF: `<element> "excerpt"` for
    /// each. Returned only with [`RendererBuilder::strict_fonts`]`(true)`; otherwise logged. Unlike
    /// [`Error::UnresolvedFontFamilies`] this is what actually happened to a rendered document, so it also
    /// catches families the static scan cannot see (a custom property that differs where it is used).
    /// Not covered: a character its face has no glyph for (it gets a run, drawn as `.notdef`, F44) and the text
    /// of `@page` margin boxes. The message names at most 20 entries; the vector has all of them.
    #[error("text without a font, left out of the PDF: {}", join_capped(.0))]
    TextWithoutFont(Vec<String>),
    /// Bundled font files with a face the PDF writer cannot embed, or bytes that are not a font (F178): a face without
    /// a `head` table, one with no glyph outlines, junk. Such a file is left out of the bundle whole (a collection with
    /// one bad face too); without it the text would be shaped with a face that is then dropped from the PDF, or drawn
    /// as invisible glyphs. Returned by `render` / `render_to_file` only with [`RendererBuilder::strict_fonts`]`(true)`;
    /// otherwise logged when the [`Renderer`] is built and the text uses the other bundled faces.
    #[error("bundled fonts the PDF writer cannot embed: {}", join_rejected(.0))]
    RejectedFonts(Vec<RejectedFont>),
    /// The rendering engine panicked (a bug in it, not in the document). The message is the panic's.
    #[error("the rendering engine failed: {0}")]
    EnginePanic(String),
    /// `<img>` elements that cannot be drawn: no bundled image under their `src`, or an image that does
    /// not decode. Returned only with [`RendererBuilder::strict_images`]`(true)`; otherwise logged.
    #[error("images that cannot be drawn: {}", join_images(.0))]
    Images(Vec<ImageProblem>),
    #[error("asset `{path}`: {source}")]
    Asset {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// A bundled font file that was left out because the PDF writer cannot embed one of its faces (F178).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedFont {
    /// Where it came from: the path for [`RendererBuilder::font_file`], `font_bytes #N` for
    /// [`RendererBuilder::font_bytes`], N counting every font the caller added, from 1.
    pub source: String,
    /// One entry per bad face: `face <index> (<full name>): <reason>`.
    pub faces: Vec<String>,
}

impl fmt::Display for RejectedFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "font `{}`: {}", self.source, self.faces.join(", "))
    }
}

fn join_rejected(list: &[RejectedFont]) -> String {
    list.iter()
        .map(|r| r.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// An `<img>` that cannot be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageProblem {
    /// The `src` attribute, entity-decoded; empty for `src=""`.
    pub src: String,
    pub kind: ImageProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageProblemKind {
    /// No image was registered under this name (nothing is ever fetched), or the `src` is empty.
    Unregistered,
    /// The image is registered but its data is not a PNG, JPEG, GIF or SVG that decodes (or is over
    /// the pixel limit). The reason is the decoder's.
    Undecodable(String),
    /// A raster inside the SVG named by `src` (a bundled SVG, or an inline `<svg>`) does not decode or is over the
    /// pixel limit, so it is left out of the SVG (F177). `image` names the raster (`data:image/png (N bytes)` or its
    /// href), `why` is the decoder's reason.
    InSvg { image: String, why: String },
    /// An `<image href>` inside the SVG named by `src` that is not a `data:` URL and is not loaded (F177b): in an inline
    /// `<svg>` it is no bundled image (`href` is looked up by name, like `<img src>`; the filesystem is never read), and
    /// an SVG used as an image (`<img>`, `background-image`, `list-style-image`) loads no external file at all.
    UnregisteredInSvg { image: String, why: String },
}

impl fmt::Display for ImageProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ImageProblemKind::Unregistered if self.src.is_empty() => {
                write!(f, "<img src=\"\"> has an empty src")
            }
            ImageProblemKind::Unregistered => {
                write!(f, "<img src=\"{}\"> is not a bundled image", self.src)
            }
            ImageProblemKind::Undecodable(why) => {
                write!(f, "<img src=\"{}\"> cannot be decoded: {why}", self.src)
            }
            ImageProblemKind::InSvg { image, why } => {
                write!(f, "{} in {} cannot be decoded: {why}", image, self.src)
            }
            ImageProblemKind::UnregisteredInSvg { image, why } => {
                write!(f, "{} in {} is not loaded: {why}", image, self.src)
            }
        }
    }
}

fn join_images(list: &[ImageProblem]) -> String {
    list.iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// `Ok(Some(why))`: drawn although damaged (a JPEG); `Err(why)`: not drawn.
type ImageVerdict = std::result::Result<Option<String>, String>;

/// Entries of a list-valued error named in its message; a document can have thousands.
const MAX_IN_MESSAGE: usize = 20;

fn join_capped(list: &[String]) -> String {
    let mut out = list
        .iter()
        .take(MAX_IN_MESSAGE)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("; ");
    if list.len() > MAX_IN_MESSAGE {
        out.push_str(&format!("; ... and {} more", list.len() - MAX_IN_MESSAGE));
    }
    out
}

fn join_unresolved(list: &[UnresolvedFamily]) -> String {
    list.iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub type Result<T> = std::result::Result<T, Error>;

/// A paper size in millimetres. Converted to PDF points inside fulgur, never here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub width_mm: f32,
    pub height_mm: f32,
}

impl PageSize {
    pub const A3: PageSize = PageSize {
        width_mm: 297.0,
        height_mm: 420.0,
    };
    pub const A4: PageSize = PageSize {
        width_mm: 210.0,
        height_mm: 297.0,
    };
    pub const A5: PageSize = PageSize {
        width_mm: 148.0,
        height_mm: 210.0,
    };
    pub const LETTER: PageSize = PageSize {
        width_mm: 215.9,
        height_mm: 279.4,
    };

    /// The same paper with its long side horizontal (css-page-3 `landscape`). Idempotent: a page that is
    /// already wider than it is tall is returned unchanged.
    pub fn landscape(self) -> Self {
        PageSize {
            width_mm: self.width_mm.max(self.height_mm),
            height_mm: self.width_mm.min(self.height_mm),
        }
    }
}

/// The four page margins in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top_mm: f32,
    pub right_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
}

impl Margins {
    pub const fn uniform_mm(mm: f32) -> Self {
        Margins {
            top_mm: mm,
            right_mm: mm,
            bottom_mm: mm,
            left_mm: mm,
        }
    }
}

/// The margin of a page whose document does not say: 15 mm on every side (ADR-0005).
pub const DEFAULT_MARGIN_MM: f32 = 15.0;

/// What the caller wants the page to be. A `None` field leaves that property to the document.
///
/// Precedence, per property (size, and each margin side) (ADR-0005, css-page-3 "Page context"):
///
/// 1. what the caller set here;
/// 2. the document's `@page :first` / `:left` / `:right` rules;
/// 3. the document's plain `@page` rule;
/// 4. the default: A4 portrait and [`DEFAULT_MARGIN_MM`].
///
/// A caller's size is final, orientation included (`@page { size: A4 landscape }` does not turn a forced portrait
/// size); use [`PageSize::landscape`] for a landscape one. Like the print dialog in a browser, setting a value here
/// overrides the document's own choice for it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Page {
    pub size: Option<PageSize>,
    pub margin: Option<Margins>,
}

/// Per-document options. Everything that varies between two renders of the
/// same template lives here; everything shared (fonts, stylesheet) lives in
/// the [`Renderer`].
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub page: Page,
    pub title: Option<String>,
    pub author: Option<String>,
    /// BCP 47 language tag, e.g. `cs`, `en`. Required for PDF/UA.
    pub lang: Option<String>,
    /// Emit a tagged PDF (structure tree). Needed for PDF/UA and helps
    /// screen readers; costs a little output size.
    pub tagged: bool,
}

/// Shared rendering state: fonts and stylesheets loaded once, reused for
/// every document. Build one per template set and keep it alive.
pub struct Renderer {
    assets: AssetBundle,
    /// `font-family` declarations of the bundled CSS, scanned once in `build()`.
    css_scan: Scan,
    registered: FamilySet,
    generics: GenericMap,
    strict_fonts: bool,
    /// Font files that were left out of the bundle because the PDF writer cannot embed a face of them (F178).
    rejected_fonts: Vec<RejectedFont>,
    /// What decoding each bundled image showed, by the name it was looked up under. Filled lazily by
    /// `check_images`, so only images a document refers to are ever decoded, each once per `Renderer`;
    /// the verdict is a pure function of the bytes, so the order documents arrive in cannot matter.
    image_verdicts: Mutex<BTreeMap<String, ImageVerdict>>,
    strict_images: bool,
    /// Whether a generated `@property` defaults layer was added to the bundled CSS.
    property_defaults: bool,
}

impl Renderer {
    pub fn builder() -> RendererBuilder {
        RendererBuilder {
            assets: AssetBundle::new(),
            css_scan: Scan::default(),
            properties: property::Registry::default(),
            registered: FamilySet::new(),
            default_family: None,
            generic_overrides: GenericMap::new(),
            strict_fonts: false,
            strict_images: false,
            fonts_added: 0,
            rejected_fonts: Vec::new(),
        }
    }

    /// Render one HTML document to PDF bytes.
    pub fn render(&self, html: &str, opts: &RenderOptions) -> Result<Vec<u8>> {
        self.check_rejected_fonts()?;
        let sites = fonts::HtmlSites::walk(html);
        self.check_fonts(html, &sites)?;
        self.check_images(html, &sites)?;
        self.warn_property_limits(html);
        let engine = self.engine(opts);
        let html = self.prepare(html, &sites);
        self.run(|| engine.render(&html))
    }

    /// Render and write to `path`.
    pub fn render_to_file(
        &self,
        html: &str,
        opts: &RenderOptions,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        self.check_rejected_fonts()?;
        let sites = fonts::HtmlSites::walk(html);
        self.check_fonts(html, &sites)?;
        self.check_images(html, &sites)?;
        self.warn_property_limits(html);
        let engine = self.engine(opts);
        let html = self.prepare(html, &sites);
        self.run(|| engine.render_file(&html, path))
    }

    /// The document as handed to the engine: media queries evaluated for print (F7), generic font
    /// families mapped to bundled faces (F3b), gradient interpolation methods removed (F8), open blocks
    /// of `<style>` elements closed. `sites` is the walk of `html` (F99f).
    ///
    /// The rewrites are applied one after another to each CSS site and the document is rebuilt once. Where each
    /// applies: the media swap and the block closing are for `<style>` elements only; the generic mapping is not
    /// for inline `<svg>` (usvg does not read the CSS escapes it writes, and fulgur's SVG font resolver maps the
    /// generics itself, F152); the gradient rewrite is for every site. A `style=""` value with a character reference
    /// is never rewritten (`HtmlSites::rewrite`).
    ///
    /// Removing text from a `<style>` element can join a `<` and a `/style` into an end tag that was not there
    /// (`to right<in oklch/style>` → `to right</style>`), so the element ends earlier for every later rewrite and for
    /// the engine. A document where that happens is rewritten pass by pass instead, each pass over a new walk
    /// ([`Self::prepare_in_passes`]), which is the text the rewrites gave before they were fused.
    fn prepare<'a>(&self, html: &'a str, sites: &fonts::HtmlSites) -> Cow<'a, str> {
        let new_end_tag = std::cell::Cell::new(false);
        let out = sites.rewrite(html, |css, site| {
            let css = self.rewrite_site(css, site, Pass::ALL);
            if !site.attribute && fonts::contains_end_tag(&css, "style") {
                new_end_tag.set(true);
            }
            css
        });
        if new_end_tag.get() {
            Cow::Owned(self.prepare_in_passes(html))
        } else {
            out
        }
    }

    /// The rewrites of [`Self::prepare`] as separate passes over the whole document, each with its own walk.
    fn prepare_in_passes(&self, html: &str) -> String {
        let mut html = html.to_string();
        for pass in [Pass::MEDIA, Pass::GENERICS, Pass::GRADIENT, Pass::CLOSE] {
            html = fonts::HtmlSites::walk(&html)
                .rewrite(&html, |css, site| self.rewrite_site(css, site, pass))
                .into_owned();
        }
        html
    }

    /// `css` (the text of `site`) with the rewrites of `passes` that apply to the site, in [`Self::prepare`]'s order.
    fn rewrite_site<'a>(&self, css: &'a str, site: fonts::SiteInfo, passes: Pass) -> Cow<'a, str> {
        let mut css = Cow::Borrowed(css);
        if passes.has(Pass::MEDIA) && !site.attribute {
            css = then(css, media::print_media);
        }
        if passes.has(Pass::GENERICS) && !site.in_svg {
            css = then(css, |c| fonts::map_generics(c, &self.generics));
        }
        if passes.has(Pass::GRADIENT) {
            css = then(css, gradient::strip_gradient_interpolation);
        }
        if passes.has(Pass::CLOSE) && !site.attribute {
            css = then(css, css_text::close_open_blocks);
        }
        css
    }

    /// Runs the engine. A panic inside it (an engine bug, e.g. on unusual CSS) becomes
    /// [`Error::EnginePanic`] instead of unwinding into the caller (F10). Each render builds its own
    /// engine, so nothing it left half-done is reused.
    fn run<T>(&self, f: impl FnOnce() -> std::result::Result<T, fulgur::Error>) -> Result<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(Err(fulgur::Error::TextWithoutFont(found))) => Err(Error::TextWithoutFont(
                found.iter().map(|d| d.to_string()).collect(),
            )),
            // F152: SVG text whose `font-family` list matches no bundled face. fulgur draws it in the serif mapping;
            // strict mode refuses it exactly as it refuses an HTML list with no bundled or mapped entry.
            Ok(Err(fulgur::Error::SvgFontFallback(found))) => Err(Error::UnresolvedFontFamilies(
                found
                    .into_iter()
                    .map(|f| UnresolvedFamily {
                        font_family: f.font_family,
                        origin: Origin::Svg,
                    })
                    .collect(),
            )),
            // F177: rasters inside an SVG that do not decode, found while the document is built (strict only).
            Ok(Err(fulgur::Error::SvgImages(dropped))) => Err(Error::Images(
                dropped
                    .into_iter()
                    .map(|d| ImageProblem {
                        src: d.svg,
                        kind: match d.cause {
                            fulgur::SvgImageCause::Undecodable => ImageProblemKind::InSvg {
                                image: d.image,
                                why: d.reason,
                            },
                            // F177b: not bundled (inline `<svg>`) or external (an SVG used as an image).
                            fulgur::SvgImageCause::NotBundled | fulgur::SvgImageCause::External => {
                                ImageProblemKind::UnregisteredInSvg {
                                    image: d.image,
                                    why: d.reason,
                                }
                            }
                        },
                    })
                    .collect(),
            )),
            Ok(r) => Ok(r?),
            Err(payload) => Err(Error::EnginePanic(
                payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".into()),
            )),
        }
    }

    /// `@property` initial values are applied only for bundled stylesheets (see `property`). fulgur adds
    /// the bundled CSS *after* the document's `<style>` elements, so (1) a defaults layer for the
    /// document's own rules would outrank the document's layers, and (2) layers the document declares
    /// rank below our defaults layer (layer order is first appearance). Both are logged, not fixed.
    fn warn_property_limits(&self, html: &str) {
        if property::mentions_at_rule(html, "property", |c| c == '-') {
            log::warn!(
                "the document's <style> contains `@property`; initial values are applied only for \
                 stylesheets passed to `RendererBuilder::css` (CLI `--css`), so these may render without \
                 their defaults"
            );
        }
        if self.property_defaults
            && property::mentions_at_rule(html, "layer", |c| !c.is_whitespace())
        {
            log::warn!(
                "the document's <style> declares `@layer`; its layers rank below the bundled `@property` \
                 defaults, so its custom-property declarations inside layers may lose to those defaults"
            );
        }
    }

    /// With `strict_fonts`, refuses a document when a font file was left out of the bundle because the PDF writer
    /// cannot embed one of its faces (F178). Without it the build already logged each file. Checked before the
    /// `font-family` lists: those are matched against the faces that remain, and the file in question is the cause.
    fn check_rejected_fonts(&self) -> Result<()> {
        if self.strict_fonts && !self.rejected_fonts.is_empty() {
            return Err(Error::RejectedFonts(self.rejected_fonts.clone()));
        }
        Ok(())
    }

    /// Static check that every `font-family` list can be satisfied by a bundled face. Without it,
    /// text whose list matches nothing is silently dropped from the PDF.
    fn check_fonts(&self, html: &str, sites: &fonts::HtmlSites) -> Result<()> {
        let document = sites.scan(html);
        let unresolved =
            fonts::check_font_families(&self.css_scan, &document, &self.registered, &self.generics);
        if unresolved.is_empty() {
            return Ok(());
        }
        if self.strict_fonts {
            return Err(Error::UnresolvedFontFamilies(unresolved));
        }
        for u in &unresolved {
            log::warn!("font-family has no bundled face, its text will be dropped: {u}");
        }
        Ok(())
    }

    /// Static check that every `<img src>` names a bundled image that decodes. Nothing is ever fetched,
    /// so an unregistered name is a missing logo, and an image that does not decode is not drawn (its
    /// box stays); without this nobody would learn of either. `src` is looked up under the name the
    /// image was registered with, after dropping a leading `./`, as the engine does. Only the images the
    /// document refers to are decoded, once per `Renderer`.
    ///
    /// A JPEG that is damaged but decodes (truncated, no end marker, stray header bytes) is embedded as
    /// Chrome draws it, and logged; it is not an error even in strict mode.
    fn check_images(&self, html: &str, sites: &fonts::HtmlSites) -> Result<()> {
        let mut problems = Vec::new();
        for src in sites.img_srcs(html) {
            let key = src.strip_prefix("./").unwrap_or(&src);
            let Some(data) = self.assets.get_image(&src).filter(|_| !src.is_empty()) else {
                problems.push(ImageProblem {
                    src,
                    kind: ImageProblemKind::Unregistered,
                });
                continue;
            };
            match self.image_verdict(key, data) {
                Ok(None) => {}
                Ok(Some(why)) => {
                    log::warn!("<img src=\"{src}\"> is embedded although damaged: {why}")
                }
                Err(why) => problems.push(ImageProblem {
                    src,
                    kind: ImageProblemKind::Undecodable(why),
                }),
            }
        }
        if problems.is_empty() {
            return Ok(());
        }
        if self.strict_images {
            return Err(Error::Images(problems));
        }
        for p in &problems {
            log::warn!("{p}, so nothing is drawn for it");
        }
        Ok(())
    }

    fn image_verdict(&self, key: &str, data: &[u8]) -> ImageVerdict {
        let lock = || {
            self.image_verdicts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        };
        if let Some(known) = lock().get(key) {
            return known.clone();
        }
        // Decoded outside the lock: another thread may do the same, with the same result.
        let verdict = fulgur::image::validate_detailed(data)
            .map(|v| v.damaged)
            .map_err(|e| e.to_string());
        lock().entry(key.to_string()).or_insert(verdict).clone()
    }

    fn engine(&self, opts: &RenderOptions) -> Engine {
        let mut b = Engine::builder()
            // Determinism: never consult host fonts. Every font must be bundled.
            .system_fonts(false)
            .assets(self.assets.clone())
            // Text no face covers is what the static check cannot see (F70): an error exactly when
            // `strict_fonts` makes the static findings errors.
            .strict_text(self.strict_fonts)
            // A raster inside an SVG that does not decode is left out with a warning, or refused (F177).
            .strict_images(self.strict_images)
            .tagged(opts.tagged);
        // Only what the caller set overrides the document. With no size fulgur's default (A4 portrait, the page
        // box of `@page { size: A4 }`) stands unless an `@page` rule says otherwise.
        if let Some(s) = opts.page.size {
            b = b.page_size(fulgur::PageSize::custom(s.width_mm, s.height_mm));
        }
        b = match opts.page.margin {
            Some(m) => b.margin(Margin::sides_mm(
                m.top_mm,
                m.right_mm,
                m.bottom_mm,
                m.left_mm,
            )),
            // The product's default, below every `@page` margin of the document, side by side.
            None => b.default_margin(Margin::uniform_mm(DEFAULT_MARGIN_MM)),
        };
        if let Some(t) = &opts.title {
            b = b.title(t.clone());
        }
        if let Some(a) = &opts.author {
            b = b.author(a.clone());
        }
        if let Some(l) = &opts.lang {
            b = b.lang(l.clone());
        }
        b.build()
    }
}

/// Applies a borrowing rewrite to text that may already be owned, keeping it borrowed while nothing
/// changed.
fn then<'a>(text: Cow<'a, str>, rewrite: impl Fn(&str) -> Cow<'_, str>) -> Cow<'a, str> {
    match text {
        Cow::Borrowed(t) => rewrite(t),
        Cow::Owned(t) => match rewrite(&t) {
            Cow::Borrowed(_) => Cow::Owned(t),
            Cow::Owned(r) => Cow::Owned(r),
        },
    }
}

/// A set of the rewrites [`Renderer::prepare`] applies.
#[derive(Clone, Copy)]
struct Pass(u8);

impl Pass {
    const MEDIA: Pass = Pass(1);
    const GENERICS: Pass = Pass(2);
    const GRADIENT: Pass = Pass(4);
    const CLOSE: Pass = Pass(8);
    const ALL: Pass = Pass(15);

    fn has(self, pass: Pass) -> bool {
        self.0 & pass.0 != 0
    }
}

pub struct RendererBuilder {
    assets: AssetBundle,
    css_scan: Scan,
    /// `@property` registrations of the bundled CSS, emulated by a generated defaults stylesheet.
    properties: property::Registry,
    registered: FamilySet,
    /// Family name of the first font the caller bundled; becomes the document default.
    default_family: Option<String>,
    /// Generic families the caller mapped explicitly (`generic_family`).
    generic_overrides: GenericMap,
    strict_fonts: bool,
    strict_images: bool,
    /// How many fonts the caller added so far (`font_file` and `font_bytes`), for `font_bytes #N`.
    fonts_added: usize,
    /// Font files `add_font` left out (F178), in the order they were added.
    rejected_fonts: Vec<RejectedFont>,
}

impl RendererBuilder {
    /// Add a stylesheet from a string. Applied in insertion order, before any
    /// `<style>` in the document itself.
    pub fn css(mut self, css: impl Into<String>) -> Self {
        let css = css.into();
        self.css_scan
            .extend(fonts::scan_css(&css, Origin::Stylesheet));
        self.properties.extend(property::scan_property_rules(&css));
        let css = media::print_media(&css);
        let css = gradient::strip_gradient_interpolation(&css);
        self.assets
            .add_css(css_text::close_open_blocks(&css).into_owned());
        self
    }

    pub fn css_file(self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let css = std::fs::read_to_string(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        Ok(self.css(css))
    }

    /// Bundle a font (TTF / OTF / TTC / WOFF2). The family name comes from the
    /// font's own name table and is what `font-family` in CSS must match.
    ///
    /// A file whose faces the PDF writer cannot all embed (no `head` table, no glyph outlines, a collection with one
    /// such face, bytes that are not a font) is left out whole, with a warning when the [`Renderer`] is built; with
    /// [`strict_fonts`](Self::strict_fonts) rendering fails with [`Error::RejectedFonts`] instead. Its text uses the
    /// other bundled faces. A file that cannot be read, a WOFF1 file and a WOFF2 file that does not decode are still
    /// errors here.
    pub fn font_file(self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        self.add_font(bytes, Some(path.display().to_string()))
    }

    /// Bundle a font from memory; see [`font_file`](Self::font_file).
    pub fn font_bytes(self, bytes: Vec<u8>) -> Result<Self> {
        self.add_font(bytes, None)
    }

    fn add_font(mut self, bytes: Vec<u8>, source: Option<String>) -> Result<Self> {
        self.fonts_added += 1;
        match self.assets.add_font_bytes(bytes) {
            Ok(()) => {}
            // F178: left out whole, so its families are neither registered nor the document's default.
            Err(fulgur::Error::FontRejected(rejection)) => {
                self.rejected_fonts.push(RejectedFont {
                    source: source.unwrap_or_else(|| format!("font_bytes #{}", self.fonts_added)),
                    faces: rejection.faces.iter().map(|f| f.to_string()).collect(),
                });
                return Ok(self);
            }
            Err(e) => return Err(e.into()),
        }
        // fulgur may have decoded the bytes (WOFF2); read the family names from what it kept.
        if let Some(stored) = self.assets.fonts.last() {
            for name in fonts::family_names(stored) {
                if self.default_family.is_none() {
                    self.default_family = Some(name.clone());
                }
                self.registered.insert(fonts::fold(&name));
            }
        }
        Ok(self)
    }

    /// Whether a `font-family` list that matches no bundled face is an error, or only logged with
    /// `log::warn!` (the default). The text of such a list is dropped from the PDF either way.
    ///
    /// The check is a *static* scan of every declaration in the bundled CSS and the document, not a
    /// cascade: an unused rule (`.never-used { font-family: Georgia, serif }`) or a `@media screen`
    /// block trips it too. That is why strict mode is opt-in — use it for fixtures, tests and CI,
    /// where every declaration is under your control, not for arbitrary customer templates.
    ///
    /// The same switch makes *text for which no glyph run was produced* an error ([`Error::TextWithoutFont`]):
    /// after layout the engine knows which pieces of text got none, whatever the cascade made of the
    /// `font-family` lists, and says so (logged without strict mode). That catches what the static scan cannot
    /// see, such as a custom property that names an unbundled family where it is used and a bundled one
    /// elsewhere. Not covered: a character the chosen face has no glyph for (a run is produced and the glyph
    /// is `.notdef`, F44) and the text of `@page` margin boxes.
    ///
    /// It also refuses a bundled font file that the PDF writer cannot embed ([`Error::RejectedFonts`], F178), which
    /// is otherwise left out of the bundle with a warning when the [`Renderer`] is built. That is checked when a
    /// document is rendered, so the switch can be set before or after the fonts are added.
    pub fn strict_fonts(mut self, strict: bool) -> Self {
        self.strict_fonts = strict;
        self
    }

    /// Whether an `<img>` that cannot be drawn is an error, or only logged with `log::warn!` (the
    /// default). That is an `<img src>` naming no bundled image (an empty `src` too), and a bundled image
    /// that is not a PNG, JPEG, GIF or SVG that decodes in full. Either way the image is not drawn; an
    /// undecodable one keeps its layout box. Independent of [`strict_fonts`](Self::strict_fonts).
    ///
    /// Like the font check this is a static scan of the document's `<img>` tags, not of the page: an
    /// `<img>` that CSS hides is still checked, and `<img>` without `src`, inside comments or raw-text
    /// elements are not. CSS `url()` images are not checked.
    pub fn strict_images(mut self, strict: bool) -> Self {
        self.strict_images = strict;
        self
    }

    /// Map a generic `font-family` keyword (`monospace`, `serif`, …) to a bundled family. Without this,
    /// `monospace` and `ui-monospace` map to the built-in mono face and every other generic to the first
    /// bundled font. Fonts are never read from the host, so a generic keyword has no face of its own.
    pub fn generic_family(mut self, generic: GenericFamily, family: impl Into<String>) -> Self {
        self.generic_overrides.insert(generic, family.into());
        self
    }

    /// Register an image under `name`, referenced from HTML as `<img src="name">`.
    pub fn image(mut self, name: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.assets.add_image(name, bytes);
        self
    }

    pub fn image_file(mut self, name: impl Into<String>, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        self.assets.add_image(name, bytes);
        Ok(self)
    }

    pub fn build(self) -> Renderer {
        // F178: say once per file what was left out. With `strict_fonts` the render refuses instead.
        if !self.strict_fonts {
            let fallback = match &self.default_family {
                Some(_) => "the other bundled faces",
                None => "the built-in face (JetBrains Mono), no other font is bundled",
            };
            for r in &self.rejected_fonts {
                log::warn!(
                    "font `{}` is left out: {}; its text uses {fallback}",
                    r.source,
                    r.faces.join(", ")
                );
            }
        }
        let mut assets = self.assets;
        let mut registered = self.registered;
        // The engine has no user-agent default `font-family`: text in an element with no family
        // anywhere in its ancestry is dropped. Give the document one — the first bundled font, then
        // the built-in mono face — at the lowest possible priority (an `html` rule in the first
        // stylesheet; any author declaration on html/body/… overrides it through the cascade).
        let default_family = self
            .default_family
            .as_deref()
            .unwrap_or(fonts::BUILTIN_MONO_FAMILY);
        // Generic keywords resolve to nothing without host fonts; name the bundled family instead (F3b).
        // Done before the preludes below are added: they contain no generic keywords.
        let generics = fonts::generic_map(default_family, &self.generic_overrides);
        for css in assets.css.iter_mut() {
            if let Cow::Owned(mapped) = fonts::map_generics(css, &generics) {
                *css = mapped;
            }
        }
        // The rewrite above reaches only CSS that passes through us. The engine's own user-agent sheet gives
        // `pre`, `code`, `kbd`, `samp`, `tt` (and `xmp`, `listing`, `plaintext`) `font-family: monospace`, and
        // `<link>`ed sheets and attributes with character references are not rewritten either, so the engine
        // resolves the generics itself from the same map (F70).
        for (generic, family) in &generics {
            assets.set_generic_family(generic.keyword(), family.as_str());
        }
        assets.css.insert(
            0,
            format!(
                "html {{ font-family: \"{}\", \"{}\"; }}\n",
                default_family.replace('"', ""),
                fonts::BUILTIN_MONO_FAMILY
            ),
        );
        // `@property` initial values, which the engine ignores. First in the bundled CSS, so its layer is
        // declared before any other layer there and every author rule overrides it.
        let defaults = self.properties.prelude();
        let property_defaults = defaults.is_some();
        if let Some(defaults) = defaults {
            assets.css.insert(0, defaults);
        }
        // Registered last, so faces the caller supplied take precedence over the built-in one. If the
        // caller already supplied a family of the same name, theirs is used alone.
        if registered.insert(fonts::fold(fonts::BUILTIN_MONO_FAMILY)) {
            assets
                .add_font_bytes(fonts::BUILTIN_MONO.to_vec())
                .expect("built-in font is a valid TTF");
        }
        Renderer {
            assets,
            css_scan: self.css_scan,
            registered,
            generics,
            strict_fonts: self.strict_fonts,
            rejected_fonts: self.rejected_fonts,
            image_verdicts: Mutex::default(),
            strict_images: self.strict_images,
            property_defaults,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_minimal_document() {
        let r = Renderer::builder().build();
        let pdf = r
            .render("<p>hello</p>", &RenderOptions::default())
            .expect("render");
        assert!(pdf.starts_with(b"%PDF-"), "output is not a PDF");
    }

    #[test]
    fn text_without_font_message_is_capped() {
        let many: Vec<String> = (0..5000).map(|i| format!("<p> \"t{i}\"")).collect();
        let msg = Error::TextWithoutFont(many).to_string();
        assert!(msg.contains("t19\"") && !msg.contains("t20\""), "{msg}");
        assert!(msg.ends_with("; ... and 4980 more"), "{msg}");
        assert!(msg.len() < 1000, "{} bytes", msg.len());
    }

    #[test]
    fn same_input_same_bytes() {
        let r = Renderer::builder().build();
        let a = r
            .render("<p>determinism</p>", &RenderOptions::default())
            .unwrap();
        let b = r
            .render("<p>determinism</p>", &RenderOptions::default())
            .unwrap();
        assert_eq!(a, b, "render is not deterministic");
    }

    /// The rewrites `prepare` used to run one after another, each over the whole document with its own walk (F99f).
    fn prepare_in_separate_passes(r: &Renderer, html: &str) -> String {
        use crate::fonts::HtmlSites;
        let step = |html: &str, f: &dyn Fn(&str, fonts::SiteInfo) -> Cow<'_, str>| {
            HtmlSites::walk(html).rewrite(html, f).into_owned()
        };
        let html = step(html, &|css, s| {
            if s.attribute {
                Cow::Borrowed(css)
            } else {
                media::print_media(css)
            }
        });
        let html = step(&html, &|css, s| {
            if s.in_svg {
                Cow::Borrowed(css)
            } else {
                fonts::map_generics(css, &r.generics)
            }
        });
        let html = step(&html, &|css, _| gradient::strip_gradient_interpolation(css));
        step(&html, &|css, s| {
            if s.attribute {
                Cow::Borrowed(css)
            } else {
                css_text::close_open_blocks(css)
            }
        })
    }

    #[test]
    fn prepare_in_one_pass_gives_the_text_of_the_separate_rewrites() {
        let r = Renderer::builder().build();
        let sites = fonts::HtmlSites::walk(fonts::tests::MIXED_HTML);
        let one = r.prepare(fonts::tests::MIXED_HTML, &sites);
        let separate = prepare_in_separate_passes(&r, fonts::tests::MIXED_HTML);
        assert_eq!(one, separate);
        // Every rewrite did something to this document, so the comparison is not vacuous.
        assert_ne!(one, fonts::tests::MIXED_HTML);
        assert!(one.contains("@media screen { .a {"), "media swap");
        assert!(one.contains("linear-gradient(red, blue)"), "gradient");
        assert!(one.contains("color:red}</STYLE>"), "open block");
        assert!(!one.contains("font-family: sans-serif }"), "generic");
        // A document with nothing to rewrite stays borrowed.
        let plain = "<style>p{color:red}</style><p style=\"color:blue\">x</p>";
        assert!(matches!(
            r.prepare(plain, &fonts::HtmlSites::walk(plain)),
            Cow::Borrowed(_)
        ));
    }

    /// The gradient rewrite removes `in oklch` and joins `<` and `/style` into an end tag: the element ends there, the
    /// rest is HTML, and the block closing sees only the text before it (vrt-validator, F99f).
    #[test]
    fn prepare_rewrites_pass_by_pass_where_a_rewrite_ends_a_style_element() {
        let r = Renderer::builder().build();
        for html in [
            "<style>a{background:linear-gradient(to right<in oklch/style><p>X</p></style>",
            "<style>a{background:linear-gradient(to right<in oklch/style><img src=zz.png>\
             <p style=\"font-family:sans-serif\">M</p>{}</style><p>Ahoj</p>",
            "<style>@media screen{x}a{background:linear-gradient(to right<in oklch/STYLE >\
             <style>b{color:red</style>",
        ] {
            let one = r.prepare(html, &fonts::HtmlSites::walk(html));
            assert_eq!(one, prepare_in_separate_passes(&r, html), "{html}");
            assert!(one.contains("to right)}</"), "{one}");
        }
    }
}
