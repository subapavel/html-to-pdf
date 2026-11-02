# 38 — terms as two body-level paragraphs, a print sheet asks for `orphans: 3`

Two-page Czech invoice. Page 1 is the header, party grid and an items table of nine rows, "Celkem k úhradě", then
`<h2 class="text-sm font-semibold mb-2">Obchodní podmínky</h2>` and two body-level paragraphs `<p class="text-xs text-gray-600
leading-relaxed">`: clauses 1-25 (about 15 lines of 19.5 px) and clauses 26-50 (about 16 lines), then `<p class="mt-6 text-sm">Převzal: …</p>`.
The inline style sheet holds `@media print { p { orphans: 3; widows: 3; } }`, the usual line of a print sheet.

The items table is sized so that the first terms paragraph ends on page 1 with room for 2.3 lines (ours; Chrome 2.4) under it: the room
is 5 pt from the two-line boundary and 10 pt from the three-line one, clear of the 1 px difference between our page capacity and
Chrome's (F30f-c).

Exercises:
- css-break-3 §3.3, §4.4: a break between line boxes needs at least `orphans` lines before it. The second paragraph has room for two lines
  on page 1, which `orphans: 2` (the initial value) accepts and `orphans: 3` does not; there is no break on that page that keeps three
  lines, so the paragraph moves to the next one (the break before it is the allowed one, §4.1). Chrome: page 1 ends with clause 25 ("odpovídá"),
  page 2 starts with "26.", 2 pages. The same document without the declaration puts two lines of clause 26 at the bottom of page 1, which is
  what the fulgur fork did before it read the property.
- The declaration comes from a `<style>` block inside `@media print`; the fork's reader (`column_css`) sees it as the author's rule.

**On the binary before the change:** `orphans` / `widows` were not read, so the paragraph started on page 1 with two lines (clause 26 and the
beginning of 27 at the bottom, page 2 starting with "samostatnou") and the signature was 28 pt higher than in Chrome.

The test is `terms_print_orphans_invoice_moves_the_second_paragraph_to_page_two` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`orphans_widows.rs`. Every class is in the compiled `fixtures/tailwind/tailwind.css`. Plan: `docs/plans/2026-11-02-f30c-o-orphans-widows.md`.
