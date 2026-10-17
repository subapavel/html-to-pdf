//! `htmltopdf` — HTML + Tailwind CSS → PDF for invoices and business documents.
//!
//! Thin, opinionated layer over [`fulgur`] (Blitz/Stylo/Taffy/Parley + Krilla).
//! No browser, no JavaScript, deterministic output.
//!
//! The crate owns three things fulgur deliberately leaves to the caller:
//!
//! 1. **Print medium defaults** — A4, margins, `@page` handling for invoices.
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

use fulgur::{AssetBundle, Engine, Margin, PageSize};

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

fn join_unresolved(list: &[UnresolvedFamily]) -> String {
    list.iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub type Result<T> = std::result::Result<T, Error>;

/// Page geometry. Values in millimetres; converted to PDF points internally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Page {
    pub width_mm: f32,
    pub height_mm: f32,
    pub margin_mm: f32,
}

impl Page {
    pub const A4: Page = Page {
        width_mm: 210.0,
        height_mm: 297.0,
        margin_mm: 15.0,
    };
    pub const LETTER: Page = Page {
        width_mm: 215.9,
        height_mm: 279.4,
        margin_mm: 15.0,
    };

    pub fn landscape(self) -> Self {
        Page {
            width_mm: self.height_mm,
            height_mm: self.width_mm,
            ..self
        }
    }
}

impl Default for Page {
    fn default() -> Self {
        Page::A4
    }
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
        }
    }

    /// Render one HTML document to PDF bytes.
    pub fn render(&self, html: &str, opts: &RenderOptions) -> Result<Vec<u8>> {
        self.check_fonts(html)?;
        self.check_images(html)?;
        self.warn_property_limits(html);
        let engine = self.engine(opts);
        let html = self.prepare(html);
        self.run(|| engine.render(&html))
    }

    /// Render and write to `path`.
    pub fn render_to_file(
        &self,
        html: &str,
        opts: &RenderOptions,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        self.check_fonts(html)?;
        self.check_images(html)?;
        self.warn_property_limits(html);
        let engine = self.engine(opts);
        let html = self.prepare(html);
        self.run(|| engine.render_file(&html, path))
    }

    /// The document as handed to the engine: media queries evaluated for print (F7), generic font
    /// families mapped to bundled faces (F3b), gradient interpolation methods removed (F8).
    fn prepare<'a>(&self, html: &'a str) -> Cow<'a, str> {
        let html = media::print_media_in_html(html);
        let html = then(html, |h| fonts::map_generics_in_html(h, &self.generics));
        let html = then(html, |h| {
            fonts::rewrite_css_in_html(h, gradient::strip_gradient_interpolation)
        });
        then(html, |h| {
            fonts::rewrite_style_elements(h, css_text::close_open_blocks)
        })
    }

    /// Runs the engine. A panic inside it (an engine bug, e.g. on unusual CSS) becomes
    /// [`Error::EnginePanic`] instead of unwinding into the caller (F10). Each render builds its own
    /// engine, so nothing it left half-done is reused.
    fn run<T>(&self, f: impl FnOnce() -> std::result::Result<T, fulgur::Error>) -> Result<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
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

    /// Static check that every `font-family` list can be satisfied by a bundled face. Without it,
    /// text whose list matches nothing is silently dropped from the PDF.
    fn check_fonts(&self, html: &str) -> Result<()> {
        let document = fonts::scan_html(html);
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
    fn check_images(&self, html: &str) -> Result<()> {
        let mut problems = Vec::new();
        for src in fonts::img_srcs(html) {
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
            .page_size(PageSize::custom(opts.page.width_mm, opts.page.height_mm))
            .margin(Margin::uniform_mm(opts.page.margin_mm))
            // Determinism: never consult host fonts. Every font must be bundled.
            .system_fonts(false)
            .assets(self.assets.clone())
            .tagged(opts.tagged);
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
    pub fn font_file(self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        self.add_font(bytes)
    }

    pub fn font_bytes(self, bytes: Vec<u8>) -> Result<Self> {
        self.add_font(bytes)
    }

    fn add_font(mut self, bytes: Vec<u8>) -> Result<Self> {
        self.assets.add_font_bytes(bytes)?;
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
}
