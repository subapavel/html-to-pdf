# 23 — `::before` / `::after` on table boxes, 1 page

A one-page Czech invoice with generated content on two table boxes:

- the items `<table>` (`border-collapse: collapse`, `thead` / `tbody` / `tfoot`) has a block `::after` note under the totals ("Ceny jsou uvedeny v korunách českých bez DPH. DPH je rekapitulováno níže."). The pseudo box is the last child of the table, so it sits in an anonymous row and cell: as wide as the first column, as in Chrome (the note wraps in two lines),
- the VAT recap is a `display: table` box of divs with three bare `display: table-cell` children, a `table-cell` `::before` label ("Rekapitulace DPH") and a `table-cell` `::after` ("Uhrazeno"). CSS 2.1 §17.2.1 puts the label, the bare cells and the closing cell in one anonymous row.

Exercises (F163, css-pseudo-4 §3.1, CSS 2.1 §17.2.1):
- A table box's `::before` / `::after` is its first / last child and takes part in its table fixup.
  **On main's binary** the render panics (`EnginePanic`, "index out of bounds" in Blitz's table wrapper, because the recap has fewer rows than pseudo boxes) and there is no PDF. With only the crash fixed, the note, the label and the closing cell are left out of the PDF (strict mode names the recap's two).
- The explicit column widths of the items table (`style="width:…"` on the `th`s) are there on purpose: without them Chrome's automatic layout gives the note's long text to the first column and squeezes the amounts (`12 000,00` wraps), which our column algorithm does not do (cf. F129); that is not what this fixture is about.
- Single-side borders are not used in the recap (`border-left: 1px solid #e5e7eb` is painted black on any box, F171).

Everything is in the fixture's `<style>` (the Tailwind classes of the body are the ones fixtures 20-22 use), so `fixtures/tailwind/tailwind.css` is not rebuilt. The tests that pin the behaviour without a fixture are `crates/htmltopdf/tests/table_pseudo.rs` (T1-T17, positions measured in Chrome), `display_contents.rs` (the fuzz generator makes tables with pseudo-elements, no rows and bare cells) and `fixtures.rs` (`table_pseudo_invoice_prints_the_generated_content`).

Compare: 3.5 % (1 page, Chrome 1). The rest is text drift, the explicit column widths (our columns are 4 to 20 px off Chrome's) and the monospace `dd` of the payment block (Chrome bolds the amount).
