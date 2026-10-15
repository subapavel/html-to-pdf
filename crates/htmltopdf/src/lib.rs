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

use std::path::{Path, PathBuf};

use fulgur::{AssetBundle, Engine, Margin, PageSize};

mod fonts;

/// Errors produced by this crate. Wraps fulgur's error so callers never
/// depend on fulgur directly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("render failed: {0}")]
    Render(#[from] fulgur::Error),
    #[error("asset `{path}`: {source}")]
    Asset {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
}

impl Renderer {
    pub fn builder() -> RendererBuilder {
        RendererBuilder {
            assets: AssetBundle::new(),
        }
    }

    /// Render one HTML document to PDF bytes.
    pub fn render(&self, html: &str, opts: &RenderOptions) -> Result<Vec<u8>> {
        let engine = self.engine(opts);
        Ok(engine.render(html)?)
    }

    /// Render and write to `path`.
    pub fn render_to_file(
        &self,
        html: &str,
        opts: &RenderOptions,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        let engine = self.engine(opts);
        Ok(engine.render_file(html, path)?)
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

pub struct RendererBuilder {
    assets: AssetBundle,
}

impl RendererBuilder {
    /// Add a stylesheet from a string. Applied in insertion order, before any
    /// `<style>` in the document itself.
    pub fn css(mut self, css: impl Into<String>) -> Self {
        self.assets.add_css(css);
        self
    }

    pub fn css_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let css = std::fs::read_to_string(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        self.assets.add_css(css);
        Ok(self)
    }

    /// Bundle a font (TTF / OTF / TTC / WOFF2). The family name comes from the
    /// font's own name table and is what `font-family` in CSS must match.
    pub fn font_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Asset {
            path: path.into(),
            source,
        })?;
        self.assets.add_font_bytes(bytes)?;
        Ok(self)
    }

    pub fn font_bytes(mut self, bytes: Vec<u8>) -> Result<Self> {
        self.assets.add_font_bytes(bytes)?;
        Ok(self)
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
        // Registered last, so faces the caller supplied take precedence over the built-in one.
        assets
            .add_font_bytes(fonts::BUILTIN_MONO.to_vec())
            .expect("built-in font is a valid TTF");
        Renderer { assets }
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
