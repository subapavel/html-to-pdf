# 22 — whitespace at the inner edges of inline elements, 1 page

A one-page Czech invoice written the way templates emit it: the space after a label's colon is part of the element
(`<b>Splatnost: </b>2. 11. 2026`, `<strong>IČO: </strong>12345678`), a link ends in a space
(`<a>portálu zákazníka </a>nebo`), amounts use `&nbsp;`, generated labels carry spaces
(`::before { content: "Číslo objednávky: " }`), a `white-space: pre-line` note has spans that end in a space and lines that
are indented in the source, and a few lines have a space in front of `<br>`.

Exercises (F154, F159(f), CSS Text 3 §4.1.1 and §4.1.3):
- A collapsible space that follows another one collapses with it *across* element boundaries; the first one survives, in the
  font of its own element. Spaces at the start and the end of a line, and around a forced break, are removed. NBSP is never
  collapsible.
- **On main's binary** Parley trimmed the start and the end of every inline element: every label was glued to its value
  (`Splatnost:4. 11. 2026`, `IČO:12345678`, `portálu zákazníkanebo`), the generated labels touched the text and the NBSP at the end
  of a `<b>` was deleted. Main's compare number is 4.0 %; with the fix 3.5 %.
- The space after the link is underlined like Chrome underlines it.

Everything is in the fixture's `<style>` and inline `style` attributes (the Tailwind classes of the body are the ones fixture 20
uses), so `fixtures/tailwind/tailwind.css` is not rebuilt. The tests that pin the behaviour without a fixture are
`crates/htmltopdf/tests/inline_whitespace.rs` (W1-W17, each with Chrome's gaps) and `fixtures.rs`
(`inline_spacing_invoice_keeps_its_label_spaces`).

Compare: 3.5 % (1 page, Chrome 1). The rest is text drift and the `<em>` (the bundled Inter has no italic, so it is synthesised).
