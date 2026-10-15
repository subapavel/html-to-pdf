# html-to-pdf

HTML + Tailwind CSS → PDF **without a browser**. Rust. First target: Czech/EU invoices.

Built on [fulgur](https://github.com/fulgur-rs/fulgur) (Blitz/Stylo/Taffy/Parley layout, Krilla PDF).
A single-page invoice renders in ~13 ms; output is byte-deterministic.

```bash
scripts/fetch-fonts.sh                       # Inter TTFs → fixtures/fonts/ (once)
cargo build --release -p htmltopdf-cli       # ~7 min cold, seconds warm
scripts/render-fixtures.sh                   # all fixtures → out/<name>/htmltopdf.pdf + PNGs
python3 scripts/compare.py                   # vs Chrome reference → out/compare/*.png
```

Status, findings and plan: [`docs/plans/2026-10-15-m0-findings.md`](docs/plans/2026-10-15-m0-findings.md),
[`docs/roadmap.md`](docs/roadmap.md). How we work (humans and agents): [`AGENTS.md`](AGENTS.md).

## Fixtures

Five invoices in `fixtures/invoices/`, each with a README naming the CSS features it exercises and a
`reference/` directory with headless-Chrome output (PDF + PNG per page). Chrome is the reference, the CSS
spec is the truth.

| | Fixture | Exercises |
|---|---|---|
| 01 | basic | flex/grid, table, diacritics, `@page` |
| 02 | long | 60 rows, pagination, repeated `thead`, "Strana X / Y" |
| 03 | logo-qr | PNG logo, inline badges, `border-collapse`, inline-SVG QR Platba |
| 04 | bilingual-vat | spans, side-by-side tables, `divide-y`, `oklch` palette |
| 05 | fancy | full-bleed gradients, shadows, rings, `print:` variants |

## Library

```rust
use htmltopdf::{Renderer, RenderOptions};
let r = Renderer::builder()
    .css_file("fixtures/tailwind/tailwind.css")?
    .font_file("fixtures/fonts/Inter-Regular.ttf")?
    .build();
let pdf = r.render(html, &RenderOptions::default())?;
```

Licence: MIT OR Apache-2.0 (same as fulgur). Inter font: SIL OFL 1.1.
