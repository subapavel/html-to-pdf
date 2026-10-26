# 25 — a payment schedule table inside two-column terms, 2 pages

A two-page Czech invoice (the header, parties, items and total of fixture 12) whose "Obchodní podmínky" are a
`<section class="terms">` with `columns: 2; column-gap: 24px`: clauses 1–4, a "Splátkový kalendář" heading and a
`table.plan` (collapsed borders, a `thead` Splátka · Splatnost · Částka, 56 rows, zebra rows), then clauses 5–8.

Chrome breaks the table between its rows across the columns and the page and repeats its header at the top of each
column piece: Splátka 1–14 at the bottom of column 1 of page 1, 15–42 in column 2, 43–56 at the top of column 1 of
page 2, then the clauses.

Exercises (F174; css-break-3 §2 and §4.1, css-multicol-1 §7.1 / §8.2, CSS 2.1 §17.2):
- a table in a multicol container is cut between its rows by the column flow, not placed whole. **On main's binary**
  (`e7ee896`) the table is one box of the F30d flow: placed whole in column 1, it runs past the bottom of page 1 and
  most of its rows are not printed (fulgur warns).
- the header row is repeated at the top of every column piece; row backgrounds and collapsed borders are drawn once
  per piece, at the piece's column.

Not exercised: margins inside the columns (F30d-b: the fixture sets none), `orphans` / `widows` (set to 1), a table
that is not as wide as its column (F174b: tables in columns are laid out at the column width; this one is `width:
100%`), a table inside a wrapper in the columns (F174c).

Everything multicol and table is in the fixture's `<style>`; `fixtures/tailwind/tailwind.css` is not rebuilt. Tests
that pin the behaviour without a fixture: `crates/htmltopdf/tests/multicol_tables.rs`; `fixtures.rs` has
`terms_table_columns_invoice_prints_every_instalment`.
