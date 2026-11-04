# Roadmap

| Milestone | Goal | Done when |
|---|---|---|
| **M0 — oracle** | Five invoice fixtures with Chrome reference renders, pixel-diff script | `python3 scripts/compare.py` prints a number per fixture ✔ |
| **M1 — first light** | `htmltopdf render` produces PDFs for all fixtures; fonts embedded; 01 within a few % of Chrome | CI green, 01 diff under threshold |
| **M2 — pagination** | 02 matches Chrome page breaks, repeated `thead`, "Strana X / Y" | 02 diff under threshold on all 3 pages |
| **M3 — tables & inline** | `border-collapse`, inline badges (carried patches in our forks) | 03 diff under threshold |
| **M4 — colour & effects** | `oklch` conversion verified, gradients, shadows, `print:` variants | 04, 05 within agreed tolerance |
| **M4b — pilot-ready** | Ordinary Czech/EU invoices can be sent to real customers | every bullet of *Pilot bar* below holds |
| **M5 — product** | Java (Panama) + Node bindings, HTTP service template+JSON→PDF, PDF/A-3 with ISDOC attachment, QR Platba helper | first real invoice sent to a real customer |
| **M6 — autonomy** | Cloud routines: nightly triage, one-fix-per-night, read-only review | a week of nightly PRs with no human babysitting |

*Owner decision 2026-10-27: a 200-page table of items (`bench/corpus/b200-items.html`) at about 1.2 s on the dev VM is
accepted; the 1 s goal stays for other 200-page documents. Table speed work is no longer a priority (F99).*

*Owner decision 2026-10-26: PDF/A-3 and e-invoicing (ISDOC / ZUGFeRD attachments) are not needed for now; do not plan them until the owner asks.*

Thresholds are set per fixture in `scripts/compare.py` once M1 lands and we see real numbers.

## Pilot bar

*Owner decision 2026-11-03.* The library is good enough for real invoices (M4b) when all of these hold:

1. Every invoice fixture (`fixtures/invoices/`) and every `bench/corpus/` document matches Chrome within its
   `scripts/compare.py` threshold, with Chrome's page count (except the fixtures in `PAGE_COUNT_MAY_DIFFER`).
2. No finding of severity high, medium-high or critical is open.
3. No open medium finding loses, hides or garbles text on an invoice-like layout: item tables, cards, flex / grid
   headers, running headers and footers, a different first page. Shapes an invoice does not use (multi-column text,
   tagged-PDF structure, PDF/A, synthetic stress probes) do not count.

Closing every finding is not the goal: validators keep recording edge cases from synthetic probes (ADR-0011).

### Open findings that block the pilot (first classification, 2026-11-03)

Re-checked whenever one of them is closed or a new medium finding is filed. The findings queue keeps its own order;
these go before anything that is not on this list.

| Finding | Why it blocks |
|---|---|
| F213e-b | a first `break-inside: avoid` card under a letterhead margin hangs below the content box and loses lines |
| F213e-c | the same card after a zero-height sibling (a `counter-reset` div) hangs the same way |
| F217 | a fixed-size QR box (`inline-block w-28 h-28`) in a flex payment row does not count in the row height; the next block is painted over it |
| F30h | rows of several items around a split paragraph |
| F35 | text of cells next to a split cell (`vertical-align: middle / bottom`) is lost |
| F38 | first content taller than page 1's room goes off the page |
| F88 | a paragraph with large vertical padding loses most of its lines, silently |
| F46 | an inline `<img>` in a split paragraph is dropped and two lines merge |
| F109 | a `:first` page box different from later pages clips or loses content |
| F24 | a `width: 100%` table overflows its card's padding |
| F129 | free table width is split equally, not by content: column positions of most item tables |
| F130 | columns with an explicit `width` are not stretched |
| F47 | a single-side cell border (`border-b`) can be painted black |
| F203 | negative vertical margins (`-mt-*`) are ignored |
| F208 | horizontal padding / margin / border of an inline element (badge `px-2`) moves no text |
| F120 | an `inline-flex` badge is laid out at max-content width |
| F66g | a wide `nowrap` table loses its right columns at the page edge (warned) |
| F75 | text in a family with no face is dropped |
| F101 / F44 | a glyph missing from every bundled font prints `.notdef` without a warning |
| F17 | non-sRGB colours in SVG logos paint black |

F107 was done on 2026-11-04 (a logo `<img>`, PNG or SVG, in a running header is painted in the margin box, fixture 47); it was taken off the list. F213e was done on 2026-11-03 (#93); its leftovers F213e-b and F213e-c took its place. F213f was on the first list and was taken off on 2026-11-03: no invoice-like flex row loses words (its plan §7); the remaining shape needs a multi-column item and went to low-medium.

Not on the list on purpose: tagging / PDF/UA (F85, F123, F198k, F200, F42), PDF/A (F62, owner decision 2026-10-26),
speed of long tables (F45, owner decision 2026-10-27), multi-column edge shapes (F198*, F173, F174*, F193, F30d-*),
and medium findings whose repro is a synthetic stress shape.
