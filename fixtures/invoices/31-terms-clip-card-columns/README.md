# 31 — terms in one clipping card with a header strip, in two columns, with inline icons and a logo

The invoice of fixture 30 whose "Obchodní podmínky" card is Tailwind's card idiom: `overflow-hidden rounded-lg border border-gray-300`
with a grey header strip on top (`<h3 class="bg-gray-100 px-3 py-2 …">`, which the rounded corners clip) and the 20 clauses in a padded body.
It sits in a `columns: 2` section and is taller than a page in a single column (about 2400 px at the column width), so it starts in column 1
of page 1, goes on in column 2, then in both columns of page 2, and closes above the signature. Seven clause headings and five paragraphs hold
an inline heroicon `<svg class="inline icon">`, and clause 3 an inline `<img>` logo (`display: inline`, Tailwind's preflight makes `img` a block).

Exercises (css-break-3 §2: only a clip box with a block size of its own is monolithic, an auto-height one is fragmented like any block;
§4.1 / §4.4 breaks between line boxes, an inline box belongs to its line; §5.4 `box-decoration-break: slice`; css-overflow-3 §3 the clip is the
padding box of each fragment; css-multicol-1 §7.1 / §8.2):
- every clause is printed once, in order, the signature after the last one (938 words like Chrome's),
- the header strip is clipped to the rounded top corners in column 1 of page 1 only; no strip, border or radius at the top of column 2 and page 2,
- the side borders and the card's clip run down both columns of both pages, the bottom border and radius are at the end of column 2 of page 2 only,
- every icon is drawn once, beside its word, and the logo once, in its line,
- 2 pages like Chrome's.

Not exercised: a clip box with a block size of its own (`h-28 overflow-hidden`, F198g), an absolutely positioned or floating child (F198e / F198f),
a table in the card (F174c), `orphans` / `widows` (set to 1: fulgur does not implement them in columns).

**On main's binary** the clip card is one box placed whole in column 1: its text runs below the page edge (not printed), column 2 is empty:
325 of the 938 words are printed; compare 9.5 / 7.5 %, 2 pages. With the fix 938 words, 9.6 / 6.6 % (Chrome 2 pages).

The icon and logo rules are in the fixture's `<style>` (`.icon` is what `size-3` would be); `fixtures/tailwind/tailwind.css` is not rebuilt.
The tests that pin the behaviour without a fixture are in `crates/htmltopdf/tests/multicol_boxes.rs`; `fixtures.rs` has
`terms_clip_card_columns_invoice_prints_every_clause`. Plan: `docs/plans/2026-10-29-f198b-multicol-box-contents.md`.

Compare: 9.6 / 6.6 % (2 pages, Chrome 2): the text drift every fixture has, over two pages of small text in columns. Lines that hold an icon are 17 px
tall in Parley and 16.8 in Chrome (F198h), so a column can end a line early; the page breaks still fall at the same clauses as Chrome's.
