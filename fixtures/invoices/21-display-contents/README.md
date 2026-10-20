# 21 — `display: contents` component wrappers, 1 page

A one-page Czech invoice written the way component frameworks emit it (Vue, Livewire, Tailwind's `contents` class): wrapper
elements with `display: contents` that take part in no layout. They hold

- the two address blocks of the `grid` that is their parent (the blocks are the grid items),
- every row of the items table, which is a `grid` of divs (the cells are the grid items; the last row spans four columns),
- the two paragraphs of the notes, with a `::before` label on the parent and an inline wrapper that sets `color` and `font-weight`
  on the due date,
- two `<li>`s of an `<ol>` whose third item is outside the wrapper (numbering continues across the wrapper).

Exercises (F150, F153, css-display-3 §2.5, css-flexbox-1 §4, css-grid-2 §6, css-lists-3):
- An element with `display: contents` generates no box; its children generate boxes as if they were children of its parent.
  **On main's binary** a block inside a wrapper was never laid out (its text was lost: the supplier's lines, the whole items grid, the
  notes, the first two terms), a wrapper that was the only child of its parent made the parent an inline root, and mixed content
  with a whitespace text node before a wrapper panicked ("invalid key", F150). Main's compare number is 7.4 % with most of the page
  missing; with the fix 3.7 %.
- The inline wrapper's `color` / `font-weight` reach its text (Chrome prints the due date red and bold; text inside a `display: contents`
  element keeps the element's style).

Everything is in the fixture's `<style>` and inline `style` attributes (the Tailwind classes of the body are the ones fixture 20
uses), so `fixtures/tailwind/tailwind.css` is not rebuilt. The tests that pin the behaviour without a fixture are
`crates/htmltopdf/tests/display_contents.rs` (C1-C16 and a seeded fuzz test) and `fixtures.rs`
(`display_contents_invoice_prints_everything_the_wrappers_hold`).

Compare: 3.7 % (1 page, Chrome 1). The rest is text drift and the monospace `dd` of the payment block: Chrome bolds the amount (its mono
font has a bold face), the bundled mono does not.
