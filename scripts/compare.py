#!/usr/bin/env python3
"""Pixel-diff htmltopdf output against the Chrome reference for every fixture.

    python3 scripts/compare.py [fixture ...] [--threshold-check]

Reads  out/<fixture>/htmltopdf-p<N>.png   (from scripts/render-fixtures.sh)
       fixtures/invoices/<fixture>/reference/chrome-p-<N>.png
Writes out/compare/<fixture>-p<N>.png      side-by-side: ours | chrome | diff heatmap
       out/compare/summary.json

Metric: fraction of pixels whose max channel difference exceeds 32/255, after both pages
are padded to the same size. Page-count mismatch is reported separately and counts as failure.
Chrome is the reference, not the truth; a diff is a question, not automatically a bug.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "out"
CMP = OUT / "compare"
PIXEL_TOL = 32  # per-channel, 0..255
# Per-fixture acceptance (fraction of differing pixels on the worst page). These are a REGRESSION
# GUARD: set at the current number plus a small margin, and lowered in the same PR as the fix that
# earns it. Never raise one to make CI green; fix the fixture or the engine instead.
THRESHOLDS = {
    # Lowered in F22 (collapsed table borders: Blitz lays tables out with `border-collapse: collapse`, fulgur paints the
    # resolved lines; every ruled row used to be short by its rule's width). Worst page, main -> F22 (threshold = new + 1
    # point): 01 4.4 -> 3.0, 02 11.1 -> 4.8 (19 rows on page 1 like Chrome), 03 5.7 -> 4.7, 04 5.7 -> 3.8, 05 3.5 -> 3.3,
    # 06 15.9 -> 11.8, 07 7.2 -> 3.7, 08 13.2 -> 12.1, 09 5.1 -> 4.0, 10 5.9 -> 5.2, 11 6.9 -> 3.1, 12 14.6 -> 8.8,
    # 14 7.5 -> 3.9, 15 8.7 -> 4.5, 17 12.0 -> 5.7, 18 5.3 -> 3.1, 20 8.9 -> 3.3 %. The worst pages of 13, 16 and 19 did
    # not move (no collapsed table on them; 13 p1 1.9 -> 1.5 and 16 p1 16.6 -> 9.5 did). What is left is text drift and, on 03 and 20, grid lines that sit 1 px off Chrome's.
    # Lowered in F1 (sRGB colour conversion): 01 5.3 → 4.4, 02 10.6 → 10.4, 03 10.1 → 8.9, 04 9.9 → 5.8,
    # 05 32.3 → 21.5 %.
    "01-basic": 0.04,
    # 02: re-baselined by F5 (4 → 3 pages, ADR-0003); F12 (repeated thead) 10.3 / 11.8 / 9.6 → 10.3 / 11.5 / 9.9 %
    #     (p3 grew 0.3: the header band is content that was missing). Rows still drift against Chrome's because
    #     collapsed `<tr>` borders are not painted (Blitz #386, F22). F13 (vertical-align) 10.3 / 11.5 / 9.9 →
    #     10.2 / 11.4 / 9.8 %. F6 (`@page` margin honoured: the document's 20 mm bottom margin, not the renderer's 15 mm)
    #     10.2 / 11.4 / 9.8 -> 10.2 / 11.1 / 9.5 %; 16 words move from page 2 to page 3 and the footer sits in the 20 mm band like
    #     Chrome's (ours is not centred vertically in it, F103). Worst page + 1 point.
    #     F103 + F110 (margin-box content aligned in its box: the footer is centred in the 20 mm band, `Strana n / 3` ends at the
    #     right content edge, both where Chrome puts them) 4.84 / 4.76 / 4.58 -> 4.61 / 4.53 / 4.35 %. Worst page + 1 point.
    "02-long": 0.057,
    # Lowered in F5 (table column widths): 03 8.9 → 8.2, 04 5.8 → 5.7 %; F19 (row backgrounds) 03 8.2 → 6.2 %.
    # F9 (<img> natural size: the logo renders) 03 6.2 → 5.7 %.
    "03-logo-qr": 0.058,
    "04-bilingual-vat": 0.049,
    # 05: F6 (`@page { size: A4; margin: 0 }` honoured: the header band is full-bleed like Chrome's instead of printed 15 mm
    #     inside the page) 21.6 -> 3.5 %. The rest is the text drift every fixture has and the shadow under the totals box.
    #     Worst page + 1 point.
    "05-fancy": 0.044,
    # 06: new in F30 (a paragraph taller than a page inside a bordered box). 15.9 / 12.5 %: one dense paragraph, so
    #     the diff is the line-wrapping drift against Chrome's text metrics over ~100 lines, not layout; the page
    #     break itself matches (page 1 filled, 2 pages like Chrome). Chrome also leaves the box open at the break,
    #     we draw a border there (box-decoration-break is not implemented).
    #     F33 (outside bullets painted; the fixture uses the default outside disc again): p2 12.5 -> 11.8 %, p1 15.9 % unchanged,
    #     so the worst page and the threshold stay.
    "06-long-notes": 0.129,
    # 07: new in F29 (page 0 starts 32 px down because the first child's `mt-8` collapses through <body>). 7.2 / 7.1 %:
    #     page 1 holds 21 rows to Chrome's 20 (row heights drift by well under a pixel and add up over 20 rows); without
    #     F29 a 22nd row hung 13 pt below the content box. The page break itself moves like Chrome's.
    "07-heading-margin": 0.047,
    # 08: new in F30b (a paragraph taller than a page that is a grid item, `grid gap-4` card, and a flex-row footer).
    #     13.2 / 12.1 %: same cause as 06, one dense paragraph whose line wrapping drifts against Chrome's over ~100
    #     lines; the page break matches (2 pages like Chrome, main printed 3 pages and cut the paragraph after clause 21). Ours also draws the
    #     card's top border again on page 2 (box-decoration-break is not implemented). Worst page + 1 point.
    "08-terms-grid": 0.132,
    # 09: new in F48 (`h-12` on every `td`, four descriptions wrapping to 3 lines). 5.1 / 4.9 / 0.7 %: the rest is text
    #     drift and the `<tr>` row rules Chrome paints and we do not (Blitz #386, F22); the page count and every row's
    #     page match (3 pages; before F48: 2 pages, wrapped rows overlapping the next row, 6.4 / 6.7 / 0.6 %).
    #     Worst page + 1 point.
    "09-fixed-row-height": 0.051,
    # 10: new in F50 (items as `tr.block` / `td.block` cards with a `td.flex justify-between` line, totals as `td.flex`).
    #     5.9 / 1.5 %: before F50 the items and totals were not painted at all (1 page instead of Chrome's 2, 5.1 / 1.2 %
    #     of mostly empty page). The rest is text drift (cards come out 0.5 px shorter than Chrome's, adding up over 10
    #     cards) and the card separator lines Chrome paints as `<tr>` borders and we do not (Blitz #386, F22).
    #     Worst page + 1 point.
    "10-stacked-items": 0.063,
    # 11: new in F59 (section, VAT, total and notes rows are `<td colspan="99">` in a four-column items table). 6.9 %
    #     (before F59: 8.1 %, the description column collapsed and the header row stopped at 405 px): the columns
    #     now stand within 0.25 pt of Chrome's and the page count matches. The rest is text drift, rows that come out
    #     1-2 px taller than Chrome's (they add up over 9 rows) and the cell rules Chrome paints as collapsed borders
    #     (Blitz #386, F22). Worst page + 1 point.
    "11-full-width-rows": 0.042,
    # 12: new in F30d (a two-column terms block, `columns:2` with `<h3>` / `<p>` children, 1.3 pages tall under the items
    #     table). 14.6 / 7.2 %: before F30d the block printed no text (every column was one slice taller than a page and
    #     the renderer skipped it). Both pages hold the same clauses in the same columns as Chrome up to the last page's
    #     column split, where Chrome keeps clause 21 together (widows / orphans = 2 in columns, F30d-b) and we cut it
    #     after a line. The rest is the text drift every fixture has (headings and rows about 3 px lower than Chrome's).
    #     Worst page + 1 point.
    "12-terms-columns": 0.099,
    # 13: new in F83 (one body-level `break-inside: avoid` paragraph of 37 clauses, 1.3 pages tall, under the items table).
    #     1.9 / 11.7 / 1.9 %: on main the strip slicer printed 1158 of its 1167 words (the line straddling the page cut was
    #     dropped); now all 1167. The paragraph starts at the top of page 2 like
    #     Chrome's, breaks after the same word at the bottom of page 2 and ends at the same line of page 3. The rest is the
    #     text drift every fixture has (page 2 is a dense paragraph, a line of drift part-way down). Worst page + 1 point.
    #     Since F87 the paragraph carries the Tailwind class `break-inside-avoid` instead of an inline style; the PDF is byte-identical.
    "13-terms-keep-together": 0.127,
    # 14: new in F83b (a `break-inside-avoid` wrapper with the totals, the payment details and the signature, about 300 px,
    #     after an items table that ends about 170 px above the page-1 content bottom). 7.5 / 1.6 %: on main the wrapper
    #     was entered and split (totals table under the items on page 1, payment and signature on page 2: 7.8 / 1.9 %);
    #     now it starts page 2 whole like Chrome's. The rest is the text drift every fixture has (rows about 3 px lower
    #     than Chrome's, adding up over 8 rows) and the `border-t-2` Chrome paints on the total's `<tr>` and we do not
    #     (Blitz #386, F22). Worst page + 1 point.
    "14-totals-keep-together": 0.05,
    # 15: new in F30g (an invoice in one `position:relative` `<main>` with an absolutely positioned "Zaplaceno" stamp at its
    #     top and a payment note at its bottom, the items table spanning the page break). 8.7 / 4.9 %: on main's binary the
    #     stamp and the note were not printed at all (8.9 / 4.8 %); now both are, in Chrome's places. The rest is the text
    #     drift every fixture has, the totals table starting about 20 px above Chrome's and the `border-t-2` Chrome paints
    #     on the total's `<tr>` (Blitz #386, F22). Worst page + 1 point.
    "15-paid-stamp": 0.055,
    # 16: new in F30f (an `overflow-hidden rounded-lg border` card whose one long paragraph is a direct child and crosses the
    #     page break, the shape of 06 with the clip added). 16.6 / 20.8 %, 2 pages like Chrome's: on main's binary the paragraph
    #     was placed whole, its tail was lost (the warning says 307 px below the content box), the payment block moved to a
    #     third page: 3 pages, 11.4 / 25.0 / 0.9 %. The diff is dense text: ours repeats the card's top border and padding
    #     on page 2 (`box-decoration-break: slice` is not implemented, F96), so every line of page 2 sits about 20 px lower
    #     than Chrome's, and the drift every fixture has. Worst page + 1 point; F96 brings it down.
    "16-terms-card": 0.218,
    # 17: new in F102 (a running footer, `position: running(paticka)` in an inline `<style>`, printed by `@bottom-center` on
    #     both pages; Chrome ignores `running()`, so its reference prints the same two halves from `@bottom-left` /
    #     `@bottom-right` strings, see the fixture's README). 12.0 / 10.0 %, 2 pages like Chrome's. The diff is the text drift
    #     every fixture has and the `border-b` Chrome paints on every item row (`<tr>` borders, Blitz #386 / F22; ours rows are
    #     about 0.5 px shorter, so page 1 has 24 rows against Chrome's 23); the footer itself is once per page, in the margin.
    #     On main's binary the footer was painted in the body as well. Worst page + 1 point; F22 brings it down.
    #     F103 (the footer block is centred in its 15 mm band, not on its top edge) 5.72 / 4.64 -> 5.68 / 4.59 %. Worst page + 1 point.
    #     Little moves: the footer is two wrapped lines here (a flex container in a third of the band) against Chrome's one line
    #     of two strings, so only the vertical position approaches Chrome's, not the shape (the fixture README, reference shape).
    "17-running-footer": 0.067,
    # 18: new in F6 (an A5 receipt, `@page { size: A5; margin: 10mm }` with a `@bottom-center` string, one page; Chrome's page is
    #     420 x 594.96 pt, ours 419.53 x 595.28, both 560 x 794 px). 5.3 %: the text drift every fixture has, the `border-b` rules
    #     Chrome paints on the item rows (`<tr>` borders, Blitz #386 / F22) and the footer, which Chrome centres in the margin box
    #     while ours starts at its left edge and sits about 11 px higher (centre margin box alignment, F110, and vertical centring, F103). On main's binary the page was
    #     A4 with 15 mm margins. Worst page + 1 point.
    #     F103 + F110 (the footer is centred in its box both ways, in Chrome's place) 3.10 -> 2.86 %. Worst page + 1 point.
    "18-a5-receipt": 0.039,
    # 19: new in F111 (a plain-text invoice: 85 lines of text and `<br>` directly in `<body>`, no block around them, 2 pages like
    #     Chrome's; the shape of e-mail and ERP templates). 3.1 / 2.8 %: only the text drift every fixture has (glyph edges), page 2
    #     starts with the same line as Chrome's (`PLATEBNÍ ÚDAJE`). On main's binary this printed 1 page with the first 50 lines
    #     and no warning (the other 35 were lost). Worst page + 1 point.
    "19-plain-text-invoice": 0.041,
    # 20: new in F36 (an items table written `thead, tfoot, tbody`, the HTML 4.01 order, with the dark total row in the footer, one
    #     page like Chrome's). 8.9 %: on main's binary the totals were printed straight under the column headings and the 8 items
    #     below them (12.4 %). The rest is the text drift every fixture has and the 11 rows of the table, each about 0.9 px taller
    #     than Chrome's because collapsed `<tr>` borders are not painted (Blitz #386 / F22), which adds up to 7 px at the last item.
    #     Worst page + 1 point; F22 brings it down.
    "20-tfoot-first": 0.044,
    # 21: new in F150 / F153 (a Czech invoice whose address blocks, item-grid rows, notes and list items sit in
    #     `display: contents` wrappers, one page like Chrome's). 3.7 %: the text drift every fixture has. On main's binary
    #     the item grid, the notes, the first two terms and the supplier's lines were lost and the rest was misplaced
    #     (7.4 %, though most of the page was missing). Worst page + 1 point.
    "21-display-contents": 0.047,
    # 22: new in F154 (a Czech invoice whose labels are written `<b>Splatnost: </b>2. 11. 2026`, with spaces at the inner edges
    #     of `<strong>`, `<em>`, `<a>` and `<span>`, a `::before` / `::after` label, `pre-line` notes and NBSP amounts; one page like
    #     Chrome's). 3.5 %: the text drift every fixture has. On main's binary every label was glued to its value (`Splatnost:2. 11.`,
    #     `portálu zákazníkanebo`; 4.0 %). Worst page + 1 point.
    "22-inline-spacing": 0.046,
    # 23: new in F163 (a Czech invoice whose items table has a block `::after` note and whose `display: table` VAT recap has a
    #     table-cell `::before` and `::after` around bare cells; one page like Chrome's). 3.46 %: the text drift every fixture has
    #     and the explicit column widths (Chrome 4 to 20 px off per column). Main's binary panics on it (`EnginePanic`, no PDF);
    #     a build with only the crash fixed (Blitz commit A) prints none of the generated texts. Worst page + 1 point.
    "23-table-pseudo": 0.045,
    # 24: new in F161 / F165 (a Czech invoice whose terms are a `columns:2` block with two `column-span: all` headings over two
    #     pages, then a `display: contents` signature; two pages like Chrome's). 8.6 %: the text drift every fixture has, over a
    #     page of small text in columns. Main's binary loses 86 of the 948 words. Worst page + 1 point.
    "24-columns-span": 0.096,
    # 25: new in F174 (a Czech invoice whose two-column terms hold a 56-row payment schedule that Chrome breaks between its
    #     rows across the columns and the page, repeating its header in each of the 3 column pieces; two pages like Chrome's).
    #     8.3 %: the text drift every fixture has. Main's binary places the table whole in column 1 and loses most of its rows.
    #     Worst page + 1 point.
    "25-terms-table-columns": 0.093,
}
# Fixtures whose page count is known to differ from Chrome (open finding). Remove the entry in the
# PR that fixes it; a mismatch anywhere else fails the check.
PAGE_COUNT_MAY_DIFFER: dict[str, str] = {}


def pages(dir_: Path, prefix: str) -> list[Path]:
    return sorted(dir_.glob(f"{prefix}*.png"), key=lambda p: int("".join(c for c in p.stem.split("-")[-1] if c.isdigit()) or 0))


def pad_to(im: Image.Image, w: int, h: int) -> Image.Image:
    if im.size == (w, h):
        return im
    canvas = Image.new("RGB", (w, h), "white")
    canvas.paste(im, (0, 0))
    return canvas


def diff_fraction(a: Image.Image, b: Image.Image) -> tuple[float, Image.Image]:
    w, h = max(a.width, b.width), max(a.height, b.height)
    a, b = pad_to(a.convert("RGB"), w, h), pad_to(b.convert("RGB"), w, h)
    d = ImageChops.difference(a, b)
    # max over channels, then threshold
    mask = d.convert("L").point(lambda v: 255 if v > PIXEL_TOL else 0)
    # the L conversion averages channels; redo with per-channel max for accuracy
    r, g, bb = d.split()
    mx = ImageChops.lighter(ImageChops.lighter(r, g), bb)
    mask = mx.point(lambda v: 255 if v > PIXEL_TOL else 0)
    frac = (sum(mask.histogram()[1:])) / (w * h)
    heat = Image.new("RGB", (w, h), "white")
    heat.paste(Image.new("RGB", (w, h), (220, 38, 38)), mask=mask)
    return frac, heat


def label(im: Image.Image, text: str) -> Image.Image:
    bar = Image.new("RGB", (im.width, 22), (17, 24, 39))
    ImageDraw.Draw(bar).text((6, 4), text, fill="white", font=ImageFont.load_default())
    out = Image.new("RGB", (im.width, im.height + 22), "white")
    out.paste(bar, (0, 0))
    out.paste(im, (0, 22))
    return out


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    check = "--threshold-check" in sys.argv
    CMP.mkdir(parents=True, exist_ok=True)
    names = args or sorted(p.name for p in (ROOT / "fixtures/invoices").iterdir() if (p / "index.html").exists())
    summary: dict[str, dict] = {}
    failed = False

    for n in names:
        ours = pages(OUT / n, "htmltopdf-p")
        ref = pages(ROOT / "fixtures/invoices" / n / "reference", "chrome-p")
        if not ours:
            print(f"{n:18} no output — run scripts/render-fixtures.sh {n}")
            failed = True
            continue
        per_page = []
        for i in range(max(len(ours), len(ref))):
            a = Image.open(ours[i]) if i < len(ours) else Image.new("RGB", Image.open(ref[i]).size, "white")
            b = Image.open(ref[i]) if i < len(ref) else Image.new("RGB", a.size, "white")
            frac, heat = diff_fraction(a, b)
            per_page.append(frac)
            w, h = max(a.width, b.width), max(a.height, b.height)
            sheet = Image.new("RGB", (w * 3 + 20, h + 22), (156, 163, 175))
            sheet.paste(label(pad_to(a.convert("RGB"), w, h), f"{n} p{i+1}  htmltopdf"), (0, 0))
            sheet.paste(label(pad_to(b.convert("RGB"), w, h), "chrome (reference)"), (w + 10, 0))
            sheet.paste(label(heat, f"diff {frac*100:.2f}%  (>{PIXEL_TOL}/255)"), (2 * w + 20, 0))
            sheet.save(CMP / f"{n}-p{i+1}.png")
        worst = max(per_page)
        thr = THRESHOLDS.get(n, 0.10)
        pages_ok = len(ours) == len(ref) or n in PAGE_COUNT_MAY_DIFFER
        ok = pages_ok and worst <= thr
        summary[n] = {"pages_ours": len(ours), "pages_ref": len(ref), "diff_per_page": per_page, "worst": worst, "threshold": thr, "ok": ok}
        flag = "ok " if ok else "FAIL"
        print(f"{n:18} pages {len(ours)}/{len(ref)}  diff " + " ".join(f"{p*100:5.1f}%" for p in per_page) + f"   worst {worst*100:.1f}% (thr {thr*100:.0f}%)  {flag}")
        failed |= not ok

    (CMP / "summary.json").write_text(json.dumps(summary, indent=2))
    print(f"\nside-by-sides in {CMP.relative_to(ROOT)}/")
    return 1 if (check and failed) else 0


if __name__ == "__main__":
    sys.exit(main())
