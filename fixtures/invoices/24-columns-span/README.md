# 24 — terms in two columns with `column-span: all` headings, 2 pages

A two-page Czech invoice (the header, parties, items and total of fixture 12) whose "Obchodní podmínky" are a
`<section class="terms">` with `columns: 2; column-gap: 24px` and 27 clauses (an `<h3>` and a `<p>` each), cut into
three column rows by two `column-span: all` headings:

- group A, clauses 1–6, balanced on page 1 above the first spanner. Clause 3 is a one-line paragraph ("Dodavatel je
  plátcem DPH.") followed by a long one (clause 4) that wraps across the column break: the F161 shape,
- "Reklamační řád" (spanner 1, page 1),
- group B, clauses 7–23, fills both columns to the bottom of page 1 and continues on page 2, balanced above
- "Ochrana osobních údajů" (spanner 2, page 2: a continuation page with two column rows),
- group C, clauses 24–27, balanced as the last row,
- then the signature line in a `<div style="display:contents">` (the F165 shape: the content after a multicol
  container in a contents wrapper).

Exercises (F161, F165; css-multicol-1 §6.1 `column-span`, §7.1 / §8.2 balancing and overflow into the next page,
css-break-3 §4.1 breaks between line boxes, css-display-3 §2.5):
- every word is printed once, in order. **On main's binary** (`182ffb73`) 86 of the 948 words are lost: a container
  with a spanner never reached the F30d fragmenter, and the column rows after the spanner were cut into strips.
- a paragraph that starts within a line of a column end is not cut after its first line and then overwritten.

Not exercised (F30d-b): margins inside the columns and around a spanner (spanner margins are dropped, so the spanners
have padding and the clauses have no margins), `column-rule`, `orphans` / `widows` (set to 1 in the fixture: fulgur
does not implement them in columns, and with Chrome's default 2 the column ends differ by a line), a spanner taller
than a page (a fulgur lib test covers it).

Everything multicol is in the fixture's `<style>`; `fixtures/tailwind/tailwind.css` is not rebuilt. The tests that
pin the behaviour without a fixture are `crates/htmltopdf/tests/multicol_lost_words.rs` and the multicol floor of the
`display_contents.rs` fuzz test; `fixtures.rs` has `columns_span_invoice_prints_every_clause`.

Compare: 8.6 % worst page (2 pages, Chrome 2). The column rows, spanners and page break are where Chrome puts them;
the rest is the text drift every fixture has, over a page of small text.
