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
    #     collapsed `<tr>` borders are not painted (Blitz #386) and cells are not vertically centred (F13).
    "02-long": 0.125,
    # Lowered in F5 (table column widths): 03 8.9 → 8.2, 04 5.8 → 5.7 %.
    "03-logo-qr": 0.09,
    "04-bilingual-vat": 0.06,
    # 05: the rest is mostly the header printed inside the page margin instead of full-bleed (F6) and the
    #     totals box sitting lower than in Chrome.
    "05-fancy": 0.22,
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
