# 39 — terms inside a `<section>`: the long paragraph starts where it is and splits

Two-page Czech invoice. Page 1 is the header, party grid, an items table of nine rows and "Celkem k úhradě", then
`<section class="mt-6 text-xs text-gray-600 leading-relaxed">` holding `<h2 class="text-sm font-semibold mb-2">Obchodní
podmínky</h2>`, a 2-line intro `<p class="mb-2">`, one `<p class="mb-2">` of clauses 1 to 60 as running text (about 29 lines of
19.5 px: it fits a page, not the 240 pt left on page 1) and a 5-line `<p>`; after the section `<p class="mt-6 text-sm border-t
pt-2">Převzal: …</p>`. The section has no border or padding on purpose (F96 is a different finding).

Exercises:
- css-break-3 §4.1 / §4.4: a break between line boxes (Class B) is a break opportunity at any nesting depth; a paragraph may
  start on a page when its first two lines (`orphans`) fit and two lines (`widows`) can be left for the next one, whether it is
  in `<body>` (fixture 36) or in a wrapper. Chrome: the long paragraph starts on page 1 below the intro (twelve lines, clauses 1
  to 27), "28." heads page 2 at 44.5 pt, "65." at 358.0 pt, "Převzal:" at 411.3 pt, 2 pages.

**On main's binary:** the paragraph had a wrapper, so the pagination moved it to page 2 whole (a leaf the block walk did not
split), leaving a third of page 1 empty ("28." at 278.3 pt, Chrome 44.5 pt; "65." at 591.8 pt, Chrome 358.0 pt; "Převzal:" at 645.1 pt, Chrome 411.3 pt).

The test is `terms_in_section_invoice_starts_the_long_paragraph_in_place` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes
are in `nested_start_in_place.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`. Plan:
`docs/plans/2026-11-02-f30c-n-nested-paragraph-split.md`.
