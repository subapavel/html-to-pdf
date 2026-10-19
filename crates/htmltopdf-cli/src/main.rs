use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use htmltopdf::{Margins, Page, PageSize, RenderOptions, Renderer};

#[derive(Parser)]
#[command(
    name = "htmltopdf",
    version,
    about = "HTML + Tailwind → PDF, no browser"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Render one HTML file to PDF.
    Render {
        /// Input HTML file.
        input: PathBuf,
        /// Output PDF path.
        #[arg(short, long)]
        output: PathBuf,
        /// Stylesheet(s) to bundle, applied in order (e.g. compiled Tailwind).
        #[arg(long = "css")]
        css: Vec<PathBuf>,
        /// Font file(s) to bundle (TTF/OTF/TTC/WOFF2). No system fonts are used.
        #[arg(long = "font")]
        fonts: Vec<PathBuf>,
        /// Image(s) as name=path, referenced from HTML as <img src="name">.
        #[arg(long = "image", value_parser = parse_image)]
        images: Vec<(String, PathBuf)>,
        /// Paper size: A3, A4, A5, letter, or WIDTHxHEIGHT in millimetres (e.g. 210x99). Overrides the
        /// document's `@page { size }`; without it the document decides (A4 when it says nothing).
        #[arg(long, value_parser = parse_page_size)]
        page_size: Option<PageSize>,
        /// Page margin in millimetres on every side. Overrides the document's `@page { margin }`; without it
        /// the document decides (15 mm when it says nothing).
        #[arg(long, value_parser = parse_margin_mm)]
        margin_mm: Option<f32>,
        /// Landscape orientation: the long side of --page-size is horizontal. Without --page-size the page is
        /// A4 landscape: the flag replaces the document's `@page { size }` (an A5 document becomes A4 landscape),
        /// it does not rotate it. Use `@page { size: A5 landscape }` in the document to rotate its own size.
        #[arg(long)]
        landscape: bool,
        /// PDF title metadata.
        #[arg(long)]
        title: Option<String>,
        /// Document language (BCP 47), e.g. cs or en.
        #[arg(long)]
        lang: Option<String>,
        /// Emit a tagged PDF (structure tree).
        #[arg(long)]
        tagged: bool,
        /// Fail (exit 1) when any font-family list in the CSS or document matches no bundled font,
        /// or when no glyph run was produced for some text (it would be missing from the PDF),
        /// instead of warning. A character its font has no glyph for, and `@page` margin box text,
        /// are not checked. Use for templates you control; see RendererBuilder::strict_fonts.
        #[arg(long)]
        strict_fonts: bool,
        /// Fail (exit 1) when an <img> names no bundled image or a bundled image does not decode,
        /// instead of warning and drawing nothing. See RendererBuilder::strict_images.
        #[arg(long)]
        strict_images: bool,
    },
}

fn parse_image(s: &str) -> std::result::Result<(String, PathBuf), String> {
    let (name, path) = s
        .split_once('=')
        .ok_or_else(|| format!("expected name=path, got `{s}`"))?;
    Ok((name.to_string(), PathBuf::from(path)))
}

fn parse_page_size(s: &str) -> std::result::Result<PageSize, String> {
    match s.to_ascii_lowercase().as_str() {
        "a3" => return Ok(PageSize::A3),
        "a4" => return Ok(PageSize::A4),
        "a5" => return Ok(PageSize::A5),
        "letter" => return Ok(PageSize::LETTER),
        _ => {}
    }
    let bad =
        || format!("unknown page size `{s}`; expected A3, A4, A5, letter or WIDTHxHEIGHT in mm");
    let (w, h) = s.split_once(['x', 'X']).ok_or_else(bad)?;
    let dim = |v: &str| v.parse::<f32>().ok().filter(|v| v.is_finite() && *v > 0.0);
    match (dim(w), dim(h)) {
        (Some(width_mm), Some(height_mm)) => Ok(PageSize {
            width_mm,
            height_mm,
        }),
        _ => Err(bad()),
    }
}

fn parse_margin_mm(s: &str) -> std::result::Result<f32, String> {
    s.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| format!("margin must be a number of millimetres >= 0, got `{s}`"))
}

fn main() -> Result<()> {
    // Warnings (dropped text, unresolved fonts) must be visible without RUST_LOG.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Render {
            input,
            output,
            css,
            fonts,
            images,
            page_size,
            margin_mm,
            landscape,
            title,
            lang,
            tagged,
            strict_fonts,
            strict_images,
        } => {
            let html = std::fs::read_to_string(&input)
                .with_context(|| format!("reading {}", input.display()))?;

            let mut b = Renderer::builder()
                .strict_fonts(strict_fonts)
                .strict_images(strict_images);
            for c in &css {
                b = b.css_file(c)?;
            }
            for f in &fonts {
                b = b.font_file(f)?;
            }
            for (name, path) in &images {
                b = b.image_file(name.clone(), path)?;
            }
            let renderer = b.build();

            // Only what a flag sets overrides the document (ADR-0005); `--landscape` alone means A4 landscape.
            let size = match (page_size, landscape) {
                (Some(s), true) => Some(s.landscape()),
                (Some(s), false) => Some(s),
                (None, true) => Some(PageSize::A4.landscape()),
                (None, false) => None,
            };
            let page = Page {
                size,
                margin: margin_mm.map(Margins::uniform_mm),
            };
            let opts = RenderOptions {
                page,
                title,
                author: None,
                lang,
                tagged,
            };

            let started = std::time::Instant::now();
            renderer.render_to_file(&html, &opts, &output)?;
            log::info!("wrote {} in {:?}", output.display(), started.elapsed());
            eprintln!(
                "{} ({:.0} ms)",
                output.display(),
                started.elapsed().as_secs_f64() * 1000.0
            );
            Ok(())
        }
    }
}
