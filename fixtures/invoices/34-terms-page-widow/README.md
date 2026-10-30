# 34 — terms that overflow their page by half a line: Chrome moves the last two clauses to the next page

Three-page Czech invoice. Page 1 is the header, party grid, items table, "Celkem k úhradě" and the "Platební údaje" block of 32 (without
the note card). Page 2 starts with `break-before: page` on a plain `<div class="text-xs text-gray-600" style="line-height: 20px">` that
holds the title in `<strong>` and fifty numbered one-line clauses, each ending in `<br>` except the last: 51 lines of 20 px, 1020 px
(an ERP template that pours its terms into one block). Then `<p class="mt-6 text-sm">Převzal: …</p>`.

Exercises:
- css-break-3 §4.4 (unforced breaks): a break between lines needs at least `orphans` lines before it and `widows` lines after it
  (2 and 2), and the rules are relaxed only when no break keeps the content in the fragmentainer. The block is 10.9 px taller than
  the 1009.13 px content box, so one line does not fit; a break before the last line would leave a widow of one, so the break is
  pulled back one line: 49 lines on page 2, clauses 49 and 50 on page 3, the signature below them (Chrome: "49." at 44.5 pt,
  "Převzal:" at 91.8 pt).
- 3 pages like Chrome's.

**On main's binary:** the line path skipped a break that left a one-line widow instead of pulling it back, so page 2 held all 51
lines, clause 50 ended at 805 pt (the content box ends at 799.4 pt, 6 pt of it in the bottom margin) and only the signature was on page 3.

The tests are in `crates/htmltopdf/tests/sub_line_overflow.rs` (the bare shapes) and `fixtures.rs`. Every class is in the compiled
`fixtures/tailwind/tailwind.css`. Plan: `docs/plans/2026-10-29-f30f-l-sub-line-overflow.md`.
