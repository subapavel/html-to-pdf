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
    # Lowered in F1 (sRGB colour conversion): 01 5.3 → 4.4, 02 10.6 → 10.4, 03 10.1 → 8.9, 04 9.9 → 5.8,
    # 05 32.3 → 21.5 %.
    "01-basic": 0.05,
    # 02: re-baselined by F5 (4 → 3 pages, ADR-0003); F12 (repeated thead) 10.3 / 11.8 / 9.6 → 10.3 / 11.5 / 9.9 %
    #     (p3 grew 0.3: the header band is content that was missing). Rows still drift against Chrome's because
    #     collapsed `<tr>` borders are not painted (Blitz #386, F22). F13 (vertical-align) 10.3 / 11.5 / 9.9 →
    #     10.2 / 11.4 / 9.8 %.
    "02-long": 0.124,
    # Lowered in F5 (table column widths): 03 8.9 → 8.2, 04 5.8 → 5.7 %; F19 (row backgrounds) 03 8.2 → 6.2 %.
    # F9 (<img> natural size: the logo renders) 03 6.2 → 5.7 %.
    "03-logo-qr": 0.067,
    "04-bilingual-vat": 0.06,
    # 05: the rest is mostly the header printed inside the page margin instead of full-bleed (F6) and the
    #     totals box sitting lower than in Chrome.
    "05-fancy": 0.22,
    # 06: new in F30 (a paragraph taller than a page inside a bordered box). 15.9 / 12.5 %: one dense paragraph, so
    #     the diff is the line-wrapping drift against Chrome's text metrics over ~100 lines, not layout; the page
    #     break itself matches (page 1 filled, 2 pages like Chrome). Chrome also leaves the box open at the break,
    #     we draw a border there (box-decoration-break is not implemented).
    #     F33 (outside bullets painted; the fixture uses the default outside disc again): p2 12.5 -> 11.8 %, p1 15.9 % unchanged,
    #     so the worst page and the threshold stay.
    "06-long-notes": 0.169,
    # 07: new in F29 (page 0 starts 32 px down because the first child's `mt-8` collapses through <body>). 7.2 / 7.1 %:
    #     page 1 holds 21 rows to Chrome's 20 (row heights drift by well under a pixel and add up over 20 rows); without
    #     F29 a 22nd row hung 13 pt below the content box. The page break itself moves like Chrome's.
    "07-heading-margin": 0.082,
    # 08: new in F30b (a paragraph taller than a page that is a grid item, `grid gap-4` card, and a flex-row footer).
    #     13.2 / 12.1 %: same cause as 06, one dense paragraph whose line wrapping drifts against Chrome's over ~100
    #     lines; the page break matches (2 pages like Chrome, main printed 3 pages and cut the paragraph after clause 21). Ours also draws the
    #     card's top border again on page 2 (box-decoration-break is not implemented). Worst page + 1 point.
    "08-terms-grid": 0.142,
    # 09: new in F48 (`h-12` on every `td`, four descriptions wrapping to 3 lines). 5.1 / 4.9 / 0.7 %: the rest is text
    #     drift and the `<tr>` row rules Chrome paints and we do not (Blitz #386, F22); the page count and every row's
    #     page match (3 pages; before F48: 2 pages, wrapped rows overlapping the next row, 6.4 / 6.7 / 0.6 %).
    #     Worst page + 1 point.
    "09-fixed-row-height": 0.061,
    # 10: new in F50 (items as `tr.block` / `td.block` cards with a `td.flex justify-between` line, totals as `td.flex`).
    #     5.9 / 1.5 %: before F50 the items and totals were not painted at all (1 page instead of Chrome's 2, 5.1 / 1.2 %
    #     of mostly empty page). The rest is text drift (cards come out 0.5 px shorter than Chrome's, adding up over 10
    #     cards) and the card separator lines Chrome paints as `<tr>` borders and we do not (Blitz #386, F22).
    #     Worst page + 1 point.
    "10-stacked-items": 0.069,
    # 11: new in F59 (section, VAT, total and notes rows are `<td colspan="99">` in a four-column items table). 6.9 %
    #     (before F59: 8.1 %, the description column collapsed and the header row stopped at 405 px): the columns
    #     now stand within 0.25 pt of Chrome's and the page count matches. The rest is text drift, rows that come out
    #     1-2 px taller than Chrome's (they add up over 9 rows) and the cell rules Chrome paints as collapsed borders
    #     (Blitz #386, F22). Worst page + 1 point.
    "11-full-width-rows": 0.079,
    # 12: new in F30d (a two-column terms block, `columns:2` with `<h3>` / `<p>` children, 1.3 pages tall under the items
    #     table). 14.6 / 7.2 %: before F30d the block printed no text (every column was one slice taller than a page and
    #     the renderer skipped it). Both pages hold the same clauses in the same columns as Chrome up to the last page's
    #     column split, where Chrome keeps clause 21 together (widows / orphans = 2 in columns, F30d-b) and we cut it
    #     after a line. The rest is the text drift every fixture has (headings and rows about 3 px lower than Chrome's).
    #     Worst page + 1 point.
    "12-terms-columns": 0.156,
    # 13: new in F83 (one body-level `break-inside: avoid` paragraph of 37 clauses, 1.3 pages tall, under the items table).
    #     1.9 / 11.7 / 1.9 %: on main the strip slicer printed 1158 of its 1167 words (the line straddling the page cut was
    #     dropped); now all 1167. The paragraph starts at the top of page 2 like
    #     Chrome's, breaks after the same word at the bottom of page 2 and ends at the same line of page 3. The rest is the
    #     text drift every fixture has (page 2 is a dense paragraph, a line of drift part-way down). Worst page + 1 point.
    "13-terms-keep-together": 0.127,
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
