use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use htmltopdf::{Page, RenderOptions, Renderer};

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
        /// Page margin in millimetres.
        #[arg(long, default_value_t = 15.0)]
        margin_mm: f32,
        /// Landscape orientation.
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
        /// instead of warning. Use for templates you control; see RendererBuilder::strict_fonts.
        #[arg(long)]
        strict_fonts: bool,
    },
}

fn parse_image(s: &str) -> std::result::Result<(String, PathBuf), String> {
    let (name, path) = s
        .split_once('=')
        .ok_or_else(|| format!("expected name=path, got `{s}`"))?;
    Ok((name.to_string(), PathBuf::from(path)))
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
            margin_mm,
            landscape,
            title,
            lang,
            tagged,
            strict_fonts,
        } => {
            let html = std::fs::read_to_string(&input)
                .with_context(|| format!("reading {}", input.display()))?;

            let mut b = Renderer::builder().strict_fonts(strict_fonts);
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

            let mut page = Page {
                margin_mm,
                ..Page::A4
            };
            if landscape {
                page = page.landscape();
            }
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
