# 15 — "Zaplaceno" stamp and payment note on an invoice that spans two pages

Czech invoice of 16 item rows (two to four lines each) inside one `<main style="position:relative; padding-bottom:2.5rem">`:
header, parties, the items table (it spans the page break and repeats its header on page 2), totals, payment details.
Two absolutely positioned boxes hang on the `<main>`:

- the stamp, `<div class="… uppercase" style="position:absolute; top:0; right:0; border:4px solid #16a34a; …">Zaplaceno</div>`:
  top right of page 1, level with the title;
- the note, `<p style="position:absolute; bottom:0; right:0">Uhrazeno převodem dne 12. 10. 2026</p>`: the bottom of the
  `<main>`, which is on page 2, under the payment details (the padding keeps it clear of the last line).

Exercises:
- an absolutely positioned box in a containing block that the fragmenter enters and splits across pages (F30g;
  CSS 2.1 §10.1 and §10.6.4: a box is positioned against its containing block as if it were not fragmented, css-position-3).
  **Before F30g (main) the fragmenter skipped both boxes (out-of-flow children are skipped, and nothing else placed them), so
  the stamp and the note were missing from the PDF.** Compare numbers: main's binary 8.9 / 4.8 %, F30g 8.7 / 4.9 %
  (the stamp is back and level with Chrome's, which is the 0.2 point on page 1; the note is back and 32 pt under the last line
  like Chrome's, which adds 0.1 point on page 2 because every glyph of it is a few px off Chrome's text; both have 2 pages like Chrome's)
- `bottom: 0` locates the note from the end of the last fragment of its containing block (page 2, bottom padding included), not from the first

The rest of the diff is the text drift every fixture has and a divergence that is not F30g: the totals table starts about
20 px higher than Chrome's under the items table (its `mt-6` is partly lost after a table that splits; not analysed), and the rule above
"Celkem" is the collapsed `<tr>` border of Blitz #386.

The positions are inline `style` attributes because the compiled `fixtures/tailwind/tailwind.css` has no `absolute` / `top-0` /
`right-0` utilities and is not rebuilt (the rebuild needs npm). The border width and colours of the stamp are inline for the
same reason (`border-4`, `border-green-600` and `text-green-700` are not in the compiled sheet). Unrotated: `transform` is not
under test. The tests that pin the shapes are in `crates/htmltopdf/tests/abspos_flow.rs`.

Not exercised: an abspos box inside a transformed, clipping or multicol ancestor (left to the old model), `position: fixed`
(repeated whole on every page, CSS 2.1 §9.6.1), an abspos box under `display: flex` parents.
